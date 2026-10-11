//! 测试用的假 MCP 服务（施工 X-1）：在进程里经一对内存管道照剧本演，新旧两个时代、几种不好好说话的都演得出。记下收到的
//! 每一条，测试照它查客户端发了什么。
//!
//! 几件工具：`echo` 照参数 `text` 回文字；`picture` 回一张图和一句话；`fail` 说自己出错了；`structured` 只回结构化的；
//! `slow` 不回（等取消）；`ask` 先反过来问客户端要东西，等它回了再答；`pinged` 先反过来 `ping` 客户端，照它回没回成答；
//! `input` 要补信息（新时代）；`die` 一收到就退出。别的名字回「不认识这件工具」（`-32602`）。

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

use crate::wire::{self, Read};

/// 演哪个时代、怎么对待 `server/discover`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 新时代：答 `server/discover`，每个请求要带认得的版本，不认 `initialize`。
    Modern,
    /// 新时代，但答 `server/discover` 说缺客户端的本事（`-32021`，规范留给自己的错）。
    ModernDemands,
    /// 旧时代：`server/discover` 回「没有这个方法」。
    LegacyRefuses,
    /// 旧时代：`server/discover` 不回。
    LegacySilent,
    /// 旧时代：收到 `server/discover` 就退出。
    LegacyDies,
}

/// 剧本。
#[derive(Debug, Clone)]
pub struct Script {
    /// 哪个时代。
    pub kind: Kind,
    /// 新时代：认得的版本；旧时代：握手时答第一个。
    pub versions: Vec<String>,
    /// `tools/list` 列的，原样。
    pub tools: Vec<Value>,
    /// 一页几件，0 是一页列完。
    pub page: usize,
    /// 新时代的 `ttlMs`。
    pub ttl_ms: Option<u64>,
    /// 第一次列完以后说一次工具变了。
    pub change_after_list: bool,
    /// 给模型的说明。
    pub instructions: Option<String>,
}

impl Script {
    /// 新时代，认 `2026-07-28`，带上面那几件工具。
    pub fn modern() -> Script {
        Script {
            kind: Kind::Modern,
            versions: vec!["2026-07-28".to_string()],
            tools: tools(),
            page: 0,
            ttl_ms: None,
            change_after_list: false,
            instructions: None,
        }
    }

    /// 旧时代 `kind`，握手答 `2025-11-25`，带上面那几件工具。
    pub fn legacy(kind: Kind) -> Script {
        Script {
            kind,
            versions: vec!["2025-11-25".to_string()],
            ..Script::modern()
        }
    }
}

/// 剧本里的几件工具。
pub fn tools() -> Vec<Value> {
    [
        "echo",
        "picture",
        "fail",
        "structured",
        "slow",
        "ask",
        "input",
        "die",
    ]
    .iter()
    .map(|name| {
        json!({
            "name": name,
            "description": format!("The {name} tool."),
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
            "annotations": {"readOnlyHint": *name == "echo"},
        })
    })
    .collect()
}

/// 演着的假服务：查它收到了什么。
#[derive(Clone, Default)]
pub struct Fake {
    received: Arc<Mutex<Vec<Value>>>,
}

impl Fake {
    /// 收到的每一条，照先后。
    pub fn received(&self) -> Vec<Value> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 收到的每一条的方法，回应（没有方法的）写 `answer`。
    pub fn methods(&self) -> Vec<String> {
        self.received()
            .iter()
            .map(|message| {
                message
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or("answer")
                    .to_string()
            })
            .collect()
    }

    fn note(&self, message: Value) {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message);
    }
}

/// 起一个假服务：交回客户端读的一头、写的一头，和查它的 [`Fake`]。
pub fn start(script: Script) -> (ReadHalf<DuplexStream>, WriteHalf<DuplexStream>, Fake) {
    let (ours, theirs) = tokio::io::duplex(1 << 20);
    let (client_read, client_write) = tokio::io::split(ours);
    let fake = Fake::default();
    tokio::spawn(serve(theirs, script, fake.clone()));
    (client_read, client_write, fake)
}

/// 一直演到客户端断开、或者剧本让它退出。
async fn serve(stream: DuplexStream, script: Script, fake: Fake) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut stage = Stage::default();
    while let Ok(Read::Line(line)) = wire::read_line(&mut reader).await {
        let Ok(message) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        fake.note(message.clone());
        let Some(out) = stage.answer(&script, &message) else {
            return;
        };
        for line in out {
            if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() {
                return;
            }
        }
    }
}

/// 演到哪了。
#[derive(Default)]
struct Stage {
    /// 说过工具变了。
    changed: bool,
    /// `ask` 那一次在等客户端回的调用编号。
    asking: Option<Value>,
}

