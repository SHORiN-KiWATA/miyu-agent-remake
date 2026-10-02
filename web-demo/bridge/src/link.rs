//! 一个浏览器标签页：WebSocket 一帧一条消息，核心那头一行一条（`04-核心协议.md` P2）。
//!
//! 一个标签页一条核心连接，像另一个头（01 第五节：几个头同时连着同一个会话）。浏览器发来的照转，只动这几样：
//! `hello` 里塞上本机令牌。别的都是核心答：读历史用核心的订阅补发，家目录、工作区在握手回应的 `host` 里、真实位置问
//! `fs.realpath`（核心施工 W-3，原来桥答 `web.info`、`web.realpath`）。

use std::process::Command;
use std::sync::Arc;

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

use crate::Site;

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
    let connected = match connect().await {
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
    let reading = {
        let out = out.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
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
        // 只动握手：塞上本机令牌（浏览器读不到，也不该读到）；别的照转
        if message["method"].as_str() == Some("hello") {
            message["params"]["token"] = json!(token);
        }
        if writer.write_all(format!("{message}\n").as_bytes()).await.is_err() {
            break;
        }
    }
    reading.abort();
    writing.abort();
}

pub(crate) type Connected = (miyu_ipc::Connection, String);

/// 往浏览器写的那一头收到它就关掉页面的线：核心那头断了。开头是一个空字符，核心写来的一行 JSON 不会是它。
const CORE_GONE: &str = "\u{0}core-gone";

/// 找数据根、连核心：给了 `MIYU_CORE_BIN` 的，没在跑就拉起来；没给的只连（照 TUI 演示和 `miyu ask`）。
pub(crate) async fn connect() -> Result<Connected, String> {
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
    Ok(connected)
}
