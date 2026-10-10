//! `miyu onebot venue show <场所>`（施工 O-21，`onebot.md` 第一条「场所规则和出厂数据」第 7 条）：不连核心，读的文件和桥
//! 一样；规则设到的一项一行印值和来处，接一句没列出的参数照出厂的，问题印在后面；场所编号认不出是用法不对，出厂的写错说
//! 是哪些问题。真的程序照系统的语言说；`serve` 出厂的写错握手以前就退，系统的写错照常起来、问题进运行日志。

use std::time::Duration;

use miyu_onebot::texts::Texts;
use miyu_onebot::venue::show;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::rules::{copied_resources, system_rule};
use crate::support::ports::on_free_port;
use crate::support::spawning::{program, served_up, text};
use crate::support::*;

/// 中文的字，资源目录是 `resources`。
fn zh(resources: &std::path::Path) -> Texts {
    Texts::load(ResourceRoot::at(resources), "zh").expect("读得出来")
}

/// 照中文跑一次 `venue show <venue>`：交回退出码、标准输出、标准错误。
fn shown(root: &DataRoot, venue: &str, texts: &Texts) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = show(root, venue, texts, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("UTF-8"),
        String::from_utf8(err).expect("UTF-8"),
    )
}

/// 删掉临时目录；删不掉就留在临时目录里，不影响测试。
fn clean(dir: &std::path::Path) {
    if std::fs::remove_dir_all(dir).is_err() {
        // 留着。
    }
}

/// 系统的两份：`80-test.toml` 给群 1 设限流和一项参数，`90-bad.toml` 第 2 条规则的限流写错了。
fn test_rules(root: &DataRoot) {
    system_rule(
        root,
        "80-test.toml",
        "[[rule]]\nmatch = { group = [1] }\nrate = \"30/60s\"\nchatty = { probability = 80 }\n",
    );
    system_rule(
        root,
        "90-bad.toml",
        "[[rule]]\nmatch = { group = [2] }\nparallel = 2\n\n[[rule]]\nrate = \"abc\"\n",
    );
}

#[test]
fn values_and_origins_come_one_a_line_and_problems_after() {
    let (dir, root) = temp_root();
    test_rules(&root);
    assert_eq!(
        shown(&root, "qq:group:1", &zh(&resources())),
        (
            0,
            "chatty.probability = 80（系统 80-test.toml 第 1 条规则，第 4 行）\n\
             discipline = \"chatty\"（出厂 50-defaults.toml 第 1 条规则，第 14 行）\n\
             parallel = 1（出厂 50-defaults.toml 第 1 条规则，第 16 行）\n\
             rate = \"30/60s\"（系统 80-test.toml 第 1 条规则，第 3 行）\n\
             没列出的参数照出厂的 defaults.toml。\n\
             读文件时发现的问题（写错的那一项、那一条规则、那一份文件不用，别的照用）：\n  \
             系统 90-bad.toml 第 2 条规则，第 6 行：rate 写法不对：\"abc\"\n"
                .to_string(),
            String::new()
        )
    );
    clean(&dir);
}

#[test]
fn without_system_rules_the_factory_ones_are_shown() {
    let (dir, root) = temp_root();
    assert_eq!(
        shown(&root, "qq:private:5", &zh(&resources())),
        (
            0,
            "discipline = \"every-message\"（出厂 50-defaults.toml 第 2 条规则，第 20 行）\n\
             parallel = 0（出厂 50-defaults.toml 第 2 条规则，第 22 行）\n\
             rate = \"5/300s\"（出厂 50-defaults.toml 第 2 条规则，第 21 行）\n\
             没列出的参数照出厂的 defaults.toml。\n"
                .to_string(),
            String::new()
        )
    );
    clean(&dir);
}

#[test]
fn a_venue_nothing_sets_says_so() {
    let (dir, root) = temp_root();
    // 同名的空文件替换出厂的那一份：什么规则都没有。
    system_rule(&root, "50-defaults.toml", "");
    assert_eq!(
        shown(&root, "qq:group:1", &zh(&resources())),
        (
            0,
            "没有规则设到这个场所。\n没列出的参数照出厂的 defaults.toml。\n".to_string(),
            String::new()
        )
    );
    clean(&dir);
}

#[test]
fn a_bad_venue_id_is_a_usage_error() {
    let (dir, root) = temp_root();
    for wrong in [
        "qq:channel:1",
        "qq:group:",
        "qq:group:1 2",
        "QQ:group:1",
        "nonsense",
        "",
    ] {
        assert_eq!(
            shown(&root, wrong, &zh(&resources())),
            (
                2,
                String::new(),
                format!(
                    "认不出场所编号 {wrong}。写成 <平台>:group:<群号> 或 <平台>:private:<号>，例如 qq:group:123456。\n"
                )
            ),
            "{wrong:?}"
        );
    }
    clean(&dir);
}

