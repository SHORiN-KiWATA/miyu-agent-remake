//! 伪终端里的端到端测试：配置页（蓝图 `tui.md`「配置页」）。真界面连一份真核心，改了存下，看个人设置文件。

mod support;

use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

const DOWN: &[u8] = b"\x1b[B";
const TAB: &[u8] = b"\t";

/// 一家中转站：模型名带组织前缀；一个池。
const RELAY: &str = "[models]\nchat = \"relay/cline/v4\"\n\n[pools.duo]\nmodels = [\"relay/cline/v4\"]\nstrategy = \"rotate\"\n\n[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"http://relay.invalid/v1\"\n\n[providers.relay.models.\"cline/v4\"]\nwindow = 128000\n\n[providers.relay.models.\"zhipu/glm\"]\nwindow = 64000\ninputs = [\"text\", \"image\"]\n";

/// 等个人设置文件里出现 `want`。
fn wait_settings(home: &Home, tui: &mut Tui, want: &str) {
    let end = Instant::now() + support::WAIT;
    while !home.settings().contains(want) {
        assert!(
            Instant::now() < end,
            "设置里没有「{want}」：\n{}\n屏幕：\n{}",
            home.settings(),
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
}

#[test]
fn the_config_page_starts_on_its_own_edits_a_model_and_saves_to_personal_settings() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["--page", "config"]);
    tui.wait_for("供应商、默认模型、模型池");
    tui.key(b"\r");
    tui.wait_for("组织");
    tui.wait_for("cline");
    // 供应商 → 组织 → 模型；第一个模型是 cline/v4，开它的窗，改窗口。
    tui.key(b"l");
    tui.key(b"l");
    tui.key(b"\r");
    tui.wait_for("上下文窗口");
    // 模型名只读，开窗停在「支持输入」；往下一行是窗口，原来的值整段选中，一敲就换掉。
    tui.key(DOWN);
    tui.key(b"\r");
    tui.type_text("200k");
    tui.key(b"\r");
    // 在窗里按 s：当场存，存成了关窗（2026-10-07 项目主人）。
    tui.key(b"s");
    wait_settings(&home, &mut tui, "window = 200000");
    tui.wait_for("已保存");
    // 默认模型页：换成池，存。
    tui.key(b".");
    tui.wait_for("默认文本模型");
    tui.key(b"\r");
    tui.wait_for("模型池");
    // 开窗选中现在用的那个；池在最上面。选了就存。
    tui.key(b"k");
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "chat = \"@duo\"");
    tui.key(b"\x1b");
    tui.wait_for("供应商、默认模型、模型池");
}

#[test]
fn deleting_a_pool_asks_who_uses_it_and_saves_at_once() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("供应商、默认模型、模型池");
    tui.key(b"\r");
    tui.wait_for("cline");
    tui.key(b",");
    tui.key(b".");
    tui.key(b".");
    tui.wait_for("duo");
    tui.key(b"d");
    tui.wait_for("删除模型池 duo？");
    // 停在「取消」：按 Enter 什么都不做。
    tui.key(b"\r");
    tui.pump(Duration::from_millis(300));
    assert!(home.settings().contains("[pools.duo]"), "取消的不删");
    tui.key(b"d");
    tui.wait_for("删除模型池 duo？");
    tui.key(b"h");
    tui.key(b"\r");
    let end = Instant::now() + support::WAIT;
    while home.settings().contains("pools.duo") {
        assert!(Instant::now() < end, "没删掉：\n{}", home.settings());
        tui.pump(Duration::from_millis(100));
    }
}

#[test]
fn slash_config_in_a_conversation_goes_back_to_the_conversation() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.say("/config");
    tui.wait_for("供应商、默认模型、模型池");
    assert!(
        !tui.lines().iter().any(|l| l.contains("好。")),
        "配置页开着不画对话"
    );
    tui.key(b"\x1b");
    tui.wait_for("好。");
    assert!(
        !tui.lines()
            .iter()
            .any(|l| l.contains("供应商、默认模型、模型池")),
        "回到对话，原样画回来"
    );
}

#[test]
fn a_new_provider_keeps_its_key_in_the_secret_store_and_an_added_model_can_be_deleted() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("供应商、默认模型、模型池");
    tui.key(b"\r");
    tui.wait_for("cline");
    // 添加供应商：ID、地址、key（ID、显示名称、地址、接口、认证、key）。
    tui.key(b"a");
    tui.wait_for("添加供应商");
    tui.key(b"\r");
    tui.type_text("zen");
    tui.key(b"\r");
    tui.key(DOWN);
    tui.key(DOWN);
    tui.key(b"\r");
    tui.type_text("http://zen.invalid/v1");
    tui.key(b"\r");
    // 地址认不出驱动：接口选 openai-chat，不然核心当这一家用不了、不列模型。
    tui.key(DOWN);
    tui.key(b"l");
    tui.key(DOWN);
    tui.key(DOWN);
    tui.key(b"\r");
    tui.send(b"\x1b[200~sk-test-zen\n\x1b[201~");
    tui.key(b"\r");
    assert!(
        !tui.lines().iter().any(|l| l.contains("sk-test-zen")),
        "key 打码"
    );
    tui.key(b"s");
    wait_settings(&home, &mut tui, "zen-key");
    let settings = home.settings();
    assert!(settings.contains("[providers.zen]"), "{settings}");
    assert!(!settings.contains("sk-test-zen"), "明文不进个人设置");
    let secrets =
        std::fs::read_to_string(home.root().join("system/secrets.toml")).unwrap_or_default();
    assert!(secrets.contains("zen-key"), "key 进了密钥库");
    // 给它手加一个模型，存下；再删掉，存下。
    tui.key(b"n");
    tui.wait_for("添加模型");
    tui.key(b"\r");
    tui.type_text("big");
    tui.key(b"\r");
    tui.key(TAB);
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "[providers.zen.models.big]");
    tui.key(b"l");
    tui.wait_for("big");
    tui.key(b"d");
    tui.wait_for("删除模型 big？");
    tui.key(b"h");
    tui.key(b"\r");
    let end = Instant::now() + support::WAIT;
    while home.settings().contains("models.big") {
        assert!(Instant::now() < end, "没删掉：\n{}", home.settings());
        tui.pump(Duration::from_millis(100));
    }
}
