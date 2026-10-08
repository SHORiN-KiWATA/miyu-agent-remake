//! `/media`（施工 W-10，`web-ui.md`「怎么走」第三条）：带登录令牌换票据，照票据一块块给，能分段；类型照表、关在空的来源里；
//! 下载的名字照 RFC 5987 转义；令牌作废了票据一起作废；有媒体在给不算空闲；票据不用了会过期，有上限。

#[path = "media/fake.rs"]
mod fake;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::*;
use fake::{CHUNK, Core, LOGIN};
use miyu_web::serve::address;

const SMALL: &[u8] = b"hello, media";
const SANDBOX: &str =
    "sandbox; default-src 'none'; img-src data:; media-src data:; style-src 'unsafe-inline'";

/// 大的：1.3 MiB，每个字节不一样，拼错了看得出来。
fn big() -> Vec<u8> {
    (0..(CHUNK * 2 + CHUNK / 2))
        .map(|at| (at % 251) as u8)
        .collect()
}

/// 起网页软件和核心的替身。
async fn site(core: Core, settings: miyu_web::settings::Settings) -> (Home, Arc<Core>, u16) {
    let home = Home::new();
    let core = Arc::new(core);
    fake::serve(fake_core(&home), Arc::clone(&core));
    let (_, port, _serving) = start_with(&home, 0, settings).await;
    (home, core, port)
}

fn host(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// 带登录令牌 `login` 换一张票据。
async fn post(port: u16, login: Option<&str>, body: &Value) -> Answer {
    let bearer = login.map(|login| format!("Bearer {login}"));
    let mut extra = vec![("Content-Type", "application/json")];
    if let Some(bearer) = &bearer {
        extra.push(("Authorization", bearer));
    }
    send(
        port,
        "POST",
        "/media",
        &host(port),
        &extra,
        body.to_string().as_bytes(),
    )
    .await
}

/// 换到的地址。
fn url(answer: &Answer) -> String {
    assert_eq!(
        answer.status,
        200,
        "{}",
        String::from_utf8_lossy(&answer.body)
    );
    let body: Value = serde_json::from_slice(&answer.body).expect("是 JSON");
    let url = body["url"].as_str().expect("有 url").to_string();
    assert!(url.starts_with("/media/"), "{url}");
    let ticket = &url["/media/".len()..];
    assert!(
        ticket.len() == 64
            && ticket
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{ticket}"
    );
    url
}

async fn get(port: u16, url: &str, range: Option<&str>) -> Answer {
    let mut extra = Vec::new();
    if let Some(range) = range {
        extra.push(("Range", range));
    }
    request(port, "GET", url, &host(port), &extra).await
}

#[tokio::test]
async fn a_ticket_needs_a_login_token() {
    let (_home, _core, port) = site(
        Core::default().blob("sha256:small", SMALL.to_vec()),
        settings(600),
    )
    .await;
    let body = json!({"blob": "sha256:small"});
    assert_eq!(post(port, None, &body).await.status, 401, "没带");
    assert_eq!(post(port, Some("bad"), &body).await.status, 401, "带错了");
    let basic = send(
        port,
        "POST",
        "/media",
        &host(port),
        &[("Authorization", "Basic c0ffee")],
        body.to_string().as_bytes(),
    )
    .await;
    assert_eq!(basic.status, 401, "只认 Bearer");
    let answer = post(port, Some(LOGIN), &body).await;
    let url = url(&answer);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    assert_eq!(answer.header("set-cookie"), None);
    assert_eq!(get(port, &url, None).await.body, SMALL);
}

#[tokio::test]
async fn the_body_names_exactly_one_thing() {
    let (_home, _core, port) = site(Core::default(), settings(600)).await;
    for bad in [
        json!({}),
        json!({"blob": "sha256:a", "path": "/x"}),
        json!({"blob": 3}),
        json!({"path": "/x", "download": "yes"}),
        json!({"path": "/x", "name": 1}),
        json!(["/x"]),
    ] {
        assert_eq!(post(port, Some(LOGIN), &bad).await.status, 400, "{bad}");
    }
    let not_json = send(
        port,
        "POST",
        "/media",
        &host(port),
        &[("Authorization", "Bearer c0ffee")],
        b"{oops",
    )
    .await;
    assert_eq!(not_json.status, 400);
    assert_eq!(
        request(port, "GET", "/media", &host(port), &[])
            .await
            .status,
        405
    );
    assert_eq!(
        request(port, "POST", "/media/abc", &host(port), &[])
            .await
            .status,
        405
    );
    assert_eq!(
        request(port, "GET", "/media/abc", &host(port), &[])
            .await
            .status,
        404,
        "不认识的票据"
    );
}

#[tokio::test]
async fn what_the_core_says_becomes_the_status() {
    let core = Core::default().file("/files/a.png", SMALL.to_vec());
    let (_home, _core, port) = site(core, settings(600)).await;
    assert_eq!(
        post(port, Some(LOGIN), &json!({"blob": "sha256:none"}))
            .await
            .status,
        404
    );
    assert_eq!(
        post(port, Some(LOGIN), &json!({"path": "/files/none"}))
            .await
            .status,
        404
    );
    assert_eq!(
        post(
            port,
            Some(LOGIN),
            &json!({"path": "/data-root/system/accounts.json"})
        )
        .await
        .status,
        403
    );
    let url = url(&post(port, Some(LOGIN), &json!({"path": "/files/a.png"})).await);
    let answer = get(port, &url, None).await;
    assert_eq!(answer.body, SMALL);
    assert_eq!(
        answer.header("content-type"),
        Some("image/png"),
        "照扩展名查表"
    );
}

#[tokio::test]
async fn no_core_is_a_bad_gateway() {
    let home = Home::new();
    let (_, port, _serving) = start(&home, 0, 600).await;
    assert_eq!(
        post(port, Some(LOGIN), &json!({"blob": "sha256:small"}))
            .await
            .status,
        502
    );
}

#[tokio::test]
async fn the_same_thing_gets_the_same_ticket() {
    let (_home, core, port) = site(
        Core::default().blob("sha256:small", SMALL.to_vec()),
        settings(600),
    )
    .await;
    let one = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "type": "image/png"}),
    )
    .await);
    let again = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "type": "image/png"}),
    )
    .await);
    assert_eq!(one, again);
    let other = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "type": "image/png", "download": true}),
    )
    .await);
    assert_ne!(one, other, "三格不一样是另一张");
    assert_eq!(
        core.hellos.load(Ordering::SeqCst),
        1,
        "同一个令牌的连接留着复用"
    );
}

