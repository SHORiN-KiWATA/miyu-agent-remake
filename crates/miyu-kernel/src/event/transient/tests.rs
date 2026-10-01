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

#[test]
fn tool_progress_is_written_with_its_call() {
    let call_id = CallId::parse("call_45_1").unwrap();
    let mut transient = delta(0, Piece::End);
    transient.by = By::Tool(crate::origin::Tool { call_id });
    transient.body = TransientBody::ToolProgress(ToolProgress {
        call_id,
        text: "lib.rs\n".to_string(),
    });
    let line = transient.to_line();
    assert!(line.contains(r#""kind":"tool.progress""#), "{line}");
    assert_eq!(
        body_of(&transient),
        r#"{"call_id":"call_45_1","text":"lib.rs\n"}"#
    );
}

/// 重试的状态：换了端点当场再来的多一格 `failover`，写在最后；不是的不写（施工 8-9）。
#[test]
fn failover_is_written_only_when_it_is_true() {
    let status = |failover: bool| {
        let mut transient = delta(0, Piece::End);
        transient.body = TransientBody::Status(Status {
            seen: Seq::new(44).unwrap(),
            retry: Retry {
                attempt: 1,
                limit: 5,
                wait_ms: 0,
                class: ErrorClass::RateLimited,
                message: "429".to_string(),
                status: Some(429),
                failover,
            },
        });
        body_of(&transient)
    };
    assert_eq!(
        status(true),
        r#"{"seen":44,"retry":{"attempt":1,"limit":5,"wait_ms":0,"class":"rate_limited","message":"429","status":429,"failover":true}}"#
    );
    assert!(!status(false).contains("failover"));
}

/// `model.changed`（施工 8-9）：`ref`、`endpoint`、`model`、`limits`、`why` 照这个先后，没有的不写。
#[test]
fn model_changed_is_written_in_the_drawing_order() {
    let mut transient = delta(0, Piece::End);
    transient.by = By::Kernel;
    transient.body = TransientBody::ModelChanged(Box::new(ModelChanged {
        reference: Some("@duo".to_string()),
        endpoint: Some(ProviderId::parse("b").unwrap()),
        model: Some(ModelName::parse("y").unwrap()),
        limits: ContextLimits {
            window: Some(32_000),
            compaction_line: Some(12_000),
        },
        why: ChangeWhy::Failover,
    }));
    let line = transient.to_line();
    assert!(line.contains(r#""kind":"model.changed""#), "{line}");
    assert_eq!(
        body_of(&transient),
        r#"{"ref":"@duo","endpoint":"b","model":"y","limits":{"window":32000,"compaction_line":12000},"why":"failover"}"#
    );
    transient.body = TransientBody::ModelChanged(Box::new(ModelChanged {
        reference: None,
        endpoint: None,
        model: None,
        limits: ContextLimits {
            window: None,
            compaction_line: None,
        },
        why: ChangeWhy::Failover,
    }));
    assert_eq!(body_of(&transient), r#"{"limits":{},"why":"failover"}"#);
}
