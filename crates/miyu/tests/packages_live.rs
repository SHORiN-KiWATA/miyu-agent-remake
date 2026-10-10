//! 装卸以后生成的文件当场跟着换（施工 F-5 补，`docs/blueprint/packages.md`「配置项」第 2 条）：真的 `miyu core`，经协议装上
//! 一个声明了配置项的包，`state/config/` 的参考文件照新的清单重写、有它那一组（包的配置项的字照核心这时的清单）；卸掉又没了。

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines, ReadHalf, WriteHalf};

use crate::support::{Home, within};
use miyu_ipc::{Connection, connect_or_start};

/// 一份声明了一项配置的界面包的清单：程序不在也照样有配置项（施工 F-6 上：程序不在的扩展、小程序当没装，界面不算）。
const XCFG: &str = "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"X\" }\n\n[command]\nname = \"xcfg\"\nprogram = \"miyu-nothing\"\nabout = { en = \"X\" }\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\"]\nname = { en = \"Port\" }\n";

/// 连接的两头。
struct Talk {
    lines: Lines<BufReader<ReadHalf<Connection>>>,
    write: WriteHalf<Connection>,
}

impl Talk {
    /// 发一条请求，读到它的回应为止：中间的推送不要。
    async fn ask(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let line = within("回应", self.lines.next_line())
                .await
                .expect("读得到")
                .expect("没断开");
            let reply: Value = serde_json::from_str(&line).expect("是 JSON");
            if reply["id"] == json!(id) {
                return reply;
            }
        }
    }
}

/// 等到参考文件合 `wanted`，最多六十秒，交回它。
async fn until_reference(home: &Home, wanted: impl Fn(&str) -> bool) -> String {
    let path = home.root.state().join("config").join("reference.toml");
    within("参考文件重写", async {
        loop {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if wanted(&text) {
                return text;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
}

#[tokio::test]
async fn generated_files_follow_an_installed_package() {
    let home = Home::new();
    let source_dir = home.dir.with_extension("src");
    std::fs::create_dir_all(&source_dir).expect("建得了目录");
    let source = source_dir.join("xcfg");
    std::fs::create_dir_all(&source).expect("建得了目录");
    std::fs::write(source.join("package.toml"), XCFG).expect("写得进");
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let (read, write) = tokio::io::split(connection);
    let mut talk = Talk {
        lines: BufReader::new(read).lines(),
        write,
    };
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
                       "caps": {"input": false}, "token": token});
    let reply = talk.ask("hello-1", "hello", hello).await;
    assert!(reply.get("error").is_none(), "{reply}");
    let before = until_reference(&home, |text| !text.is_empty()).await;
    assert!(!before.contains("[xcfg]"), "{before}");

    let reply = talk
        .ask("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xcfg", "{reply}");
    let after = until_reference(&home, |text| text.contains("[xcfg]")).await;
    assert!(after.contains("port = 8400"), "{after}");

    let reply = talk
        .ask("r1", "package.remove", json!({"package": "xcfg"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    until_reference(&home, |text| !text.is_empty() && !text.contains("[xcfg]")).await;
    drop(talk);
    if let Err(error) = std::fs::remove_dir_all(&source_dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
