//! 全屏配置页：真界面连接隔离核心，保存结果直接读取临时配置，不调用真实模型。
mod support;
use miyu_session::testkit::{Play, Script};
use std::time::{Duration, Instant};
use support::{Home, Tui};

const TWO: &str = r#"
[models]
chat = "relay/alpha"

[providers.relay]
driver = "openai-chat"
base_url = "https://relay.invalid/v1"

[providers.relay.models.alpha]
window = 128000
inputs = ["text"]

[providers.relay.models.beta]
window = 64000
inputs = ["text", "image"]
"#;

fn start(home: &Home) -> Tui {
    let mut tui = home.tui_args("zh_CN.UTF-8", &["--page", "config"]);
    tui.wait_for("alpha");
    let end = Instant::now() + support::WAIT;
    while tui.shows("正在读取") {
        assert!(
            Instant::now() < end,
            "配置还没读完：{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(50));
    }
    tui
}

fn wait_settings(home: &Home, tui: &mut Tui, want: &str) {
    let end = Instant::now() + support::WAIT;
    while !home.settings().contains(want) {
        assert!(
            Instant::now() < end,
            "没有保存「{want}」，屏幕：\n{}\n配置：\n{}",
            tui.lines().join("\n"),
            home.settings()
        );
        tui.pump(Duration::from_millis(50));
    }
}

#[test]
fn independent_config_opens_four_tabs_without_creating_a_session() {
    let home = Home::new(Script::new(vec![]));
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("默认文本模型");
    tui.wait_for("默认视觉模型");
    tui.wait_for("自定义模型池");
    tui.key(b"\x1b");
    let sessions = home.root().join("home/alice/sessions");
    assert!(!sessions.exists() || std::fs::read_dir(sessions).unwrap().next().is_none());
}

#[test]
fn default_text_and_vision_choose_single_models_without_pools_or_sessions() {
    let home = Home::with_settings(Script::new(vec![]), TWO);
    let mut tui = start(&home);
    tui.key(b"]");
    tui.wait_for("relay/beta");
    tui.key(b"j");
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "chat = \"relay/beta\"");
    tui.wait_for("已保存");
    tui.key(b"]");
    tui.wait_for("relay/beta");
    assert!(!tui.shows("relay/alpha"), "视觉默认页只列支持图像的模型");
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "vision = \"relay/beta\"");
    assert!(!home.settings().contains("@"));
    let sessions = home.root().join("home/alice/sessions");
    assert!(!sessions.exists() || std::fs::read_dir(sessions).unwrap().next().is_none());
}

#[test]
fn a_pool_saves_members_in_selection_order() {
    let home = Home::with_settings(Script::new(vec![]), TWO);
    let mut tui = start(&home);
    for _ in 0..3 {
        tui.key(b"]");
    }
    tui.key(b"a");
    tui.wait_for("新建模型池");
    tui.key(b"\r");
    tui.type_text("daily");
    tui.key(b"\r");
    tui.key(b"j");
    tui.key(b"\r");
    tui.wait_for("pin");
    tui.key(b"\r");
    tui.key(b"j");
    tui.key(b"j");
    tui.key(b"\t");
    tui.key(b"k");
    tui.key(b" ");
    tui.key(b"s");
    wait_settings(
        &home,
        &mut tui,
        "models = [\"relay/beta\", \"relay/alpha\"]",
    );
    assert!(home.settings().contains("[pools.daily]"));
    assert!(home.settings().contains("strategy = \"pin\""));
}

#[test]
fn model_window_edit_is_saved_without_changing_other_models() {
    let home = Home::with_settings(Script::new(vec![]), TWO);
    let mut tui = start(&home);
    tui.key(b"l");
    tui.key(b"\r");
    tui.wait_for("上下文窗口");
    tui.key(b"j");
    tui.key(b"\r");
    tui.key(b"\x15");
    tui.type_text("96000");
    tui.key(b"\r");
    tui.key(b"s");
    wait_settings(&home, &mut tui, "window = 96000");
    assert!(home.settings().contains("window = 64000"));
}

#[test]
fn plain_api_key_is_masked_and_saved_only_in_the_secret_file() {
    const KEY: &str = "tui-test-only-api-key-731";
    let home = Home::with_settings(Script::new(vec![]), TWO);
    let mut tui = start(&home);
    tui.key(b"\r");
    tui.wait_for("API Key");
    for _ in 0..3 {
        tui.key(b"j");
    }
    tui.key(b"\r");
    tui.record();
    tui.type_text(KEY);
    assert!(!tui.shows(KEY));
    tui.key(b"\r");
    assert!(!tui.shows(KEY));
    tui.key(b"s");
    tui.wait_for("已保存");
    let recorded = String::from_utf8_lossy(&tui.recorded()).into_owned();
    assert!(!recorded.contains(KEY), "每帧输出都不得泄露输入的明文密钥");
    let settings = home.settings();
    assert!(!settings.contains(KEY));
    let secrets = std::fs::read_to_string(home.root().join("system/secrets.toml")).unwrap();
    assert!(secrets.contains(KEY));
    let name = secrets
        .lines()
        .find(|l| l.contains(KEY))
        .unwrap()
        .split('=')
        .next()
        .unwrap()
        .trim();
    assert!(name.starts_with("tui-"));
    assert!(
        settings.contains(name) && settings.contains("secret"),
        "配置只存已保存密钥的引用"
    );
    for file in ["system/journal.jsonl", "home/alice/journal.jsonl"] {
        let log = std::fs::read_to_string(home.root().join(file)).unwrap_or_default();
        assert!(!log.contains(KEY), "配置日志不得泄露密钥");
    }
}

#[test]
fn config_from_an_existing_conversation_returns_to_the_original_reply() {
    let home = Home::with_settings(Script::new([Play::Says("配置前的回答。")]), TWO);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("配置前的回答。");
    tui.say("/config");
    tui.wait_for("默认文本模型");
    tui.wait_for("自定义模型池");
    assert!(!tui.shows("配置前的回答。"), "全屏配置覆盖对话内容");
    tui.key(b"\x1b");
    tui.wait_for("配置前的回答。");
    tui.type_text("保留在原输入框");
    assert!(tui.shows("保留在原输入框"));
}
