//! Explicitly enabled process-name checks. No window titles, paths or process termination.
use serde::{Deserialize, Serialize};
#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Policy {
    pub active: bool,
    pub blocked: Vec<String>,
    pub generation: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Detection {
    pub executable: String,
    pub at: u64,
}
#[derive(Clone, Serialize, Debug)]
pub struct Event {
    pub peer_id: String,
    pub name: String,
    pub executable: String,
    pub at: u64,
}
#[derive(Default, Serialize)]
pub struct Assessment {
    pub policy: Policy,
    pub events: Vec<Event>,
}
impl Assessment {
    pub fn configure(&mut self, blocked: &str, active: bool) -> Result<(), String> {
        let mut names = Vec::new();
        for item in blocked.split([',', ';', '\n']) {
            let name = item.trim().to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            if name.len() > 80
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._- ".contains(c))
            {
                return Err("Укажи точные имена процессов без путей и символов *".into());
            }
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if names.len() > 20 || (active && names.is_empty()) {
            return Err("Укажи от 1 до 20 программ".into());
        }
        self.policy = Policy {
            active,
            blocked: names,
            generation: self.policy.generation + 1,
        };
        Ok(())
    }
    pub fn record(
        &mut self,
        id: &str,
        name: &str,
        consent: bool,
        detections: &[Detection],
        now: u64,
    ) {
        if !self.policy.active || !consent {
            return;
        }
        for d in detections.iter().take(20) {
            let executable = d.executable.to_ascii_lowercase();
            if !self.policy.blocked.contains(&executable)
                || self.events.iter().rev().any(|e| {
                    e.peer_id == id
                        && e.executable == executable
                        && now.saturating_sub(e.at) < 30_000
                })
            {
                continue;
            }
            if self.events.len() >= 200 {
                self.events.remove(0);
            }
            self.events.push(Event {
                peer_id: id.into(),
                name: name.chars().take(64).collect(),
                executable,
                at: now,
            });
        }
    }
}
#[cfg(windows)]
pub fn scan(blocked: &[String]) -> Result<Vec<Detection>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Не удалось прочитать список процессов".into());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        let mut found = Vec::new();
        while ok != 0 {
            let n = entry
                .szExeFile
                .iter()
                .position(|&v| v == 0)
                .unwrap_or(entry.szExeFile.len());
            let executable = String::from_utf16_lossy(&entry.szExeFile[..n]).to_ascii_lowercase();
            if blocked.contains(&executable)
                && !found.iter().any(|d: &Detection| d.executable == executable)
            {
                found.push(Detection {
                    executable,
                    at: crate::classroom::now(),
                });
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        Ok(found)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_validates_names_and_events_require_consent() {
        let mut a = Assessment::default();
        assert!(a.configure("C:/chrome.exe", true).is_err());
        assert!(a.configure("", true).is_err());
        a.configure("CHROME.exe;chrome.exe;discord.exe", true)
            .unwrap();
        assert_eq!(a.policy.blocked.len(), 2);
        let d = Detection {
            executable: "chrome.exe".into(),
            at: 1,
        };
        a.record("a", "A", false, &[d.clone()], 1000);
        assert!(a.events.is_empty());
        a.record("a", "A", true, &[d.clone()], 1000);
        a.record("a", "A", true, &[d], 2000);
        assert_eq!(a.events.len(), 1);
        a.record(
            "b",
            "B",
            true,
            &[Detection {
                executable: "unlisted.exe".into(),
                at: 1,
            }],
            3000,
        );
        assert_eq!(a.events.len(), 1);
    }
}

#[cfg(not(windows))]
pub fn scan(blocked: &[String]) -> Result<Vec<Detection>, String> {
    let output = std::process::Command::new("ps")
        .args(["-A", "-o", "comm="])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Не удалось получить имена процессов".into());
    }
    let mut result = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let name = std::path::Path::new(line.trim())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if blocked.contains(&name) && !result.iter().any(|d: &Detection| d.executable == name) {
            result.push(Detection {
                executable: name,
                at: crate::classroom::now(),
            });
        }
    }
    Ok(result)
}
