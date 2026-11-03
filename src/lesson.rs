//! Classroom checks: server owns deadlines, answers and the answer key.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Submission {
    pub question_id: String,
    pub choice: usize,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StudentQuestion {
    pub id: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub closes_at: u64,
    pub closed: bool,
    pub own_choice: Option<usize>,
    pub correct: Option<usize>,
}
#[derive(Clone, Serialize, Debug)]
pub struct Response {
    pub name: String,
    pub choice: usize,
    pub elapsed_ms: u64,
}
#[derive(Clone, Serialize, Debug)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub correct: usize,
    pub opened_at: u64,
    pub closes_at: u64,
    pub closed: bool,
    pub invited: BTreeMap<String, String>,
    pub answers: BTreeMap<String, Response>,
}
#[derive(Default, Serialize)]
pub struct Lesson {
    pub questions: Vec<Question>,
}
impl Lesson {
    pub fn start(
        &mut self,
        prompt: &str,
        options: Vec<String>,
        correct: usize,
        seconds: u64,
        invited: BTreeMap<String, String>,
        now: u64,
    ) -> Result<(), String> {
        if self
            .questions
            .last()
            .is_some_and(|q| !q.closed && now < q.closes_at)
        {
            return Err("Сначала заверши текущий вопрос".into());
        }
        if self.questions.len() >= 30 {
            return Err("Максимум 30 вопросов за урок. Сохрани отчёт и открой новый класс".into());
        }
        if invited.is_empty() || invited.len() > 40 {
            return Err("Для задания нужен хотя бы один подключённый ученик".into());
        }
        let prompt = prompt.trim();
        let options: Vec<String> = options.into_iter().map(|s| s.trim().to_owned()).collect();
        if prompt.is_empty()
            || prompt.chars().count() > 500
            || !(2..=6).contains(&options.len())
            || correct >= options.len()
            || options
                .iter()
                .any(|s| s.is_empty() || s.chars().count() > 180)
            || !(15..=900).contains(&seconds)
        {
            return Err(
                "Вопрос: до 500 символов; 2–6 вариантов до 180 символов; время 15–900 секунд"
                    .into(),
            );
        }
        if options.iter().enumerate().any(|(i, s)| {
            options[..i]
                .iter()
                .any(|a| a.to_lowercase() == s.to_lowercase())
        }) {
            return Err("Варианты ответа должны различаться".into());
        }
        if let Some(q) = self.questions.last_mut() {
            q.closed = true;
        }
        self.questions.push(Question {
            id: crate::classroom::random()?,
            prompt: prompt.into(),
            options,
            correct,
            opened_at: now,
            closes_at: now + seconds * 1000,
            closed: false,
            invited,
            answers: BTreeMap::new(),
        });
        Ok(())
    }
    pub fn close(&mut self) {
        if let Some(q) = self.questions.last_mut() {
            q.closed = true;
        }
    }
    pub fn expire(&mut self, now: u64) -> bool {
        if let Some(q) = self.questions.last_mut() {
            if !q.closed && now >= q.closes_at {
                q.closed = true;
                return true;
            }
        }
        false
    }
    pub fn submit(&mut self, peer: &str, answer: &Submission, now: u64) {
        self.expire(now);
        if let Some(q) = self.questions.last_mut() {
            if q.id != answer.question_id
                || q.closed
                || !q.invited.contains_key(peer)
                || answer.choice >= q.options.len()
                || q.answers.contains_key(peer)
            {
                return;
            }
            q.answers.insert(
                peer.into(),
                Response {
                    name: q.invited[peer].clone(),
                    choice: answer.choice,
                    elapsed_ms: now.saturating_sub(q.opened_at),
                },
            );
        }
    }
    pub fn for_student(&mut self, peer: &str, now: u64) -> Option<StudentQuestion> {
        self.expire(now);
        self.questions
            .last()
            .filter(|q| q.invited.contains_key(peer))
            .map(|q| StudentQuestion {
                id: q.id.clone(),
                prompt: q.prompt.clone(),
                options: q.options.clone(),
                closes_at: q.closes_at,
                closed: q.closed,
                own_choice: q.answers.get(peer).map(|a| a.choice),
                correct: q.closed.then_some(q.correct),
            })
    }
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// A local printable report. No script, remote font or remote image dependencies.
pub fn report(class_name: &str, lesson: &Lesson) -> String {
    let mut html = format!("<!doctype html><html lang=ru><meta charset=utf-8><title>Отчёт урока — OmniView</title><style>body{{font:16px system-ui;max-width:980px;margin:40px auto;padding:0 25px;color:#18291c}}h1{{font-size:32px}}h2{{margin-top:36px}}table{{width:100%;border-collapse:collapse}}td,th{{border:1px solid #ccd4cc;padding:9px;text-align:left}}small{{color:#536354}}@media print{{h2{{break-after:avoid}}tr{{break-inside:avoid}}}}</style><h1>{}</h1><p>OmniView · результаты проверки понимания</p><small>Время ответа отсчитывается сервером от публикации вопроса. Отсутствие ответа не означает ошибку. Отчёт содержит имена; передавай его только тем, кому он предназначен.</small>", escape(class_name));
    for (i, q) in lesson.questions.iter().enumerate() {
        let right = q.answers.values().filter(|a| a.choice == q.correct).count();
        html.push_str(&format!("<h2>{}. {}</h2><p>{} · Ответили {} из {}. Верно: {}. Правильный вариант: {}.</p><table><tr><th>Ученик</th><th>Ответ</th><th>Результат</th><th>Секунды</th></tr>", i+1,escape(&q.prompt),if q.closed {"Завершён"}else{"Промежуточный результат"},q.answers.len(),q.invited.len(),right,escape(&q.options[q.correct])));
        for (id, name) in &q.invited {
            if let Some(a) = q.answers.get(id) {
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.1}</td></tr>",
                    escape(name),
                    escape(&q.options[a.choice]),
                    if a.choice == q.correct {
                        "Верно"
                    } else {
                        "Нужно разобрать"
                    },
                    a.elapsed_ms as f64 / 1000.0
                ));
            } else {
                html.push_str(&format!(
                    "<tr><td>{}</td><td>Нет ответа</td><td>—</td><td>—</td></tr>",
                    escape(name)
                ));
            }
        }
        html.push_str("</table>");
    }
    html.push_str("<p><small>Это результаты конкретных заданий, а не автоматическая оценка способностей ученика.</small></p></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    fn question() -> Lesson {
        let mut l = Lesson::default();
        l.start(
            "2 + 2?",
            vec!["3".into(), "4".into()],
            1,
            15,
            BTreeMap::from([
                ("a".into(), "Аня <script>".into()),
                ("b".into(), "Борис".into()),
            ]),
            1000,
        )
        .unwrap();
        l
    }
    #[test]
    fn answer_key_is_hidden_until_closed() {
        let mut l = question();
        let s = l.for_student("a", 2000).unwrap();
        assert_eq!(s.correct, None);
        assert!(!serde_json::to_string(&s).unwrap().contains("invited"));
        assert!(l.for_student("stranger", 2000).is_none());
        l.close();
        assert_eq!(l.for_student("a", 2000).unwrap().correct, Some(1));
    }
    #[test]
    fn first_answer_wins_and_invalid_answers_are_ignored() {
        let mut l = question();
        let id = l.questions[0].id.clone();
        for (peer, qid, c) in [
            ("stranger", id.as_str(), 0),
            ("a", "old", 0),
            ("a", id.as_str(), 7),
        ] {
            l.submit(
                peer,
                &Submission {
                    question_id: qid.into(),
                    choice: c,
                },
                2000,
            );
        }
        assert!(l.questions[0].answers.is_empty());
        l.submit(
            "a",
            &Submission {
                question_id: id.clone(),
                choice: 1,
            },
            3000,
        );
        l.submit(
            "a",
            &Submission {
                question_id: id,
                choice: 0,
            },
            4000,
        );
        assert_eq!(l.questions[0].answers["a"].choice, 1);
        assert_eq!(l.questions[0].answers["a"].elapsed_ms, 2000);
    }
    #[test]
    fn deadline_and_early_close_reject_late_answers() {
        let mut l = question();
        let a = Submission {
            question_id: l.questions[0].id.clone(),
            choice: 1,
        };
        l.submit("a", &a, 16000);
        assert!(l.questions[0].closed);
        assert!(l.questions[0].answers.is_empty());
        let mut l = question();
        l.close();
        let a = Submission {
            question_id: l.questions[0].id.clone(),
            choice: 1,
        };
        l.submit("a", &a, 2000);
        assert!(l.questions[0].answers.is_empty());
    }
    #[test]
    fn report_escapes_names_questions_and_answers() {
        let l = question();
        let html = report("<img onerror=bad>", &l);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<img"));
    }
    #[test]
    fn input_is_bounded_and_questions_cannot_overlap() {
        let mut l = question();
        assert!(l
            .start(
                "Next",
                vec!["a".into(), "b".into()],
                0,
                15,
                BTreeMap::new(),
                2000
            )
            .is_err());
        l.close();
        assert!(l
            .start(
                "Next",
                vec!["a".into(), "A".into()],
                0,
                15,
                BTreeMap::from([("a".into(), "A".into())]),
                2000
            )
            .is_err());
    }
}
