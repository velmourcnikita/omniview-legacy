//! Native classroom console. Shared TLS, invitation, consent and lesson engine.
use crate::app::Core;
use serde_json::{json, Value};
use std::{ptr::null_mut, sync::Arc};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, SetFocus, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, WindowsAndMessaging::*},
};
const ROLE: u16 = 10;
const NAME: u16 = 11;
const HOST: u16 = 12;
const PASSWORD: u16 = 13;
const CODE: u16 = 14;
const CONNECT: u16 = 15;
const LOW: u16 = 16;
const PEERS: u16 = 17;
const SHARE: u16 = 18;
const HELP: u16 = 19;
const BROADCAST: u16 = 20;
const MESSAGE: u16 = 21;
const SEND: u16 = 22;
const MONITOR: u16 = 23;
const CONTROL: u16 = 24;
const REQUEST: u16 = 25;
const REPORT: u16 = 26;
const LOG: u16 = 27;
const STATUS: u16 = 28;
const PROMPT: u16 = 30;
const OPTIONS: u16 = 31;
const CORRECT: u16 = 32;
const SECONDS: u16 = 33;
const PUBLISH: u16 = 34;
const CLOSE: u16 = 35;
const ANSWERS: u16 = 36;
const SUBMIT: u16 = 37;
const BLOCKED: u16 = 38;
const POLICY: u16 = 39;
const STOP_POLICY: u16 = 40;
const CHECK: u16 = 41;
const DOWNLOAD: u16 = 42;
const INSTALL: u16 = 43;
const PREVIEW: u16 = 44;
const UPDATE_STATUS: u16 = 45;
const COPY: u16 = 46;
const AUTO: u16 = 49;
const NOTICE: u16 = 47;
const NOTICE_CLOSE: u16 = 48;
struct Ui {
    core: Arc<Core>,
    window: HWND,
    revision: u64,
    state: Value,
    peers: Vec<String>,
    selected: Option<String>,
    bitmap: Vec<u8>,
    w: i32,
    h: i32,
    frame: String,
    notice: u64,
    notice_until: u64,
    notice_text: String,
    question: String,
    last_log: String,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn error(s: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(s).as_ptr(),
            wide("OmniView Legacy").as_ptr(),
            MB_ICONERROR,
        );
    }
}
pub fn run() -> Result<(), String> {
    unsafe {
        let student = std::env::args().any(|a| a == "--student")
            || std::env::current_exe()
                .ok()
                .is_some_and(|p| p.to_string_lossy().to_lowercase().contains("student"));
        let core = Arc::new(Core::new(student));
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("OmniViewLegacyNative");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Не удалось зарегистрировать окно".into());
        }
        let ui = Box::new(Ui {
            core: core.clone(),
            window: null_mut(),
            revision: 0,
            state: Value::Null,
            peers: Vec::new(),
            selected: None,
            bitmap: Vec::new(),
            w: 0,
            h: 0,
            frame: String::new(),
            notice: 0,
            notice_until: 0,
            notice_text: String::new(),
            question: String::new(),
            last_log: String::new(),
        });
        let raw = Box::into_raw(ui);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("OmniView Legacy").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_VSCROLL | WS_HSCROLL,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1120.min(GetSystemMetrics(SM_CXSCREEN) - 40),
            850.min(GetSystemMetrics(SM_CYSCREEN) - 50),
            null_mut(),
            null_mut(),
            instance,
            raw as _,
        );
        if window.is_null() {
            drop(Box::from_raw(raw));
            return Err("Не удалось открыть окно".into());
        }
        (*raw).window = window;
        controls(&mut *raw);
        SetTimer(window, 1, 250, None);
        let smoke = std::env::args().any(|a| a == "--ui-smoke");
        ShowWindow(window, if smoke { SW_HIDE } else { SW_SHOW });
        UpdateWindow(window);
        if !smoke && (*raw).core.snapshot()["prefs"]["auto_updates"] == true {
            dispatch(&*raw, json!({"action":"check_updates"}));
        }
        if std::env::args().any(|a| a == "--ui-smoke") {
            SetTimer(window, 2, 1500, None);
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(window, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        drop(Box::from_raw(raw));
        Ok(())
    }
}
unsafe fn child(
    ui: &Ui,
    class: &str,
    text: &str,
    id: u16,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
) -> HWND {
    let hwnd = CreateWindowExW(
        if class == "EDIT" { WS_EX_CLIENTEDGE } else { 0 },
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
        x,
        y,
        w,
        h,
        ui.window,
        id as usize as HMENU,
        GetModuleHandleW(std::ptr::null()),
        null_mut(),
    );
    SendMessageW(
        hwnd,
        WM_SETFONT,
        GetStockObject(DEFAULT_GUI_FONT) as usize,
        1,
    );
    hwnd
}
unsafe fn label(ui: &Ui, text: &str, x: i32, y: i32, w: i32) {
    child(ui, "STATIC", text, 0, x, y, w, 20, 0);
}
unsafe fn edit(ui: &Ui, text: &str, id: u16, x: i32, y: i32, w: i32, h: i32, multi: bool) {
    let ctl = child(
        ui,
        "EDIT",
        text,
        id,
        x,
        y,
        w,
        h,
        if multi {
            ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL
        } else {
            ES_AUTOHSCROLL as u32
        },
    );
    SendMessageW(
        ctl,
        EM_SETLIMITTEXT,
        if id == CODE { 16004 } else { 1700 },
        0,
    );
}
unsafe fn button(ui: &Ui, text: &str, id: u16, x: i32, y: i32, w: i32) {
    child(ui, "BUTTON", text, id, x, y, w, 28, BS_PUSHBUTTON as u32);
}
unsafe fn check(ui: &Ui, text: &str, id: u16, x: i32, y: i32, w: i32, on: bool) {
    let c = child(ui, "BUTTON", text, id, x, y, w, 26, BS_AUTOCHECKBOX as u32);
    SendMessageW(
        c,
        BM_SETCHECK,
        if on { BST_CHECKED } else { BST_UNCHECKED } as usize,
        0,
    );
}
unsafe fn controls(ui: &mut Ui) {
    let s = ui.core.snapshot();
    let p = &s["prefs"];
    label(ui, "OmniView Legacy · компьютерный класс", 18, 12, 420);
    check(
        ui,
        "Проверять обновления при запуске",
        AUTO,
        760,
        8,
        300,
        p["auto_updates"] == true,
    );
    let role = child(
        ui,
        "COMBOBOX",
        "",
        ROLE,
        18,
        38,
        175,
        160,
        CBS_DROPDOWNLIST as u32,
    );
    for name in ["Преподаватель", "Ученик"] {
        SendMessageW(role, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
    }
    SendMessageW(
        role,
        CB_SETCURSEL,
        if s["role"] == "student" { 1 } else { 0 },
        0,
    );
    edit(
        ui,
        p["name"].as_str().unwrap_or(""),
        NAME,
        205,
        38,
        190,
        26,
        false,
    );
    edit(
        ui,
        p["host"].as_str().unwrap_or(""),
        HOST,
        408,
        38,
        155,
        26,
        false,
    );
    edit(ui, "", PASSWORD, 575, 38, 190, 26, false);
    SendMessageW(
        GetDlgItem(ui.window, PASSWORD as i32),
        EM_SETPASSWORDCHAR,
        0x25cf,
        0,
    );
    label(ui, "Режим", 18, 70, 160);
    label(ui, "Имя", 205, 70, 180);
    label(ui, "IPv4 учителя", 408, 70, 155);
    label(ui, "Пароль класса (от 8 символов)", 575, 70, 240);
    button(ui, "Открыть / подключиться", CONNECT, 790, 37, 270);
    edit(ui, "", CODE, 18, 98, 890, 57, true);
    button(ui, "Копировать код", COPY, 920, 99, 140);
    label(
        ui,
        "Код класса: учитель передаёт код и пароль ученикам",
        18,
        161,
        700,
    );
    check(ui, "Экономный режим", LOW, 18, 185, 190, p["low"] == true);
    check(ui, "Показывать мой экран", SHARE, 220, 185, 210, false);
    button(ui, "Нужна помощь", HELP, 440, 185, 150);
    button(ui, "Показать мой экран классу", BROADCAST, 610, 185, 240);
    button(ui, "Отчёт урока", REPORT, 870, 185, 190);
    child(
        ui,
        "LISTBOX",
        "",
        PEERS,
        18,
        223,
        310,
        190,
        LBS_NOTIFY as u32 | WS_VSCROLL | WS_BORDER,
    );
    label(
        ui,
        "Экран выбранного ученика / преподавателя",
        350,
        223,
        650,
    );
    button(ui, "Запросить управление", REQUEST, 18, 420, 310);
    check(
        ui,
        "Разрешить управление (до 5 мин, Esc — стоп)",
        CONTROL,
        350,
        420,
        480,
        false,
    );
    label(
        ui,
        "Сообщение выбранному ученику или всему классу",
        18,
        460,
        680,
    );
    edit(ui, "", MESSAGE, 18, 484, 710, 28, false);
    button(ui, "Отправить", SEND, 740, 484, 150);
    button(ui, "Завершить вопрос", CLOSE, 900, 484, 160);
    edit(ui, "", LOG, 18, 520, 1042, 62, true);
    SendMessageW(GetDlgItem(ui.window, LOG as i32), EM_SETREADONLY, 1, 0);
    label(
        ui,
        "Задание: вопрос / варианты через ; / номер правильного / секунды",
        18,
        590,
        1030,
    );
    edit(ui, "", PROMPT, 18, 614, 330, 27, false);
    edit(ui, "", OPTIONS, 360, 614, 365, 27, false);
    edit(ui, "1", CORRECT, 737, 614, 42, 27, false);
    edit(ui, "60", SECONDS, 790, 614, 50, 27, false);
    button(ui, "Отправить вопрос", PUBLISH, 855, 613, 205);
    let answers = child(
        ui,
        "COMBOBOX",
        "",
        ANSWERS,
        18,
        648,
        710,
        180,
        CBS_DROPDOWNLIST as u32,
    );
    SendMessageW(answers, CB_SETCURSEL, 0, 0);
    button(ui, "Ответить", SUBMIT, 740, 648, 150);
    edit(
        ui,
        "chrome.exe; discord.exe",
        BLOCKED,
        18,
        684,
        465,
        27,
        false,
    );
    button(ui, "Предложить проверку", POLICY, 495, 684, 200);
    button(ui, "Остановить", STOP_POLICY, 705, 684, 140);
    check(ui, "Разрешить проверку", MONITOR, 855, 684, 205, false);
    button(ui, "Проверить обновления", CHECK, 18, 725, 220);
    button(ui, "Загрузить", DOWNLOAD, 250, 725, 120);
    button(ui, "Установить и выйти", INSTALL, 382, 725, 180);
    check(
        ui,
        "Новые коммиты",
        PREVIEW,
        580,
        725,
        190,
        p["preview_updates"] == true,
    );
    child(ui, "STATIC", "", STATUS, 18, 760, 1042, 24, 0);
    child(ui, "STATIC", "", UPDATE_STATUS, 18, 786, 1042, 24, 0);
    child(
        ui,
        "STATIC",
        "",
        NOTICE,
        350,
        264,
        610,
        100,
        1u32 | WS_BORDER,
    );
    button(ui, "Закрыть уведомление", NOTICE_CLOSE, 515, 372, 240);
    ShowWindow(GetDlgItem(ui.window, NOTICE as i32), SW_HIDE);
    ShowWindow(GetDlgItem(ui.window, NOTICE_CLOSE as i32), SW_HIDE);
}
unsafe fn text(ui: &Ui, id: u16) -> String {
    let h = GetDlgItem(ui.window, id as i32);
    let n = GetWindowTextLengthW(h).min(16004);
    let mut b = vec![0u16; n as usize + 1];
    let n = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}
unsafe fn set(ui: &Ui, id: u16, s: &str) {
    SetWindowTextW(GetDlgItem(ui.window, id as i32), wide(s).as_ptr());
}
unsafe fn checked(ui: &Ui, id: u16) -> bool {
    SendMessageW(GetDlgItem(ui.window, id as i32), BM_GETCHECK, 0, 0) == BST_CHECKED as isize
}
unsafe fn enable(ui: &Ui, id: u16, on: bool) {
    EnableWindow(GetDlgItem(ui.window, id as i32), on as i32);
}
fn dispatch(ui: &Ui, value: Value) {
    let core = ui.core.clone();
    std::thread::spawn(move || {
        if let Err(e) = core.handle(value) {
            core.feedback(&e);
        }
    });
}
unsafe fn save(ui: &Ui) {
    let mut prefs = ui.state["prefs"].clone();
    prefs["name"] = json!(text(ui, NAME));
    prefs["host"] = json!(text(ui, HOST));
    prefs["low"] = json!(checked(ui, LOW));
    prefs["preview_updates"] = json!(checked(ui, PREVIEW));
    prefs["auto_updates"] = json!(checked(ui, AUTO));
    if let Err(e) = ui
        .core
        .handle(json!({"action":"save_settings","prefs":prefs}))
    {
        ui.core.feedback(&e);
    }
}
unsafe fn action(ui: &mut Ui, id: u16, code: u16) {
    let student = ui.state["role"] == "student";
    let s = &ui.state["student"];
    let t = &ui.state["teacher"];
    match id {
        ROLE if code == CBN_SELCHANGE as u16 => {
            let student = SendMessageW(GetDlgItem(ui.window, ROLE as i32), CB_GETCURSEL, 0, 0) == 1;
            if let Err(e) = ui
                .core
                .handle(json!({"action":"role","role":if student{"student"}else{"teacher"}}))
            {
                ui.core.feedback(&e);
            }
            ui.revision = 0;
        }
        LOW | PREVIEW | AUTO => save(ui),
        CONNECT => {
            save(ui);
            let value = if student {
                if s["connected"] == true || s["connecting"] == true {
                    json!({"action":"leave"})
                } else {
                    json!({"action":"join","name":text(ui,NAME),"code":text(ui,CODE),"password":text(ui,PASSWORD)})
                }
            } else if t["active"] == true {
                json!({"action":"end_class"})
            } else {
                json!({"action":"start_class","password":text(ui,PASSWORD)})
            };
            dispatch(ui, value);
        }
        COPY => {
            let h = GetDlgItem(ui.window, CODE as i32);
            SendMessageW(h, EM_SETSEL, 0, -1);
            SendMessageW(h, WM_COPY, 0, 0);
        }
        SHARE => dispatch(ui, json!({"action":"share","on":checked(ui,SHARE)})),
        HELP => {
            if student {
                dispatch(ui, json!({"action":"help"}));
            } else if let Some(id) = &ui.selected {
                dispatch(ui, json!({"action":"ack_help","id":id}));
            }
        }
        BROADCAST => dispatch(
            ui,
            json!({"action":"broadcast","on":t["broadcasting"]!=true}),
        ),
        PEERS if code == LBN_SELCHANGE as u16 => {
            let i = SendMessageW(GetDlgItem(ui.window, PEERS as i32), LB_GETCURSEL, 0, 0);
            ui.selected = ui.peers.get(i as usize).cloned();
            ui.revision = 0;
        }
        SEND => {
            let message = text(ui, MESSAGE);
            dispatch(
                ui,
                if student {
                    json!({"action":"student_message","text":message})
                } else {
                    json!({"action":"send_message","target":ui.selected.clone().unwrap_or_else(||"all".into()),"text":message})
                },
            );
            set(ui, MESSAGE, "");
        }
        MONITOR => dispatch(
            ui,
            json!({"action":"monitor_consent","on":checked(ui,MONITOR)}),
        ),
        CONTROL => dispatch(
            ui,
            json!({"action":"control_consent","on":checked(ui,CONTROL),"request":s["control_request"]["id"]}),
        ),
        REQUEST => {
            if let Some(ref id) = ui.selected {
                dispatch(
                    ui,
                    json!({"action":"control_request","id":id,"on":!ui.state["teacher"]["peers"].as_array().is_some_and(|peers|peers.iter().any(|p|p["id"]==*id&&p["control"]["request"].is_object()))}),
                );
            }
        }
        REPORT => dispatch(ui, json!({"action":"lesson_report"})),
        PUBLISH => {
            let options: Vec<_> = text(ui, OPTIONS)
                .split(';')
                .map(|s| s.trim().to_string())
                .collect();
            dispatch(
                ui,
                json!({"action":"question_start","prompt":text(ui,PROMPT),"options":options,"correct":text(ui,CORRECT).parse::<usize>().unwrap_or(1).saturating_sub(1),"seconds":text(ui,SECONDS).parse::<u64>().unwrap_or(60)}),
            );
        }
        CLOSE => dispatch(ui, json!({"action":"question_close"})),
        SUBMIT => {
            let i = SendMessageW(GetDlgItem(ui.window, ANSWERS as i32), CB_GETCURSEL, 0, 0);
            if i >= 0 {
                dispatch(
                    ui,
                    json!({"action":"question_answer","id":s["question"]["id"],"choice":i}),
                );
            }
        }
        POLICY | STOP_POLICY => dispatch(
            ui,
            json!({"action":"assessment_configure","blocked":text(ui,BLOCKED),"active":id==POLICY}),
        ),
        CHECK => {
            save(ui);
            dispatch(ui, json!({"action":"check_updates"}));
        }
        DOWNLOAD => dispatch(ui, json!({"action":"download_update"})),
        INSTALL => match ui.core.handle(json!({"action":"install_update"})) {
            Ok(()) => {
                DestroyWindow(ui.window);
            }
            Err(e) => ui.core.feedback(&e),
        },
        NOTICE_CLOSE => {
            if s["control_active"] == true {
                dispatch(ui, json!({"action":"control_consent","on":false}));
            }
            ui.notice_until = 0;
        }
        _ => {}
    }
}
unsafe fn refresh(ui: &mut Ui) {
    let revision = ui.core.revision();
    let now = crate::classroom::now();
    if revision != ui.revision {
        ui.revision = revision;
        ui.state = ui.core.snapshot();
        let st = ui.state.clone();
        let student = st["role"] == "student";
        let t = &st["teacher"];
        let s = &st["student"];
        let active = if student {
            s["connected"] == true || s["connecting"] == true
        } else {
            t["active"] == true
        };
        set(
            ui,
            CONNECT,
            if active {
                "Завершить сеанс"
            } else if student {
                "Подключиться"
            } else {
                "Открыть класс"
            },
        );
        enable(ui, ROLE, !active);
        SendMessageW(
            GetDlgItem(ui.window, ROLE as i32),
            CB_SETCURSEL,
            if student { 1 } else { 0 },
            0,
        );
        for id in [SHARE, MONITOR] {
            enable(ui, id, student && s["connected"] == true);
        }
        enable(ui, HELP, active);
        set(
            ui,
            HELP,
            if student {
                "Нужна помощь"
            } else {
                "Отметить помощь"
            },
        );
        enable(
            ui,
            CONTROL,
            student && s["control_request"].is_object() && s["sharing"] == true,
        );
        for id in [BROADCAST, REPORT, PUBLISH, CLOSE, POLICY, STOP_POLICY] {
            enable(ui, id, !student && active);
        }
        enable(ui, REQUEST, !student && active && ui.selected.is_some());
        for (id, key) in [
            (SHARE, "sharing"),
            (MONITOR, "monitoring"),
            (CONTROL, "control_active"),
        ] {
            SendMessageW(
                GetDlgItem(ui.window, id as i32),
                BM_SETCHECK,
                if s[key] == true {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                } as usize,
                0,
            );
        }
        if !student {
            set(ui, CODE, t["invite"].as_str().unwrap_or(""));
        }
        let peers = t["peers"].as_array().cloned().unwrap_or_default();
        let list = peers
            .iter()
            .map(|p| {
                format!(
                    "{} · {}{}",
                    p["name"].as_str().unwrap_or(""),
                    if p["online"] == true {
                        "в сети"
                    } else {
                        "не в сети"
                    },
                    if p["help"] == true {
                        " · нужна помощь"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>();
        let signature = list.join("\n");
        let old = ui.peers.clone();
        ui.peers = peers
            .iter()
            .filter_map(|p| p["id"].as_str().map(str::to_string))
            .collect();
        let h = GetDlgItem(ui.window, PEERS as i32);
        if old != ui.peers || text_list(h) != signature {
            SendMessageW(h, LB_RESETCONTENT, 0, 0);
            for name in &list {
                SendMessageW(h, LB_ADDSTRING, 0, wide(name).as_ptr() as isize);
            }
            if let Some(i) = ui
                .peers
                .iter()
                .position(|p| Some(p) == ui.selected.as_ref())
            {
                SendMessageW(h, LB_SETCURSEL, i, 0);
            }
        }
        let log = if student {
            s["messages"].as_array()
        } else {
            t["log"].as_array()
        }
        .map(|l| {
            l.iter()
                .rev()
                .take(12)
                .rev()
                .map(|m| {
                    format!(
                        "{}: {}",
                        m["from"].as_str().unwrap_or(""),
                        m["text"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\r\n")
        })
        .unwrap_or_default();
        let results = if !student {
            t["lesson"]["questions"]
                .as_array()
                .map(|questions| {
                    questions
                        .iter()
                        .map(|q| {
                            format!(
                                "{}: {} ответов",
                                q["prompt"].as_str().unwrap_or(""),
                                q["answers"].as_object().map(|a| a.len()).unwrap_or(0)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let log = format!(
            "{log}\r\n{results}\r\n{}",
            st["export_file"].as_str().unwrap_or("")
        );
        if log != ui.last_log {
            set(ui, LOG, &log);
            ui.last_log = log;
        }
        let q = &s["question"];
        let signature = q.to_string();
        if signature != ui.question {
            ui.question = signature;
            if student {
                set(ui, PROMPT, q["prompt"].as_str().unwrap_or(""));
                let h = GetDlgItem(ui.window, ANSWERS as i32);
                SendMessageW(h, CB_RESETCONTENT, 0, 0);
                if let Some(options) = q["options"].as_array() {
                    for o in options {
                        SendMessageW(
                            h,
                            CB_ADDSTRING,
                            0,
                            wide(o.as_str().unwrap_or("")).as_ptr() as isize,
                        );
                    }
                    SendMessageW(h, CB_SETCURSEL, 0, 0);
                }
            }
        }
        enable(
            ui,
            SUBMIT,
            student
                && q.is_object()
                && q["closed"] != true
                && s["answer_pending"] != true
                && q["own_choice"].is_null(),
        );
        let policy = s["policy"]["blocked"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        if student {
            set(ui, BLOCKED, &policy);
        }
        let notice = st["notice"]["id"].as_u64().unwrap_or(0);
        if notice != 0 && notice != ui.notice {
            ui.notice = notice;
            ui.notice_text = st["notice"]["text"].as_str().unwrap_or("").into();
            ui.notice_until = now
                + st["prefs"]["notification_seconds"]
                    .as_u64()
                    .unwrap_or(9)
                    .clamp(5, 60)
                    * 1000;
        }
        set(
            ui,
            STATUS,
            &format!(
                "{} · {}",
                if student {
                    s["status"].as_str().unwrap_or("Вне класса")
                } else if active {
                    "Класс открыт"
                } else {
                    "Класс закрыт"
                },
                ui.notice_text
            ),
        );
        let u = &st["updates"]["status"];
        set(
            ui,
            UPDATE_STATUS,
            &format!(
                "{} · {} {}",
                u["message"].as_str().unwrap_or(""),
                u["commit"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(7)
                    .collect::<String>(),
                u["commit_title"].as_str().unwrap_or("")
            ),
        );
        enable(ui, CHECK, u["busy"] != true);
        enable(ui, DOWNLOAD, u["available"] == true && u["busy"] != true);
        enable(ui, INSTALL, u["prepared"] == true && !active);
        let image = if student {
            if s["broadcasting"] == true {
                s["broadcast"]["seq"]
                    .as_u64()
                    .map(|n| format!("/preview/teacher/{n}"))
            } else {
                None
            }
        } else {
            ui.selected.as_ref().and_then(|id| {
                peers
                    .iter()
                    .find(|p| p["id"] == *id && p["sharing"] == true && p["online"] == true)
                    .and_then(|p| {
                        p["frame"]["seq"]
                            .as_u64()
                            .map(|n| format!("/preview/student/{id}/{n}"))
                    })
            })
        };
        let key = image.unwrap_or_default();
        if key != ui.frame {
            ui.frame = key.clone();
            ui.bitmap.clear();
            if let Some(bytes) = ui.core.image(&key) {
                if let Ok(image) = image::load_from_memory(&bytes) {
                    let mut image = image.to_rgba8();
                    ui.w = image.width() as i32;
                    ui.h = image.height() as i32;
                    for pixel in image.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    ui.bitmap = image.into_raw();
                }
            }
            InvalidateRect(ui.window, std::ptr::null(), 0);
        }
    }
    let assisting = ui.state["student"]["control_active"] == true;
    let show = assisting || now < ui.notice_until;
    if show {
        set(
            ui,
            NOTICE,
            if assisting {
                "Управление разрешено преподавателю. Esc или кнопка ниже остановит его."
            } else {
                &ui.notice_text
            },
        );
    }
    ShowWindow(
        GetDlgItem(ui.window, NOTICE as i32),
        if show { SW_SHOWNOACTIVATE } else { SW_HIDE },
    );
    ShowWindow(
        GetDlgItem(ui.window, NOTICE_CLOSE as i32),
        if show { SW_SHOWNOACTIVATE } else { SW_HIDE },
    );
}
unsafe fn text_list(h: HWND) -> String {
    let n = SendMessageW(h, LB_GETCOUNT, 0, 0);
    let mut lines = Vec::new();
    for i in 0..n.max(0) {
        let len = SendMessageW(h, LB_GETTEXTLEN, i as usize, 0).max(0) as usize;
        let mut b = vec![0u16; len + 1];
        SendMessageW(h, LB_GETTEXT, i as usize, b.as_mut_ptr() as isize);
        lines.push(String::from_utf16_lossy(&b[..len]));
    }
    lines.join("\n")
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
    }
    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Ui;
    if !raw.is_null() {
        let ui = &mut *raw;
        match msg {
            WM_VSCROLL | WM_HSCROLL => {
                let bar = if msg == WM_VSCROLL { SB_VERT } else { SB_HORZ };
                let previous = GetScrollPos(hwnd, bar);
                let command = (w & 0xffff) as i32;
                let mut position = previous;
                let step = if msg == WM_VSCROLL { 36 } else { 48 };
                match command {
                    SB_LINEUP => position -= step,
                    SB_LINEDOWN => position += step,
                    SB_PAGEUP => position -= step * 4,
                    SB_PAGEDOWN => position += step * 4,
                    SB_THUMBPOSITION | SB_THUMBTRACK => position = ((w >> 16) & 0xffff) as i32,
                    _ => {}
                }
                SetScrollPos(hwnd, bar, position, 1);
                let current = GetScrollPos(hwnd, bar);
                ScrollWindow(
                    hwnd,
                    if msg == WM_HSCROLL {
                        previous - current
                    } else {
                        0
                    },
                    if msg == WM_VSCROLL {
                        previous - current
                    } else {
                        0
                    },
                    std::ptr::null(),
                    std::ptr::null(),
                );
                return 0;
            }
            WM_SIZE => {
                let width = (l & 0xffff) as i32;
                let height = ((l >> 16) & 0xffff) as i32;
                SetScrollRange(hwnd, SB_VERT, 0, (820 - height).max(0), 1);
                SetScrollRange(hwnd, SB_HORZ, 0, (1080 - width).max(0), 1);
                InvalidateRect(hwnd, std::ptr::null(), 1);
                return 0;
            }
            WM_LBUTTONDOWN => {
                let x = (l & 0xffff) as i16 as i32 + GetScrollPos(hwnd, SB_HORZ);
                let y = ((l >> 16) & 0xffff) as i16 as i32 + GetScrollPos(hwnd, SB_VERT);
                if let Some((nx, ny)) = screen_point(ui, x, y) {
                    SetFocus(hwnd);
                    remote(
                        ui,
                        json!([{"kind":"move","x":nx,"y":ny},{"kind":"left","down":true},{"kind":"left","down":false}]),
                    );
                }
                return 0;
            }
            WM_KEYDOWN => {
                if w == 27 {
                    dispatch(ui, json!({"action":"control_consent","on":false}));
                    return 0;
                }
                let ctrl = GetAsyncKeyState(VK_CONTROL as i32) < 0;
                let alt = GetAsyncKeyState(VK_MENU as i32) < 0;
                if ctrl || alt || matches!(w,8|9|13|33..=40|45|46|112..=123) {
                    remote(
                        ui,
                        json!([{"kind":"key","vk":w,"ctrl":ctrl,"alt":alt,"shift":GetAsyncKeyState(VK_SHIFT as i32)<0}]),
                    );
                }
                return 0;
            }
            WM_CHAR => {
                if GetAsyncKeyState(VK_CONTROL as i32) >= 0 && GetAsyncKeyState(VK_MENU as i32) >= 0
                {
                    if let Some(ch) = char::from_u32(w as u32) {
                        if !ch.is_control() {
                            remote(ui, json!([{"kind":"text","text":ch.to_string()}]));
                        }
                    }
                }
                return 0;
            }
            WM_COMMAND => {
                action(ui, (w & 0xffff) as u16, ((w >> 16) & 0xffff) as u16);
                return 0;
            }
            WM_TIMER if w == 1 => {
                refresh(ui);
                return 0;
            }
            WM_TIMER if w == 2 => {
                let _ = std::fs::create_dir_all(crate::app::data_dir());
                let _=std::fs::write(crate::app::data_dir().join("native-ui-smoke.json"),json!({"ok":GetDlgItem(hwnd,CONNECT as i32)!=null_mut(),"role":ui.state["role"],"arch":std::env::consts::ARCH,"webview":false}).to_string());
                DestroyWindow(hwnd);
                return 0;
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(l as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 1000, y: 650 };
                return 0;
            }
            WM_PAINT => {
                let mut ps = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let r = RECT {
                    left: 350 - GetScrollPos(hwnd, SB_HORZ),
                    top: 248 - GetScrollPos(hwnd, SB_VERT),
                    right: 1060 - GetScrollPos(hwnd, SB_HORZ),
                    bottom: 410 - GetScrollPos(hwnd, SB_VERT),
                };
                FillRect(dc, &r, (COLOR_BTNFACE + 1) as HBRUSH);
                if !ui.bitmap.is_empty() {
                    let scale = (710.0 / ui.w as f64).min(162.0 / ui.h as f64);
                    let w = (ui.w as f64 * scale) as i32;
                    let h = (ui.h as f64 * scale) as i32;
                    let mut info: BITMAPINFO = std::mem::zeroed();
                    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    info.bmiHeader.biWidth = ui.w;
                    info.bmiHeader.biHeight = -ui.h;
                    info.bmiHeader.biPlanes = 1;
                    info.bmiHeader.biBitCount = 32;
                    StretchDIBits(
                        dc,
                        350 - GetScrollPos(hwnd, SB_HORZ),
                        248 - GetScrollPos(hwnd, SB_VERT),
                        w,
                        h,
                        0,
                        0,
                        ui.w,
                        ui.h,
                        ui.bitmap.as_ptr() as _,
                        &info,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                }
                EndPaint(hwnd, &ps);
                return 0;
            }
            WM_CLOSE => {
                ui.core.stop();
                DestroyWindow(hwnd);
                return 0;
            }
            WM_DESTROY => {
                ui.core.stop();
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
                return 0;
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, w, l)
}

unsafe fn screen_point(ui: &Ui, x: i32, y: i32) -> Option<(u16, u16)> {
    if ui.bitmap.is_empty() {
        return None;
    }
    let scale = (710.0 / ui.w as f64).min(162.0 / ui.h as f64);
    let w = ui.w as f64 * scale;
    let h = ui.h as f64 * scale;
    if x < 350 || y < 248 || (x - 350) as f64 >= w || (y - 248) as f64 >= h {
        return None;
    }
    Some((
        ((x - 350) as f64 / w * 65535.0) as u16,
        ((y - 248) as f64 / h * 65535.0) as u16,
    ))
}
fn remote(ui: &Ui, events: Value) {
    if ui.state["role"] != "teacher" {
        return;
    }
    if let Some(id) = &ui.selected {
        if let Some(peer) = ui.state["teacher"]["peers"]
            .as_array()
            .and_then(|a| a.iter().find(|p| p["id"] == *id))
        {
            if peer["control"]["active"] == true {
                dispatch(
                    ui,
                    json!({"action":"control_input","id":id,"request":peer["control"]["request"]["id"],"events":events}),
                );
            }
        }
    }
}
