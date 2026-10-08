//! `/apply` 和令牌当场生效（施工 O-16 补二，`onebot.md` 第一条「怎么走」第 2 条，第二条「对外的样子」「施工时定的」第 18、19
//! 条）：要登录令牌；桥重读配置，令牌照新的，两个端口里变了的开新的、关旧的，回实际听的两个端口；新端口被占回 409 和是哪个，
//! 两个都不换、旧的照旧；换了令牌，旧的不收、新的收，已经连着的那一条还在。NapCat 拿错的令牌一直连，配置隔一阵才重读一次。
//! 换了 NapCat 的端口，状态文件跟着换（施工 O-18）。核心是替身，重读的配置由测试给。

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;

use miyu_onebot::serve::Serve;
use miyu_onebot::settings::Settings;
use miyu_onebot::status_file;
use miyu_onebot::tuning::Tuning;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{FakeCore, LOGIN, fake_core};
use crate::support::http::*;
use crate::support::*;

/// 换上的新令牌。
const NEW: &str = "napcat-new-token";

/// 跑着的桥，重读配置读到的是 `now` 里放着的（测试改它），数得出读了几次。
struct Changing {
    bridge: Bridge,
    now: Arc<Mutex<Settings>>,
    reads: Arc<AtomicUsize>,
    dir: PathBuf,
    root: DataRoot,
    _core: FakeCore,
}

impl Changing {
    /// 照 `tuning` 起一个桥：起来时是 [`settings`]，重读的起先也是它。
    async fn start(tuning: Tuning) -> Changing {
        let (dir, root) = temp_root();
        let core = fake_core(&root);
        let now = Arc::new(Mutex::new(settings()));
        let reads = Arc::new(AtomicUsize::new(0));
        let (reading, counting) = (Arc::clone(&now), Arc::clone(&reads));
        let bridge = start(Serve {
            tuning,
            reload: Arc::new(move || {
                counting.fetch_add(1, Ordering::SeqCst);
                Ok(reading.lock().expect("没 panic").clone())
            }),
            ..serve(root.clone(), settings())
        })
        .await;
        Changing {
            bridge,
            now,
            reads,
            dir,
            root,
            _core: core,
        }
    }

    /// 改重读读到的配置。
    fn set(&self, change: impl FnOnce(&mut Settings)) {
        change(&mut self.now.lock().expect("没 panic"));
    }

