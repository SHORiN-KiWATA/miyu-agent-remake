//! 执行器替身本身：一个会话，一份剧本，一块内存里的「磁盘」。照 `docs/designs/02-内核.md` 第四节
//! 「执行器怎么回动作」回每个动作；人的每个动作以后，一直跑到没事可做。

use std::collections::VecDeque;

use super::script::{Line, Play};
use crate::block::{Block, Text};
use crate::event::{Body, Decision, Event, Level, ModelCalled, Response, Transient};
use crate::facts::Environment;
use crate::id::{CallId, CommandId, ContentHash, HarnessName, Seq, SessionId, TurnId};
use crate::origin::{By, Harness};
use crate::request::Request;
use crate::session::{
    Action, Answer, Command, Injection, Input, Limits, Outcome, Policy, Queued, Received, Session,
    Verdict,
};
use crate::time::Timestamp;

/// 执行器替身。
///
/// 追加的事件马上落盘；每送进一条输入，时钟往后走一秒。剧本里没排的，照默认的来：执行前的链
/// 放行，回合开始的挂接点不注入。剧本用完了还要请求模型、调工具，当场 panic：那是剧本写错了。
pub struct Stage {
    pub(super) session: Session,
    /// 会话的编号（[`super::SESSION`]、子会话是 [`super::CHILD_SESSION`]）：重启、崩了再载入时照它载入。
    pub(super) id: SessionId,
    /// 造策略的办法：重启、崩了再载入时各造一份（策略里有组装器，不能复制）。
    pub(super) policy: Box<dyn Fn() -> Policy>,
    pub(super) environment: Environment,
    /// 「磁盘」：追加过的事件，照先后，都落了盘。
    pub(super) log: Vec<Event>,
    /// 交给驱动的每一次请求，照先后：看到了第几条为止，和请求本身。
    pub(super) requests: Vec<(Seq, Request)>,
    /// 每一次请求交给驱动时，「磁盘」上的最后一条（施工 6-2 下）：摘要请求看到的比它早。
    pub(super) marks: Vec<Seq>,
    /// 每一次回应，照先后。
    pub(super) replies: Vec<(CommandId, Outcome)>,
    /// 推给头的瞬时事件。
    pub(super) transients: Vec<Transient>,
    /// 派去执行的每一次调用：调用编号、工具名、修正过的参数，照派的先后。
    pub(super) ran: Vec<(CallId, String, String)>,
    /// 剧本：模型接下来几次说什么、工具接下来几次怎么回、链接下来几次怎么判、挂接点接下来几次
    /// 交回什么。
    pub(super) lines: VecDeque<Line>,
    pub(super) plays: VecDeque<Play>,
    pub(super) verdicts: VecDeque<Verdict>,
    pub(super) injections: VecDeque<Vec<Injection>>,
    /// 停住的请求（它的 `seen` 和剩下的回复）、停住的调用。
    pub(super) held_model: Option<(Seq, Line)>,
    pub(super) held_tools: Vec<(CallId, Play)>,
    /// 叫它停过的调用，照先后（施工 4-9 再补一）。
    pub(super) stops: Vec<CallId>,
    /// 到点叫醒先扣着、不马上送回（[`Stage::hold_wakes`]），和扣着的那一个。
    pub(super) hold_wakes: bool,
    pub(super) held_wake: Option<(Timestamp, Seq)>,
    pub(super) now: Timestamp,
    /// 下一个命令编号。
    pub(super) next: u64,
    /// 人的动作都是 alice 的。
    pub(super) by: By,
    /// 交过的模型限额：重启、崩了再载入以后再交一次（施工 6-2 上）。
    pub(super) limits: Option<Limits>,
    /// 摘要请求怎么回：最后一块是这段摘要指令的请求，一律照这一句回，不占剧本（施工 6-2 上）。
    pub(super) summaries: Option<(String, Line)>,
    /// 「磁盘」上的文件、存过的 blob、交过来的重读（施工 6-5，`disk.rs`）。
    pub(super) disk: super::disk::Disk,
    /// 交出来的向上回报，照先后（施工 7-6）。
    pub(super) upward: Vec<crate::session::Upward>,
    /// 撤销交出来的停任务，照先后（施工 7-8）：替身记下来，回报由测试照停好了的样子交。
    pub(super) stopping: Vec<crate::session::Action>,
    /// 回顾的请求，照先后：照到第几条，和请求本身（施工 3-8 四补，`aside.rs`）。
    pub(super) recaps: Vec<(Seq, Request)>,
    /// 回顾的请求接下来几次怎么回；停住的那一次（它的 `upto` 和剩下的回复）。
    pub(super) recap_lines: VecDeque<Line>,
    pub(super) held_recap: Option<(Seq, Line)>,
    /// 起标题的请求，照先后（施工 3-8 五补）；接下来几次怎么回，没排的不回；停住的那一次。
    pub(super) titles: Vec<(Seq, Request)>,
    pub(super) title_lines: VecDeque<Line>,
    pub(super) held_title: Option<(Seq, Line)>,
    /// 路由：回合开始时交来的引用、解析不出的（施工 8-10，`routing.rs`）。
    pub(super) routing: super::routing::Routing,
    /// 替它看图：剧本、交出来的转述请求、扣着的（施工 8-17，`sight.rs`）。
    pub(super) sight: super::sight::Sight,
    /// 下一个回合开始时换上的策略和它的快照哈希（施工 P-1 再补，`swap.rs`）。
    pub(super) swap: Option<(ContentHash, Box<dyn Fn() -> Policy>)>,
}

