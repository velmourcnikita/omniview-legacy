use crate::app::Core;
use eframe::egui::{self, RichText};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
pub fn error(s: &str) {
    eprintln!("OmniView Legacy: {s}");
}
struct Desktop {
    core: Arc<Core>,
    state: Value,
    revision: u64,
    student: bool,
    name: String,
    host: String,
    password: String,
    code: String,
    message: String,
    selected: Option<String>,
    question: String,
    options: String,
    correct: usize,
    seconds: u64,
    blocked: String,
    choice: usize,
    settings: bool,
    texture: Option<egui::TextureHandle>,
    frame: String,
    smoke: bool,
    started: std::time::Instant,
}
pub fn run() -> Result<(), String> {
    let student = std::env::args().any(|s| s == "--student")
        || std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.file_name()
                    .map(|s| s.to_string_lossy().contains("Student"))
            })
            .unwrap_or(false);
    let core = Arc::new(Core::new(student));
    let state = core.snapshot();
    let prefs = &state["prefs"];
    let app = Desktop {
        core: core.clone(),
        state: state.clone(),
        revision: 0,
        student,
        name: prefs["name"].as_str().unwrap_or("").into(),
        host: prefs["host"].as_str().unwrap_or("").into(),
        password: String::new(),
        code: String::new(),
        message: String::new(),
        selected: None,
        question: String::new(),
        options: String::new(),
        correct: 1,
        seconds: 60,
        blocked: String::new(),
        choice: 0,
        settings: false,
        texture: None,
        frame: String::new(),
        smoke: std::env::args().any(|s| s == "--ui-smoke"),
        started: std::time::Instant::now(),
    };
    if !app.smoke && prefs["auto_updates"] == true {
        let c = core.clone();
        std::thread::spawn(move || {
            let _ = c.handle(json!({"action":"check_updates"}));
        });
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([760.0, 540.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "OmniView Legacy",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "OmniView".into(),
                Arc::new(egui::FontData::from_static(include_bytes!(
                    "../assets/AppRegular.ttf"
                ))),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "OmniView".into());
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}
impl Desktop {
    fn send(&self, value: Value) {
        #[cfg(target_os = "macos")]
        if value["on"] == true && matches!(value["action"].as_str(), Some("share" | "broadcast")) {
            if let Err(e) = crate::capture::request_permission() {
                self.core.feedback(&e);
                return;
            }
        }
        let core = self.core.clone();
        std::thread::spawn(move || {
            if let Err(e) = core.handle(value) {
                core.feedback(&e);
            }
        });
    }
    fn save_preferences(&self, prefs: Value) {
        if let Err(e) = self
            .core
            .handle(json!({"action":"save_settings","prefs":prefs}))
        {
            self.core.feedback(&e);
        }
    }
    fn live(&self) -> bool {
        self.state["teacher"]["active"] == true
            || self.state["student"]["connected"] == true
            || self.state["student"]["connecting"] == true
    }
    fn preview(&mut self, ctx: &egui::Context) {
        let s = &self.state["student"];
        let key = if self.student {
            if s["broadcasting"] == true {
                s["broadcast"]["seq"]
                    .as_u64()
                    .map(|n| format!("/preview/teacher/{n}"))
            } else {
                None
            }
        } else {
            self.selected.as_ref().and_then(|id| {
                self.state["teacher"]["peers"]
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .find(|p| p["id"] == *id && p["sharing"] == true && p["online"] == true)
                    })
                    .and_then(|p| {
                        p["frame"]["seq"]
                            .as_u64()
                            .map(|n| format!("/preview/student/{id}/{n}"))
                    })
            })
        }
        .unwrap_or_default();
        if key != self.frame {
            self.frame = key.clone();
            self.texture = None;
            if let Some(bytes) = self.core.image(&key) {
                if let Ok(image) = image::load_from_memory(&bytes) {
                    let rgba = image.to_rgba8();
                    self.texture = Some(ctx.load_texture(
                        "shared-screen",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
    }
}
impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(250));
        let revision = self.core.revision();
        if revision != self.revision {
            self.revision = revision;
            self.state = self.core.snapshot();
            self.student = self.state["role"] == "student";
            self.preview(ctx);
        }
        if self.smoke && self.started.elapsed() > Duration::from_secs(2) {
            let _ = std::fs::create_dir_all(crate::app::data_dir());
            let _ = std::fs::write(
                crate::app::data_dir().join("native-ui-smoke.json"),
                json!({"ok":true,"platform":std::env::consts::OS,"webview":false}).to_string(),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let state = self.state.clone();
        let s = &state["student"];
        let t = &state["teacher"];
        egui::TopBottomPanel::top("navigation").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("OmniView Legacy");
                ui.label(format!(
                    "{} · {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS
                ));
                ui.separator();
                if ui.selectable_label(!self.settings, "Класс").clicked() {
                    self.settings = false;
                }
                if ui.selectable_label(self.settings, "Настройки").clicked() {
                    self.settings = true;
                }
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(state["notice"]["text"].as_str().unwrap_or(""));
            if let Some(status) = s["status"].as_str() {
                ui.label(status);
            }
            if !state["export_file"].as_str().unwrap_or("").is_empty() {
                ui.label(state["export_file"].as_str().unwrap_or(""));
            }
        });
        egui::CentralPanel::default().show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
   if self.settings{
    let mut prefs=state["prefs"].clone();ui.heading("Настройки");ui.label("Имя");ui.text_edit_singleline(&mut self.name);ui.label("IPv4 учителя");ui.text_edit_singleline(&mut self.host);
    let mut low=prefs["low"]==true;let mut auto=prefs["auto_updates"]==true;let mut preview=prefs["preview_updates"]==true;let mut changed=ui.checkbox(&mut low,"Экономный режим: 640 px раз в 5 секунд").changed();changed|=ui.checkbox(&mut auto,"Проверять обновления при запуске").changed();changed|=ui.checkbox(&mut preview,"Сборки новых коммитов").changed();
    if ui.button("Сохранить").clicked()||changed{prefs["name"]=json!(self.name);prefs["host"]=json!(self.host);prefs["low"]=json!(low);prefs["auto_updates"]=json!(auto);prefs["preview_updates"]=json!(preview);self.save_preferences(prefs);}
    ui.separator();ui.heading("Обновления");ui.label(state["updates"]["status"]["message"].as_str().unwrap_or(""));ui.label(state["updates"]["status"]["commit_title"].as_str().unwrap_or(""));if ui.button("Проверить обновления").clicked(){self.send(json!({"action":"check_updates"}));}ui.hyperlink_to("Скачать сборку своей системы","https://github.com/velmourcnikita/omniview-legacy/releases");ui.label("На Linux/macOS замена программы выполняется вручную после завершения сеанса.");ui.label("Portable: создай portable.flag рядом с программой, чтобы сохранять настройки в соседнюю data.");
    ui.separator();ui.label("Приём удалённого управления пока доступен в Windows Legacy. Linux/macOS: сообщения, задания, проверка процессов и показ экрана. Wayland: показ экрана пока недоступен, выбери X11.");return;
   }
   ui.horizontal(|ui|{let live=self.live();ui.add_enabled_ui(!live,|ui|{if ui.selectable_label(!self.student,"Преподаватель").clicked(){self.send(json!({"action":"role","role":"teacher"}));}if ui.selectable_label(self.student,"Ученик").clicked(){self.send(json!({"action":"role","role":"student"}));}});ui.label("Имя:");ui.text_edit_singleline(&mut self.name);});
   if !self.live(){ui.label("Пароль класса");ui.add(egui::TextEdit::singleline(&mut self.password).password(true));if self.student{ui.label("Код класса");ui.add(egui::TextEdit::multiline(&mut self.code).desired_rows(3).desired_width(f32::INFINITY));}if ui.button(if self.student{"Подключиться"}else{"Открыть класс"}).clicked(){let mut prefs=state["prefs"].clone();prefs["name"]=json!(self.name);prefs["host"]=json!(self.host);self.save_preferences(prefs);self.send(if self.student{json!({"action":"join","name":self.name,"code":self.code,"password":self.password})}else{json!({"action":"start_class","password":self.password})});}}
   else{if ui.button("Завершить сеанс").clicked(){self.send(json!({"action":if self.student{"leave"}else{"end_class"}}));}if !self.student{let code=t["invite"].as_str().unwrap_or("");ui.collapsing("Код подключения",|ui|{let mut value=code.to_string();ui.add(egui::TextEdit::multiline(&mut value).desired_rows(3));if ui.button("Копировать код").clicked(){ctx.copy_text(code.to_string());}});}}
   if self.student&&s["connected"]==true{
    let mut share=s["sharing"]==true;if ui.checkbox(&mut share,"Показывать мой экран учителю").changed(){self.send(json!({"action":"share","on":share}));}if ui.button("Нужна помощь").clicked(){self.send(json!({"action":"help"}));}
    if s["policy"]["active"]==true{ui.label(format!("Проверка процессов: {}",s["policy"]["blocked"]));let mut on=s["monitoring"]==true;if ui.checkbox(&mut on,"Разрешить проверку этого списка").changed(){self.send(json!({"action":"monitor_consent","on":on}));}}
    let q=&s["question"];if q.is_object(){ui.separator();ui.heading(q["prompt"].as_str().unwrap_or(""));if let Some(options)=q["options"].as_array(){for(i,value)in options.iter().enumerate(){ui.radio_value(&mut self.choice,i,value.as_str().unwrap_or(""));}}if ui.add_enabled(q["closed"]!=true&&q["own_choice"].is_null()&&s["answer_pending"]!=true,egui::Button::new("Ответить")).clicked(){self.send(json!({"action":"question_answer","id":q["id"],"choice":self.choice}));}if let Some(correct)=q["correct"].as_u64(){ui.label(format!("Правильный вариант: {}",correct+1));}}
   }
   if !self.student&&t["active"]==true{
    ui.horizontal(|ui|{if ui.button(if t["broadcasting"]==true{"Остановить показ"}else{"Показать мой экран классу"}).clicked(){self.send(json!({"action":"broadcast","on":t["broadcasting"]!=true}));}if ui.button("Весь класс").clicked(){self.selected=None;}if ui.button("Сохранить отчёт урока").clicked(){self.send(json!({"action":"lesson_report"}));}});
    if let Some(peers)=t["peers"].as_array(){ui.horizontal_wrapped(|ui|{for p in peers{let id=p["id"].as_str().unwrap_or("");if ui.selectable_label(self.selected.as_deref()==Some(id),format!("{} · {}{}",p["name"].as_str().unwrap_or(""),if p["online"]==true{"в сети"}else{"не в сети"},if p["help"]==true{" · помощь"}else{""})).clicked(){self.selected=Some(id.into());self.revision=0;}}});}
    ui.collapsing("Задание",|ui|{ui.text_edit_singleline(&mut self.question);ui.label("Варианты через ;");ui.text_edit_singleline(&mut self.options);ui.horizontal(|ui|{ui.label("Правильный вариант:");ui.add(egui::DragValue::new(&mut self.correct).range(1..=6));ui.label("Секунд:");ui.add(egui::DragValue::new(&mut self.seconds).range(15..=900));});if ui.button("Отправить вопрос").clicked(){self.send(json!({"action":"question_start","prompt":self.question,"options":self.options.split(';').map(str::trim).collect::<Vec<_>>(),"correct":self.correct.saturating_sub(1),"seconds":self.seconds}));}if ui.button("Завершить вопрос").clicked(){self.send(json!({"action":"question_close"}));}if let Some(questions)=t["lesson"]["questions"].as_array(){for q in questions{ui.label(format!("{}: {} ответов",q["prompt"].as_str().unwrap_or(""),q["answers"].as_object().map(|a|a.len()).unwrap_or(0)));}}});
    ui.collapsing("Самостоятельная работа",|ui|{ui.label("Точные имена процессов через ;. На разных ОС имена различаются.");ui.text_edit_singleline(&mut self.blocked);if ui.button("Предложить проверку").clicked(){self.send(json!({"action":"assessment_configure","blocked":self.blocked,"active":true}));}if ui.button("Остановить проверку").clicked(){self.send(json!({"action":"assessment_configure","blocked":self.blocked,"active":false}));}if let Some(events)=t["assessment"]["events"].as_array(){for event in events.iter().rev().take(20){ui.label(format!("{}: {}",event["name"].as_str().unwrap_or(""),event["executable"].as_str().unwrap_or("")));}}});
   }
   ui.separator();if let Some(texture)=&self.texture{ui.add(egui::Image::new(texture).max_width(ui.available_width()).max_height(360.0));}else{ui.label(RichText::new("Экран не передаётся").weak());}
   ui.separator();ui.horizontal(|ui|{ui.add(egui::TextEdit::singleline(&mut self.message).hint_text("Сообщение"));if ui.button("Отправить").clicked(){self.send(if self.student{json!({"action":"student_message","text":self.message})}else{json!({"action":"send_message","target":self.selected.clone().unwrap_or_else(||"all".into()),"text":self.message})});self.message.clear();}});
   if let Some(messages)=if self.student{s["messages"].as_array()}else{t["log"].as_array()}{for m in messages.iter().rev().take(20){ui.label(format!("{}: {}",m["from"].as_str().unwrap_or(""),m["text"].as_str().unwrap_or("")));}}
  });});
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.core.stop();
    }
}
