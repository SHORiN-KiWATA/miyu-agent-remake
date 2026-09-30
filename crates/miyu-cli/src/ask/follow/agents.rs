//! 还有几个子代理没报（施工 7-9，`docs/blueprint/cli/ask.md`「等子代理」）：头照事件流自己数，协议不另给。
//!
//! - 跟着的那几轮里，工具结果的效果 `job.started`（`what` 是 `agent`）记下一个派出去的子代理，欠一份回报；
//! - `child.reported` 到了就不欠了，哪一种原因都算（停了、崩了的也算报过）；
//! - 效果 `job.messaged`（她给它留了言，施工 7-7）又欠一份，除非留言那次调用发出以后已经报过：它手快，做完先报了，照内核的
//!   账本算回了这句留言（`kernel/history.md`）。
//! - 这一次以前派、这一次留了言的也等（施工 7-9 补，M7 验收自测撞见）：没见过派它的那一条，没有标题。还没留言时到的回报
//!   不认，只记着序号，留言时照它算留言以后报过没有。
//!
//! 还要知道叫醒的那一轮会不会来：回报和它叫醒的那一轮在内核里是同一批（`kernel/session.md`「回报」第 5 条），头先看到回报，
//! 再看到 `turn.started`。闲着时到的、会叫醒她的，等那一轮开头；一轮里到的、这一轮最后一次请求没看到的（序号比请求的
//! `seen` 大），这一轮不是打断结束的，内核同一批接着开下一轮（「排队的消息」第 2 条），也等。这次 `miyu ask` 以前派出去、这一次
//! 没留言的不数。

use std::collections::BTreeMap;

use serde_json::Value;

use miyu_kernel::id::CallId;

/// 这次 `miyu ask` 等的子代理：这一次派出去的，和这一次留过言的（施工 7-9 补）。
#[derive(Debug, Default)]
pub(super) struct Agents {
    /// 照任务编号：等的子代理的标题，欠不欠一份回报，最近一次回报的序号。
    jobs: BTreeMap<String, Agent>,
    /// 还不等的子代理（这一次以前派、还没留言的）最近一次回报的序号：给它留言时照它算留言以后报过没有（施工 7-9 补）。
    heard: BTreeMap<String, u64>,
    /// 这一轮里到的、会叫醒她的回报，最后那一条的序号。
    arrived: Option<u64>,
    /// 这一轮的请求看到了第几条为止：`model.called` 的 `seen`，取最大的。
    seen: u64,
    /// 回报叫醒了她，那一轮还没开头：下一条就是它的 `turn.started`。
    expecting: bool,
}

/// 等的一个子代理。
#[derive(Debug)]
struct Agent {
    /// 标题：派它时写的 `description`。这一次以前派、这一次留了言的，没见过派它的那一条，没有（施工 7-9 补）。
    title: Option<String>,
    /// 欠着一份回报。
    owed: bool,
    /// 最近一次回报的序号：一次都没报过的没有。
    reported: Option<u64>,
}

impl Agents {
    /// 一次工具结果（`tool.result` 的 `body`）：派出去的子代理记下，留了言的又欠一份回报；还没记下的（这一次以前派的）也记下，
    /// 没有标题（施工 7-9 补）。
    pub(super) fn result(&mut self, body: &Value) {
        // 留言那次调用发出去的时刻：它所在的那条回复的序号（`call_<序号>_<第几个>`，`kernel/ids.md`）。读不出来的当刚发。
        let issued = body["call_id"]
            .as_str()
            .and_then(|call| CallId::parse(call).ok())
            .map(|call| call.message().get());
        for effect in body["effects"].as_array().into_iter().flatten() {
            let Some(job) = effect["job"].as_str() else {
                continue;
            };
            match effect["kind"].as_str() {
                Some("job.started") if effect["what"] == "agent" => {
                    let title = effect["title"].as_str().unwrap_or_default().to_string();
                    let agent = Agent {
                        title: Some(title),
                        owed: true,
                        reported: None,
                    };
                    self.jobs.insert(job.to_string(), agent);
                }
                Some("job.messaged") => {
                    let heard = &mut self.heard;
                    let agent = self.jobs.entry(job.to_string()).or_insert_with(|| Agent {
                        title: None,
                        owed: true,
                        reported: heard.remove(job),
                    });
                    agent.owed = match (agent.reported, issued) {
                        (Some(reported), Some(issued)) => reported < issued,
                        _ => true,
                    };
                }
                _ => {}
            }
        }
    }

    /// 一条回报（`child.reported` 整条事件）到了。`running` 是这时有没有跟着的回合在进行。交回它的编号和标题（没见过派它的
    /// 那一条的没有标题）；还不等它的（这一次以前派、还没留言的）不认，交回没有，只记着序号（施工 7-9 补）。
    pub(super) fn reported(
        &mut self,
        event: &Value,
        running: bool,
    ) -> Option<(String, Option<String>)> {
        let body = &event["body"];
        let job = body["job"].as_str()?;
        let seq = event["seq"].as_u64();
        let Some(agent) = self.jobs.get_mut(job) else {
            if let Some(seq) = seq {
                self.heard.insert(job.to_string(), seq);
            }
            return None;
        };
        agent.owed = false;
        agent.reported = seq;
        let title = agent.title.clone();
        if wakes(body) {
            match running {
                true => self.arrived = self.arrived.max(seq),
                false => self.expecting = true,
            }
        }
        Some((job.to_string(), title))
    }

    /// 一次请求的记录（`model.called` 的 `body`）：它看到了第几条为止。
    pub(super) fn called(&mut self, body: &Value) {
        self.seen = self.seen.max(body["seen"].as_u64().unwrap_or(0));
    }

    /// 开了一轮：等的那一轮来了；这一轮的请求从头数。
    pub(super) fn turn_started(&mut self) {
        self.expecting = false;
        self.arrived = None;
        self.seen = 0;
    }

    /// 一轮结束了，原因是 `reason`：这一轮里到的回报，最后一次请求没看到的，内核接着开下一轮；打断结束的不接着开。
    pub(super) fn turn_ended(&mut self, reason: &str) {
        if reason != "interrupted" && self.arrived.is_some_and(|arrived| arrived > self.seen) {
            self.expecting = true;
        }
        self.arrived = None;
        self.seen = 0;
    }

    /// 还有几个没报。
    pub(super) fn owed(&self) -> usize {
        self.jobs.values().filter(|agent| agent.owed).count()
    }

    /// 等的那一行写几个：还有没报的，也没有回报叫醒的一轮要来（要来的那一轮开了头，这一行马上又要擦掉）；不用印的没有。
    pub(super) fn shown(&self) -> Option<usize> {
        let owed = self.owed();
        (owed > 0 && !self.expecting).then_some(owed)
    }

    /// 都了结了：没有欠着回报的，也没有要来的一轮。
    pub(super) fn settled(&self) -> bool {
        self.owed() == 0 && !self.expecting
    }
}

/// 这条回报会不会叫醒她（`kernel/session.md`「回报」第 5 条）：撤销停掉的、崩了补报的、她自己停掉的（`by_model`）只记下；
/// 别的叫醒，不认识的原因也叫醒。
fn wakes(body: &Value) -> bool {
    match body["reason"].as_str() {
        Some("undone" | "aborted") => false,
        Some("stopped") => body["by_model"] != true,
        _ => true,
    }
}

#[cfg(test)]
mod tests;
