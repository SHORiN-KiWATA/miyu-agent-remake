//! 执行器（`docs/construction/3-5-试玩台（补）.md`）：照 `02-内核.md` 第四节的表回内核的每个动作。
//!
//! 会话在这一个任务里，输入排成一队、一条一条送进去；HTTP 和到点叫醒在别的任务里，回报送回这一队。
//! 追加的先写进会话日志、同步了，再送「落盘了」：S4，先落盘，后推送。人这一边是一行一句：一轮在走
//! 的时候来的字先攒着，这一轮走完了再说；Ctrl+C 打断这一轮。

use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Level, Permission, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, ContentHash, Seq, SessionId, VenueId};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::session::{Action, Command, Input, Outcome, Queued, Received, Session, Verdict};
use miyu_kernel::time::Timestamp;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use tokio::sync::{mpsc, oneshot};

use crate::call::{Caller, Cut, report};
use crate::show::Show;
use crate::{BenchError, now, policy};

/// 人那一边来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// 一行字。`/cut` 开头的是故意掐断，`/quit` 是退出。
    Line(String),
    /// Ctrl+C：有一轮在走的，打断它；空闲的，退出。
    Interrupt,
    /// 输入完了：这一轮走完了就退出。
    End,
}

/// 会话和它的执行器。
pub(crate) struct Executor<W: Write> {
    session: Session,
    log: SessionLog,
    /// 会话的目录：日志在里面。
    dir: PathBuf,
    show: Show<W>,
    caller: Caller,
    /// HTTP 和到点叫醒的回报送到这里。
    inputs: mpsc::UnboundedSender<Input>,
    /// 在路上的请求，叫停用的那一头。
    stops: BTreeMap<Seq, oneshot::Sender<()>>,
    /// 下一次请求要故意掐断的。
    cut: Option<Cut>,
    /// 这一轮是哪个命令开的；空闲时没有。
    busy: Option<CommandId>,
    person: By,
    commands: u64,
}

/// 试玩台的会话属主和场所（施工单「我定的」）。
const LOCAL: &str = "local";

impl<W: Write> Executor<W> {
    /// 开一个新会话：`sessions` 照属主和会话编号给出会话的目录（数据根的 `session_dir`），在那里
    /// 造好会话日志，第一条 `session.created` 落了盘。
    pub(crate) fn open(
        sessions: impl FnOnce(&AccountId, &SessionId) -> PathBuf,
        system: &str,
        environment: Environment,
        caller: Caller,
        show: Show<W>,
        inputs: mpsc::UnboundedSender<Input>,
    ) -> Result<Executor<W>, BenchError> {
        let account = AccountId::parse(LOCAL).map_err(BenchError::name)?;
        let id = SessionId::parse(&crate::session_id()?).map_err(BenchError::name)?;
        let dir = sessions(&account, &id);
        let log = SessionLog::create(&dir, SEGMENT_LIMIT)?;
        let created = SessionCreated {
            owner: account.clone(),
            venue: VenueId::parse(LOCAL).map_err(BenchError::name)?,
            policy: ContentHash::of(system.as_bytes()),
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
        };
        let person = By::Person(Person { account });
        let first = CommandId::parse("try-0").map_err(BenchError::name)?;
        let (session, actions) = Session::create(
            first,
            person.clone(),
            now(),
            created,
            policy::policy(system),
            environment,
        );
        let mut executor = Executor {
            session,
            log,
            dir,
            show,
            caller,
            inputs,
            stops: BTreeMap::new(),
            cut: None,
            busy: None,
            person,
            commands: 0,
        };
        executor.act_all(actions)?;
        Ok(executor)
    }

    /// 会话的目录。
    pub(crate) fn dir(&self) -> &PathBuf {
        &self.dir
    }

    /// 写终端的那一头：开头、结尾的说明也从这里写。
    pub(crate) fn show(&mut self) -> &mut Show<W> {
        &mut self.show
    }

    /// 空闲：没有一轮在走。
    pub(crate) fn idle(&self) -> bool {
        self.busy.is_none()
    }

    /// 人说了一行：`/cut` 的记下，下一次请求掐断；别的发给会话，开一轮。
    pub(crate) fn say(&mut self, line: &str) -> Result<(), BenchError> {
        match Cut::parse(line) {
            Some(Ok(cut)) => {
                self.cut = Some(cut);
                return Ok(self
                    .show
                    .line(&format!("（下一次请求{}）", cut.describe()))?);
            }
            Some(Err(usage)) => return Ok(self.show.line(&format!("（写法：{usage}）"))?),
            None => {}
        }
        let id = self.command()?;
        self.busy = Some(id.clone());
        let command = Command::Send {
            blocks: vec![Block::Text(Text {
                text: line.to_string(),
            })],
            urgent: false,
        };
        self.receive(id, command)
    }

