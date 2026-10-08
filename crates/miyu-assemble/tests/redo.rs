//! 重做以后的请求（施工 4-7 再补，`docs/blueprint/kernel/history.md`「重做」第 10 条）：真内核、出厂的组装，新的一轮的第一次
//! 请求和撤掉的那一轮的第一次请求一字不差：统一的请求的规范字节、编码成 OpenAI 兼容接口的字节都一样，内核也记不出第一
//! 处不同（缓存照样命中）。空闲时说的一句、由上一轮排着接过来的几句，两种都比；探针的几条性质照样成立。

use crate::support::{anchored, check, sent, stage, wire};
use miyu_kernel::event::{Body, ModelCalled};
use miyu_kernel::request::Request;
use miyu_kernel::testkit::{Line, Stage};

/// 第 `k` 次请求。
fn request(stage: &Stage, k: usize) -> &Request {
    &stage.requests()[k].1
}

/// 每次请求记下的 `model.called`，照先后。
fn calls(stage: &Stage) -> Vec<&ModelCalled> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .collect()
}

/// 探针的几条性质照样成立（`probe.rs`）：工具调用和结果成对、没有连着的两条 user、每一轮第一次请求的最后一块是触发它的
/// 那句，前缀延伸除了撤销过的；锚盖住的正好是锚那次请求加上它的回复。
fn well_formed(stage: &Stage) {
    if let Err(why) = check(&sent(stage)) {
        panic!("{why}");
    }
    if let Err(why) = anchored(stage) {
        panic!("{why}");
    }
}

/// 第 `k` 次和第 `j` 次请求一字不差：规范字节、线上的字节。
fn same(stage: &Stage, k: usize, j: usize) {
    assert_eq!(
        String::from_utf8(request(stage, k).canonical_bytes()).expect("是 UTF-8"),
        String::from_utf8(request(stage, j).canonical_bytes()).expect("是 UTF-8")
    );
    assert_eq!(
        String::from_utf8(wire(request(stage, k)).body).expect("是 UTF-8"),
        String::from_utf8(wire(request(stage, j)).body).expect("是 UTF-8")
    );
}

#[test]
fn a_redone_turn_asks_exactly_as_the_undone_one() {
    let mut s = stage();
    s.model([
        Line::says("你好。"),
        Line::says("这个项目是 Miyu。"),
        Line::says("这个项目是一个 AI 助手。"),
        Line::says("它是 Rust 写的。"),
    ]);
    s.say("hi");
    s.say("这个项目是做什么的");
    s.redo(None);
    same(&s, 2, 1);
    assert_eq!(
        calls(&s)[2].first_difference,
        None,
        "接着撤掉的那一轮之前的那一次往下长：内核比不出不同"
    );
    // 再重做一次，还是一样。
    s.redo(None);
    same(&s, 3, 1);
    well_formed(&s);
}

#[test]
fn the_words_taken_over_from_the_turn_before_are_asked_the_same_way() {
    let mut s = stage();
    s.model([
        Line::says("好").held(),
        Line::says("两句都听到了。"),
        Line::says("两句又都听到了。"),
    ]);
    s.say("hi");
    s.say("先读 a");
    s.say("再读 b");
    s.release_model();
    s.redo(None);
    same(&s, 2, 1);
    assert_eq!(calls(&s)[2].first_difference, None);
    well_formed(&s);
}
