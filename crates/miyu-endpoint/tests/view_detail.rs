//! `view.detail`：一次调用的完整差异（施工 9-6 三补，`docs/construction/9-6-一次调用的完整差异（三补）.md`）。改了两处的整份
//! 交、带行号、不截 20 行；新建的文件改前当空的；太大的不算差异、说原因；读文件的调用没有文件；写错的、没有的会话、没有的
//! 调用照原因拒。

use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 她照 `plays` 走一轮，交回客户端和会话。
async fn ran(home: &Home, plays: Vec<Play>) -> (Client, String) {
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

/// 日志里的工具结果，照先后：调用编号和有没有改文件。
fn calls(home: &Home, session: &str) -> Vec<(String, bool)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some((
                result.call_id.to_string(),
                result
                    .effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::FileChanged(_))),
            )),
            _ => None,
        })
        .collect()
}

/// 写一次 `a.txt`，内容是 `content`。
fn write(content: &str) -> Play {
    Play::calls(&[(
        "write",
        &json!({"file_path": "a.txt", "content": content}).to_string(),
    )])
}

async fn detail(client: &mut Client, session: &str, call: &str) -> Value {
    client
        .call(
            "d",
            "view.detail",
            json!({"session": session, "call": call}),
        )
        .await
}

fn numbered(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

#[tokio::test]
async fn a_change_comes_whole_with_line_numbers() {
    let home = Home::new();
    let old: Vec<String> = (1..=40).map(|n| format!("line {n}")).collect();
    std::fs::write(
        home.work.join("a.txt"),
        numbered(&old.iter().map(String::as_str).collect::<Vec<_>>()),
    )
    .unwrap();
    let mut new = old.clone();
    new[2] = "LINE 3".to_string();
    for n in 15..=25 {
        new[n - 1] = format!("LINE {n}");
    }
    new[34] = "LINE 35".to_string();
    let new = numbered(&new.iter().map(String::as_str).collect::<Vec<_>>());
    let plays = vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write(&new),
        Play::Says("改好了。"),
    ];
    let (mut client, session) = ran(&home, plays).await;
    let calls = calls(&home, &session);
    let (read, wrote) = (&calls[0], &calls[1]);
    assert!(!read.1 && wrote.1, "{calls:?}");
    let reply = detail(&mut client, &session, &wrote.0).await;
    let files = reply["result"]["files"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(files.len(), 1, "{reply}");
    let file = &files[0];
    assert_eq!(file["action"], "write");
    assert!(
        file["path"].as_str().unwrap_or_default().ends_with("a.txt"),
        "{file}"
    );
    let diff: Vec<&str> = file["diff"]
        .as_array()
        .expect("有差异")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(diff[0], "@@ -1,6 +1,6 @@", "{diff:?}");
    assert!(
        diff.contains(&"-line 3") && diff.contains(&"+LINE 3"),
        "{diff:?}"
    );
    assert!(
        diff.contains(&"@@ -12,17 +12,17 @@"),
        "第二段带行号：{diff:?}"
    );
    assert!(
        diff.contains(&"@@ -32,7 +32,7 @@"),
        "第三段带行号：{diff:?}"
    );
    assert!(diff.contains(&"+LINE 35"), "{diff:?}");
    assert!(diff.len() > 20, "整份交、不截 20 行：{}", diff.len());
    assert_eq!((&file["added"], &file["removed"]), (&json!(13), &json!(13)));
    assert!(file.get("skipped").is_none(), "{file}");
    let reply = detail(&mut client, &session, &read.0).await;
    assert_eq!(reply["result"]["files"], json!([]), "读文件的不算");
}

#[tokio::test]
async fn a_new_file_starts_from_nothing_and_a_big_one_says_why() {
    let home = Home::new();
    let big = "x".repeat(1_100_000);
    let plays = vec![write("a\nb\n"), write(&big), Play::Says("好了。")];
    let (mut client, session) = ran(&home, plays).await;
    let calls = calls(&home, &session);
    let reply = detail(&mut client, &session, &calls[0].0).await;
    let file = &reply["result"]["files"][0];
    assert_eq!(
        file["diff"],
        json!(["@@ -0,0 +1,2 @@", "+a", "+b"]),
        "{reply}"
    );
    assert_eq!((&file["added"], &file["removed"]), (&json!(2), &json!(0)));
    let reply = detail(&mut client, &session, &calls[1].0).await;
    let file = &reply["result"]["files"][0];
    assert_eq!(file["skipped"], "too_big", "{reply}");
    assert!(
        file.get("diff").is_none() && file.get("added").is_none(),
        "{file}"
    );
}

#[tokio::test]
async fn a_wrong_id_an_unknown_session_and_an_unknown_call_are_refused() {
    let home = Home::new();
    let (mut client, session) = ran(&home, vec![write("a\n"), Play::Says("好了。")]).await;
    let reply = detail(&mut client, &session, "nope").await;
    assert_eq!(reply["error"]["code"], json!(-32602), "{reply}");
    let reply = detail(&mut client, &session, "call_999_1").await;
    assert_eq!(reason(&reply), Some("unknown_call"), "{reply}");
    let missing = "01900000-0000-7000-8000-00000000abcd";
    let reply = detail(&mut client, missing, "call_999_1").await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}
