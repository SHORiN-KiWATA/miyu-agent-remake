//! 撤销、恢复的回应里给人看的几样（施工 4-7 下）：会话的工作目录、那一轮人说的话、执行过几条命令、每个文件怎样；
//! 之后又被改过的附上差异（上下文 3 行、最多 20 行、结尾没换行不加提示），太大的、不是文本的不附；只算撤掉的那几轮；
//! 恢复时对照的是撤销以后的样子。改回了内容的也附上差异，连同新增、删掉的行数（施工 4-7 再补，`crates/miyu-endpoint/
//! src/undo/tests.rs` 另有新建的文件、`trash`、本来就一样几种的单元测试）。

mod support;

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 基础系统的工具，加一件假的执行命令的 `run`（施工 4-7 下：撤掉的几轮执行过几条命令）。
fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    let run: Arc<dyn Tool> = Fake::new("run", Access::Execute, Act::Echo);
    tools.push(run);
    Catalog::new(tools).expect("合写法")
}

/// 在场地的工作目录里造会话，她照 `plays` 走一轮：把 `a.txt` 从 `old` 改成 `new`。交回客户端和会话。
async fn edited(home: &Home, plays: Vec<Play>) -> (Client, String) {
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

/// 读 `a.txt`，改成 `new`，顺手执行一条命令。
fn edit_and_run() -> Vec<Play> {
    vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[
            ("write", r#"{"file_path":"a.txt","content":"new\n"}"#),
            ("run", "{}"),
        ]),
        Play::Says("改好了。"),
    ]
}

/// 给人看的路径：Windows 上去掉 `\\?\` 这个前缀，和回应里写的一样。
fn plain(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// 回应里的 `result`。
fn result_of(reply: &Value) -> &Value {
    assert!(reply.get("error").is_none(), "{reply}");
    &reply["result"]
}

/// 不写回合编号的撤最后一轮（施工 4-7 下）：回应里写着会话的工作目录、那一轮人说的话、执行过几条命令、每个文件
/// 怎样。
#[tokio::test]
async fn an_undo_says_which_turn_and_what_came_back() {
    let home = Home::new();
    let (mut client, session) = edited(&home, edit_and_run()).await;
    let reply = client
        .call("c3", "session.revert", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    assert_eq!(
        result["events"].as_array().map(Vec::len),
        Some(2),
        "{result}"
    );
    assert_eq!(result["cwd"], json!(plain(&home.work)));
    assert_eq!(result["turns"], json!(1));
    assert_eq!(result["said"], json!("改一下"));
    assert_eq!(result["commands"], json!(1));
    assert!(
        result.get("compactions").is_none(),
        "没撤掉压缩的不写：{result}"
    );
    let path = plain(&home.work.join("a.txt"));
    assert_eq!(
        result["files"],
        json!([{
            "path": path, "action": "write", "outcome": "restored",
            "diff": ["@@ -1 +1 @@", "-new", "+old"], "added": 1, "removed": 1,
        }])
    );
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "old\n"
    );
    // 恢复：说的是同一句，不说执行过几条命令；改回的是重新做的内容，差异反过来。
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    assert_eq!(result["said"], json!("改一下"));
    assert!(result.get("commands").is_none(), "{result}");
    assert!(result.get("compactions").is_none(), "{result}");
    assert_eq!(
        result["files"],
        json!([{
            "path": path, "action": "write", "outcome": "restored",
            "diff": ["@@ -1 +1 @@", "-old", "+new"], "added": 1, "removed": 1,
        }])
    );
}

/// 之后又被改过的：没动，附上她改完的和现在的差异；太长的只交 20 行，说还有几行。
#[tokio::test]
async fn what_changed_since_comes_with_a_diff() {
    let home = Home::new();
    let (mut client, session) = edited(&home, edit_and_run()).await;
    std::fs::write(home.work.join("a.txt"), "someone\n").expect("写得进");
    let reply = client
        .call("c3", "session.revert", json!({"session": session}))
        .await;
    let file = &result_of(&reply)["files"][0];
    assert_eq!(file["outcome"], json!("changed"));
    assert_eq!(file["diff"], json!(["@@ -1 +1 @@", "-new", "+someone"]));
    assert!(file.get("more").is_none());
    assert_eq!(file["added"], json!(1), "{file}");
    assert_eq!(file["removed"], json!(1), "{file}");
    // 改了很多行的：只交 20 行，`more` 是截断以后的，`added`、`removed` 照整份差异数。
    let long: String = (0..30).map(|n| format!("line {n}\n")).collect();
    std::fs::write(home.work.join("a.txt"), &long).expect("写得进");
    client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    let reply = client
        .call("c5", "session.revert", json!({"session": session}))
        .await;
    let file = &result_of(&reply)["files"][0];
    assert_eq!(file["diff"].as_array().map(Vec::len), Some(20), "{file}");
    assert_eq!(
        file["more"],
        json!(12),
        "差异一共 32 行：一行 @@、一行删掉的、30 行加上的"
    );
    assert_eq!(file["added"], json!(30), "{file}");
    assert_eq!(file["removed"], json!(1), "{file}");
}

/// 写一次 `a.txt`，内容是 `content`。
fn write(content: &str) -> Play {
    Play::calls(&[(
        "write",
        &json!({"file_path": "a.txt", "content": content}).to_string(),
    )])
}

/// 撤最后一轮，交回回应里的 `result`。
async fn undo(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client
        .call(id, "session.revert", json!({"session": session}))
        .await;
    result_of(&reply).clone()
}

/// 两轮：第一轮执行一条命令，第二轮人说了两行、前后带空格，她改了 `a.txt`。撤最后一轮：说的是第二轮那句的第一行，
/// 执行过的命令只算撤掉的那一轮。
#[tokio::test]
async fn only_the_undone_turn_is_counted() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let plays = vec![
        Play::calls(&[("run", "{}")]),
        Play::Says("跑完了。"),
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write("new\n"),
        Play::Says("改好了。"),
    ];
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "  改一下  \n顺便看看").await;
    home.until_turns(&session, 2).await;
    let result = undo(&mut client, "c4", &session).await;
    assert_eq!(result["said"], json!("改一下"), "{result}");
    assert_eq!(result["commands"], json!(0), "{result}");
    assert_eq!(result["files"][0]["outcome"], json!("restored"));
}

