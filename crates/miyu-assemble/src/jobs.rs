//! 两种回报渲染成一块带标签的事实（施工 7-2，`docs/blueprint/kernel/request.md`「回报」，`agents.md` 第九条）；子代理发来的
//! 留言也包一层标签，注明是哪个子代理（施工 7-7）。
//!
//! 标签带任务编号、标题、结束的原因，里面一行一句。标题照有效历史记着的派出去过的任务取（[`History::dispatched`]）：
//! 派它的那一条压缩掉了也在。派它的那一轮撤掉了的不渲染：派它的调用已经不在上下文里了（`agents.md` 第七条第 2 条）。
//!
//! 后台命令结束只写结束了、退出码或者信号、用时、输出有多少字和怎么看，不带输出本身；子代理的回报带正文，原样放、不
//! 转义：和检查点里的摘要一样，是模型写的多行正文。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{ChildReported, JobReported};
use miyu_kernel::history::{Dispatched, History};
use miyu_kernel::id::JobId;
use miyu_kernel::origin::By;
use miyu_kernel::template::Template;

use crate::texts::JobTexts;

/// 后台命令结束的那一块；派它的那一轮撤掉了、没派过的，没有。
pub(crate) fn command(
    history: &History,
    reported: &JobReported,
    texts: &JobTexts,
) -> Option<String> {
    let dispatched = shown(history, reported.job)?;
    let mut block = open(
        &texts.command_open,
        reported.job,
        dispatched,
        reported.reason.as_str(),
    );
    // 输出没存下来的，字数也不写：她读不到。
    let chars = reported.chars.filter(|_| reported.output.is_some());
    let lines = [
        (
            &texts.command_exit,
            "code",
            reported.exit_code.map(|n| n.to_string()),
        ),
        (
            &texts.command_signal,
            "signal",
            reported.signal.map(|n| n.to_string()),
        ),
        (
            &texts.command_duration,
            "ms",
            reported.duration_ms.map(|n| n.to_string()),
        ),
        (&texts.command_output, "chars", chars.map(|n| n.to_string())),
    ];
    for (template, field, value) in lines {
        if let Some(value) = value {
            block.push_str(&fill(template, &[(field, &value)]));
        }
    }
    block.push_str(&texts.command_close);
    Some(block)
}

/// 子代理的回报那一块：人插过话的、截过的各注明一句，接着是正文；一个字都没说的，写它没说话就结束了。派它的那一轮撤掉
/// 了、没派过的，没有。
pub(crate) fn subagent(
    history: &History,
    reported: &ChildReported,
    texts: &JobTexts,
) -> Option<String> {
    let dispatched = shown(history, reported.job)?;
    let mut block = open(
        &texts.subagent_open,
        reported.job,
        dispatched,
        reported.reason.as_str(),
    );
    if reported.person {
        block.push_str(&texts.subagent_person);
    }
    if reported.truncated {
        block.push_str(&texts.subagent_truncated);
    }
    match reported.text.is_empty() {
        true => block.push_str(&texts.subagent_silent),
        false => {
            block.push_str(&reported.text);
            if !reported.text.ends_with('\n') {
                block.push('\n');
            }
        }
    }
    block.push_str(&texts.subagent_close);
    Some(block)
}

/// 人这边的一条消息的块（施工 7-7，`docs/blueprint/kernel/request.md`「子代理的留言」）：发消息的是这个会话派的子代理
/// （`by` 是它的子会话）的，包一层标签，注明是哪个子代理（编号、标题），它的话原样放、不转义，末尾没有换行的补一个；
/// 字以外的块接在后面。派它的那一轮撤掉了的，一块都不出：派它的调用已经不在上下文里了，和它的回报一样。别人发的（人、
/// 父会话发给子代理的交代和留言）原样交回。以前造的快照没有标签，标签是空的，只剩它的话。
pub(crate) fn message(
    history: &History,
    by: &By,
    blocks: Vec<Block>,
    texts: Option<&JobTexts>,
) -> Vec<Block> {
    let (By::Session(session), Some(texts)) = (by, texts) else {
        return blocks;
    };
    let Some((job, dispatched)) = history.subagent(&session.id) else {
        return blocks;
    };
    if dispatched.undone {
        return Vec::new();
    }
    let job = job.to_string();
    let open = &texts.subagent_message_open;
    let mut text = fill(open, &[("job", &job), ("title", &dispatched.title)]);
    let mut rest = Vec::new();
    for block in blocks {
        match block {
            Block::Text(said) => text.push_str(&said.text),
            other => rest.push(other),
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&texts.subagent_message_close);
    let mut tagged = vec![Block::Text(Text { text })];
    tagged.extend(rest);
    tagged
}

/// 派出去过、派它的那一轮还在的任务。
fn shown(history: &History, job: JobId) -> Option<&Dispatched> {
    history
        .dispatched(job)
        .filter(|dispatched| !dispatched.undone)
}

/// 标签那一行：编号、标题、原因，字段照模板的规矩转义。
fn open(template: &Template, job: JobId, dispatched: &Dispatched, reason: &str) -> String {
    let job = job.to_string();
    fill(
        template,
        &[
            ("job", &job),
            ("title", &dispatched.title),
            ("reason", reason),
        ],
    )
}

/// 换字段。造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
fn fill(template: &Template, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    template.render(&fields).unwrap_or_default()
}

#[cfg(test)]
mod tests;
