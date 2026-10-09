//! 线路规程（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 10 条）：四种的写法读得回来，没设的、认不出的照 `chatty`；
//! 各留下哪些条件、看不看顶替、走哪条路。

use miyu_chat::{Conditions, Hit, Kind, Route, Standing};

use super::Discipline;

/// 五种条件都成立的一份（照插槽的先后），抽样只是为了看留不留。
fn all() -> Conditions {
    let hit = |kind, bonus| Hit { kind, bonus };
    Conditions {
        hits: vec![
            hit(Kind::Direct, 0.3),
            hit(Kind::Continuation, 0.1),
            hit(Kind::AfterSpeaking, 0.1),
            hit(Kind::Moderation, 0.0),
            hit(Kind::Probability, 0.0),
        ],
    }
}

/// 留下的种类。
fn kinds(conditions: &Conditions) -> Vec<Kind> {
    conditions.hits.iter().map(|hit| hit.kind).collect()
}

/// 只有这几种的一份。
fn only(kinds: &[Kind]) -> Conditions {
    Conditions {
        hits: kinds
            .iter()
            .map(|kind| Hit {
                kind: *kind,
                bonus: 0.0,
            })
            .collect(),
    }
}

#[test]
fn the_four_are_read_by_name_and_anything_else_is_chatty() {
    for discipline in [
        Discipline::Chatty,
        Discipline::WhenCalled,
        Discipline::Wake,
        Discipline::EveryMessage,
    ] {
        assert_eq!(Discipline::read(Some(discipline.name())), discipline);
    }
    assert_eq!(
        [
            Discipline::Chatty.name(),
            Discipline::WhenCalled.name(),
            Discipline::Wake.name(),
            Discipline::EveryMessage.name(),
        ],
        ["chatty", "when-called", "wake", "every-message"]
    );
    assert_eq!(
        Discipline::read(None),
        Discipline::Chatty,
        "没设的照 chatty"
    );
    assert_eq!(Discipline::read(Some("Chatty")), Discipline::Chatty);
    assert_eq!(Discipline::read(Some("")), Discipline::Chatty);
}

#[test]
fn each_keeps_its_own_conditions() {
    use Kind::{AfterSpeaking, Continuation, Direct, Moderation, Probability};
    assert_eq!(
        kinds(&Discipline::Chatty.keep(all(), false)),
        [Direct, Continuation, AfterSpeaking, Moderation, Probability]
    );
    assert_eq!(
        kinds(&Discipline::Chatty.keep(all(), true)),
        [Direct, Continuation, AfterSpeaking, Moderation],
        "额度满了不抽样"
    );
    for discipline in [Discipline::WhenCalled, Discipline::Wake] {
        for full in [false, true] {
            assert_eq!(
                kinds(&discipline.keep(all(), full)),
                [Direct, Continuation, Moderation],
                "{discipline:?}"
            );
        }
    }
    for full in [false, true] {
        assert_eq!(
            kinds(&Discipline::EveryMessage.keep(all(), full)),
            [Direct, Continuation, AfterSpeaking, Moderation]
        );
    }
    assert_eq!(
        Discipline::Chatty.keep(all(), false).hits[0].bonus,
        0.3,
        "加分照原样"
    );
}

#[test]
fn only_chatty_and_when_called_look_for_a_follow_up() {
    assert!(Discipline::Chatty.supersedes());
    assert!(Discipline::WhenCalled.supersedes());
    assert!(!Discipline::Wake.supersedes());
    assert!(!Discipline::EveryMessage.supersedes());
}

#[test]
fn each_goes_its_own_way() {
    let none = only(&[]);
    let direct = only(&[Kind::Direct]);
    let flagged = only(&[Kind::Moderation]);
    // chatty 照群聊内核：没条件只记下，主人冲她来开一轮，只有违规旗只查违规，别的问判官。
    let chatty = Discipline::Chatty;
    assert_eq!(chatty.route(&none, Standing::Member, false), Route::Record);
    assert_eq!(chatty.route(&direct, Standing::Owner, false), Route::Commit);
    assert_eq!(
        chatty.route(&direct, Standing::Trusted, false),
        Route::Judge
    );
    assert_eq!(
        chatty.route(&flagged, Standing::Member, false),
        Route::ModerationOnly
    );
    // when-called、wake：只有违规旗的判官只查违规；和别的条件一起的、别的有条件的开一轮；没有的只记下。不打分。
    let both = only(&[Kind::Direct, Kind::Moderation]);
    let continued = only(&[Kind::Continuation]);
    for discipline in [Discipline::WhenCalled, Discipline::Wake] {
        assert_eq!(
            discipline.route(&none, Standing::Member, false),
            Route::Record
        );
        assert_eq!(
            discipline.route(&flagged, Standing::Member, false),
            Route::ModerationOnly,
            "{discipline:?}"
        );
        for conditions in [&direct, &both, &continued] {
            assert_eq!(
                discipline.route(conditions, Standing::Member, false),
                Route::Commit,
                "{discipline:?}"
            );
        }
    }
    // every-message：有字的都开一轮，没条件也是；只有带的东西的只记下。
    let every = Discipline::EveryMessage;
    assert_eq!(every.route(&none, Standing::Member, false), Route::Commit);
    assert_eq!(
        every.route(&flagged, Standing::Member, false),
        Route::Commit,
        "违规旗不改它：有字的本来就回"
    );
    assert_eq!(every.route(&direct, Standing::Member, true), Route::Record);
}
