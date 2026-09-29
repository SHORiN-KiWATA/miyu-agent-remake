//! 看守查改回文件（`docs/designs/10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：
//!
//! - 撤销、恢复的那几轮里改过文件的，同一批交出改回文件，一个改过的文件一步；没改过的不交；
//! - 改回文件、读回日志（施工 6-9）的时候，新来的命令一律拒绝，原因码 `restoring`；不开新的一轮；
//! - 结局回来了，只追加一条 `files.restored`，一步一项照原样，`by` 是撤销、恢复的人，不属于哪一轮；
//! - 没在改的时候来的结局是过时的，不理。

use super::*;
use crate::event::{Effect, FileChanged, FilesRestored, Restored};
use crate::session::Step;

/// 看守记着的改回文件：交出去了、结局还没回来的那几步。
#[derive(Default)]
pub(in super::super) struct Restoring {
    pub(in super::super) pending: Option<Vec<Step>>,
}

/// 送进一条输入之前判出来的。
pub(super) enum Expect {
    /// 改回文件的时候来的新命令：拒绝，原因码 `restoring`。
    Refused,
    /// 改回文件的结局：只追加一条 `files.restored`，一步一项照原样。
    Recorded(Vec<Restored>),
    /// 没在改的时候来的结局：不理。
    Stale,
}

impl Watch {
    /// 新来的命令是不是撞上了改回文件、读回日志：撞上的，别的看守不再照自己的规矩判。
    pub(super) fn restoring_refuses(&self, input: &Input) -> bool {
        (self.restoring.pending.is_some() || self.undo.reading.is_some())
            && matches!(input, Input::Command(received) if self.fresh(&received.id))
    }

    /// 送进一条输入之前：改回文件相关的，照规矩判出该怎样。
    pub(super) fn before_restore(&self, input: &Input) -> Option<Expect> {
        if self.restoring_refuses(input) {
            return Some(Expect::Refused);
        }
        match (input, &self.restoring.pending) {
            (Input::Restored { files, .. }, Some(_)) => Some(Expect::Recorded(files.clone())),
            (Input::Restored { .. }, None) => Some(Expect::Stale),
            _ => None,
        }
    }

    /// 送进去以后：照判出来的查。
    pub(super) fn after_restore(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        match expect {
            None => {}
            Some(Expect::Refused) => {
                self.seen_paths.insert(match self.undo.reading {
                    Some(_) => "读回日志时来的命令被拒",
                    None => "改回文件时来的命令被拒",
                });
                assert!(
                    matches!(
                        actions,
                        [Action::Reply {
                            outcome: Outcome::Rejected {
                                reason: Reason::Restoring
                            },
                            ..
                        }]
                    ),
                    "种子 {seed}：改回文件的时候来的命令应该拒绝，原因码 restoring：{actions:?}"
                );
            }
            Some(Expect::Recorded(files)) => {
                self.seen_paths.insert("改回文件的结局记下了");
                self.restoring.pending = None;
                let expected = Body::FilesRestored(FilesRestored { files });
                // 后面还有的，是恢复以后记在一边的回报接着开的那一轮（施工 7-2，`watch/reports.rs` 查）。
                assert!(
                    matches!(actions, [Action::Append(events)] if matches!(events.as_slice(), [event, ..] if event.body == expected && event.by == alice() && event.turn.is_none())),
                    "种子 {seed}：结局只追加一条 files.restored，照原样：{actions:?}"
                );
            }
            Some(Expect::Stale) => {
                self.seen_paths.insert("过时的改回结局不理");
                assert!(actions.is_empty(), "种子 {seed}：过时的结局：{actions:?}");
            }
        }
    }

    /// 内核交出改回文件：没在改；刚追加的是撤销或者恢复；步数正好是那几轮里改过文件的次数（随机测试里只有
    /// `file.changed`）。
    pub(super) fn restore_asked(&mut self, steps: Vec<Step>) {
        let seed = self.seed;
        self.seen_paths.insert("撤销、恢复时改回文件");
        assert!(
            self.restoring.pending.is_none(),
            "种子 {seed}：上一次的还没回来又交出改回文件"
        );
        let turns = match self.events.last().map(|event| &event.body) {
            Some(Body::TurnReverted(reverted)) => reverted.turns.clone(),
            Some(Body::TurnUnreverted(unreverted)) => unreverted.turns.clone(),
            _ => panic!("种子 {seed}：没撤销、没恢复却交出改回文件：{steps:?}"),
        };
        assert_eq!(
            steps.len(),
            self.changes_in(&turns),
            "种子 {seed}：一个改过的文件一步：{steps:?}"
        );
        self.restoring.pending = Some(steps);
    }

    /// 接受了的撤销、恢复：那几轮改过文件的，同一批交出改回文件；没改过的不交，当场照旧。
    pub(super) fn restore_matches(&self, actions: &[Action], turns: Option<Vec<TurnId>>) {
        let Some(turns) = turns else {
            return;
        };
        let asked = actions
            .iter()
            .any(|action| matches!(action, Action::Restore { .. }));
        assert_eq!(
            asked,
            self.changes_in(&turns) > 0,
            "种子 {}：那几轮改过文件才交出改回文件：{actions:?}",
            self.seed
        );
    }

    /// 工具报的效果，不占随机数（原来那串输入不跟着错开）：照有了结果的调用数轮着来，三条里一条新建了一个文件、
    /// 一条改了一个文件，一条什么都没报。撤销、恢复时才有要改回的。每一条还派一个任务（施工 7-1，`jobs.rs`）：
    /// 执行器交回来记下的结果三百例里只有二十来条，少派几个就凑不齐一个会话派两个。
    pub(in super::super) fn some_effects(&self) -> Vec<Effect> {
        let n = self.resulted.len();
        // 读过的文件换着来（施工 6-5）：压后重建挑候选时，尾巴里没读过的才重读。
        let read = Effect::FileRead(crate::event::FileRead {
            path: format!("/w/r{}.txt", n % 4),
            lines: Some([1, 2]),
            hash: ContentHash::of(b"read"),
        });
        let changed = |before| {
            Effect::FileChanged(FileChanged {
                path: "/w/a.txt".to_string(),
                before,
                after: ContentHash::of(b"after"),
            })
        };
        let mut effects = match n % 3 {
            0 => vec![changed(None), read],
            1 => vec![changed(Some(ContentHash::of(b"before"))), read],
            _ => vec![read],
        };
        effects.push(self.some_job(n));
        effects
    }

    /// 这几轮里的工具结果一共改过几次文件：读过的、派了任务的不算。撤销、恢复以后没交出改回文件的，这里该是 0。
    pub(super) fn changes_in(&self, turns: &[TurnId]) -> usize {
        self.events
            .iter()
            .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
            .map(|event| match &event.body {
                Body::ToolResult(result) => result
                    .effects
                    .iter()
                    .filter(|effect| {
                        matches!(effect, Effect::FileChanged(_) | Effect::FileTrashed(_))
                    })
                    .count(),
                _ => 0,
            })
            .sum()
    }
}