impl Stage {
    /// 模型接下来几次请求，照先后这样回。
    pub fn model(&mut self, lines: impl IntoIterator<Item = Line>) {
        self.lines.extend(lines);
    }

    /// 接下来几次调工具，照派出去的先后这样回。
    pub fn tools(&mut self, plays: impl IntoIterator<Item = Play>) {
        self.plays.extend(plays);
    }

    /// 执行前的链接下来几次这样判；排完了照默认的放行。
    pub fn guards(&mut self, verdicts: impl IntoIterator<Item = Verdict>) {
        self.verdicts.extend(verdicts);
    }

    /// 回合开始的挂接点接下来几次交回这些注入；排完了不注入。
    pub fn hooks(&mut self, injections: impl IntoIterator<Item = Vec<Injection>>) {
        self.injections.extend(injections);
    }

    /// 说一句。返回这个命令的编号。
    pub fn say(&mut self, words: &str) -> CommandId {
        self.command(Command::Send {
            blocks: text(words),
            urgent: false,
        })
    }

    /// 发这几块内容，例如带图的（施工 6-3 上）。
    pub fn send(&mut self, blocks: Vec<Block>) -> CommandId {
        self.command(Command::Send {
            blocks,
            urgent: false,
        })
    }

    /// 别的 harness `name` 说一句（施工 7-10）：`by` 是 `harness`，一块字。返回这个命令的编号。
    ///
    /// # Panics
    ///
    /// `name` 不合短名字的写法。
    pub fn harness_says(&mut self, name: &str, words: &str) -> CommandId {
        let name = HarnessName::parse(name).unwrap_or_else(|e| panic!("名字的写法坏了：{e}"));
        let by = By::Harness(Harness { name });
        self.command_as(
            by,
            Command::Send {
                blocks: text(words),
                urgent: false,
            },
        )
    }

    /// 急着插话。
    pub fn say_urgently(&mut self, words: &str) -> CommandId {
        self.command(Command::Send {
            blocks: text(words),
            urgent: true,
        })
    }

    /// 打断，排着队的照 `queued` 办。
    pub fn interrupt(&mut self, queued: Queued) -> CommandId {
        self.command(Command::Interrupt { queued })
    }

    /// 回答调用 `call_id` 的确认。
    pub fn decide(
        &mut self,
        call_id: CallId,
        decision: Decision,
        reason: Option<&str>,
    ) -> CommandId {
        self.command(Command::Answer {
            call_id,
            answer: Answer::Approval {
                decision,
                reason: reason.map(str::to_string),
            },
        })
    }

    /// 回答调用 `call_id` 问的那组题。
    pub fn reply(&mut self, call_id: CallId, answers: Vec<Response>) -> CommandId {
        self.command(Command::Answer {
            call_id,
            answer: Answer::Questions(answers),
        })
    }

    /// 切权限级别：改哪样写哪样。
    pub fn set_permission(&mut self, level: Option<Level>, read_only: Option<bool>) -> CommandId {
        self.command(Command::SetPermission { level, read_only })
    }

    /// 从回合 `turn` 起撤销。
    pub fn revert(&mut self, turn: TurnId) -> CommandId {
        self.command(Command::Revert { turn: Some(turn) })
    }

    /// 恢复最近一次撤销。
    pub fn unrevert(&mut self) -> CommandId {
        self.command(Command::Unrevert)
    }

