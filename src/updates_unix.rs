//! Unix release notifications; installation stays in the OS package/download flow.
use serde::Serialize;
use serde_json::{json, Value};
use std::{io::Read, sync::Mutex, time::Duration};
const REPO: &str = "velmourcnikita/omniview-legacy";
pub const COMMIT: &str = match option_env!("OMNIVIEW_BUILD_COMMIT") {
    Some(s) => s,
    None => "local",
};
#[derive(Default, Serialize)]
pub struct Status {
    pub busy: bool,
    pub message: String,
    pub available: bool,
    pub version: String,
    pub commit: String,
    pub commit_title: String,
    pub prepared: bool,
}
pub struct Updates {
    pub state: Mutex<Status>,
}
impl Updates {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(Status {
                message: "Обновления ещё не проверялись".into(),
                ..Status::default()
            }),
        }
    }
    pub fn check(&self, preview: bool) -> Result<(), String> {
        {
            let mut s = self.state.lock().unwrap();
            if s.busy {
                return Err("Проверка уже выполняется".into());
            }
            s.busy = true;
        }
        let result = (|| {
            let latest = get(&format!("commits?per_page=1"))?;
            let tag = if preview { "tags/preview" } else { "latest" };
            let release = get(&format!("releases/{tag}"));
            let mut s = self.state.lock().unwrap();
            s.commit = latest[0]["sha"].as_str().unwrap_or("").into();
            s.commit_title = latest[0]["commit"]["message"]
                .as_str()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            match release {
                Ok(r) => {
                    s.version = r["tag_name"].as_str().unwrap_or("").into();
                    let commit = r["target_commitish"].as_str().unwrap_or("");
                    s.available = if preview {
                        commit.len() == 40 && commit != COMMIT
                    } else {
                        newer(&s.version, env!("CARGO_PKG_VERSION"))
                    };
                    s.message = if s.available {
                        "Доступна новая сборка в релизах GitHub"
                    } else {
                        "Установлена актуальная версия"
                    }
                    .into();
                }
                Err(e) if e.contains("404") => {
                    s.message = "Готовых сборок пока нет".into();
                }
                Err(e) => return Err(e),
            }
            Ok(())
        })();
        let mut s = self.state.lock().unwrap();
        s.busy = false;
        if let Err(ref e) = result {
            s.message = format!("Не удалось проверить обновления: {e}");
        }
        result
    }
    pub fn prepare(&self) -> Result<(), String> {
        Err("Скачай сборку своей платформы из релизов GitHub".into())
    }
    pub fn launch(&self) -> Result<(), String> {
        Err("Замени программу после завершения сеанса".into())
    }
}
pub fn apply(_: &std::path::Path) -> Result<(), String> {
    Err("Автоматическая замена файлов этой платформы пока недоступна".into())
}
pub fn summary(updates: &Updates) -> Value {
    json!({"build_commit":COMMIT,"status":*updates.state.lock().unwrap()})
}
fn get(path: &str) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(25))
        .build();
    let response = agent
        .get(&format!("https://api.github.com/repos/{REPO}/{path}"))
        .set("User-Agent", "OmniView-Legacy")
        .call()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(2_000_001)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("Слишком большой ответ GitHub".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| {
        s.trim_start_matches('v')
            .split('.')
            .map(|n| n.parse::<u64>().ok())
            .collect::<Option<Vec<_>>>()
    };
    matches!((parse(a),parse(b)),(Some(a),Some(b))if a.len()==3&&b.len()==3&&a>b)
}
