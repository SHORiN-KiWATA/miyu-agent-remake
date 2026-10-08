//! 问人（施工 D-2，`docs/blueprint/session/tools.md`「问人」）：工具经提问的端口交题，内核记 `question.asked`（选项的
//! `preview` 原样）、等；人回答（带 `notes`），回答落了盘送回工具，成了这次调用的结果。等的时候打断，这次调用只有一条结果。
//! 没人能回答的会话不给端口。

use std::sync::Arc;

use miyu_kernel::event::{Body, Choice, Event, Question, Response, ToolStatus};
use miyu_kernel::id::CallId;
use miyu_kernel::session::{Answer, Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 一道单选题，第一个选项带预览。
fn questions() -> Vec<Question> {
    vec![Question {
        header: Some("库".to_string()),
        question: "用哪个？".to_string(),
        options: vec![
            Choice {
                label: "甲".to_string(),
                description: None,
                preview: Some("fn a() {}\n".to_string()),
            },
            Choice {
                label: "乙".to_string(),
                description: None,
                preview: None,
            },
        ],
        multiple: false,
    }]
}

/// 造一个会话，只有一件会问人的假工具 `ask`；有没有人能回答照 `attended`。
async fn session(home: &Home, script: &Script, attended: bool) -> Handle {
    let ask = Fake::new("ask", Access::Read, Act::Asks(questions()));
    let catalog = Catalog::new([ask as Arc<dyn Tool>]).expect("合写法");
    let opening = Opening {
        attended,
        ..Opening::default()
    };
    home.create_as(script, &catalog, opening).await
}

fn asked(log: &[Event]) -> Option<(CallId, Vec<Question>)> {
    log.iter().find_map(|event| match &event.body {
        Body::QuestionAsked(asked) => Some((asked.call_id, asked.questions.clone())),
        _ => None,
    })
}

fn results(log: &[Event]) -> Vec<(ToolStatus, String)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some((
                result.status.clone(),
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        miyu_kernel::block::Block::Text(text) => text.text.clone(),
                        other => format!("{other:?}"),
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// 说一句，等到她问了：交回调用编号。
async fn until_asked(home: &Home, handle: &Handle) -> CallId {
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let session = handle.id().clone();
    until_logged(home, &session, |log| asked(log).is_some()).await;
    asked(&home.log(&session)).expect("问了").0
}

#[tokio::test]
async fn the_answer_comes_back_to_the_tool_as_its_result() {
    let home = Home::new();
    let script = Script::new([Play::calls(&[("ask", "{}")]), Play::Says("好，用甲。")]);
    let handle = session(&home, &script, true).await;
    let mut pushes = watch(&handle).await;
    let call_id = until_asked(&home, &handle).await;
    let log = home.log(handle.id());
    assert_eq!(asked(&log).expect("问了").1, questions(), "题原样，带预览");
    assert!(results(&log).is_empty(), "还在等");
    let answers = vec![Response {
        picked: vec!["甲".to_string()],
        text: None,
        notes: Some("先这样".to_string()),
    }];
    ask(
        &handle,
        "cmd-2",
        Command::Answer {
            call_id,
            answer: Answer::Questions(answers),
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert_eq!(
        results(&log),
        [(
            ToolStatus::Ok,
            r#"answered [{"picked":["甲"],"notes":"先这样"}]"#.to_string()
        )]
    );
    assert!(kinds(&log).contains(&"question.answered"));
}

#[tokio::test]
async fn an_interrupt_while_asking_leaves_one_result() {
    let home = Home::new();
    let script = Script::new([Play::calls(&[("ask", "{}")])]);
    let handle = session(&home, &script, true).await;
    let mut pushes = watch(&handle).await;
    until_asked(&home, &handle).await;
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 工具那一头当没答收场，交回的不再记：只有内核补的那一条。
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].0, ToolStatus::Cancelled);
}

#[tokio::test]
async fn a_session_no_one_can_answer_gives_no_port() {
    let home = Home::new();
    let script = Script::new([Play::calls(&[("ask", "{}")]), Play::Says("好。")]);
    let handle = session(&home, &script, false).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert!(asked(&log).is_none(), "没问");
    assert_eq!(
        results(&log),
        [(ToolStatus::Error, "no one to ask".to_string())]
    );
}

/// 工具面（`Agents::asks`）：有人能回答的本机主会话有 `ask_user`，没人能回答的（`miyu ask` 开的那种）没有，别的一件不少。
#[tokio::test]
async fn only_a_session_someone_can_answer_offers_ask_user() {
    let home = Home::new();
    let catalog = Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出"))
        .expect("合写法");
    let mut faces = Vec::new();
    for attended in [true, false] {
        let script = Script::new([Play::Says("好。")]);
        let opening = Opening {
            attended,
            ..Opening::default()
        };
        let handle = home.create_as(&script, &catalog, opening).await;
        let mut pushes = watch(&handle).await;
        ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
        let requests = script.requests();
        let names: Vec<String> = requests[0]
            .1
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect();
        faces.push(names);
    }
    assert!(
        faces[0].iter().any(|name| name == "ask_user"),
        "{:?}",
        faces[0]
    );
    let without: Vec<String> = faces[0]
        .iter()
        .filter(|name| name.as_str() != "ask_user")
        .cloned()
        .collect();
    assert_eq!(faces[1], without, "只少这一件");
}
