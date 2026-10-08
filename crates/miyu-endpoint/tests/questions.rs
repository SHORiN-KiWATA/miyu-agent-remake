//! 回答提问（施工 D-2，`docs/blueprint/protocol.md` 的 `session.answer`，`tools/ask_user.md`）：真核心、真的 `ask_user`。
//! 她问一道题，日志里有 `question.asked`（选项带 `preview`）；`session.answer` 交回答（带 `notes`），回应列出事件，她收到的
//! 结果一道一行；选了题目里没有的是 `bad_answer`，什么都没记。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 起核心（真的基础系统）、握手、造会话、说一句：交回连接和会话编号。事件照日志看，不订阅：订阅着的推送会排在回应前面。
async fn started(home: &Home, plays: Vec<Play>) -> (Client, String) {
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools, TOKEN));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let created = client
        .call("create-1", "session.create", json!({ "cwd": cwd }))
        .await;
    let session = created["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("应该造出会话：{created}"))
        .to_string();
    client.say("send-1", &session, "hi").await;
    (client, session)
}

/// 她调一次 `ask_user`：一道单选题，第一个选项带预览。
fn ask() -> Play {
    let args = json!({"questions": [{"question": "用哪个？", "options": [
        {"label": "甲", "preview": "fn a() {}\n"},
        {"label": "乙"}
    ]}]});
    Play::calls(&[("ask_user", &args.to_string())])
}

/// 等到日志里有 `question.asked`，交回它。
async fn asked(home: &Home, session: &str) -> Value {
    let find = || {
        logged(home, session)
            .into_iter()
            .find(|event| event["kind"] == json!("question.asked"))
    };
    until("question.asked", || find().is_some()).await;
    find().expect("有")
}

#[tokio::test]
async fn an_answer_with_notes_comes_back_to_her() {
    let home = Home::new();
    let (mut client, session) = started(&home, vec![ask(), Play::Says("好，用甲。")]).await;
    let event = asked(&home, &session).await;
    assert_eq!(
        event["body"]["questions"][0]["options"][0]["preview"],
        json!("fn a() {}\n")
    );
    let call = event["body"]["call_id"].clone();
    let params = json!({"session": session, "call": call, "answers": [{"picked": ["甲"], "notes": "先这样"}]});
    let reply = client.call("answer-1", "session.answer", params).await;
    assert_eq!(
        reply["result"]["events"].as_array().map(Vec::len),
        Some(1),
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let answered = log
        .iter()
        .find_map(|event| match &event.body {
            Body::QuestionAnswered(answered) => Some(answered.clone()),
            _ => None,
        })
        .expect("记了回答");
    assert_eq!(answered.answers[0].notes.as_deref(), Some("先这样"));
    let result = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .expect("有结果");
    let [Block::Text(text)] = result.blocks.as_slice() else {
        panic!("一段字：{:?}", result.blocks);
    };
    assert_eq!(
        text.text,
        "\"用哪个？\" = 甲 (note: 先这样)\nGo on with these answers in mind.\n"
    );
}

#[tokio::test]
async fn an_answer_that_does_not_fit_is_refused() {
    let home = Home::new();
    let (mut client, session) = started(&home, vec![ask(), Play::Says("好。")]).await;
    let call = asked(&home, &session).await["body"]["call_id"].clone();
    let params = json!({"session": session, "call": call, "answers": [{"picked": ["丙"]}]});
    let reply = client.call("answer-1", "session.answer", params).await;
    assert_eq!(
        reply["error"]["data"]["reason"],
        json!("bad_answer"),
        "{reply}"
    );
    assert!(
        !logged(&home, &session)
            .iter()
            .any(|event| event["kind"] == json!("question.answered")),
        "什么都没记"
    );
}
