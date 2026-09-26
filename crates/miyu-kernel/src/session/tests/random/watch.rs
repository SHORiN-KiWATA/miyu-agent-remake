//! 随机测试的看守：一路看着会话吐出来的动作，边看边查；也替执行器记着哪些请求、哪些调用
//! 在路上，好送下一条像样的回报。

use std::collections::{BTreeMap, BTreeSet};

use super::*;

mod approval;
mod lookup;
mod permission;
mod question;
mod queue;

/// 看守。
pub(super) struct Watch {
    seed: u64,
    /// 追加过的事件，照先后。
    events: Vec<Event>,
    pushed: BTreeSet<Seq>,
    /// 每个编号收到了几次、回应了几次。
    pub(super) received: BTreeMap<CommandId, usize>,
    pub(super) replied: BTreeMap<CommandId, usize>,
    /// 叫跑过挂接点的回合；送回过对得上的结果的回合；跑过结束挂接点的回合。
    pub(super) hooked: BTreeSet<TurnId>,
    done: BTreeSet<TurnId>,
    end_hooked: BTreeSet<TurnId>,
    /// 每个回合请求过几次模型。
    requests: BTreeMap<TurnId, u32>,
    /// 交给执行器的请求；报过「发出去了」的；记了 `model.called` 的；在路上的那次。
    issued: BTreeSet<Seq>,
    sent: BTreeSet<Seq>,
    recorded: BTreeSet<Seq>,
    pub(super) asking: Option<Seq>,
    /// 交给了链的调用；在跑的调用；有了结果的调用；在跑时被打断、补了「已取消」的调用。
    admitted: BTreeSet<CallId>,
    pub(super) running: BTreeSet<CallId>,
    resulted: BTreeSet<CallId>,
    stopped: BTreeSet<CallId>,
    /// 现在的工作目录，和每个回合开始时的那一个。
    pub(super) cwd: String,
    turn_cwd: BTreeMap<TurnId, String>,
    /// 走到过哪些路：随机的输入要真走到这些地方，查的才不是空话。
    pub(super) seen_paths: BTreeSet<&'static str>,
    /// 执行器替身：在路上的那次请求，下一块从第几块开始、开着的那一块（第几块、是不是工具
    /// 调用、参数写了没有）。
    pub(super) next_block: usize,
    pub(super) open_block: Option<(usize, bool, bool)>,
    /// 风平浪静：打断、乱来的增量少，一轮才走得深。
    pub(super) calm: bool,
    /// 排着队的消息：这一轮里来的，还没被请求看到过。
    queued: Vec<Seq>,
    /// 正在送进去的那次新的打断，排着队的怎么办。
    interrupting: Option<Queued>,
    /// 现在的权限，和看守照规矩推出来的实际生效的那一级。
    permission: Permission,
    effective: Permission,
    /// 确认：交给链的、链的结论、在等人的、人允许了的。
    pub(super) approvals: approval::Approvals,
    /// 提问：问着人的、答完了还没交给工具的。
    pub(super) questions: question::Questions,
}

impl Watch {
    pub(super) fn new(seed: u64) -> Watch {
        Watch {
            seed,
            events: Vec::new(),
            pushed: BTreeSet::from([seq(1)]),
            received: BTreeMap::new(),
            replied: BTreeMap::new(),
            hooked: BTreeSet::new(),
            done: BTreeSet::new(),
            end_hooked: BTreeSet::new(),
            requests: BTreeMap::new(),
            issued: BTreeSet::new(),
            sent: BTreeSet::new(),
            recorded: BTreeSet::new(),
            asking: None,
            admitted: BTreeSet::new(),
            running: BTreeSet::new(),
            resulted: BTreeSet::new(),
            stopped: BTreeSet::new(),
            cwd: "~/src/miyu".to_string(),
            turn_cwd: BTreeMap::new(),
            seen_paths: BTreeSet::new(),
            next_block: 0,
            open_block: None,
            calm: false,
            queued: Vec::new(),
            interrupting: None,
            permission: lookup::created_permission(),
            effective: lookup::created_permission(),
            approvals: approval::Approvals::new(),
            questions: question::Questions::new(),
        }
    }

