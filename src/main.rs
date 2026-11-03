#![cfg_attr(windows, windows_subsystem = "windows")]
mod app;
mod assessment;
#[cfg(windows)]
mod capture;
#[cfg(not(windows))]
#[path = "capture_unix.rs"]
mod capture;
mod classroom;
mod compatibility;
#[cfg(windows)]
mod control;
#[cfg(not(windows))]
#[path = "control_unix.rs"]
mod control;
mod invitation;
mod lesson;
#[cfg(windows)]
mod native;
#[cfg(not(windows))]
#[path = "desktop.rs"]
mod native;
#[cfg(windows)]
mod updates;
#[cfg(not(windows))]
#[path = "updates_unix.rs"]
mod updates;
fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "--apply-update") {
        if let Some(p) = args.get(2) {
            if let Err(e) = updates::apply(std::path::Path::new(p)) {
                native::error(&e);
            }
        }
        return;
    }
    let commands: Vec<_> = std::env::args().collect();
    if commands
        .get(1)
        .is_some_and(|a| a == "--compat-server" || a == "--compat-client")
    {
        let result = commands
            .get(2)
            .ok_or("No test folder".to_string())
            .and_then(|p| compatibility::run(&commands[1], std::path::Path::new(p)));
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if std::env::args().any(|a| a == "--update-check") {
        let updates = updates::Updates::new();
        let result = updates.check(std::env::args().any(|a| a == "--preview"));
        let _ = std::fs::create_dir_all(app::data_dir());
        let _ = std::fs::write(
            app::data_dir().join("update-check.json"),
            updates::summary(&updates).to_string(),
        );
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if args.iter().any(|a| a == "--self-test") {
        let result = app::self_test();
        let _ = std::fs::create_dir_all(app::data_dir());
        let _ = std::fs::write(app::data_dir().join("self-test.json"), result.to_string());
        std::process::exit(if result["ok"] == true { 0 } else { 1 });
    }
    if let Err(e) = native::run() {
        native::error(&e);
        std::process::exit(1);
    }
}
