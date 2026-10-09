//! 回顾的请求（施工 3-8 四补，`docs/blueprint/kernel/request.md`「回顾的请求」）：照 codex 的做法（`codex-rs/tui/src/app/
//! recap_history.rs`、`context-fragments/src/recap_prompt.rs`，2026-10-01 看过源码后定），单独一次辅助请求，不接主对话的前缀、
//! 不带 system 和工具面：长会话也不用把整段上下文再读一遍，快满窗口时也不会超长。
//!
//! 只喂有效历史里最近几轮你看得到的对话：人这边的话（别的 harness、别的会话、子代理发来的照主请求里的外壳写，看得出来处），她每一轮
//! 最后一条有正文的回复；工具调用、工具结果、思考、注入的事实都不要。一轮是一段人这边的话，连同她接着的回答：从新往旧数，
//! 答过的至多 `turns` 轮，最新那一轮没答的也带上。
//!
//! 整份连指令至多 `tokens` 个 token（照本地估算的字节/4 折成字节）。放不下的，先整轮去掉最老的、最前写一行省略了（最新
//! 那一轮回答和它后面没答的那句一定留）；还放不下，每一段留头尾、截掉中间，写明截过。截法照 codex 的 `recap_history`。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::estimate::BYTES_PER_TOKEN;
use miyu_kernel::event::Body;
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, TurnId};
use miyu_kernel::request::{Message, Request};

use crate::texts::{Recap, Texts};

/// 段与段、轮与轮之间空一行：记录的格式，照 codex。
pub(crate) const GAP: &str = "\n\n";

/// 回顾的请求，和它照到的那一条：喂进去的最新那一条消息的序号。快照里没有回顾的字的、她一个带正文的回复都没有的，没有。
pub(crate) fn request(history: &History, texts: &Texts) -> Option<(Request, Seq)> {
    let recap = texts.recap.as_ref()?;
    let turns = exchanges(history, texts, recap.turns)?;
    let upto = turns.iter().filter_map(|turn| turn.last).max()?;
    let limit = usize::try_from(recap.tokens.saturating_mul(BYTES_PER_TOKEN)).unwrap_or(usize::MAX);
    let room = limit.saturating_sub(recap.instruction.len());
    // 标签、空行本身就放不下的（上限定得太小），整份截到上限，照 codex 的 `RecapPrompt::new`。
    let mut transcript = transcript(&turns, recap, room);
    transcript.truncate(transcript.floor_char_boundary(room));
    let text = format!("{}{transcript}", recap.instruction);
    let request = Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::User {
            blocks: vec![Block::Text(Text { text })],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
    };
    Some((request, upto))
}

/// 一轮：人这边的话，她的回答（都可以是空的，不会都空），和这一轮里最新那一条的序号。
#[derive(Debug, Default)]
pub(crate) struct Exchange {
    pub(crate) user: String,
    pub(crate) assistant: String,
    pub(crate) last: Option<Seq>,
}

impl Exchange {
    /// 这一轮的几段：标签和原话，空的不写。
    fn fields<'a>(&'a self, recap: &'a Recap) -> impl Iterator<Item = (&'a str, &'a str)> {
        [
            (recap.user.as_str(), self.user.as_str()),
            (recap.assistant.as_str(), self.assistant.as_str()),
        ]
        .into_iter()
        .filter(|(_, text)| !text.is_empty())
    }
}

/// 从有效历史里照先后取最近的几轮（照 codex 的 `recent_exchanges`，从新往旧数）：挨着的几段人这边的话并成一段，挨着的几段
/// 回答也是；数到答过的 `turns` 轮为止，最新那一轮没答的也带上。最老那一轮只有回答、没有人这边的话的也留（codex 丢掉它：压缩
/// 以后留着的尾巴可以从她的回答开头）。她一个带正文的回复都没有的，没有。
fn exchanges(history: &History, texts: &Texts, turns: usize) -> Option<Vec<Exchange>> {
    let entries = entries(history, texts);
    if !entries.iter().any(|entry| entry.assistant) {
        return None;
    }
    let mut exchanges = Vec::new();
    let mut current = Exchange::default();
    let mut answered = 0;
    for entry in entries.iter().rev() {
        // 倒着看，人这边的话前面的回答属于前一轮。
        if entry.assistant && !current.user.is_empty() {
            answered += usize::from(!current.assistant.is_empty());
            exchanges.push(std::mem::take(&mut current));
            if answered == turns {
                break;
            }
        }
        let field = match entry.assistant {
            true => &mut current.assistant,
            false => &mut current.user,
        };
        let mut text = entry.text.clone();
        if !field.is_empty() {
            text.push_str(GAP);
            text.push_str(field);
        }
        *field = text;
        current.last = current.last.max(Some(entry.seq));
    }
    if !current.user.is_empty() || !current.assistant.is_empty() {
        exchanges.push(current);
    }
    exchanges.reverse();
    Some(exchanges)
}