    /// 叫桥停下，删掉临时目录。
    async fn stop(self) {
        self.bridge.stop().await.expect("停得下");
        if std::fs::remove_dir_all(&self.dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
}

/// 一个空着的端口：系统挑一个，马上放掉。
fn free_port() -> u16 {
    let free = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    free.local_addr().expect("有地址").port()
}

/// 带着登录令牌 `POST /apply`。
async fn apply(web: u16) -> Answer {
    let bearer = format!("Bearer {LOGIN}");
    let host = format!("127.0.0.1:{web}");
    request(web, "POST", "/apply", &host, &[("Authorization", &bearer)]).await
}

/// 带着登录令牌取一次 `/status`。
async fn status(web: u16) -> serde_json::Value {
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

#[tokio::test]
async fn applying_needs_the_login_token() {
    let changing = Changing::start(tuning()).await;
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
async fn a_new_napcat_port_takes_over() {
    let changing = Changing::start(tuning()).await;
    let (old, web) = (changing.bridge.port, changing.bridge.web);
    let new = free_port();
    changing.set(|settings| settings.port = new);
    let applied = apply(web).await;
    assert_eq!(applied.status, 200);
    assert_eq!(applied.json(), json!({"listen": new, "web": web}));
    // NapCat 连进来以前：只有换端口这一件叫状态文件再写。
    within("状态文件跟着换", async {
        while status_file::read(&changing.root).is_none_or(|file| file["listen"] != new) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    owner_napcat(new).await.close().await;
    closed(old).await;
    assert_eq!(status(web).await["listen"], new);
    let again = apply(web).await;
    assert_eq!(
        (again.status, again.json()),
        (200, json!({"listen": new, "web": web})),
        "配置没再变：再照一次什么都不换"
    );
    owner_napcat(new).await.close().await;
    changing.stop().await;
}

#[tokio::test]
async fn a_new_web_port_takes_over() {
    let changing = Changing::start(tuning()).await;
    let old = changing.bridge.web;
    let new = free_port();
    changing.set(|settings| settings.web = new);
    let applied = apply(old).await;
    assert_eq!(applied.status, 200);
    assert_eq!(
        applied.json(),
        json!({"listen": changing.bridge.port, "web": new})
    );
    assert_eq!(get(new, "/", &[]).await.status, 200, "新地址上有页面");
    assert_eq!(status(new).await["web"], new);
    closed(old).await;
    changing.stop().await;
}

#[tokio::test]
async fn a_taken_port_is_refused_and_the_old_ones_stay() {
    let changing = Changing::start(tuning()).await;
    let (listen, web) = (changing.bridge.port, changing.bridge.web);
    let held = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    let taken = held.local_addr().expect("有地址").port();
    changing.set(|settings| settings.port = taken);
    let refused = apply(web).await;
    assert_eq!(refused.status, 409);
    assert_eq!(refused.json(), json!({"in_use": taken}));
    owner_napcat(listen).await.close().await;
    changing.set(|settings| {
        settings.port = 0;
        settings.web = taken;
    });
    let refused = apply(web).await;
    assert_eq!(refused.status, 409);
    assert_eq!(refused.json(), json!({"in_use": taken}));
    assert_eq!(get(web, "/", &[]).await.status, 200, "WebUI 照旧");
    // 两个都变、WebUI 的被占：NapCat 的也不换，开好的新端口放掉。
    let new = free_port();
    changing.set(|settings| {
        settings.port = new;
        settings.web = taken;
    });
    assert_eq!(apply(web).await.status, 409);
    assert_eq!(status(web).await["listen"], listen, "NapCat 的端口也没换");
    closed(new).await;
    owner_napcat(listen).await.close().await;
    changing.stop().await;
}

#[tokio::test]
async fn a_new_token_takes_over_once_applied() {
    let changing = Changing::start(tuning()).await;
    let (port, web) = (changing.bridge.port, changing.bridge.web);
    let _open = owner_napcat(port).await;
    changing.set(|settings| *settings = with_token(Some(NEW)));
    assert_eq!(apply(web).await.status, 200);
    let old = napcat(port, "/ws", Auth::Bearer(TOKEN), None).await;
    assert_eq!(old.err(), Some(401), "旧的不收");
    let fresh = napcat(port, "/ws", Auth::Bearer(NEW), None)
        .await
        .expect("新的进得来");
    let napcat_status = status(web).await["napcat"].clone();
    assert_eq!(
        (&napcat_status["connected"], &napcat_status["self_id"]),
        (&json!(true), &json!(BOT.to_string())),
        "已经连着的那一条还在"
    );
    fresh.close().await;
    changing.stop().await;
}

#[tokio::test]
async fn a_wrong_token_reloads_the_config_at_most_once_a_while() {
    let mut slow = tuning();
    reload_every(&mut slow, 3600);
    let changing = Changing::start(slow).await;
    let port = changing.bridge.port;
    for _ in 0..5 {
        let refused = napcat(port, "/ws", Auth::Bearer("wrong"), None).await;
        assert_eq!(refused.err(), Some(401));
    }
    assert_eq!(changing.reads.load(Ordering::SeqCst), 1, "对不上的只读一次");
    owner_napcat(port).await.close().await;
    assert_eq!(changing.reads.load(Ordering::SeqCst), 1, "对得上的不读");
    assert_eq!(apply(changing.bridge.web).await.status, 200);
    assert_eq!(
        changing.reads.load(Ordering::SeqCst),
        2,
        "/apply 照读，不管隔了多久"
    );
    changing.stop().await;
}
