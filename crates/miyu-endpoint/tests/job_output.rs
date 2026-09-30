//! 协议上读后台命令的输出（施工 7-4 补，`docs/blueprint/protocol.md` 的 `job.output`）：真核心走一遍，后台命令用假的
//! （`miyu_tool::testkit::Held`，三个平台一样）。跑着的读到这时为止的、`running` 是真；结束了的读存下的那一份、`running` 是假，
//! 和她用 `jobs` 读到的一样；只交最后 `tail` 行，`lines` 数一共几行，前面还有没交的 `truncated`；超了上限的从前面按整行去掉；
//! 空的；没有这个任务、编号不合写法、`tail` 不对、是子代理的各一种拒绝，中文、英文。

mod support;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{JobId, SessionId};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, Tool};

use support::*;

/// 基础系统的工具（有 `jobs`、`agent`），加一件假的 `start`：把 `held` 交给任务端口。
fn tools(held: &Arc<Held>) -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    let start: Arc<dyn Tool> = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    tools.push(start);
    Catalog::new(tools).expect("合写法")
}

/// 造一个会话，叫她把 `held` 放到后台，等这一轮说完。`plays` 接在「调 `start`、说一句」后面。
async fn started(home: &Home, held: &Arc<Held>, plays: Vec<Play>) -> (Client, String) {
    let mut all = vec![Play::calls(&[("start", "{}")]), Play::Says("放出去了。")];
    all.extend(plays);
    let mut client = Client::connect(home.core_with_tools(&Script::new(all), tools(held), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "后台跑").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

/// 读 `j1` 的输出，`tail` 照给的（`None` 是不写）。交回回应的 `result`。
async fn read(client: &mut Client, id: &str, session: &str, tail: Option<Value>) -> Value {
    let mut params = json!({"session": session, "job": "j1"});
    if let Some(tail) = tail {
        params["tail"] = tail;
    }
    let reply = client.call(id, "job.output", params).await;
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

/// 一直读 `j1` 的输出，直到 `done` 成立，最多十秒：读输出的线程把字写进文件要一会儿。
async fn read_until(client: &mut Client, session: &str, done: impl Fn(&Value) -> bool) -> Value {
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        let mut n = 0;
        loop {
            n += 1;
            let got = read(client, &format!("w{n}"), session, None).await;
            if done(&got) {
                return got;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.expect("十秒内读到了")
}

/// 日志里第 `n` 条工具结果的字。
fn tool_text(log: &[Event], n: usize) -> String {
    let texts: Vec<String> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    texts[n].clone()
}

fn reported(log: &[Event]) -> bool {
    log.iter()
        .any(|event| matches!(event.body, Body::JobReported(_)))
}

#[tokio::test]
async fn a_running_command_reads_so_far_and_an_ended_one_reads_it_all() {
    let home = Home::new();
    let held = Held::new(&["building\n", "half\n"]);
    let output = json!({"action": "output", "id": "j1"}).to_string();
    let plays = vec![
        Play::calls(&[("jobs", output.as_str())]),
        Play::Says("还在跑。"),
        Play::Says("跑完了。"),
        Play::calls(&[("jobs", output.as_str())]),
        Play::Says("看完了。"),
    ];
    let (mut client, session) = started(&home, &held, plays).await;

    let running = read_until(&mut client, &session, |got| got["lines"] == json!(2)).await;
    assert_eq!(
        running,
        json!({"lines": 2, "output": "building\nhalf\n", "running": true, "truncated": false})
    );
    client.say("c3", &session, "看一眼").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        tool_text(&home.log(&session), 1),
        "building\nhalf\n(j1 is still running.)\n",
        "她用 `jobs` 读到的是同一份"
    );

    held.end(Exit::Code(0));
    until("它结束了、回报落了盘", || {
        reported(&home.log(&session))
    })
    .await;
    home.until_turns(&session, 3).await;
    // 结束了的读存下的那一份（blob），不读会话目录下的输出文件：拿掉它照样读得到。
    let dir = home
        .root
        .session_dir(&alice(), &SessionId::parse(&session).unwrap());
    let file = miyu_store::jobs::output_path(&dir, &JobId::new(1).unwrap());
    std::fs::remove_file(&file).expect("输出文件在");
    let ended = read(&mut client, "r1", &session, None).await;
    assert_eq!(
        ended,
        json!({"lines": 2, "output": "building\nhalf\n", "running": false, "truncated": false})
    );
    client.say("c4", &session, "再看一眼").await;
    home.until_turns(&session, 4).await;
    assert_eq!(
        tool_text(&home.log(&session), 2),
        ended["output"].as_str().unwrap(),
        "和她用 `jobs` 读到的一字不差"
    );
}

#[tokio::test]
async fn only_the_last_lines_are_read() {
    let home = Home::new();
    let held = Held::new(&["1\n", "2\n", "3\n", "4\n", "5"]);
    let (mut client, session) = started(&home, &held, Vec::new()).await;
    let all = read_until(&mut client, &session, |got| got["lines"] == json!(5)).await;
    assert_eq!(
        all,
        json!({"lines": 5, "output": "1\n2\n3\n4\n5", "running": true, "truncated": false}),
        "最后一段没有换行也算一行，原样交"
    );
    for (tail, output, truncated) in [
        (json!(2), "4\n5", true),
        (json!(1), "5", true),
        (json!(5), "1\n2\n3\n4\n5", false),
        (json!(2000), "1\n2\n3\n4\n5", false),
    ] {
        let got = read(&mut client, "t1", &session, Some(tail.clone())).await;
        assert_eq!(
            (got["output"].as_str(), &got["truncated"], &got["lines"]),
            (Some(output), &json!(truncated), &json!(5)),
            "tail {tail}"
        );
    }
}

#[tokio::test]
async fn the_default_is_the_last_200_lines() {
    let home = Home::new();
    let lines: Vec<String> = (1..=250).map(|n| format!("{n}\n")).collect();
    let pieces: Vec<&str> = lines.iter().map(String::as_str).collect();
    let held = Held::new(&pieces);
    let (mut client, session) = started(&home, &held, Vec::new()).await;
    let got = read_until(&mut client, &session, |got| got["lines"] == json!(250)).await;
    assert_eq!(got["output"].as_str(), Some(lines[50..].concat().as_str()));
    assert_eq!(got["truncated"], json!(true));
}

#[tokio::test]
async fn an_output_that_cannot_be_opened_reads_empty() {
    let home = Home::new();
    let held = Held::new(&["x\n"]);
    let (mut client, session) = started(&home, &held, Vec::new()).await;
    read_until(&mut client, &session, |got| got["lines"] == json!(1)).await;
    let dir = home
        .root
        .session_dir(&alice(), &SessionId::parse(&session).unwrap());
    std::fs::remove_file(miyu_store::jobs::output_path(&dir, &JobId::new(1).unwrap()))
        .expect("输出文件在");
    assert_eq!(
        read(&mut client, "r1", &session, None).await,
        json!({"lines": 0, "output": "", "running": true, "truncated": false}),
        "开不了的当是空的，和 `jobs` 一样"
    );
}

#[tokio::test]
async fn too_much_drops_whole_lines_from_the_front() {
    let home = Home::new();
    // 一次最多 128 KiB（`protocol.md` 的 `job.output` 第 3 条）：三行各五万字节，最后两行放得下。
    let line = |c: &str| format!("{}\n", c.repeat(49_999));
    let (a, b, c) = (line("a"), line("b"), line("c"));
    let held = Held::new(&[&a, &b, &c]);
    let (mut client, session) = started(&home, &held, Vec::new()).await;
    let got = read_until(&mut client, &session, |got| got["lines"] == json!(3)).await;
    assert_eq!(got["output"].as_str(), Some(format!("{b}{c}").as_str()));
    assert_eq!(got["truncated"], json!(true));
    let one = read(&mut client, "t1", &session, Some(json!(1))).await;
    assert_eq!(
        (one["output"].as_str(), &one["truncated"]),
        (Some(c.as_str()), &json!(true))
    );
}

#[tokio::test]
async fn an_empty_output_has_no_lines() {
    let home = Home::new();
    let held = Held::new(&[]);
    let (mut client, session) = started(&home, &held, vec![Play::Says("跑完了。")]).await;
    assert_eq!(
        read(&mut client, "r1", &session, None).await,
        json!({"lines": 0, "output": "", "running": true, "truncated": false})
    );
    held.end(Exit::Code(0));
    until("它结束了、回报落了盘", || {
        reported(&home.log(&session))
    })
    .await;
    assert_eq!(
        read(&mut client, "r2", &session, None).await,
        json!({"lines": 0, "output": "", "running": false, "truncated": false})
    );
}

#[tokio::test]
async fn what_cannot_be_read_is_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("s1", "job.output", json!({"session": session, "job": "j1"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_job"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("没有这个任务，或者它已经结束了。")
    );
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("e1", "job.output", json!({"session": session, "job": "j1"}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!("There is no such job, or it has already ended.")
    );
    for job in [
        json!("1"),
        json!("j0"),
        json!("j1.0"),
        json!("j1."),
        json!(1),
        Value::Null,
    ] {
        let reply = client
            .call("s2", "job.output", json!({"session": session, "job": job}))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{job}: {reply}");
    }
    for tail in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!("10"),
        json!(true),
        Value::Null,
        json!(2001),
    ] {
        let reply = client
            .call(
                "s3",
                "job.output",
                json!({"session": session, "job": "j1", "tail": tail}),
            )
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "tail {tail}: {reply}");
    }
    let nobody = "01a0d78c-ca52-7d19-8b64-000000000000";
    let reply = client
        .call(
            "s4",
            "job.output",
            json!({"session": nobody, "job": "j1", "tail": 0}),
        )
        .await;
    assert_eq!(
        reason(&reply),
        Some("bad_params"),
        "`tail` 先查，不找会话：{reply}"
    );
    let reply = client
        .call("s5", "job.output", json!({"session": nobody, "job": "j1"}))
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    let reply = client
        .call("s6", "job.output", json!({"session": "x", "job": "j1"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}

#[tokio::test]
async fn a_subagent_is_not_a_command() {
    let home = Home::new();
    let args = json!({"description": "查 crate", "prompt": "Read Cargo.toml."}).to_string();
    // 派出去以后，父会话的下一次请求和子会话的第一次请求都停住。
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Holds,
        Play::Holds,
    ]);
    let core = home.core_with_tools(&script, tools(&Held::new(&[])), TOKEN);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let parent = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &parent, "派一个去查").await;
    until("两次请求都停住", || script.requests().len() == 3).await;

    let reply = client
        .call("o1", "job.output", json!({"session": parent, "job": "j1"}))
        .await;
    assert_eq!(reason(&reply), Some("not_a_command"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("这是子代理，不是后台命令：去看它的会话。")
    );
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("o2", "job.output", json!({"session": parent, "job": "j1"}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!("This is a subagent, not a background command; open its session instead.")
    );
}
