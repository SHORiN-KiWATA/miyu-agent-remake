//! 分块读 `blob.get`、`fs.read`（施工 W-6，`docs/blueprint/web-module.md`「七、分块读」）：读一段、读到结尾就
//! 停、`offset` 过了结尾是空的、`length` 写 0 只问大小；没有这个 blob `unknown_blob`；`fs.read` 的路径落在
//! 数据根里 `path_forbidden`、工作区里能读、相对的 `bad_params`、`~` 接家目录、不是普通文件或者没有
//! `path_unreadable`；`length` 超过 512 KiB `bad_params`。

mod support;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_session::testkit::Script;

use support::*;

/// 一块最多多少字节（`web-module.md`「怎么走」第七条第 3 款，`fs.read`、`blob.get` 同一个数）。
const MAX_LENGTH: u64 = 512 * 1024;

/// 连上、握手（中文），交回客户端。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 在工作目录里写一个文件，交回它的绝对路径。
fn write(home: &Home, name: &str, bytes: &[u8]) -> String {
    let path = home.work.join(name);
    std::fs::write(&path, bytes).expect("写得了");
    path.to_string_lossy().into_owned()
}

/// 一份有规律、不会和常见长度撞上周期的内容：第 i 个字节是 `i % 251`。
fn pattern(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

/// 回应里的 `data`，解出 base64。
fn data(reply: &Value) -> Vec<u8> {
    STANDARD
        .decode(reply["result"]["data"].as_str().expect("data 是字符串"))
        .expect("data 是 base64")
}

#[tokio::test]
async fn blob_get_reads_a_segment_stops_at_the_end_and_past_the_end_is_empty() {
    let home = Home::new();
    let mut client = client(&home).await;
    let content = b"0123456789".to_vec();
    let put = client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(&content), "name": "a.bin"}),
        )
        .await;
    let blob = put["result"]["blob"].as_str().expect("blob").to_string();

    let reply = client
        .call(
            "g1",
            "blob.get",
            json!({"blob": blob, "offset": 3, "length": 4}),
        )
        .await;
    assert_eq!(data(&reply), b"3456");
    assert_eq!(reply["result"]["size"], json!(10));

    let reply = client
        .call(
            "g2",
            "blob.get",
            json!({"blob": blob, "offset": 8, "length": 100}),
        )
        .await;
    assert_eq!(data(&reply), b"89", "length 超过剩下的，读到结尾就停");
    assert_eq!(reply["result"]["size"], json!(10));

    let reply = client
        .call("g3", "blob.get", json!({"blob": blob, "offset": 20}))
        .await;
    assert_eq!(data(&reply), Vec::<u8>::new(), "offset 过了结尾是空的");
    assert_eq!(reply["result"]["size"], json!(10));

    let reply = client
        .call(
            "g4",
            "blob.get",
            json!({"blob": blob, "offset": 5, "length": 0}),
        )
        .await;
    assert_eq!(data(&reply), Vec::<u8>::new(), "length 写 0 只问大小");
    assert_eq!(reply["result"]["size"], json!(10));
}

#[tokio::test]
async fn blob_get_without_offset_or_length_reads_from_the_start_up_to_512kib() {
    let home = Home::new();
    let mut client = client(&home).await;
    let content = pattern(MAX_LENGTH as usize + 100);
    let path = write(&home, "big.bin", &content);
    let put = client.call("p", "blob.put", json!({"path": path})).await;
    let blob = put["result"]["blob"].as_str().expect("blob").to_string();

    let reply = client.call("g1", "blob.get", json!({"blob": blob})).await;
    assert_eq!(
        data(&reply),
        content[..MAX_LENGTH as usize],
        "不写 length 读满 512 KiB"
    );
    assert_eq!(reply["result"]["size"], json!(content.len()));

    let reply = client
        .call(
            "g2",
            "blob.get",
            json!({"blob": blob, "offset": MAX_LENGTH}),
        )
        .await;
    assert_eq!(
        data(&reply),
        content[MAX_LENGTH as usize..],
        "接着读剩下的字节"
    );
}

