//! 看守查换模型（施工 8-10，`docs/blueprint/models.md`「怎么走」第六条），照看守自己记下的日志算，不看会话：
//!
//! - 收下的换模型：和记着的引用一样的，回应不带事件、什么都不记；不一样的，只记一条 `session.policy_changed`，只有
//!   `model`，`by` 是换的人、`cause` 是命令，回合进行中的带上这个回合；
//! - 叫跑回合开始的挂接点时交的引用就是记着的那一个；
//! - 挂接点的结果带着退回：结果收下了、交来的原来的正是记着的、退回的不一样，追加的第一条就是退回的那一条（`model` 是
//!   退回的，`replaced` 是原来的，`by` 是内核，带着回合，`cause` 是回合的）；别的不记；
//! - 引用照日志里每一条带 `model` 的 `session.policy_changed` 换，撤掉的回合里的也算；熔断只看最近一次换模型以后的
//!   （`watch/breaker.rs`）。

use super::*;
use crate::event::PolicyChanged;
use crate::session::Replaced;

/// 看守记着的：会话的引用，最近一次换模型写在第几条。
#[derive(Default)]
pub(super) struct Models {
    reference: Option<String>,
    changed: Option<Seq>,
}

/// 送进去之前判出来的：该怎样。
pub(super) enum Expect {
    /// 换成和现在一样的：接受，什么都不记。
    Same(CommandId),
    /// 换成别的：记一条，换成它。
    Changed(CommandId, String),
    /// 挂接点的结果带着退回：该记的是这一次，不该记的是没有。
    Fallback(Option<Replaced>),
}

impl Watch {
    /// 会话现在的引用，照看守记下的日志。
    pub(in super::super) fn reference(&self) -> Option<&str> {
        self.models.reference.as_deref()
    }

    /// 第 `seq` 条写在最近一次换模型后面；没换过的都算（熔断照它数）。
    pub(super) fn after_model_change(&self, seq: Seq) -> bool {
        self.models.changed.is_none_or(|changed| seq > changed)
    }

    /// 送进一条输入之前：新的换模型、带着退回的挂接点结果，照规矩判出该怎样。改回文件的时候来的命令拒绝（`refused`）。
    pub(super) fn before_configure(&self, input: &Input, refused: bool) -> Option<Expect> {
        match input {
            Input::Command(Received {
                id,
                command: Command::Configure { model },
                ..
            }) if self.fresh(id) && !refused => Some(match self.reference() == Some(model) {
                true => Expect::Same(id.clone()),
                false => Expect::Changed(id.clone(), model.clone()),
            }),
            Input::TurnStartHooksDone {
                turn,
                replaced: Some(replaced),
                ..
            } => {
                let taken = self.turn_open()
                    && self.open_turn() == *turn
                    && self.hooked.contains(turn)
                    && !self.done.contains(turn);
                let records = taken
                    && self.reference() == Some(replaced.from.as_str())
                    && replaced.from != replaced.to;
                Some(Expect::Fallback(records.then(|| replaced.clone())))
            }
            _ => None,
        }
    }

    /// 送进去以后：照判出来的查吐出来的动作。
    pub(super) fn after_configure(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let appended: Vec<&Event> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Append(events) => Some(events),
                _ => None,
            })
            .flatten()
            .collect();
        match expect {
            None => {}
            Some(Expect::Same(id)) => {
                self.seen_paths.insert("换成一样的模型");
                assert!(appended.is_empty(), "种子 {seed}：换成一样的还记了");
                assert!(
                    actions.contains(&Action::Reply {
                        id,
                        outcome: Outcome::Accepted { events: Vec::new() }
                    }),
                    "种子 {seed}：换成一样的要当场回应、不带事件"
                );
            }
            Some(Expect::Changed(id, model)) => {
                self.seen_paths.insert("换了模型");
                assert_eq!(appended.len(), 1, "种子 {seed}：换模型只记一条");
                let event = appended[0];
                assert_eq!(event.body, changed(Some(model), None), "种子 {seed}");
                assert_eq!(event.by, alice(), "种子 {seed}：换的人");
                assert_eq!(event.cause, Some(id), "种子 {seed}");
                let turn = self.turn_open().then(|| self.open_turn());
                assert_eq!(event.turn, turn, "种子 {seed}：回合进行中的带上这个回合");
            }
            Some(Expect::Fallback(Some(Replaced { from, to }))) => {
                self.seen_paths.insert("退回了默认");
                let first = appended
                    .first()
                    .unwrap_or_else(|| panic!("种子 {seed}：退回了默认却什么都没记"));
                assert_eq!(first.body, changed(Some(to), Some(from)), "种子 {seed}");
                assert_eq!(first.by, By::Kernel, "种子 {seed}");
                assert_eq!(first.turn, Some(self.open_turn()), "种子 {seed}");
                assert_eq!(first.cause, self.cause_of(self.open_turn()), "种子 {seed}");
            }
            Some(Expect::Fallback(None)) => {
                self.seen_paths.insert("对不上的退回不记");
                assert!(
                    !appended.iter().any(|event| matches!(&event.body, Body::PolicyChanged(changed) if changed.replaced.is_some())),
                    "种子 {seed}：对不上的退回也记了"
                );
            }
        }
    }

    /// 叫跑回合开始的挂接点：交的引用就是记着的那一个。
    pub(super) fn hooks_model(&mut self, model: Option<&str>) {
        if model.is_some() {
            self.seen_paths.insert("回合开始交了引用");
        }
        assert_eq!(
            model,
            self.reference(),
            "种子 {}：交的不是会话的引用",
            self.seed
        );
    }

    /// 追加了一条带 `model` 的 `session.policy_changed`：不带权限、策略；人换的不带 `replaced`，内核写的带；引用换成它。
    pub(super) fn model_appended(&mut self, event: &Event) {
        let Body::PolicyChanged(changed) = &event.body else {
            return;
        };
        let Some(model) = &changed.model else {
            return;
        };
        let seed = self.seed;
        assert_eq!(
            (&changed.policy, &changed.permission),
            (&None, &None),
            "种子 {seed}"
        );
        assert_eq!(
            changed.replaced.is_some(),
            event.by == By::Kernel,
            "种子 {seed}：退回的才带 replaced，由内核写"
        );
        self.models.reference = Some(model.clone());
        self.models.changed = Some(event.seq);
    }

    /// 回合 `turn` 的 `cause`：它的 `turn.started` 的。
    fn cause_of(&self, turn: TurnId) -> Option<CommandId> {
        self.events
            .iter()
            .find(|event| event.seq == turn.started())
            .and_then(|event| event.cause.clone())
    }
}

/// 换模型的那一条的 `body`。
fn changed(model: Option<String>, replaced: Option<String>) -> Body {
    Body::PolicyChanged(PolicyChanged {
        model,
        replaced,
        ..PolicyChanged::default()
    })
}