/// 恢复时被人改过的：对照的是撤销以后的样子（改前）。
#[tokio::test]
async fn a_redo_compares_against_the_undone_content() {
    let home = Home::new();
    let (mut client, session) = edited(&home, edit_and_run()).await;
    undo(&mut client, "c3", &session).await;
    std::fs::write(home.work.join("a.txt"), "someone\n").expect("写得进");
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    let file = &result_of(&reply)["files"][0];
    assert_eq!(file["outcome"], json!("changed"));
    assert_eq!(file["diff"], json!(["@@ -1 +1 @@", "-old", "+someone"]));
}

/// 差异带上下文 3 行；结尾没有换行的，不加「没有换行」那一句。
#[tokio::test]
async fn the_diff_has_three_lines_of_context_and_no_newline_hint() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write("1\n2\n3\nB\n5\n6\n7"),
        Play::Says("改好了。"),
    ];
    let (mut client, session) = edited(&home, plays).await;
    std::fs::write(home.work.join("a.txt"), "1\n2\n3\nC\n5\n6\n7").expect("写得进");
    let result = undo(&mut client, "c3", &session).await;
    assert_eq!(
        result["files"][0]["diff"],
        json!([
            "@@ -1,7 +1,7 @@",
            " 1",
            " 2",
            " 3",
            "-B",
            "+C",
            " 5",
            " 6",
            " 7"
        ]),
        "{result}"
    );
}

/// 太大的（超过 1 MiB）、不是文本的：没动，不附差异。
#[tokio::test]
async fn big_or_binary_files_come_without_a_diff() {
    let home = Home::new();
    let (mut client, session) = edited(&home, edit_and_run()).await;
    std::fs::write(home.work.join("a.txt"), vec![b'x'; (1 << 20) + 1]).expect("写得进");
    let result = undo(&mut client, "c3", &session).await;
    assert_eq!(result["files"][0]["outcome"], json!("changed"));
    assert!(result["files"][0].get("diff").is_none(), "{result}");
    client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    std::fs::write(home.work.join("a.txt"), [0xff, 0xfe, 0x00]).expect("写得进");
    let result = undo(&mut client, "c5", &session).await;
    assert_eq!(result["files"][0]["outcome"], json!("changed"));
    assert!(result["files"][0].get("diff").is_none(), "{result}");
}

/// 会话的工作目录是一个链接：回应里写真实的位置，头照它写相对的路径才对得上。
#[cfg(unix)]
#[tokio::test]
async fn the_working_directory_is_given_as_its_real_place() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let link = home.work.with_extension("link");
    std::os::unix::fs::symlink(&home.work, &link).expect("建得了链接");
    let script = Script::new(edit_and_run());
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &link.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    let result = undo(&mut client, "c3", &session).await;
    std::fs::remove_file(&link).expect("删得掉");
    assert_eq!(result["cwd"], json!(plain(&home.work)), "{result}");
}

/// 被打断的一轮（施工 4-9 再补一）：跑到一半的命令算跑过，排在它后面、还没派的不算；人说的话开头是空行的，取第一行
/// 不空的。
#[tokio::test]
async fn only_commands_that_ran_are_counted() {
    let home = Home::new();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let run = Fake::new("run", Access::Execute, Act::Holds);
    let mut tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    tools.push(Arc::clone(&run) as Arc<dyn Tool>);
    let tools = Catalog::new(tools).expect("合写法");
    let plays = vec![Play::calls(&[("run", "{}"), ("run", "{}")])];
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "\n\n  跑两条  \n别的").await;
    until("第一条开始跑", || run.calls().len() == 1).await;
    let stop = json!({"session": session, "queued": "return"});
    client.call("c3", "session.interrupt", stop).await;
    home.until_turns(&session, 1).await;
    let result = undo(&mut client, "c4", &session).await;
    assert_eq!(result["commands"], json!(1), "{result}");
    assert_eq!(result["said"], json!("跑两条"), "{result}");
}

/// 人说的全是空白：没有 `said`（施工 4-9 再补一：以前是空字符串）。
#[tokio::test]
async fn blank_words_give_no_said() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let plays = vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write("new\n"),
        Play::Says("改好了。"),
    ];
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "  \n\t\n ").await;
    home.until_turns(&session, 1).await;
    let result = undo(&mut client, "c3", &session).await;
    assert!(result.get("said").is_none(), "{result}");
    assert_eq!(result["turns"], json!(1), "{result}");
}
