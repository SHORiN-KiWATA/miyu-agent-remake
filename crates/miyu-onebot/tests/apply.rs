//! 推来的配置当场生效和 `/apply`（施工 O-16 补二、O-20，`onebot.md` 第一条「怎么走」第 1、2 条、「施工时定的」第 39、45 条，
//! 第二条「对外的样子」「施工时定的」第 19 条）：核心推来 `extension.config`，桥不重启就照新的：令牌换了，旧的不收、新的收，
//! 已经连着的那一条还在；令牌没了一律 401；端口变了照 `/apply` 的办法换，旧的关了，状态文件跟着换。新端口被占的两个都不换、
//! 旧的照旧，`/apply`（要登录令牌）再试一次，还被占回 409 和是哪个，空出来了就换上。核心是替身，推送由测试照核心的样子写。

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::net::TcpStream;

use miyu_onebot::status_file;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{FakeCore, LOGIN, fake_core};
use crate::support::http::*;
use crate::support::ports::TRIES;
use crate::support::spawning::free_port;
use crate::support::*;

/// 换上的新令牌。
const NEW: &str = "napcat-new-token";

/// 跑着的桥（握手交的两个端口是 0、令牌是 [`TOKEN`]），推配置的那一头。
struct Changing {
    bridge: Bridge,
    push: Relay,
    dir: PathBuf,
    root: DataRoot,
    _core: FakeCore,
}

impl Changing {
    /// 起一个桥：握手交的是 [`settings`]。
    async fn start() -> Changing {
        let (dir, root) = temp_root();
        let core = fake_core(&root);
        let (serve, push) = serve_pushing(root.clone(), settings());
        let bridge = start(serve).await;
        Changing {
            bridge,
            push,
            dir,
            root,
            _core: core,
        }
    }

