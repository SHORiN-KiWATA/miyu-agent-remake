//! 场景：问人的两种。她问一组题，你回答，工具拿到回答写成结果；要确认的，允许这一次，拒绝并写了
//! 理由，她接着干（`02-内核.md` 第六节「确认怎么走」「提问怎么走」）。

use super::*;
use crate::event::{Decision, Question, Response};

/// 一组题：旧的 build 目录删掉还是保留。
fn build_question() -> Vec<Question> {
    serde_json::from_str(
        r#"[{"question":"旧的 build 目录要删掉，还是保留？","options":[{"label":"删掉"},{"label":"保留"}]}]"#,
    )
    .unwrap()
}

#[test]
fn she_asks_and_the_tool_gets_the_answer() {
    let mut stage = stage();
    stage.model([
        Line::calls("清之前先问你一句。", &[("ask_user", "{}")]),
        Line::says("好，build 目录保留。"),
    ]);
    stage.tools([Play::Asks(build_question())]);
    stage.say("把旧的构建产物清一清");
    stage.reply(
        call(6, 1),
        vec![Response {
            picked: vec!["保留".to_string()],
            text: None,
        }],
    );
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 question.asked tool call_6_1 t3",
            "9 question.answered alice t3",
            "10 tool.result:ok tool call_6_1 t3",
            "11 message.assistant model t3",
            "12 model.called:ok kernel t3",
            "13 turn.ended:completed kernel t3",
        ])
    );
    let Body::ToolResult(result) = &stage.log()[9].body else {
        panic!("10 号应该是工具结果");
    };
    assert_eq!(
        result.blocks,
        [Block::Text(Text {
            text: "The user answered: 保留".to_string()
        })]
    );
}

#[test]
fn a_decision_either_way_and_she_goes_on() {
    let ask = || Verdict::Ask {
        module: crate::id::ModuleId::parse("permissions").unwrap(),
        access: Access::Write,
        rule: None,
        detail: None,
    };
    let mut stage = stage();
    stage.model([
        Line::calls("我改一下 a。", &[("write", r#"{"path":"a"}"#)]),
        Line::calls("再改 b。", &[("write", r#"{"path":"b"}"#)]),
        Line::says("a 改了，b 你没让改。"),
    ]);
    stage.guards([ask(), ask()]);
    stage.tools([Play::done("wrote a")]);
    stage.say("改 a 和 b");
    stage.decide(call(6, 1), Decision::Once, None);
    stage.decide(call(11, 1), Decision::Deny, Some("b 别动"));
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.approval_requested permissions t3",
            "9 tool.approval_decided alice t3",
            "10 tool.result:ok tool call_6_1 t3",
            "11 message.assistant model t3",
            "12 model.called:ok kernel t3",
            "13 tool.approval_requested permissions t3",
            "14 tool.approval_decided alice t3",
            "15 tool.result:denied alice t3",
            "16 message.assistant model t3",
            "17 model.called:ok kernel t3",
            "18 turn.ended:completed kernel t3",
        ])
    );
}
