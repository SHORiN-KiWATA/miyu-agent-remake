//! 一次性入口的测试共用的（施工 8-20）：照系统配置的字冻结一份配置、从路由拿一次性入口、造一份问法、读假服务器收到的
//! 请求体和认证头、几个 key 的配置。

use std::sync::Arc;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Reply, Server};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::Message;
use miyu_session::{Ask, Models, OneShot, Routes, Turn, TurnConfig};
use miyu_store::blob::Blobs;

use super::Scratch;
use super::routing::{configs, resolved};

/// 照系统配置的字 `source` 冻结一份，另带取得到的几个密钥。
pub fn frozen(source: &str, secrets: &[(Reference, &str)]) -> TurnConfig {
    let configs = configs(source, secrets);
    let from = Arc::clone(&*configs.borrow());
    Arc::new(Turn::new(resolved(source), from))
}

/// 路由交出的一次性入口。
pub fn entry(routes: &Routes) -> OneShot {
    Models::one_shot(routes).expect("路由交得出一次性入口")
}

/// 一句 user 的话。
pub fn user(text: &str) -> Message {
    Message::User {
        blocks: vec![Block::Text(Text {
            text: text.to_string(),
        })],
    }
}

/// 问一句 `text`：用途 `purpose`，引用 `model`（没有的照 `models.chat`），没有 system、不限输出。
pub fn asking(model: Option<&str>, purpose: &str, text: &str) -> Ask {
    Ask {
        model: model.map(str::to_string),
        purpose: purpose.to_string(),
        system: String::new(),
        messages: vec![user(text)],
        max_tokens: None,
        owner: miyu_kernel::id::AccountId::parse("admin").expect("账号合写法"),
    }
}

/// 一个空的 blob 目录：拿着 `Scratch` 的时候在。
pub fn blobs() -> (Scratch, Blobs) {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.join("blobs"));
    (scratch, blobs)
}

/// 假服务器收到的第 `at` 个请求的请求体。
pub fn body(server: &Server, at: usize) -> serde_json::Value {
    serde_json::from_slice(&server.received()[at].body).expect("请求体是 JSON")
}

/// 假服务器收到的第 `at` 个请求的认证头。
pub fn bearer(server: &Server, at: usize) -> Option<String> {
    server.received()[at]
        .header("authorization")
        .map(str::to_string)
}

/// 几个候选的配置（施工 8-25 起一家一个 key，几个候选只来自池）：`a1`……`a<count>` 几家都在 `base_url`，key 照
/// `{ env = "K<n>" }` 写，值是 `sk-<n>`。一家的 `models.chat` 是 `a1/m`；几家的放进钉住的池 `p`（照这个先后），`models.chat`
/// 是 `@p`。交回配置的字和取得到的几个（`set` 里的，从 1 数）。
pub fn keyed(base_url: &str, count: usize, set: &[usize]) -> (String, Vec<(Reference, String)>) {
    let providers: String = (1..=count)
        .map(|n| {
            format!(
                "[providers.a{n}]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"K{n}\" }}\n\n"
            )
        })
        .collect();
    let chat = match count {
        1 => "[models]\nchat = \"a1/m\"\n".to_string(),
        _ => {
            let members: Vec<String> = (1..=count).map(|n| format!("\"a{n}/m\"")).collect();
            format!(
                "[pools.p]\nmodels = [{}]\nstrategy = \"pin\"\n\n[models]\nchat = \"@p\"\n",
                members.join(", ")
            )
        }
    };
    let secrets = set
        .iter()
        .map(|n| (Reference::Env(format!("K{n}")), format!("sk-{n}")))
        .collect();
    (format!("{providers}{chat}"), secrets)
}

/// 照 [`keyed`] 冻结一份。
pub fn keyed_config(base_url: &str, count: usize, set: &[usize]) -> TurnConfig {
    let (text, secrets) = keyed(base_url, count, set);
    let secrets: Vec<(Reference, &str)> = secrets
        .iter()
        .map(|(reference, value)| (reference.clone(), value.as_str()))
        .collect();
    frozen(&text, &secrets)
}

/// 一次限速：429，没说等多久。
pub fn limited() -> Reply {
    Reply::error(429, &[], r#"{"error":{"message":"Rate limit reached"}}"#)
}
