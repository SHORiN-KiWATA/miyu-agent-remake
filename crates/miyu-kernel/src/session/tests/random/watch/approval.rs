//! 看守查确认（`docs/designs/02-内核.md` 第六节「确认怎么走」）：
//!
//! - 交给链的：回复落了盘，只交一次；不是只读的不和别的占着位置的一起，只读的不和不是只读的
//!   一起，不越过前面还没结果的；带着回合开始时的工作目录、实际生效的那一级；
//! - 派去跑的：链放行过，或者人允许过、那条决定也落了盘；只派一次；只读生效的时候不派要写入的；
//! - 请求只在链说要问人时记，`by` 是提问的模块，写的就是链交回的；没人能确认的、只读时要写入的，
//!   不记，当场拒绝；
//! - 回答：看守自己照规矩判该接受还是拒绝；接受的记一条决定，`by` 是回答的人；
//! - 被拒绝的结果，谁拒的写成 `by`：链拒的是那个模块，人拒的是回答的人，没人能确认的是内核。

use super::*;
use crate::event::Decision;

/// 看守替执行器记着的确认：交给链的、送回去的结论、在等人的、人允许了的、人拒绝了的。
pub(in super::super) struct Approvals {
    /// 有没有人能确认。
    pub(in super::super) attended: bool,
    /// 交给了链、还没送回结论的调用。
    pub(in super::super) guarding: BTreeSet<CallId>,
    /// 送回去的结论。
    verdicts: BTreeMap<CallId, Verdict>,
    /// 在等人回答的调用，照日志里的请求。
    pub(in super::super) asking: BTreeSet<CallId>,
    /// 人允许了的调用，和那条决定的序号。
    approved: BTreeMap<CallId, Seq>,
    /// 人拒绝了的调用，和拒绝的人：它的结果要是这个人拒的。
    denied: BTreeMap<CallId, By>,
}

impl Approvals {
    pub(super) fn new() -> Approvals {
        Approvals {
            attended: true,
            guarding: BTreeSet::new(),
            verdicts: BTreeMap::new(),
            asking: BTreeSet::new(),
            approved: BTreeMap::new(),
            denied: BTreeMap::new(),
        }
    }
}

