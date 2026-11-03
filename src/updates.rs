//! GitHub release checks and a verified, opt-in portable update.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use windows_sys::Win32::Networking::WinHttp::*;

const REPO: &str = "velmourcnikita/omniview-legacy";
pub const COMMIT: &str = match option_env!("OMNIVIEW_BUILD_COMMIT") {
    Some(s) => s,
    None => "local",
};
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub busy: bool,
    pub message: String,
    pub available: bool,
    pub version: String,
    pub commit: String,
    pub commit_title: String,
    pub prepared: bool,
    #[serde(skip)]
    release: Option<Value>,
    #[serde(skip)]
    plan: Option<PathBuf>,
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
            s.message = "Проверяем GitHub…".into();
        }
        let result = (|| {
            let commits: Value = serde_json::from_slice(&get(
                &format!("https://api.github.com/repos/{REPO}/commits?per_page=1"),
                2_000_000,
            )?)
            .map_err(err)?;
            let newest = commits.get(0).ok_or("Нет сведений о коммитах")?;
            let endpoint = if preview { "tags/preview" } else { "latest" };
            let release = get(
                &format!("https://api.github.com/repos/{REPO}/releases/{endpoint}"),
                2_000_000,
            );
            let mut s = self.state.lock().unwrap();
            s.prepared = false;
            s.plan = None;
            s.commit = newest["sha"].as_str().unwrap_or("").to_string();
            s.commit_title = newest["commit"]["message"]
                .as_str()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(180)
                .collect();
            match release {
                Ok(bytes) => {
                    let r: Value = serde_json::from_slice(&bytes).map_err(err)?;
                    let tag = r["tag_name"].as_str().ok_or("В релизе нет версии")?;
                    let built = r["target_commitish"].as_str().unwrap_or("");
                    s.available = if preview {
                        built.len() == 40 && built != COMMIT
                    } else {
                        newer(tag, env!("CARGO_PKG_VERSION"))
                    };
                    s.version = tag.to_string();
                    s.message = if s.available {
                        "Доступна новая сборка"
                    } else {
                        "Установлена актуальная версия"
                    }
                    .into();
                    s.release = Some(r);
                }
                Err(e) if e.contains("HTTP 404") => {
                    s.available = false;
                    s.release = None;
                    s.message = "Готовых сборок пока нет. Последний коммит показан ниже".into();
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
        let release = {
            let mut s = self.state.lock().unwrap();
            if s.busy || !s.available {
                return Err("Сначала проверь доступную сборку".into());
            }
            s.busy = true;
            s.message = "Загружаем и проверяем файлы…".into();
            s.release.clone().ok_or("Нет релиза")?
        };
        let result = (|| {
            let arch = if cfg!(target_arch = "x86") {
                "win32"
            } else {
                "win64"
            };
            let assets = release["assets"].as_array().ok_or("Нет файлов релиза")?;
            let exe = std::env::current_exe().map_err(err)?;
            let destination = exe
                .parent()
                .ok_or("Нет папки программы")?
                .canonicalize()
                .map_err(err)?;
            let stage = destination.join(format!(".omniview-update-{}", crate::classroom::now()));
            std::fs::create_dir(&stage).map_err(err)?;
            let mut files = Vec::new();
            for name in ["OmniView.exe", "OmniView-Student.exe"] {
                let asset_name = format!("{arch}-{name}");
                let asset = assets
                    .iter()
                    .find(|a| a["name"] == asset_name)
                    .ok_or_else(|| format!("В релизе отсутствует {asset_name}"))?;
                let url = asset["browser_download_url"]
                    .as_str()
                    .ok_or("Нет ссылки файла")?;
                if !url.starts_with(&format!("https://github.com/{REPO}/releases/download/")) {
                    return Err("Посторонний адрес обновления".into());
                }
                let sha = asset["digest"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("sha256:"))
                    .filter(|s| valid_hash(s))
                    .ok_or("GitHub не предоставил SHA-256 файла")?;
                let bytes = get(url, 25_000_000)?;
                if hash(&bytes) != sha {
                    return Err(format!("Контрольная сумма не совпадает: {name}"));
                }
                let mut output = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(stage.join(name))
                    .map_err(err)?;
                output.write_all(&bytes).map_err(err)?;
                output.sync_all().map_err(err)?;
                files.push(FileHash {
                    name: name.into(),
                    sha256: sha.into(),
                });
            }
            let launch = exe
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| matches!(*s, "OmniView.exe" | "OmniView-Student.exe"))
                .ok_or("Запусти EXE под исходным именем")?
                .to_string();
            let plan = Plan {
                destination,
                parent: std::process::id(),
                launch,
                files,
            };
            let path = stage.join("plan.json");
            std::fs::write(&path, serde_json::to_vec(&plan).map_err(err)?).map_err(err)?;
            let mut s = self.state.lock().unwrap();
            s.plan = Some(path);
            s.prepared = true;
            s.message =
                "Файлы проверены. Заверши класс и нажми «Установить и перезапустить»".into();
            Ok(())
        })();
        let mut s = self.state.lock().unwrap();
        s.busy = false;
        if let Err(ref e) = result {
            s.message = format!("Обновление не установлено: {e}");
        }
        result
    }
    pub fn launch(&self) -> Result<(), String> {
        let plan = self
            .state
            .lock()
            .unwrap()
            .plan
            .clone()
            .ok_or("Сначала загрузи обновление")?;
        let helper = plan.parent().unwrap().join("OmniView.exe");
        let mut command = std::process::Command::new(helper);
        command.arg("--apply-update").arg(&plan);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000).spawn().map_err(err)?;
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct FileHash {
    name: String,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Plan {
    destination: PathBuf,
    parent: u32,
    launch: String,
    files: Vec<FileHash>,
}
pub fn apply(path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(err)?
        .canonicalize()
        .map_err(err)?;
    let stage = exe.parent().ok_or("Нет папки обновления")?;
    if path.canonicalize().map_err(err)? != stage.join("plan.json") {
        return Err("Неверный план обновления".into());
    }
    let plan: Plan = serde_json::from_slice(&std::fs::read(path).map_err(err)?).map_err(err)?;
    if stage.parent() != Some(plan.destination.as_path())
        || !stage
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".omniview-update-")
        || !matches!(
            plan.launch.as_str(),
            "OmniView.exe" | "OmniView-Student.exe"
        )
    {
        return Err("Неверная папка назначения".into());
    }
    let expected = ["OmniView.exe", "OmniView-Student.exe"];
    if plan.files.len() != 2
        || !expected
            .iter()
            .all(|n| plan.files.iter().filter(|f| &f.name == n).count() == 1)
    {
        return Err("Неполный комплект обновления".into());
    }
    for f in &plan.files {
        if !valid_hash(&f.sha256)
            || hash(&std::fs::read(stage.join(&f.name)).map_err(err)?) != f.sha256
        {
            return Err("Проверка обновления не пройдена".into());
        }
    }
    unsafe {
        use windows_sys::Win32::System::Threading::*;
        let process = OpenProcess(0x00100000, 0, plan.parent);
        if !process.is_null() {
            let result = WaitForSingleObject(process, 60_000);
            windows_sys::Win32::Foundation::CloseHandle(process);
            if result != 0 {
                return Err("Программа не завершилась за минуту".into());
            }
        }
    }
    install(&plan, stage)
}
fn install(plan: &Plan, stage: &Path) -> Result<(), String> {
    for f in &plan.files {
        let target = plan.destination.join(&f.name);
        if target.exists()
            && std::fs::symlink_metadata(&target)
                .map_err(err)?
                .file_type()
                .is_symlink()
        {
            return Err("Ссылка вместо файла программы".into());
        }
    }
    // Rename first: a running second instance prevents replacement, leaving the old set intact.
    let mut backed = Vec::new();
    for f in &plan.files {
        let target = plan.destination.join(&f.name);
        let backup = stage.join(format!("{}.previous", f.name));
        if target.exists() {
            if let Err(e) = std::fs::rename(&target, &backup) {
                for name in &backed {
                    let _ = std::fs::rename(
                        stage.join(format!("{name}.previous")),
                        plan.destination.join(name),
                    );
                }
                return Err(format!("Закрой остальные копии OmniView и повтори: {e}"));
            }
            backed.push(f.name.clone());
        }
    }
    let mut installed = Vec::new();
    for f in &plan.files {
        if let Err(e) = std::fs::copy(stage.join(&f.name), plan.destination.join(&f.name)) {
            let _ = std::fs::remove_file(plan.destination.join(&f.name));
            for name in &installed {
                let _ = std::fs::remove_file(plan.destination.join(name));
            }
            for name in &backed {
                let _ = std::fs::rename(
                    stage.join(format!("{name}.previous")),
                    plan.destination.join(name),
                );
            }
            return Err(format!("Возвращена прежняя версия: {e}"));
        }
        installed.push(f.name.clone());
    }
    if let Err(e) = std::process::Command::new(plan.destination.join(&plan.launch)).spawn() {
        for name in &installed {
            let _ = std::fs::remove_file(plan.destination.join(name));
        }
        for name in &backed {
            let _ = std::fs::rename(
                stage.join(format!("{name}.previous")),
                plan.destination.join(name),
            );
        }
        return Err(format!(
            "Новая версия не запустилась, возвращены прежние файлы: {e}"
        ));
    }
    Ok(())
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn hash(b: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, b)
        .as_ref()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn newer(tag: &str, current: &str) -> bool {
    fn parts(s: &str) -> Option<Vec<u64>> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().ok())
            .collect()
    }
    matches!((parts(tag),parts(current)),(Some(a),Some(b)) if a.len()==3&&b.len()==3&&a>b)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Handle(*mut std::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn checked(h: *mut std::ffi::c_void) -> Result<Handle, String> {
    if h.is_null() {
        Err(format!(
            "Сетевая ошибка {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(Handle(h))
    }
}
fn get(url: &str, max: usize) -> Result<Vec<u8>, String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or("Обновление требует HTTPS")?;
    let (host, path) = rest
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .ok_or("Неверный адрес")?;
    unsafe {
        let session = checked(WinHttpOpen(
            wide("OmniView-Updater/0.6").as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 20_000, 30_000);
        let connection = checked(WinHttpConnect(session.0, wide(host).as_ptr(), 443, 0))?;
        let request = checked(WinHttpOpenRequest(
            connection.0,
            wide("GET").as_ptr(),
            wide(&path).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        if WinHttpSendRequest(
            request.0,
            wide("Accept: application/vnd.github+json\r\n").as_ptr(),
            u32::MAX,
            std::ptr::null(),
            0,
            0,
            0,
        ) == 0
            || WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0
        {
            return Err(format!("HTTPS: {}", std::io::Error::last_os_error()));
        }
        let mut code = 0u32;
        let mut len = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut code as *mut _ as _,
            &mut len,
            std::ptr::null_mut(),
        ) == 0
            || code != 200
        {
            return Err(format!("GitHub HTTP {code}"));
        }
        let mut data = Vec::new();
        let mut buffer = [0u8; 32768];
        loop {
            let mut read = 0;
            if WinHttpReadData(
                request.0,
                buffer.as_mut_ptr() as _,
                buffer.len() as u32,
                &mut read,
            ) == 0
            {
                return Err("Загрузка прервана".into());
            }
            if read == 0 {
                break;
            }
            if data.len() + read as usize > max {
                return Err("Файл превышает допустимый размер".into());
            }
            data.extend_from_slice(&buffer[..read as usize]);
        }
        Ok(data)
    }
}
pub fn summary(updates: &Updates) -> Value {
    json!({"build_commit":COMMIT,"status":*updates.state.lock().unwrap()})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(newer("v0.6.0", "0.5.1"));
        assert!(!newer("v0.5.1", "0.5.1"));
        assert!(!newer("preview", "0.5.1"));
        assert!(!newer("v0.4.9", "0.5.1"));
    }
    #[test]
    fn digest() {
        assert_eq!(
            hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!valid_hash("../x"));
    }
    #[test]
    fn failed_launch_restores_previous_files() {
        let root = std::env::temp_dir().join(format!(
            "omniview-rollback-{}",
            crate::classroom::random().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        let stage = root.join(".omniview-update-test");
        std::fs::create_dir(&stage).unwrap();
        for name in ["OmniView.exe", "OmniView-Student.exe"] {
            std::fs::write(root.join(name), b"previous valid fixture").unwrap();
            std::fs::write(stage.join(name), b"not an executable").unwrap();
        }
        let plan = Plan {
            destination: root.clone(),
            parent: 0,
            launch: "OmniView.exe".into(),
            files: ["OmniView.exe", "OmniView-Student.exe"]
                .into_iter()
                .map(|n| FileHash {
                    name: n.into(),
                    sha256: hash(b"not an executable"),
                })
                .collect(),
        };
        assert!(install(&plan, &stage).is_err());
        for name in ["OmniView.exe", "OmniView-Student.exe"] {
            assert_eq!(
                std::fs::read(root.join(name)).unwrap(),
                b"previous valid fixture"
            );
        }
        // This path was just created by this test beneath the OS temporary directory.
        assert!(root.parent() == Some(std::env::temp_dir().as_path()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
