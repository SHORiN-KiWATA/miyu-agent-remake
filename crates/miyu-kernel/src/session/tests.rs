//! 会话的测试：造会话；发消息；空消息；同一个编号落盘前后再来；拒绝过的再来；落盘到一半；
//! 落盘超出追加过的；只记最近 1024 个；随机一串输入，每收到一次命令恰好回应一次，接受的
//! 回应都在它的事件落盘以后。

use std::collections::{BTreeMap, BTreeSet};

use super::recent::CAPACITY;
use super::*;
use crate::block::{Block, Text};

const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

fn id(n: u64) -> CommandId {
    CommandId::parse(&format!("cmd-{n}")).unwrap()
}

fn alice() -> By {
    serde_json::from_str(r#"{"kind":"person","account":"alice"}"#).unwrap()
}

fn at(second: u64) -> Timestamp {
    Timestamp::parse(&format!("2026-09-25T07:00:{second:02}.000Z")).unwrap()
}

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

fn seqs(numbers: &[u64]) -> Vec<Seq> {
    numbers.iter().map(|&n| seq(n)).collect()
}

/// 编号是 `n` 的命令：alice 发一条消息，内容是 `words`；`words` 是空的就一块内容都没有。
fn send(n: u64, words: &str) -> Input {
    let blocks = if words.is_empty() {
        Vec::new()
    } else {
        vec![Block::Text(Text {
            text: words.to_string(),
        })]
    };
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Send { blocks },
    })
}

fn stored(upto: u64) -> Input {
    Input::Stored { upto: seq(upto) }
}

fn accepted_reply(n: u64, events: &[u64]) -> Action {
    Action::Reply {
        id: id(n),
        outcome: Outcome::Accepted {
            events: seqs(events),
        },
    }
}

/// 一个造好、第 1 条已经落了盘的会话。造会话的命令编号是 0。
fn session() -> Session {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let (mut session, _) = Session::create(id(0), alice(), at(0), created);
    session.handle(stored(1));
    session
}

/// 追加动作里的事件的序号。
fn appended(actions: &[Action]) -> Vec<Seq> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Append(events) => Some(events.iter().map(|event| event.seq)),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn creating_a_session_appends_session_created_and_replies_once_stored() {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let (mut session, actions) = Session::create(id(0), alice(), at(0), created);
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("造会话应该只追加一条：{actions:?}");
    };
    let [event] = events.as_slice() else {
        panic!("造会话应该只追加一条：{events:?}");
    };
    assert_eq!(event.seq, seq(1));
    assert_eq!(event.cause, Some(id(0)));
    assert!(matches!(event.body, Body::SessionCreated(_)));
    let actions = session.handle(stored(1));
    assert_eq!(
        actions,
        [Action::Push(vec![event.clone()]), accepted_reply(0, &[1])]
    );
}

#[test]
fn a_message_is_appended_and_answered_once_stored() {
    let mut session = session();
    let actions = session.handle(send(1, "看看 src 目录"));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("发消息应该只追加，不回应：{actions:?}");
    };
    let event = &events[0];
    assert_eq!(event.seq, seq(2));
    assert_eq!(event.at, at(1));
    assert_eq!(event.by, alice());
    assert_eq!(event.cause, Some(id(1)));
    assert_eq!(event.turn, None);
    assert!(matches!(&event.body, Body::MessageUser(message) if message.blocks.len() == 1));
    let actions = session.handle(stored(2));
    assert_eq!(
        actions,
        [Action::Push(events.clone()), accepted_reply(1, &[2])]
    );
}

#[test]
fn an_empty_message_is_rejected_on_the_spot() {
    let mut session = session();
    let actions = session.handle(send(1, ""));
    assert_eq!(
        actions,
        [Action::Reply {
            id: id(1),
            outcome: Outcome::Rejected {
                reason: Reason::EmptyMessage
            },
        }]
    );
    assert_eq!(Reason::EmptyMessage.code(), "empty_message");
    // 什么都没追加：下一条消息还是 2 号。
    assert_eq!(appended(&session.handle(send(2, "hi"))), seqs(&[2]));
}

#[test]
fn a_command_sent_again_before_it_is_stored_is_answered_twice_after() {
    let mut session = session();
    session.handle(send(1, "hi"));
    assert!(session.handle(send(1, "hi")).is_empty());
    let actions = session.handle(stored(2));
    let replies: Vec<&Action> = actions
        .iter()
        .filter(|action| matches!(action, Action::Reply { .. }))
        .collect();
    assert_eq!(
        replies,
        [&accepted_reply(1, &[2]), &accepted_reply(1, &[2])]
    );
}