#[tokio::test]
async fn whole_or_one_range() {
    let bytes = big();
    let size = bytes.len();
    let (_home, core, port) = site(
        Core::default().blob("sha256:big", bytes.clone()),
        settings(600),
    )
    .await;
    let url = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:big", "type": "video/mp4"}),
    )
    .await);
    let whole = get(port, &url, None).await;
    assert_eq!(whole.status, 200);
    assert_eq!(whole.body, bytes, "一块块拼起来一个字节不差");
    assert_eq!(
        whole.header("content-length"),
        Some(size.to_string().as_str())
    );
    assert_eq!(whole.header("accept-ranges"), Some("bytes"));
    assert_eq!(whole.header("content-type"), Some("video/mp4"));
    let reads = core.reads.lock().expect("没崩").clone();
    assert!(
        reads.iter().all(|(_, length)| *length <= CHUNK as u64),
        "{reads:?}"
    );
    assert!(
        reads.iter().filter(|(_, length)| *length > 0).count() >= 3,
        "分几次问：{reads:?}"
    );

    let part = get(port, &url, Some("bytes=600000-600009")).await;
    assert_eq!(part.status, 206);
    assert_eq!(part.body, &bytes[600_000..600_010]);
    assert_eq!(
        part.header("content-range"),
        Some(format!("bytes 600000-600009/{size}").as_str())
    );
    assert_eq!(part.header("content-length"), Some("10"));
    core.overfill.store(true, Ordering::SeqCst);
    let exact = get(port, &url, Some("bytes=10-19")).await;
    assert_eq!(
        (exact.status, exact.body.as_slice()),
        (206, &bytes[10..20]),
        "核心多给的不写"
    );
    core.overfill.store(false, Ordering::SeqCst);
    let tail = get(port, &url, Some("bytes=-5")).await;
    assert_eq!(
        (tail.status, tail.body.as_slice()),
        (206, &bytes[size - 5..])
    );
    let rest = get(port, &url, Some(&format!("bytes={}-", size - 3))).await;
    assert_eq!(
        (rest.status, rest.body.as_slice()),
        (206, &bytes[size - 3..])
    );
    let clipped = get(
        port,
        &url,
        Some(&format!("bytes={}-{}", size - 2, size + 100)),
    )
    .await;
    assert_eq!(
        (clipped.status, clipped.body.as_slice()),
        (206, &bytes[size - 2..])
    );
    let beyond = get(port, &url, Some(&format!("bytes={size}-"))).await;
    assert_eq!(beyond.status, 416);
    assert_eq!(
        beyond.header("content-range"),
        Some(format!("bytes */{size}").as_str())
    );
    for ignored in ["bytes=0-1,5-6", "items=0-1", "bytes=9-1", "bytes=x-"] {
        let answer = get(port, &url, Some(ignored)).await;
        assert_eq!(
            (answer.status, answer.body.len()),
            (200, size),
            "{ignored}：照没写"
        );
    }
}

