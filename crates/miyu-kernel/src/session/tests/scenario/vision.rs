//! 场景：替看不了图的模型看图（施工 8-17，`docs/blueprint/kernel/session.md`「替它看图」，`models.md`「怎么走」第十三条）。限额
//! 说看不了图，请求里有还没转述过的图：先交出转述，回来了记 `image.described`（内核记的、不带回合编号、`cause` 是这一轮的），
//! 落了盘再组装、发请求，请求的 `described` 里是它；同一张图只转述一次，第二轮、崩了载入、撤销、压缩以后照样用；看得了图的
//! 不转述；没成的不记、这一轮不再试、下一轮再试；转述带上人这一轮最近说的那一句；看图时打断的直接结束，回来的照样记；
//! 看图时切了级别，回到「准备好」时查事实；快照里没有转述的字的不转述。
//!
//! 替身的组装（[`Listing`]）把人的消息带的图接在那一行后面，转述的请求写着「describe」和人的话。

use super::*;
use crate::block::Image;
use crate::event::ImageDescribed;
use crate::id::{ContentHash, MediaType};
use crate::session::Limits;
use crate::testkit::{model, vision_model};

/// 第 `k` 张图。
fn picture(k: u8) -> Image {
    Image {
        blob: ContentHash::of(&[k]),
        name: None,
        media_type: MediaType::parse("image/png").unwrap(),
        width: 640,
        height: 480,
        path: None,
    }
}

/// 一句话带一张图；`words` 是空的只有图。
fn with_picture(words: &str, k: u8) -> Vec<Block> {
    let mut blocks = Vec::new();
    if !words.is_empty() {
        blocks.push(Block::Text(Text {
            text: words.to_string(),
        }));
    }
    blocks.push(Block::Image(picture(k)));
    blocks
}

/// 交限额：`blind` 的是看不了图。
fn see(stage: &mut Stage, blind: bool) {
    stage.limits_with(Limits {
        model: model(),
        window: None,
        max_output: None,
        images: None,
        blind,
    });
}

/// 看不了图的替身。
fn blind_stage() -> Stage {
    let mut stage = stage();
    see(&mut stage, true);
    stage
}

/// 日志里的 `image.described` 和它的那一条，照先后。
fn descriptions(stage: &Stage) -> Vec<(&Event, &ImageDescribed)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ImageDescribed(described) => Some((event, described)),
            _ => None,
        })
        .collect()
}

/// 第 `k` 次请求的转述。
fn described(stage: &Stage, k: usize) -> BTreeMap<ContentHash, String> {
    stage.requests()[k].1.described.clone()
}

/// 转述请求的字：替身的组装写的「describe」和人的话。
fn asked(stage: &Stage, k: usize) -> String {
    match &stage.describes()[k].1.messages[..] {
        [Message::User { blocks }] => match &blocks[..] {
            [Block::Text(text), Block::Image(image)] => {
                assert_eq!(image.blob, stage.describes()[k].0, "请求里是这张图");
                text.text.clone()
            }
            other => panic!("一块字、一张图：{other:?}"),
        },
        other => panic!("一条 user：{other:?}"),
    }
}

#[test]
fn a_blind_model_gets_the_picture_described_before_the_request() {
    let mut stage = blind_stage();
    stage.vision([Some("A red sign that reads STOP.")]);
    stage.model([Line::says("是停车标志。")]);
    let sent = stage.send(with_picture("这是什么？", 1));
    assert_eq!(stage.describes().len(), 1);
    assert_eq!(stage.describes()[0].0, picture(1).blob);
    let [(event, body)] = descriptions(&stage)[..] else {
        panic!("记了一条转述：{:?}", story(&stage));
    };
    assert_eq!(
        body,
        &ImageDescribed {
            blob: picture(1).blob,
            endpoint: vision_model().endpoint,
            model: vision_model().model,
            text: "A red sign that reads STOP.".to_string(),
        }
    );
    assert_eq!(
        (&event.by, event.turn, &event.cause),
        (&By::Kernel, None, &Some(sent)),
        "内核记的，不带回合编号，cause 是这一轮的"
    );
    let [(seen, _)] = stage.requests() else {
        panic!("发了一次请求");
    };
    assert!(event.seq <= *seen, "转述落了盘才请求");
    assert_eq!(
        described(&stage, 0),
        BTreeMap::from([(picture(1).blob, "A red sign that reads STOP.".to_string())])
    );
}

#[test]
fn a_picture_is_described_once_and_the_requests_keep_it() {
    let mut stage = blind_stage();
    stage.vision([Some("A cat.")]);
    stage.model([Line::says("一只猫。"), Line::says("还是那只猫。")]);
    stage.send(with_picture("看看", 1));
    stage.send(with_picture("再看看", 1));
    assert_eq!(stage.describes().len(), 1, "同一张图只转述一次");
    assert_eq!(described(&stage, 1), described(&stage, 0));
    // 崩了载入、撤销、压缩以后照样用：转述过哪些从日志里算，不看有效历史。
    stage.crash();
    stage.model([Line::says("猫。")]);
    stage.send(with_picture("", 1));
    let last = stage.turns().last().copied().unwrap();
    stage.revert(last);
    stage.compact("They looked at a cat.");
    stage.model([Line::says("又是猫。")]);
    stage.send(with_picture("最后一次", 1));
    assert_eq!(stage.describes().len(), 1, "载入、撤销、压缩以后都不再转述");
    assert_eq!(descriptions(&stage).len(), 1);
    let last = stage.requests().len() - 1;
    assert_eq!(described(&stage, last), described(&stage, 0));
}

