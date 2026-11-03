use crate::classroom::{self, Changed, Student, Teacher};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

pub fn data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("portable.flag").is_file() {
                return dir.join("data");
            }
        }
    }
    #[cfg(windows)]
    return std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("OmniView-Legacy");
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/OmniView-Legacy");
    #[cfg(target_os = "linux")]
    return std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".local/share")
        })
        .join("omniview-legacy");
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub name: String,
    pub class_name: String,
    pub host: String,
    pub low: bool,
    pub theme: String,
    pub animations: bool,
    pub notification_seconds: u64,
    pub auto_updates: bool,
    pub preview_updates: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            name: std::env::var("USERNAME").unwrap_or_else(|_| "Пользователь".into()),
            class_name: "Компьютерный класс".into(),
            host: classroom::host_ip(),
            low: true,
            theme: "dark".into(),
            animations: true,
            notification_seconds: 9,
            auto_updates: true,
            preview_updates: false,
        }
    }
}
fn save_preferences(prefs: &Preferences) -> Result<(), String> {
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let path = data_dir().join("settings.json");
    let tmp = data_dir().join("settings.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}
pub struct Core {
    pub updates: crate::updates::Updates,
    prefs: Mutex<Preferences>,
    student_role: AtomicBool,
    teacher: Mutex<Option<Arc<Teacher>>>,
    student: Mutex<Option<Arc<Student>>>,
    rev: Arc<AtomicU64>,
    notice: Mutex<(u64, String)>,
    export_file: Mutex<String>,
    protected_invite: Mutex<String>,
}
impl Core {
    pub fn new(student: bool) -> Self {
        let mut prefs = std::fs::read(data_dir().join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Preferences>(&b).ok())
            .unwrap_or_default();
        prefs.notification_seconds = prefs.notification_seconds.clamp(5, 60);
        Self {
            updates: crate::updates::Updates::new(),
            prefs: Mutex::new(prefs),
            student_role: AtomicBool::new(student),
            teacher: Mutex::new(None),
            student: Mutex::new(None),
            rev: Arc::new(AtomicU64::new(1)),
            notice: Mutex::new((0, String::new())),
            export_file: Mutex::new(String::new()),
            protected_invite: Mutex::new(String::new()),
        }
    }
    pub fn revision(&self) -> u64 {
        if let Some(t) = self.teacher.lock().unwrap().as_ref() {
            if t.lesson.lock().unwrap().expire(classroom::now()) {
                self.rev.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.rev.load(Ordering::Relaxed)
    }
    fn changed(&self) -> Changed {
        let rev = self.rev.clone();
        Arc::new(move || {
            rev.fetch_add(1, Ordering::Relaxed);
        })
    }
    pub fn feedback(&self, message: &str) {
        let mut notice = self.notice.lock().unwrap();
        let rev = self.rev.fetch_add(1, Ordering::Relaxed) + 1;
        *notice = (rev, message.to_string());
    }
    pub fn snapshot(&self) -> Value {
        let teacher = self.teacher.lock().unwrap();
        let student = self.student.lock().unwrap();
        let notice = self.notice.lock().unwrap();
        let t=teacher.as_ref().map(|t|{
   let peers=t.list().into_iter().map(|p|json!({"id":p.id,"name":p.name,"machine":p.machine,"online":p.online,"help":p.help,"sharing":p.sharing,"monitoring":p.monitoring,"monitor_error":p.monitor_error,"control":p.control,"last_seen":p.last_seen,"frame":p.frame.map(|f|json!({"seq":f.seq}))})).collect::<Vec<_>>();
   json!({"active":!t.stop.load(Ordering::Relaxed),"invite":*self.protected_invite.lock().unwrap(),"address":format!("{}:{}",t.invite.host,t.invite.port),"peers":peers,"log":*t.log.lock().unwrap(),"broadcasting":t.broadcasting.load(Ordering::Relaxed),"lesson":*t.lesson.lock().unwrap(),"assessment":*t.assessment.lock().unwrap()})
  });
        let s=student.as_ref().map(|s|{let st=s.state.lock().unwrap();json!({"connected":st.connected,"connecting":st.connecting,"status":st.status,"sharing":st.sharing,"help":st.help,"messages":st.messages,"broadcasting":st.broadcasting,"broadcast":st.broadcast.as_ref().map(|f|json!({"seq":f.seq})),"question":st.question,"answer_pending":st.answer.is_some(),"policy":st.policy,"monitoring":st.monitoring,"detections":st.detections,"monitor_error":st.monitor_error,"control_request":st.control_request,"control_active":st.control_active,"control_pending":st.control_grant.is_some(),"control_error":st.control_error})});
        json!({"updates":crate::updates::summary(&self.updates),"portable":std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("portable.flag").is_file())).unwrap_or(false),"version":env!("CARGO_PKG_VERSION"),"arch":std::env::consts::ARCH,"role":if self.student_role.load(Ordering::Relaxed){"student"}else{"teacher"},"prefs":*self.prefs.lock().unwrap(),"teacher":t,"student":s,"notice":{"id":notice.0,"text":notice.1},"export_file":*self.export_file.lock().unwrap()})
    }
    pub fn image(&self, url: &str) -> Option<Vec<u8>> {
        let parts: Vec<_> = url.split('/').collect();
        if parts.get(2) == Some(&"student") && parts.len() == 5 {
            let teacher = self.teacher.lock().ok()?;
            let t = teacher.as_ref()?;
            if t.stop.load(Ordering::Relaxed) {
                return None;
            }
            let peers = t.peers.lock().ok()?;
            let p = peers.get(parts[3])?;
            if !p.online || !p.sharing {
                return None;
            }
            let f = p.frame.as_ref()?;
            // A newer frame may arrive between state delivery and the image request.
            STANDARD.decode(f.data.as_bytes()).ok()
        } else if parts.get(2) == Some(&"teacher") && parts.len() == 4 {
            let student = self.student.lock().ok()?;
            let state = student.as_ref()?.state.lock().ok()?;
            if !state.connected || !state.broadcasting {
                return None;
            }
            let f = state.broadcast.as_ref()?;
            STANDARD.decode(f.data.as_bytes()).ok()
        } else {
            None
        }
    }
    pub fn handle(&self, value: Value) -> Result<(), String> {
        let action = value["action"].as_str().ok_or("Неизвестное действие")?;
        let text = |key: &str| -> Result<String, String> {
            value[key]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("Отсутствует поле {key}"))
        };
        match action {
            "check_updates" => {
                let preview = self.prefs.lock().unwrap().preview_updates;
                let result = self.updates.check(preview);
                self.rev.fetch_add(1, Ordering::Relaxed);
                result?;
            }
            "download_update" => {
                let result = self.updates.prepare();
                self.rev.fetch_add(1, Ordering::Relaxed);
                result?;
            }
            "install_update" => {
                if self.teacher.lock().unwrap().is_some()
                    || self.student.lock().unwrap().as_ref().is_some_and(|s| {
                        let st = s.state.lock().unwrap();
                        st.connected || st.connecting
                    })
                {
                    return Err("Сначала заверши класс и выйди из подключения".into());
                }
                self.updates.launch()?;
            }
            "control_request" => {
                let on = value["on"].as_bool().ok_or("Укажи состояние управления")?;
                self.teacher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Открой класс")?
                    .request_control(&text("id")?, on)?;
                self.feedback(if on {
                    "Запрос управления отправлен. Дождись согласия ученика"
                } else {
                    "Управление остановлено"
                });
            }
            "control_input" => {
                let events: Vec<crate::control::InputEvent> =
                    serde_json::from_value(value["events"].clone()).map_err(|_| "Неверный ввод")?;
                self.teacher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Открой класс")?
                    .control_input(&text("id")?, &text("request")?, events)?;
            }
            "control_consent" => {
                let on = value["on"].as_bool().ok_or("Укажи согласие")?;
                let student = self.student.lock().unwrap();
                let s = student.as_ref().ok_or("Подключись к классу")?;
                let mut st = s.state.lock().unwrap();
                if on {
                    if !cfg!(windows) {
                        return Err(
                            "Управление этим компьютером пока доступно только в Windows Legacy"
                                .into(),
                        );
                    }
                    if !st.connected || !st.sharing {
                        return Err("Сначала включи показ экрана".into());
                    }
                    let id = st
                        .control_request
                        .as_ref()
                        .ok_or("Нет запроса управления")?
                        .id
                        .clone();
                    if value["request"].as_str() != Some(&id) {
                        return Err("Запрос изменился. Подтверди новый запрос".into());
                    }
                    s.input.authorize(id.clone());
                    st.control_grant = Some(id);
                    st.control_error = None;
                } else {
                    s.input.revoke();
                    st.control_grant = None;
                    st.control_active = false;
                }
                drop(st);
                self.feedback(if on {
                    "Разрешение отправлено. Esc останавливает управление"
                } else {
                    "Управление остановлено"
                });
            }
            "save_settings" => {
                let mut prefs: Preferences = serde_json::from_value(value["prefs"].clone())
                    .map_err(|_| "Неверные настройки")?;
                prefs.name = prefs.name.trim().chars().take(64).collect();
                prefs.class_name = prefs.class_name.trim().chars().take(64).collect();
                if prefs.name.is_empty() || prefs.class_name.is_empty() {
                    return Err("Укажи имя и название класса".into());
                }
                if prefs.host.parse::<std::net::Ipv4Addr>().is_err() {
                    return Err("Нужен корректный IPv4-адрес".into());
                }
                if prefs.theme != "dark" && prefs.theme != "light" {
                    return Err("Неизвестная тема".into());
                }
                prefs.notification_seconds = prefs.notification_seconds.clamp(5, 60);
                save_preferences(&prefs)?;
                if let Some(t) = self.teacher.lock().unwrap().as_ref() {
                    t.low.store(prefs.low, Ordering::Relaxed);
                }
                if let Some(s) = self.student.lock().unwrap().as_ref() {
                    s.low.store(prefs.low, Ordering::Relaxed);
                }
                *self.prefs.lock().unwrap() = prefs;
                self.feedback("Настройки сохранены");
            }
            "role" => {
                if self.teacher.lock().unwrap().is_some()
                    || self.student.lock().unwrap().as_ref().is_some_and(|s| {
                        let st = s.state.lock().unwrap();
                        st.connected || st.connecting
                    })
                {
                    return Err("Сначала заверши текущий сеанс".into());
                }
                self.student_role
                    .store(text("role")? == "student", Ordering::Relaxed);
                self.feedback("Режим изменён");
            }
            "start_class" => {
                let password = text("password")?;
                if password.chars().count() < 8 || password.len() > 256 {
                    return Err("Пароль класса: от 8 символов до 256 байт".into());
                }
                if self.student_role.load(Ordering::Relaxed) {
                    return Err("Переключись в режим учителя".into());
                }
                let mut teacher = self.teacher.lock().unwrap();
                if teacher.is_some() {
                    return Err("Класс уже открыт".into());
                }
                let prefs = self.prefs.lock().unwrap().clone();
                let t = Teacher::start(prefs.host, prefs.low, self.changed())?;
                match t.invite.encrypt(&password) {
                    Ok(code) => *self.protected_invite.lock().unwrap() = code,
                    Err(e) => {
                        t.end();
                        return Err(e);
                    }
                }
                *teacher = Some(t);
                self.feedback("Класс открыт. Передай код ученикам");
            }
            "end_class" => {
                *self.protected_invite.lock().unwrap() = String::new();
                if let Some(t) = self.teacher.lock().unwrap().take() {
                    t.end();
                }
                self.feedback("Класс закрыт. Старый код больше не действует");
            }
            "send_message" => {
                let target = text("target")?;
                let message = text("text")?;
                if message.chars().count() > 1000 {
                    return Err("Максимум 1000 символов".into());
                }
                let teacher = self.teacher.lock().unwrap();
                let count = teacher
                    .as_ref()
                    .ok_or("Сначала открой класс")?
                    .send(&target, &message)?;
                self.feedback(&format!("Сообщение передано в очередь: {count}"));
            }
            "ack_help" => {
                self.teacher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Класс закрыт")?
                    .acknowledge(&text("id")?)?;
                self.feedback("Запрос помощи отмечен");
            }
            "broadcast" => {
                let on = value["on"].as_bool().ok_or("Нужен статус показа")?;
                #[cfg(not(windows))]
                if on {
                    crate::capture::screen(640)?;
                }
                self.teacher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Сначала открой класс")?
                    .set_broadcast(on);
                self.feedback(if on {
                    "Твой основной экран виден подключённым ученикам"
                } else {
                    "Показ твоего экрана остановлен"
                });
            }
            "join" => {
                if !self.student_role.load(Ordering::Relaxed) {
                    return Err("Переключись в режим ученика".into());
                }
                let mut student = self.student.lock().unwrap();
                if student.as_ref().is_some_and(|s| {
                    let st = s.state.lock().unwrap();
                    st.connected || st.connecting
                }) {
                    return Err("Подключение уже открыто".into());
                }
                if let Some(old) = student.take() {
                    old.end();
                }
                let name = text("name")?.trim().chars().take(64).collect::<String>();
                if name.is_empty() {
                    return Err("Введи имя ученика".into());
                }
                *student = Some(Student::connect_protected(
                    &text("code")?,
                    &text("password")?,
                    name,
                    self.prefs.lock().unwrap().low,
                    self.changed(),
                )?);
                self.feedback("Подключение к классу…");
            }
            "leave" => {
                if let Some(s) = self.student.lock().unwrap().as_ref() {
                    s.end();
                }
                self.feedback("Отключение от класса");
            }
            "share" => {
                let on = value["on"].as_bool().ok_or("Нужен статус показа")?;
                #[cfg(not(windows))]
                if on {
                    crate::capture::screen(640)?;
                }
                let student = self.student.lock().unwrap();
                let s = student.as_ref().ok_or("Сначала подключись")?;
                if !s.state.lock().unwrap().connected {
                    return Err("Сначала подключись к классу".into());
                }
                s.share(on);
                self.feedback(if on {
                    "Учитель видит твой основной экран"
                } else {
                    "Показ твоего экрана выключен"
                });
            }
            "help" => {
                let student = self.student.lock().unwrap();
                let s = student.as_ref().ok_or("Сначала подключись")?;
                if !s.state.lock().unwrap().connected {
                    return Err("Сначала подключись к классу".into());
                }
                s.help();
                self.feedback("Учитель получил запрос помощи");
            }
            "student_message" => {
                self.student
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Сначала подключись")?
                    .note(text("text")?)?;
                self.feedback("Сообщение отправляется учителю");
            }
            "export" => {
                let teacher = self.teacher.lock().unwrap();
                let t = teacher.as_ref().ok_or("Сначала открой класс")?;
                let peers=t.list().into_iter().map(|p|json!({"name":p.name,"machine":p.machine,"online":p.online,"help":p.help,"sharing":p.sharing,"last_seen":p.last_seen})).collect::<Vec<_>>();
                let dir = data_dir().join("exports");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let file = dir.join(format!("class-{}.json", classroom::now()));
                let data = json!({"class":self.prefs.lock().unwrap().class_name,"exported_at":classroom::now(),"peers":peers,"messages":*t.log.lock().unwrap()});
                std::fs::write(
                    &file,
                    serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                *self.export_file.lock().unwrap() = file.display().to_string();
                self.feedback("Отчёт сохранён. Путь доступен в настройках");
            }
            "question_start" => {
                let options: Vec<String> = serde_json::from_value(value["options"].clone())
                    .map_err(|_| "Нужны варианты ответа")?;
                let correct = value["correct"].as_u64().ok_or("Выбери правильный ответ")?;
                if correct > 5 {
                    return Err("Неверный правильный вариант".into());
                }
                let seconds = value["seconds"].as_u64().ok_or("Укажи время")?;
                let teacher = self.teacher.lock().unwrap();
                let t = teacher.as_ref().ok_or("Открой класс")?;
                let invited = t
                    .list()
                    .into_iter()
                    .filter(|p| p.online)
                    .map(|p| (p.id, p.name))
                    .collect();
                t.lesson.lock().unwrap().start(
                    &text("prompt")?,
                    options,
                    correct as usize,
                    seconds,
                    invited,
                    classroom::now(),
                )?;
                self.feedback("Вопрос опубликован");
            }
            "assessment_configure" => {
                let teacher = self.teacher.lock().unwrap();
                let t = teacher.as_ref().ok_or("Открой класс")?;
                t.assessment.lock().unwrap().configure(
                    &text("blocked")?,
                    value["active"]
                        .as_bool()
                        .ok_or("Укажи состояние проверки")?,
                )?;
                self.feedback(
                    "Условия самостоятельной работы обновлены. Ученикам нужно подтвердить участие",
                );
            }
            "monitor_consent" => {
                let on = value["on"].as_bool().ok_or("Укажи согласие")?;
                let student = self.student.lock().unwrap();
                let s = student.as_ref().ok_or("Подключись к классу")?;
                let mut st = s.state.lock().unwrap();
                if !st.connected || !st.policy.active {
                    return Err("Самостоятельная работа не включена".into());
                }
                st.monitoring = on;
                st.detections.clear();
                st.monitor_error = None;
                drop(st);
                self.feedback(if on {
                    "Проверка списка программ включена"
                } else {
                    "Проверка списка программ выключена"
                });
            }
            "question_close" => {
                self.teacher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or("Открой класс")?
                    .lesson
                    .lock()
                    .unwrap()
                    .close();
                self.feedback("Приём ответов завершён");
            }
            "question_answer" => {
                let id = text("id")?;
                let choice = value["choice"].as_u64().ok_or("Выбери ответ")?;
                let student = self.student.lock().unwrap();
                let s = student.as_ref().ok_or("Подключись к классу")?;
                let mut st = s.state.lock().unwrap();
                let q = st.question.as_ref().ok_or("Нет текущего вопроса")?;
                if !st.connected
                    || st.answer.is_some()
                    || q.id != id
                    || q.closed
                    || q.own_choice.is_some()
                    || choice >= q.options.len() as u64
                {
                    return Err("Этот ответ уже нельзя отправить".into());
                }
                st.answer = Some(crate::lesson::Submission {
                    question_id: id,
                    choice: choice as usize,
                });
                drop(st);
                self.feedback("Ответ отправляется. Дождись подтверждения");
            }
            "lesson_report" => {
                let teacher = self.teacher.lock().unwrap();
                let t = teacher.as_ref().ok_or("Открой класс")?;
                let mut lesson = t.lesson.lock().unwrap();
                lesson.expire(classroom::now());
                let assessment = t.assessment.lock().unwrap();
                if lesson.questions.is_empty() && assessment.events.is_empty() {
                    return Err("Сначала проведи вопрос или самостоятельную работу".into());
                }
                let mut html =
                    crate::lesson::report(&self.prefs.lock().unwrap().class_name, &lesson);
                html.push_str("<h2>События самостоятельной работы</h2><p>Факт обнаружения процесса не доказывает нарушение. Полноту проверки и отсутствие обхода программа не гарантирует.</p><table><tr><th>Ученик</th><th>Программа</th><th>Время сервера (Unix ms)</th></tr>");
                for e in &assessment.events {
                    html.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                        crate::lesson::escape(&e.name),
                        crate::lesson::escape(&e.executable),
                        e.at
                    ));
                }
                html.push_str("</table>");
                html = html.replace("</html>", "");
                html.push_str("</html>");
                let dir = data_dir().join("exports");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let file = dir.join(format!("lesson-{}.html", classroom::now()));
                std::fs::write(&file, html).map_err(|e| e.to_string())?;
                *self.export_file.lock().unwrap() = file.display().to_string();
                self.feedback("Отчёт урока сохранён. Путь доступен в настройках");
            }
            _ => return Err("Неизвестное действие".into()),
        }
        Ok(())
    }
    pub fn stop(&self) {
        if let Some(t) = self.teacher.lock().unwrap().take() {
            t.end();
        }
        if let Some(s) = self.student.lock().unwrap().take() {
            s.end();
        }
    }
}
pub fn self_test() -> Value {
    let net = classroom::network_self_test();
    let worker = classroom::worker_self_test();
    let capture = crate::capture::screen(640);
    let prefs = serde_json::to_vec(&Preferences::default())
        .ok()
        .and_then(|b| serde_json::from_slice::<Preferences>(&b).ok())
        .is_some();
    json!({"ok":net.is_ok() && worker.is_ok() && capture.is_ok() && prefs,"arch":std::env::consts::ARCH,"version":env!("CARGO_PKG_VERSION"),"tls_pairing_messages_help":net.err().unwrap_or_else(||"ok".into()),"real_student_messages_consent_broadcast":worker.err().unwrap_or_else(||"ok".into()),"capture_base64_bytes":capture.as_ref().map(|s|s.len()).unwrap_or(0),"capture_error":capture.err(),"settings_roundtrip":prefs})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_preferences_migrate_and_roundtrip() {
        let old: Preferences =
            serde_json::from_value(json!({"name":"Учитель", "low":false})).unwrap();
        assert!(old.animations);
        assert_eq!(old.notification_seconds, 9);
        assert!(!old.low);
        assert_eq!(old.name, "Учитель");
        let disabled = Preferences {
            animations: false,
            notification_seconds: 20,
            ..old
        };
        let saved = serde_json::to_vec(&disabled).unwrap();
        let restored: Preferences = serde_json::from_slice(&saved).unwrap();
        assert!(!restored.animations);
        assert_eq!(restored.notification_seconds, 20);
    }
}
