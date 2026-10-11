//! 客户端（施工 X-1，`docs/blueprint/mcp.md`「怎么走」第一条、第四条）：认时代、握手，之后列工具、调工具。
//!
//! 认时代照 MCP 2026-07-28 版的 stdio 办法：先发 `server/discover`，答得上（`DiscoverResult`，或者说版本不对的
//! `-32022`）的是新时代，每个请求在 `_meta` 里带版本、自己是谁、本事；答了别的错、限定时间里没答的是旧时代，退回
//! `initialize` 握手。MCP 规范留给规范自己用的错误码（`-32099` 到 `-32020`）算新时代的错：照它报，不退回。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;

use crate::content::{Called, Listed, Tool};
use crate::error::Failed;
use crate::link::Link;
use crate::{LEGACY, MODERN};

/// 版本不对（新时代的 `UnsupportedProtocolVersionError`）。
const UNSUPPORTED_VERSION: i64 = -32022;

/// MCP 规范留给自己用的错误码：落在这里的是新时代的服务。
const MODERN_ERRORS: std::ops::RangeInclusive<i64> = -32099..=-32020;

/// 翻页最多翻几页：服务一直给下一页的，到这里停。
const MOST_PAGES: usize = 1000;

/// 说哪个时代、哪一版。缓存照它写：下次起来照它直接说。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "era", content = "version", rename_all = "snake_case")]
pub enum Era {
    /// 2026-07-28 起：每个请求自己带版本。
    Modern(String),
    /// 2025-11-25 及更早：先握手。
    Legacy(String),
}

/// 我们是谁：`clientInfo`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// 名字。
    pub name: String,
    /// 版本。
    pub version: String,
}

/// 各等多久。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waits {
    /// 探 `server/discover` 等多久：旧时代的服务可能不答。
    pub probe: Duration,
    /// 握手、列工具每一次等多久。
    pub answer: Duration,
}

impl Default for Waits {
    /// 探 5 秒，别的 30 秒。
    fn default() -> Waits {
        Waits {
            probe: Duration::from_secs(5),
            answer: Duration::from_secs(30),
        }
    }
}

/// 一个说得上话的 MCP 服务。
pub struct Client {
    link: Link,
    era: Era,
    hello: Hello,
    waits: Waits,
    instructions: Option<String>,
}