    /// 在路上、还没报发出去了的那次请求。
    pub(super) fn unsent(&self) -> Option<Seq> {
        self.asking.filter(|seen| !self.sent.contains(seen))
    }

    /// 追加过的最后一条。造会话那一条算在里面。
    pub(super) fn last(&self) -> u64 {
        self.events.last().map_or(1, |event| event.seq.get())
    }

    /// 送进一条输入之前记下它，送进去以后查吐出来的动作。新的打断：回合开着的，这一批以
    /// 被打断的 `turn.ended` 收尾；没开着的，拒绝，原因码 `not_running`。
    pub(super) fn feed(&mut self, session: &mut Session, input: Input) {
        let judged = self.before_approval(&input);
        let replied = self.before_question(&input);
        let fresh_interrupt = match &input {
            Input::Command(command) => match command.command {
                Command::Interrupt { queued } if !self.received.contains_key(&command.id) => {
                    Some(queued)
                }
                _ => None,
            },
            _ => None,
        };
        self.interrupting = fresh_interrupt;
        let was_open = self.turn_open();
        match &input {
            Input::Command(command) => {
                *self.received.entry(command.id.clone()).or_default() += 1;
            }
            Input::TurnStartHooksDone { turn, .. } if self.hooked.contains(turn) => {
                self.done.insert(*turn);
            }
            Input::RequestSent { seen, .. } if Some(*seen) == self.asking => {
                self.sent.insert(*seen);
            }
            Input::Environment(environment) => self.cwd = environment.cwd.clone(),
            _ => {}
        }
        let actions = session.handle(input);
        if fresh_interrupt.is_some() {
            self.interrupted(&actions, was_open);
        }
        self.after_approval(&actions, judged);
        self.after_question(&actions, replied);
        for action in actions {
            self.check(action);
        }
        self.interrupting = None;
    }

