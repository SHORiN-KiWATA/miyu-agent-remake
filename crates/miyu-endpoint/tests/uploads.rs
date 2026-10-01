//! 分块上传 `blob.open`、`blob.write`、`blob.close`（施工 W-5，`docs/blueprint/web-module.md`「六、分块上传」）：
//! 传完和 `blob.put` 同一个回应、同一个 blob；接不上回 `upload_offset`；没收齐不收；别的连接用不了；断开、
//! 60 秒不写作废并删暂存；超过 20 MiB 当场拒；同时最多 4 个；参数不对的几种；图片照 `blob.put` 的规矩认；
//! 核心起来时清掉留下的暂存。

mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use support::*;

/// 一份核心，分块上传多久没写就作废照 `idle`（施工 W-5）：测试里设短的，不用真等 60 秒。`support/mod.rs`
/// 已经在门禁的行数上限上，这个只有这里用得上的构造就不往那张共用的表里加了。
fn core_with_upload_idle(home: &Home, idle: Duration) -> Arc<Core> {
    Arc::new(
        Core::new(
            home.root.clone(),
            ResourceRoot::at(default_resources()),
            Arc::new(Script::new([])),
            Catalog::default(),
            None,
            alice(),
            TOKEN.to_string(),
        )
        .with_upload_idle(idle),
    )
}

/// 一张 `width` × `height` 的 PNG 的开头：签名和 IHDR，量宽高只看它（和 `tests/attach.rs` 的同名函数一样，
/// 测试二进制各自独立，不共用）。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 连上、握手（中文），交回客户端。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 两个客户端连着同一份核心（和「两次 `home.core(...)`」不一样：那是两份独立的家底，像核心重启过）。
async fn two_clients_on_one_core(home: &Home) -> (Client, Client) {
    let core = home.core(&Script::new([]));
    let mut a = Client::connect(Arc::clone(&core));
    a.hello().await;
    let mut b = Client::connect(core);
    b.hello().await;
    (a, b)
}

async fn open_upload(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "blob.open", params).await
}

async fn write_chunk(
    client: &mut Client,
    id: &str,
    upload: &str,
    offset: u64,
    data: &[u8],
) -> Value {
    client
        .call(
            id,
            "blob.write",
            json!({"upload": upload, "offset": offset, "data": STANDARD.encode(data)}),
        )
        .await
}

async fn close_upload(client: &mut Client, id: &str, upload: &str) -> Value {
    client
        .call(id, "blob.close", json!({"upload": upload}))
        .await
}

async fn put_via_data(client: &mut Client, content: &[u8]) -> Value {
    client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(content), "name": "once.bin"}),
        )
        .await
}

/// 读回应的 `result.upload`，要是一个字符串。
fn upload_id(reply: &Value) -> String {
    reply["result"]["upload"]
        .as_str()
        .unwrap_or_else(|| panic!("应该开出一个上传：{reply}"))
        .to_string()
}

/// `tmp/` 里几个文件。
fn tmp_count(home: &Home) -> usize {
    fs::read_dir(home.root.blobs(&alice()).join("tmp"))
        .map(|entries| entries.count())
        .unwrap_or(0)
}

/// 开一个上传，按 512 KiB 一块写完，收齐了关掉，交回 `blob.close` 的回应。
async fn chunked_put(
    client: &mut Client,
    name: &str,
    media_type: Option<&str>,
    content: &[u8],
) -> Value {
    const CHUNK: usize = 512 * 1024;
    let open = open_upload(
        client,
        "open",
        json!({"name": name, "media_type": media_type, "size": content.len() as u64}),
    )
    .await;
    let upload = upload_id(&open);
    let mut sent = 0usize;
    while sent < content.len() {
        let end = (sent + CHUNK).min(content.len());
        let reply = write_chunk(client, "write", &upload, sent as u64, &content[sent..end]).await;
        assert_eq!(reply["result"]["received"], json!(end as u64), "{reply}");
        sent = end;
    }
    close_upload(client, "close", &upload).await
}

#[tokio::test]
async fn a_chunked_upload_matches_blob_put_for_the_same_content() {
    let home = Home::new();
    let mut client = client(&home).await;
    let picture = png(640, 480);
    let reply = chunked_put(&mut client, "shot.png", None, &picture).await;
    let blob = ContentHash::of(&picture);
    assert_eq!(
        reply["result"],
        json!({"blob": blob.as_str(), "height": 480, "kind": "image", "media_type": "image/png", "name": "shot.png", "width": 640}),
        "{reply}"
    );
    let stored = Blobs::new(home.root.blobs(&alice()));
    assert_eq!(stored.get(&blob).unwrap(), picture);
    assert_eq!(tmp_count(&home), 0, "暂存改名走了");
}

