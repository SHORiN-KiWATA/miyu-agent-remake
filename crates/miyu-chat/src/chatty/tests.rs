//! 加值项和走哪条路（`chat.md` 第三条「守着它的」）：每一种成立和不成立、几个同时成立加分相加、窗口两头、续聊只认最近
//! 一轮回的人、@ 了别人和引用别人的不算续聊、只有表情的不算刚说过话、回谁是 `None` 的；抽样；插槽的先后（比结果）；
//! 主触发的先后；四条路。

use miyu_kernel::id::VenueId;

use crate::{Flag, Standing};

use super::test_support::{OTHER, SECOND, SENDER, chatty, facts, hits, mills, now, reply, seq};
use super::{Chatty, Conditions, Hit, Kind, Route, conditions, route};

/// 成立了的种类，照插槽的先后。
fn kinds(conditions: &Conditions) -> Vec<Kind> {
    conditions.hits.iter().map(|hit| hit.kind).collect()
}

/// 加分之和，到千分之几。
fn bonus(conditions: &Conditions) -> i64 {
    mills(conditions.hits.iter().map(|hit| hit.bonus).sum())
}

#[test]
fn slots_keep_their_order() {
    // 排先后比的是结果（第二条施工时定的第 14 条）。四样同时成立：照插槽的先后排，调换相邻的两个就红。
    let mut msg = facts();
    msg.said.addressed = true;
    let replies = [reply(5 * SECOND, &[SENDER])];
    let got = hits(&msg, &[Flag::Moderation], &replies, &chatty());
    assert_eq!(
        kinds(&got),
        [
            Kind::Direct,
            Kind::Continuation,
            Kind::AfterSpeaking,
            Kind::Moderation
        ]
    );
    // 抽样排最后、看得到前面的：违规旗成立了就不抽。排到违规旗前面，就会先抽中。
    let always = Chatty {
        probability: 1000,
        ..chatty()
    };
    let got = hits(&facts(), &[Flag::Moderation], &[], &always);
    assert_eq!(kinds(&got), [Kind::Moderation]);
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
    msg.said.addressed = true;
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
    let got = hits(&facts(), &[], &[reply(5 * SECOND, &[SENDER])], &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
    assert_eq!(bonus(&got), 200);
    // 再冲她来、再插违规旗：四样都在，加分不封顶。
    let mut msg = facts();
    msg.said.addressed = true;
    let mut big = chatty();
    big.direct = 0.9;
    let got = hits(
        &msg,
        &[Flag::Moderation],
        &[reply(5 * SECOND, &[SENDER])],
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
    kinds(&hits(&facts(), &[], &[reply(ago, &[SENDER])], &chatty()))
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
    let replies = [reply(2 * SECOND, &[OTHER]), reply(5 * SECOND, &[SENDER])];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::AfterSpeaking]);
    // 先后不要紧：倒过来交进来也一样。
    let reversed = [replies[1].clone(), replies[0].clone()];
    assert_eq!(hits(&facts(), &[], &reversed, &chatty()), got);
    // 晚于此刻的那一轮不算最近的：此刻之前最近的一轮回的是他。
    let replies = [reply(-SECOND, &[OTHER]), reply(5 * SECOND, &[SENDER])];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
}

#[test]
fn mentions_or_quotes_of_others_break_continuation() {
    let replies = [reply(5 * SECOND, &[SENDER])];
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
fn continuation_counts_anyone_the_round_answered() {
    // 最近一轮回了两个人：两个人接着说都算续聊，没回到的第三个人不算（第七条第 4 条，`Reply::to` 是列表）。
    let replies = [reply(5 * SECOND, &[OTHER, SENDER])];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
    let replies = [reply(5 * SECOND, &[SENDER, OTHER])];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::Continuation, Kind::AfterSpeaking]);
    let replies = [reply(5 * SECOND, &[OTHER, "qq:10004"])];
    let got = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(kinds(&got), [Kind::AfterSpeaking]);
}

#[test]
fn reply_to_nobody_is_not_a_continuation() {
    let got = hits(&facts(), &[], &[reply(5 * SECOND, &[])], &chatty());
    assert_eq!(kinds(&got), [Kind::AfterSpeaking]);
}

#[test]
fn textless_is_not_after_speaking() {
    let mut sticker = facts();
    sticker.textless = true;
    let got = hits(&sticker, &[], &[reply(5 * SECOND, &[OTHER])], &chatty());
    assert_eq!(kinds(&got), []);
    // 续聊不看有没有字。
    let got = hits(&sticker, &[], &[reply(5 * SECOND, &[SENDER])], &chatty());
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

/// 抽样的千分比是 `probability`，这一条消息（场所 `venue`、序号 `msg`）抽中没有。别的条件都不成立。
fn drawn(venue: &str, msg: u64, probability: u16) -> bool {
    let mut message = facts();
    message.venue = VenueId::parse(venue).expect(venue);
    message.msg = seq(msg);
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
    // 种子写死（第七条第 1 条）：SHA-256("qq:group:123456\n7") 的前 8 个字节大端是 x，x × 1000 ÷ 2⁶⁴ 取整是 462；
    // SHA-256("qq:private:10002\n42") 的是 512。小于千分比才中，正好相等不中。数是拿 Python 的 hashlib 另算的：
    // `(int.from_bytes(hashlib.sha256(b"qq:group:123456\n7").digest()[:8], "big") * 1000) >> 64`。
    assert!(!drawn("qq:group:123456", 7, 462));
    assert!(drawn("qq:group:123456", 7, 463));
    assert!(!drawn("qq:private:10002", 42, 512));
    assert!(drawn("qq:private:10002", 42, 513));
}

#[test]
fn sampling_is_repeatable_and_bounded() {
    for msg in 1..=200 {
        assert!(!drawn("qq:group:123456", msg, 0), "千分比 0 永不中");
        assert!(drawn("qq:group:123456", msg, 1000), "千分比 1000 必中");
        assert_eq!(drawn("qq:group:1", msg, 500), drawn("qq:group:1", msg, 500));
    }
    // 200 条里抽中的大约一半：种子真看了序号。
    let hit = (1..=200)
        .filter(|&msg| drawn("qq:group:1", msg, 500))
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
    msg.said.addressed = true;
    assert_eq!(kinds(&hits(&msg, &[], &[], &always)), [Kind::Direct]);
    assert_eq!(
        kinds(&hits(&facts(), &[Flag::Moderation], &[], &always)),
        [Kind::Moderation]
    );
    let replies = [reply(20 * SECOND, &[])];
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
    msg.said.standing = Standing::Owner;
    msg.said.addressed = true;
    let got = conditions(&msg, &[], &[], now(), &chatty());
    assert_eq!(route(&got, msg.said.standing), Route::Commit);
    assert_eq!(got.primary(), Some(Kind::Direct));
}