impl Watch {
    /// 送进一条输入之前：送回链的结论的，记下它；新的回答，照规矩判出该接受还是拒绝。
    pub(super) fn before_approval(&mut self, input: &Input) -> Option<Result<(), Reason>> {
        match input {
            Input::ToolGuarded {
                call_id, verdict, ..
            } if self.approvals.guarding.remove(call_id) => {
                self.approvals.verdicts.insert(*call_id, verdict.clone());
                None
            }
            Input::Command(received) if !self.received.contains_key(&received.id) => {
                match &received.command {
                    Command::Answer {
                        call_id,
                        decision,
                        reason,
                    } => Some(self.judge(*call_id, decision, reason.as_deref())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// 看守自己判一次回答：不在等的、选项不认识的、没有规则却选了记住的、允许却带了理由的，拒绝。
    fn judge(
        &self,
        call_id: CallId,
        decision: &Decision,
        reason: Option<&str>,
    ) -> Result<(), Reason> {
        if !self.approvals.asking.contains(&call_id) {
            return Err(Reason::NotAsking);
        }
        let rule = matches!(
            self.approvals.verdicts.get(&call_id),
            Some(Verdict::Ask { rule: Some(_), .. })
        );
        let reason = reason.filter(|reason| !reason.trim().is_empty());
        match decision {
            Decision::Other(_) => Err(Reason::UnknownDecision),
            Decision::Session | Decision::Workspace if !rule => Err(Reason::NoRule),
            Decision::Deny => Ok(()),
            _ if reason.is_some() => Err(Reason::UnexpectedReason),
            _ => Ok(()),
        }
    }

    /// 送进去以后：新的回答，照判出来的查回应。拒绝的只有一个回应；接受的记了一条决定。
    pub(super) fn after_approval(
        &mut self,
        actions: &[Action],
        judged: Option<Result<(), Reason>>,
    ) {
        let seed = self.seed;
        match judged {
            None => {}
            Some(Err(reason)) => {
                self.seen_paths.insert("回答被拒");
                assert!(
                    matches!(actions, [Action::Reply { outcome: Outcome::Rejected { reason: got }, .. }] if *got == reason),
                    "种子 {seed}：这个回答应该被拒，原因码 {}：{actions:?}",
                    reason.code()
                );
            }
            Some(Ok(())) => assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::Append(events)
                    if events.iter().any(|event| matches!(event.body, Body::ApprovalDecided(_))))),
                "种子 {seed}：这个回答应该记一条决定：{actions:?}"
            ),
        }
    }

    /// 交给链一个调用：回复落了盘；只交一次；不是只读的不和别的占着位置的一起，只读的不和不是
    /// 只读的一起；不越过前面还没结果的；带着回合开始时的工作目录、实际生效的那一级。
    pub(super) fn guard(
        &mut self,
        call_id: CallId,
        name: &str,
        cwd: &str,
        permission: &Permission,
    ) {
        let seed = self.seed;
        let reply = call_id.message();
        assert!(
            self.pushed.contains(&reply),
            "种子 {seed}：{call_id} 的回复还没落盘就交给了链"
        );
        assert!(
            self.admitted.insert(call_id),
            "种子 {seed}：{call_id} 交给了链两次"
        );
        let occupying: Vec<CallId> = self
            .admitted
            .iter()
            .filter(|id| **id != call_id && !self.resulted.contains(id))
            .copied()
            .collect();
        if name == "read" {
            assert!(
                occupying.iter().all(|id| self.name_of(*id) == "read"),
                "种子 {seed}：只读的 {call_id} 和不是只读的一起"
            );
        } else {
            assert!(occupying.is_empty(), "种子 {seed}：{name} 和别的一起");
        }
        let earlier: Vec<CallId> = self
            .calls_of(reply)
            .filter(|id| id.index() < call_id.index())
            .collect();
        for id in earlier {
            let exclusive = name != "read" || self.name_of(id) != "read";
            assert!(
                !exclusive || self.resulted.contains(&id),
                "种子 {seed}：{call_id} 越过了还没结果的 {id}"
            );
        }
        let turn = self.open_turn();
        assert_eq!(self.turn_cwd.get(&turn).map(String::as_str), Some(cwd));
        assert_eq!(
            permission, &self.effective,
            "种子 {seed}：交给链的不是实际生效的那一级"
        );
        self.approvals.guarding.insert(call_id);
    }

    /// 派一个调用去跑：链放行过，或者人允许过、那条决定也落了盘；只派一次；只读生效的时候不派
    /// 要写入的。
    pub(super) fn run(&mut self, call_id: CallId) {
        let seed = self.seed;
        self.seen_paths.insert("派了工具");
        let allowed = matches!(self.approvals.verdicts.get(&call_id), Some(Verdict::Allow));
        let approved = self
            .approvals
            .approved
            .get(&call_id)
            .is_some_and(|decided| self.pushed.contains(decided));
        assert!(
            allowed || approved,
            "种子 {seed}：{call_id} 没被链放行，也没被人允许（或者决定还没落盘）就派了"
        );
        let writes = self.writes(call_id);
        self.permission_run(call_id, writes);
        assert!(
            self.running.insert(call_id),
            "种子 {seed}：{call_id} 派了两次"
        );
    }

    /// 这个调用要不要写入：工具是写文件的，或者链问人时要的是写入。
    pub(super) fn writes(&self, call_id: CallId) -> bool {
        self.name_of(call_id) == "write"
            || matches!(
                self.approvals.verdicts.get(&call_id),
                Some(Verdict::Ask { access, .. }) if access.writes()
            )
    }

    /// 这一批里的第 `k` 条，照确认的规矩查。
    pub(super) fn approval_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::ApprovalRequested(request) => {
                self.seen_paths.insert("要问人");
                let call_id = request.call_id;
                let Some(Verdict::Ask {
                    module,
                    access,
                    rule,
                    detail,
                }) = self.approvals.verdicts.get(&call_id)
                else {
                    panic!("种子 {seed}：链没说要问人，{call_id} 却记了请求");
                };
                assert_eq!(
                    event.by,
                    By::Module(crate::origin::Module { id: module.clone() })
                );
                assert_eq!(
                    (&request.access, &request.rule, &request.detail),
                    (access, rule, detail),
                    "种子 {seed}：请求写的不是链交回的"
                );
                assert!(
                    self.approvals.attended,
                    "种子 {seed}：没人能确认，不该记请求"
                );
                assert!(
                    !(access.writes() && self.read_only_in_effect()),
                    "种子 {seed}：只读时要写入的，不该记请求"
                );
                self.approvals.asking.insert(call_id);
            }
            Body::ApprovalDecided(decided) => {
                let call_id = decided.call_id;
                assert!(
                    self.approvals.asking.remove(&call_id),
                    "种子 {seed}：{call_id} 不在等人，却有了决定"
                );
                assert!(
                    matches!(event.by, By::Person(_)),
                    "种子 {seed}：决定是人做的"
                );
                if decided.decision == Decision::Deny {
                    self.approvals.denied.insert(call_id, event.by.clone());
                } else {
                    self.seen_paths.insert("人允许了");
                    self.approvals.approved.insert(call_id, event.seq);
                }
            }
            Body::ToolResult(result) => {
                self.approvals.guarding.remove(&result.call_id);
                self.approvals.asking.remove(&result.call_id);
                if result.status == ToolStatus::Denied {
                    self.denial(event, result.call_id);
                }
            }
            _ => {}
        }
    }

    /// 一条被拒绝的结果：谁拒的写成 `by`。只读拦下的由看守的权限那一份查。
    fn denial(&mut self, event: &Event, call_id: CallId) {
        let seed = self.seed;
        let verdict = self.approvals.verdicts.get(&call_id);
        match &event.by {
            By::Person(_) => {
                self.seen_paths.insert("人拒绝了");
                assert_eq!(
                    self.approvals.denied.remove(&call_id).as_ref(),
                    Some(&event.by),
                    "种子 {seed}：人拒绝的 {call_id}，前面要有这个人的拒绝"
                );
            }
            By::Module(module) => {
                self.seen_paths.insert("链拒绝了");
                assert!(
                    matches!(verdict, Some(Verdict::Deny { module: m, .. }) if *m == module.id),
                    "种子 {seed}：{call_id} 不是这个模块拒的"
                );
            }
            By::Kernel if text_of(event) == "unattended" => {
                self.seen_paths.insert("没人能确认被拒");
                assert!(
                    !self.approvals.attended,
                    "种子 {seed}：有人能确认，却说没人"
                );
                assert!(
                    matches!(verdict, Some(Verdict::Ask { .. })),
                    "种子 {seed}：链没说要问人"
                );
            }
            By::Kernel => {}
            other => panic!("种子 {seed}：被拒绝的 {call_id}，by 不对：{other:?}"),
        }
    }
}

/// 一条工具结果里的那一句。
pub(super) fn text_of(event: &Event) -> &str {
    match &event.body {
        Body::ToolResult(result) => match result.blocks.as_slice() {
            [Block::Text(text)] => &text.text,
            _ => "",
        },
        _ => "",
    }
}