#[tokio::test]
async fn closing_onto_content_already_there_discards_the_temp_file() {
    let home = Home::new();
    let mut client = client(&home).await;
    let content = b"same content, two different ways in".to_vec();
    let put_reply = put_via_data(&mut client, &content).await;
    let blob = put_reply["result"]["blob"].as_str().unwrap().to_string();
    let close_reply = chunked_put(&mut client, "again.bin", None, &content).await;
    assert_eq!(close_reply["result"]["blob"], json!(blob), "{close_reply}");
    assert_eq!(tmp_count(&home), 0, "暂存删掉了");
    let stored = Blobs::new(home.root.blobs(&alice()));
    let fan = stored.path(&ContentHash::of(&content));
    assert_eq!(
        fs::read_dir(fan.parent().unwrap()).unwrap().count(),
        1,
        "没有存出第二份"
    );
}

#[tokio::test]
async fn an_offset_that_does_not_match_what_was_received_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 10u64})).await;
    let upload = upload_id(&open);
    let first = write_chunk(&mut client, "w1", &upload, 0, b"hello").await;
    assert_eq!(first["result"]["received"], json!(5), "{first}");
    let bad = write_chunk(&mut client, "w2", &upload, 2, b"xx").await;
    assert_eq!(reason(&bad), Some("upload_offset"), "{bad}");
    assert_eq!(bad["error"]["data"]["received"], json!(5), "{bad}");
    // 从核心说的地方接着传，还能成。
    let fixed = write_chunk(&mut client, "w3", &upload, 5, b"world").await;
    assert_eq!(fixed["result"]["received"], json!(10), "{fixed}");
}

#[tokio::test]
async fn closing_before_everything_arrived_is_refused_and_more_can_still_be_written() {
    let home = Home::new();
    let mut client = client(&home).await;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 8u64})).await;
    let upload = upload_id(&open);
    write_chunk(&mut client, "w1", &upload, 0, b"abcd").await;
    let early = close_upload(&mut client, "c1", &upload).await;
    assert_eq!(reason(&early), Some("upload_incomplete"), "{early}");
    assert_eq!(early["error"]["data"]["received"], json!(4), "{early}");
    write_chunk(&mut client, "w2", &upload, 4, b"efgh").await;
    let done = close_upload(&mut client, "c2", &upload).await;
    assert_eq!(done["result"]["kind"], "file", "{done}");
    let blob = ContentHash::of(b"abcdefgh");
    assert_eq!(done["result"]["blob"], json!(blob.as_str()));
    // 两块真的接在一起落了盘，不是光报对了哈希：第二块真的写在第一块后面，不是从头盖过去的。
    let stored = Blobs::new(home.root.blobs(&alice()));
    assert_eq!(stored.get(&blob).unwrap(), b"abcdefgh");
}

#[tokio::test]
async fn another_connections_upload_id_cannot_be_used() {
    let home = Home::new();
    let (mut a, mut b) = two_clients_on_one_core(&home).await;
    let open = open_upload(&mut a, "o1", json!({"name": "f.bin", "size": 4u64})).await;
    let upload = upload_id(&open);
    let reply = write_chunk(&mut b, "w1", &upload, 0, b"ab").await;
    assert_eq!(reason(&reply), Some("upload_unknown"), "{reply}");
    // 开它的那个连接还能照常用。
    let own = write_chunk(&mut a, "w2", &upload, 0, b"abcd").await;
    assert_eq!(own["result"]["received"], json!(4), "{own}");
}

#[tokio::test]
async fn an_idle_upload_expires_and_its_temp_file_is_removed() {
    let home = Home::new();
    let idle = Duration::from_millis(80);
    let core = core_with_upload_idle(&home, idle);
    let mut client = Client::connect(core);
    client.hello().await;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 4u64})).await;
    let upload = upload_id(&open);
    assert_eq!(tmp_count(&home), 1, "暂存文件建好了");
    tokio::time::sleep(idle + Duration::from_millis(40)).await;
    let reply = write_chunk(&mut client, "w1", &upload, 0, b"ab").await;
    assert_eq!(reason(&reply), Some("upload_unknown"), "{reply}");
    assert_eq!(tmp_count(&home), 0, "暂存删掉了");
}

