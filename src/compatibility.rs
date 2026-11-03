//! Loopback-only test between independently built modern and legacy binaries.
use crate::classroom::{Student, Teacher};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(check: impl Fn() -> bool) -> Result<(), String> {
    let begin = Instant::now();
    while !check() {
        if begin.elapsed() > Duration::from_secs(20) {
            return Err("Истекло время межверсионной проверки".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
pub fn run(mode: &str, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if mode == "--compat-server" {
        let teacher = Teacher::start("127.0.0.1".into(), true, Arc::new(|| {}))?;
        let result = (|| {
            let code = teacher.invite.encrypt("OmniView loopback compatibility")?;
            std::fs::write(dir.join("invite.txt"), code).map_err(|e| e.to_string())?;
            wait(|| teacher.list().iter().any(|p| p.online))?;
            wait(|| {
                teacher
                    .log
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|m| m.text == "compatibility-client")
            })?;
            let peer = teacher
                .list()
                .into_iter()
                .find(|p| p.online)
                .ok_or("Ученик отключился")?;
            if peer.sharing || peer.monitoring || peer.control.active {
                return Err("Согласие появилось без действия ученика".into());
            }
            teacher.send(&peer.id, "compatibility-server")?;
            wait(|| dir.join("client-ok.json").exists())?;
            std::fs::write(
                dir.join("server-ok.json"),
                "{\"ok\":true,\"tls\":true,\"no_implicit_consent\":true}",
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })();
        teacher.end();
        result
    } else {
        wait(|| dir.join("invite.txt").exists())?;
        let code = std::fs::read_to_string(dir.join("invite.txt")).map_err(|e| e.to_string())?;
        let student = Student::connect_protected(
            &code,
            "OmniView loopback compatibility",
            "Cross-version loopback".into(),
            true,
            Arc::new(|| {}),
        )?;
        let result = (|| {
            wait(|| student.state.lock().unwrap().connected)?;
            student.note("compatibility-client".into())?;
            wait(|| {
                student
                    .state
                    .lock()
                    .unwrap()
                    .messages
                    .iter()
                    .any(|m| m.text == "compatibility-server")
            })?;
            let state = student.state.lock().unwrap();
            if state.sharing || state.monitoring || state.control_active {
                return Err("Согласие появилось без действия ученика".into());
            }
            std::fs::write(
                dir.join("client-ok.json"),
                "{\"ok\":true,\"messages_both_directions\":true}",
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })();
        student.end();
        result
    }
}
