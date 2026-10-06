//! 撤销、恢复时要改回的几步（`docs/designs/10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：照撤掉、恢复的
//! 那几轮的效果算，纯逻辑。执行器照它先核对再动手，一步一项交回结局（[`Restored`]），内核记成 `files.restored`。
//!
//! 移进过回收站的，现在在哪要照日志往后找：恢复时又移进了回收站，位置就变了；撤销时移回来了，就在原处
//! （[`where_is`]）。

use std::collections::BTreeMap;

use crate::event::{Body, Effect, Event, RestoreAction, RestoreOutcome, Restored};
use crate::id::{ContentHash, Seq};

/// 执行器交回的结局对不上交出去的那一步时，写进 `error` 的那一句（施工 4-9 再补一）。
const MISMATCH: &str = "executor report did not match";

/// 执行器交回的结局对照交出去的几步（施工 4-9 再补一）：一步一项、先后一样，每一项的 `result`、`effect`、`path`、
/// `action` 和那一步一样；移进回收站成了的要带着 `trash`。对不上的那一项改成出错；少了的照那一步补一项出错，
/// 多出来的不要。出错的记下以后，下一次撤销、恢复照它当那一步没做成。
pub(super) fn checked(steps: &[Step], files: Vec<Restored>) -> Vec<Restored> {
    let mut files = files.into_iter();
    steps
        .iter()
        .map(|step| match files.next() {
            Some(file) if fits(step, &file) => file,
            _ => Restored {
                outcome: RestoreOutcome::Failed,
                error: Some(MISMATCH.to_string()),
                ..step.restored()
            },
        })
        .collect()
}

/// 交回的这一项是不是那一步的：编号、路径、做什么都一样；移进回收站成了的带着新位置。
fn fits(step: &Step, file: &Restored) -> bool {
    let want = step.restored();
    let trashed_without_place = file.action == RestoreAction::Trash
        && file.outcome == RestoreOutcome::Restored
        && file.trash.is_none();
    file.result == want.result
        && file.effect == want.effect
        && file.path == want.path
        && file.action == want.action
        && !trashed_without_place
}

/// 改回的一步：照哪一条 `tool.result` 的第几个效果，改哪里，做什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// 那一条 `tool.result` 的序号。
    pub result: Seq,
    /// 那一条的第几个效果，从 0 数起。
    pub effect: u32,
    /// 改哪里：效果里记的路径。
    pub path: String,
    /// 做什么。
    pub action: StepAction,
}

/// 一步做什么，动手之前原处该是什么样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepAction {
    /// 写回 `content`（blob 的哈希）。原处要是 `expect` 的样子。
    Write {
        /// 原处该是什么样。
        expect: Expect,
        /// 写回的内容。
        content: ContentHash,
    },
    /// 移进回收站。原处要是 `expect` 的样子。
    Trash {
        /// 原处该是什么样。
        expect: Expect,
    },
    /// 从回收站的 `from` 移回原处。原处要空着，`from` 要还在。
    Untrash {
        /// 在回收站里的位置。
        from: String,
    },
}

/// 动手之前原处该是什么样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// 空着：什么都没有。
    Absent,
    /// 一个文件，内容是这个哈希。
    Content(ContentHash),
    /// 有东西就行：文件、目录都算（撤销时移回来的是目录，没记哈希）。
    Present,
}

impl Step {
    /// 这一步照做成了的结局：改回了，别的附项都是空的。执行器照它改。
    pub fn restored(&self) -> Restored {
        Restored {
            result: self.result,
            effect: self.effect,
            path: self.path.clone(),
            action: match self.action {
                StepAction::Write { .. } => RestoreAction::Write,
                StepAction::Trash { .. } => RestoreAction::Trash,
                StepAction::Untrash { .. } => RestoreAction::Untrash,
            },
            outcome: RestoreOutcome::Restored,
            found: None,
            trash: None,
            hash: None,
            error: None,
        }
    }
}

