//! 加值项和走哪条路（`chat.md` 第三条「守着它的」）：每一种成立和不成立、几个同时成立加分相加、窗口两头、续聊只认最近
//! 一轮回的人、@ 了别人和引用别人的不算续聊、只有表情的不算刚说过话、回谁是 `None` 的；抽样；主触发的先后；四条路。

use crate::{Flag, Standing};

use super::test_support::{OTHER, SECOND, SENDER, chatty, facts, hits, mills, now, reply};
use super::{Chatty, Conditions, Hit, Kind, Route, bonuses, conditions, lifts, route};

/// 成立了的种类，照插槽的先后。
fn kinds(conditions: &Conditions) -> Vec<Kind> {
    conditions.hits.iter().map(|hit| hit.kind).collect()
}

/// 加分之和，到千分之几。
fn bonus(conditions: &Conditions) -> i64 {
    mills(conditions.hits.iter().map(|hit| hit.bonus).sum())
}

#[test]
fn builtin_slots_in_order() {
    let names: Vec<_> = bonuses()
        .iter()
        .map(|bonus| bonus.name().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "direct",
            "continuation",
            "after_speaking",
            "moderation",
            "probability"
        ]
    );
    let names: Vec<_> = lifts().iter().map(|lift| lift.name().to_string()).collect();
    assert_eq!(names, ["restraint"]);
}

#[test]
fn nothing_holds() {
    let got = hits(&facts(), &[], &[], &chatty());
    assert_eq!(got, Conditions::default());
    assert_eq!(got.primary(), None);
}

#[test]
fn direct_holds_when_addressed() {
    let mut msg = facts();
    msg.addressed = true;
    let got = hits(&msg, &[], &[], &chatty());
    assert_eq!(
        got.hits,
        [Hit {
            kind: Kind::Direct,
            bonus: 0.3
        }]
    );
}

#[test]
fn continuation_and_after_speaking_add_up() {
    // 她 5 秒前回了这个人：续聊、刚说过话都成立，加分相加。
    let got = hits(&facts(), &[], &[reply(5 * SECOND, Some(SENDER))], &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
    assert_eq!(bonus(&got), 200);
    // 再冲她来、再插违规旗：四样都在，加分不封顶。
    let mut msg = facts();
    msg.addressed = true;
    let mut big = chatty();
    big.direct = 0.9;
    let got = hits(
        &msg,
        &[Flag::Moderation],
        &[reply(5 * SECOND, Some(SENDER))],
        &big,
    );
    assert_eq!(
        kinds(&got),
        [
            Kind::Direct,
            Kind::Continuation,
            Kind::AfterSpeaking,
            Kind::Moderation
        ]
    );
    assert_eq!(bonus(&got), 1100);
}

/// 她在此刻之前 `ago` 毫秒回了发的人一轮，这条消息成立了哪些。
fn after_reply(ago: i64) -> Vec<Kind> {
    kinds(&hits(&facts(), &[], &[reply(ago, Some(SENDER))], &chatty()))
}

#[test]
fn windows_are_open_on_the_far_side() {
    assert_eq!(
        after_reply(0),
        [Kind::Continuation, Kind::AfterSpeaking],
        "此刻的算"
    );
    assert_eq!(
        after_reply(15 * SECOND - 1),
        [Kind::Continuation, Kind::AfterSpeaking]
    );
    assert_eq!(
        after_reply(15 * SECOND),
        [Kind::AfterSpeaking],
        "正好 15 秒以前的不算续聊"
    );
    assert_eq!(after_reply(30 * SECOND - 1), [Kind::AfterSpeaking]);
    assert_eq!(after_reply(30 * SECOND), [], "正好 30 秒以前的不算刚说过话");
    assert_eq!(after_reply(-1), [], "晚于此刻的不算");
}

#[test]
fn continuation_follows_only_the_latest_reply() {
    // 她先回了这个人，后来又回了别人：最近一轮回的不是他，不算续聊。
    let replies = [
        reply(2 * SECOND, Some(OTHER)),
        reply(5 * SECOND, Some(SENDER)),
    ];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::AfterSpeaking]);
    // 先后不要紧：倒过来交进来也一样。
    let reversed = [replies[1].clone(), replies[0].clone()];
    assert_eq!(hits(&facts(), &[], &reversed, &chatty()), got);
    // 晚于此刻的那一轮不算最近的：此刻之前最近的一轮回的是他。
    let replies = [reply(-SECOND, Some(OTHER)), reply(5 * SECOND, Some(SENDER))];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
}

#[test]
fn mentions_or_quotes_of_others_break_continuation() {
    let replies = [reply(5 * SECOND, Some(SENDER))];
    let mut mentions = facts();
    mentions.mentions_others = true;
    assert_eq!(
        kinds(&hits(&mentions, &[], &replies, &chatty())),
        [Kind::AfterSpeaking]
    );
    let mut quotes = facts();
    quotes.quotes_other = true;
    assert_eq!(
        kinds(&hits(&quotes, &[], &replies, &chatty())),
        [Kind::AfterSpeaking]
    );
}

#[test]
fn reply_to_nobody_is_not_a_continuation() {
    let got = hits(&facts(), &[], &[reply(5 * SECOND, None)], &chatty());
    assert_eq!(kinds(&got), [Kind::AfterSpeaking]);
}

#[test]
fn textless_is_not_after_speaking() {
    let mut sticker = facts();
    sticker.textless = true;
    let got = hits(&sticker, &[], &[reply(5 * SECOND, Some(OTHER))], &chatty());
    assert_eq!(kinds(&got), []);
    // 续聊不看有没有字。
    let got = hits(&sticker, &[], &[reply(5 * SECOND, Some(SENDER))], &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation]);
}

