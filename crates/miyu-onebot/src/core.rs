//! 跟核心的那一头（`onebot.md` 第一条「怎么走」第 1、7 到 11 条）：在给的管道（[`Pipe`]：程序里是核心亲手给的标准输入输出，
//! 施工 O-18；测试里是内存里的管道）上握手，不带凭据（`protocol.md`「握手」第 3 条）；之后说 JSON-RPC，一行一条。管道上只有
//! 协议：桥别处不往标准输出写。握手等回应有期限（`bridge.json` 的 `hello_seconds`）：从终端跑起来的等不到就退。握手的回应里
//! 核心交来这个包自己的配置（`config`，施工 O-20，`extensions.md`「配置」），之后变了推 `extension.config`，`route` 交给
//! `serve`。
//!
//! 一条连接只有一个用的人（`route`）：它发一条请求、等到回应才发下一条；等回应时来的推送留着，之后照先后交出去，一条都
//! 不丢（照终端的头的 `rpc.rs`）。命令编号自己编的（`venue.session`、`subscribe`）带一段随机前缀：同一个编号再发，核心交回
//! 上一次的结果（`venues.md`「`venue.session`」第 4 条），桥重启以后从 1 数起就会撞上。`session.send`、`command.run`（O-19）
//! 的编号由调的一方拼（第 8 条）。
//!
//! 问判官的任务不等这一个用的人：经并着发的调用口（[`Caller`]，施工 O-23 下）调，回应由读的一头照编号分出去。
//!
//! 桥也是提供者（施工 O-26，`provider`）：核心发来的请求（`tool.call`）读的一头当场答，不交给这一个用的人。后台页调的方法
//! （`method.call`，施工 O-28 上，`methods`）也一样。撤回、禁言、戳一戳（施工 O-31）照推送交给这一个用的人，答的时候照
//! [`Core::answerer`] 写回去。

pub(crate) mod blobs;
mod caller;
pub(crate) mod methods;
mod provider;
pub(crate) mod route;

use std::collections::VecDeque;
use std::sync::Arc;

use serde_json::{Value, json};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::rules::Tools;
use crate::serve::{Failure, Pipe};
pub(crate) use caller::Caller;
use caller::{Waiting, Writer, write_line};
use methods::Methods;
use provider::Heard;
pub(crate) use provider::{Answerer, provide};

/// 核心关了管道、写不出去：桥照第 11 条好好停下。
#[derive(Debug)]
pub(crate) struct Gone;

/// 连着核心的一头。
pub(crate) struct Core {
    /// 写的一头：和并着发的调用口共用（施工 O-23 下）。
    writer: Writer,
    /// 并着发的调用口等着的回应：读的一头照编号分出去（施工 O-23 下）。
    waiting: Waiting,
    /// 读进来的回应和推送，照先后。
    incoming: mpsc::UnboundedReceiver<Value>,
    /// 等回应时来的推送。
    held: VecDeque<Value>,
    /// 自己编的命令编号的前缀：`onebot-` 加一段随机数。
    prefix: String,
    /// 下一条自己编的序号。
    next: u64,
    /// 读的任务：放下 `Core` 时掐掉它，连接当场关上。
    reading: JoinHandle<()>,
    /// 握手回的语言：`zh`、`en`、`ja` 之一。
    pub(crate) language: String,
    /// 握手回应交来的配置（施工 O-20）：没带这一格的是 `null`，读的一方照默认值（`crate::settings::Settings::handed`）。
    pub(crate) config: Value,
    /// 握手回的桥自己的账号（核心 O-4 中以后是系统账号 `onebot`）：`venue.session` 回的会话属主是它的，是陌生人（`route`，
    /// 「施工时定的」第 49 条）。没回的是空的。
    pub(crate) account: Option<String>,
}

