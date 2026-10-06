//! 照登录令牌连核心（`web-ui.md`「怎么走」第三条第 2 款）：一个令牌一条连接，几个请求在同一条上同时问，照编号把回应分回去；
//! 60 秒没人用就关（`web-ui.md`「施工时定的」第 12 条）。核心断了、握手被拒的，下一问重连。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::oneshot;

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::serve::CoreCommand;

/// 多久没人用就关。
const IDLE: Duration = Duration::from_secs(60);

/// 握手最多等多久。
const HELLO: Duration = Duration::from_secs(30);

/// 问核心没问成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failed {
    /// 连不上、断了：原因。
    Unreachable(String),
    /// 握手被拒：登录令牌不对、作废了、过期了。
    BadLogin,
    /// 核心拒了这一问：原因码。
    Refused(String),
}

/// 一问的结果。
type Answer = Result<Value, Failed>;

/// 等着回应的：编号 → 交给谁。读的任务和问的一方共用。
#[derive(Default)]
struct Waiting {
    /// 连接还通着。
    alive: AtomicBool,
    senders: Mutex<HashMap<u64, oneshot::Sender<Answer>>>,
}

impl Waiting {
    fn senders(&self) -> std::sync::MutexGuard<'_, HashMap<u64, oneshot::Sender<Answer>>> {
        self.senders.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 断了：先记下，再叫醒全部等着的。
    fn gone(&self) {
        self.alive.store(false, Ordering::SeqCst);
        for (_, sender) in self.senders().drain() {
            if sender
                .send(Err(Failed::Unreachable("core disconnected".into())))
                .is_err()
            {
                // 问的一方已经不等了。
            }
        }
    }
}

/// 一条连着的。
struct Link {
    write: tokio::sync::Mutex<WriteHalf<Connection>>,
    waiting: Arc<Waiting>,
    next: AtomicU64,
    used: Mutex<Instant>,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl Link {
    fn alive(&self) -> bool {
        self.waiting.alive.load(Ordering::SeqCst)
    }

    fn idle_for(&self) -> Duration {
        self.used
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .elapsed()
    }

    /// 问一次，等它的回应。
    async fn call(&self, method: &str, params: Value) -> Answer {
        *self.used.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (sender, answer) = oneshot::channel();
        self.waiting.senders().insert(id, sender);
        // 先登记再看通不通：读的任务先记下断了、再叫醒全部，两边怎么交错都不会漏掉这一问。
        if !self.alive() {
            self.waiting.senders().remove(&id);
            return Err(Failed::Unreachable("core disconnected".into()));
        }
        let line =
            json!({"jsonrpc": "2.0", "id": format!("m{id}"), "method": method, "params": params});
        let written = {
            let mut write = self.write.lock().await;
            match write.write_all(format!("{line}\n").as_bytes()).await {
                Ok(()) => write.flush().await,
                Err(error) => Err(error),
            }
        };
        if let Err(error) = written {
            self.waiting.senders().remove(&id);
            self.waiting.gone();
            return Err(Failed::Unreachable(error.to_string()));
        }
        answer
            .await
            .unwrap_or_else(|_| Err(Failed::Unreachable("core disconnected".into())))
    }
}

/// 连着的几条：登录令牌 → 连接。
#[derive(Default)]
pub(crate) struct Cores {
    links: tokio::sync::Mutex<HashMap<String, Arc<Link>>>,
}

impl Cores {
    /// 照登录令牌 `login` 问一次。用着的连接断了的（核心重启、令牌作废了核心断开），重连再问一次。
    pub(crate) async fn call(
        &self,
        root: &DataRoot,
        core: &CoreCommand,
        login: &str,
        method: &str,
        params: Value,
    ) -> Answer {
        let (link, reused) = self.link(root, core, login).await?;
        match link.call(method, params.clone()).await {
            Err(Failed::Unreachable(_)) if reused => {
                let (link, _) = self.link(root, core, login).await?;
                link.call(method, params).await
            }
            answer => answer,
        }
    }

