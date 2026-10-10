//! 伪终端里的端到端测试：配置页（蓝图 `tui.md`「配置页」）。真界面连一份真核心，改了存下，看个人设置文件。

mod support;

use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

const DOWN: &[u8] = b"\x1b[B";
const TAB: &[u8] = b"\t";

/// 一家中转站：模型名带组织前缀；一个池。
const RELAY: &str = "[models]\nchat = \"relay/cline/v4\"\n\n[pools.duo]\nmodels = [\"relay/cline/v4\"]\nstrategy = \"rotate\"\n\n[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"http://relay.invalid/v1\"\n\n[providers.relay.models.\"cline/v4\"]\nwindow = 128000\n\n[providers.relay.models.\"zhipu/glm\"]\nwindow = 64000\ninputs = [\"text\", \"image\"]\n";

/// 主菜单上从「通用」挪到「供应商和模型」进去（2026-10-07 起通用排第一行）：先等清单读到、菜单里有了通用，
/// 不然 `j` 落到人格上。
fn to_models(tui: &mut Tui) {
    tui.wait_for("供应商、默认模型、模型池");
    tui.wait_for("显示");
    tui.key(b"j");
    tui.key(b"\r");
}

/// 照清单画的一页里，名字是 `name` 的那一项是第几项：一项一行、名字和值中间空着几格，组名（只有一个词）、空行不算；
/// 只看下边框那条线以上（下面是按键提示）。
fn item_index(tui: &Tui, name: &str) -> usize {
    let lines = tui.lines();
    let end = lines
        .iter()
        .position(|l| l.contains("────"))
        .unwrap_or(lines.len());
    let items: Vec<String> = lines[..end]
        .iter()
        .skip(1)
        .filter(|l| l.trim().contains("  "))
        .cloned()
        .collect();
    items
        .iter()
        .position(|l| l.contains(name))
        .unwrap_or_else(|| panic!("页里没有「{name}」：{items:?}"))
}

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
    to_models(&mut tui);
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
    to_models(&mut tui);
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
    to_models(&mut tui);
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

#[test]
fn audio_can_be_ticked_and_the_core_takes_it() {
    // 2026-10-07 项目主人要；核心 8-27 起收。
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    to_models(&mut tui);
    tui.wait_for("cline");
    tui.key(b"l");
    tui.key(b"l");
    tui.key(b"\r");
    tui.wait_for("音频");
    // 停在「支持输入」：往右三下到音频，勾上，存。
    for _ in 0..3 {
        tui.key(b"\x1b[C");
    }
    tui.key(b" ");
    tui.key(b"s");
    wait_settings(&home, &mut tui, "\"audio\"");
}

#[test]
fn slash_connect_goes_straight_to_providers_and_back_to_the_chat() {
    // 2026-10-07 项目主人：/connect 直接进供应商和模型。
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/connect");
    tui.wait_for("组织");
    assert!(
        !tui.lines()
            .iter()
            .any(|l| l.contains("供应商、默认模型、模型池")),
        "不经过主菜单"
    );
    tui.key(b"\x1b");
    tui.wait_for("工作区");
    assert!(
        !tui.lines().iter().any(|l| l.contains("组织")),
        "直接回对话"
    );
}

