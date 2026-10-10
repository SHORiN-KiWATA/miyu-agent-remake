//! 多模态（一）（施工 O-33，`onebot.md` 第一条「平台工具（二）」）：真核心拉起真桥、假 NapCat 答 `get_msg`、`get_image`、
//! `get_file`、本机回环上的假 HTTP 服务器给图、她照台词说。
//!
//! - 近况那一行写大小、一条里不止一样的标第几个；冲她来的那条和它引用的那条的图到了她的请求里（url、base64 两条路都走到），
//!   旁听的不取；存成的 blob 拷进了会话属主（系统账号）名下。
//! - `fetch_media`：取图交回图片块；取文件、视频写进工作区的 `qq-files/` 回路径；编号不对、别的群的、第几个不对、语音和小黄脸、
//!   QQ 回失败、一轮第五次各回一句；同一个文件取两次换成新的；文件名里的 `../` 和开头的点洗掉；`qq-files` 不是真目录的不写。
//! - 私聊：只有图的照交、带图；取不到图又没有字的不送；工具面里没有 `fetch_media`。
//!
//! 她真的看得到：工具结果里的图片块，核心照调用它的会话属主拷 blob（核心 O-2 三补，2026-10-11 合进 main）：桥上传的存在管理员
//! 名下，系统账号（群会话的属主）名下也要有（`fetch_media_brings_back_pictures_and_saves_files`）。O-2 三补以前的核心上这一条是红的。

use std::path::PathBuf;

use serde_json::json;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::id::AccountId;

use crate::support::group::*;
use crate::support::media::*;
use crate::support::platform::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 假 HTTP 服务器回一次 `bytes`。
fn serving(bytes: &[u8]) -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "image/png".to_string())],
        body: vec![Piece::Bytes(bytes.to_vec())],
    }
}

/// 系统账号的工作区下的 `qq-files/`（真实位置）。
fn qq_files(home: &Home) -> PathBuf {
    let onebot = AccountId::parse("onebot").expect("合写法");
    std::fs::canonicalize(home.root.workspace(&onebot))
        .expect("工作区在")
        .join("qq-files")
}

#[tokio::test]
async fn a_message_calling_her_brings_its_pictures_and_the_quoted_ones() {
    let (picture, quoted) = (png(4, 3), png(2, 2));
    let server = Server::start(vec![serving(&picture)]).await;
    let lines = Lines::new([Line::says("看到了。")]);
    let (home, napcat) = up(&lines).await;
    let quoted_words = json!([plain("这张"), image("q.jpg", 2048)]);
    napcat.stock_message(500, in_group(quoted_words.clone()));
    napcat.stock_file(
        "q.jpg",
        Ok(json!({"file": "/napcat/elsewhere/q.jpg", "base64": format!("base64://{}", encoded(&quoted))})),
    );
    napcat.stock_file(
        "a.jpg",
        Ok(json!({"url": format!("{}/a.jpg", server.base_url), "file": "/napcat/elsewhere/a.jpg"})),
    );
    // 旁听的：不取。
    says(&home, &napcat, (MEMBER, "member"), 500, quoted_words).await;
    assert!(napcat.fetched().is_empty(), "{:?}", napcat.fetched());
    // 终端管理员引用它、@ 她，带一张图、一个小黄脸：两条的图都带上，引用的那一条的在前。
    let words = json!([
        quote(500),
        at(BOT),
        plain(" 看看"),
        image("a.jpg", 834_213),
        face("14", "/微笑")
    ]);
    says(&home, &napcat, (ADMIN, "member"), 501, words).await;
    results(&home, 1).await;
    let request = &lines.requests()[0];
    let seen = pictures(request);
    let names: Vec<&str> = seen.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["msg-500-1", "msg-501-1"], "{request:?}");
    let text = user_text(request);
    assert!(
        text.contains("[msg=501]: @米尤 看看 [image #1: 834 KB] [sticker #2: /微笑]"),
        "{text}"
    );
    assert!(text.contains("[msg=500]: 这张 [image: 2 KB]"), "{text}");
    assert!(has_blob(&home, "onebot", &picture), "拷进了会话属主名下");
    assert!(has_blob(&home, "onebot", &quoted));
    assert_eq!(
        napcat.fetched(),
        [
            ("get_msg".to_string(), "500".to_string()),
            ("get_image".to_string(), "q.jpg".to_string()),
            ("get_image".to_string(), "a.jpg".to_string()),
        ]
    );
    assert_eq!(server.received().len(), 1, "图照 url 下了一次");
    stopped(home).await;
}