impl Stage {
    /// 收到一条，交回要写出去的几行；交回空的是退出。
    fn answer(&mut self, script: &Script, message: &Value) -> Option<Vec<Vec<u8>>> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端回了我们反过来问的：`ask`、`pinged` 那一次接着答，`pinged` 照它回没回成说。
            return Some(match (message.get("id"), self.asking.take()) {
                (Some(id), Some(call)) if id == "s1" => {
                    vec![complete(script, &call, text("asked"))]
                }
                (Some(id), Some(call)) if id == "p1" => {
                    let said = match message.get("result") {
                        Some(_) => "pong",
                        None => "no pong",
                    };
                    vec![complete(script, &call, text(said))]
                }
                _ => Vec::new(),
            });
        };
        let Some(id) = message.get("id").cloned() else {
            return Some(Vec::new());
        };
        let modern = matches!(script.kind, Kind::Modern | Kind::ModernDemands);
        if method == "server/discover" {
            return self.discover(script, &id, message);
        }
        if method == "initialize" {
            return Some(vec![match modern {
                true => wire::answer(&id, Err((-32601, "Method not found"))),
                false => wire::answer(
                    &id,
                    Ok(json!({
                        "protocolVersion": script.versions[0],
                        "capabilities": {"tools": {"listChanged": true}},
                        "serverInfo": {"name": "fake", "version": "0"},
                        "instructions": script.instructions,
                    })),
                ),
            }]);
        }
        if modern && let Some(refused) = wrong_version(script, &id, message) {
            return Some(vec![refused]);
        }
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "tools/list" => Some(self.list(script, &id, &params)),
            "tools/call" => self.call(script, &id, &params),
            _ => Some(vec![wire::answer(&id, Err((-32601, "Method not found")))]),
        }
    }

    fn discover(&self, script: &Script, id: &Value, message: &Value) -> Option<Vec<Vec<u8>>> {
        match script.kind {
            Kind::Modern => Some(vec![wrong_version(script, id, message).unwrap_or_else(|| {
                wire::answer(
                    id,
                    Ok(json!({
                        "resultType": "complete",
                        "supportedVersions": script.versions,
                        "capabilities": {"tools": {"listChanged": true}},
                        "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "fake", "version": "0"}},
                        "instructions": script.instructions,
                    })),
                )
            })]),
            Kind::ModernDemands => Some(vec![wire::answer(id, Err((-32021, "Missing required client capability")))]),
            Kind::LegacyRefuses => Some(vec![wire::answer(id, Err((-32601, "Method not found")))]),
            Kind::LegacySilent => Some(Vec::new()),
            Kind::LegacyDies => None,
        }
    }

    fn list(&mut self, script: &Script, id: &Value, params: &Value) -> Vec<Vec<u8>> {
        let start = params
            .get("cursor")
            .and_then(Value::as_str)
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0);
        let size = match script.page {
            0 => script.tools.len(),
            page => page,
        };
        let end = (start + size).min(script.tools.len());
        let mut result = json!({"tools": script.tools.get(start..end).unwrap_or_default()});
        if end < script.tools.len() {
            result["nextCursor"] = json!(end.to_string());
        }
        if let Some(ttl) = script.ttl_ms {
            result["ttlMs"] = json!(ttl);
        }
        let mut out = vec![complete(script, id, result)];
        if script.change_after_list && !self.changed && end >= script.tools.len() {
            self.changed = true;
            out.push(wire::notice("notifications/tools/list_changed", json!({})));
        }
        out
    }

    fn call(&mut self, script: &Script, id: &Value, params: &Value) -> Option<Vec<Vec<u8>>> {
        let said = params["arguments"]["text"].as_str().unwrap_or_default();
        let result = match params["name"].as_str().unwrap_or_default() {
            "echo" => text(said),
            "picture" => json!({"content": [
                {"type": "image", "data": "aGk=", "mimeType": "image/png"},
                {"type": "text", "text": "a picture"},
            ]}),
            "fail" => json!({"content": [{"type": "text", "text": "broke"}], "isError": true}),
            "structured" => json!({"content": [], "structuredContent": {"answer": 42}}),
            "slow" => return Some(Vec::new()),
            "pinged" => {
                self.asking = Some(id.clone());
                return Some(vec![wire::request_raw("p1", "ping")]);
            }
            "ask" => {
                self.asking = Some(id.clone());
                return Some(vec![wire::request_raw("s1", "sampling/createMessage")]);
            }
            "input" => json!({"resultType": "input_required", "inputRequests": {}}),
            "die" => return None,
            _ => return Some(vec![wire::answer(id, Err((-32602, "Unknown tool")))]),
        };
        Some(vec![complete(script, id, result)])
    }
}

/// 一段文字的结果。
fn text(text: &str) -> Value {
    json!({"content": [{"type": "text", "text": text}]})
}

/// 回一个成了的结果：新时代的补上 `resultType`（已经写了的不动）。
fn complete(script: &Script, id: &Value, mut result: Value) -> Vec<u8> {
    let modern = matches!(script.kind, Kind::Modern | Kind::ModernDemands);
    if modern && result.get("resultType").is_none() {
        result["resultType"] = json!("complete");
    }
    wire::answer(id, Ok(result))
}

/// 新时代：请求带的版本不在认得的里面，回 `-32022`。
fn wrong_version(script: &Script, id: &Value, message: &Value) -> Option<Vec<u8>> {
    let requested = message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"]
        .as_str()
        .unwrap_or_default();
    if script.versions.iter().any(|version| version == requested) {
        return None;
    }
    Some(wire::answer_with(
        id,
        -32022,
        "Unsupported protocol version",
        json!({"supported": script.versions, "requested": requested}),
    ))
}
