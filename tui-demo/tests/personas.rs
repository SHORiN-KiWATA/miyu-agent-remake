//! 伪终端里的端到端测试：新会话先选人格（蓝图 `tui.md`「新会话：人格、工作区」，2026-10-07 项目主人定：每次开新会话
//! 都先弹框选）。真界面连一份照剧本回话的核心，测试家目录里放几个人格。

mod support;

use miyu_session::testkit::{Play, Script};
use support::Home;

/// 不照测具默认的无人格：弹人格框。
const ASK: (&str, &str) = ("MIYU_TUI_PERSONA", "");
/// 不照测具默认的全部功能：弹预设框。
const ASK_PRESET: (&str, &str) = ("MIYU_TUI_PRESET", "");
const UP: &[u8] = b"\x1b[A";

/// 输入框里那一行。
fn input(tui: &support::Tui) -> String {
    tui.lines()
        .into_iter()
        .find(|l| l.contains('❯'))
        .unwrap_or_default()
}

#[test]
fn every_new_session_asks_for_a_persona_first_and_the_chosen_one_goes_into_the_session() {
    let script = Script::new([Play::Says("好。"), Play::Says("再见。")]);
    let home = Home::new(script.clone());
    home.persona(
        "pirate",
        "[persona]\nname = { zh = \"海盗\", en = \"Pirate\" }\n",
        Some("你是海盗，暗号 X1Y2。"),
    );
    home.persona("broken", "[persona]\nbad = 1\n", None);
    let mut tui = home.tui_with("zh_CN.UTF-8", &[ASK, ASK_PRESET]);
    tui.wait_for("请选择人格");
    tui.wait_for("海盗");
    assert!(tui.shows("broken"), "写错的也列着（选不了）");
    let broken = tui
        .lines()
        .into_iter()
        .find(|l| l.contains("broken"))
        .unwrap_or_default();
    // 原因照界面语言说（核心 P-3 再补，2026-10-08 起列表的 `problem` 照连接的语言给）。
    assert!(
        broken.contains("不认识的键"),
        "写错的后面写核心给的原因：{broken}"
    );
    // 框开着时打的字不进输入框：选了才能打字。
    tui.type_text("abc");
    assert!(!input(&tui).contains("abc"), "{}", input(&tui));
    // 第一行「无人格」（2026-10-08 项目主人：人格允许为空），默认人格没设，光标停在它上面；往下挪到海盗（写错的跳过）。
    assert!(tui.shows("无人格"), "{}", tui.lines().join("\n"));
    assert!(picked(&tui).contains("无人格"), "{}", picked(&tui));
    // 框开着不能打字：直接 j、k 上下（2026-10-11 项目主人）。
    for _ in 0..8 {
        if picked(&tui).contains("海盗") {
            break;
        }
        tui.key(b"j");
    }
    assert!(picked(&tui).contains("海盗"), "{}", tui.lines().join("\n"));
    tui.key(b"\r");
    // 人格选好了接着选预设（2026-10-08 项目主人定先人格、再预设）：照编号排 dev、full（出厂叫「基础功能」「全部功能」，核心 P-4 下），光标停在默认的全部功能上。
    tui.wait_for("请选择预设");
    tui.wait_for("全部功能");
    tui.key(b"k");
    tui.key(b"\r");
    // 选好了框关上、能打字（首页那一行不写人格、预设，2026-10-08 项目主人）；选的照发出去的请求认。
    closed(&mut tui);
    tui.say("在吗");
    tui.wait_for("好。");
    let sent = format!("{:?}", script.requests().last());
    assert!(sent.contains("X1Y2"), "开会话带上了选的人格：{sent}");
    // 开了会话不能换。
    tui.say("/persona");
    tui.wait_for("无法更换人格");
    tui.say("/preset");
    tui.wait_for("无法更换预设");
    // `/new` 以后又先问；`Esc` 照默认的。
    tui.say("/new");
    tui.wait_for("请选择人格");
    tui.key(b"\x1b");
    tui.wait_for("请选择预设");
    tui.key(b"\x1b");
    closed(&mut tui);
}

/// 框里选中的那一行（行首 `❯`，在输入框上面，先找到的就是它）。
fn picked(tui: &support::Tui) -> String {
    tui.lines()
        .into_iter()
        .find(|l| l.contains('❯'))
        .unwrap_or_default()
}

/// 人格框、预设框都关上了：框的标题没了，输入框能打字。
fn closed(tui: &mut support::Tui) {
    let end = std::time::Instant::now() + support::WAIT;
    while tui.shows("请选择") {
        assert!(
            std::time::Instant::now() < end,
            "框没关：\n{}",
            tui.lines().join("\n")
        );
        tui.pump(std::time::Duration::from_millis(50));
    }
}

#[test]
fn a_session_needs_a_preset_and_a_broken_default_keeps_the_box_open() {
    // 核心 Y12：开会话一定要有预设；默认预设指着没有的，`Esc` 不关，选了才能打字。
    let home = Home::with_settings(
        Script::new([Play::Says("好。")]),
        "[preset]\ndefault = \"nope\"\n",
    );
    let mut tui = home.tui_with("zh_CN.UTF-8", &[ASK_PRESET]);
    tui.wait_for("请选择预设");
    tui.key(b"\x1b");
    tui.wait_for("请先选择预设");
    assert!(tui.shows("请选择预设"), "框还开着");
    tui.key(b"\r");
    closed(&mut tui);
    tui.say("在吗");
    tui.wait_for("好。");
}

#[test]
fn an_open_session_shows_its_persona_in_the_sidebar() {
    // 核心 P-1 下起订阅的回应带 `persona`：侧边栏会话名下面写一行（蓝图「新会话：人格、工作区」第 6 条）。
    let home = Home::new(Script::new([Play::Says("好。")]));
    home.persona("pirate", "[persona]\nname = \"海盗\"\n", Some("你是海盗。"));
    let mut tui = home.tui_wide_with("zh_CN.UTF-8", 140, &[("MIYU_TUI_PERSONA", "pirate")]);
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.wait_for("人格 海盗");
    tui.wait_for("预设 全部功能");
}

#[test]
fn a_session_without_a_persona_shows_no_persona_line() {
    // 无人格的会话订阅回应里没有 `persona`（核心 P-4 上）：侧边栏不写人格那一行。
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.wait_for("预设 全部功能");
    assert!(!tui.shows("人格 "), "{}", tui.lines().join("\n"));
}

#[test]
fn choosing_no_persona_opens_a_session_without_one_even_when_a_default_is_set() {
    // 2026-10-08 项目主人：「软件默认不自带任何人格，人格允许为空」；框里第一行「无人格」，选了开会话时发
    // `"persona": null`，不照默认人格（核心 P-4 上）。
    let script = Script::new([Play::Says("好。")]);
    let home = Home::with_settings(script.clone(), "[persona]\ndefault = \"pirate\"\n");
    home.persona(
        "pirate",
        "[persona]\nname = \"海盗\"\n",
        Some("你是海盗，暗号 X1Y2。"),
    );
    let mut tui = home.tui_with("zh_CN.UTF-8", &[ASK]);
    tui.wait_for("请选择人格");
    tui.wait_for("不使用人格，记忆不生效");
    // 光标停在默认的海盗上：挪到最上面的「无人格」。
    for _ in 0..5 {
        tui.key(UP);
    }
    tui.key(b"\r");
    closed(&mut tui);
    tui.say("在吗");
    tui.wait_for("好。");
    let sent = format!("{:?}", script.requests().last());
    assert!(!sent.contains("X1Y2"), "无人格不带默认人格的人设：{sent}");
}
