//! 后台任务的几种事件和别处来的话（蓝图 `tui.md`「后台命令、子代理和侧边栏」「别处来的话」，`kernel/events-bodies.md`）：
//! 工具结果 `effects` 里的 `job.started`、`job.messaged`，`job.reported`、`child.reported`，不是这个界面发的
//! `message.user`。

use serde_json::Value;

use super::Push;

/// 一句不是这个界面发的话（`message.user`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// 它的序号。
    pub seq: u64,
    /// 谁发的。
    pub from: Sender,
    /// 说的字（文字那几块连起来）。
    pub text: String,
}

/// 谁发的（`by`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sender {
    /// 人：同一个人在别的地方（网页、命令行）说的。
    Person,
    /// 别的 harness 报的名字（`miyu ask --from`，施工 7-10）。名字不可信。
    Harness(String),
    /// 另一个会话：子代理给主会话的留言，或者子会话里看到的主会话的交代、留言（施工 7-7）。
    Session(String),
    /// 别的（新版本才有的）。
    Other,
}

/// 派出去一个后台任务（`job.started`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStart {
    /// 哪一步派的：照它的参数写命令本身、交代的活。
    pub call_id: String,
    /// 任务编号（`j2`、`j2.1`）。
    pub job: String,
    /// 是子代理（`what` 是 `agent`）；别的当后台命令。
    pub agent: bool,
    /// 调用时写的 `description`。
    pub title: String,
    /// 子代理的会话。
    pub session: Option<String>,
}

/// 为什么结束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobReason {
    /// 命令自己退出了（被信号杀掉的也算）；子代理那一轮做完了（`done`）。
    Finished,
    /// 被停掉了。
    Stopped,
    /// 撤销派它的那一轮时停掉的。
    Undone,
    /// 核心重启、崩了停掉的（`restarted`、`aborted`）。
    Restarted,
}

impl JobReason {
    fn parse(text: &str) -> Self {
        match text {
            "stopped" => Self::Stopped,
            "undone" => Self::Undone,
            "restarted" | "aborted" => Self::Restarted,
            _ => Self::Finished,
        }
    }
}

/// 一个后台任务结束了（`job.reported`、`child.reported`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEnd {
    /// 任务编号。
    pub job: String,
    /// 为什么结束。
    pub reason: JobReason,
    /// 命令的退出码。
    pub exit_code: Option<i32>,
    /// 杀掉命令的信号。
    pub signal: Option<u32>,
    /// 跑了多久（毫秒）。
    pub duration_ms: Option<u64>,
    /// 子代理交回来的回答；命令没有。
    pub text: String,
}

/// 工具结果里的 `effects`：派出去的、留了言的。
pub(super) fn effects(body: &Value, out: &mut Vec<Push>) {
    let call_id = body["call_id"].as_str().unwrap_or_default();
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    for effect in body["effects"].as_array().into_iter().flatten() {
        match effect["kind"].as_str() {
            Some("job.started") => out.push(Push::JobStarted(JobStart {
                call_id: call_id.to_string(),
                job: text(&effect["job"]),
                agent: effect["what"] == "agent",
                title: text(&effect["title"]),
                session: effect["session"].as_str().map(str::to_string),
            })),
            Some("job.messaged") => out.push(Push::JobMessaged(text(&effect["job"]))),
            _ => {}
        }
    }
}

/// `job.reported`、`child.reported`。
pub(super) fn reported(body: &Value) -> Push {
    let int = |k: &str| body[k].as_i64();
    Push::JobEnded(JobEnd {
        job: body["job"].as_str().unwrap_or_default().to_string(),
        reason: JobReason::parse(body["reason"].as_str().unwrap_or_default()),
        exit_code: int("exit_code").and_then(|c| i32::try_from(c).ok()),
        signal: int("signal").and_then(|c| u32::try_from(c).ok()),
        duration_ms: body["duration_ms"].as_u64(),
        text: body["text"].as_str().unwrap_or_default().to_string(),
    })
}

/// `message.user`：这个界面发的交回序号，别处来的交回谁发的、说了什么。`mine` 照 `cause` 认。
pub(super) fn said(event: &Value, mine: &dyn Fn(&str) -> bool) -> Option<Push> {
    let seq = event["seq"].as_u64()?;
    let by = &event["by"];
    if by["kind"] == "person" && event["cause"].as_str().is_some_and(mine) {
        return Some(Push::UserMessage(seq));
    }
    let from = match by["kind"].as_str() {
        Some("person") => Sender::Person,
        Some("harness") => Sender::Harness(by["name"].as_str().unwrap_or_default().to_string()),
        Some("session") => Sender::Session(by["id"].as_str().unwrap_or_default().to_string()),
        _ => Sender::Other,
    };
    let text: Vec<&str> = event["body"]["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect();
    Some(Push::Foreign(Said {
        seq,
        from,
        text: text.join("\n"),
    }))
}