/// 记录里的一段：哪一条、是不是她的回答、原话（去掉了前后空白，不是空的）。
pub(crate) struct Entry {
    pub(crate) seq: Seq,
    pub(crate) assistant: bool,
    pub(crate) text: String,
}

/// 照投影的先后（`History::ordered`）取人这边的话和她每一轮最后一条有正文的回复。人这边的话照主请求里的写法渲染：别的
/// harness、别的会话、子代理发来的包着外壳，派它的那一轮撤掉了的子代理的话不出（`render.rs` 的 `said`）。只要字，附件不要。
pub(crate) fn entries(history: &History, texts: &Texts) -> Vec<Entry> {
    let answers = last_answers(history);
    let mut entries = Vec::new();
    for event in history.ordered() {
        let (assistant, blocks) = match &event.body {
            Body::MessageUser(message) => (
                false,
                crate::render::spoken(history, event, message, message.blocks.clone(), texts),
            ),
            Body::MessageAssistant(reply)
                if event
                    .turn
                    .is_some_and(|turn| answers.get(&turn) == Some(&event.seq)) =>
            {
                (true, reply.blocks.clone())
            }
            _ => continue,
        };
        let text = written(&blocks);
        if !text.is_empty() {
            entries.push(Entry {
                seq: event.seq,
                assistant,
                text,
            });
        }
    }
    entries
}

/// 每一轮最后一条有正文的回复的序号，照回合编号。只有工具调用、思考的回复不算。
fn last_answers(history: &History) -> BTreeMap<TurnId, Seq> {
    let mut answers = BTreeMap::new();
    for event in history.events() {
        if let (Body::MessageAssistant(reply), Some(turn)) = (&event.body, event.turn)
            && !written(&reply.blocks).is_empty()
        {
            answers.insert(turn, event.seq);
        }
    }
    answers
}

/// 几块里的字连起来，去掉前后空白。
fn written(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// 对话记录，至多 `room` 字节（照 codex 的 `recap_history`）：放得下的照原样；放不下的先整轮去掉最老的，最前写一行省略了；
/// 还放不下，每一段留头尾、截掉中间。最新那一轮一定留，它没答的，前一轮（最新的回答）也留。
pub(crate) fn transcript(exchanges: &[Exchange], recap: &Recap, room: usize) -> String {
    let blocks: Vec<String> = exchanges
        .iter()
        .map(|exchange| {
            exchange
                .fields(recap)
                .map(|(label, text)| format!("{label}{text}"))
                .collect::<Vec<_>>()
                .join(GAP)
        })
        .collect();
    let mut bytes =
        blocks.iter().map(String::len).sum::<usize>() + GAP.len() * blocks.len().saturating_sub(1);
    if bytes <= room {
        return blocks.join(GAP);
    }
    let latest_answered = exchanges
        .last()
        .is_some_and(|exchange| !exchange.assistant.is_empty());
    let retained = if latest_answered { 1 } else { 2 };
    let oldest_retained = exchanges.len().saturating_sub(retained);
    let mut start = 0;
    while bytes > room.saturating_sub(recap.omitted.len()) && start < oldest_retained {
        bytes -= blocks[start].len() + GAP.len();
        start += 1;
    }
    let omission = if start > 0 {
        recap.omitted.as_str()
    } else {
        ""
    };
    let budget = room.saturating_sub(omission.len());
    if bytes <= budget {
        return format!("{omission}{}", blocks[start..].join(GAP));
    }
    let fields: Vec<(&str, &str)> = exchanges[start..]
        .iter()
        .flat_map(|exchange| exchange.fields(recap))
        .collect();
    let overhead = fields.iter().map(|(label, _)| label.len()).sum::<usize>()
        + GAP.len() * fields.len().saturating_sub(1);
    let mut remaining = budget.saturating_sub(overhead);
    let mut excerpts = Vec::with_capacity(fields.len());
    for (index, (label, text)) in fields.iter().enumerate() {
        let share = remaining / (fields.len() - index);
        // 给后面的几段各留一份，短的只留它自己那么长，不白占地方。
        let reserved = fields[index + 1..]
            .iter()
            .map(|(_, text)| text.len().min(share))
            .sum::<usize>();
        let excerpt = excerpt(text, remaining.saturating_sub(reserved), &recap.excerpted);
        remaining = remaining.saturating_sub(excerpt.len());
        excerpts.push(format!("{label}{excerpt}"));
    }
    format!("{omission}{}", excerpts.join(GAP))
}

/// 一段截到至多 `limit` 字节：放得下的照原样；放不下的留头尾各一半，中间夹 `marker`；连 `marker` 都放不下的，只留开头。
/// 只在一个字的边界上截。
fn excerpt(text: &str, limit: usize, marker: &str) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let Some(content) = limit.checked_sub(marker.len()) else {
        return text[..text.floor_char_boundary(limit)].to_string();
    };
    let head = text.floor_char_boundary(content / 2);
    let tail = text.ceil_char_boundary(text.len() - (content - content / 2));
    format!("{}{marker}{}", &text[..head], &text[tail..])
}

#[cfg(test)]
mod tests;
