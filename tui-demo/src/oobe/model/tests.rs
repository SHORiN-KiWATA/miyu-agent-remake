//! 接模型那一步（「第一次打开的引导」第 15–20 条）：一步步走，查发给核心的请求。

use std::time::Instant;

use ratatui::crossterm::event::KeyCode;
use serde_json::{Value, json};

use super::Phase;
use crate::oobe::tests::{asks, find, fresh, key, texts, to_model, typed};
use crate::oobe::{Oobe, Step, Texts};

fn catalog() -> Value {
    json!({"providers": [
        {"id": "deepseek", "name": "DeepSeek", "supported": true},
        {"id": "openai", "name": "OpenAI", "supported": true}]})
}

/// 走到接模型、三份回应和配置都回来；`chat` 是现在的聊天模型。
fn at_model(t0: Instant, texts: &Texts, chat: Option<&str>) -> Oobe {
    let mut oobe = fresh(t0);
    to_model(&mut oobe, t0, texts);
    let sent = asks(&mut oobe);
    let list = match chat {
        Some(chat) => json!({"uses": {"chat": chat}, "providers": [{"id": "zen", "models": [
            {"model": "big", "ref": "zen/big", "facts": {"name": {"value": "Big Model"}}}]}]}),
        None => json!({"uses": {"chat": null}, "providers": []}),
    };
    for (tag, method, _) in sent {
        let reply = match method {
            "model.list" => list.clone(),
            "provider.detect" => {
                json!({"keys": [{"env": "DEEPSEEK_API_KEY", "provider": "deepseek", "supported": true}]})
            }
            "provider.catalog" => catalog(),
            "config.get" => json!({"items": {}}),
            _ => json!({}),
        };
        oobe.answer(tag, Ok(reply), t0, texts);
    }
    oobe
}

#[test]
fn a_found_key_is_tested_right_away_and_the_chosen_model_is_saved_by_reference() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_model(t0, &texts, None);
    assert!(matches!(oobe.model.phase, Phase::List));
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(matches!(oobe.model.phase, Phase::Testing(_)));
    assert!(oobe.react.waiting, "试的时候抬头");
    let (tag, params) = find(&asks(&mut oobe), "provider.test");
    assert_eq!(
        params,
        json!({"candidate": {"catalog": "deepseek", "key": {"env": "DEEPSEEK_API_KEY"}}})
    );
    let ok = json!({"ok": true, "models": ["deepseek-v4-pro", "deepseek-flash"], "model": "deepseek-flash"});
    oobe.answer(tag, Ok(ok), t0, &texts);
    let Phase::Models(models) = &oobe.model.phase else {
        panic!("{:?}", oobe.model.phase)
    };
    assert_eq!(
        models.names,
        ["deepseek-flash", "deepseek-v4-pro"],
        "试的排第一"
    );
    assert!(!oobe.react.waiting && !oobe.react.droop);
    // `/` 搜、打字筛：只剩一个，回车搜完，再回车存。
    oobe.key(key(KeyCode::Char('/')), t0, &texts);
    typed(&mut oobe, "pro", t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(asks(&mut oobe).is_empty(), "搜的时候回车只是搜完");
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let sent = asks(&mut oobe);
    assert!(
        sent.iter().all(|(_, m, _)| *m != "secret.set"),
        "找到的 key 不存"
    );
    let (tag, params) = find(&sent, "config.set");
    let changes = params["changes"].as_array().unwrap();
    assert!(
        changes.contains(
            &json!({"key": "providers.deepseek.key", "value": {"env": "DEEPSEEK_API_KEY"}})
        )
    );
    assert!(changes.contains(&json!({"key": "models.chat", "value": "deepseek/deepseek-v4-pro"})));
    oobe.answer(tag, Ok(json!({})), t0, &texts);
    assert_eq!(oobe.step, Step::Persona);
    assert_eq!(oobe.summary().model, "deepseek-v4-pro");
}

