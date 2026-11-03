//! Explicit, short-lived assistance. The network cannot arm this gate.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Request {
    pub id: String,
    pub expires_at: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputEvent {
    Move {
        x: u16,
        y: u16,
    },
    Left {
        down: bool,
    },
    RightClick,
    Wheel {
        steps: i32,
    },
    Text {
        text: String,
    },
    Key {
        vk: u16,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
}
impl InputEvent {
    pub fn valid(&self) -> bool {
        match self {
            Self::Text { text } => {
                !text.is_empty()
                    && text.chars().count() <= 64
                    && !text.chars().any(|c| c.is_control())
            }
            Self::Wheel { steps } => (-3..=3).contains(steps) && *steps != 0,
            Self::Key { vk, .. } => matches!(*vk,8|9|13|27|32..=40|45|46|48..=90|112..=123),
            _ => true,
        }
    }
}
/// Server-side request and queue; never grants consent itself.
#[derive(Clone, Default, Serialize)]
pub struct Session {
    pub request: Option<Request>,
    pub active: bool,
    #[serde(skip)]
    queue: VecDeque<InputEvent>,
}
impl Session {
    pub fn request(&mut self, now: u64) -> Result<(), String> {
        self.stop();
        self.request = Some(Request {
            id: crate::classroom::random()?,
            expires_at: now + 300_000,
        });
        Ok(())
    }
    pub fn stop(&mut self) {
        self.active = false;
        self.request = None;
        self.queue.clear();
    }
    pub fn update(&mut self, grant: Option<&str>, sharing: bool, now: u64) {
        if self.request.as_ref().is_some_and(|r| r.expires_at <= now) {
            self.stop();
            return;
        }
        let active = sharing
            && self
                .request
                .as_ref()
                .is_some_and(|r| Some(r.id.as_str()) == grant);
        if self.active && !active {
            self.stop();
            return;
        }
        self.active = active;
        if !active {
            self.queue.clear();
        }
    }
    pub fn enqueue(
        &mut self,
        request: &str,
        events: Vec<InputEvent>,
        now: u64,
    ) -> Result<(), String> {
        if !self.active
            || !self
                .request
                .as_ref()
                .is_some_and(|r| r.id == request && now < r.expires_at)
        {
            return Err("Ученик не разрешил управление или сеанс завершён".into());
        }
        if events.is_empty() || events.len() > 16 || events.iter().any(|e| !e.valid()) {
            return Err("Недопустимые события ввода".into());
        }
        if self.queue.len() + events.len() > 64 {
            self.stop();
            return Err("Очередь ввода переполнена. Запроси разрешение заново".into());
        }
        self.queue.extend(events);
        Ok(())
    }
    pub fn drain(&mut self) -> Vec<InputEvent> {
        if self.active {
            self.queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }
}
struct Permission {
    id: Option<String>,
    armed_at: Instant,
    heard: Instant,
    left_down: bool,
}
pub struct InputGate(Mutex<Permission>);
impl Default for InputGate {
    fn default() -> Self {
        Self(Mutex::new(Permission {
            id: None,
            armed_at: Instant::now(),
            heard: Instant::now(),
            left_down: false,
        }))
    }
}
impl InputGate {
    /// Called only by the local consent action.
    pub fn authorize(&self, id: String) {
        self.revoke();
        let mut p = self.0.lock().unwrap();
        p.id = Some(id);
        p.armed_at = Instant::now();
        p.heard = Instant::now();
    }
    pub fn revoke(&self) {
        let mut p = self.0.lock().unwrap();
        p.id = None;
        release(&mut p);
    }
    pub fn watchdog(&self) -> bool {
        let mut p = self.0.lock().unwrap();
        if p.id.is_some()
            && (p.heard.elapsed() > Duration::from_secs(2)
                || p.armed_at.elapsed() > Duration::from_secs(300)
                || unsafe { GetAsyncKeyState(VK_ESCAPE as i32) } < 0)
        {
            p.id = None;
            release(&mut p);
            return true;
        }
        false
    }
    pub fn deliver(&self, id: &str, events: &[InputEvent]) -> Result<(), String> {
        let mut p = self.0.lock().unwrap();
        if p.id.as_deref() != Some(id)
            || p.heard.elapsed() > Duration::from_secs(2)
            || p.armed_at.elapsed() > Duration::from_secs(300)
        {
            p.id = None;
            release(&mut p);
            return Err("Разрешение управления отозвано".into());
        }
        if events.len() > 64 || events.iter().any(|e| !e.valid()) {
            p.id = None;
            release(&mut p);
            return Err("Отклонён некорректный ввод".into());
        }
        p.heard = Instant::now();
        for event in events {
            let mut inputs = Vec::new();
            match event {
                InputEvent::Move { x, y } => inputs.push(mouse(
                    *x as i32,
                    *y as i32,
                    0,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                )),
                InputEvent::Left { down } => {
                    p.left_down = *down;
                    inputs.push(mouse(
                        0,
                        0,
                        0,
                        if *down {
                            MOUSEEVENTF_LEFTDOWN
                        } else {
                            MOUSEEVENTF_LEFTUP
                        },
                    ));
                }
                InputEvent::RightClick => {
                    inputs.push(mouse(0, 0, 0, MOUSEEVENTF_RIGHTDOWN));
                    inputs.push(mouse(0, 0, 0, MOUSEEVENTF_RIGHTUP));
                }
                InputEvent::Wheel { steps } => {
                    inputs.push(mouse(0, 0, (steps * 120) as u32, MOUSEEVENTF_WHEEL))
                }
                InputEvent::Text { text } => {
                    for ch in text.encode_utf16() {
                        inputs.push(key(0, ch, KEYEVENTF_UNICODE));
                        inputs.push(key(0, ch, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                    }
                }
                InputEvent::Key {
                    vk,
                    ctrl,
                    alt,
                    shift,
                } => {
                    for (k, on) in [(VK_CONTROL, *ctrl), (VK_MENU, *alt), (VK_SHIFT, *shift)] {
                        if on {
                            inputs.push(key(k, 0, 0));
                        }
                    }
                    let flags = if matches!(*vk, 33..=40 | 45 | 46) {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        0
                    };
                    inputs.push(key(*vk, 0, flags));
                    inputs.push(key(*vk, 0, flags | KEYEVENTF_KEYUP));
                    for (k, on) in [(VK_SHIFT, *shift), (VK_MENU, *alt), (VK_CONTROL, *ctrl)] {
                        if on {
                            inputs.push(key(k, 0, KEYEVENTF_KEYUP));
                        }
                    }
                }
            }
            if unsafe {
                SendInput(
                    inputs.len() as u32,
                    inputs.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                )
            } != inputs.len() as u32
            {
                // Always release modifiers if Windows only accepted part of a chord.
                let cleanup = [
                    key(VK_CONTROL, 0, KEYEVENTF_KEYUP),
                    key(VK_MENU, 0, KEYEVENTF_KEYUP),
                    key(VK_SHIFT, 0, KEYEVENTF_KEYUP),
                ];
                unsafe {
                    SendInput(
                        cleanup.len() as u32,
                        cleanup.as_ptr(),
                        std::mem::size_of::<INPUT>() as i32,
                    );
                }
                p.id = None;
                release(&mut p);
                return Err(
                    "Windows отклонила ввод. Защищённые и повышенные окна не поддерживаются".into(),
                );
            }
        }
        Ok(())
    }
}
fn release(p: &mut Permission) {
    if p.left_down {
        let input = mouse(0, 0, 0, MOUSEEVENTF_LEFTUP);
        unsafe {
            SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        }
        p.left_down = false;
    }
}
fn mouse(dx: i32, dy: i32, data: u32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn key(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_packets_cannot_authorize_native_input() {
        let gate = InputGate::default();
        assert!(gate.deliver("server-claims-consent", &[]).is_err());
        gate.authorize("local".into());
        assert!(gate.deliver("other-request", &[]).is_err());
        assert!(gate.deliver("local", &[]).is_err());
    }
    #[test]
    fn revoke_and_watchdog_close_gate() {
        let gate = InputGate::default();
        gate.authorize("local".into());
        assert!(gate.deliver("local", &[]).is_ok());
        gate.revoke();
        assert!(gate.deliver("local", &[]).is_err());
        gate.authorize("local".into());
        gate.0.lock().unwrap().heard = Instant::now() - Duration::from_secs(3);
        assert!(gate.watchdog());
        assert!(gate.deliver("local", &[]).is_err());
    }
    #[test]
    fn queue_requires_matching_consent_and_is_bounded() {
        let mut s = Session::default();
        s.request(1000).unwrap();
        let id = s.request.as_ref().unwrap().id.clone();
        assert!(s
            .enqueue(&id, vec![InputEvent::Left { down: true }], 1001)
            .is_err());
        s.update(Some("other"), true, 1002);
        assert!(!s.active);
        s.update(Some(&id), true, 1003);
        assert!(s.active);
        assert!(s
            .enqueue(&id, vec![InputEvent::Wheel { steps: 99 }], 1004)
            .is_err());
        s.enqueue(&id, vec![InputEvent::Move { x: 0, y: 65535 }], 1005)
            .unwrap();
        assert_eq!(s.drain().len(), 1);
        s.update(None, true, 1006);
        assert!(s.request.is_none());
        assert!(s.drain().is_empty());
        s.request(2000).unwrap();
        let id = s.request.as_ref().unwrap().id.clone();
        s.update(Some(&id), true, 2001);
        for _ in 0..4 {
            s.enqueue(&id, vec![InputEvent::Move { x: 1, y: 2 }; 16], 2002)
                .unwrap();
        }
        assert!(s
            .enqueue(&id, vec![InputEvent::Left { down: true }], 2003)
            .is_err());
        assert!(s.request.is_none());
        assert!(s.drain().is_empty());
    }
    #[test]
    fn expiry_and_text_limits() {
        let mut s = Session::default();
        s.request(1000).unwrap();
        let id = s.request.as_ref().unwrap().id.clone();
        s.update(Some(&id), true, 301000);
        assert!(s.request.is_none());
        assert!(!InputEvent::Text {
            text: "a".repeat(65)
        }
        .valid());
        assert!(!InputEvent::Key {
            vk: 0xffff,
            ctrl: false,
            alt: false,
            shift: false
        }
        .valid());
    }
}
