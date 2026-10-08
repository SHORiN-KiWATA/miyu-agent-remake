//! 传附件 `blob.put`（施工 3-9 三补，`docs/blueprint/protocol.md` 的 `blob.put`）：传路径、传内容两种；照内容认图片、PDF、
//! 文本、别的文件，图片量宽高；太大的、数据根里的、读不了的各有各的拒绝；参数不对的几种。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;

use crate::support::*;

/// 一张 `width` × `height` 的 PNG 的开头：签名和 IHDR，量宽高只看它。
pub fn png(width: u32, height: u32) -> Vec<u8> {
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

/// 在工作目录里写一个文件，交回它的绝对路径。
fn write(home: &Home, name: &str, bytes: &[u8]) -> String {
    let path = home.work.join(name);
    std::fs::write(&path, bytes).expect("写得了");
    path.to_string_lossy().into_owned()
}

async fn put(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "blob.put", params).await
}

#[tokio::test]
async fn a_path_is_read_stored_and_told_apart_by_its_content() {
    let home = Home::new();
    let mut client = client(&home).await;
    let picture = png(800, 600);
    // 扩展名说是 JPEG、其实是 PNG：照内容认。
    let path = write(&home, "shot.jpg", &picture);
    let reply = put(&mut client, "b1", json!({"path": path})).await;
    let blob = ContentHash::of(&picture);
    assert_eq!(
        reply["result"],
        json!({"blob": blob.as_str(), "height": 600, "kind": "image", "media_type": "image/png", "name": "shot.jpg", "width": 800}),
        "{reply}"
    );
    let line = serde_json::to_string(&reply).unwrap();
    assert!(
        line.contains(r#""result":{"blob":"#) && line.contains(r#""name":"shot.jpg","width":800}"#),
        "格照名字的字母先后排：{line}"
    );
    let stored = Blobs::new(home.root.blobs(&alice()));
    assert_eq!(
        stored.get(&blob).expect("存下了"),
        picture,
        "存成管理员的 blob"
    );
    // PDF、文本、别的文件：都是文件，媒体类型照内容认。
    for (name, bytes, media_type) in [
        (
            "报告.pdf",
            &b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"[..],
            "application/pdf",
        ),
        ("notes.md", "# 待办\n".as_bytes(), "text/plain"),
        ("data.bin", b"\x00\x01\x02", "application/octet-stream"),
    ] {
        let path = write(&home, name, bytes);
        let reply = put(&mut client, "b2", json!({"path": path})).await;
        assert_eq!(
            reply["result"],
            json!({"blob": ContentHash::of(bytes).as_str(), "kind": "file", "media_type": media_type, "name": name}),
            "{name}：{reply}"
        );
    }
}

#[tokio::test]
async fn data_comes_with_a_name_and_a_media_type_counts_unless_the_content_says_otherwise() {
    let home = Home::new();
    let mut client = client(&home).await;
    let csv = b"a,b\n1,2\n";
    let reply = put(
        &mut client,
        "b1",
        json!({"data": STANDARD.encode(csv), "name": "表.csv", "media_type": "text/csv"}),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"blob": ContentHash::of(csv).as_str(), "kind": "file", "media_type": "text/csv", "name": "表.csv"}),
        "{reply}"
    );
    // 写成 PDF、图片的，内容不是就不算。
    let reply = put(
        &mut client,
        "b2",
        json!({"data": STANDARD.encode(csv), "name": "a.pdf", "media_type": "application/pdf"}),
    )
    .await;
    assert_eq!(reply["result"]["media_type"], "text/plain", "{reply}");
    // 路径上也能改名、写媒体类型；图片写了也不算。
    let path = write(&home, "a.png", &png(2, 3));
    let reply = put(
        &mut client,
        "b3",
        json!({"path": path, "name": "截图.png", "media_type": "image/gif"}),
    )
    .await;
    assert_eq!(reply["result"]["name"], "截图.png", "{reply}");
    assert_eq!(reply["result"]["media_type"], "image/png", "{reply}");
    // 写 `null` 等于没写。
    let reply = put(
        &mut client,
        "b4",
        json!({"path": path, "data": null, "name": null, "media_type": null}),
    )
    .await;
    assert_eq!(reply["result"]["name"], "a.png", "{reply}");
}