#[tokio::test]
async fn disconnecting_discards_open_uploads_and_their_temp_files() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 4u64})).await;
    assert_eq!(tmp_count(&home), 1);
    drop(client);
    until("断开以后暂存清掉", || tmp_count(&home) == 0).await;
}

#[tokio::test]
async fn a_size_over_the_limit_is_refused_at_open() {
    let home = Home::new();
    let mut client = client(&home).await;
    let limit = 20 * 1024 * 1024u64;
    let ok = open_upload(&mut client, "o1", json!({"name": "a.bin", "size": limit})).await;
    assert!(
        ok["result"]["upload"].is_string(),
        "正好 20 MiB 开得成：{ok}"
    );
    let too_big = open_upload(
        &mut client,
        "o2",
        json!({"name": "b.bin", "size": limit + 1}),
    )
    .await;
    assert_eq!(reason(&too_big), Some("attachment_too_big"), "{too_big}");
    assert_eq!(tmp_count(&home), 1, "太大的那次没留暂存");
}

#[tokio::test]
async fn a_fifth_concurrent_upload_is_refused_until_one_closes() {
    let home = Home::new();
    let mut client = client(&home).await;
    let mut ids = Vec::new();
    for i in 0..4 {
        let reply = open_upload(
            &mut client,
            &format!("o{i}"),
            json!({"name": format!("f{i}.bin"), "size": 1u64}),
        )
        .await;
        ids.push(upload_id(&reply));
    }
    let fifth = open_upload(&mut client, "o4", json!({"name": "f4.bin", "size": 1u64})).await;
    assert_eq!(reason(&fifth), Some("too_many_uploads"), "{fifth}");
    write_chunk(&mut client, "w0", &ids[0], 0, b"x").await;
    close_upload(&mut client, "c0", &ids[0]).await;
    let now_ok = open_upload(&mut client, "o5", json!({"name": "f5.bin", "size": 1u64})).await;
    assert!(
        now_ok["result"]["upload"].is_string(),
        "关了一个腾出位置：{now_ok}"
    );
}

#[tokio::test]
async fn data_that_is_not_base64_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 4u64})).await;
    let upload = upload_id(&open);
    let reply = client
        .call(
            "w1",
            "blob.write",
            json!({"upload": upload, "offset": 0, "data": "not base64!"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn a_chunk_over_the_block_limit_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let size = 600 * 1024u64;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": size})).await;
    let upload = upload_id(&open);
    let chunk = vec![0u8; 512 * 1024 + 1];
    let reply = write_chunk(&mut client, "w1", &upload, 0, &chunk).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn a_chunk_that_would_overflow_the_declared_size_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let open = open_upload(&mut client, "o1", json!({"name": "f.bin", "size": 4u64})).await;
    let upload = upload_id(&open);
    let reply = write_chunk(&mut client, "w1", &upload, 0, b"abcdef").await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn an_oversized_image_is_refused_like_blob_put_and_leaves_no_temp_file() {
    let home = Home::new();
    let mut client = client(&home).await;
    let wide = png(8001, 10);
    let reply = chunked_put(&mut client, "wide.png", None, &wide).await;
    assert_eq!(reason(&reply), Some("attachment_too_big"), "{reply}");
    assert_eq!(tmp_count(&home), 0, "{reply}");
}

/// 核心起来时清暂存（`web-module.md`「怎么走」第六条第 6 款）的底层能力：实际在启动时调用它的是
/// `crates/miyu-core/src/packages.rs::clear_uploads`（它在更上一层的 crate，`miyu-endpoint` 够不到），
/// 这里直接验 `miyu-store` 交出的这个方法；`crates/miyu-core/tests/packages.rs` 另有一条测调用它的那一层。
#[tokio::test]
async fn clear_uploads_removes_leftover_temp_files_without_touching_others() {
    let home = Home::new();
    let blobs = Blobs::new(home.root.blobs(&alice()));
    blobs.create_upload("leftover1").unwrap();
    blobs.create_upload("leftover2").unwrap();
    // `blob.put` 自己崩了留下的那种（数字加连字符）：clear_uploads 不该碰它。
    let tmp = home.root.blobs(&alice()).join("tmp");
    fs::write(tmp.join("4242-0"), b"half a put").unwrap();
    let removed = blobs.clear_uploads().unwrap();
    assert_eq!(removed, 2);
    let left: Vec<_> = fs::read_dir(&tmp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(left, vec!["4242-0".to_string()], "{left:?}");
}
