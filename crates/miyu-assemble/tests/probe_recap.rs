//! 请求形状探针：回顾的那一张脸（施工 3-8 四补，`docs/blueprint/kernel/request.md`「回顾的请求」）。真内核照剧本跑，组装、
//! 给模型看的字都是出厂的：人问一句，她读了文件答；Claude Code 闲着时说一句，开了一轮；人再问，她正答着时要一句回顾；答完
//! 再要一句；中间没有新内容再要一次，交回上一句、不请求。
//!
//! 回顾的请求是单独的一次：一条 user，指令接对话记录，没有 system、工具面；和存档（`docs/designs/samples/probe/recap/`
//! 的 `recaps/`）逐字节比。主请求照样查五条性质、和存档比：回顾夹在中间，不改它们。

use crate::support::{check, files, matches_the_archive, recap, sent, stage};
use miyu_kernel::block::Block;
use miyu_kernel::id::CommandId;
use miyu_kernel::request::Message;
use miyu_kernel::session::Outcome;
use miyu_kernel::testkit::{Line, Play, Stage};

/// 剧本。
fn script() -> Stage {
    let mut s = stage();
    // 1. 人问一句，她读了文件答。
    s.model([
        Line::calls(
            "我读一下测试的临时目录。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
        Line::says("CI 红在 macOS：临时目录在 /var 这个链接下面，安全打开不走链接，拒绝了。"),
    ]);
    s.tools([Play::done(
        "fn temp_dir() -> PathBuf { std::env::temp_dir() }",
    )]);
    s.say("CI 为什么红了？");
    // 2. 她闲着，Claude Code 说一句：由它开一轮。
    s.advance(2);
    s.model([Line::says("好，那我等你改完。")]);
    s.harness_says("claude-code", "我来把临时目录换成真实路径。");
    // 3. 人再问，她正答着时要一句回顾：照这时落了盘的。
    s.advance(2);
    s.model([Line::says("改好了，测试也过了。").held()]);
    s.say("改完了吗？");
    s.recap_model([
        Line::says("在修 macOS 上的 CI：临时目录在链接下面被拒。Claude Code 在换成真实路径，你刚问改完没有。"),
        Line::says("macOS 上的 CI 修好了：临时目录换成了真实路径，测试也过了。"),
    ]);
    s.recap();
    s.release_model();
    // 4. 答完再要一句，照到的是她刚答的那一句；再要一次，中间没有新内容，交回上一句。
    s.recap();
    s.recap();
    s
}

#[test]
fn the_recap_session_matches_the_archive() {
    matches_the_archive("recap", &script());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&script()), files(&script()));
}

/// 回顾的请求：一条 user，指令在最前，后面是人的话、她每一轮最后的回答，Claude Code 的话带着外壳；工具的输出不在里面。
/// 两次请求，第三次交回上一句。主请求的五条性质照查。
#[test]
fn a_recap_is_its_own_request_and_leaves_the_main_ones_alone() {
    let session = script();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let recaps = session.recaps();
    assert_eq!(recaps.len(), 2, "第三次交回上一句");
    let instruction = recap().instruction;
    for (_, request) in recaps {
        assert!(request.tools.is_empty() && request.system.is_empty());
        let [Message::User { blocks }] = request.messages.as_slice() else {
            panic!("一条 user：{:?}", request.messages);
        };
        let [Block::Text(text)] = blocks.as_slice() else {
            panic!("一块字：{blocks:?}");
        };
        assert!(text.text.starts_with(&instruction), "指令在最前");
        assert!(text.text.contains("User: CI 为什么红了？"));
        assert!(text.text.contains("<agent-message from=\"claude-code\">"));
        assert!(!text.text.contains("temp_dir"), "工具的输出不在里面");
        assert!(!text.text.contains("我读一下测试"), "中间一步说的不在里面");
    }
    let last = |k: usize| {
        let (_, request) = &recaps[k];
        let Message::User { blocks } = &request.messages[0] else {
            unreachable!("上面查过");
        };
        match &blocks[0] {
            Block::Text(text) => text.text.clone(),
            other => panic!("{other:?}"),
        }
    };
    assert!(
        last(0).ends_with("User: 改完了吗？"),
        "正答着时：最后是还没答的那一句"
    );
    assert!(last(1).ends_with("Assistant: 改好了，测试也过了。"));
    // 人说了两句、Claude Code 说了一句，三次回顾是第 4 到 6 个命令。
    let cached: Vec<Option<bool>> = (4..=6)
        .map(|n| match session.outcome(&command(n)) {
            Some(Outcome::Recapped { cached, .. }) => Some(*cached),
            _ => None,
        })
        .collect();
    assert_eq!(cached, [Some(false), Some(false), Some(true)]);
}

/// 第 `n` 个命令的编号。
fn command(n: u64) -> CommandId {
    CommandId::parse(&format!("cmd-{n}")).expect("命令编号合写法")
}