    /// 这个令牌的连接：通着的照用（交回真），没有的、断了的新连一条。
    async fn link(
        &self,
        root: &DataRoot,
        core: &CoreCommand,
        login: &str,
    ) -> Result<(Arc<Link>, bool), Failed> {
        let mut links = self.links.lock().await;
        if let Some(link) = links.get(login)
            && link.alive()
        {
            return Ok((Arc::clone(link), true));
        }
        links.remove(login);
        let link = Arc::new(connect(root, core, login).await?);
        links.insert(login.to_string(), Arc::clone(&link));
        Ok((link, false))
    }

    /// 关掉断了的、60 秒没人用的（正在问的不关）。
    pub(crate) fn sweep(&self) {
        if let Ok(mut links) = self.links.try_lock() {
            links.retain(|_, link| {
                link.alive() && (link.idle_for() < IDLE || Arc::strong_count(link) > 1)
            });
        }
    }
}

/// 连核心（没在跑就拉起来），照登录令牌握手。
async fn connect(root: &DataRoot, core: &CoreCommand, login: &str) -> Result<Link, Failed> {
    let connection = miyu_ipc::connect_or_start_bare(root, || core())
        .await
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "core unreachable");
            Failed::Unreachable(error.to_string())
        })?;
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let hello = json!({
        "jsonrpc": "2.0",
        "id": "hello",
        "method": "hello",
        "params": {
            "protocol": [1, 1],
            "head": {"kind": "miyu-web", "version": env!("CARGO_PKG_VERSION")},
            "login": login,
        },
    });
    let shaken = async {
        write.write_all(format!("{hello}\n").as_bytes()).await?;
        write.flush().await?;
        let mut line = String::new();
        lines.read_line(&mut line).await?;
        Ok::<_, std::io::Error>(line)
    };
    let line = tokio::time::timeout(HELLO, shaken)
        .await
        .map_err(|_| Failed::Unreachable("no answer to hello".into()))?
        .map_err(|error| Failed::Unreachable(error.to_string()))?;
    let reply: Value =
        serde_json::from_str(&line).map_err(|_| Failed::Unreachable("core disconnected".into()))?;
    if reply.get("result").is_none() {
        return Err(match reason(&reply).as_str() {
            "bad_login" => Failed::BadLogin,
            other => Failed::Refused(other.to_string()),
        });
    }
    let waiting = Arc::new(Waiting::default());
    waiting.alive.store(true, Ordering::SeqCst);
    let reader = tokio::spawn(read_answers(lines, Arc::clone(&waiting)));
    Ok(Link {
        write: tokio::sync::Mutex::new(write),
        waiting,
        next: AtomicU64::new(0),
        used: Mutex::new(Instant::now()),
        reader,
    })
}

/// 读的任务：一行一个回应，照编号交给等着的；断了叫醒全部。
async fn read_answers(mut lines: BufReader<ReadHalf<Connection>>, waiting: Arc<Waiting>) {
    let mut line = String::new();
    while lines.read_line(&mut line).await.is_ok_and(|read| read > 0) {
        if let Ok(reply) = serde_json::from_str::<Value>(&line)
            && let Some(id) = reply["id"]
                .as_str()
                .and_then(|id| id.strip_prefix('m'))
                .and_then(|id| id.parse::<u64>().ok())
            && let Some(sender) = waiting.senders().remove(&id)
        {
            let answer = match reply.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(Failed::Refused(reason(&reply))),
            };
            if sender.send(answer).is_err() {
                // 问的一方已经不等了（浏览器走了）。
            }
        }
        line.clear();
    }
    waiting.gone();
}

/// 拒绝的原因码：`error.data.reason`，没有的写 `refused`。
fn reason(reply: &Value) -> String {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or("refused")
        .to_string()
}