#[test]
fn the_general_page_comes_first_changes_a_select_resets_it_and_the_personas_page_shows_details() {
    // 2026-10-07 项目主人：通用放第一行（默认人格在这里），人格单独一个菜单、只看。
    let home = Home::with_settings(Script::new([Play::Says("好。")]), RELAY);
    // 出厂不带人格（核心 P-4 下）：放一个自己的。
    home.persona("pirate", "[persona]\nname = \"海盗\"\n", Some("你是海盗。"));
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("显示");
    let menu = tui.lines();
    let at = |word: &str| menu.iter().position(|l| l.contains(word));
    assert!(at("通用") < at("供应商和模型"), "通用排第一行");
    assert!(
        at("权限") < at("人格和记忆") && at("人格和记忆") < at("高级"),
        "人格在权限后面、高级前面"
    );
    tui.key(b"\r");
    tui.wait_for("界面语言");
    tui.wait_for("默认人格");
    // 第一项是常用的界面语言：开选择窗，挪到「中文」，回车当场存。
    tui.key(b"\r");
    tui.wait_for("跟随系统");
    tui.key(DOWN);
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "language = \"zh\"");
    // 值后面不写来自哪一层（2026-10-08 项目主人：「所有的这种默认文字都去掉」）。
    tui.wait_for("中文");
    assert!(!tui.shows("个人设置"), "{}", tui.lines().join("\n"));
    // `d` 恢复默认：问一句，挪到「恢复」，回车。
    tui.key(b"d");
    tui.wait_for("恢复默认：界面语言？");
    tui.key(b"h");
    tui.key(b"\r");
    let end = Instant::now() + support::WAIT;
    while home.settings().contains("language") {
        assert!(Instant::now() < end, "没去掉：\n{}", home.settings());
        tui.pump(Duration::from_millis(100));
    }
    // 默认人格：开的是人格列表，不是填字的编辑窗（2026-10-07 项目主人报：打开成了编辑框）。照屏幕上数它是第几项再挪，
    // 核心以后在前面加了项（9-3 加了「用哪个界面」）也不错位。
    for _ in 0..item_index(&tui, "默认人格") {
        tui.key(b"j");
    }
    tui.key(b"\r");
    tui.wait_for("↑/↓ 选择");
    assert!(tui.shows("海盗"), "列人格：\n{}", tui.lines().join("\n"));
    assert!(!tui.shows("Enter 保存"), "不是编辑窗");
    // 第一行「无人格」：选了就不设默认人格（2026-10-08 项目主人：人格允许为空）。
    assert!(tui.shows("无人格"), "{}", tui.lines().join("\n"));
    tui.key(b"\x1b");
    // 默认预设（人格那一组之后的第五项）：一样开预设列表（核心 P-2 上）。
    tui.key(b"j");
    tui.key(b"\r");
    tui.wait_for("↑/↓ 选择");
    assert!(
        tui.shows("全部功能") && tui.shows("基础功能"),
        "列预设：\n{}",
        tui.lines().join("\n")
    );
    tui.key(b"\x1b");
    // 回主菜单，挪到人格（第三项：「权限」并进了「通用」）进去，回车看详情。
    tui.key(b"\x1b");
    tui.wait_for("人格和记忆");
    for _ in 0..2 {
        tui.key(b"j");
    }
    tui.key(b"\r");
    tui.wait_for("海盗");
    // 列表右边不写来自哪一层（2026-10-08 项目主人：用户用不上）。
    assert!(!tui.shows("内置"), "{}", tui.lines().join("\n"));
    tui.key(b"\r");
    tui.wait_for("人设提醒短语");
    assert!(!tui.shows("来自"), "{}", tui.lines().join("\n"));
    tui.key(b"\x1b");
    // 回主菜单，人格下面一项是预设（2026-10-07 项目主人：人格和预设分两个菜单）：列着两个出厂的（「基础功能」「全部功能」，
    // 核心 P-4 下改名），基础功能的窗里一个功能一个开关（核心 F-3 下起是功能，不是软件包），写了没装的写「未安装」。
    tui.key(b"\x1b");
    tui.wait_for("决定开关哪些功能");
    tui.key(b"j");
    tui.key(b"\r");
    tui.wait_for("全部功能");
    tui.wait_for("基础功能");
    tui.key(b"\r");
    tui.wait_for("功能");
    tui.wait_for("未安装");
    // 预设不再管默认人格（2026-10-08 项目主人）。
    assert!(!tui.shows("默认人格"), "{}", tui.lines().join("\n"));
    // 功能写给人看的名字，不写编号（2026-10-08 项目主人：「都是英文谁看得懂？」，核心 P-3 补给名字）。
    assert!(tui.shows("文件读写"), "{}", tui.lines().join("\n"));
    assert!(!tui.shows("files"), "{}", tui.lines().join("\n"));
    tui.key(b"\x1b");
}

#[test]
fn the_terminals_own_switches_sit_on_the_general_page() {
    // 「配置页」第 33 条、「时间线」第 18 条（2026-10-10 项目主人：「通用配置界面里没有 timeline 样式的配置项吗」「timeline
    // 的配置项不应该是 tui 自己的吗」）：终端清单里声明的 `tui.*` 挪到通用页「终端界面」一组，开关当场存。
    let home = Home::with_settings(Script::new([]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("显示");
    tui.key(b"\r");
    tui.wait_for("界面语言");
    // 一组在最后：往下挪到看得见「完成后折叠」，光标停在它上面。
    for _ in 0..60 {
        if tui.shows("完成后折叠")
            && tui
                .lines()
                .iter()
                .any(|l| l.contains('┃') && l.contains("完成后折叠"))
        {
            break;
        }
        tui.key(b"j");
    }
    assert!(tui.shows("终端界面"), "{}", tui.lines().join("\n"));
    tui.key(b"\r");
    wait_settings(&home, &mut tui, "timeline_fold = false");
}

#[test]
fn the_mascot_item_offers_the_built_in_one_first() {
    // 「吉祥物包」第 3 条（2026-10-11 项目主人）：通用页「终端界面」一组多一项「吉祥物」，没设的写「内置」；回车开选择窗，
    // 第一行「内置」，下面是装了的吉祥物包。
    let home = Home::with_settings(Script::new([]), RELAY);
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("显示");
    tui.key(b"\r");
    tui.wait_for("界面语言");
    for _ in 0..80 {
        if tui
            .lines()
            .iter()
            .any(|l| l.contains('┃') && l.contains("吉祥物") && !l.contains("显示"))
        {
            break;
        }
        tui.key(b"j");
    }
    let row = tui
        .lines()
        .into_iter()
        .find(|l| l.contains('┃') && l.contains("吉祥物"))
        .unwrap_or_default();
    assert!(row.contains("内置"), "{}", tui.lines().join("\n"));
    tui.key(b"\r");
    tui.pump(std::time::Duration::from_millis(300));
    assert!(
        tui.lines()
            .iter()
            .any(|l| l.contains("内置") && !l.contains('┃')),
        "选择窗第一行「内置」：\n{}",
        tui.lines().join("\n")
    );
}
