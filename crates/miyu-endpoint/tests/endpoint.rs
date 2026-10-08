//! 协议端点（`docs/construction/3-8-协议端点（上）.md` 验收第 2 条）：在内存里的管道上连核心，握手、造会话、
//! 说话、打断；会话表载入这次运行里没在跑的会话；读不懂的、不是请求的、太长的各回 JSON-RPC 标准的错。

use serde_json::json;
use tokio::io::AsyncWriteExt;

use crate::support::{Client, Home, alice, reason, until};
use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;

/// 一行的上限：和核心的一样，1 MiB。
const LINE_LIMIT: usize = 1024 * 1024;

#[tokio::test]
async fn hello_comes_first_and_is_checked() {
    let home = Home::new();
    let core = home.core(&Script::new([]));

    // 第一条不是 hello：拒绝，连接还在；没握手，话是英文。
    let mut client = Client::connect(core.clone());
    let reply = client
        .call("c1", "session.create", json!({"cwd": "~"}))
        .await;
    assert_eq!(reason(&reply), Some("hello_first"), "{reply}");
    assert_eq!(reply["error"]["code"], json!(-32010));
    assert_eq!(reply["id"], json!("c1"));
    assert_eq!(
        reply["error"]["message"],
        json!("Say hello first after connecting.")
    );

    // 握手：选定的主版本、核心的版本、你是谁。
    let reply = client.hello().await;
    assert_eq!(reply["result"]["protocol"], json!(1), "{reply}");
    assert_eq!(reply["result"]["account"], json!("alice"));
    assert!(reply["result"]["core"]["version"].is_string());

    // 令牌不对：拒绝，断开。
    let mut client = Client::connect(core.clone());
    let reply = client
        .call(
            "h1",
            "hello",
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": "wrong"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_token"), "{reply}");
    assert!(client.next().await.is_none(), "令牌不对的断开");

    // 只对上前一截的令牌也不行：每个字节都比，长短也比。
    let mut client = Client::connect(core.clone());
    let reply = client
        .call(
            "h1",
            "hello",
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": &crate::support::TOKEN[..5]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_token"), "{reply}");

    // 主版本没有交集：拒绝，断开。
    let mut client = Client::connect(core);
    let reply = client
        .call(
            "h1",
            "hello",
            json!({"protocol": [2, 3], "head": {"kind": "test", "version": "0"}, "token": crate::support::TOKEN}),
        )
        .await;
    assert_eq!(reason(&reply), Some("protocol_mismatch"), "{reply}");
    assert!(client.next().await.is_none(), "版本对不上的断开");
}

#[tokio::test]
async fn a_session_is_created_and_spoken_to() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("你好。")]));
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": "~/src/miyu"}))
        .await;
    assert_eq!(reply["result"]["events"], json!([1]), "{reply}");
    let session = reply["result"]["session"]
        .as_str()
        .expect("有会话编号")
        .to_string();
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    assert!(
        reply["result"]["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty()),
        "{reply}"
    );
    assert_eq!(reply["id"], json!("c2"));
    home.until_turns(&session, 1).await;
}

#[tokio::test]
async fn the_same_create_sent_twice_makes_one_session() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(core);
    client.hello().await;
    let first = client.create("c-dup", "~").await;
    let again = client.create("c-dup", "~").await;
    assert_eq!(first, again, "断线重发的造会话不再造一个新的");
}

#[tokio::test]
async fn an_unknown_session_is_not_found() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(core);
    client.hello().await;
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    let reply = client
        .call(
            "c1",
            "session.send",
            json!({"session": missing, "text": "hi"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("没有这个会话。"));
}

#[tokio::test]
async fn a_session_from_an_earlier_run_is_loaded() {
    let home = Home::new();
    let script = Script::new([Play::Says("你好。"), Play::Says("又见面了。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;

    // 换一份核心，像重启过：会话表是空的，说给那个会话，从磁盘载入。
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "again"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 2).await;
}

#[tokio::test]
async fn two_connections_loading_one_session_start_one_actor() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "one"}),
        )
        .await;
    home.until_turns(&session, 1).await;

    // 换一份核心，两个连接同时说给这个没在跑的会话：只载入一次。起了两个 actor 的话，后写的那个
    // 序号接不上，写不进去，会话停下。
    let core = home.core(&script);
    let (mut a, mut b) = (Client::connect(core.clone()), Client::connect(core));
    a.hello().await;
    b.hello().await;
    let (first, second) = tokio::join!(
        a.call(
            "c3",
            "session.send",
            json!({"session": session, "text": "two"})
        ),
        b.call(
            "c4",
            "session.send",
            json!({"session": session, "text": "three"})
        ),
    );
    assert!(first["result"]["events"].is_array(), "{first}");
    assert!(second["result"]["events"].is_array(), "{second}");
    // 第二句排在第一句那一轮后面，接着开一轮：一共三轮。
    home.until_turns(&session, 3).await;
}

#[tokio::test]
async fn interrupting_nothing_is_refused_in_the_heads_language() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("not_running"), "{reply}");
    assert_eq!(reply["error"]["code"], json!(-32010));
    assert_eq!(
        reply["error"]["message"],
        json!("没有正在进行的回合，打断不了。")
    );
}

