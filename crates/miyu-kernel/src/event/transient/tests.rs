//! 瞬时事件的测试：图纸上的那一行，代码里造出来写出去一字不差；三种增量各自的写法；
//! 不属于回合、没有命令的，那两格不写。

use super::*;
use crate::id::{ModelName, ProviderId};
use crate::origin::Model;

/// 03 第五节图纸上的那一行。
const DRAWING: &str = r#"{"at":"2026-09-25T07:04:06.210Z","kind":"model.delta","turn":42,"by":{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"},"cause":"cmd-7f3a","body":{"seen":44,"index":0,"text":"我先"}}"#;

fn delta(index: usize, piece: Piece) -> Transient {
    Transient {
        at: Timestamp::parse("2026-09-25T07:04:06.210Z").unwrap(),
        turn: Some(TurnId::new(Seq::new(42).unwrap())),
        by: By::Model(Model {
            endpoint: ProviderId::parse("deepseek").unwrap(),
            model: ModelName::parse("deepseek-v4").unwrap(),
        }),
        cause: Some(CommandId::parse("cmd-7f3a").unwrap()),
        body: TransientBody::ModelDelta(ModelDelta {
            seen: Seq::new(44).unwrap(),
            index,
            piece,
        }),
    }
}

/// 写出去以后 `body` 那一段。
fn body_of(transient: &Transient) -> String {
    let line = transient.to_line();
    let start = line.find(r#""body":"#).unwrap() + r#""body":"#.len();
    line[start..line.len() - 1].to_string()
}

#[test]
fn the_line_from_the_drawing_is_written_exactly() {
    assert_eq!(delta(0, Piece::Text("我先".to_string())).to_line(), DRAWING);
}

#[test]
fn each_piece_is_written_its_own_way() {
    for (piece, body) in [
        (
            Piece::Start(Kind::Text),
            r#"{"seen":44,"index":1,"start":"text"}"#,
        ),
        (
            Piece::Start(Kind::Reasoning),
            r#"{"seen":44,"index":1,"start":"reasoning"}"#,
        ),
        (
            Piece::Start(Kind::ToolCall {
                name: "read".to_string(),
            }),
            r#"{"seen":44,"index":1,"start":"tool_call","name":"read"}"#,
        ),
        (Piece::End, r#"{"seen":44,"index":1,"end":true}"#),
    ] {
        assert_eq!(body_of(&delta(1, piece)), body);
    }
}

#[test]
fn missing_turn_and_cause_are_not_written() {
    let mut transient = delta(0, Piece::End);
    transient.turn = None;
    transient.cause = None;
    let line = transient.to_line();
    assert!(!line.contains("turn") && !line.contains("cause"), "{line}");
}
