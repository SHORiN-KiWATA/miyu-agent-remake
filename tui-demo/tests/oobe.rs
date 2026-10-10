//! 第一次打开的引导（蓝图 `tui.md`「第一次打开的引导」）：`--page oobe` 起来，一步步走到底，查核心那边写下的人格、默认
//! 人格、默认预设。测具的核心没有目录、不能真的试一家，接模型那一步走「已配好 · 用它」；选一家、试、存的那一段在单元
//! 测试里（`src/oobe/model/tests.rs`）。

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

/// 配好了一家、默认模型是它：接模型那一步写「已配好」。
const CONFIGURED: &str = "[providers.fake]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[models]\nchat = \"fake/m\"\n";

/// 等 `check` 成立，最多 [`support::WAIT`]；等不到的把屏幕打出来。
fn until(tui: &mut Tui, what: &str, check: impl Fn() -> bool) {
    let end = Instant::now() + support::WAIT;
    while !check() {
        assert!(
            Instant::now() < end,
            "等不到{what}，屏幕是：\n{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
}

/// 起引导，走过欢迎、语言、图标、接模型（用已配好的），停在建人格。
fn to_persona(home: &Home) -> Tui {
    let mut tui = home.tui_args("zh_CN.UTF-8", &["--page", "oobe"]);
    tui.wait_for("按 Enter 开始");
    tui.key(b"\r");
    tui.wait_for("界面语言");
    tui.key(b"\r");
    tui.wait_for("方块或乱码");
    tui.key(b"\r");
    tui.wait_for("已配置");
    tui.key(b"\r");
    tui.wait_for("创建人格");
    tui
}

fn persona_file(home: &Home, name: &str) -> PathBuf {
    home.root().join("home/alice/personas/persona-1").join(name)
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn a_home_that_never_walked_the_guide_opens_it_and_one_that_did_does_not() {
    // 核心 8-11 四补：没走过的（`ui.welcomed` 是假）连上就进引导，盖住首页；走过的照常进首页。
    let fresh = Home::unwelcomed(Script::new([]), CONFIGURED);
    let mut tui = fresh.tui("zh_CN.UTF-8");
    // 问到 `ui.welcomed` 以前不画首页（2026-10-10 项目主人报：进引导前闪一下空会话）。
    let start = tui.record_from_here();
    tui.wait_for("按 Enter 开始");
    let flashed = support::frames(start, &tui.recorded())
        .into_iter()
        .find(|f| f.iter().any(|l| l.contains("Tab 切换权限级别")));
    assert!(
        flashed.is_none(),
        "闪过首页：\n{}",
        flashed.unwrap_or_default().join("\n")
    );
    let walked = Home::with_settings(Script::new([]), CONFIGURED);
    let mut tui = walked.tui("zh_CN.UTF-8");
    tui.wait_for("Tab 切换权限级别");
    tui.pump(Duration::from_millis(800));
    assert!(!tui.shows("按 Enter 开始"), "{}", tui.lines().join("\n"));
}

#[test]
fn walking_the_whole_guide_creates_the_persona_and_sets_the_defaults() {
    let home = Home::unwelcomed(Script::new([Play::Says("好。")]), CONFIGURED);
    let mut tui = to_persona(&home);
    // 四格：名字（回车编辑、回车写好）、人格提示词（回车开大编辑浮窗，Esc 收起）、示范对话（回车开浮窗，a 加一轮）、
    // 人设提醒短语（第 7、21a 条）。
    tui.key(b"\r");
    tui.wait_for("Enter 完成");
    tui.type_text("小助手");
    tui.key(b"\r");
    tui.key(b"j");
    tui.key(b"j");
    tui.key(b"\r");
    tui.wait_for("Esc 完成");
    tui.type_text("你是小助手。");
    tui.key(b"\x1b");
    tui.wait_for("你是小助手。");
    tui.key(b"\x1b[B");
    tui.key(b"\r");
    tui.wait_for("暂无示范对话");
    tui.key(b"a");
    tui.wait_for("你问的");
    tui.type_text("在吗");
    tui.key(b"\r");
    tui.type_text("在的");
    tui.key(b"\r");
    tui.wait_for("在的");
    tui.key(b"\x1b");
    tui.wait_for("1 轮");
    tui.key(b"\x1b[B");
    tui.key(b"\r");
    tui.wait_for("Esc 完成");
    tui.type_text("说话简短。");
    tui.key(b"\x1b");
    tui.wait_for("说话简短。");
    tui.key(b"\x1b[B");
    tui.key(b"\r");
    // 建好、写成默认人格，名字写到吉祥物脚下；到了选预设。
    tui.wait_for("选择预设");
    assert!(read(&persona_file(&home, "persona.toml")).contains("小助手"));
    assert!(read(&persona_file(&home, "prompts/persona.md")).contains("你是小助手。"));
    assert!(read(&persona_file(&home, "prompts/examples.md")).contains("在吗"));
    assert!(read(&persona_file(&home, "prompts/reminders.md")).contains("说话简短。"));
    until(&mut tui, "默认人格", || {
        home.settings().contains("persona-1")
    });
    // 选着现在的默认预设（全部功能），回车。
    tui.wait_for("全部功能");
    tui.key(b"\r");
    tui.wait_for("按 Enter 开始对话");
    assert!(
        tui.shows("小助手") && tui.shows("全部功能"),
        "{}",
        tui.lines().join("\n")
    );
    until(&mut tui, "默认预设", || {
        home.settings().contains("[preset]")
    });
    // 回车走：记下走过了，回到平常的首页。
    tui.key(b"\r");
    tui.wait_for("Tab 切换权限级别");
    assert!(!tui.shows("按 Enter 开始对话"));
    until(&mut tui, "记下走过了", || {
        home.settings().contains("welcomed = true")
    });
    assert!(!tui.shows("没记下"), "{}", tui.lines().join("\n"));
}

#[test]
fn a_name_is_required_and_esc_walks_back_a_step() {
    let home = Home::with_settings(Script::new([]), CONFIGURED);
    let mut tui = to_persona(&home);
    // 名称、头像、人格提示词、示范对话、人设提醒短语，下面是「下一步」。
    for _ in 0..5 {
        tui.key(b"\x1b[B");
    }
    tui.key(b"\r");
    tui.wait_for("请填写名称");
    assert!(
        !persona_file(&home, "persona.toml").exists(),
        "名字空着不建"
    );
    tui.key(b"\x1b");
    tui.wait_for("已配置");
    tui.key(b"\x1b");
    tui.wait_for("方块或乱码");
}

#[test]
fn choosing_english_switches_the_guide_and_is_saved() {
    let home = Home::with_settings(Script::new([]), CONFIGURED);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["--page", "oobe"]);
    tui.wait_for("按 Enter 开始");
    tui.key(b"\r");
    tui.wait_for("界面语言");
    // 跟随系统、中文、English、日本語：往下两行。
    tui.key(b"\x1b[B");
    tui.key(b"\x1b[B");
    tui.key(b"\r");
    tui.wait_for("Boxes or garbage");
    until(&mut tui, "写进个人设置", || {
        home.settings().contains("language = \"en\"")
    });
}

#[test]
fn a_custom_preset_is_created_named_and_made_the_default() {
    let home = Home::with_settings(Script::new([]), CONFIGURED);
    let mut tui = to_persona(&home);
    tui.key(b"\r");
    tui.type_text("小助手");
    tui.key(b"\r");
    // 名称、头像、人格提示词、示范对话、人设提醒短语，下面是「下一步」。
    for _ in 0..5 {
        tui.key(b"\x1b[B");
    }
    tui.key(b"\r");
    tui.wait_for("选择预设");
    tui.wait_for("全部功能");
    // 自定义在最后：回车开浮窗（光标在名字），回车填名字，关掉第一项功能，到最后一行「创建」回车建好。
    assert!(tui.shows("自选启用的功能"));
    tui.key(b"\x1b[B");
    tui.key(b"\r");
    tui.wait_for("输入名称");
    assert!(tui.shows("[*]"));
    tui.key(b"\r");
    tui.type_text("写代码");
    tui.key(b"\r");
    tui.key(b"\x1b[B");
    tui.key(b" ");
    tui.wait_for("[ ]");
    for _ in 0..40 {
        tui.key(b"j");
    }
    // 功能多的往下滚，停上去就露出来。
    tui.wait_for("创建 →");
    tui.key(b"\r");
    tui.wait_for("按 Enter 开始对话");
    let file = home.root().join("home/alice/presets/preset-1.toml");
    let text = read(&file);
    assert!(
        text.contains("写代码") && text.contains("= false"),
        "{text}"
    );
    until(&mut tui, "默认预设", || {
        home.settings().contains("preset-1")
    });
}

#[test]
fn the_first_session_after_the_guide_uses_what_it_just_chose_without_asking() {
    // 2026-10-09 项目主人定（照推荐）：刚在引导里选完人格、预设，紧接着的新会话不再弹两个框，直接用刚选的。
    let home = Home::unwelcomed(Script::new([Play::Says("好。")]), CONFIGURED);
    let picks = [("MIYU_TUI_PERSONA", ""), ("MIYU_TUI_PRESET", "")];
    let mut tui = home.tui_wide_with("zh_CN.UTF-8", 140, &picks);
    tui.wait_for("按 Enter 开始");
    tui.key(b"\r");
    tui.wait_for("界面语言");
    tui.key(b"\r");
    tui.wait_for("方块或乱码");
    tui.key(b"\r");
    tui.wait_for("已配置");
    tui.key(b"\r");
    tui.wait_for("创建人格");
    tui.key(b"\r");
    tui.type_text("小助手");
    tui.key(b"\r");
    // 名称、头像、人格提示词、示范对话、人设提醒短语，下面是「下一步」。
    for _ in 0..5 {
        tui.key(b"\x1b[B");
    }
    tui.key(b"\r");
    tui.wait_for("全部功能");
    // 选基础功能（不是出厂的默认）。
    tui.key(b"\x1b[A");
    tui.key(b"\r");
    tui.wait_for("按 Enter 开始对话");
    tui.key(b"\r");
    tui.wait_for("Tab 切换权限级别");
    tui.pump(Duration::from_millis(800));
    assert!(
        !tui.shows("请选择人格") && !tui.shows("请选择预设"),
        "{}",
        tui.lines().join("\n")
    );
    // 说一句：会话照刚选的开，侧边栏写人格、预设。
    tui.type_text("你好");
    tui.key(b"\r");
    tui.wait_for("好。");
    tui.wait_for("人格 小助手");
    tui.wait_for("预设 基础功能");
}
