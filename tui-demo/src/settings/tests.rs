use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::popup::Popup;
use super::test_support::{config_all, model_list};
use super::{Outcome, Settings, Texts, Tone};
use crate::config::Config;
use crate::core::Refusal;

fn texts() -> Texts {
    Config::builtin().unwrap().text.settings
}

fn press(page: &mut Settings, code: KeyCode) -> Outcome {
    page.key(KeyEvent::new(code, KeyModifiers::NONE), &texts())
}

/// 开页、把三条读的回应交回去，进「供应商和模型」。
fn opened(standalone: bool) -> Settings {
    let mut page = Settings::open(standalone, true);
    answer_loads(&mut page);
    press(&mut page, KeyCode::Enter);
    page
}

fn answer_loads(page: &mut Settings) {
    for (tag, method, _) in page.take_asks() {
        let got = match method {
            "model.list" => model_list(),
            "config.get" => config_all(),
            _ => json!({"secrets": [{"name": "relay-key", "set": true}]}),
        };
        page.answer(tag, Ok(got), &texts());
    }
}

#[test]
fn opening_reads_what_it_needs_and_a_change_during_the_read_reads_again() {
    let mut page = Settings::open(false, true);
    let asks = page.take_asks();
    let methods: Vec<&str> = asks.iter().map(|(_, m, _)| *m).collect();
    // 供应商和模型那三样，加上通用这类页面的清单、人格（2026-10-07 加）、预设（2026-10-08 加）、吉祥物包（2026-10-11 加）。
    assert_eq!(
        methods,
        [
            "model.list",
            "config.get",
            "secret.list",
            "config.schema",
            "persona.list",
            "preset.list",
            "package.list"
        ]
    );
    assert_eq!(asks[1].2, json!({"all": true}));
    page.changed();
    assert!(page.take_asks().is_empty(), "正在读：读完再补");
    for (tag, method, _) in asks {
        let got = if method == "model.list" {
            model_list()
        } else {
            config_all()
        };
        page.answer(tag, Ok(got), &texts());
    }
    assert!(!page.loading());
    assert_eq!(page.take_asks().len(), 7, "读完补读一次");
}

#[test]
fn escape_from_the_menu_quits_only_when_started_for_the_config_page() {
    let mut page = opened(true);
    assert_eq!(press(&mut page, KeyCode::Esc), Outcome::Stay, "回主菜单");
    assert!(page.on_menu);
    assert_eq!(press(&mut page, KeyCode::Esc), Outcome::Quit);
    let mut page = opened(false);
    press(&mut page, KeyCode::Esc);
    assert_eq!(
        press(&mut page, KeyCode::Esc),
        Outcome::Back,
        "对话里打开的回对话"
    );
}

#[test]
fn saving_stores_new_keys_first_then_writes_the_config_with_their_names() {
    let mut page = opened(false);
    press(&mut page, KeyCode::Char('a'));
    assert!(matches!(page.popup, Some(Popup::Form(_))));
    press(&mut page, KeyCode::Enter);
    for c in "zen".chars() {
        press(&mut page, KeyCode::Char(c));
    }
    press(&mut page, KeyCode::Enter);
    // 挪到 key 那一行（ID、显示名、地址、接口、认证、key）。
    for _ in 0..5 {
        press(&mut page, KeyCode::Down);
    }
    press(&mut page, KeyCode::Enter);
    for c in "sk-zen".chars() {
        press(&mut page, KeyCode::Char(c));
    }
    press(&mut page, KeyCode::Enter);
    // 在窗里按 s 当场存（2026-10-07 项目主人：不用回到页面再存一次）。
    press(&mut page, KeyCode::Char('s'));
    assert!(page.popup.is_some(), "回应到了才关窗");
    assert_eq!(
        page.nav.provider(&page.view).unwrap().id,
        "zen",
        "选中新加的"
    );
    let asks = page.take_asks();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].1, "secret.set");
    assert_eq!(asks[0].2, json!({"name": "zen-key", "value": "sk-zen"}));
    page.answer(asks[0].0, Ok(json!({"replaced": false})), &texts());
    let asks = page.take_asks();
    assert_eq!(asks[0].1, "config.set");
    let sent = asks[0].2.to_string();
    assert!(sent.contains("\"providers.zen.key\""));
    assert!(sent.contains("zen-key"));
    assert!(!sent.contains("sk-zen"), "明文不进配置");
    page.answer(asks[0].0, Ok(json!({"keys": {}})), &texts());
    assert!(page.draft.is_empty());
    assert!(page.popup.is_none(), "新加的存成了关窗");
    let asks = page.take_asks();
    let refresh: Vec<_> = asks
        .iter()
        .filter(|(_, _, p)| p["refresh"] == true)
        .collect();
    assert_eq!(
        refresh.len(),
        1,
        "存完新加的供应商马上取模型列表（2026-10-07 项目主人：不用再按 r）"
    );
    assert_eq!(
        refresh[0].1, "model.list",
        "只取列表，不发对话请求、不花额度（核心说的）"
    );
    assert_eq!(refresh[0].2, json!({"provider": "zen", "refresh": true}));
    assert_eq!(page.status.as_ref().unwrap().1, Tone::Busy);
    let got = json!({"providers": [{"id": "zen", "models": [{"model": "a"}, {"model": "b"}]}]});
    page.answer(refresh[0].0, Ok(got), &texts());
    let (text, tone) = page.status.clone().unwrap();
    assert_eq!(tone, Tone::Good);
    assert!(text.contains('2'), "{text}");
}

