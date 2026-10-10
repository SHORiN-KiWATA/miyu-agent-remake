//! 推来的配置当场生效（施工 O-16 补二、O-20，O-28 下改；`onebot.md` 第一条「怎么走」第 1、2 条、「施工时定的」第 45、167 条）：
//! 核心推来 `extension.config`，桥不重启就照新的：令牌换了，旧的不收、新的收，已经连着的那一条还在；令牌没了一律 401；NapCat
//! 的端口变了先开新的再换上，旧的关了，状态文件、后台页的 `status` 跟着换，连着的那一条不断。新端口被占的不换、旧的照旧，
//! 空出来了也不另试（原来靠 `/apply` 再试一次，随桥自己的网页去掉）。推来的 `onebot.web` 不认，不开第二个端口。核心是替身，
//! 推送由测试照核心的样子写；状态照状态文件和后台页的 `status`（测试当核心发 `method.call`）看。

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;

use miyu_onebot::status_file;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{FakeCore, fake_core};
use crate::support::ports::TRIES;
use crate::support::spawning::free_port;
use crate::support::*;

/// 换上的新令牌。
const NEW: &str = "napcat-new-token";

/// 跑着的桥（握手交的端口是 0、令牌是 [`TOKEN`]），推配置的那一头。
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

    /// 等状态文件里的 `listen` 合 `wanted`：交回那时的端口。状态文件是换了端口以后另一个任务写的：等「换成别的」以前先等它
    /// 写上现在的，不然会读到更早的那一个。
    async fn written(&self, wanted: impl Fn(u64) -> bool) -> u16 {
        within("状态文件跟着换", async {
            loop {
                let port = status_file::read(&self.root).and_then(|file| file["listen"].as_u64());
                if let Some(port) = port.filter(|port| wanted(*port)) {
                    return u16::try_from(port).expect("是端口");
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
    }

    /// 推一个挑来的空端口当 `onebot.listen`，等状态文件换成它：交回它。挑来放掉以后被别人先拿走了的（桥换不成，旧的照旧）
    /// 换一个再来，最多 [`TRIES`] 次（`support/ports.rs`）。不推 0：配置里写 0 的每次都是 0，桥不当成变了（`onebot.md`
    /// 第一条「怎么走」第 1 条：照配置里写的比）。
    async fn moved(&self) -> u16 {
        for _ in 0..TRIES {
            let new = free_port();
            self.push.config(json!({"onebot.listen": new}));
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
            while tokio::time::Instant::now() < deadline {
                if status_file::read(&self.root).is_some_and(|file| file["listen"] == new) {
                    return new;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        panic!("试了 {TRIES} 个端口，桥都没换上");
    }

    /// 叫桥停下，删掉临时目录。
    async fn stop(self) {
        self.bridge.stop().await.expect("停得下");
        if std::fs::remove_dir_all(&self.dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
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

#[tokio::test]
async fn a_pushed_napcat_port_takes_over_and_the_open_connection_stays() {
    let changing = Changing::start().await;
    let first = changing.bridge.port;
    let _open = admin_napcat(first).await;
    changing.written(|now| now == u64::from(first)).await;
    // 推来的 `onebot.web`（原来桥自己的网页的端口）不认：不开第二个端口。
    let web = free_port();
    changing.push.config(json!({"onebot.web": web}));
    // 再推一次 NapCat 的：当场换。
    let moved = changing.moved().await;
    closed(first).await;
    napcat(moved, "/ws", Auth::Bearer(TOKEN), None)
        .await
        .expect("新的端口收")
        .close()
        .await;
    let status = page_status(&changing.push, "core-1").await;
    assert_eq!(status["listen"], moved, "{status}");
    assert_eq!(
        (&status["napcat"]["connected"], &status["napcat"]["self_id"]),
        (&json!(true), &json!(BOT.to_string())),
        "换端口不断连着的那一条：{status}"
    );
    assert!(
        TcpStream::connect(("127.0.0.1", web)).await.is_err(),
        "推来的 onebot.web 不开"
    );
    changing.stop().await;
}

#[tokio::test]
async fn a_pushed_port_that_is_taken_changes_nothing_and_is_not_tried_again() {
    let changing = Changing::start().await;
    let first = changing.bridge.port;
    changing.written(|now| now == u64::from(first)).await;
    let (taken, port) = held();
    changing.push.config(json!({"onebot.listen": port}));
    // 换不成：旧的照旧。推送是异步的，等一阵再看（照对的样子，这一阵里一直是旧的）。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        page_status(&changing.push, "core-1").await["listen"],
        first,
        "旧的照旧"
    );
    admin_napcat(first).await.close().await;
    // 空出来了也不另试：人换一个再存，配置变了核心再推（「施工时定的」第 167 条）。
    drop(taken);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        page_status(&changing.push, "core-2").await["listen"],
        first,
        "不再另试"
    );
    assert_eq!(
        status_file::read(&changing.root).and_then(|file| file["listen"].as_u64()),
        Some(u64::from(first))
    );
    // 换一个再推：换上。
    let moved = changing.moved().await;
    assert_ne!(moved, port);
    closed(first).await;
    changing.stop().await;
}

#[tokio::test]
async fn a_pushed_token_takes_over_and_the_open_connection_stays() {
    let changing = Changing::start().await;
    let port = changing.bridge.port;
    let _open = admin_napcat(port).await;
    changing.push.config(json!({"onebot.token": NEW}));
    // 不报号：不顶掉连着的那一条。
    admitted(port, "/ws", Auth::Bearer(NEW), None)
        .await
        .close()
        .await;
    let old = napcat(port, "/ws", Auth::Bearer(TOKEN), None).await;
    assert_eq!(old.err(), Some(401), "旧的不收");
    let status = page_status(&changing.push, "core-1").await;
    assert_eq!(
        (&status["napcat"]["connected"], &status["napcat"]["self_id"]),
        (&json!(true), &json!(BOT.to_string())),
        "已经连着的那一条还在：{status}"
    );
    assert_eq!(status["token"], "set");
    // 令牌没了：一律 401，`status` 说没设。
    changing.push.config(json!({"onebot.token": null}));
    refused(port, NEW).await;
    assert_eq!(page_status(&changing.push, "core-2").await["token"], "none");
    let nothing = napcat(port, "/ws", Auth::Nothing, None).await;
    assert_eq!(nothing.err(), Some(401));
    changing.stop().await;
}