#[tokio::test]
async fn blob_get_of_an_unknown_blob_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let hash = ContentHash::of(b"never stored");
    let reply = client
        .call("g1", "blob.get", json!({"blob": hash.as_str()}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_blob"), "{reply}");
}

#[tokio::test]
async fn blob_get_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let put = client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(b"x"), "name": "a.bin"}),
        )
        .await;
    let blob = put["result"]["blob"].as_str().expect("blob").to_string();
    for params in [
        json!({}),
        json!({"blob": "not-a-hash"}),
        json!({"blob": blob, "length": MAX_LENGTH + 1}),
        json!({"blob": blob, "offset": "0"}),
        json!({"blob": blob, "length": -1}),
    ] {
        let reply = client.call("g", "blob.get", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
}

#[tokio::test]
async fn fs_read_rejects_the_data_root_and_lets_the_workspace_through() {
    let home = Home::new();
    home.root
        .prepare_home(&alice())
        .expect("建得了家目录、工作区");
    let workspace = std::fs::canonicalize(home.root.workspace(&alice())).expect("工作区在");
    let mut client = client(&home).await;

    let content = b"hello workspace".to_vec();
    let mine = workspace.join("mine.txt");
    std::fs::write(&mine, &content).expect("写得进");
    let reply = client
        .call("r1", "fs.read", json!({"path": mine.display().to_string()}))
        .await;
    assert_eq!(data(&reply), content, "{reply}");
    assert_eq!(reply["result"]["size"], json!(content.len()));

    // 数据根里、不是账号的工作区：不给。
    let inside = home.root.path().join("home").join("alice").join("note.txt");
    std::fs::create_dir_all(inside.parent().expect("有上级")).expect("建得了目录");
    std::fs::write(&inside, b"secret").expect("写得进");
    let reply = client
        .call(
            "r2",
            "fs.read",
            json!({"path": inside.display().to_string()}),
        )
        .await;
    assert_eq!(reason(&reply), Some("path_forbidden"), "{reply}");
}

#[tokio::test]
async fn fs_read_a_relative_path_is_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = client.call("r1", "fs.read", json!({"path": "a.txt"})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    // 没有系统的家目录：`~` 换不成真实的位置，不是「相对的」。
    let reply = client
        .call("r2", "fs.read", json!({"path": "~/a.txt"}))
        .await;
    assert_eq!(reason(&reply), Some("path_unreadable"), "{reply}");
}

#[tokio::test]
async fn fs_read_a_tilde_path_expands_to_the_system_home() {
    let home = Home::new();
    // 系统的家目录放在 `home.work` 底下：`Home` 自己的 `Drop` 会把它整个删掉，不用另起一份要自己清的临时目录。
    let sys_home = home.work.join("sys-home");
    std::fs::create_dir_all(&sys_home).expect("建得了目录");
    std::fs::write(sys_home.join("a.txt"), b"home file").expect("写得进");
    let mut client = Client::connect(home.core_at_home(&Script::new([]), sys_home));
    client.hello().await;
    let reply = client
        .call("r1", "fs.read", json!({"path": "~/a.txt"}))
        .await;
    assert_eq!(data(&reply), b"home file", "{reply}");
}

#[tokio::test]
async fn fs_read_unreadable_targets_are_path_unreadable() {
    let home = Home::new();
    let mut client = client(&home).await;
    let missing = home.work.join("missing.txt");
    let reply = client
        .call(
            "r1",
            "fs.read",
            json!({"path": missing.display().to_string()}),
        )
        .await;
    assert_eq!(reason(&reply), Some("path_unreadable"), "没有：{reply}");

    let reply = client
        .call(
            "r2",
            "fs.read",
            json!({"path": home.work.display().to_string()}),
        )
        .await;
    assert_eq!(reason(&reply), Some("path_unreadable"), "目录：{reply}");

    #[cfg(unix)]
    {
        // 套接字建在 /tmp 下的短目录里：macOS 上套接字的路径最长 104 字节，测试的工作目录太长。
        let short = std::path::PathBuf::from(format!("/tmp/miyu-w6-{}", std::process::id()));
        std::fs::create_dir_all(&short).expect("建得了目录");
        let path = short.join("sock");
        #[expect(
            clippy::let_underscore_must_use,
            reason = "上一次留下的、删不掉的都不要紧：bind 失败会在下一行报出来"
        )]
        let _ = std::fs::remove_file(&path);
        let _listening = std::os::unix::net::UnixListener::bind(&path).expect("绑得上");
        let reply = client
            .call("r3", "fs.read", json!({"path": path.display().to_string()}))
            .await;
        assert_eq!(reason(&reply), Some("path_unreadable"), "套接字：{reply}");
        drop(_listening);
        #[expect(
            clippy::let_underscore_must_use,
            reason = "删不掉就留在 /tmp 里，不影响测试"
        )]
        let _ = std::fs::remove_dir_all(&short);
    }
}

#[tokio::test]
async fn fs_read_length_over_512kib_is_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let path = write(&home, "a.txt", b"x");
    let reply = client
        .call(
            "r1",
            "fs.read",
            json!({"path": path, "length": MAX_LENGTH + 1}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn refusals_are_in_the_heads_language() {
    let home = Home::new();
    let mut english = Client::connect(home.core(&Script::new([])));
    english.hello_without_input().await;
    let hash = ContentHash::of(b"never stored");
    let reply = english
        .call("e1", "blob.get", json!({"blob": hash.as_str()}))
        .await;
    assert_eq!(reply["error"]["message"], "This content cannot be found.");
}