#[tokio::test]
async fn too_big_is_refused_and_the_limits_themselves_are_taken() {
    let home = Home::new();
    let mut client = client(&home).await;
    let limit = 20 * 1024 * 1024;
    let exact = write(&home, "exact.bin", &vec![0u8; limit]);
    let reply = put(&mut client, "b1", json!({"path": exact})).await;
    assert_eq!(reply["result"]["kind"], "file", "正好 20 MiB 的收：{reply}");
    let over = write(&home, "over.bin", &vec![0u8; limit + 1]);
    let reply = put(&mut client, "b2", json!({"path": over})).await;
    assert_eq!(reason(&reply), Some("attachment_too_big"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        "附件太大：一个最多 20 MiB，图片最多 5 MiB、每边最多 8000 像素。"
    );
    // 图片照图片的上限：宽高、大小。
    let wide = write(&home, "wide.png", &png(8001, 10));
    let reply = put(&mut client, "b3", json!({"path": wide})).await;
    assert_eq!(reason(&reply), Some("attachment_too_big"), "{reply}");
    let mut heavy = png(10, 10);
    heavy.resize(5 * 1024 * 1024 + 1, 0);
    let path = write(&home, "heavy.png", &heavy);
    let reply = put(&mut client, "b4", json!({"path": path})).await;
    assert_eq!(reason(&reply), Some("attachment_too_big"), "{reply}");
    heavy.pop();
    let path = write(&home, "fits.png", &heavy);
    let reply = put(&mut client, "b5", json!({"path": path})).await;
    assert_eq!(
        reply["result"]["kind"], "image",
        "正好 5 MiB 的图收：{reply}"
    );
    // 传内容的照同样的上限认（一行 JSON 最长 1 MiB，传不了 20 MiB 那么大的，只验太宽的图）。
    let reply = put(
        &mut client,
        "b6",
        json!({"data": STANDARD.encode(png(9000, 1)), "name": "wide.png"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("attachment_too_big"), "{reply}");
}

#[tokio::test]
async fn files_in_the_data_root_are_not_given_but_the_workspace_is() {
    let home = Home::new();
    let mut client = client(&home).await;
    let inside = home.root.path().join("home").join("alice").join("note.txt");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, b"secret").unwrap();
    let reply = put(&mut client, "b1", json!({"path": inside})).await;
    assert_eq!(reason(&reply), Some("attachment_in_data_root"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        "Miyu 的数据根里的文件不能当附件。"
    );
    // 管理员的工作区在数据根里，照边界表是能读能写的那一片：给。
    let workspace = home.root.workspace(&alice());
    std::fs::create_dir_all(&workspace).unwrap();
    let mine = workspace.join("mine.txt");
    std::fs::write(&mine, b"mine").unwrap();
    let reply = put(&mut client, "b2", json!({"path": mine})).await;
    assert_eq!(reply["result"]["media_type"], "text/plain", "{reply}");
    // 链接照指向的地方算：工作目录里指到数据根里的链接，照样不给；指到别处的给，名字取头写的那一段。
    #[cfg(unix)]
    {
        let link = home.work.join("link.txt");
        std::os::unix::fs::symlink(&inside, &link).unwrap();
        let reply = put(&mut client, "b3", json!({"path": link})).await;
        assert_eq!(reason(&reply), Some("attachment_in_data_root"), "{reply}");
        let alias = home.work.join("alias.txt");
        std::os::unix::fs::symlink(&mine, &alias).unwrap();
        let reply = put(&mut client, "b4", json!({"path": alias})).await;
        assert_eq!(reply["result"]["name"], "alias.txt", "{reply}");
    }
}

#[tokio::test]
async fn what_cannot_be_read_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let missing = home.work.join("missing.txt");
    let reply = put(&mut client, "b1", json!({"path": missing})).await;
    assert_eq!(reason(&reply), Some("attachment_unreadable"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        "读不了这个文件：没有、不是普通文件，或者没有权限。"
    );
    let reply = put(&mut client, "b2", json!({"path": home.work})).await;
    assert_eq!(
        reason(&reply),
        Some("attachment_unreadable"),
        "目录：{reply}"
    );
    // 没有家目录的核心，`~` 换不成真实的位置。
    let reply = put(&mut client, "b3", json!({"path": "~/a.txt"})).await;
    assert_eq!(reason(&reply), Some("attachment_unreadable"), "{reply}");
}

#[tokio::test]
async fn params_that_do_not_fit_are_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let path = write(&home, "a.txt", b"a");
    let data = STANDARD.encode(b"a");
    for params in [
        json!({}),
        json!({"path": null, "data": null}),
        json!({"path": path, "data": data, "name": "a.txt"}),
        json!({"path": "a.txt"}),
        json!({"path": "~alice/a.txt"}),
        json!({"data": data}),
        json!({"data": "not base64!", "name": "a.txt"}),
        json!({"data": data, "name": "a/b.txt"}),
        json!({"data": data, "name": ""}),
        json!({"path": path, "media_type": "Text/Plain"}),
        json!({"path": path, "media_type": "text"}),
        json!({"path": 7}),
    ] {
        let reply = put(&mut client, "b1", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    assert!(
        std::fs::read_dir(home.root.blobs(&alice())).is_err(),
        "一个都没存"
    );
}

#[tokio::test]
async fn refusals_are_in_the_heads_language() {
    let home = Home::new();
    let mut english = Client::connect(home.core(&Script::new([])));
    english.hello_without_input().await;
    let missing = home.work.join("missing.txt");
    let reply = put(&mut english, "e1", json!({"path": missing})).await;
    assert_eq!(
        reply["error"]["message"],
        "This file cannot be read: it is missing, not a regular file, or not permitted."
    );
    let reply = english
        .call(
            "e2",
            "session.send",
            json!({"session": "0192f3a0-0000-7000-8000-000000000001", "text": "x",
                   "attachments": [{"blob": ContentHash::of(b"none").as_str(), "name": "a", "media_type": "text/plain"}]}),
        )
        .await;
    assert_eq!(
        reply["error"]["message"],
        "The attachment is not in the core; upload it with blob.put first."
    );
    let over = write(&home, "wide.png", &png(9000, 1));
    let reply = put(&mut english, "e3", json!({"path": over})).await;
    assert_eq!(
        reply["error"]["message"],
        "The attachment is too big: at most 20 MiB, and an image at most 5 MiB and 8000 pixels a side."
    );
    let inside = home.root.path().join("state").join("x.txt");
    std::fs::write(&inside, b"x").unwrap();
    let reply = put(&mut english, "e4", json!({"path": inside})).await;
    assert_eq!(
        reply["error"]["message"],
        "Files in Miyu's data root cannot be attached."
    );
}
