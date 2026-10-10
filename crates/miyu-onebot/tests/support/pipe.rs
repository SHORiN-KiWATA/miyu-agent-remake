//! 桥跟核心的那一头在进程里跑的测试里（施工 O-18，`onebot.md` 第一条「施工时定的」第 28 条）：内存里的管道。程序里那是
//! 核心亲手给的标准输入输出，握手不带凭据；测试里的核心（真的、替身）只在本机套接字上听，要本机令牌。这一层在测试那一头：
//! 读桥发的第一行（握手），查它没带凭据，补上本机令牌，经本机套接字交给核心，之后两头照转。核心「不看凭据」的入口不为测试
//! 公开；桥不带凭据也握得成，由真核心拉起真桥的测试守着（`spawned.rs`）。
//!
//! 握手交配置、推送 `extension.config` 只给核心亲手拉起的连接（施工 O-20，`extensions.md`「配置」），本机套接字上的核心不给：
//! 这一层照它的样子在握手的回应里填上 `config`，测试要改配置的经 [`Relay`] 照推送的样子写给桥（「施工时定的」第 47 条）。
//! 要的话再照核心 O-4 中（系统账号）以后的样子改写账号（[`Accounts`]，「施工时定的」第 49 条），记下桥问了核心哪些方法。
//! 施工 O-26：照核心反向调用的样子往桥推一条请求（[`Relay::request`]），记下桥回核心的回应（[`Relay::answers`]）；回应照转给
//! 核心，核心对不上编号的不理。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use miyu_onebot::serve::Pipe;
use miyu_store::root::DataRoot;

/// 照核心 O-4 中以后的样子改写的账号：握手回应里桥自己的 `account` 写成 `own`；接受了的 `venue.session` 回应带上这个会话的
/// 属主 `account`，写成 `venue`，空的不带这一格。
#[derive(Debug, Clone, Copy)]
pub struct Accounts {
    /// 桥自己的账号（O-4 中以后是系统账号 `onebot`）。
    pub own: &'static str,
    /// `venue.session` 找到、造出的会话的属主。
    pub venue: Option<&'static str>,
}

/// 测试那一头的转接：往桥那一头推核心的推送，看桥问了核心什么。放下了只是不再推，照转照旧。
#[derive(Clone)]
pub struct Relay {
    push: mpsc::UnboundedSender<Value>,
    asked: Arc<Mutex<Vec<String>>>,
    /// 桥回核心的回应（没有 `method` 的一行），照先后（施工 O-26）。
    answers: Arc<Mutex<Vec<Value>>>,
}

impl Relay {
    /// 照核心推 `extension.config` 的样子推一次配置的变化（施工 O-20）：`keys` 是 `{键: 新值或 null}`。插在核心说的两行之间，
    /// 不切断哪一行。
    pub fn config(&self, keys: Value) {
        let pushed =
            json!({"jsonrpc": "2.0", "method": "extension.config", "params": {"keys": keys}});
        if self.push.send(pushed).is_err() {
            // 照转已经停了（桥、核心有一头断了）：推不过去，和核心那边的推送一样丢掉。
        }
    }

    /// 握手以后桥发给核心的请求的方法，照先后。
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("没 panic").clone()
    }

    /// 照核心反向调用的样子往桥推一条请求或者通知 `message`（施工 O-26）：插在核心说的两行之间，不切断哪一行。
    pub fn request(&self, message: Value) {
        if self.push.send(message).is_err() {
            // 照转已经停了：推不过去，和核心那边断了一样。
        }
    }

    /// 桥回核心的回应（施工 O-26），照先后。
    pub fn answers(&self) -> Vec<Value> {
        self.answers.lock().expect("没 panic").clone()
    }
}