#[test]
fn picking_a_default_saves_at_once_and_a_conflict_says_what_is_there_now() {
    let mut page = opened(false);
    press(&mut page, KeyCode::Char('.'));
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Down);
    press(&mut page, KeyCode::Enter);
    let (tag, method, params) = page.take_asks().remove(0);
    assert_eq!(method, "config.set", "选了就存");
    assert_eq!(
        params["changes"],
        json!([{"key": "models.chat", "value": "dev/flash", "expect": {"value": "@daily"}}])
    );
    let refusal = Refusal {
        reason: Some("config_conflict".into()),
        message: "conflict".into(),
        data: json!({"current": {"value": "relay/gpt-oss"}}),
    };
    page.answer(tag, Err(refusal), &texts());
    assert!(page.draft.is_empty(), "没存成的扔掉，界面照读来的");
    assert_eq!(page.view.chat.as_deref(), Some("@daily"));
    let (text, tone) = page.status.clone().unwrap();
    assert_eq!(tone, Tone::Bad);
    assert!(text.contains("relay/gpt-oss"));
}

#[test]
fn a_form_saves_with_s_and_closes_keeps_its_input_when_refused_and_escape_needs_no_question() {
    let mut page = opened(false);
    press(&mut page, KeyCode::Down);
    press(&mut page, KeyCode::Enter);
    // 中转站的窗：停在显示名称，改它，按 s 存了关窗。
    press(&mut page, KeyCode::Enter);
    for c in "Relay".chars() {
        press(&mut page, KeyCode::Char(c));
    }
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Char('s'));
    let (tag, _, _) = page.take_asks().remove(0);
    let refusal = Refusal {
        reason: Some("config_invalid".into()),
        message: "配置有几处不对，没有改。".into(),
        data: json!({"problems": [{"message": "name 太长"}]}),
    };
    page.answer(tag, Err(refusal), &texts());
    let Some(Popup::Form(form)) = &page.popup else {
        panic!("没存成：窗留着");
    };
    assert!(form.error.as_deref().unwrap().contains("name 太长"));
    assert_eq!(form.text(super::forms::Field::Name), "Relay", "填的字留着");
    press(&mut page, KeyCode::Char('s'));
    let (tag, _, _) = page.take_asks().remove(0);
    page.answer(tag, Ok(json!({"keys": {}})), &texts());
    assert!(
        page.popup.is_none(),
        "s：和「保存」按钮一样，存成了关窗（2026-10-07 项目主人）"
    );
    assert_eq!(press(&mut page, KeyCode::Esc), Outcome::Stay);
    assert!(page.on_menu, "改动都存了，返回不用问");
}

