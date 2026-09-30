//! 一个浏览器标签页：WebSocket 一帧一条消息，核心那头一行一条（`04-核心协议.md` P2）。
//!
//! 一个标签页一条核心连接，像另一个头（01 第五节：几个头同时连着同一个会话）。浏览器发来的照转，只动这几样：
//! `hello` 里塞上本机令牌；`events.read` 由桥读日志回（`history.rs`）；`web.info` 回桥知道的几样（在哪个目录、家目录在哪）；
//! `web.human` 回给人看的字（`human.rs`）；`web.mermaid` 回画好的 SVG（`mermaid.rs`）；`web.link_preview` 回链接卡片
//! （`link_preview/`，抓得慢，另起任务回，不挡这条连接上别的消息）；`web.realpath` 回一个路径的真实位置（预览工作区照它比）；
//! `web.upload_done` 删掉桥先收下的附件（`upload.rs`）。

use std::process::Command;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::{Site, history, human, upload};

/// 接一个 WebSocket：口令、Origin 对得上才接；连上核心以后两头照转，哪头断了都停。
pub async fn run(stream: TcpStream, site: Arc<Site>) {
    let (key, port) = (site.key.clone(), site.port);
    #[expect(clippy::result_large_err, reason = "回调的签名是 tungstenite 定的")]
    let check = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let origin = req.headers().get("origin").and_then(|v| v.to_str().ok()).unwrap_or("");
        let origin_ok = origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}");
        let key_ok = req.uri().query().is_some_and(|q| q.split('&').any(|kv| kv == format!("k={key}")));
        if origin_ok && key_ok {
            return Ok(resp);
        }
        let mut refused = ErrorResponse::new(Some("口令或来源不对".to_string()));
        *refused.status_mut() = StatusCode::FORBIDDEN;
        Err(refused)
    };
    let Ok(ws) = accept_hdr_async(stream, check).await else { return };
    let (mut sink, mut source) = ws.split();
    let (root, connected) = match connect().await {
        Ok(ok) => ok,
        Err(reason) => {
            let note = json!({"jsonrpc": "2.0", "method": "bridge.error", "params": {"message": reason}});
            if let Err(e) = sink.send(Message::text(note.to_string())).await {
                eprintln!("{reason}；告诉页面时又出错了：{e}");
            }
            return;
        }
    };
    let (connection, token) = connected;
    let (reader, mut writer) = tokio::io::split(connection);
    let (out, mut outbox) = mpsc::unbounded_channel::<String>();
    // 往浏览器写：核心的推送、回应，和桥自己回的，一条一帧、照先后；核心那头断了（`CORE_GONE`），发一个关闭帧把页面这条线也关掉，
    // 页面收到关闭就去重连（蓝图 `web.md`「连核心」第 1 条；原来只停了读核心的那一半，页面以为自己还在线）
    let writing = tokio::spawn(async move {
        while let Some(text) = outbox.recv().await {
            if text == CORE_GONE {
                drop(sink.send(Message::Close(None)).await);
                return;
            }
            if sink.send(Message::text(text)).await.is_err() {
                return;
            }
        }
    });
    // 握手的回应里有「你是谁」：`events.read` 照这个账号找会话的目录
    let hello_id = Arc::new(Mutex::new(None::<Value>));
    let account = Arc::new(Mutex::new(None::<String>));
    let reading = {
        let (out, hello_id, account, site) = (out.clone(), hello_id.clone(), account.clone(), site.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(message) = serde_json::from_str::<Value>(&line)
                    && let Ok(id) = hello_id.lock()
                    && id.as_ref().is_some_and(|id| *id == message["id"])
                    && let Some(who) = message["result"]["account"].as_str()
                    && let Ok(mut account) = account.lock()
                {
                    *account = Some(who.to_string());
                    // `/blob` 照它找会话日志和 blob（HTTP 的请求不经这条连接）
                    if let Ok(mut shared) = site.account.lock() {
                        *shared = Some(who.to_string());
                    }
                }
                if out.send(line).is_err() {
                    return;
                }
            }
            // 核心那头断了（重启、退出）：叫写的那一头把页面的线关掉
            drop(out.send(CORE_GONE.to_string()));
        })
    };
    while let Some(Ok(frame)) = source.next().await {
        let Message::Text(text) = frame else {
            if matches!(frame, Message::Close(_)) { break }
            continue;
        };
        let Ok(mut message) = serde_json::from_str::<Value>(text.as_str()) else { continue };
        let reply = |result: Result<Value, String>| match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": message["id"], "result": result}),
            Err(reason) => json!({"jsonrpc": "2.0", "id": message["id"], "error": {"code": -32010, "message": reason, "data": {"reason": "bridge"}}}),
        };
        match message["method"].as_str() {
            Some("events.read") => {
                let who = account.lock().ok().and_then(|a| a.clone()).unwrap_or_default();
                let p = &message["params"];
                let got = history::read(&root, &who, p["session"].as_str().unwrap_or(""), p["after"].as_u64().unwrap_or(0))
                    .map(|events| json!({"events": events}));
                if out.send(reply(got).to_string()).is_err() { break }
                continue;
            }
            Some("web.info") => {
                let home = std::env::home_dir().map(|h| h.display().to_string());
                if out.send(reply(Ok(json!({"core": "real", "cwd": cwd(), "home": home}))).to_string()).is_err() { break }
                continue;
            }
            Some("web.mermaid") => {
                let source = message["params"]["source"].as_str().unwrap_or("").to_string();
                let site = site.clone();
                let got = tokio::task::spawn_blocking(move || site.mermaid.render(&source).map(|svg| json!({"svg": svg})))
                    .await
                    .unwrap_or_else(|e| Err(format!("画图的线程出错了：{e}")));
                if out.send(reply(got).to_string()).is_err() { break }
                continue;
            }
            Some("web.link_preview") => {
                // 抓一页要几秒（还要跟重定向、抓图，每一跳的预算照 `link_preview.json`）：另起一个任务，好了再经 `out` 回，这条连接上别的
                // 消息照常走；回应照 `id` 对得上。总是成功的回应：做不出卡片的是 `{ok: false, reason}`
                let (url, id) = (message["params"]["url"].as_str().unwrap_or("").to_string(), message["id"].clone());
                let (site, out) = (site.clone(), out.clone());
                tokio::spawn(async move {
                    let result = site.link_preview.preview(&url).await;
                    // 标签页关了的回不了也不要紧：抓到的照样记着，下次再问直接给
                    drop(out.send(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()));
                });
                continue;
            }
            Some("web.realpath") => {
                // 预览工作区照真实位置比路径（效果里的路径是换成真实位置以后的），页面换不了，桥换。约定的目录还没建（她还没写
                // 第一个文件）的，照最近那一层在的上级换，后面照接：建出来以后是同一个位置
                let path = std::path::Path::new(message["params"]["path"].as_str().unwrap_or(""));
                let real = path.ancestors().find_map(|up| {
                    let base = std::fs::canonicalize(up).ok()?;
                    let rest = path.strip_prefix(up).ok()?;
                    Some(base.join(rest).display().to_string())
                });
                if out.send(reply(Ok(json!({"path": real}))).to_string()).is_err() { break }
                continue;
            }
            Some("web.files") => {
                // `@` 选文件（`mention.rs`）：列一层、模糊找；建清单要走一遍目录，另起一个线程，数据根不给
                let (params, site) = (message["params"].clone(), site.clone());
                let got = tokio::task::spawn_blocking(move || {
                    let root = DataRoot::locate(&Env::current()).map_err(|e| format!("找不到数据根：{e}"))?;
                    site.mention.request(&params, root.path(), &|p| site.types.of(p))
                })
                .await
                .unwrap_or_else(|e| Err(format!("找文件的线程出错了：{e}")));
                if out.send(reply(got).to_string()).is_err() { break }
                continue;
            }
            Some("web.upload_done") => {
                // 核心存好了附件（`blob.put`），桥先收下的那一份删掉（`upload.rs`）
                let got = upload::done(message["params"]["path"].as_str().unwrap_or(""));
                if out.send(reply(got).to_string()).is_err() { break }
                continue;
            }
            Some("web.human") => {
                let got = human::load(message["params"]["language"].as_str().unwrap_or("zh"));
                if out.send(reply(got).to_string()).is_err() { break }
                continue;
            }
            Some("hello") => {
                message["params"]["token"] = json!(token);
                if let Ok(mut id) = hello_id.lock() {
                    *id = Some(message["id"].clone());
                }
            }
            _ => {}
        }
        if writer.write_all(format!("{message}\n").as_bytes()).await.is_err() {
            break;
        }
    }
    reading.abort();
    writing.abort();
}

