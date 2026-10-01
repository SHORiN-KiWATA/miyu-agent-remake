//! 替身发过的每一次请求，和发它时的情形（施工 1-14、2-9 下；施工 C-2 从 `mod.rs` 挪出来）：[`super::check`] 照它查五条性质。

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ContentHash, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::testkit::Stage;

use super::is_summary;

/// 一次请求，和发它时的情形。
pub struct Sent {
    /// 组装出来的请求。
    pub request: Request,
    /// 上一次请求以后撤销、恢复或者压缩过，或者上一次请求里的图这时才转述好（施工 8-17：转述没成、下一轮补上，那条消息在看不了
    /// 图的端点上从占位换成转述）：前缀可以改写。
    pub rewritten: bool,
    /// 这是一个回合的第一次请求，由人的一句话触发：那句话的最后一块。
    pub trigger: Option<Block>,
    /// 这是压缩的摘要请求：最后一块是摘要指令（施工 6-2 上）。
    pub summary: bool,
}

/// 替身发过的每一次请求，和发它时的情形：上一次请求交出去以后、这一次交出去以前，日志里有撤销、恢复、压缩的，算
/// 改写过；一个回合的第一次主请求（上一次主请求在这一轮开始以前），触发它的是人的消息的，记下那句话的最后一块。摘要请求
/// 看到的比交出去时的日志早（施工 6-2 下），所以改写照交出去的那一刻算，第一次照上一次主请求算。
pub fn sent(stage: &Stage) -> Vec<Sent> {
    let log = stage.log();
    let mut before: Option<Seq> = None;
    let mut before_main: Option<Seq> = None;
    let mut sent = Vec::new();
    // 认别的会话：父会话、派的子代理照整份日志记着的（施工 C-2）。
    let mut kept = History::whole();
    for event in log {
        kept.note(event);
    }
    for ((seen, request), mark) in stage.requests().iter().zip(stage.marks()) {
        let since =
            |event: &&Event| before.is_none_or(|before| event.seq > before) && event.seq <= *mark;
        let previous = sent.last().map(|sent: &Sent| &sent.request);
        let rewritten = log.iter().filter(since).any(|event| match &event.body {
            Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::ContextCompacted(_) => true,
            Body::ImageDescribed(described) => {
                previous.is_some_and(|request| shows(request, &described.blob))
            }
            _ => false,
        });
        let summary = is_summary(request);
        let trigger = log
            .iter()
            .filter(|event| event.seq <= *seen)
            .rev()
            .find_map(|event| match &event.body {
                Body::TurnStarted(started) => Some((event.seq, started.trigger)),
                _ => None,
            })
            .filter(|_| !summary)
            .filter(|(turn, _)| before_main.is_none_or(|before| before < *turn))
            .and_then(|(_, trigger)| log.iter().find(|event| Some(event.seq) == trigger))
            // 别的 harness、别的会话发来的话渲染时包了一层标签（施工 7-10、C-2）：最后一块照它们自己的探针查
            // （`probe_harness.rs`、`probe_peers.rs`）。
            .filter(|event| match &event.by {
                By::Harness(_) => false,
                By::Session(session) => !kept.is_peer(&session.id),
                _ => true,
            })
            .and_then(|event| match &event.body {
                Body::MessageUser(message) => message.blocks.last().cloned(),
                _ => None,
            });
        sent.push(Sent {
            summary,
            request: request.clone(),
            rewritten,
            trigger,
        });
        before = Some(*mark);
        if !summary {
            before_main = Some(*seen);
        }
    }
    sent
}

/// 请求里有这张图：user、tool 消息里的图片块（施工 8-17）。
fn shows(request: &Request, blob: &ContentHash) -> bool {
    request.messages.iter().any(|message| match message {
        Message::User { blocks } | Message::Tool { blocks, .. } => blocks
            .iter()
            .any(|block| matches!(block, Block::Image(image) if image.blob == *blob)),
        Message::Assistant { .. } => false,
    })
}
