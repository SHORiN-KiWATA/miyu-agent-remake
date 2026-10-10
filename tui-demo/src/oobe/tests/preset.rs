//! 选预设那一步（「第一次打开的引导」第 24–26 条）。

use std::time::Instant;

use ratatui::crossterm::event::KeyCode;
use serde_json::json;

use super::{asks, ctrl, find, fresh, key, texts, typed};
use crate::oobe::{Move, Oobe, Step, Texts};

/// 走到选预设：出厂两个，默认 `full`。
fn at_preset(t0: Instant, texts: &Texts) -> Oobe {
    let mut oobe = fresh(t0);
    oobe.step = Step::Persona;
    oobe.moved(Move::Next, t0);
    assert_eq!(oobe.step, Step::Preset);
    let sent = asks(&mut oobe);
    let (list, _) = find(&sent, "preset.list");
    let (default, _) = find(&sent, "config.get");
    oobe.answer(
        list,
        Ok(json!({"presets": [{"preset": "dev", "name": "基础功能"}, {"preset": "full", "name": "全部功能"},
                              {"preset": "broken", "problem": "写错了"}]})),
        t0,
        texts,
    );
    oobe.answer(
        default,
        Ok(json!({"items": {"preset.default": {"value": "full"}}})),
        t0,
        texts,
    );
    for (tag, _, params) in asks(&mut oobe) {
        let on = params["preset"] == "full";
        let tool = |name: &str, label: &str| json!({"name": name, "label": label, "on": on});
        let reply = json!({"features": [
            {"id": "files", "name": "文件读写", "on": true, "installed": true,
             "tools": [{"name": "glob", "label": "找文件", "on": true}, {"name": "grep", "label": "搜内容", "on": true}]},
            {"id": "memory", "name": "人格记忆", "on": on, "installed": true,
             "tools": [tool("remember", "记一条"), tool("forget", "忘掉")]},
            {"id": "goal", "name": "长期目标", "on": true, "installed": false, "tools": []}]});
        oobe.answer(tag, Ok(reply), t0, texts);
    }
    oobe
}

#[test]
fn the_default_is_chosen_first_and_enter_makes_it_the_default() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_preset(t0, &texts);
    assert_eq!(oobe.preset.cards.len(), 2, "写错的不列");
    assert_eq!(oobe.preset.cursor, 1, "选着现在的默认预设");
    let card = &oobe.preset.cards[0];
    assert_eq!(card.features.as_ref().map(Vec::len), Some(2), "没装的不算");
    oobe.key(key(KeyCode::Up), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (tag, params) = find(&asks(&mut oobe), "config.set");
    assert_eq!(
        params["changes"],
        json!([{"key": "preset.default", "value": "dev"}])
    );
    oobe.answer(tag, Ok(json!({})), t0, &texts);
    assert_eq!(oobe.step, Step::Done);
    assert_eq!(oobe.summary().preset, "基础功能");
    assert_eq!(
        oobe.picks().1.as_deref(),
        Some("dev"),
        "紧接着的新会话照它开"
    );
}

