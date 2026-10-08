//! 桥跟核心的那一头在进程里跑的测试里（施工 O-18，`onebot.md` 第一条「施工时定的」第 28 条）：内存里的管道。程序里那是
//! 核心亲手给的标准输入输出，握手不带凭据；测试里的核心（真的、替身）只在本机套接字上听，要本机令牌。这一层在测试那一头：
//! 读桥发的第一行（握手），查它没带凭据，补上本机令牌，经本机套接字交给核心，之后两头原样照转。核心「不看凭据」的入口不为
//! 测试公开；桥不带凭据也握得成，由真核心拉起真桥的测试守着（`spawned.rs`）。
//!
//! 握手交配置、推送 `extension.config` 只给核心亲手拉起的连接（施工 O-20，`extensions.md`「配置」），本机套接字上的核心不给：
//! 这一层照它的样子在握手的回应里填上 `config`，测试要改配置的经 [`Push`] 照推送的样子写给桥（「施工时定的」第 47 条）。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use miyu_onebot::serve::Pipe;
use miyu_store::root::DataRoot;

/// 往桥那一头推核心的推送：照核心推 `extension.config` 的样子（施工 O-20）。放下了只是不再推，照转照旧。
#[derive(Clone)]
pub struct Push(mpsc::UnboundedSender<Value>);

impl Push {
    /// 推一次配置的变化：`keys` 是 `{键: 新值或 null}`。插在核心说的两行之间，不切断哪一行。
    pub fn config(&self, keys: Value) {
        let pushed =
            json!({"jsonrpc": "2.0", "method": "extension.config", "params": {"keys": keys}});
        if self.0.send(pushed).is_err() {
            // 照转已经停了（桥、核心有一头断了）：推不过去，和核心那边的推送一样丢掉。
        }
    }
}

/// 一条接到数据根 `root` 上那个核心的管道：握手接受了的回应里填上 `config`（`null` 的不填），交回管道和推配置的那一头。核心
/// 连不上的，桥读到头（照握手时管道关了说）。
pub fn pipe_to(root: &DataRoot, config: Value) -> (Pipe, Push) {
    let (bridge, near) = tokio::io::duplex(1 << 16);
    let (push, mut pushes) = mpsc::unbounded_channel::<Value>();
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
        // 核心的第一行是握手的回应：接受了的照核心拉起扩展时的样子填上 `config`。
        let mut reply = String::new();
        if core_read.read_line(&mut reply).await.unwrap_or(0) == 0 {
            return;
        }
        let mut reply: Value = serde_json::from_str(&reply).expect("握手的回应是一行 JSON");
        if reply["result"].is_object() && !config.is_null() {
            reply["result"]["config"] = config;
        }
        if near_write
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .is_err()
        {
            return;
        }
        // 之后两头照转；推送插在核心说的两行之间（读整行用 `read_until`：取消了的话读了一半的留在 `line` 里，不丢）。
        let down = async {
            let mut line = Vec::new();
            loop {
                tokio::select! {
                    read = core_read.read_until(b'\n', &mut line) => {
                        if !matches!(read, Ok(read) if read > 0) || near_write.write_all(&line).await.is_err() {
                            return;
                        }
                        line.clear();
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
            _ = tokio::io::copy(&mut near_read, &mut core_write) => {}
            () = down => {}
        }
    });
    let (read, write) = tokio::io::split(bridge);
    (Pipe::new(read, write), Push(push))
}