    /// 等状态文件里的 `field`（`listen`、`web`）合 `wanted`：交回那时的端口。状态文件是换了端口以后另一个任务写的，比
    /// `/apply` 的回应、`/status` 晚（`status_file.rs`）：等「换成别的」以前先等它写上现在的，不然会读到更早的那一个。
    async fn written(&self, field: &str, wanted: impl Fn(u64) -> bool) -> u16 {
        within("状态文件跟着换", async {
            loop {
                let port = status_file::read(&self.root).and_then(|file| file[field].as_u64());
                if let Some(port) = port.filter(|port| wanted(*port)) {
                    return u16::try_from(port).expect("是端口");
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
    }

    /// 叫桥停下，删掉临时目录。
    async fn stop(self) {
        self.bridge.stop().await.expect("停得下");
        if std::fs::remove_dir_all(&self.dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
}

/// 带着登录令牌 `POST /apply`。
async fn apply(web: u16) -> Answer {
    let bearer = format!("Bearer {LOGIN}");
    let host = format!("127.0.0.1:{web}");
    request(web, "POST", "/apply", &host, &[("Authorization", &bearer)]).await
}

/// 一直 `POST /apply`，直到回应合 `wanted`（推来的还没到桥的，`/apply` 照手里旧的答）：交回那一次的状态码和正文。
async fn until_applied(web: u16, wanted: impl Fn(u16, &Value) -> bool) -> (u16, Value) {
    within("/apply 照推来的", async {
        loop {
            let answer = apply(web).await;
            let body = answer.json();
            if wanted(answer.status, &body) {
                return (answer.status, body);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
}

/// 带着登录令牌取一次 `/status`。
async fn status(web: u16) -> Value {
    let bearer = format!("Bearer {LOGIN}");
    let answer = get(web, "/status", &[("Authorization", &bearer)]).await;
    assert_eq!(answer.status, 200);
    answer.json()
}

/// 等到 `port` 上连不上了（换下来的监听关掉）。
async fn closed(port: u16) {
    within("旧的端口关掉", async {
        while TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
}

/// 拿 `token` 连 `port`（不报号：不顶掉连着的那一条），直到被拒（401）。
async fn refused(port: u16, token: &str) {
    within("NapCat 被拒", async {
        loop {
            match napcat(port, "/ws", Auth::Bearer(token), None).await {
                Err(401) => return,
                Ok(napcat) => napcat.close().await,
                Err(status) => panic!("回的不是 401：{status}"),
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}

/// 系统挑一个端口、一直拿着：桥开不了它。
fn held() -> (std::net::TcpListener, u16) {
    let held = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    let port = held.local_addr().expect("有地址").port();
    (held, port)
}

/// 推来一个被占着的端口当 `key`（`onebot.listen`、`onebot.web`）：两个都不换、旧的照旧（`/status` 照旧说 `before` 这两个，
/// NapCat 照旧连得进旧的），`/apply` 再试一次也是 409、说的是它。放开以后再 `/apply`：交回放开的端口和那一次的回应。放开以后
/// 被别人先拿走了的（409 说的还是它：负载高时这个号又被别的测试、别的连接拿去，`support/ports.rs`）换一个再来，最多 [`TRIES`]
/// 次。`web` 是问 `/apply` 的那个 WebUI 端口。
async fn taken_then_freed(
    changing: &Changing,
    web: u16,
    key: &str,
    before: (u16, u16),
) -> (u16, Answer) {
    for _ in 0..TRIES {
        let (taken, port) = held();
        changing.push.config(json!({key: port}));
        until_applied(web, |status, body| {
            status == 409 && body["in_use"] == json!(port)
        })
        .await;
        let now = status(web).await;
        assert_eq!(
            (&now["listen"], &now["web"]),
            (&json!(before.0), &json!(before.1)),
            "旧的照旧"
        );
        napcat(before.0, "/ws", Auth::Bearer(TOKEN), None)
            .await
            .expect("旧的端口照旧收")
            .close()
            .await;
        // 空出来了：`/apply` 再试一次就换上（配置没再变，核心不会再推）。
        drop(taken);
        let applied = apply(web).await;
        if (applied.status, applied.json()) != (409, json!({"in_use": port})) {
            return (port, applied);
        }
    }
    panic!("试了 {TRIES} 个端口，放开以后都被别人先拿走了");
}

#[tokio::test]
async fn applying_needs_the_login_token() {
    let changing = Changing::start().await;
    let web = changing.bridge.web;
    let host = format!("127.0.0.1:{web}");
    assert_eq!(
        request(web, "POST", "/apply", &host, &[]).await.status,
        401,
        "没带"
    );
    for header in [format!("Bearer {LOGIN}x"), format!("Token {LOGIN}")] {
        let refused = request(web, "POST", "/apply", &host, &[("Authorization", &header)]).await;
        assert_eq!(refused.status, 401, "{header}");
    }
    let bearer = format!("Bearer {LOGIN}");
    let got = request(web, "GET", "/apply", &host, &[("Authorization", &bearer)]).await;
    assert_eq!(got.status, 405);
    let applied = apply(web).await;
    assert_eq!(applied.status, 200);
    assert_eq!(applied.header("cache-control"), Some("no-store"));
    assert_eq!(
        applied.json(),
        json!({"listen": changing.bridge.port, "web": web}),
        "什么都没变：两个端口照旧"
    );
    changing.stop().await;
}

#[tokio::test]
async fn a_pushed_napcat_port_takes_over_once_it_can() {
    let changing = Changing::start().await;
    let (first, web) = (changing.bridge.port, changing.bridge.web);
    let _open = owner_napcat(first).await;
    let (port, applied) = taken_then_freed(&changing, web, "onebot.listen", (first, web)).await;
    assert_eq!(
        (applied.status, applied.json()),
        (200, json!({"listen": port, "web": web}))
    );
    closed(first).await;
    changing
        .written("listen", |now| now == u64::from(port))
        .await;
    // 再推一次（0：让系统挑）：不用 `/apply` 就换。
    changing.push.config(json!({"onebot.listen": 0}));
    let moved = changing
        .written("listen", |now| now != u64::from(port))
        .await;
    closed(port).await;
    napcat(moved, "/ws", Auth::Bearer(TOKEN), None)
        .await
        .expect("新的端口收")
        .close()
        .await;
    assert_eq!(status(web).await["listen"], moved);
    let napcat_status = status(web).await["napcat"].clone();
    assert_eq!(
        (&napcat_status["connected"], &napcat_status["self_id"]),
        (&json!(true), &json!(BOT.to_string())),
        "换端口不断连着的那一条"
    );
    let again = apply(web).await;
    assert_eq!(
        (again.status, again.json()),
        (200, json!({"listen": moved, "web": web})),
        "推来的已经换好：再照一次什么都不换"
    );
    changing.stop().await;
}

#[tokio::test]
async fn a_pushed_web_port_takes_over_once_it_can() {
    let changing = Changing::start().await;
    let (listen, first) = (changing.bridge.port, changing.bridge.web);
    let (port, applied) = taken_then_freed(&changing, first, "onebot.web", (listen, first)).await;
    assert_eq!(
        (applied.status, applied.json()),
        (200, json!({"listen": listen, "web": port})),
        "回应走的是旧端口上接进来的这一条"
    );
    closed(first).await;
    assert_eq!(get(port, "/", &[]).await.status, 200, "新地址上有页面");
    assert_eq!(status(port).await["web"], port);
    // 状态文件是换完以后另一个任务照这一刻写的，比 `/apply` 的回应、`/status` 晚：先等它写上新端口，下面等「不是它」的才
    // 等的是推来的那一次（不然读到的还是最早的那个端口，它早就关了）。
    changing.written("web", |now| now == u64::from(port)).await;
    changing.push.config(json!({"onebot.web": 0}));
    let moved = changing.written("web", |now| now != u64::from(port)).await;
    closed(port).await;
    assert_eq!(
        get(moved, "/", &[]).await.status,
        200,
        "推来的不用 /apply 就换"
    );
    assert_eq!(status(moved).await["web"], moved);
    changing.stop().await;
}

#[tokio::test]
async fn when_one_pushed_port_is_taken_neither_changes() {
    let changing = Changing::start().await;
    let (listen, web) = (changing.bridge.port, changing.bridge.web);
    let (_taken, port) = held();
    // 两个都变、WebUI 的被占：NapCat 的也不换，开好的新端口放掉。挑的 NapCat 端口被别人先拿走了（409 说的是它）的换一个再来。
    for _ in 0..TRIES {
        let new = free_port();
        changing
            .push
            .config(json!({"onebot.listen": new, "onebot.web": port}));
        // 推来的还没到的，`/apply` 照手里旧的答（200）；上一回挑的那个的 409 也不算。
        let (_, body) = until_applied(web, |status, body| {
            status == 409 && (body["in_use"] == json!(new) || body["in_use"] == json!(port))
        })
        .await;
        if body == json!({"in_use": new}) {
            continue;
        }
        assert_eq!(body, json!({"in_use": port}));
        assert_eq!(status(web).await["listen"], listen, "NapCat 的端口也没换");
        closed(new).await;
        owner_napcat(listen).await.close().await;
        changing.stop().await;
        return;
    }
    panic!("试了 {TRIES} 个端口，桥都说被占了");
}

#[tokio::test]
async fn a_pushed_token_takes_over_and_the_open_connection_stays() {
    let changing = Changing::start().await;
    let (port, web) = (changing.bridge.port, changing.bridge.web);
    let _open = owner_napcat(port).await;
    changing.push.config(json!({"onebot.token": NEW}));
    // 不报号：不顶掉连着的那一条。
    admitted(port, "/ws", Auth::Bearer(NEW), None)
        .await
        .close()
        .await;
    let old = napcat(port, "/ws", Auth::Bearer(TOKEN), None).await;
    assert_eq!(old.err(), Some(401), "旧的不收");
    let napcat_status = status(web).await["napcat"].clone();
    assert_eq!(
        (&napcat_status["connected"], &napcat_status["self_id"]),
        (&json!(true), &json!(BOT.to_string())),
        "已经连着的那一条还在"
    );
    assert_eq!(status(web).await["token"], "set");
    // 令牌没了：一律 401，`/status` 说没设。
    changing.push.config(json!({"onebot.token": null}));
    refused(port, NEW).await;
    assert_eq!(status(web).await["token"], "none");
    let nothing = napcat(port, "/ws", Auth::Nothing, None).await;
    assert_eq!(nothing.err(), Some(401));
    changing.stop().await;
}