/// 一条接到数据根 `root` 上那个核心的管道：握手接受了的回应里填上 `config`（`null` 的不填），`accounts` 有的照它改写账号。
/// 交回管道和转接。核心连不上的，桥读到头（照握手时管道关了说）。
pub fn pipe_to(root: &DataRoot, config: Value, accounts: Option<Accounts>) -> (Pipe, Relay) {
    let (bridge, near) = tokio::io::duplex(1 << 16);
    let (push, mut pushes) = mpsc::unbounded_channel::<Value>();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let answers = Arc::new(Mutex::new(Vec::new()));
    let relay = Relay {
        push,
        asked: Arc::clone(&asked),
        answers: Arc::clone(&answers),
    };
    let root = root.clone();
    tokio::spawn(async move {
        let (near_read, mut near_write) = tokio::io::split(near);
        let mut near_read = BufReader::new(near_read);
        let mut first = String::new();
        if near_read.read_line(&mut first).await.unwrap_or(0) == 0 {
            return;
        }
        let Ok((core, token)) = miyu_ipc::connect(&root).await else {
            return;
        };
        let mut hello: Value = serde_json::from_str(&first).expect("握手是一行 JSON");
        assert_eq!(hello["method"], "hello", "第一行是握手：{hello}");
        for credential in ["token", "code", "login", "user", "password"] {
            assert!(
                hello["params"].get(credential).is_none(),
                "桥握手不带凭据：{hello}"
            );
        }
        hello["params"]["token"] = json!(token);
        let (core_read, mut core_write) = tokio::io::split(core);
        let mut core_read = BufReader::new(core_read);
        if core_write
            .write_all(format!("{hello}\n").as_bytes())
            .await
            .is_err()
        {
            return;
        }
        // 核心的第一行是握手的回应：接受了的照核心拉起扩展时的样子填上 `config`，要的话改写桥自己的账号。
        let mut reply = String::new();
        if core_read.read_line(&mut reply).await.unwrap_or(0) == 0 {
            return;
        }
        let mut reply: Value = serde_json::from_str(&reply).expect("握手的回应是一行 JSON");
        if reply["result"].is_object() {
            if !config.is_null() {
                reply["result"]["config"] = config;
            }
            // 桥自己的账号照核心拉起它时的样子：系统账号 `onebot`（核心 O-4 下）。本机套接字上的核心当它是管理员，不改写的话主人
            // 私聊的属主（管理员）和它一样，会被当成陌生人。
            reply["result"]["account"] = json!(accounts.map_or("onebot", |accounts| accounts.own));
        }
        if near_write
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .is_err()
        {
            return;
        }
        // 之后两头照转（读整行用 `read_until`：取消了的话读了一半的留在 `line` 里，不丢）。桥发的记下方法，`venue.session` 记下
        // 编号，回应（施工 O-26：答核心的请求）记下整行；核心回它的照 `accounts` 改写；推送插在核心说的两行之间。
        let venues = Mutex::new(HashSet::new());
        let up = async {
            let mut line = Vec::new();
            while matches!(near_read.read_until(b'\n', &mut line).await, Ok(read) if read > 0) {
                if let Ok(request) = serde_json::from_slice::<Value>(&line) {
                    match request["method"].as_str() {
                        Some(method) => {
                            asked.lock().expect("没 panic").push(method.to_string());
                            if method == "venue.session" {
                                venues
                                    .lock()
                                    .expect("没 panic")
                                    .insert(request["id"].to_string());
                            }
                        }
                        None => answers.lock().expect("没 panic").push(request),
                    }
                }
                if core_write.write_all(&line).await.is_err() {
                    return;
                }
                line.clear();
            }
        };
        let down = async {
            let mut line = Vec::new();
            loop {
                tokio::select! {
                    read = core_read.read_until(b'\n', &mut line) => {
                        if !matches!(read, Ok(read) if read > 0) {
                            return;
                        }
                        let line = std::mem::take(&mut line);
                        let line = match accounts {
                            Some(accounts) => owned(line, &venues, accounts),
                            None => line,
                        };
                        if near_write.write_all(&line).await.is_err() {
                            return;
                        }
                    }
                    Some(pushed) = pushes.recv() => {
                        if near_write.write_all(format!("{pushed}\n").as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            }
        };
        tokio::select! {
            () = up => {}
            () = down => {}
        }
    });
    let (read, write) = tokio::io::split(bridge);
    (Pipe::new(read, write), relay)
}

/// 核心说的一行 `line`：是桥问 `venue.session`（编号在 `venues` 里）、核心接受了的回应，照 `accounts.venue` 改写属主；别的原样。
fn owned(line: Vec<u8>, venues: &Mutex<HashSet<String>>, accounts: Accounts) -> Vec<u8> {
    let Ok(mut reply) = serde_json::from_slice::<Value>(&line) else {
        return line;
    };
    let asked = venues
        .lock()
        .expect("没 panic")
        .contains(&reply["id"].to_string());
    if !asked || !reply["result"].is_object() {
        return line;
    }
    match accounts.venue {
        Some(account) => reply["result"]["account"] = json!(account),
        None => {
            if let Some(result) = reply["result"].as_object_mut() {
                result.remove("account");
            }
        }
    }
    format!("{reply}\n").into_bytes()
}
