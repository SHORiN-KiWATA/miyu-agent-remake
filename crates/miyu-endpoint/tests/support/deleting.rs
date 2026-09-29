//! 删会话的几个测试共用的（施工 3-8 三补）：回收处在哪、原处还在不在、发删的请求；派子代理的剧本分派、从日志里认出子会话。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::Script;
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use super::{Client, Home, alice, default_resources};

/// 回收处里会话 `session` 的目录。
pub fn trashed(home: &Home, session: &str) -> PathBuf {
    home.root
        .account_dir(&alice())
        .join("trash")
        .join("sessions")
        .join(session)
}

/// 会话目录还在不在原处。
pub fn in_place(home: &Home, session: &str) -> bool {
    let id = SessionId::parse(session).expect("合写法");
    home.root.session_dir(&alice(), &id).exists()
}

/// 回收处里那一份的日志。
pub fn trashed_log(home: &Home, session: &str) -> Vec<Event> {
    read_events(&trashed(home, session)).expect("回收处里的日志读得回来")
}

/// `session.list` 里有哪些会话。
pub async fn listed(client: &mut Client, id: &str) -> Vec<String> {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .map(|item| item["session"].as_str().expect("有编号").to_string())
        .collect()
}

/// 删会话，交回回应之前读到的推送和回应。
pub async fn delete(client: &mut Client, id: &str, session: &str) -> (Vec<Value>, Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "session.delete", "params": {"session": session}});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// 各个会话的剧本：请求里人这边有哪一句，就照哪一份回（几个会话同时请求模型，谁先到不一定）。
#[derive(Clone)]
pub struct Router(pub Arc<Vec<(&'static str, Script)>>);

impl Router {
    pub fn script(&self, request: &Request) -> &Script {
        let said = |key: &str| {
            request.messages.iter().any(|message| match message {
                Message::User { blocks, .. } => blocks.contains(&Block::Text(Text {
                    text: key.to_string(),
                })),
                _ => false,
            })
        };
        self.0
            .iter()
            .find(|(key, _)| said(key))
            .map(|(_, script)| script)
            .unwrap_or_else(|| panic!("没有哪份剧本认这次请求"))
    }
}

impl Models for Router {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Router {
    fn model(&self) -> &Model {
        self.0[0].1.model()
    }

    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel) {
        let script = self.script(&request).clone();
        script.call(seen, request, reports, cancel);
    }
}

/// 派一个子代理，交代是 `prompt`。
pub fn agent(prompt: &str) -> String {
    json!({"description": "查", "prompt": prompt}).to_string()
}

/// 日志里 `job.started` 记着的子会话。
pub fn started_child(log: &[Event]) -> Option<SessionId> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .find_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
}

/// 日志里记了几条 `job.started`。
pub fn started_jobs(log: &[Event]) -> usize {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter(|effect| matches!(effect, Effect::JobStarted(_)))
        .count()
}

/// 日志里有没有回报：后台命令结束的、子会话交来的。
pub fn reported_jobs(log: &[Event]) -> bool {
    log.iter()
        .any(|event| matches!(event.body, Body::JobReported(_) | Body::ChildReported(_)))
}

/// 基础系统的工具：派子代理用。
pub fn base_tools() -> Catalog {
    Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具")).expect("合写法")
}
