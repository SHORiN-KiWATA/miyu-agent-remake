//! 共用的：照剧本起假服务、连上。

use std::time::Duration;

use miyu_mcp::testkit::{self, Fake, Script};
use miyu_mcp::{Client, Era, Failed, Hello, Waits};

/// 我们是谁。
pub fn hello() -> Hello {
    Hello {
        name: "miyu".to_string(),
        version: "0.0.0".to_string(),
    }
}

/// 探 200 毫秒（不回的旧服务不用真等 5 秒），别的 10 秒。
pub fn waits() -> Waits {
    Waits {
        probe: Duration::from_millis(200),
        answer: Duration::from_secs(10),
    }
}

/// 照剧本起一个假服务、连上：`known` 是缓存的时代。
pub async fn connect(script: Script, known: Option<Era>) -> (Result<Client, Failed>, Fake) {
    let (reader, writer, fake) = testkit::start(script);
    let client = Client::connect(reader, writer, hello(), known, waits()).await;
    (client, fake)
}

/// 照剧本起、连上，连不上的测试当场失败。
pub async fn connected(script: Script) -> (Client, Fake) {
    let (client, fake) = connect(script, None).await;
    (client.expect("连得上"), fake)
}

/// 等假服务收到 `count` 条，交回它们的方法：通知是写出去就算，收没收到要等一下。最多十秒。
pub async fn methods(fake: &Fake, count: usize) -> Vec<String> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let methods = fake.methods();
            if methods.len() >= count {
                return methods;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("十秒内收齐")
}