#[test]
fn broken_factory_data_is_said_with_every_problem() {
    let (dir, root) = temp_root();
    let resources = copied_resources();
    let defaults = resources.join("software/onebot/defaults.toml");
    let good = std::fs::read_to_string(&defaults).expect("读得了");
    let line = good
        .lines()
        .position(|line| line.starts_with("probability = 50 "))
        .expect("有这一行")
        + 1;
    std::fs::write(
        &defaults,
        good.replace("probability = 50 ", "probability = 5000 "),
    )
    .expect("写得进");
    let texts = zh(&resources);
    let (code, out, err) = shown(&root, "qq:group:1", &texts);
    assert_eq!((code, out.as_str()), (1, ""));
    assert_eq!(
        err,
        format!(
            "QQ 桥的出厂数据有问题（是打包的错），起不来：\n  出厂 defaults.toml 第 {line} 行：chatty.probability 超出范围：5000\n"
        )
    );
    clean(&dir);
    clean(resources.parent().expect("有上一级"));
}

#[test]
fn the_program_shows_in_the_system_language() {
    let (dir, root) = temp_root();
    test_rules(&root);
    let shown = program(&root, &["venue", "show", "qq:group:1"])
        .output()
        .expect("跑得了");
    assert_eq!(shown.status.code(), Some(0), "{}", text(&shown.stderr));
    assert_eq!(text(&shown.stderr), "");
    assert_eq!(
        text(&shown.stdout),
        "chatty.probability = 80 (system 80-test.toml, rule 1, line 4)\n\
         discipline = \"chatty\" (factory 50-defaults.toml, rule 1, line 14)\n\
         parallel = 1 (factory 50-defaults.toml, rule 1, line 16)\n\
         rate = \"30/60s\" (system 80-test.toml, rule 1, line 3)\n\
         Parameters not listed follow the factory defaults.toml.\n\
         Problems found while reading (the wrong item, rule or file is not used; the rest is):\n  \
         system 90-bad.toml, rule 2, line 6: rate is not written right: \"abc\"\n"
    );
    for wrong in [
        &["venue"][..],
        &["venue", "show"],
        &["venue", "list", "qq:group:1"],
    ] {
        let refused = program(&root, wrong).output().expect("跑得了");
        assert_eq!(refused.status.code(), Some(2), "{wrong:?}");
        assert!(
            text(&refused.stderr).starts_with("usage: "),
            "{wrong:?}：{}",
            text(&refused.stderr)
        );
    }
    let refused = program(&root, &["venue", "show", "qq:room:1"])
        .output()
        .expect("跑得了");
    assert_eq!(refused.status.code(), Some(2));
    assert!(text(&refused.stderr).starts_with("Not a venue id: qq:room:1."));
    clean(&dir);
}

#[test]
fn serve_with_broken_factory_data_stops_before_the_handshake() {
    let (dir, root) = temp_root();
    let resources = copied_resources();
    std::fs::write(
        resources.join("software/onebot/venues.d/50-defaults.toml"),
        "[[rule]]\nrate = \"abc\"\n",
    )
    .expect("写得进");
    let mut command = program(&root, &["serve"]);
    command
        .env("MIYU_RESOURCES", &resources)
        .stdin(std::process::Stdio::null());
    let exited = command.output().expect("跑得了");
    assert_eq!(exited.status.code(), Some(1), "{}", text(&exited.stderr));
    assert_eq!(
        text(&exited.stdout),
        "",
        "握手以前就退：标准输出上什么都没有"
    );
    assert_eq!(
        text(&exited.stderr),
        "The QQ bridge cannot start: its factory data has problems (a packaging error):\n  \
         factory 50-defaults.toml, rule 1, line 2: rate is not written right: \"abc\"\n"
    );
    clean(&dir);
    clean(resources.parent().expect("有上一级"));
}

#[tokio::test]
async fn serve_with_a_system_mistake_starts_and_logs_it() {
    let (dir, root) = temp_root();
    test_rules(&root);
    let mut served = on_free_port(async |listen| served_up(&root, listen).await).await;
    // 核心请它退出：关它的标准输入；好好停下，运行日志写完。
    drop(served.stdin.take());
    let exited = tokio::time::timeout(Duration::from_secs(5), served.child.wait_with_output())
        .await
        .expect("5 秒内退出")
        .expect("等得到");
    assert_eq!(exited.status.code(), Some(0), "{}", text(&exited.stderr));
    let log =
        std::fs::read_to_string(root.state().join("logs").join("onebot.log")).expect("有运行日志");
    let problem = log
        .lines()
        .find(|line| line.contains("venue rules problem"))
        .unwrap_or_else(|| panic!("记了问题：{log}"));
    for must in [
        "WARN",
        "bad_format",
        "90-bad.toml",
        "rule=2",
        "key=rate",
        "line=6",
    ] {
        assert!(problem.contains(must), "{must}：{problem}");
    }
    assert!(log.contains("venue rules read problems=1"), "{log}");
    clean(&dir);
}