#[tokio::test]
async fn requests_on_one_connection_get_their_own_answers() {
    let core = Core::default()
        .blob("sha256:slow", b"slow one".to_vec())
        .blob("sha256:small", SMALL.to_vec());
    let (_home, core, port) = site(core, settings(600)).await;
    let slow = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:slow"})).await);
    let small = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:small"})).await);
    let (a, b, c) = tokio::join!(
        get(port, &slow, None),
        get(port, &small, None),
        get(port, &slow, Some("bytes=0-3"))
    );
    assert_eq!(a.body, b"slow one");
    assert_eq!(b.body, SMALL);
    assert_eq!(c.body, b"slow");
    assert_eq!(core.hellos.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn it_is_shut_in_an_empty_origin() {
    let core = Core::default()
        .blob("sha256:small", SMALL.to_vec())
        .file("/files/notes.weird", SMALL.to_vec());
    let (_home, _core, port) = site(core, settings(600)).await;
    let page = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "type": "text/html; charset=utf-8"}),
    )
    .await);
    let answer = get(port, &page, None).await;
    assert_eq!(
        answer.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(answer.header("content-security-policy"), Some(SANDBOX));
    assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(answer.header("cache-control"), Some("private, no-cache"));
    assert_eq!(answer.header("content-disposition"), None);
    let made_up = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "type": "application/x-made-up"}),
    )
    .await);
    assert_eq!(
        get(port, &made_up, None).await.header("content-type"),
        Some("application/octet-stream"),
        "表里没有的类型不认"
    );
    let unknown = url(&post(port, Some(LOGIN), &json!({"path": "/files/notes.weird"})).await);
    assert_eq!(
        get(port, &unknown, None).await.header("content-type"),
        Some("application/octet-stream")
    );
}

#[tokio::test]
async fn a_download_carries_its_name() {
    let core = Core::default()
        .blob("sha256:small", SMALL.to_vec())
        .file("/files/plain.txt", SMALL.to_vec());
    let (_home, _core, port) = site(core, settings(600)).await;
    let named = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "name": "../a\\b/报告 1;\"x\".pdf", "download": true}),
    )
    .await);
    assert_eq!(
        get(port, &named, None).await.header("content-disposition"),
        Some("attachment; filename*=UTF-8''%E6%8A%A5%E5%91%8A%201%3B%22x%22.pdf"),
        "只留最后一段，照 RFC 5987 转义"
    );
    let from_path = url(&post(
        port,
        Some(LOGIN),
        &json!({"path": "/files/plain.txt", "download": true}),
    )
    .await);
    assert_eq!(
        get(port, &from_path, None)
            .await
            .header("content-disposition"),
        Some("attachment; filename*=UTF-8''plain.txt"),
        "没写名字的照文件名"
    );
    let blob = url(&post(
        port,
        Some(LOGIN),
        &json!({"blob": "sha256:small", "download": true}),
    )
    .await);
    assert_eq!(
        get(port, &blob, None).await.header("content-disposition"),
        Some("attachment")
    );
}

#[tokio::test]
async fn a_revoked_login_voids_its_tickets() {
    let (_home, core, port) = site(
        Core::default().blob("sha256:small", SMALL.to_vec()),
        settings(600),
    )
    .await;
    let url = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:small"})).await);
    assert_eq!(get(port, &url, None).await.status, 200);
    core.revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        get(port, &url, None).await.status,
        401,
        "核心断开了，重连握手被拒"
    );
    core.revoked.store(false, Ordering::SeqCst);
    assert_eq!(get(port, &url, None).await.status, 404, "票据一起作废了");
}

#[tokio::test]
async fn serving_media_is_not_idle() {
    let core = Core::default().blob("sha256:big", big());
    core.delay_ms.store(700, Ordering::SeqCst);
    let home = Home::new();
    let core = Arc::new(core);
    fake::serve(fake_core(&home), Arc::clone(&core));
    let (_, port, serving) = start_with(&home, 0, settings(1)).await;
    let url = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:big"})).await);
    // 三块，每块晚 0.7 秒：给完要 2 秒多，空闲 1 秒就退的到这时还在。
    let fetching = tokio::spawn(async move { get(port, &url, None).await });
    tokio::time::sleep(Duration::from_millis(1600)).await;
    assert!(!serving.is_finished(), "有媒体在给不算空闲");
    let answer = within("给完", fetching).await.expect("没崩");
    assert_eq!(answer.body.len(), big().len());
    let ran = within("空闲退出", serving).await.expect("没崩");
    assert_eq!(ran, Ok(()));
    assert_eq!(address(&home.root), None);
}

#[tokio::test]
async fn tickets_expire_and_are_capped() {
    let core = Core::default()
        .blob("sha256:a", b"a".to_vec())
        .blob("sha256:b", b"b".to_vec())
        .blob("sha256:c", b"c".to_vec());
    let mut short = settings(600);
    short.ticket_idle_seconds = 1;
    short.most_tickets = 2;
    let (_home, _core, port) = site(core, short).await;
    let a = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:a"})).await);
    let b = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:b"})).await);
    assert_eq!(
        get(port, &a, None).await.status,
        200,
        "用过一次，b 成了最久没用的"
    );
    let c = url(&post(port, Some(LOGIN), &json!({"blob": "sha256:c"})).await);
    assert_eq!(get(port, &b, None).await.status, 404, "满了丢最久没用的");
    assert_eq!(get(port, &a, None).await.body, b"a");
    assert_eq!(get(port, &c, None).await.body, b"c");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(get(port, &c, None).await.status, 404, "不用了的作废");
}
