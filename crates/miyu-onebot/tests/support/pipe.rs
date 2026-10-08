//! 桥跟核心的那一头在进程里跑的测试里（施工 O-18，`onebot.md` 第一条「施工时定的」第 28 条）：内存里的管道。程序里那是
//! 核心亲手给的标准输入输出，握手不带凭据；测试里的核心（真的、替身）只在本机套接字上听，要本机令牌。这一层在测试那一头：
//! 读桥发的第一行（握手），查它没带凭据，补上本机令牌，经本机套接字交给核心，之后两头原样照转。核心「不看凭据」的入口不为
//! 测试公开；桥不带凭据也握得成，由真核心拉起真桥的测试守着（`spawned.rs`）。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_onebot::serve::Pipe;
use miyu_store::root::DataRoot;

/// 一条接到数据根 `root` 上那个核心的管道。核心连不上的，桥读到头（照握手时管道关了说）。
pub fn pipe_to(root: &DataRoot) -> Pipe {
    let (bridge, near) = tokio::io::duplex(1 << 16);
    let root = root.clone();
    tokio::spawn(async move {
        let mut near = BufReader::new(near);
        let mut first = String::new();
        if near.read_line(&mut first).await.unwrap_or(0) == 0 {
            return;
        }
        let Ok((mut core, token)) = miyu_ipc::connect(&root).await else {
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
        let sent = core.write_all(format!("{hello}\n").as_bytes()).await;
        if sent.is_err() {
            return;
        }
        if tokio::io::copy_bidirectional(&mut near, &mut core)
            .await
            .is_err()
        {
            // 哪一头断了：照转到这里为止，两头随之关上。
        }
    });
    let (read, write) = tokio::io::split(bridge);
    Pipe::new(read, write)
}
