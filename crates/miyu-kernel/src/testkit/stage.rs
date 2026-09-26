//! 执行器替身本身：一个会话，一份剧本，一块内存里的「磁盘」。照 `docs/designs/02-内核.md` 第四节
//! 「执行器怎么回动作」回每个动作；人的每个动作以后，一直跑到没事可做。

use std::collections::VecDeque;

use super::script::{Line, Play};
use crate::block::{Block, Text};
use crate::event::{
    Body, Decision, Event, Level, ModelCalled, Response, SessionCreated, Transient,
};
use crate::facts::Environment;
use crate::id::{CallId, CommandId, Seq, TurnId};
use crate::origin::By;
use crate::request::Request;
use crate::session::{
    Action, Answer, Command, Injection, Input, Outcome, Policy, Queued, Received, Session, Verdict,
};
use crate::time::Timestamp;

/// 造会话时的样子：alice 的会话，在本机，工作区的权限，只读关着。
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 执行器替身。
///
/// 追加的事件马上落盘；每送进一条输入，时钟往后走一秒。剧本里没排的，照默认的来：执行前的链
/// 放行，回合开始的挂接点不注入。剧本用完了还要请求模型、调工具，当场 panic：那是剧本写错了。
pub struct Stage {
    pub(super) session: Session,
    /// 造策略的办法：重启、崩了再载入时各造一份（策略里有组装器，不能复制）。
    pub(super) policy: Box<dyn Fn() -> Policy>,
    pub(super) environment: Environment,
    /// 「磁盘」：追加过的事件，照先后，都落了盘。
    pub(super) log: Vec<Event>,
    /// 交给驱动的每一次请求，照先后。
    pub(super) requests: Vec<Request>,
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
    pub(super) now: Timestamp,
    /// 下一个命令编号。
    pub(super) next: u64,
    /// 人的动作都是 alice 的。
    pub(super) by: By,
}

impl Stage {
    /// 在 `now` 这一刻，照 `policy` 造一个会话，在 `environment` 里。造会话的命令编号是 `cmd-0`。
    ///
    /// # Panics
    ///
    /// 造会话那一条的写法坏了才会：那是替身自己的 bug。
    pub fn new(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
    ) -> Stage {
        let by: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#)
            .unwrap_or_else(|e| panic!("alice 的写法坏了：{e}"));
        let created: SessionCreated =
            serde_json::from_str(CREATED).unwrap_or_else(|e| panic!("造会话的写法坏了：{e}"));
        let (session, actions) = Session::create(
            command_id(0),
            by.clone(),
            now,
            created,
            policy(),
            environment.clone(),
        );
        let mut stage = Stage {
            session,
            policy: Box::new(policy),
            environment,
            log: Vec::new(),
            requests: Vec::new(),
            replies: Vec::new(),
            transients: Vec::new(),
            ran: Vec::new(),
            lines: VecDeque::new(),
            plays: VecDeque::new(),
            verdicts: VecDeque::new(),
            injections: VecDeque::new(),
            held_model: None,
            held_tools: Vec::new(),
            now,
            next: 1,
            by,
        };
        stage.settle(actions);
        stage
    }

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
        self.command(Command::Revert { turn })
    }

    /// 恢复最近一次撤销。
    pub fn unrevert(&mut self) -> CommandId {
        self.command(Command::Unrevert)
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

    /// 有计划地重启：送进「要重启了」，再从「磁盘」载入。停住的请求、调用跟着没了。
    pub fn restart(&mut self) {
        let at = self.tick();
        self.run(Input::Restarting { at });
        self.reload();
    }

    /// 崩了：停住的请求、调用跟着没了，从「磁盘」载入。
    pub fn crash(&mut self) {
        self.reload();
    }

    /// 「磁盘」上的事件，照先后。
    pub fn log(&self) -> &[Event] {
        &self.log
    }

    /// 交给驱动的每一次请求，照先后。
    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    /// 推给头的瞬时事件，照先后。
    pub fn transients(&self) -> &[Transient] {
        &self.transients
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
    fn command(&mut self, command: Command) -> CommandId {
        let id = command_id(self.next);
        self.next += 1;
        let at = self.tick();
        self.run(Input::Command(Received {
            id: id.clone(),
            by: self.by.clone(),
            at,
            command,
        }));
        id
    }

    /// 送一条输入，跑到没事可做。
    fn run(&mut self, input: Input) {
        self.drain(VecDeque::from([input]));
    }

    /// 一条条送，回每个动作，直到没有要送的。
    fn drain(&mut self, mut inputs: VecDeque<Input>) {
        while let Some(input) = inputs.pop_front() {
            let actions = self.session.handle(input);
            for action in actions {
                inputs.extend(self.act(action));
            }
        }
    }

    /// 载入吐出来的、造会话吐出来的动作，也照样回。
    fn settle(&mut self, actions: Vec<Action>) {
        let mut inputs = VecDeque::new();
        for action in actions {
            inputs.extend(self.act(action));
        }
        self.drain(inputs);
    }

    /// 从「磁盘」载入一个新会话，回它吐出来的动作。
    fn reload(&mut self) {
        self.held_model = None;
        self.held_tools.clear();
        let at = self.tick();
        let (session, actions) = Session::load(
            self.log.clone(),
            at,
            (self.policy)(),
            self.environment.clone(),
        )
        .unwrap_or_else(|e| panic!("替身的「磁盘」载入不了：{e}"));
        self.session = session;
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
fn command_id(n: u64) -> CommandId {
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