#[tokio::test]
async fn json_rpc_errors_follow_the_standard() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client.line("not json").await;
    let reply = client.next().await.expect("有回应");
    assert_eq!(reply["error"]["code"], json!(-32700), "{reply}");
    client.line("[1, 2]").await;
    let reply = client.next().await.expect("有回应");
    assert_eq!(reply["error"]["code"], json!(-32600), "{reply}");
    let reply = client.call("c1", "session.fly", json!({})).await;
    assert_eq!(reply["error"]["code"], json!(-32601), "{reply}");
    let reply = client
        .call("c2", "session.send", json!({"session": "x"}))
        .await;
    assert_eq!(reply["error"]["code"], json!(-32602), "{reply}");
    // 通知不回应：下一行就是下一条请求的回应。
    client
        .line(r#"{"jsonrpc":"2.0","method":"session.send","params":{}}"#)
        .await;
    let reply = client.call("c3", "session.fly", json!({})).await;
    assert_eq!(reply["id"], json!("c3"), "通知没有回应：{reply}");
}

#[tokio::test]
async fn a_line_too_long_is_refused_and_closes() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    // 超长的一行放到另一个任务里写：核心读到上限就不读了，写的一头不能挡着读的一头。
    let mut writer = client.detach_writer();
    let written = tokio::spawn(async move {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "核心读到上限就断开了，剩下的写不进去是对的"
        )]
        let _ = writer.write_all(&vec![b'x'; LINE_LIMIT + 10]).await;
    });
    let reply = client.next().await.expect("有回应");
    assert_eq!(reply["error"]["code"], json!(-32700), "{reply}");
    assert!(client.next().await.is_none(), "太长的一行以后断开");
    written.await.expect("写的任务没 panic");
}

#[tokio::test]
async fn the_working_directory_stays_with_the_session() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~/src/one").await;
    client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "again", "cwd": "~/src/two"}),
        )
        .await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let text = |n: usize| String::from_utf8(requests[n].1.canonical_bytes()).expect("请求是 UTF-8");
    // 工作区是会话的属性（施工 9-7 上）：说话带着的目录照收不理，换工作区另走 `session.set_workspace`。
    assert!(text(0).contains("~/src/one"), "{}", text(0));
    assert!(!text(1).contains("~/src/two"), "{}", text(1));
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::WorkspaceChanged(_))),
        "什么都没换"
    );
}

#[tokio::test]
async fn a_head_that_cannot_take_input_makes_unattended_sessions() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello_without_input().await;
    let session = client.create("c1", "~").await;
    // 有没有人能确认，冻在会话的策略快照里：不能输入的头造的，没有人确认。
    let log = home.log(&session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    let snapshot = Snapshot::from_bytes(&bytes).expect("读得懂");
    assert!(!snapshot.attended);
}

#[tokio::test]
async fn an_empty_message_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.say("c2", &session, "").await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
}

#[tokio::test]
async fn a_stopped_session_is_loaded_again_next_time() {
    let home = Home::new();
    // 端口一叫就 panic：这个会话的 actor 停了。
    let script = Script::new([Play::Panics, Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.say("c2", &session, "hi").await;
    assert!(
        reply["result"]["events"].is_array(),
        "落了盘就回应了：{reply}"
    );
    // 下一句：会话停了，从表里拿掉。
    let reply = client.say("c3", &session, "again").await;
    assert_eq!(reason(&reply), Some("session_stopped"), "{reply}");
    // 再下一句：重新载入，崩了的那一轮收尾，这一句接着说完。
    let reply = client.say("c4", &session, "again").await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 2).await;
}

#[tokio::test]
async fn interrupting_with_queued_send_goes_on_with_the_queue() {
    let home = Home::new();
    let script = Script::new([Play::Holds, Play::Says("二。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "one").await;
    until("请求停住", || script.requests().len() == 1).await;
    // 第二句排着队；打断时说「排着的接着发」：打断以后马上由它开一轮。
    client.say("c3", &session, "two").await;
    let reply = client
        .call(
            "c4",
            "session.interrupt",
            json!({"session": session, "queued": "send"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 2).await;
    assert_eq!(script.requests().len(), 2);
}

/// 握手的回应照核心探到的报沙盒（施工 5-4 下）：能用的只说能用，用不了的带原因；没设的当找不到助手。
#[tokio::test]
async fn hello_says_whether_the_sandbox_can_be_used() {
    use miyu_sandbox::{Availability, Unusable};
    use miyu_tool::Catalog;

    let home = Home::new();
    let script = Script::new([]);
    let usable = Availability::Usable(std::path::PathBuf::from("miyu-sandbox"));
    let mut client =
        Client::connect(home.core_sandboxed(&script, Catalog::default(), usable, None));
    let reply = client.hello().await;
    assert_eq!(
        reply["result"]["sandbox"],
        json!({"usable": true}),
        "{reply}"
    );
    for (reason, code) in [
        (Unusable::HelperMissing, "helper_missing"),
        (Unusable::HelperFailed, "helper_failed"),
        (Unusable::NoMechanism, "no_mechanism"),
    ] {
        let core = home.core_sandboxed(
            &script,
            Catalog::default(),
            Availability::Unusable(reason),
            None,
        );
        let mut client = Client::connect(core);
        let reply = client.hello().await;
        assert_eq!(
            reply["result"]["sandbox"],
            json!({"usable": false, "reason": code}),
            "{reply}"
        );
    }
    let mut client = Client::connect(home.core_without_sandbox(&script, Catalog::default()));
    let reply = client.hello().await;
    assert_eq!(
        reply["result"]["sandbox"],
        json!({"usable": false, "reason": "helper_missing"}),
        "没设的当找不到助手：{reply}"
    );
}
