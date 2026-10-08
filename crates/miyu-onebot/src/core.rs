//! 跟核心的那一头（`onebot.md` 第一条「怎么走」第 1、7 到 11 条）：在给的管道（[`Pipe`]：程序里是核心亲手给的标准输入输出，
//! 施工 O-18；测试里是内存里的管道）上握手，不带凭据（`protocol.md`「握手」第 3 条）；之后说 JSON-RPC，一行一条。管道上只有
//! 协议：桥别处不往标准输出写。握手等回应有期限（`bridge.json` 的 `hello_seconds`）：从终端跑起来的等不到就退。
//!
//! 一条连接只有一个用的人（`route`）：它发一条请求、等到回应才发下一条；等回应时来的推送留着，之后照先后交出去，一条都
//! 不丢（照终端的头的 `rpc.rs`）。命令编号自己编的（`venue.session`、`subscribe`）带一段随机前缀：同一个编号再发，核心交回
//! 上一次的结果（`venues.md`「`venue.session`」第 4 条），桥重启以后从 1 数起就会撞上。`session.send`、`command.run`（O-19）
//! 的编号由调的一方拼（第 8 条）。

pub(crate) mod route;

use std::collections::VecDeque;

use serde_json::{Value, json};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::serve::{Failure, Pipe};

/// 核心关了管道、写不出去：桥照第 11 条好好停下。
#[derive(Debug)]
pub(crate) struct Gone;

/// 连着核心的一头。
pub(crate) struct Core {
    /// 写的一头。
    writer: Box<dyn AsyncWrite + Send + Unpin>,
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
    /// 在管道 `pipe` 上握手：哪个头、系统的语言 `locale`、没有人能当场回答，不带凭据（管道是核心亲手给的）。回应最多等
    /// `wait`。
    ///
    /// # Errors
    ///
    /// `wait` 里等不到回应：[`Failure::NotSpawned`]（从终端跑起来的）。被拒、握手时管道关了、回应没带 `language`：
    /// [`Failure::Core`]，带原因。
    pub(crate) async fn connect(
        pipe: Pipe,
        locale: Option<&str>,
        wait: Duration,
    ) -> Result<Core, Failure> {
        let (sender, incoming) = mpsc::unbounded_channel();
        let mut core = Core {
            writer: pipe.write,
            incoming,
            held: VecDeque::new(),
            prefix: format!("onebot-{}", prefix()),
            next: 0,
            reading: tokio::spawn(read_all(BufReader::new(pipe.read), sender)),
            language: String::new(),
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

/// 读的一头：一行一条，读不懂的不要。读到头（核心关了管道）就停：`incoming` 跟着关，用的一方看到的是断开。
async fn read_all(
    mut reader: BufReader<Box<dyn AsyncRead + Send + Unpin>>,
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