#[test]
fn a_command_sent_again_after_it_is_stored_is_answered_on_the_spot() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(2));
    assert_eq!(session.handle(send(1, "hi")), [accepted_reply(1, &[2])]);
}

#[test]
fn a_rejected_command_is_judged_again() {
    let mut session = session();
    let rejected = |actions: Vec<Action>| {
        matches!(
            actions.as_slice(),
            [Action::Reply {
                outcome: Outcome::Rejected { .. },
                ..
            }]
        )
    };
    assert!(rejected(session.handle(send(1, ""))));
    assert!(rejected(session.handle(send(1, ""))));
    assert_eq!(appended(&session.handle(send(1, "这回有字了"))), seqs(&[2]));
}

#[test]
fn a_partial_store_answers_only_the_commands_fully_stored() {
    let mut session = session();
    session.handle(send(1, "one"));
    session.handle(send(2, "two"));
    let first = session.handle(stored(2));
    assert!(
        matches!(first.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(1, &[2]))
    );
    let second = session.handle(stored(3));
    assert!(
        matches!(second.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(2, &[3]))
    );
}

#[test]
fn a_store_beyond_what_was_appended_counts_only_what_was_appended() {
    let mut session = session();
    session.handle(send(1, "one"));
    let actions = session.handle(stored(100));
    assert_eq!(
        actions.last(),
        Some(&accepted_reply(1, &[2])),
        "追加过的都落了盘"
    );
    // 之后追加的，要等它自己落了盘。
    session.handle(send(2, "two"));
    assert!(session.handle(send(2, "two")).is_empty());
    assert!(
        session.handle(stored(2)).is_empty(),
        "不比上一次往后的，什么都不做"
    );
    let actions = session.handle(stored(3));
    assert_eq!(
        actions
            .iter()
            .filter(|action| matches!(action, Action::Reply { .. }))
            .count(),
        2
    );
}

#[test]
fn only_the_latest_commands_are_remembered() {
    let mut session = session();
    for n in 1..=CAPACITY as u64 + 1 {
        session.handle(send(n, "hi"));
    }
    let last = CAPACITY as u64 + 2;
    session.handle(stored(last));
    // 第 2 个还记得，照上一次回应；第 1 个已经忘了，当新命令。
    assert_eq!(session.handle(send(2, "hi")), [accepted_reply(2, &[3])]);
    assert_eq!(appended(&session.handle(send(1, "hi"))), seqs(&[last + 1]));
}

/// SplitMix64：随机一串输入用。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn random_inputs_get_one_reply_per_command_after_their_events_are_stored() {
    for seed in 0..300 {
        let mut rng = Rng(seed);
        let mut session = session();
        let mut last = 1;
        let mut next_id = 1;
        // 每个编号收到了几次、回应了几次；推送过的事件。
        let mut received: BTreeMap<CommandId, usize> = BTreeMap::new();
        let mut replied: BTreeMap<CommandId, usize> = BTreeMap::new();
        let mut pushed: BTreeSet<Seq> = BTreeSet::new();
        let mut check = |actions: Vec<Action>, last: &mut u64| {
            for action in actions {
                match action {
                    Action::Append(events) => {
                        for event in events {
                            assert_eq!(event.seq.get(), *last + 1, "种子 {seed}：序号要连着");
                            *last += 1;
                        }
                    }
                    Action::Push(events) => pushed.extend(events.iter().map(|event| event.seq)),
                    Action::Reply { id, outcome } => {
                        if let Outcome::Accepted { events } = &outcome {
                            assert!(
                                events.iter().all(|event| pushed.contains(event)),
                                "种子 {seed}：{id} 的事件还没落盘就回应了"
                            );
                        }
                        *replied.entry(id).or_default() += 1;
                    }
                }
            }
        };
        for _ in 0..40 {
            let input = match rng.below(10) {
                0..=4 => {
                    next_id += 1;
                    send(next_id, "hi")
                }
                5 | 6 => send(1 + rng.below(next_id), "hi"),
                7 => {
                    next_id += 1;
                    send(next_id, "")
                }
                _ => stored(1 + rng.below(last)),
            };
            if let Input::Command(command) = &input {
                *received.entry(command.id.clone()).or_default() += 1;
            }
            let actions = session.handle(input);
            check(actions, &mut last);
        }
        let actions = session.handle(stored(last));
        check(actions, &mut last);
        assert_eq!(
            received, replied,
            "种子 {seed}：每收到一次命令要恰好回应一次"
        );
    }
}