impl Client {
    /// 经服务的标准输出 `reader`、标准输入 `writer` 说话：认时代、握手。`known` 是上一次认出来的（缓存的）：旧时代的直接握手，
    /// 握不成再重认；新时代的、没有的先探。
    ///
    /// # Errors
    ///
    /// 断了（探的时候服务退出了的也是：旧时代的服务有的收到不认识的请求就退出，调的一方下次照旧时代起它）、等不到、两边
    /// 没有共同的版本、握手被拒。
    pub async fn connect<R, W>(
        reader: R,
        writer: W,
        hello: Hello,
        known: Option<Era>,
        waits: Waits,
    ) -> Result<Client, Failed>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut client = Client {
            link: Link::start(reader, writer),
            era: Era::Legacy(String::new()),
            hello,
            waits,
            instructions: None,
        };
        let legacy_first = matches!(known, Some(Era::Legacy(_)));
        if !legacy_first || client.shake().await.is_err() {
            client.discover().await?;
        }
        Ok(client)
    }

    /// 认出来的时代和版本。
    pub fn era(&self) -> &Era {
        &self.era
    }

    /// 服务给模型的说明（`instructions`），没给的没有。
    pub fn instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// 列全工具（`tools/list`，照 `nextCursor` 翻页）。写法不对的那几项不收，记在 [`Listed::skipped`]。
    ///
    /// # Errors
    ///
    /// 断了、等不到、服务报错、回的写法不对。
    pub async fn tools(&self) -> Result<Listed, Failed> {
        let mut listed = Listed {
            tools: Vec::new(),
            skipped: Vec::new(),
            fresh_for: None,
        };
        let mut cursor: Option<String> = None;
        for page in 0..MOST_PAGES {
            let mut params = Map::new();
            if let Some(cursor) = cursor.take() {
                params.insert("cursor".to_string(), Value::String(cursor));
            }
            let result = self
                .ask("tools/list", params, Some(self.waits.answer))
                .await?;
            let Some(tools) = result.get("tools").and_then(Value::as_array) else {
                return Err(Failed::Malformed("tools/list without tools".to_string()));
            };
            for raw in tools {
                match Tool::read(raw) {
                    Ok(tool) => listed.tools.push(tool),
                    Err(problem) => listed.skipped.push(problem),
                }
            }
            if page == 0 {
                listed.fresh_for = result
                    .get("ttlMs")
                    .and_then(Value::as_u64)
                    .map(Duration::from_millis);
            }
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
                _ => return Ok(listed),
            }
        }
        Err(Failed::Malformed(format!(
            "tools/list went past {MOST_PAGES} pages"
        )))
    }

    /// 调工具 `name`，参数 `arguments`（一个对象）。不限时：调的一方自己限，限到了丢掉这个 future，服务收到取消。
    ///
    /// # Errors
    ///
    /// 断了、服务报错（工具不存在、参数不对）、回的写法不对、要补信息再问。工具自己说出错了不算：照 [`Called::is_error`]。
    pub async fn call(&self, name: &str, arguments: Value) -> Result<Called, Failed> {
        let mut params = Map::new();
        params.insert("name".to_string(), Value::String(name.to_string()));
        params.insert("arguments".to_string(), arguments);
        let result = self.ask("tools/call", params, None).await?;
        Ok(Called::read(&result))
    }

    /// 服务说工具变了几次：变了就再列一次。
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.link.changes()
    }

    /// 等到连接断了（服务退出了、关了输出）。
    pub async fn closed(&self) {
        self.link.closed().await;
    }

    /// 照认出来的时代发一个请求：新时代的参数里带 `_meta`、结果要是完整的。
    async fn ask(
        &self,
        method: &str,
        mut params: Map<String, Value>,
        wait: Option<Duration>,
    ) -> Result<Value, Failed> {
        if let Era::Modern(version) = &self.era {
            params.insert("_meta".to_string(), self.meta(version));
        }
        let result = self
            .link
            .ask(method, Value::Object(params), wait, true)
            .await?;
        match result.get("resultType").and_then(Value::as_str) {
            None | Some("complete") => Ok(result),
            Some("input_required") => Err(Failed::InputRequired),
            Some(other) => Err(Failed::Malformed(format!("unknown resultType {other}"))),
        }
    }

    /// 新时代每个请求带的 `_meta`。
    fn meta(&self, version: &str) -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": version,
            "io.modelcontextprotocol/clientInfo": {"name": self.hello.name, "version": self.hello.version},
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    }

    /// 探：`server/discover`。新时代的照它列的挑版本；版本不对的照错里列的挑；别的错、没答的退回握手。
    async fn discover(&mut self) -> Result<(), Failed> {
        let mut version = MODERN[0].to_string();
        for _ in 0..2 {
            let params = json!({"_meta": self.meta(&version)});
            let probed = self
                .link
                .ask("server/discover", params, Some(self.waits.probe), false)
                .await;
            let offered = match probed {
                Ok(result) => {
                    self.instructions = text(&result, "instructions");
                    versions(result.get("supportedVersions"))
                }
                Err(Failed::Rpc { code, data, .. }) if code == UNSUPPORTED_VERSION => {
                    versions(data.as_ref().and_then(|data| data.get("supported")))
                }
                Err(Failed::Rpc { code, .. }) if !MODERN_ERRORS.contains(&code) => {
                    return self.shake().await;
                }
                Err(Failed::TimedOut | Failed::Malformed(_)) => return self.shake().await,
                Err(other) => return Err(other),
            };
            let Some(common) = MODERN
                .iter()
                .find(|ours| offered.iter().any(|theirs| theirs == *ours))
            else {
                return Err(Failed::NoCommonVersion(offered));
            };
            if *common == version {
                self.era = Era::Modern(version);
                return Ok(());
            }
            version = (*common).to_string();
        }
        Err(Failed::Malformed(
            "server/discover kept changing its versions".to_string(),
        ))
    }

    /// 旧时代的握手：报我们认得的最新一版，服务答的那一版我们认得就照它说，再发 `notifications/initialized`。
    async fn shake(&mut self) -> Result<(), Failed> {
        let params = json!({
            "protocolVersion": LEGACY[0],
            "capabilities": {},
            "clientInfo": {"name": self.hello.name, "version": self.hello.version},
        });
        let result = self
            .link
            .ask("initialize", params, Some(self.waits.answer), false)
            .await?;
        let Some(version) = text(&result, "protocolVersion") else {
            return Err(Failed::Malformed(
                "initialize without protocolVersion".to_string(),
            ));
        };
        if !LEGACY.contains(&version.as_str()) {
            return Err(Failed::NoCommonVersion(vec![version]));
        }
        self.instructions = text(&result, "instructions");
        self.link.tell("notifications/initialized", json!({}));
        self.era = Era::Legacy(version);
        Ok(())
    }
}

/// 一格字，没有的、不是字的没有。
fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// 一串版本：不是数组的当空的，不是字的不算。
fn versions(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}
