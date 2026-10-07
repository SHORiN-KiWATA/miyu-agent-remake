//! 派子代理时挑人格（施工 P-2 补，走查 C5）：真核心走一遍。没挑的是软件工程师；挑了的照挑的造；预设一律照父会话的，写了
//! `preset` 的也不理（2026-10-08 项目主人定）；写了这个会话没列着的人格，照参数不对、不派。能挑的人格进 `subagent` 的
//! `persona` 那一格的 `enum`。

mod support;

use serde_json::json;

use miyu_kernel::event::{Body, Effect, SessionCreated};
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;
use support::{Client, Home, TOKEN, default_resources};

/// 父会话这一轮派出去的子会话，照先后。
fn children(log: &[miyu_kernel::event::Event]) -> Vec<SessionId> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
        .collect()
}

fn created(home: &Home, session: &SessionId) -> SessionCreated {
    match &home.log(session.as_str())[0].body {
        Body::SessionCreated(created) => created.clone(),
        other => panic!("第 1 条应该是造会话：{other:?}"),
    }
}

#[tokio::test]
async fn a_subagent_takes_the_chosen_persona_and_preset_or_the_defaults() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let plain = json!({"description": "照常", "prompt": "Read."}).to_string();
    let chosen =
        json!({"description": "挑了", "prompt": "Read.", "persona": "none", "preset": "dev"})
            .to_string();
    let wrong = json!({"description": "写错", "prompt": "Read.", "persona": "kiki"}).to_string();
    // 父会话一轮里连着调三次；两个子会话各答一句，谁先到不一定。
    let script = Script::new([
        Play::calls(&[
            ("subagent", &plain),
            ("subagent", &chosen),
            ("subagent", &wrong),
        ]),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": work, "preset": "full"}),
        )
        .await;
    let parent = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("c2", &parent, "派几个去查").await;
    home.until_turns(&parent, 1).await;
    let log = home.log(&parent);
    let made = children(&log);
    assert_eq!(made.len(), 2, "写错的不派");
    // 读的调用连着一起派，谁先造好不一定：照人格排了再比。
    let mut picks: Vec<(Option<String>, Option<String>)> = made
        .iter()
        .map(|child| {
            let created = created(&home, child);
            (created.persona, created.preset)
        })
        .collect();
    picks.sort();
    let some = |text: &str| Some(text.to_string());
    assert_eq!(
        picks,
        [
            (some("engineer"), some("full")),
            (some("none"), some("full"))
        ],
        "没挑的是软件工程师；预设一律照父会话的"
    );
    let said = serde_json::to_string(&log).expect("写得成 JSON");
    assert!(
        said.contains("unknown variant `kiki`, expected `engineer` or `none`"),
        "写错的照参数不对，列出能写的"
    );
    let (_, first) = script.requests().into_iter().next().expect("发了请求");
    let subagent = first
        .tools
        .iter()
        .find(|tool| tool.name == "subagent")
        .expect("有 subagent");
    let parameters: serde_json::Value =
        serde_json::from_str(subagent.parameters.get()).expect("参数是 JSON");
    assert_eq!(
        parameters["properties"]["persona"]["enum"],
        json!(["engineer", "none"]),
        "能挑的人格同池，进 enum"
    );
}