    /// 一次新的打断吐出来的动作。
    fn interrupted(&mut self, actions: &[Action], was_open: bool) {
        let seed = self.seed;
        if was_open {
            self.seen_paths.insert("打断了回合");
            let ended = actions.iter().any(|action| match action {
                Action::Append(events) => events.iter().any(|event| {
                    matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Interrupted)
                }),
                _ => false,
            });
            assert!(
                ended,
                "种子 {seed}：打断以后回合没以被打断结束：{actions:?}"
            );
        } else {
            self.seen_paths.insert("空闲时打断被拒");
            assert!(
                matches!(
                    actions,
                    [Action::Reply {
                        outcome: Outcome::Rejected {
                            reason: Reason::NotRunning
                        },
                        ..
                    }]
                ),
                "种子 {seed}：空闲时的打断应该拒绝：{actions:?}"
            );
        }
    }

    fn check(&mut self, action: Action) {
        let seed = self.seed;
        match action {
            Action::Append(events) => self.appended(events),
            Action::Push(events) => self.pushed.extend(events.iter().map(|event| event.seq)),
            Action::Reply { id, outcome } => {
                if let Outcome::Accepted { events } = &outcome {
                    assert!(
                        events.iter().all(|event| self.pushed.contains(event)),
                        "种子 {seed}：{id} 的事件还没推送就回应了"
                    );
                }
                *self.replied.entry(id).or_default() += 1;
            }
            Action::RunTurnStartHooks { turn } => {
                assert!(
                    self.opening(turn).all(|event| self.pushed.contains(&event)),
                    "种子 {seed}：回合 {turn} 的开头还没落盘就跑挂接点"
                );
                assert!(
                    self.hooked.insert(turn),
                    "种子 {seed}：回合 {turn} 叫了两次"
                );
            }
            Action::CallModel { seen, request } => self.called(seen, &request),
            Action::PushTransient(transient) => self.transient(&transient),
            Action::CancelModel { seen } => {
                self.seen_paths.insert("叫执行器别再发");
                assert!(
                    self.recorded.contains(&seen),
                    "种子 {seed}：叫执行器别再发的请求 {seen}，没记出错"
                );
            }
            Action::RunTurnEndHooks { turn } => {
                self.seen_paths.insert("跑了回合结束的挂接点");
                let ended = self.events.iter().find(|event| {
                    event.turn == Some(turn) && matches!(event.body, Body::TurnEnded(_))
                });
                assert!(
                    ended.is_some_and(|event| self.pushed.contains(&event.seq)),
                    "种子 {seed}：回合 {turn} 的结束还没落盘就跑挂接点"
                );
                assert!(
                    self.end_hooked.insert(turn),
                    "种子 {seed}：回合 {turn} 的结束挂接点跑了两次"
                );
            }
            Action::GuardTool {
                call_id,
                name,
                cwd,
                permission,
                ..
            } => self.guard(call_id, &name, &cwd, &permission),
            Action::RunTool { call_id, .. } => self.run(call_id),
            Action::AnswerTool { call_id, answers } => self.handed(call_id, &answers),
            Action::CancelTool { call_id } => {
                self.seen_paths.insert("打断了工具");
                assert!(
                    self.stopped.contains(&call_id),
                    "种子 {seed}：叫停的 {call_id} 不是在跑时被取消的"
                );
            }
        }
    }

    /// 请求模型：挂接点跑完了、事件都落了盘、上一步的调用都有了结果；请求照全部历史；
    /// 一个回合的请求不超过上限。
    fn called(&mut self, seen: Seq, request: &Request) {
        let seed = self.seed;
        let turn = self.open_turn();
        assert!(
            self.done.contains(&turn),
            "种子 {seed}：挂接点还没跑完就请求"
        );
        assert!(
            self.events
                .iter()
                .all(|event| self.pushed.contains(&event.seq)),
            "种子 {seed}：还有事件没落盘就请求"
        );
        assert!(
            self.calls_in(turn)
                .all(|call| self.resulted.contains(&call)),
            "种子 {seed}：上一步还有调用没结果就请求"
        );
        assert_eq!(seen.get(), self.last(), "种子 {seed}：seen 是最后一条");
        // 请求照的是全部历史，撤回的消息和撤回那一条除外。
        let withdrawn: BTreeSet<Seq> = self
            .events
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageWithdrawn(withdrawn) => Some(withdrawn.messages.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        let mut all = vec![self.created()];
        all.extend(
            self.events
                .iter()
                .filter(|event| {
                    !withdrawn.contains(&event.seq)
                        && !matches!(event.body, Body::MessageWithdrawn(_))
                })
                .cloned(),
        );
        assert_eq!(
            listed_request(request),
            listing(&all),
            "种子 {seed}：请求照全部历史，撤回的除外"
        );
        self.permission_request();
        let count = self.requests.entry(turn).or_default();
        *count += 1;
        assert!(
            *count <= STEP_LIMIT,
            "种子 {seed}：回合 {turn} 请求超过了上限"
        );
        if *count > 1 {
            self.seen_paths.insert("一步接一步");
        }
        self.issued.insert(seen);
        self.asking = Some(seen);
        self.next_block = 0;
        self.open_block = None;
    }

    /// 推给头的：增量是在路上的那次请求的；工具的输出是在跑的调用的。
    fn transient(&mut self, transient: &Transient) {
        let seed = self.seed;
        assert_eq!(transient.turn, Some(self.open_turn()));
        match &transient.body {
            TransientBody::ModelDelta(delta) => {
                self.seen_paths.insert("推了增量");
                assert_eq!(
                    Some(delta.seen),
                    self.asking,
                    "种子 {seed}：推的增量不是在路上的那次请求的"
                );
                assert!(
                    self.sent.contains(&delta.seen),
                    "种子 {seed}：请求还没发出去就推了增量"
                );
                assert!(matches!(transient.by, By::Model(_)));
            }
            TransientBody::ToolProgress(progress) => {
                self.seen_paths.insert("推了工具的输出");
                assert!(
                    self.running.contains(&progress.call_id),
                    "种子 {seed}：{} 不在跑，却推了它的输出",
                    progress.call_id
                );
            }
        }
    }

    /// 追加的一批：序号连着；`model.called` 每次请求至多一条，排在它的回复后面，出错的后面
    /// 紧跟着出错的 `turn.ended`；工具结果每个调用一条；步数上限只在请求满了的回合。
    fn appended(&mut self, events: Vec<Event>) {
        let seed = self.seed;
        for (k, event) in events.iter().enumerate() {
            assert_eq!(event.seq.get(), self.last() + 1, "种子 {seed}：序号要连着");
            match &event.body {
                Body::TurnStarted(_) => {
                    if !self.hooked.is_empty() {
                        self.seen_paths.insert("开了第二轮");
                    }
                    self.turn_cwd
                        .insert(TurnId::new(event.seq), self.cwd.clone());
                }
                Body::MessageAssistant(reply)
                    if reply
                        .blocks
                        .iter()
                        .any(|block| matches!(block, Block::ToolCall(_))) =>
                {
                    self.seen_paths.insert("回复里有工具调用");
                }
                Body::ModelCalled(called) => self.model_called(called, &events, k),
                Body::ToolResult(result) => {
                    assert!(
                        self.resulted.insert(result.call_id),
                        "种子 {seed}：{} 有了两条结果",
                        result.call_id
                    );
                    let was_running = self.running.remove(&result.call_id);
                    if matches!(event.by, By::Tool(_)) {
                        assert!(was_running, "种子 {seed}：没在跑的调用有了结果");
                    }
                    match result.status {
                        ToolStatus::Cancelled if was_running => {
                            self.stopped.insert(result.call_id);
                        }
                        ToolStatus::Skipped if was_running => {
                            assert!(
                                self.skips_running(event, result.call_id),
                                "种子 {seed}：在跑的调用被跳过了"
                            );
                            self.stopped.insert(result.call_id);
                        }
                        ToolStatus::Skipped => {
                            self.seen_paths.insert("急着插话跳过");
                        }
                        _ => {}
                    }
                }
                Body::TurnEnded(ended) if ended.reason == EndReason::StepLimit => {
                    self.seen_paths.insert("走到步数上限");
                    let turn = self.open_turn();
                    assert_eq!(
                        self.requests.get(&turn).copied(),
                        Some(STEP_LIMIT),
                        "种子 {seed}：请求没满就说到了上限"
                    );
                }
                _ => {}
            }
            self.queue_check(&events, k);
            self.permission_check(&events, k);
            self.approval_check(&events, k);
            self.question_check(&events, k);
            self.events.push(event.clone());
        }
    }

    /// 一条 `model.called`：交给过执行器、只记一次；说完了的前面是它的回复，出错的后面
    /// 紧跟着出错的回合结束。
    fn model_called(&mut self, called: &crate::event::ModelCalled, events: &[Event], k: usize) {
        let seed = self.seed;
        assert!(
            self.issued.contains(&called.seen),
            "种子 {seed}：没交给执行器的请求 {} 记了一条",
            called.seen
        );
        assert!(
            self.recorded.insert(called.seen),
            "种子 {seed}：请求 {} 记了两条",
            called.seen
        );
        let before = k.checked_sub(1).map(|k| &events[k].body);
        let after = events.get(k + 1).map(|event| &event.body);
        let interrupted_later = events[k..].iter().any(|event| {
            matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Interrupted)
        });
        match called.result {
            CallResult::Ok => {
                self.seen_paths.insert("说完了");
                assert!(
                    matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen),
                    "种子 {seed}：说完了的，前面是它的回复"
                );
            }
            CallResult::Interrupted => {
                self.seen_paths.insert("打断了请求");
                assert!(
                    interrupted_later,
                    "种子 {seed}：被打断的请求，这一批里接着是被打断的回合结束"
                );
            }
            _ => {
                self.seen_paths.insert("出错了");
                assert!(
                    matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error),
                    "种子 {seed}：出错的，后面紧跟着出错的回合结束"
                );
            }
        }
        if self.asking == Some(called.seen) {
            self.asking = None;
        }
    }
}