#[test]
fn moderation_flag_adds_nothing() {
    let got = hits(&facts(), &[Flag::Moderation], &[], &chatty());
    assert_eq!(
        got.hits,
        [Hit {
            kind: Kind::Moderation,
            bonus: 0.0
        }]
    );
}

/// 抽样的千分比是 `probability`，这一条消息（场所 `venue`、消息 `msg`）抽中没有。别的条件都不成立。
fn drawn(venue: &str, msg: &str, probability: u16) -> bool {
    let mut message = facts();
    message.venue = venue.to_string();
    message.msg = msg.to_string();
    let params = Chatty {
        probability,
        ..chatty()
    };
    let got = hits(&message, &[], &[], &params);
    match kinds(&got).as_slice() {
        [] => false,
        [Kind::Probability] => {
            assert_eq!(bonus(&got), 0, "抽样加 0 分");
            true
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn sampling_seed_is_pinned() {
    // 种子写死：SHA-256("qq:group:123456\n1001") 的前 8 个字节大端是 x，x × 1000 ÷ 2⁶⁴ 取整是 955；
    // "…\n1002" 的是 914。小于千分比才中，正好相等不中。数是拿 Python 的 hashlib 另算的。
    assert!(!drawn("qq:group:123456", "1001", 955));
    assert!(drawn("qq:group:123456", "1001", 956));
    assert!(!drawn("qq:group:123456", "1002", 914));
    assert!(drawn("qq:group:123456", "1002", 915));
}

#[test]
fn sampling_is_repeatable_and_bounded() {
    for msg in 0..200 {
        let msg = msg.to_string();
        assert!(!drawn("qq:group:123456", &msg, 0), "千分比 0 永不中");
        assert!(drawn("qq:group:123456", &msg, 1000), "千分比 1000 必中");
        assert_eq!(
            drawn("qq:group:1", &msg, 500),
            drawn("qq:group:1", &msg, 500)
        );
    }
    // 200 条里抽中的大约一半：种子真看了消息编号。
    let hit = (0..200)
        .filter(|msg| drawn("qq:group:1", &msg.to_string(), 500))
        .count();
    assert!((60..140).contains(&hit), "{hit}");
}

#[test]
fn sampling_skips_media_only_and_other_hits() {
    let always = Chatty {
        probability: 1000,
        ..chatty()
    };
    let mut picture = facts();
    picture.media_only = true;
    assert_eq!(kinds(&hits(&picture, &[], &[], &always)), []);
    // 只有表情的照样抽。
    let mut sticker = facts();
    sticker.textless = true;
    assert_eq!(
        kinds(&hits(&sticker, &[], &[], &always)),
        [Kind::Probability]
    );
    // 别的条件成立时不抽，违规旗也算。
    let mut msg = facts();
    msg.addressed = true;
    assert_eq!(kinds(&hits(&msg, &[], &[], &always)), [Kind::Direct]);
    assert_eq!(
        kinds(&hits(&facts(), &[Flag::Moderation], &[], &always)),
        [Kind::Moderation]
    );
    let replies = [reply(20 * SECOND, None)];
    assert_eq!(
        kinds(&hits(&facts(), &[], &replies, &always)),
        [Kind::AfterSpeaking]
    );
}

/// 照这些种类造一个条件集合，加分都是 0。
fn of(kinds: &[Kind]) -> Conditions {
    Conditions {
        hits: kinds.iter().map(|&kind| Hit { kind, bonus: 0.0 }).collect(),
    }
}

#[test]
fn primary_order() {
    use Kind::{AfterSpeaking, Continuation, Direct, Moderation, Probability};
    assert_eq!(of(&[Moderation, Direct]).primary(), Some(Direct));
    assert_eq!(
        of(&[AfterSpeaking, Continuation]).primary(),
        Some(Continuation)
    );
    assert_eq!(
        of(&[Moderation, AfterSpeaking]).primary(),
        Some(AfterSpeaking)
    );
    // 抽样排在违规旗前面，和插槽的先后不一样。
    assert_eq!(of(&[Moderation, Probability]).primary(), Some(Probability));
    assert_eq!(of(&[Moderation]).primary(), Some(Moderation));
}

#[test]
fn routes() {
    use Kind::{AfterSpeaking, Direct, Moderation, Probability};
    assert_eq!(route(&of(&[]), Standing::Owner), Route::Record);
    assert_eq!(route(&of(&[Direct]), Standing::Owner), Route::Commit);
    assert_eq!(
        route(&of(&[Direct, Moderation]), Standing::Owner),
        Route::Commit
    );
    assert_eq!(
        route(&of(&[Direct]), Standing::Trusted),
        Route::Judge,
        "自己人的 @ 也过判官"
    );
    assert_eq!(route(&of(&[Direct]), Standing::Member), Route::Judge);
    assert_eq!(
        route(&of(&[AfterSpeaking]), Standing::Owner),
        Route::Judge,
        "主人没 @ 照样过判官"
    );
    assert_eq!(
        route(&of(&[Moderation]), Standing::Member),
        Route::ModerationOnly
    );
    assert_eq!(
        route(&of(&[Moderation]), Standing::Owner),
        Route::ModerationOnly
    );
    assert_eq!(route(&of(&[Probability]), Standing::Member), Route::Judge);
    assert_eq!(
        route(&of(&[AfterSpeaking, Moderation]), Standing::Member),
        Route::Judge
    );
}

#[test]
fn owner_addressing_commits_end_to_end() {
    let mut msg = facts();
    msg.standing = Standing::Owner;
    msg.addressed = true;
    let got = conditions(&msg, &[], &[], now(), &chatty());
    assert_eq!(route(&got, msg.standing), Route::Commit);
    assert_eq!(got.primary(), Some(Kind::Direct));
}
