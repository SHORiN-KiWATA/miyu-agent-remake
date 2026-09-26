//! 累积器的测试：三种块各自拼对；调用编号；空块；交错的字；被打断；对不上的增量；
//! 随机切片：同一份回复随机切成增量、随机交错着送进去，拼出来都一样。

use super::*;

fn private(json: &str) -> Private {
    serde_json::from_str(json).unwrap()
}

fn signature() -> Private {
    private(r#"{"driver":"anthropic","data":{"signature":"c2ln"}}"#)
}

fn provider_id(id: &str) -> Private {
    private(&format!(r#"{{"driver":"openai","data":{{"id":"{id}"}}}}"#))
}

fn reply() -> Seq {
    Seq::new(45).unwrap()
}

fn call(k: u32) -> CallId {
    CallId::new(reply(), k).unwrap()
}

fn read() -> Kind {
    Kind::ToolCall {
        name: "read".to_string(),
    }
}

fn start(index: usize, kind: Kind) -> Delta {
    Delta::Start { index, kind }
}

fn text(index: usize, text: &str) -> Delta {
    Delta::Text {
        index,
        text: text.to_string(),
    }
}

fn end(index: usize) -> Delta {
    Delta::End { index }
}

/// 把这些增量依次送进一个新的累积器。
fn fed(deltas: Vec<Delta>) -> Accumulator {
    let mut accumulator = Accumulator::default();
    for delta in deltas {
        accumulator.apply(delta).unwrap();
    }
    accumulator
}

#[test]
fn each_kind_of_block_is_put_together() {
    let blocks = fed(vec![
        start(0, Kind::Text),
        text(0, "我先"),
        text(0, "看一下目录。"),
        end(0),
        start(1, Kind::Reasoning),
        text(1, "用户想看 src。"),
        Delta::Private {
            index: 1,
            private: signature(),
        },
        end(1),
        start(2, read()),
        Delta::Private {
            index: 2,
            private: provider_id("toolu_1"),
        },
        text(2, r#"{"path":"#),
        text(2, r#""src"}"#),
        end(2),
    ])
    .finish(reply());
    assert_eq!(
        blocks,
        [
            Block::Text(Text {
                text: "我先看一下目录。".to_string()
            }),
            Block::Reasoning(Reasoning {
                text: "用户想看 src。".to_string(),
                private: Some(signature()),
            }),
            Block::ToolCall(ToolCall {
                call_id: call(1),
                name: "read".to_string(),
                args: r#"{"path":"src"}"#.to_string(),
                private: Some(provider_id("toolu_1")),
            }),
        ]
    );
}

#[test]
fn calls_are_numbered_after_the_reply_one_by_one() {
    let blocks = fed(vec![
        start(0, read()),
        text(0, "{}"),
        end(0),
        start(1, Kind::Text),
        text(1, "然后"),
        end(1),
        start(2, read()),
        text(2, "{}"),
        end(2),
    ])
    .finish(reply());
    let ids: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolCall(call) => Some(call.call_id.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["call_45_1", "call_45_2"]);
}

#[test]
fn empty_blocks_are_dropped_but_a_signed_empty_reasoning_stays() {
    let blocks = fed(vec![
        start(0, Kind::Text),
        end(0),
        start(1, Kind::Reasoning),
        end(1),
        start(2, Kind::Reasoning),
        Delta::Private {
            index: 2,
            private: signature(),
        },
        end(2),
        start(3, Kind::Text),
        text(3, "好。"),
        end(3),
    ])
    .finish(reply());
    assert_eq!(
        blocks,
        [
            Block::Reasoning(Reasoning {
                text: String::new(),
                private: Some(signature()),
            }),
            Block::Text(Text {
                text: "好。".to_string()
            }),
        ]
    );
}

#[test]
fn interleaved_text_is_joined_per_block() {
    let blocks = fed(vec![
        start(0, read()),
        start(1, read()),
        text(0, r#"{"path""#),
        text(1, r#"{"path""#),
        text(1, r#":"b"}"#),
        text(0, r#":"a"}"#),
        end(1),
        end(0),
    ])
    .finish(reply());
    let args: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolCall(call) => Some(call.args.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(args, [r#"{"path":"a"}"#, r#"{"path":"b"}"#]);
}

#[test]
fn a_cut_off_reply_keeps_what_came_and_only_the_calls_that_ended() {
    let blocks = fed(vec![
        start(0, Kind::Text),
        text(0, "我先改"),
        start(1, read()),
        text(1, r#"{"path":"src/ma"#),
        start(2, read()),
        text(2, r#"{"path":"README.md"}"#),
        end(2),
    ])
    .cut_off(reply());
    assert_eq!(
        blocks,
        [
            Block::Text(Text {
                text: "我先改".to_string()
            }),
            Block::ToolCall(ToolCall {
                call_id: call(1),
                name: "read".to_string(),
                args: r#"{"path":"README.md"}"#.to_string(),
                private: None,
            }),
        ]
    );
}

#[test]
fn mismatched_deltas_are_driver_errors() {
    let cases: Vec<(Vec<Delta>, Delta, &str)> = vec![
        (vec![], text(0, "hi"), "还没开始"),
        (vec![], start(1, Kind::Text), "跳过了编号"),
        (
            vec![start(0, Kind::Text)],
            start(0, Kind::Text),
            "已经开始过",
        ),
        (
            vec![start(0, Kind::Text), end(0)],
            text(0, "hi"),
            "已经收全了",
        ),
        (vec![start(0, Kind::Text), end(0)], end(0), "已经收全了"),
        (
            vec![start(0, Kind::Text)],
            Delta::Private {
                index: 0,
                private: signature(),
            },
            "正文块没有私有数据",
        ),
        (
            vec![
                start(0, Kind::Reasoning),
                Delta::Private {
                    index: 0,
                    private: signature(),
                },
            ],
            Delta::Private {
                index: 0,
                private: signature(),
            },
            "来了两次",
        ),
    ];
    for (before, delta, why) in cases {
        let mut accumulator = fed(before);
        let error = accumulator.apply(delta.clone()).unwrap_err();
        assert!(error.why.contains(why), "{delta:?}：{error}");
        assert!(error.to_string().contains("第 "), "{error}");
    }
}

/// SplitMix64：随机切片用。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// 一块要送进去的东西：种类、切好的几段字、私有数据。
struct Plan {
    kind: Kind,
    pieces: Vec<String>,
    private: Option<Private>,
}

/// 把一段字在随机的字符边界上切成一到四段，可能有空段。
fn slice(rng: &mut Rng, whole: &str) -> Vec<String> {
    let bounds: Vec<usize> = whole
        .char_indices()
        .map(|(at, _)| at)
        .chain([whole.len()])
        .collect();
    let mut cuts: Vec<usize> = (0..rng.below(4))
        .map(|_| bounds[rng.below(bounds.len())])
        .collect();
    cuts.sort_unstable();
    let mut pieces = Vec::new();
    let mut from = 0;
    for cut in cuts {
        pieces.push(whole[from..cut].to_string());
        from = cut;
    }
    pieces.push(whole[from..].to_string());
    pieces
}

/// 照计划随机交错着造增量：块一块接一块地开始；开始了的块，随机轮着送字、私有数据；
/// 字送完、私有数据也送了，才收全。
fn shuffled(rng: &mut Rng, plans: &[Plan]) -> Vec<Delta> {
    let mut sent = vec![0; plans.len()];
    let mut private_sent: Vec<bool> = plans.iter().map(|plan| plan.private.is_none()).collect();
    let mut ended = vec![false; plans.len()];
    let mut started = 0;
    let mut deltas = Vec::new();
    while ended.iter().any(|done| !done) {
        let mut choices = Vec::new();
        if started < plans.len() {
            choices.push(None);
        }
        choices.extend((0..started).filter(|&index| !ended[index]).map(Some));
        match choices[rng.below(choices.len())] {
            None => {
                deltas.push(start(started, plans[started].kind.clone()));
                started += 1;
            }
            Some(index) => {
                let plan = &plans[index];
                let text_left = sent[index] < plan.pieces.len();
                if !private_sent[index] && (!text_left || rng.below(2) == 0) {
                    deltas.push(Delta::Private {
                        index,
                        private: plan.private.clone().unwrap(),
                    });
                    private_sent[index] = true;
                } else if text_left {
                    deltas.push(text(index, &plan.pieces[sent[index]]));
                    sent[index] += 1;
                } else {
                    deltas.push(end(index));
                    ended[index] = true;
                }
            }
        }
    }
    deltas
}

#[test]
fn random_slices_give_the_same_reply() {
    let whole: Vec<(Kind, &str, Option<Private>)> = vec![
        (Kind::Text, "我先看一下目录，再读两个文件。", None),
        (Kind::Reasoning, "用户想看 src 目录。", Some(signature())),
        (read(), r#"{"path":"src"}"#, Some(provider_id("toolu_1"))),
        (
            read(),
            r#"{"path":"src/lib.rs"}"#,
            Some(provider_id("toolu_2")),
        ),
    ];
    let expected = {
        let plans: Vec<Plan> = whole
            .iter()
            .map(|(kind, text, private)| Plan {
                kind: kind.clone(),
                pieces: vec![text.to_string()],
                private: private.clone(),
            })
            .collect();
        let mut rng = Rng(0);
        fed(shuffled(&mut rng, &plans)).finish(reply())
    };
    assert_eq!(expected.len(), 4);
    for seed in 0..300 {
        let mut rng = Rng(seed);
        let plans: Vec<Plan> = whole
            .iter()
            .map(|(kind, text, private)| Plan {
                kind: kind.clone(),
                pieces: slice(&mut rng, text),
                private: private.clone(),
            })
            .collect();
        let deltas = shuffled(&mut rng, &plans);
        assert_eq!(fed(deltas).finish(reply()), expected, "种子 {seed}");
    }
}
