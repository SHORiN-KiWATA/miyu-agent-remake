//! 跟核心的那一头（`onebot.md` 第一条「怎么走」第 1、7 到 11 条）：用本机套接字连核心，没在跑就拉起（照终端的头，
//! `connect_or_start`），出示本机令牌握手；之后在这一条连接上说 JSON-RPC，一行一条（`protocol.md`）。
//!
//! 一条连接只有一个用的人（`route`）：它发一条请求、等到回应才发下一条；等回应时来的推送留着，之后照先后交出去，一条都
//! 不丢（照终端的头的 `rpc.rs`）。命令编号自己编的（`venue.session`、`subscribe`）带一段随机前缀：同一个编号再发，核心交回
//! 上一次的结果（`venues.md`「`venue.session`」第 4 条），桥重启以后从 1 数起就会撞上。`session.send` 的编号由调的一方拼
//! （第 8 条）。

pub(crate) mod route;

use std::collections::VecDeque;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use crate::serve::{CoreCommand, Failure};

/// 核心断开了、写不出去：桥照第 11 条退出。
#[derive(Debug)]
pub(crate) struct Gone;

/// 核心断了，桥照 [`Failure::CoreGone`] 退出。
impl From<Gone> for Failure {
    fn from(Gone: Gone) -> Failure {
        Failure::CoreGone
    }
}

/// 连着核心的一头。
pub(crate) struct Core {
    /// 写的一头。
    writer: WriteHalf<Connection>,
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
}

impl Core {
    /// 连核心（没在跑的照 `start` 拉起来），握手：哪个头、系统的语言 `locale`、没有人能当场回答。
    ///
    /// # Errors
    ///
    /// 连不上、拉不起来，握手被拒、核心断开，握手的回应没带 `language`：[`Failure::Core`]，带原因。
    pub(crate) async fn connect(
        root: &DataRoot,
        start: CoreCommand,
        locale: Option<&str>,
    ) -> Result<Core, Failure> {
        let (connection, token) = miyu_ipc::connect_or_start(root, || start())
            .await
            .map_err(|error| Failure::Core(error.to_string()))?;
        let (reader, writer) = tokio::io::split(connection);
        let (sender, incoming) = mpsc::unbounded_channel();
        let mut core = Core {
            writer,
            incoming,
            held: VecDeque::new(),
            prefix: format!("onebot-{}", prefix()),
            next: 0,
            reading: tokio::spawn(read_all(BufReader::new(reader), sender)),
            language: String::new(),
        };
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": "onebot", "version": env!("CARGO_PKG_VERSION")},
            "locale": locale,
            "caps": {"input": false},
            "token": token,
        });
        let gone = || Failure::Core("disconnected during hello".to_string());
        let reply = core.call("hello", hello).await.map_err(|Gone| gone())?;
        if let Some(message) = reply["error"]["message"].as_str() {
            return Err(Failure::Core(message.to_string()));
        }
        // 握手的回应一定带 `language`（`protocol.md`「握手」）：没带的是协议不对，照连不上核心退，不猜一种。
        let Some(language) = reply["result"]["language"].as_str() else {
            return Err(Failure::Core("hello reply without a language".to_string()));
        };
        core.language = language.to_string();
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
        let written = async {
            self.writer
                .write_all(format!("{request}\n").as_bytes())
                .await?;
            self.writer.flush().await
        };
        written.await.map_err(|_| Gone)?;
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

/// 读的一头：一行一条，读不懂的不要。读完了（核心断开）就停：`incoming` 跟着关，用的一方看到的是断开。
async fn read_all(
    mut reader: BufReader<ReadHalf<Connection>>,
    sender: mpsc::UnboundedSender<Value>,
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
