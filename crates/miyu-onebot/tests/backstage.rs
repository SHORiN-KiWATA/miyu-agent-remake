//! 后台页（施工 O-28 上，`onebot.md` 第一条「后台页」，`package-pages.md`）：真核心照开关拉起真桥，头照网页给后台页转的样子经
//! 核心走一遍：`package.list` 里接入QQ 带图标和后台页；`package.call` 调桥登记的 `status`、`connection.token`，答的是桥手里
//! 最新的（NapCat 连上、令牌去掉都跟着变），没登记的核心不转；`package.file` 读得到页面目录里的每一份文件，字节和出厂的一样。
//! 运行日志记登记成了、取过令牌，不记令牌的值。桥认不出的方法（核心不会转，测试当核心直接发）回 -32601。

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::Connection;
use miyu_session::testkit::{Play, Script};
use miyu_store::root::DataRoot;

use crate::support::ports::on_free_ports;
use crate::support::spawning::{bridge_up, cli, ports_config, served_up, text};
use crate::support::*;

/// 出厂的后台页目录。
fn page_dir() -> PathBuf {
    resources().join("packages/onebot/page")
}

/// 照网页软件的样子连着核心的头：出示本机令牌，一问一答，交回整条回应（拒绝的 `data` 也在）。
struct Head {
    lines: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    next: u64,
}

impl Head {
    /// 连数据根 `root` 上跑着的核心、握手。
    async fn connect(root: &DataRoot) -> Head {
        let (connection, token) = miyu_ipc::connect(root).await.expect("连得上核心");
        let (read, write) = tokio::io::split(connection);
        let mut head = Head {
            lines: BufReader::new(read),
            write,
            next: 0,
        };
        let hello =
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": token});
        let shaken = head.call("hello", hello).await;
        assert!(shaken.get("result").is_some(), "握手：{shaken}");
        head
    }

    /// 发一条请求，等到它的回应，原样交回。
    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = format!("head-{}", self.next);
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        self.write.flush().await.expect("写得出");
        within("核心回", async {
            loop {
                let mut line = String::new();
                let read = self.lines.read_line(&mut line).await.expect("读得了");
                assert!(read > 0, "核心断开了");
                let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if reply["id"] == id && reply.get("method").is_none() {
                    return reply;
                }
            }
        })
        .await
    }

    /// 经 `package.call` 调接入QQ 登记的 `method`：交回整条回应。
    async fn page_call(&mut self, method: &str) -> Value {
        self.call(
            "package.call",
            json!({"package": "onebot", "method": method, "params": {}}),
        )
        .await
    }

    /// 调 `method`，直到回的 `result` 合 `wanted`（桥登记方法、换配置、NapCat 连上都是异步的）：交回那一次的 `result`。
    async fn until(&mut self, method: &str, wanted: impl Fn(&Value) -> bool) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            let reply = self.page_call(method).await;
            if wanted(&reply["result"]) {
                return reply["result"].clone();
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "等不到 {method}：{reply}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// 经 `package.file` 读后台页里的 `path`，一块块读到头：交回整份字节。
    async fn file(&mut self, path: &str) -> Vec<u8> {
        let mut all = Vec::new();
        loop {
            let reply = self
                .call(
                    "package.file",
                    json!({"package": "onebot", "path": path, "offset": all.len()}),
                )
                .await;
            let data = reply["result"]["data"]
                .as_str()
                .unwrap_or_else(|| panic!("{path}：{reply}"));
            all.extend(base64(data));
            if reply["result"]["eof"] == true {
                assert_eq!(reply["result"]["size"], all.len(), "{path}");
                return all;
            }
        }
    }
}

/// 标准的 base64 解出字节（`package.file` 的 `data`；测试不为它加依赖）。
fn base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let (mut bits, mut held) = (0u32, 0u32);
    for c in text.bytes().filter(|c| *c != b'=') {
        let value = ALPHABET
            .iter()
            .position(|known| *known == c)
            .unwrap_or_else(|| panic!("不是 base64：{c}"));
        bits = (bits << 6) | u32::try_from(value).expect("小于 64");
        held += 6;
        if held >= 8 {
            held -= 8;
            bytes.push(u8::try_from((bits >> held) & 0xff).expect("一个字节"));
        }
    }
    bytes
}

/// 目录 `dir` 里每一份文件相对它的路径（`/` 隔开）。
fn files(dir: &Path, prefix: &str, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("列得出") {
        let entry = entry.expect("读得了");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().expect("看得出种类").is_dir() {
            files(&entry.path(), &format!("{name}/"), found);
        } else {
            found.push(name);
        }
    }
}