#[test]
fn a_model_that_sees_gets_the_picture_itself() {
    let mut stage = stage();
    see(&mut stage, false);
    stage.model([Line::says("一只狗。")]);
    stage.send(with_picture("这是什么？", 2));
    assert!(stage.describes().is_empty(), "看得了图的不转述");
    assert!(descriptions(&stage).is_empty());
    assert!(described(&stage, 0).is_empty());
    // 没交过限额的也一样。
    let mut stage = self::stage();
    stage.model([Line::says("一只狗。")]);
    stage.send(with_picture("这是什么？", 2));
    assert!(stage.describes().is_empty());
}

#[test]
fn a_failed_description_is_tried_again_next_turn() {
    let mut stage = blind_stage();
    stage.vision([None, Some("Line three: 42.")]);
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("看不清。"),
        Line::says("第三行是 42。"),
    ]);
    stage.tools([Play::done("a")]);
    stage.send(with_picture("第三行是什么？", 3));
    assert_eq!(stage.describes().len(), 1, "这一轮没成的不再试");
    assert!(descriptions(&stage).is_empty(), "没成的不记");
    assert_eq!(stage.requests().len(), 2, "主请求照发");
    assert!(described(&stage, 0).is_empty() && described(&stage, 1).is_empty());
    stage.say("再试试？");
    assert_eq!(stage.describes().len(), 2, "下一轮再试");
    assert_eq!(descriptions(&stage).len(), 1);
    assert_eq!(
        described(&stage, 2),
        BTreeMap::from([(picture(3).blob, "Line three: 42.".to_string())])
    );
}

#[test]
fn the_latest_words_of_this_turn_go_with_the_picture() {
    let mut stage = blind_stage();
    stage.vision([Some("A chart."), Some("A map.")]);
    stage.model([
        Line::says("好。"),
        Line::says("图表。"),
        Line::says("地图。"),
    ]);
    stage.say("先说一句");
    stage.send(with_picture("第三行写的是什么？", 4));
    assert_eq!(asked(&stage, 0), "describe 第三行写的是什么？");
    // 这一轮只发了一张图：人上一轮说的不算。
    stage.send(with_picture("", 5));
    assert_eq!(asked(&stage, 1), "describe ");
}

#[test]
fn an_interrupted_look_ends_the_turn_and_the_description_still_lands() {
    let mut stage = blind_stage();
    stage.vision([Some("A receipt.")]);
    stage.hold_vision();
    let sent = stage.send(with_picture("多少钱？", 6));
    assert!(stage.requests().is_empty(), "转述回来以前不请求");
    stage.interrupt(Queued::Return);
    assert!(
        story(&stage)
            .last()
            .unwrap()
            .contains("turn.ended:interrupted"),
        "{:?}",
        story(&stage)
    );
    stage.release_vision();
    assert!(stage.requests().is_empty(), "打断了的那一轮不再请求");
    let [(event, _)] = descriptions(&stage)[..] else {
        panic!("回来的照样记：{:?}", story(&stage));
    };
    assert_eq!((event.turn, &event.cause), (None, &Some(sent)));
    stage.model([Line::says("一共 42 元。")]);
    stage.send(with_picture("多少钱？", 6));
    assert_eq!(stage.describes().len(), 1, "记下了就不再转述");
    assert_eq!(
        described(&stage, 0),
        BTreeMap::from([(picture(6).blob, "A receipt.".to_string())])
    );
}

/// 看图时切了级别：回到「准备好」时查一遍事实，权限那一块排在转述后面、请求前面。
#[test]
fn a_level_switched_while_looking_is_injected_before_the_request() {
    let mut stage = blind_stage();
    stage.vision([Some("A diagram.")]);
    stage.model([Line::says("一张图。")]);
    stage.hold_vision();
    stage.send(with_picture("看看", 7));
    stage.set_permission(None, Some(true));
    stage.release_vision();
    let story = story(&stage);
    let at = |kind: &str| {
        story
            .iter()
            .rposition(|line| line.contains(kind))
            .unwrap_or_else(|| panic!("没有 {kind}：{story:?}"))
    };
    assert!(at("image.described") < at("context.injected:permission"));
    assert!(at("context.injected:permission") < at("message.assistant"));
}

/// 快照里没有转述的字的（以前造的会话）：组装器交不出转述的请求，不转述，照常请求。
#[test]
fn an_old_snapshot_does_not_describe() {
    struct Old;
    impl Assembler for Old {
        fn assemble(&self, history: &History) -> Request {
            Listing.assemble(history)
        }
        fn summarize(
            &self,
            history: &History,
            upto: Seq,
            cut: Option<Seq>,
            instructions: Option<&str>,
        ) -> Request {
            Listing.summarize(history, upto, cut, instructions)
        }
        fn summarize_isolated(
            &self,
            history: &History,
            upto: Seq,
            cut: Option<Seq>,
            instructions: Option<&str>,
        ) -> Request {
            Listing.summarize_isolated(history, upto, cut, instructions)
        }
        fn summary(&self, reply: &[Block]) -> Option<String> {
            Listing.summary(reply)
        }
    }
    let old = || Policy {
        assembler: Box::new(Old),
        ..policy()
    };
    let mut stage = Stage::new(old, environment("~/src/miyu"), at(0));
    see(&mut stage, true);
    stage.model([Line::says("看不到图。")]);
    stage.send(with_picture("这是什么？", 8));
    assert!(stage.describes().is_empty());
    assert_eq!(stage.requests().len(), 1);
    assert!(described(&stage, 0).is_empty());
}