/// 撤销：`reverted` 是撤掉的那几轮的事件，`history` 是有效历史（找移进过回收站的现在在哪）。每条工具结果的效果照
/// 先后排好，倒过来：改过的写回改前的，新建的移进回收站，删掉的从回收站移回来。读过的、派了任务的、写了待办的、不认识的不用改回。
pub(super) fn undo(reverted: &[Event], history: &[Event]) -> Vec<Step> {
    let places = where_is(history);
    let mut steps: Vec<Step> = effects(reverted)
        .filter_map(|(result, effect, change)| {
            let (path, action) = match change {
                Effect::FileChanged(changed) => (
                    &changed.path,
                    match &changed.before {
                        Some(before) => StepAction::Write {
                            expect: Expect::Content(changed.after.clone()),
                            content: before.clone(),
                        },
                        None => StepAction::Trash {
                            expect: Expect::Content(changed.after.clone()),
                        },
                    },
                ),
                Effect::FileTrashed(trashed) => (
                    &trashed.path,
                    match places.get(&(result, effect)) {
                        None => StepAction::Untrash {
                            from: trashed.trash.clone(),
                        },
                        Some(Place::Trash(from)) => StepAction::Untrash { from: from.clone() },
                        Some(Place::Back(_)) => return None,
                    },
                ),
                // 派出去的任务不是改文件：撤销时停下它们随 7-8（`agents.md` 第七条）。订别的会话也不是（施工 C-1）。
                Effect::FileRead(_)
                | Effect::JobStarted(_)
                | Effect::JobMessaged(_)
                | Effect::PeerWatch(_)
                | Effect::TodoWritten(_)
                | Effect::Unknown(_) => return None,
            };
            Some(Step {
                result,
                effect,
                path: path.clone(),
                action,
            })
        })
        .collect();
    steps.reverse();
    steps
}

/// 恢复：`unreverted` 是要放回的那几轮的事件。同样那些效果照先后正着再做一遍：改过的写回改后的，新建的原处空着才
/// 写，删掉的上一次撤销真移回来了才再移进回收站。
pub(super) fn redo(unreverted: &[Event], history: &[Event]) -> Vec<Step> {
    let places = where_is(history);
    effects(unreverted)
        .filter_map(|(result, effect, change)| {
            let (path, action) = match change {
                Effect::FileChanged(changed) => (
                    &changed.path,
                    StepAction::Write {
                        expect: changed
                            .before
                            .clone()
                            .map_or(Expect::Absent, Expect::Content),
                        content: changed.after.clone(),
                    },
                ),
                Effect::FileTrashed(trashed) => (
                    &trashed.path,
                    match places.get(&(result, effect)) {
                        Some(Place::Back(hash)) => StepAction::Trash {
                            expect: hash.clone().map_or(Expect::Present, Expect::Content),
                        },
                        None | Some(Place::Trash(_)) => return None,
                    },
                ),
                // 派出去的任务不是改文件：撤销时停下它们随 7-8（`agents.md` 第七条）。订别的会话也不是（施工 C-1）。
                Effect::FileRead(_)
                | Effect::JobStarted(_)
                | Effect::JobMessaged(_)
                | Effect::PeerWatch(_)
                | Effect::TodoWritten(_)
                | Effect::Unknown(_) => return None,
            };
            Some(Step {
                result,
                effect,
                path: path.clone(),
                action,
            })
        })
        .collect()
}

/// 移进过回收站的一个效果现在在哪。
#[derive(Debug)]
enum Place {
    /// 在回收站的这个位置。
    Trash(String),
    /// 撤销时移回了原处；是文件的，记着它的哈希。
    Back(Option<ContentHash>),
}

/// 照有效历史里的 `files.restored` 往后找：每个效果最后一次改成了的，是移进回收站（在哪）还是移回来（原处）。
/// 没改成的不算：东西还在上一次的地方。
fn where_is(history: &[Event]) -> BTreeMap<(Seq, u32), Place> {
    let mut places = BTreeMap::new();
    for event in history {
        let Body::FilesRestored(restored) = &event.body else {
            continue;
        };
        for file in &restored.files {
            if file.outcome != RestoreOutcome::Restored {
                continue;
            }
            let place = match (&file.action, &file.trash) {
                (RestoreAction::Untrash, _) => Place::Back(file.hash.clone()),
                (RestoreAction::Trash, Some(trash)) => Place::Trash(trash.clone()),
                _ => continue,
            };
            places.insert((file.result, file.effect), place);
        }
    }
    places
}

/// 这几条事件里每条工具结果的每个效果，照先后：那一条的序号、第几个、效果。
fn effects(events: &[Event]) -> impl Iterator<Item = (Seq, u32, &Effect)> {
    events.iter().flat_map(|event| {
        let effects: &[Effect] = match &event.body {
            Body::ToolResult(result) => &result.effects,
            _ => &[],
        };
        (0u32..)
            .zip(effects)
            .map(move |(index, effect)| (event.seq, index, effect))
    })
}

#[cfg(test)]
mod tests;