    /// 重做最后一轮（施工 4-7 再补）：`text` 是开这一轮的那一句里的字换成的块，`None` 是原样重发；附件照原来的。
    pub fn redo(&mut self, text: Option<Vec<Block>>) -> CommandId {
        self.redo_with(text, None)
    }

    /// 同上，附件换成 `attachments`（`None` 是照原来的，空的是不要附件）。
    pub fn redo_with(
        &mut self,
        text: Option<Vec<Block>>,
        attachments: Option<Vec<Block>>,
    ) -> CommandId {
        self.command(Command::Redo { text, attachments })
    }

    /// 工作目录换成 `cwd`。
    pub fn cd(&mut self, cwd: &str) {
        self.environment.cwd = cwd.to_string();
        self.run(Input::Environment(self.environment.clone()));
    }

    /// 时间往后拨 `minutes` 分钟。
    ///
    /// # Panics
    ///
    /// 拨出了时间的范围（公元 9999 年以后）。
    pub fn advance(&mut self, minutes: i64) {
        self.now = Timestamp::from_unix_millis(self.now.unix_millis() + minutes * 60_000)
            .unwrap_or_else(|| panic!("拨出了时间的范围"));
    }

    /// 放行停住的那次请求：送说完了。
    ///
    /// # Panics
    ///
    /// 没有停住的请求。
    pub fn release_model(&mut self) {
        let (seen, line) = self
            .held_model
            .take()
            .unwrap_or_else(|| panic!("没有停住的请求"));
        let ended = self.ended(seen, &line);
        self.run(ended);
    }

    /// 从现在起，到点叫醒先扣着，不马上送回：好在等着重试的时候插手（打断、重启）。
    pub fn hold_wakes(&mut self) {
        self.hold_wakes = true;
    }

    /// 之后的到点叫醒不再扣着，当场送回；已经扣着的那一个照旧等 [`Stage::release_wake`]（施工 4-9 再补三上）。
    pub fn unhold_wakes(&mut self) {
        self.hold_wakes = false;
    }

    /// 送回扣着的那一次到点了：时钟拨到那一刻。
    ///
    /// # Panics
    ///
    /// 没有扣着的到点叫醒。
    pub fn release_wake(&mut self) {
        let (at, seen) = self
            .held_wake
            .take()
            .unwrap_or_else(|| panic!("没有扣着的到点叫醒"));
        let inputs = self.wake(at, seen);
        self.drain(inputs.into());
    }

    /// 放行停住的调用 `call_id`：照它排好的回。
    ///
    /// # Panics
    ///
    /// 这个调用没有停住。
    pub fn release_tool(&mut self, call_id: CallId) {
        let k = self
            .held_tools
            .iter()
            .position(|(held, _)| *held == call_id)
            .unwrap_or_else(|| panic!("{call_id} 没有停住"));
        let (_, play) = self.held_tools.remove(k);
        let inputs = self.play(call_id, play);
        self.drain(inputs.into());
    }

    /// 停住的调用 `call_id` 看到旗，停在了改之前（施工 4-9 再补一）：交回 `stopped`。
    ///
    /// # Panics
    ///
    /// 这个调用没有停住。
    pub fn release_stopped(&mut self, call_id: CallId) {
        let k = self
            .held_tools
            .iter()
            .position(|(held, _)| *held == call_id)
            .unwrap_or_else(|| panic!("{call_id} 没有停住"));
        self.held_tools.remove(k);
        let input = self.stopped(call_id);
        self.drain([input].into());
    }

    /// 有计划地重启：送进「要重启了」，再从「磁盘」载入。停住的请求、调用跟着没了。
    pub fn restart(&mut self) {
        self.restarting();
        self.reload();
    }

    /// 只送进「要重启了」，不载入：执行器停下之前还要交后台命令的结束（施工 7-3）。
    pub fn restarting(&mut self) {
        let at = self.tick();
        self.run(Input::Restarting { at });
    }

    /// 内核照日志算的用过的最大任务编号（施工 7-3）：执行器从它往后数。
    pub fn last_job_number(&self) -> u64 {
        self.session.last_job_number()
    }

    /// 崩了：停住的请求、调用跟着没了，从「磁盘」载入。
    pub fn crash(&mut self) {
        self.reload();
    }

    /// 「磁盘」上的事件，照先后。
    pub fn log(&self) -> &[Event] {
        &self.log
    }