#[test]
fn a_pasted_key_is_needed_tested_and_stored_before_the_config() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_model(t0, &texts, None);
    oobe.key(key(KeyCode::Down), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(matches!(oobe.model.phase, Phase::Form(_)), "请填写密钥");
    // 光标停在密钥上打字不进去（第 7 条）：回车才编辑。
    typed(&mut oobe, "x", t0, &texts);
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert_eq!(
        oobe.model.error.as_deref(),
        Some(texts.model.key_needed.as_str()),
        "停在「测试连接」上回车：没填不试"
    );
    oobe.paste("sk-test\n");
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(asks(&mut oobe).is_empty(), "回车只是写好");
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (tag, params) = find(&asks(&mut oobe), "provider.test");
    assert_eq!(params["candidate"]["key"], json!({"value": "sk-test"}));
    // 没试通：红字照哪一步加原话，耷拉耳朵，光标回到密钥。
    let failed =
        json!({"ok": false, "stage": "request", "error": {"class": "auth", "message": "bad key"}});
    oobe.answer(tag, Ok(failed), t0, &texts);
    assert!(matches!(oobe.model.phase, Phase::Form(_)));
    assert!(oobe.react.droop);
    let error = oobe.model.error.clone().unwrap();
    assert!(
        error.contains("发送请求") && error.contains("bad key"),
        "{error}"
    );
    oobe.key(key(KeyCode::Down), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (tag, _) = find(&asks(&mut oobe), "provider.test");
    oobe.answer(
        tag,
        Ok(json!({"ok": true, "models": ["gpt-4o"], "model": "gpt-4o"})),
        t0,
        &texts,
    );
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let sent = asks(&mut oobe);
    let (tag, params) = find(&sent, "secret.set");
    assert_eq!(params, json!({"name": "openai", "value": "sk-test"}));
    assert!(
        sent.iter().all(|(_, m, _)| *m != "config.set"),
        "key 存好了再写配置"
    );
    oobe.answer(tag, Ok(json!({"replaced": false})), t0, &texts);
    let (_, params) = find(&asks(&mut oobe), "config.set");
    let changes = params["changes"].as_array().unwrap();
    assert!(
        changes.contains(&json!({"key": "providers.openai.key", "value": {"secret": "openai"}}))
    );
    assert!(
        changes.contains(&json!({"key": "pools.lite.models", "value": []})),
        "一个池都没有的写三个预设的池"
    );
}

#[test]
fn a_custom_endpoint_needs_a_good_address_and_asks_for_a_model_name_when_it_cannot_list() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_model(t0, &texts, None);
    for _ in 0..5 {
        oobe.key(key(KeyCode::Down), t0, &texts);
    }
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let Phase::Form(form) = &oobe.model.phase else {
        panic!()
    };
    assert!(form.provider.is_none(), "自定义");
    oobe.key(key(KeyCode::Enter), t0, &texts);
    typed(&mut oobe, "api.example.invalid", t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert_eq!(
        oobe.model.error.as_deref(),
        Some(texts.model.url_bad.as_str())
    );
    oobe.key(key(KeyCode::Enter), t0, &texts);
    oobe.key(
        ratatui::crossterm::event::KeyEvent::new(
            KeyCode::Char('u'),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        ),
        t0,
        &texts,
    );
    typed(&mut oobe, "https://api.example.invalid/v1/", t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert_eq!(oobe.model.error, None, "地址对了红字去掉");
    // 接口那一格 `l` 换下一个；再往下两格到「测试连接」。
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Char('l')), t0, &texts);
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (tag, params) = find(&asks(&mut oobe), "provider.test");
    assert_eq!(
        params,
        json!({"candidate": {"driver": "anthropic", "base_url": "https://api.example.invalid/v1"}}),
        "空着的 key 不带"
    );
    oobe.answer(
        tag,
        Ok(json!({"ok": false, "stage": "list", "error": {"message": "404"}})),
        t0,
        &texts,
    );
    assert_eq!(
        oobe.model.warn.as_deref(),
        Some(texts.model.no_list.as_str())
    );
    assert!(!oobe.react.droop, "列不出模型不算没连上");
    oobe.key(key(KeyCode::Enter), t0, &texts);
    typed(&mut oobe, "glm-5", t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (_, params) = find(&asks(&mut oobe), "provider.test");
    assert_eq!(params["model"], "glm-5");
}

#[test]
fn an_existing_chat_model_can_be_kept_with_one_enter() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_model(t0, &texts, Some("zen/big"));
    assert!(matches!(oobe.model.phase, Phase::Ready(true)));
    assert_eq!(oobe.model.chat.as_deref(), Some("Big Model"), "写显示名");
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert_eq!(oobe.step, Step::Persona);
    assert_eq!(oobe.summary().model, "Big Model");
}

#[test]
fn more_providers_opens_a_searchable_window_and_enter_fills_in_that_one() {
    // 第 16a 条（2026-10-10 项目主人：多一个「更多供应商」，浮窗里上面一格搜索，回车进填写那一屏）。
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_model(t0, &texts, None);
    // 两家常用的、「其他」段名（停不上）、更多供应商。
    for _ in 0..2 {
        oobe.key(key(KeyCode::Down), t0, &texts);
    }
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(oobe.model.more.is_some(), "开了浮窗");
    let (tag, params) = find(&asks(&mut oobe), "provider.catalog");
    assert!(params.get("featured").is_none(), "拿全目录：{params}");
    assert!(params["limit"].as_u64().unwrap() >= 1000, "{params}");
    let all = json!({"providers": [
        {"id": "zai", "name": "Z.AI", "supported": true},
        {"id": "moonshotai-cn", "name": "Moonshot AI (China)", "supported": true},
        {"id": "weird", "name": "Moon Weird", "supported": false},
        {"id": "moonshotai", "name": "Moonshot AI", "supported": true}]});
    oobe.answer(tag, Ok(all), t0, &texts);
    // 开窗就在搜索那一格里：打字就筛，j 也是字。
    typed(&mut oobe, "moon", t0, &texts);
    let more = oobe.model.more.as_ref().unwrap();
    let names: Vec<&str> = more.matches().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Moon Weird", "Moonshot AI", "Moonshot AI (China)"]);
    assert_eq!(more.cursor, 1, "接不上的停不上");
    oobe.key(key(KeyCode::Down), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(oobe.model.more.is_none(), "关窗");
    let Phase::Form(form) = &oobe.model.phase else {
        panic!("{:?}", oobe.model.phase)
    };
    assert_eq!(form.provider.as_ref().unwrap().catalog, "moonshotai-cn");
    // Esc 只关窗。
    oobe.key(key(KeyCode::Esc), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    asks(&mut oobe);
    oobe.key(key(KeyCode::Esc), t0, &texts);
    assert!(oobe.model.more.is_none() && matches!(oobe.model.phase, Phase::List));
    assert_eq!(oobe.step, Step::Model);
}
