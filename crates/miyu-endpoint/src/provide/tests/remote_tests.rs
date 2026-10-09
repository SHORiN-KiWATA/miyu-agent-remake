//! 提供者的工具（施工 O-2 下）：`tool.call` 带是谁要的、是不是主人；超时交回那一句并发 `tool.cancel`；叫它停发 `tool.cancel`、
//! 照样等它回；掐掉（future 被丢掉）发 `tool.cancel`；回应先到的不发。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, CallId, ExternalId, Seq, SessionId, VenueId};
use miyu_kernel::origin::{By, External, Person};
use miyu_tool::{CallIds, Progress};
use tokio::sync::mpsc;

use super::*;
use crate::wire::Response;

const SESSION: &str = "01900000-0000-7000-8000-000000000001";

/// 一件提供者的工具，等 `timeout`；它的包由一个假连接提供，交回往外写的那一头收的。
fn remote(timeout: Duration) -> (RemoteTool, Peer, mpsc::Receiver<String>) {
    let provided = Arc::new(Provided::default());
    let (out, lines) = mpsc::channel(8);
    let peer = Peer::new(out);
    provided.register("onebot", peer.clone());
    let mut checked = checked(tool("read", json!(["local"]))).expect("合写法");
    checked.timeout = timeout;
    let remote = RemoteTool::new(checked, "onebot", provided, texts());
    (remote, peer, lines)
}

/// 一次调用：会话、编号 7 的第 1 件，`asked` 要的。
fn call_by(asked: Option<By>) -> miyu_tool::Call {
    let mut call = miyu_tool::testkit::call(r#"{"to":"群"}"#);
    call.ids = Some(CallIds {
        session: SessionId::parse(SESSION).expect("合写法"),
        call: CallId::new(Seq::new(7).expect("合写法"), 1).expect("合写法"),
        asked,
    });
    call
}

/// 读下一行，认成 JSON。
async fn next(lines: &mut mpsc::Receiver<String>) -> Value {
    serde_json::from_str(&lines.recv().await.expect("有一行")).expect("是 JSON")
}

fn text(done: &miyu_tool::Done) -> String {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("{other:?}"),
    }
}

/// 群里的 `n` 号人；`account` 是主人对应表里对着的账号。
fn member(n: u32, account: Option<&str>) -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").unwrap(),
        id: ExternalId::parse(&format!("qq:{n}")).unwrap(),
        account: account.map(|name| AccountId::parse(name).unwrap()),
        role: None,
    })
}

/// 调一次，交回发出去的 `tool.call` 的参数；回一句 `done`，交回结果。
async fn sent_for(asked: Option<By>) -> (Value, String) {
    let (remote, peer, mut lines) = remote(Duration::from_secs(60));
    let running =
        tokio::spawn(async move { remote.run(call_by(asked), Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(json!({"blocks": [{"type": "text", "text": "done"}]})),
    });
    let done = running.await.unwrap();
    assert!(lines.try_recv().is_err(), "回应先到的不发 tool.cancel");
    (sent, text(&done))
}

#[tokio::test]
async fn a_call_carries_who_asked_and_whether_it_is_the_owner() {
    let (sent, answer) = sent_for(Some(member(1, Some("alice")))).await;
    assert_eq!(answer, "done");
    assert_eq!(sent["method"], "tool.call");
    assert_eq!(sent["params"]["by"], json!(member(1, Some("alice"))));
    assert_eq!(sent["params"]["owner"], json!(true), "对应表里有的是主人");
    assert_eq!(
        sent["params"]["call_id"],
        json!(call_by(None).ids.unwrap().call.to_string())
    );
    let (sent, _) = sent_for(Some(By::Person(Person::new(
        AccountId::parse("alice").unwrap(),
    ))))
    .await;
    assert_eq!(sent["params"]["owner"], json!(true), "本机的人是主人");
    let (sent, _) = sent_for(Some(member(2, None))).await;
    assert_eq!(sent["params"]["owner"], json!(false), "不在对应表里的不是");
    let (sent, _) = sent_for(None).await;
    assert_eq!(sent["params"]["owner"], json!(false), "没有触发的不是");
    assert!(sent["params"].get("by").is_none(), "没有触发的不写 by");
}

#[tokio::test]
async fn a_call_not_answered_in_time_is_timed_out_and_cancelled() {
    let (remote, _peer, mut lines) = remote(Duration::from_millis(50));
    let done = remote.run(call_by(None), Progress::new(|_| {})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The tool \"send_group\" did not answer within 1 seconds.\n"
    );
    assert_eq!(
        done.human,
        Some(
            miyu_kernel::event::Said::new("core/tool-results/timed-out")
                .with("name", "send_group")
                .with("seconds", "1")
        )
    );
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    let cancel = next(&mut lines).await;
    assert_eq!(
        cancel,
        json!({"jsonrpc": "2.0", "method": "tool.cancel",
               "params": {"session": SESSION, "call_id": call_by(None).ids.unwrap().call.to_string()}})
    );
    assert!(lines.try_recv().is_err(), "只发一次");
}

#[tokio::test]
async fn a_call_told_to_stop_is_cancelled_and_still_answered() {
    let (remote, peer, mut lines) = remote(Duration::from_secs(60));
    let call = call_by(None);
    let stop = call.stop.clone();
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    stop.raise();
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(
            json!({"blocks": [{"type": "text", "text": "stopped halfway"}], "error": true}),
        ),
    });
    let done = running.await.unwrap();
    assert_eq!(text(&done), "stopped halfway", "照样等它回");
    assert!(lines.try_recv().is_err(), "只发一次");
}

#[tokio::test]
async fn a_call_dropped_while_waiting_is_cancelled() {
    let (remote, _peer, mut lines) = remote(Duration::from_secs(60));
    let running =
        tokio::spawn(async move { remote.run(call_by(None), Progress::new(|_| {})).await });
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    running.abort();
    assert!(running.await.is_err());
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
}

/// 叫它停发过了，再超时没回：不再发第二次。
#[tokio::test]
async fn a_call_told_to_stop_and_then_timed_out_is_cancelled_once() {
    let (remote, _peer, mut lines) = remote(Duration::from_millis(400));
    let call = call_by(None);
    let stop = call.stop.clone();
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    stop.raise();
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
    let done = running.await.unwrap();
    assert!(text(&done).contains("did not answer"), "{}", text(&done));
    assert!(lines.try_recv().is_err(), "只发一次");
}
