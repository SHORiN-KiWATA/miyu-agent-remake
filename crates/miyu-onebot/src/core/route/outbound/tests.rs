//! 出站（施工 O-25 上，`onebot.md` 第一条「群里怎么叫她」第 9 条、「怎么走」第 10 条）：群里的情形照投影填，本来想要的引用、@
//! 要那一条有编号、发的人解得出号；过了链的照纯文本拆段，第一段带的引用、@ 照链的结果填；丢了的原因写成什么；私聊两样都是假，
//! 这一轮发出去的换了回合就清。参数照出厂的 `defaults.toml`。

use miyu_chat::{File, OutWhy, Params, Source, Target};
use miyu_kernel::id::ExternalId;
use miyu_kernel::time::Timestamp;

use super::super::projection::{Aim, Speaking};
use super::{Spoken, group_ctx, pass, private_ctx, why_name};
use crate::onebot::Lead;

/// 出厂参数。
fn params() -> Params {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/software/onebot/defaults.toml"
    );
    let file = File {
        source: Source::Factory,
        name: "defaults.toml".to_string(),
        text: std::fs::read_to_string(path).expect("出厂的读得到"),
    };
    Params::read(&file).expect("读得出")
}

/// 第 `seconds` 秒。
fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix_millis(1_760_000_000_000 + seconds * 1000).expect("在范围里")
}

/// 主人第 0 秒说的、编号是 `21` 的那一条。
fn aim() -> Aim {
    Aim {
        msg: Some("21".to_string()),
        sender: ExternalId::parse("qq:10001").expect("合写法"),
        at: at(0),
    }
}

/// 她回 [`aim`] 的时候：之后别人说了 `others` 条。
fn speaking(others: u64) -> Speaking {
    Speaking {
        turn: 3,
        to: Vec::new(),
        aim: Some(aim()),
        others,
        last_is_own: false,
        sent: vec!["说过的".to_string()],
    }
}

#[test]
fn the_group_wants_both_and_counts_from_the_message_she_answers() {
    let ctx = group_ctx(&speaking(2), at(20), params().outbound);
    assert_eq!(
        ctx.target,
        Target {
            quote: true,
            mention: true
        }
    );
    assert_eq!(ctx.since.others, 2);
    assert_eq!(ctx.since.elapsed, 20_000, "从那一条记下算到此刻");
    assert!(!ctx.since.last_is_own);
    assert_eq!(ctx.sent.texts, ["说过的"]);
    let mut nothing = speaking(2);
    nothing.aim = None;
    nothing.last_is_own = true;
    let ctx = group_ctx(&nothing, at(20), params().outbound);
    assert_eq!(
        ctx.target,
        Target::default(),
        "没有她回的那一条：两样都不要"
    );
    assert_eq!((ctx.since.elapsed, ctx.since.last_is_own), (0, true));
    let mut unnumbered = speaking(2);
    unnumbered.aim = Some(Aim { msg: None, ..aim() });
    let ctx = group_ctx(&unnumbered, at(20), params().outbound);
    assert_eq!(
        ctx.target,
        Target {
            quote: false,
            mention: true
        },
        "没有编号的引用不了"
    );
}

#[test]
fn only_the_first_piece_carries_what_the_chain_left() {
    let mut params = params();
    params.split_chars = 2;
    let quoted = pass(
        "**看到**了",
        &group_ctx(&speaking(4), at(1), params.outbound.clone()),
        Some(&aim()),
        params.split_chars,
    )
    .expect("过得了");
    assert_eq!(quoted.pieces, ["看到", "了"], "转成纯文本再拆");
    assert_eq!(
        quoted.lead,
        Lead {
            reply: Some("21".to_string()),
            at: None
        },
        "隔了 4 条引用，没到 15 秒不 @"
    );
    let mentioned = pass(
        "嗯",
        &group_ctx(&speaking(1), at(15), params.outbound.clone()),
        Some(&aim()),
        params.split_chars,
    )
    .expect("过得了");
    assert_eq!(
        mentioned.lead,
        Lead {
            reply: None,
            at: Some("10001".to_string())
        },
        "隔了 15 秒、有人说过话 @ 发它的人"
    );
    let right_away = pass(
        "嗯",
        &group_ctx(&speaking(0), at(1), params.outbound.clone()),
        Some(&aim()),
        params.split_chars,
    )
    .expect("过得了");
    assert_eq!(right_away.lead, Lead::default(), "紧接着回两样都不带");
}

#[test]
fn what_the_chain_drops_and_how_it_is_named() {
    let params = params();
    let ctx = private_ctx(vec!["我在看这个问题。".to_string()], params.outbound);
    assert_eq!(ctx.target, Target::default(), "私聊两样都是假");
    let cases = [
        ("<tool_call>{}</tool_call>", OutWhy::Leaked, "leaked"),
        (" \n ", OutWhy::Blank, "blank"),
        ("（眨眼）", OutWhy::Aside, "aside"),
        ("我在看这个问题！", OutWhy::Repeated, "repeated"),
    ];
    for (text, why, name) in cases {
        assert_eq!(pass(text, &ctx, None, 0), Err(why), "{text}");
        assert_eq!(why_name(why), name);
    }
    let cleaned = pass("好<tool_call>{}</tool_call>的", &ctx, None, 0).expect("过得了");
    assert_eq!(cleaned.pieces, ["好的"]);
    assert_eq!(cleaned.lead, Lead::default());
}

#[test]
fn what_was_spoken_is_kept_for_one_turn() {
    let mut spoken = Spoken::default();
    spoken.add(3, "一".to_string());
    spoken.add(3, "二".to_string());
    assert_eq!(spoken.of(3), ["一", "二"]);
    assert!(spoken.of(4).is_empty(), "别的回合的不给");
    spoken.add(4, "三".to_string());
    assert_eq!(spoken.of(4), ["三"], "换了回合就清");
    assert!(spoken.of(3).is_empty());
}