#[test]
fn a_new_provider_must_be_saved_before_testing_and_a_failed_test_says_why() {
    let mut page = opened(false);
    press(&mut page, KeyCode::Char('r'));
    let (tag, method, params) = page.take_asks().remove(0);
    assert_eq!(
        (method, params),
        ("provider.test", json!({"provider": "dev"}))
    );
    let failed = json!({"ok": false, "stage": "request", "error": {"class": "auth", "message": "401 bad key"}});
    page.answer(tag, Ok(failed), &texts());
    assert!(page.status.as_ref().unwrap().0.contains("401 bad key"));
    page.draft.new_providers.push("zen".into());
    page.refresh();
    page.nav.vertical(5, &page.view);
    press(&mut page, KeyCode::Char('r'));
    assert!(page.take_asks().is_empty());
    assert_eq!(page.status.as_ref().unwrap().1, Tone::Bad);
}

#[test]
fn deleting_asks_who_uses_it_and_built_in_models_cannot_be_deleted() {
    let mut page = opened(false);
    page.nav.page_step(true);
    page.nav.page_step(true);
    press(&mut page, KeyCode::Char('d'));
    let Some(Popup::Confirm(confirm)) = &page.popup else {
        panic!("先问");
    };
    assert!(
        confirm
            .lines
            .iter()
            .any(|(l, _)| l.contains("默认文本模型")),
        "daily 是默认文本模型"
    );
    press(&mut page, KeyCode::Left);
    press(&mut page, KeyCode::Enter);
    assert!(page.view.pool("daily").is_none());
    let (_, method, params) = page.take_asks().remove(0);
    assert_eq!(method, "config.set", "删了就存");
    assert_eq!(params["changes"].as_array().unwrap().len(), 2);
    page.nav.page_step(false);
    page.nav.page_step(false);
    page.nav.vertical(1, &page.view);
    page.nav.col = 2;
    press(&mut page, KeyCode::Char('d'));
    assert!(page.popup.is_none());
    assert_eq!(
        page.status.as_ref().unwrap().1,
        Tone::Bad,
        "目录列出的删不掉"
    );
}

#[test]
fn connect_opens_straight_on_the_providers_page_and_escape_goes_back_to_the_chat() {
    // 2026-10-07 项目主人：/connect 直接进供应商和模型。
    let mut page = Settings::connect(true);
    answer_loads(&mut page);
    assert!(!page.on_menu, "不停在主菜单");
    assert_eq!(page.nav.page, super::nav::Page::Providers);
    assert_eq!(press(&mut page, KeyCode::Esc), Outcome::Back, "直接回对话");
}

#[test]
fn q_on_a_page_goes_back_like_escape() {
    // 2026-10-07 项目主人：q 返回，和 Esc 一样。
    let mut page = opened(false);
    assert_eq!(press(&mut page, KeyCode::Char('q')), Outcome::Stay);
    assert!(page.on_menu, "回主菜单");
    let mut page = Settings::connect(true);
    answer_loads(&mut page);
    assert_eq!(
        press(&mut page, KeyCode::Char('q')),
        Outcome::Back,
        "/connect 进来的直接回对话"
    );
}

/// 配置清单里只有「语义模型」那一项（4a 的 R-5 补：`model_or`，带 `options`，没有默认值）。
fn embedding_schema() -> serde_json::Value {
    json!({"pages": [{"id": "models", "name": "模型"}], "groups": [{"id": "uses", "name": "用途", "page": "models"}],
        "items": [{"key": "models.embedding", "type": "model_or", "control": "text", "layers": ["system", "personal"],
            "options": [{"name": "内置模型", "value": "local", "note": "bge-m3", "available": false},
                        {"name": "关", "value": "off"}],
            "name": "语义模型", "page": "models", "group": "uses"}]})
}

