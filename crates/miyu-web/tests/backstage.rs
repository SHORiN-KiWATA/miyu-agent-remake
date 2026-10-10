//! 软件后台页（施工 F-6 下，`web-ui.md`「怎么走」第四条）：带登录令牌换票据（这个包有后台页的才给），照票据一块块给页面
//! 文件；类型照表，响应头带沙箱、不许联网、允许空来源取；票据和包对得上才给；令牌作废了票据一起作废；方法只认 POST、GET。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use crate::fake::{self, CHUNK, Core, LOGIN};
use crate::support::*;

const INDEX: &[u8] = b"<!doctype html><title>demo</title>";
/// 测试的设置里写的后台页内容安全策略（`support::settings`）。
const CSP: &str = "sandbox allow-scripts allow-forms; connect-src 'none'; frame-ancestors 'self'";

/// 一个有后台页的包 `demo`、一个没有的 `plain`。
fn demo() -> Core {
    Core::default()
        .page("demo", "index.html", INDEX.to_vec())
        .page("demo", "app.js", b"export const x = 1;".to_vec())
        .page("demo", "sub/index.html", b"sub".to_vec())
        .plain("plain")
}

/// 起网页软件和核心的替身。
async fn site(core: Core) -> (Home, Arc<Core>, u16) {
    let home = Home::new();
    let core = Arc::new(core);
    fake::serve(fake_core(&home), Arc::clone(&core));
    let (_, port, _serving) = start_with(&home, 0, settings(600)).await;
    (home, core, port)
}

