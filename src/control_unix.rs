//! Explicit, short-lived assistance. The network cannot arm this gate.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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

pub struct InputGate;
impl Default for InputGate {
    fn default() -> Self {
        Self
    }
}
impl InputGate {
    pub fn authorize(&self, _id: String) {}
    pub fn revoke(&self) {}
    pub fn watchdog(&self) -> bool {
        true
    }
    pub fn deliver(&self, _id: &str, _events: &[InputEvent]) -> Result<(), String> {
        Err("Ввод на этой платформе пока недоступен".into())
    }
}