#[test]
fn the_embedding_row_picks_an_option_or_takes_a_typed_model() {
    // 「配置页」第 21 条（2026-10-09 加）：默认模型页第三行「语义模型」；选「关」当场存成 `off`，「填一个模型…」开编辑窗。
    let mut page = Settings::open(false, true);
    for (tag, method, _) in page.take_asks() {
        let got = match method {
            "model.list" => model_list(),
            "config.get" => config_all(),
            "config.schema" => embedding_schema(),
            _ => json!({"secrets": [{"name": "relay-key", "set": true}]}),
        };
        page.answer(tag, Ok(got), &texts());
    }
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Char('.'));
    press(&mut page, KeyCode::Char('j'));
    press(&mut page, KeyCode::Char('j'));
    press(&mut page, KeyCode::Enter);
    let Some(Popup::Choose(choose)) = &page.popup else {
        panic!("开了选择窗：{:?}", page.popup);
    };
    let labels: Vec<&str> = choose.choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["内置模型", "关", "输入模型…"]);
    // 核心 R-5 再补、三补：内置模型的名字暗色接在后面；没装内置语义模型的包时灰的、选不了。
    assert_eq!(choose.choices[0].note.as_deref(), Some("bge-m3"));
    assert!(choose.choices[0].off.is_some(), "用不了");
    assert!(choose.choices[1].off.is_none());
    assert_eq!(choose.sel, 1, "用不了的停不上，停在「关」");
    press(&mut page, KeyCode::Enter);
    let asks = page.take_asks();
    let sent = asks
        .iter()
        .find(|(_, m, _)| *m == "config.set")
        .expect("当场存")
        .2
        .to_string();
    assert!(
        sent.contains("\"models.embedding\"") && sent.contains("\"off\""),
        "{sent}"
    );
    let (tag, _, _) = asks
        .iter()
        .find(|(_, m, _)| *m == "config.set")
        .cloned()
        .expect("有");
    page.answer(tag, Ok(json!({})), &texts());
    for (tag, method, _) in page.take_asks() {
        let got = match method {
            "model.list" => model_list(),
            "config.schema" => embedding_schema(),
            _ => config_all(),
        };
        page.answer(tag, Ok(got), &texts());
    }
    // 填一个模型：开编辑窗，填了回车存成那个字。
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Char('j'));
    press(&mut page, KeyCode::Char('j'));
    press(&mut page, KeyCode::Enter);
    assert!(
        matches!(page.popup, Some(Popup::Line(_))),
        "{:?}",
        page.popup
    );
    for c in "relay/bge-m3".chars() {
        press(&mut page, KeyCode::Char(c));
    }
    press(&mut page, KeyCode::Enter);
    let asks = page.take_asks();
    let sent = asks
        .iter()
        .find(|(_, m, _)| *m == "config.set")
        .expect("当场存")
        .2
        .to_string();
    assert!(sent.contains("relay/bge-m3"), "{sent}");
}

/// 配置清单里只有「运行日志级别」：只有系统配置这一层（核心 `miyu-log` 的 `layers: [System]`）。
fn log_schema() -> serde_json::Value {
    json!({"pages": [{"id": "advanced", "name": "高级"}], "groups": [{"id": "log", "name": "日志", "page": "advanced"}],
        "items": [{"key": "log.level", "control": "select", "layers": ["system"], "default": "info",
            "options": [{"name": "信息", "value": "info"}, {"name": "调试", "value": "debug"}],
            "name": "运行日志级别", "page": "advanced", "group": "log"}]})
}

#[test]
fn an_item_only_the_system_config_holds_is_written_there() {
    // 「配置页」第 33 条（2026-10-10 项目主人：「怎么改不了运行日志级别」）：写系统配置，`expect` 照系统配置里写的。
    let mut page = Settings::open(false, true);
    for (tag, method, _) in page.take_asks() {
        let got = match method {
            "model.list" => model_list(),
            "config.get" => {
                let mut all = config_all();
                let system = json!({"origin": {"layer": "system", "line": 3}, "value": "info", "used": true});
                all["items"]["log.level"] = json!({"layers": [system]});
                all
            }
            "config.schema" => log_schema(),
            _ => json!({"secrets": []}),
        };
        page.answer(tag, Ok(got), &texts());
    }
    for _ in 0..8 {
        if page.more.selected() == super::pages::Entry::Page("advanced".into()) {
            break;
        }
        press(&mut page, KeyCode::Char('j'));
    }
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Enter);
    assert!(
        matches!(page.popup, Some(Popup::Choose(_))),
        "开了选择窗：{:?} {:?}",
        page.popup,
        page.status
    );
    press(&mut page, KeyCode::Char('j'));
    press(&mut page, KeyCode::Enter);
    let asks = page.take_asks();
    let (_, _, params) = asks
        .iter()
        .find(|(_, m, _)| *m == "config.set")
        .expect("当场存");
    assert_eq!(
        params,
        &json!({"layer": "system", "changes": [{"key": "log.level", "value": "debug", "expect": {"value": "info"}}]})
    );
}