    /// Ctrl+C：打断这一轮，排着的退回来（试玩台一轮在走的时候不往会话里发字，其实没有排着的）。
    pub(crate) fn interrupt(&mut self) -> Result<(), BenchError> {
        let id = self.command()?;
        self.receive(
            id,
            Command::Interrupt {
                queued: Queued::Return,
            },
        )
    }

    /// 下一个命令编号。
    fn command(&mut self) -> Result<CommandId, BenchError> {
        self.commands += 1;
        CommandId::parse(&format!("try-{}", self.commands)).map_err(BenchError::name)
    }

    /// 送进一个命令。
    fn receive(&mut self, id: CommandId, command: Command) -> Result<(), BenchError> {
        let received = Received {
            id,
            by: self.person.clone(),
            at: now(),
            command,
        };
        self.feed(Input::Command(received))
    }

    /// 送进一条输入，连同它引出来的：动作一个个回，回出来的输入接着送，送到没有为止。
    pub(crate) fn feed(&mut self, input: Input) -> Result<(), BenchError> {
        let mut queue = VecDeque::from([input]);
        while let Some(input) = queue.pop_front() {
            if let Input::ModelEnded { seen, .. } = &input {
                self.stops.remove(seen);
            }
            for action in self.session.handle(input) {
                queue.extend(self.act(action)?);
            }
        }
        Ok(())
    }

    /// 回一串动作，回出来的输入接着送。
    fn act_all(&mut self, actions: Vec<Action>) -> Result<(), BenchError> {
        for action in actions {
            for input in self.act(action)? {
                self.feed(input)?;
            }
        }
        Ok(())
    }

    /// 照 02 第四节的表回一个动作，交回马上要送回去的输入；要等的（HTTP、到点叫醒）由别的任务送。
    fn act(&mut self, action: Action) -> Result<Vec<Input>, BenchError> {
        match action {
            Action::Append(events) => {
                self.log.append(&events)?;
                Ok(events
                    .last()
                    .map(|event| Input::Stored { upto: event.seq })
                    .into_iter()
                    .collect())
            }
            Action::Reply { id, outcome } => {
                if let Outcome::Rejected { reason } = outcome {
                    self.show.line(&format!("（没收下：{}）", reason.code()))?;
                    if self.busy.as_ref() == Some(&id) {
                        self.busy = None;
                    }
                }
                Ok(Vec::new())
            }
            Action::Push(events) => {
                for event in &events {
                    self.show.event(event)?;
                    if matches!(event.body, Body::TurnEnded(_)) {
                        self.busy = None;
                    }
                }
                Ok(Vec::new())
            }
            Action::PushTransient(transient) => {
                self.show.transient(&transient)?;
                Ok(Vec::new())
            }
            Action::RunTurnStartHooks { turn } => Ok(vec![Input::TurnStartHooksDone {
                at: now(),
                turn,
                injected: Vec::new(),
            }]),
            Action::CallModel { seen, request } => {
                let cut = self.cut.take();
                match self.caller.call(seen, &request, cut, self.inputs.clone()) {
                    Ok(stop) => {
                        self.stops.insert(seen, stop);
                        Ok(Vec::new())
                    }
                    Err(error) => Ok(vec![Input::ModelEnded {
                        at: now(),
                        seen,
                        usage: None,
                        error: Some(error),
                        wait_ms: None,
                    }]),
                }
            }
            Action::CancelModel { seen } => {
                if let Some(stop) = self.stops.remove(&seen) {
                    // 请求已经收场了的，送不到，不要紧。
                    stop.send(()).unwrap_or(());
                }
                Ok(Vec::new())
            }
            Action::Wake { at, seen } => {
                self.wake(at, seen);
                Ok(Vec::new())
            }
            // 工具面是空的，模型编出来的工具名由内核自己回「没有这件工具」，这几样实际不会来。来了的，
            // 放行以后照执行出错回。
            Action::GuardTool { call_id, .. } => Ok(vec![Input::ToolGuarded {
                at: now(),
                call_id,
                verdict: Verdict::Allow,
            }]),
            Action::RunTool { call_id, .. } => Ok(vec![Input::ToolDone {
                at: now(),
                call_id,
                error: true,
                blocks: vec![Block::Text(Text {
                    text: "The trial bench has no tools.".to_string(),
                })],
                duration_ms: None,
            }]),
            Action::RunTurnEndHooks { .. }
            | Action::AnswerTool { .. }
            | Action::CancelTool { .. } => Ok(Vec::new()),
        }
    }

    /// 到点叫醒：另一个任务睡够了，送回「到点了」。
    fn wake(&self, at: Timestamp, seen: Seq) {
        let wait = u64::try_from(at.unix_millis() - now().unix_millis()).unwrap_or(0);
        let inputs = self.inputs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(wait)).await;
            report(&inputs, Input::Woke { at: now(), seen });
        });
    }
}
