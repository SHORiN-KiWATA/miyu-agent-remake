//! 撤销、恢复（终端蓝图 `tui.md`「正文」）：撤掉的几轮藏起来，别处来的话不跟着藏；另起一条撤销说明，改回了哪些文件、
//! 停掉了哪些任务记在它上面。恢复时那几轮显示回来，最近那一条撤销说明藏起来。

use miyu_kernel::event::{Event, FilesRestored, TurnReverted, TurnUnreverted};
use miyu_kernel::id::TurnId;

use super::Projector;
use crate::entry::{Body, EntryId};
use crate::notice::{Notice, RevertedFile};

impl Projector {
    /// 撤掉了几轮。
    pub(super) fn reverted(&mut self, event: &Event, reverted: &TurnReverted) {
        let said = self.first_said(&reverted.turns);
        let ids = self.of_turns(&reverted.turns);
        self.hide(ids, true);
        let id = EntryId::event(event.seq);
        self.notice(
            id.clone(),
            event.at,
            Notice::Reverted {
                turns: reverted.turns.clone(),
                said,
                files: Vec::new(),
                jobs: Vec::new(),
            },
        );
        self.reverted = Some(id);
    }

    /// 恢复了最近一次撤掉的几轮。
    pub(super) fn unreverted(&mut self, unreverted: &TurnUnreverted) {
        let ids = self.of_turns(&unreverted.turns);
        self.hide(ids, false);
        if let Some(id) = self.reverted.take() {
            self.hide(vec![id], true);
        }
    }

    /// 改回文件的结局：记在最近那一条撤销说明上（恢复时改回的不记）。
    pub(super) fn restored(&mut self, restored: &FilesRestored) {
        let Some(id) = self.reverted.clone() else {
            return;
        };
        let files: Vec<RevertedFile> = restored
            .files
            .iter()
            .map(|file| RevertedFile {
                path: file.path.clone(),
                outcome: file.outcome.clone(),
            })
            .collect();
        self.touch(&id, |entry| {
            if let Body::Notice(Notice::Reverted { files: mine, .. }) = &mut entry.body {
                mine.extend(files);
            }
        });
    }

    /// 属于这几轮的条目：别处来的话不算；讲到这几轮的回顾也算。
    fn of_turns(&self, turns: &[TurnId]) -> Vec<EntryId> {
        self.entries
            .iter()
            .filter(|entry| match &entry.body {
                Body::User(user) => {
                    user.from.is_none() && entry.turn.is_some_and(|t| turns.contains(&t))
                }
                Body::Notice(Notice::Recap { covers, .. }) => {
                    covers.is_some_and(|t| turns.contains(&t))
                        || entry.turn.is_some_and(|t| turns.contains(&t))
                }
                _ => entry.turn.is_some_and(|t| turns.contains(&t)),
            })
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// 撤掉的第一轮里人说的第一句的头一行。
    fn first_said(&self, turns: &[TurnId]) -> Option<String> {
        let first = turns.iter().min()?;
        self.entries.iter().find_map(|entry| match &entry.body {
            Body::User(user) if entry.turn == Some(*first) => {
                user.text.lines().next().map(|line| line.trim().to_string())
            }
            _ => None,
        })
    }
}