#[test]
fn a_custom_preset_is_named_toggled_created_and_made_the_default() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_preset(t0, &texts);
    oobe.key(key(KeyCode::Down), t0, &texts);
    assert!(
        oobe.preset.custom() && !oobe.preset.custom_open,
        "停到自定义不展开"
    );
    // 回车开浮窗（第 26 条，2026-10-10 项目主人：「自定义哪里也改了吧」）。
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(oobe.preset.custom_open);
    let all_on = |oobe: &crate::oobe::Oobe| {
        oobe.preset
            .features
            .iter()
            .all(|f| f.on && f.tools.iter().all(|t| t.on))
    };
    assert!(all_on(&oobe), "先照全开：功能、工具都开着");
    assert_eq!(oobe.preset.rows().len(), 6, "两个功能各带两件工具，平铺");
    // 最后一行「创建」：名字、六行功能和工具以后。
    for _ in 0..7 {
        oobe.key(key(KeyCode::Char('j')), t0, &texts);
    }
    assert_eq!(oobe.preset.inner, oobe.preset.create_row());
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert!(asks(&mut oobe).is_empty(), "名字空着不建");
    assert!(oobe.preset.error.is_some());
    assert_eq!(oobe.preset.inner, 0, "光标回到名称");
    // 名称：回车开始编辑，再回车写好（第 7 条）。
    oobe.key(key(KeyCode::Enter), t0, &texts);
    typed(&mut oobe, "写代码", t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    assert_eq!(oobe.preset.name.text(), "写代码");
    // 功能那几行不打字：vim 的 j 往下挪（第 7 条）。
    oobe.key(key(KeyCode::Down), t0, &texts);
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    assert!(
        !oobe.preset.features[0].tools[0].on,
        "空格单独关掉「找文件」"
    );
    oobe.key(ctrl('a'), t0, &texts);
    assert!(all_on(&oobe), "有没开的就全开");
    oobe.key(ctrl('a'), t0, &texts);
    assert!(
        oobe.preset.features.iter().all(|f| !f.on),
        "都开着就全关功能"
    );
    // 功能关着时在它的工具上按空格：打开这个功能。
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    assert!(oobe.preset.features[0].on && !oobe.preset.features[1].on);
    for _ in 0..5 {
        oobe.key(key(KeyCode::Char('j')), t0, &texts);
    }
    oobe.key(key(KeyCode::Enter), t0, &texts);
    let (tag, params) = find(&asks(&mut oobe), "preset.set");
    assert_eq!(
        params,
        json!({"changes": [{"key": "preset.name", "value": "写代码"}, {"key": "features.memory", "value": false}]})
    );
    oobe.answer(tag, Ok(json!({"preset": "preset-1"})), t0, &texts);
    let (tag, params) = find(&asks(&mut oobe), "config.set");
    assert_eq!(
        params["changes"],
        json!([{"key": "preset.default", "value": "preset-1"}])
    );
    oobe.answer(tag, Ok(json!({})), t0, &texts);
    assert_eq!(oobe.step, Step::Done);
    assert_eq!(oobe.summary().preset, "写代码");
    assert_eq!(oobe.picks().1.as_deref(), Some("preset-1"));
}

#[test]
fn space_opens_a_read_only_window_of_what_a_preset_turns_on() {
    // 第 24 条（2026-10-10 项目主人：空格看详情，「甚至不如打开一个悬浮窗口」）。
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_preset(t0, &texts);
    oobe.key(key(KeyCode::Up), t0, &texts);
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    assert!(oobe.preset.viewing, "空格开浮窗");
    // 浮窗里 j、k 只滚，不挪下面的光标，不改什么。
    let at = oobe.preset.cursor;
    oobe.key(key(KeyCode::Char('j')), t0, &texts);
    assert_eq!(oobe.preset.cursor, at);
    assert!(asks(&mut oobe).is_empty(), "只看不改");
    oobe.key(key(KeyCode::Esc), t0, &texts);
    assert!(
        !oobe.preset.viewing && oobe.step == Step::Preset,
        "Esc 只关窗"
    );
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    assert!(!oobe.preset.viewing, "再按空格关");
}

#[test]
fn escape_while_naming_a_custom_preset_only_stops_editing() {
    let texts = texts();
    let t0 = Instant::now();
    let mut oobe = at_preset(t0, &texts);
    oobe.key(key(KeyCode::Down), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    oobe.key(key(KeyCode::Enter), t0, &texts);
    typed(&mut oobe, "写", t0, &texts);
    oobe.key(key(KeyCode::Esc), t0, &texts);
    assert_eq!(oobe.step, Step::Preset, "编辑时 Esc 不回上一步");
    assert!(
        !oobe.preset.name.editing() && oobe.preset.custom_open,
        "只退出编辑"
    );
    assert_eq!(oobe.preset.name.text(), "", "回到编辑前的字");
    oobe.key(key(KeyCode::Esc), t0, &texts);
    assert!(
        !oobe.preset.custom_open && oobe.step == Step::Preset,
        "再按 Esc 关窗"
    );
    // 空格和回车一样开（2026-10-10 项目主人）。
    oobe.key(key(KeyCode::Char(' ')), t0, &texts);
    assert!(oobe.preset.custom_open, "空格也开");
}