#[tokio::test]
async fn the_page_reaches_the_bridge_through_the_core() {
    let script = Script::new([Play::Says("在。")]);
    let (home, listen) = on_free_ports(async |listen, web| {
        let home = Home::spawning(&script, &ports_config(listen, web));
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen, web, None).await?;
        Ok((home, listen))
    })
    .await;
    let mut head = Head::connect(&home.root).await;

    let listed = head.call("package.list", json!({})).await;
    let onebot = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|one| one["package"] == "onebot")
        .cloned()
        .unwrap_or_else(|| panic!("没有 onebot：{listed}"));
    assert_eq!(onebot["icon"], "message-circle", "{onebot}");
    assert_eq!(onebot["page"], true, "{onebot}");

    // 桥起来、开好监听以后才登记：等到调得到。
    let status = head.until("status", |result| result.is_object()).await;
    assert_eq!(
        status,
        json!({
            "napcat": {"connected": false},
            "listen": listen,
            "path": "/ws",
            "token": "set",
            "platform": "qq",
        })
    );
    let mut napcat = admin_napcat(listen).await;
    napcat.version().await;
    let status = head
        .until("status", |result| result["napcat"]["version"].is_string())
        .await;
    assert_eq!(
        status["napcat"],
        json!({
            "connected": true,
            "self_id": BOT.to_string(),
            "implementation": "NapCat.Onebot",
            "version": "4.8.0",
        })
    );
    assert_eq!(
        head.page_call("connection.token").await["result"],
        json!({"token": TOKEN})
    );
    let unknown = head.page_call("apply").await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unregistered",
        "核心只转桥登记了的：{unknown}"
    );

    // 去掉令牌：核心推给桥，桥手里没有了。
    let unset = head
        .call(
            "config.set",
            json!({"layer": "system", "changes": [{"key": "onebot.token", "unset": true}]}),
        )
        .await;
    assert!(unset.get("result").is_some(), "{unset}");
    head.until("status", |result| result["token"] == "none")
        .await;
    assert_eq!(
        head.page_call("connection.token").await["result"],
        json!({"token": null})
    );

    let mut found = Vec::new();
    files(&page_dir(), "", &mut found);
    assert!(found.contains(&"index.html".to_string()), "{found:?}");
    assert!(found.contains(&"texts.js".to_string()), "{found:?}");
    for path in &found {
        let shipped = std::fs::read(page_dir().join(path)).expect("读得了");
        assert_eq!(head.file(path).await, shipped, "{path}");
    }
    assert_eq!(
        head.file("").await,
        std::fs::read(page_dir().join("index.html")).expect("读得了"),
        "空的路径是入口"
    );

    let log = std::fs::read_to_string(home.root.state().join("logs").join("onebot.log"))
        .expect("桥写了运行日志");
    assert!(log.contains("methods registered count=2"), "{log}");
    assert_eq!(log.matches("page token read").count(), 2, "{log}");
    assert!(!log.contains(TOKEN), "运行日志不记令牌的值：{log}");
    napcat.close().await;
    home.stop_extensions().await;
}

#[tokio::test]
async fn the_bridge_registers_its_methods_and_refuses_ones_it_does_not_know() {
    let (dir, root) = temp_root();
    let (mut served, listen) =
        on_free_ports(async |listen, web| Ok((served_up(&root, listen, web).await?, listen))).await;
    // 握手以后先登记工具，再登记方法（不写时限）。
    let registered = within("桥登记方法", async {
        loop {
            let found = served
                .seen
                .lock()
                .expect("没 panic")
                .iter()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|one| one["method"] == "package.methods");
            if let Some(found) = found {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert_eq!(
        registered["params"],
        json!({"methods": [{"name": "status"}, {"name": "connection.token"}]})
    );
    for (id, method) in [
        ("core-1", "apply"),
        ("core-2", "status"),
        ("core-3", "connection.token"),
    ] {
        served
            .send(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "method.call",
                "params": {"method": method, "params": {}},
            }))
            .await;
    }
    let refused = served.answer("core-1").await;
    assert_eq!(
        refused["error"],
        json!({"code": -32601, "message": "unknown_method", "data": {"reason": "unknown_method"}})
    );
    // 测试当的核心握手时没交令牌。
    assert_eq!(
        served.answer("core-2").await["result"],
        json!({
            "napcat": {"connected": false},
            "listen": listen,
            "path": "/ws",
            "token": "none",
            "platform": "qq",
        })
    );
    assert_eq!(
        served.answer("core-3").await["result"],
        json!({"token": null})
    );
    drop(served.stdin.take());
    let exited = within("桥退出", served.child.wait_with_output())
        .await
        .expect("等得到");
    assert_eq!(exited.status.code(), Some(0), "{}", text(&exited.stderr));
    let log =
        std::fs::read_to_string(root.state().join("logs").join("onebot.log")).expect("有运行日志");
    assert!(
        log.contains("page method not understood method=apply"),
        "{log}"
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}