impl Core {
    /// 在管道 `pipe` 上握手：哪个头、系统的语言 `locale`、没有人能当场回答，不带凭据（管道是核心亲手给的）。回应最多等
    /// `wait`。核心发来的请求照 `tools` 答（施工 O-26），`method.call` 照 `methods` 答（施工 O-28 上）。
    ///
    /// # Errors
    ///
    /// `wait` 里等不到回应：[`Failure::NotSpawned`]（从终端跑起来的）。被拒、握手时管道关了、回应没带 `language`：
    /// [`Failure::Core`]，带原因。
    pub(crate) async fn connect(
        pipe: Pipe,
        locale: Option<&str>,
        wait: Duration,
        tools: Arc<Tools>,
        methods: Arc<Methods>,
    ) -> Result<Core, Failure> {
        let (sender, incoming) = mpsc::unbounded_channel();
        let prefix = format!("onebot-{}", prefix());
        let waiting = Waiting::new(format!("{prefix}-side-"));
        let writer: Writer = Arc::new(tokio::sync::Mutex::new(pipe.write));
        let asked = Asked {
            tools,
            methods,
            writer: Arc::clone(&writer),
        };
        let reading = tokio::spawn(read_all(
            BufReader::new(pipe.read),
            sender,
            waiting.clone(),
            asked,
        ));
        let mut core = Core {
            writer,
            waiting,
            incoming,
            held: VecDeque::new(),
            prefix,
            next: 0,
            reading,
            language: String::new(),
            config: Value::Null,
            account: None,
        };
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": "onebot", "version": env!("CARGO_PKG_VERSION")},
            "locale": locale,
            "caps": {"input": false},
        });
        let gone = || Failure::Core("disconnected during hello".to_string());
        let reply = tokio::time::timeout(wait, core.call("hello", hello))
            .await
            .map_err(|_| Failure::NotSpawned)?
            .map_err(|Gone| gone())?;
        if let Some(message) = reply["error"]["message"].as_str() {
            return Err(Failure::Core(message.to_string()));
        }
        // 握手的回应一定带 `language`（`protocol.md`「握手」）：没带的是协议不对，照连不上核心退，不猜一种。
        let Some(language) = reply["result"]["language"].as_str() else {
            return Err(Failure::Core("hello reply without a language".to_string()));
        };
        core.language = language.to_string();
        core.config = reply["result"]["config"].clone();
        core.account = reply["result"]["account"].as_str().map(str::to_string);
        Ok(core)
    }

    /// 发一条请求，编号自己编，等到它的回应（接受的、拒绝的都交回原样）。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(crate) async fn call(&mut self, method: &str, params: Value) -> Result<Value, Gone> {
        self.next += 1;
        let id = format!("{}-{}", self.prefix, self.next);
        self.call_as(&id, method, params).await
    }

    /// 同 [`Core::call`]，命令编号是 `id`。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(crate) async fn call_as(
        &mut self,
        id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, Gone> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        write_line(&self.writer, &request).await?;
        while let Some(message) = self.incoming.recv().await {
            if message["id"] == json!(id) && message.get("method").is_none() {
                return Ok(message);
            }
            self.held.push_back(message);
        }
        Err(Gone)
    }

    /// 下一条推送（或者不认识的回应），先交留着的。核心断开了是空的。可以在 `select!` 里取消：取消了什么都不丢。
    pub(crate) async fn next(&mut self) -> Option<Value> {
        match self.held.pop_front() {
            Some(message) => Some(message),
            None => self.incoming.recv().await,
        }
    }

    /// 往核心写 `tool.call` 回应的一头（施工 O-31）：平台工具的任务各拿一份。
    pub(crate) fn answerer(&self) -> Answerer {
        Answerer::new(&self.writer)
    }

    /// 并着发的调用口（施工 O-23 下）：问判官的任务各拿一份，不等这一头手上的事。
    pub(crate) fn caller(&self) -> Caller {
        Caller::new(Arc::clone(&self.writer), self.waiting.clone())
    }

    /// 留着的推送里会话 `session` 的事件（`event`），照先后取出来；别的照旧留着，先后不变（施工 O-23，`onebot.md` 第一条
    /// 「群里怎么叫她」第 3 条）。核心先推、后回应：一条命令记下的事件，回应到了就都在留着的里面，判一条群消息以前先收它们。
    pub(crate) fn take_events(&mut self, session: &str) -> Vec<Value> {
        let (taken, kept): (VecDeque<Value>, VecDeque<Value>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|message| {
                message["method"] == "event" && message["params"]["session"] == session
            });
        self.held = kept;
        taken.into()
    }
}

/// 放下了：读的任务跟着停，连接两半都放下，关上。
impl Drop for Core {
    fn drop(&mut self) {
        self.reading.abort();
    }
}

/// 拒绝的回应里给程序看的原因码；接受的是空的。
pub(crate) fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// 编号前缀：8 个随机字节写成 16 位十六进制；取不到随机数的，用进程号和此刻的纳秒（照终端的头）。
fn prefix() -> String {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        Ok(()) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        Err(_) => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            format!("{:x}{nanos:x}", std::process::id())
        }
    }
}

/// 核心发来的请求怎么答（施工 O-26）：照桥的工具答，后台页调的方法照 `methods` 答（施工 O-28 上），回应往这一头写。
struct Asked {
    tools: Arc<Tools>,
    methods: Arc<Methods>,
    writer: Writer,
}

/// 读的一头：一行一条，读不懂的不要；并着发的调用口的回应照编号交给 `waiting`（施工 O-23 下），核心发来的请求照 `asked` 当场
/// 答（施工 O-26）。读到头（核心关了管道）就停：`incoming` 跟着关、`waiting` 关上，用的一方看到的是断开。
async fn read_all(
    reader: BufReader<Box<dyn AsyncRead + Send + Unpin>>,
    sender: mpsc::UnboundedSender<Value>,
    waiting: Waiting,
    asked: Asked,
) {
    read_lines(reader, &sender, &waiting, &asked).await;
    waiting.close();
}

/// [`read_all`] 读到头为止的那一段。
async fn read_lines(
    mut reader: BufReader<Box<dyn AsyncRead + Send + Unpin>>,
    sender: &mpsc::UnboundedSender<Value>,
    waiting: &Waiting,
    asked: &Asked,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        match serde_json::from_str::<Value>(&line) {
            Ok(message) => {
                let Some(message) = waiting.sort(message) else {
                    continue;
                };
                let message = match provider::heard(&asked.tools, &asked.methods, message) {
                    Heard::Asked(reply) => {
                        provider::reply(&asked.writer, reply);
                        continue;
                    }
                    Heard::Cancelled => continue,
                    Heard::Other(message) => message,
                };
                if sender.send(message).is_err() {
                    return;
                }
            }
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = %error, "core line not understood");
            }
        }
    }
}