    /// 交给驱动的每一次请求，照先后：看到了第几条为止，和请求本身。
    pub fn requests(&self) -> &[(Seq, Request)] {
        &self.requests
    }

    /// 每一次请求交给驱动时「磁盘」上的最后一条，和 [`Stage::requests`] 一一对应。主请求就是它的 `seen`；压缩的摘要
    /// 请求看到的比它早（施工 6-2 下）。
    pub fn marks(&self) -> &[Seq] {
        &self.marks
    }

    /// 推给头的瞬时事件，照先后。
    pub fn transients(&self) -> &[Transient] {
        &self.transients
    }

    /// 叫它停过的调用，照先后（施工 4-9 再补一）。
    pub fn stops(&self) -> &[CallId] {
        &self.stops
    }

    /// 派去执行的每一次调用：调用编号、工具名、修正过的参数，照派的先后。
    pub fn ran(&self) -> &[(CallId, String, String)] {
        &self.ran
    }

    /// 命令 `id` 最近一次的回应。
    pub fn outcome(&self, id: &CommandId) -> Option<&Outcome> {
        self.replies
            .iter()
            .rev()
            .find(|(replied, _)| replied == id)
            .map(|(_, outcome)| outcome)
    }

    /// 开过的回合，照先后。
    pub fn turns(&self) -> Vec<TurnId> {
        self.log
            .iter()
            .filter(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| TurnId::new(event.seq))
            .collect()
    }

    /// 每次请求记下的 `model.called`，照先后。
    pub fn model_calls(&self) -> Vec<&ModelCalled> {
        self.log
            .iter()
            .filter_map(|event| match &event.body {
                Body::ModelCalled(called) => Some(called),
                _ => None,
            })
            .collect()
    }

    /// 送一个命令，跑到没事可做。
    pub(super) fn command(&mut self, command: Command) -> CommandId {
        self.command_as(self.by.clone(), command)
    }

    /// 以 `by` 的名义送一个命令，跑到没事可做（施工 7-2：子会话交回报）。
    pub(super) fn command_as(&mut self, by: By, command: Command) -> CommandId {
        let id = command_id(self.next);
        self.next += 1;
        let at = self.tick();
        self.run(Input::Command(Received {
            id: id.clone(),
            by,
            at,
            command,
        }));
        id
    }

    /// 送一条输入，跑到没事可做。
    pub(super) fn run(&mut self, input: Input) {
        self.drain(VecDeque::from([input]));
    }

    /// 一条条送，回每个动作，直到没有要送的。
    pub(super) fn drain(&mut self, mut inputs: VecDeque<Input>) {
        while let Some(input) = inputs.pop_front() {
            let actions = self.session.handle(input);
            for action in actions {
                inputs.extend(self.act(action));
            }
        }
    }

    /// 载入吐出来的、造会话吐出来的动作，也照样回。
    pub(super) fn settle(&mut self, actions: Vec<Action>) {
        let mut inputs = VecDeque::new();
        for action in actions {
            inputs.extend(self.act(action));
        }
        self.drain(inputs);
    }

    /// 从「磁盘」载入一个新会话，回它吐出来的动作。
    pub(super) fn reload(&mut self) {
        self.held_model = None;
        self.held_tools.clear();
        self.held_wake = None;
        let at = self.tick();
        let (session, actions) = Session::load(
            self.id.clone(),
            self.log.clone(),
            at,
            (self.policy)(),
            self.environment.clone(),
        )
        .unwrap_or_else(|e| panic!("替身的「磁盘」载入不了：{e}"));
        self.session = session;
        if let Some(limits) = self.limits.clone() {
            self.session.handle(Input::Limits(limits));
        }
        self.settle(actions);
    }

    /// 送一条输入用的时刻：往后走一秒。
    pub(super) fn tick(&mut self) -> Timestamp {
        self.now = Timestamp::from_unix_millis(self.now.unix_millis() + 1000)
            .unwrap_or_else(|| panic!("走出了时间的范围"));
        self.now
    }
}

/// 编号是 `n` 的命令：`cmd-<n>`。
pub(super) fn command_id(n: u64) -> CommandId {
    CommandId::parse(&format!("cmd-{n}")).unwrap_or_else(|e| panic!("命令编号的写法坏了：{e}"))
}

/// 一段话写成的内容块；空的就一块都没有。
fn text(words: &str) -> Vec<Block> {
    match words.is_empty() {
        true => Vec::new(),
        false => vec![Block::Text(Text {
            text: words.to_string(),
        })],
    }
}