fn host(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// 带登录令牌 `login` 换一张后台页的票据。
async fn post(port: u16, login: Option<&str>, body: &[u8]) -> Answer {
    let bearer = login.map(|login| format!("Bearer {login}"));
    let mut extra = vec![("Content-Type", "application/json")];
    if let Some(bearer) = &bearer {
        extra.push(("Authorization", bearer));
    }
    send(port, "POST", "/page", &host(port), &extra, body).await
}

fn ask(package: &str) -> Vec<u8> {
    json!({ "package": package }).to_string().into_bytes()
}

/// 换到的地址：`/p/<64 位小写十六进制>/<包>/`。
fn url(answer: &Answer, package: &str) -> String {
    assert_eq!(
        answer.status,
        200,
        "{}",
        String::from_utf8_lossy(&answer.body)
    );
    let body: Value = serde_json::from_slice(&answer.body).expect("是 JSON");
    let url = body["url"].as_str().expect("有 url").to_string();
    let ticket = url
        .strip_prefix("/p/")
        .and_then(|rest| rest.strip_suffix(&format!("/{package}/")))
        .unwrap_or_else(|| panic!("{url}"));
    assert!(
        ticket.len() == 64
            && ticket
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{ticket}"
    );
    url
}

async fn get(port: u16, path: &str) -> Answer {
    request(port, "GET", path, &host(port), &[]).await
}

#[tokio::test]
async fn a_page_ticket_needs_a_login_and_a_page() {
    let (_home, _core, port) = site(demo()).await;
    assert_eq!(post(port, None, &ask("demo")).await.status, 401, "没带");
    assert_eq!(
        post(port, Some("bad"), &ask("demo")).await.status,
        401,
        "带错了"
    );
    let basic = send(
        port,
        "POST",
        "/page",
        &host(port),
        &[("Authorization", "Basic c0ffee")],
        &ask("demo"),
    )
    .await;
    assert_eq!(basic.status, 401, "只认 Bearer");
    for bad in [
        &b"{}"[..],
        br#"{"package": 3}"#,
        br#"{"package": "a b"}"#,
        br#"{"package": ""}"#,
        b"{oops",
        br#"["demo"]"#,
    ] {
        assert_eq!(
            post(port, Some(LOGIN), bad).await.status,
            400,
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
    assert_eq!(
        post(port, Some(LOGIN), &ask("plain")).await.status,
        404,
        "没有后台页"
    );
    assert_eq!(
        post(port, Some(LOGIN), &ask("nope")).await.status,
        404,
        "没有这个包"
    );
    let answer = post(port, Some(LOGIN), &ask("demo")).await;
    let first = url(&answer, "demo");
    assert_eq!(answer.header("content-type"), Some("application/json"));
    assert_eq!(answer.header("set-cookie"), None);
    assert_eq!(
        url(&post(port, Some(LOGIN), &ask("demo")).await, "demo"),
        first,
        "同一个令牌、同一个包交回同一张"
    );
    assert_eq!(get(port, "/page").await.status, 405);
}

#[tokio::test]
async fn files_come_with_the_backstage_headers() {
    let (_home, _core, port) = site(demo()).await;
    let base = url(&post(port, Some(LOGIN), &ask("demo")).await, "demo");
    let index = get(port, &base).await;
    assert_eq!(index.status, 200);
    assert_eq!(index.body, INDEX);
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(index.header("content-security-policy"), Some(CSP));
    assert_eq!(index.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(index.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(index.header("cache-control"), Some("no-cache"));
    assert_eq!(
        index.header("access-control-allow-origin"),
        Some("*"),
        "框的来源是空的，模块脚本、字体照跨源取"
    );
    assert_eq!(index.header("set-cookie"), None);
    let script = get(port, &format!("{base}app.js")).await;
    assert_eq!(script.body, b"export const x = 1;");
    assert_eq!(
        script.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        get(port, &format!("{base}sub/")).await.body,
        b"sub",
        "以 / 结尾的补 index.html"
    );
    assert_eq!(get(port, &format!("{base}missing.css")).await.status, 404);
    assert_eq!(
        get(port, &format!("{base}%2e%2e/x")).await.status,
        400,
        "核心拒的照原因码"
    );
    assert_eq!(
        request(port, "POST", &base, &host(port), &[]).await.status,
        405
    );
}

#[tokio::test]
async fn tickets_only_open_their_own_package() {
    let (_home, _core, port) = site(demo().page("other", "index.html", b"other".to_vec())).await;
    let base = url(&post(port, Some(LOGIN), &ask("demo")).await, "demo");
    let ticket = base.trim_start_matches("/p/").trim_end_matches("/demo/");
    assert_eq!(
        get(port, &format!("/p/{ticket}/other/")).await.status,
        404,
        "票据是 demo 的"
    );
    assert_eq!(
        get(port, &format!("/p/{}/demo/", "0".repeat(64)))
            .await
            .status,
        404,
        "不认识的票据"
    );
    assert_eq!(
        get(port, &format!("/p/{ticket}/bad%20name/")).await.status,
        404
    );
    assert_eq!(get(port, &format!("/p/{ticket}")).await.status, 404);
}

#[tokio::test]
async fn big_files_come_in_chunks() {
    let big: Vec<u8> = (0..(CHUNK * 2 + CHUNK / 2))
        .map(|at| (at % 251) as u8)
        .collect();
    let (_home, core, port) = site(demo().page("demo", "big.js", big.clone())).await;
    let base = url(&post(port, Some(LOGIN), &ask("demo")).await, "demo");
    core.reads.lock().expect("没崩").clear();
    let answer = get(port, &format!("{base}big.js")).await;
    assert_eq!(
        answer.header("content-length"),
        Some(big.len().to_string().as_str())
    );
    assert!(answer.body == big, "一个字节不差");
    let offsets: Vec<u64> = core
        .reads
        .lock()
        .expect("没崩")
        .iter()
        .map(|(offset, _)| *offset)
        .collect();
    assert_eq!(offsets, [0, CHUNK as u64, 2 * CHUNK as u64], "一块一块问");
}

#[tokio::test]
async fn a_revoked_login_drops_its_page_tickets() {
    let (_home, core, port) = site(demo()).await;
    let base = url(&post(port, Some(LOGIN), &ask("demo")).await, "demo");
    assert_eq!(get(port, &base).await.status, 200);
    core.revoked.store(true, Ordering::SeqCst);
    assert_eq!(get(port, &base).await.status, 401, "握手被拒");
    core.revoked.store(false, Ordering::SeqCst);
    assert_eq!(
        get(port, &base).await.status,
        404,
        "这个令牌的票据一起作废了"
    );
}
