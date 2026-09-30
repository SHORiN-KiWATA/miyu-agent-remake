//! 请求形状探针：起标题的那一张脸（施工 3-8 五补，`docs/blueprint/kernel/request.md`「起标题的请求」）。真内核照剧本跑，
//! 组装、给模型看的字都是出厂的：人问一句，她读了文件答，答完内核起标题；人再问，她再答，不再起。
//!
//! 起标题的请求是单独的一次：一条 user，指令接第一轮的对话记录，没有 system、工具面；和存档（`docs/designs/samples/probe/
//! title/` 的 `titles/`）逐字节比。主请求照样查五条性质、和存档比：标题的两条夹在两轮中间，第二轮的请求照旧接着第一轮的
//! 往后长。

mod support;

use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_kernel::request::Message;
use miyu_kernel::testkit::{Line, Play, Stage};
use support::{check, files, matches_the_archive, sent, title, titled_stage};

/// 剧本。
fn script() -> Stage {
    let mut s = titled_stage();
    // 1. 人问一句，她读了文件答；答完起标题。
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
    s.title_model([Line::says("macOS 上 CI 变红的原因")]);
    s.say("CI 为什么红了？");
    // 2. 人再问，她再答：起过了，不再起。
    s.advance(2);
    s.model([Line::says("换成真实路径：先 canonicalize 再打开。")]);
    s.say("那怎么改？");
    s
}

#[test]
fn the_title_session_matches_the_archive() {
    matches_the_archive("title", &script());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&script()), files(&script()));
}

/// 起标题的请求：一条 user，指令在最前，后面是第一轮人的话和她那一轮最后的回答；工具的输出、中间一步说的不在里面。只起
/// 一次，标题记在日志里。主请求的五条性质照查。
#[test]
fn a_title_is_its_own_request_and_leaves_the_main_ones_alone() {
    let session = script();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let [(_, request)] = session.titles() else {
        panic!("只起一次：{:?}", session.titles().len());
    };
    assert!(request.tools.is_empty() && request.system.is_empty());
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    let instruction = title().instruction;
    assert_eq!(
        text.text,
        format!(
            "{instruction}User: CI 为什么红了？\n\nAssistant: CI 红在 macOS：临时目录在 /var 这个链接下面，安全打开不走链接，拒绝了。"
        )
    );
    let titled = session.log().iter().any(|event| {
        matches!(&event.body, Body::MetaChanged(changed) if changed.title.as_deref() == Some("macOS 上 CI 变红的原因"))
    });
    assert!(titled, "标题记在日志里");
}