#[tokio::test]
async fn fetch_media_brings_back_pictures_and_saves_files() {
    let picture = png(5, 5);
    let server = Server::start(vec![serving(&picture)]).await;
    let fetch = |args: &'static str| Line::uses("", "fetch_media", args);
    let lines = Lines::new([
        // 第一轮：图、文件、小黄脸、没有的编号、第五次。
        fetch(r#"{"msg":"601","index":1}"#),
        fetch(r#"{"msg":"602"}"#),
        fetch(r#"{"msg":"601","index":2}"#),
        fetch(r#"{"msg":"999"}"#),
        fetch(r#"{"msg":"601","index":3}"#),
        Line::says("好。"),
        // 第二轮：语音、第几个超了、别的群的、QQ 取不到。
        fetch(r#"{"msg":"603"}"#),
        fetch(r#"{"msg":"601","index":3}"#),
        fetch(r#"{"msg":"604"}"#),
        fetch(r#"{"msg":605}"#),
        Line::says("好。"),
        // 第三轮：同一个文件再取、名字带 `../` 和开头的点、视频。
        fetch(r#"{"msg":"602"}"#),
        fetch(r#"{"msg":"606"}"#),
        fetch(r#"{"msg":"607"}"#),
        Line::says("好。"),
        // 第四轮：`qq-files` 不是真目录了。
        fetch(r#"{"msg":"602"}"#),
        Line::says("好。"),
    ]);
    let (home, napcat) = up(&lines).await;
    let groups = [
        (601, json!([image("p.jpg", 4096), face("14", "/微笑")])),
        (602, json!([file("排班.pdf", "u-602", 12)])),
        (603, json!([voice("v.amr", 5321)])),
        (605, json!([file("旧.zip", "u-605", 99)])),
        (606, json!([file("../../.bashrc", "u-606", 3)])),
        (607, json!([video("vid-607", 12_345_678)])),
    ];
    for (id, words) in &groups {
        napcat.stock_message(*id, in_group(words.clone()));
        says(&home, &napcat, (MEMBER, "member"), *id, words.clone()).await;
    }
    let mut elsewhere = in_group(json!([image("p.jpg", 4096)]));
    elsewhere["group_id"] = json!(99);
    napcat.stock_message(604, elsewhere);
    napcat.stock_file(
        "p.jpg",
        Ok(json!({"url": format!("{}/p.jpg", server.base_url)})),
    );
    napcat.stock_file(
        "u-602",
        Ok(json!({"file_name": "排班.pdf", "base64": encoded(b"%PDF-1.4 one")})),
    );
    napcat.stock_file("u-605", Err("文件已过期"));
    napcat.stock_file("u-606", Ok(json!({"base64": encoded(b"rc")})));
    napcat.stock_file(
        "vid-607",
        Ok(json!({"file_name": "clip.mp4", "base64": encoded(b"movie")})),
    );
    let admin_asks = async |n: i64| {
        let words = json!([at(BOT), plain(" 看看")]);
        says(&home, &napcat, (ADMIN, "member"), n, words).await;
    };
    admin_asks(610).await;
    let first = results(&home, 1).await;
    let saved = qq_files(&home).join("602-1-排班.pdf");
    let saved_text = sentence("saved", &[("path", "qq-files/602-1-排班.pdf")]);
    assert_eq!(first.len(), 5, "{first:#?}");
    // 图：交回图片块，名字照消息编号和第几个；blob 桥传在管理员名下，核心照会话属主拷进系统账号名下：她看得到。
    let block = &first[0]["blocks"][0];
    assert_eq!(block["type"], "image", "{first:#?}");
    assert_eq!(block["name"], "msg-601-1");
    assert_eq!(
        (block["width"].as_u64(), block["height"].as_u64()),
        (Some(5), Some(5))
    );
    assert_eq!(first[0]["status"], "ok");
    assert!(has_blob(&home, "admin", &picture));
    assert!(
        has_blob(&home, "onebot", &picture),
        "核心照会话属主拷了 blob"
    );
    assert_eq!(
        (text_of(&first[1]), first[1]["status"].as_str()),
        (saved_text.clone(), Some("ok"))
    );
    assert_eq!(std::fs::read(&saved).expect("存下了"), b"%PDF-1.4 one");
    let refusals: Vec<(String, String)> = first[2..]
        .iter()
        .map(|result| {
            (
                text_of(result),
                result["status"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        refusals,
        [
            refused("not-fetchable", &[]),
            refused("not-found", &[]),
            refused("too-many", &[("count", "4")]),
        ]
    );
    admin_asks(611).await;
    let second = results(&home, 2).await;
    let refusals: Vec<(String, String)> = second[5..]
        .iter()
        .map(|result| {
            (
                text_of(result),
                result["status"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        refusals,
        [
            refused("not-fetchable", &[]),
            refused("no-item", &[("count", "2")]),
            refused("not-found", &[]),
            refused("failed", &[("detail", "文件已过期")]),
        ]
    );
    // 第三轮以前 QQ 那边的文件换了：再取照新的换掉，路径不变。
    napcat.stock_file(
        "u-602",
        Ok(json!({"file_name": "排班.pdf", "base64": encoded(b"%PDF-1.4 two")})),
    );
    admin_asks(612).await;
    let third = results(&home, 3).await;
    let dir = qq_files(&home);
    let texts: Vec<String> = third[9..].iter().map(text_of).collect();
    assert_eq!(
        texts,
        [
            saved_text,
            sentence("saved", &[("path", "qq-files/606-1-bashrc")]),
            sentence("saved", &[("path", "qq-files/607-1-clip.mp4")]),
        ]
    );
    assert_eq!(std::fs::read(&saved).unwrap(), b"%PDF-1.4 two");
    assert_eq!(std::fs::read(dir.join("606-1-bashrc")).unwrap(), b"rc");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["602-1-排班.pdf", "606-1-bashrc", "607-1-clip.mp4"],
        "没有留下临时文件"
    );
    // `qq-files` 换成一个文件：不写，回一句。
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(&dir, b"not a folder").unwrap();
    admin_asks(613).await;
    let fourth = results(&home, 4).await;
    let last = &fourth[12];
    assert_eq!(last["status"], "error", "{last}");
    assert!(
        text_of(last).starts_with(sentence("not-saved", &[("detail", "")]).trim_end()),
        "{last}"
    );
    assert_eq!(std::fs::read(&dir).unwrap(), b"not a folder");
    stopped(home).await;
}

#[tokio::test]
async fn a_private_picture_comes_along_and_fetch_media_is_for_groups() {
    let picture = png(3, 3);
    let lines = Lines::new([Line::says("嗯。"), Line::says("在。")]);
    let (home, napcat) = up(&lines).await;
    napcat.stock_file("pv.jpg", Ok(json!({"base64": encoded(&picture)})));
    napcat.send(private_frame(ADMIN, 701, json!([image("pv.jpg", 77)])));
    until_private_turns(&home, 1).await;
    let request = &lines.requests()[0];
    let names: Vec<String> = pictures(request)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["msg-701-1"]);
    assert!(
        request.tools.iter().all(|tool| tool.name != "fetch_media"),
        "私聊的工具面里没有 fetch_media"
    );
    // 取不到图、又没有字的不送；后面那一句照送。
    napcat.send(private_frame(ADMIN, 702, json!([image("gone.jpg", 77)])));
    napcat.send(private_frame(ADMIN, 703, json!([plain("在吗")])));
    until_private_turns(&home, 2).await;
    let said: Vec<String> = home
        .said()
        .iter()
        .filter(|event| event["by"]["kind"] == "person")
        .map(|event| event["cause"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.iter().all(|cause| !cause.contains(":702:")),
        "{said:?}"
    );
    stopped(home).await;
}