type Connected = (miyu_ipc::Connection, String);

/// 往浏览器写的那一头收到它就关掉页面的线：核心那头断了。开头是一个空字符，核心写来的一行 JSON 不会是它。
const CORE_GONE: &str = "\u{0}core-gone";

/// 找数据根、连核心：给了 `MIYU_CORE_BIN` 的，没在跑就拉起来；没给的只连（照 TUI 演示和 `miyu ask`）。
async fn connect() -> Result<(DataRoot, Connected), String> {
    let env = Env::current();
    let root = DataRoot::locate(&env).map_err(|e| format!("找不到数据根：{e}"))?;
    root.prepare().map_err(|e| format!("建不了数据根：{e}"))?;
    let connected = match std::env::var_os("MIYU_CORE_BIN") {
        Some(bin) => miyu_ipc::connect_or_start(&root, move || {
            let mut core = Command::new(bin);
            core.arg("core");
            core
        })
        .await
        .map_err(|e| format!("拉不起核心：{e}"))?,
        None => miyu_ipc::connect(&root).await.map_err(|e| match e {
            miyu_ipc::ConnectError::NotRunning => "核心没在跑：启动桥时设 MIYU_CORE_BIN 指向重制版的 miyu".to_string(),
            e => format!("连不上核心：{e}"),
        })?,
    };
    Ok((root, connected))
}

/// 桥是在哪个目录里起的：新会话在这里干活。报绝对路径，和 TUI 演示一样（核心收下 `~/…` 的写法，
/// 但按它解析相对路径时没展开 `~`，相对路径全读不到：2026-09-29 实测，已记下来交给施工那边）。
fn cwd() -> String {
    std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| ".".to_string())
}
