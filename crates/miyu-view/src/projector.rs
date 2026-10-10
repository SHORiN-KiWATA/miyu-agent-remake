//! 投影这台小机器（`docs/blueprint/view.md`「怎么走」）：事件照先后一条条喂进去，每喂一条交出它引起的变化。编号只看
//! 日志里的位置，所以从哪一条开始喂都一样；流式来的块和落了盘以后的最后一样（喂完同一段日志，[`Projector::entries`]
//! 一样）。
//!
//! 一条事件里同一条可能改好几次：先记下动过哪些（`Op`），喂完一条再照最后的样子交出变化（[`Projector::drain`]）。

mod blocks;
mod notices;
mod status;
mod tools;
mod turns;
mod undo;
mod users;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use miyu_kernel::event::{Body as EventBody, CallError, Event, Transient, TransientBody, Usage};
use miyu_kernel::id::{CallId, ContentHash, JobId, ModelName, ProviderId, Seq, TurnId};
use miyu_kernel::time::Timestamp;

use crate::change::Change;
use crate::entry::{Body, Entry, EntryId, Group, Notice};
use crate::summary;
use crate::words::Texts;

/// 改了的文件加减了几行：投影是纯逻辑、不读 blob，由外面交进来（端点照两个 blob 算，同 `view.detail`）。
pub trait Lines: Send + Sync {
    /// 改前是 `before`（新建的没有）、改后是 `after` 的那一个文件，加了几行、删了几行；算不出的是 `None`。
    fn count(&self, before: Option<&ContentHash>, after: &ContentHash) -> Option<(u64, u64)>;
}

/// 投影。
pub struct Projector {
    texts: Arc<Texts>,
    lines: Option<Arc<dyn Lines>>,
    /// 条目，照显示的先后。
    entries: Vec<Entry>,
    /// 编号到它在 `entries` 里第几条。
    at: BTreeMap<EntryId, usize>,
    /// 这一批动过哪些，喂完一条事件照最后的样子交出去。
    ops: Vec<Op>,
    /// 在进行的那一段。
    group: Option<EntryId>,
    /// 一步什么时候结束：思考停了、工具有了结果。
    ended: BTreeMap<EntryId, Timestamp>,
    /// 在跑的这一轮。
    turn: Option<Running>,
    /// 这次请求的块：块号到条目（还没出字的正文、思考没有条目）。
    blocks: BTreeMap<usize, Block>,
    /// 这次请求的 `seen`。
    request: Option<Seq>,
    /// 调用编号到那一步。
    calls: BTreeMap<CallId, EntryId>,
    /// 还排着的消息：序号到条目。
    queued: BTreeMap<Seq, EntryId>,
    /// 每条消息：序号到条目（撤回、开轮时照它找）。
    users: BTreeMap<Seq, EntryId>,
    /// 现在的权限级别：发出去的消息、一轮的收尾照它记。
    level: Option<String>,
    /// 派出去的后台任务。
    jobs: BTreeMap<JobId, Job>,
    /// 一组题：调用编号到问的那一组。
    questions: BTreeMap<CallId, Vec<miyu_kernel::event::Question>>,
    /// 最近一条回复：`seen` 和每一块（照回复里的位置）的条目，`model.called` 照它对上块的起止。
    reply: Option<(Seq, Vec<Option<EntryId>>)>,
    /// 在压的那一条。
    compacting: Option<EntryId>,
    /// 最近一条撤销说明：改回的文件、停掉的任务记在它上面。
    reverted: Option<EntryId>,
    /// 最近一次说话的端点、模型。
    model: (Option<ProviderId>, Option<ModelName>),
    /// 最后一条落了盘的序号，和它后面已经有几条只在视图流里的旁白。
    last: (u64, u32),
    /// 开过的每一轮，照先后：回顾讲到哪一轮照它找。
    turns: Vec<TurnId>,
    /// 最近一条事件的时刻：块收全、步停表照它。
    now: Timestamp,
    /// 会话状态里投影算得出的那一半（施工 9-8 补上）。
    tracking: status::Tracking,
}

/// 在跑的这一轮。
struct Running {
    id: TurnId,
    started: Timestamp,
    usage: Option<Usage>,
    level: Option<String>,
    failure: Option<CallError>,
    /// 没有触发的那一轮：手动压缩、清空，不另起收尾。
    manual: bool,
    /// 这一轮她出过字、做过事。
    spoke: bool,
}

/// 这次请求的一块。
#[derive(Debug, Clone)]
enum Block {
    /// 正文：出了字才有条目。
    Text(Option<EntryId>),
    /// 思考：出了字才有条目。
    Thought(Option<EntryId>),
    /// 调工具：开始就有条目。
    Tool(EntryId),
}

/// 一件后台任务派出去时的样子。
#[derive(Debug, Clone)]
struct Job {
    what: miyu_kernel::event::JobKind,
    title: String,
    command: Option<String>,
}

/// 这一批动过的。
#[derive(Debug, Clone)]
enum Op {
    Add(EntryId),
    Touch(EntryId),
    Move(EntryId),
    Append(EntryId, String),
    Hidden(Vec<EntryId>, bool),
    Remove(EntryId),
}

impl Projector {
    /// 照这几样字起一台。`lines` 是算改了多少行的端口，没有的照参数估。
    #[must_use]
    pub fn new(texts: Arc<Texts>, lines: Option<Arc<dyn Lines>>) -> Projector {
        Projector {
            texts,
            lines,
            entries: Vec::new(),
            at: BTreeMap::new(),
            ops: Vec::new(),
            group: None,
            ended: BTreeMap::new(),
            turn: None,
            blocks: BTreeMap::new(),
            request: None,
            calls: BTreeMap::new(),
            queued: BTreeMap::new(),
            users: BTreeMap::new(),
            level: None,
            jobs: BTreeMap::new(),
            questions: BTreeMap::new(),
            reply: None,
            compacting: None,
            reverted: None,
            model: (None, None),
            last: (0, 0),
            turns: Vec::new(),
            now: Timestamp::from_unix_millis(0).unwrap_or_else(|| unreachable!("0 是合法的时刻")),
            tracking: status::Tracking::default(),
        }
    }

    /// 喂一条落了盘的事件，交回它引起的变化。
    pub fn event(&mut self, event: &Event) -> Vec<Change> {
        self.last = (event.seq.get(), 0);
        self.now = event.at;
        match &event.body {
            EventBody::SessionCreated(created) => {
                self.level = Some(users::level(&created.permission));
            }
            EventBody::PolicyChanged(changed) => self.policy(event, changed),
            EventBody::WorkspaceChanged(changed) => self.workspace(event, changed),
            EventBody::SessionRecapped(recap) => self.recap(event, recap),
            EventBody::CommandRan(_) => self.settle(),
            EventBody::TurnStarted(started) => self.started(event, started),
            EventBody::TurnJoined(joined) => self.joined(event, joined),
            EventBody::TurnEnded(ended) => self.ended(event, ended),
            EventBody::TurnReverted(reverted) => self.reverted(event, reverted),
            EventBody::TurnUnreverted(unreverted) => self.unreverted(unreverted),
            EventBody::FilesRestored(restored) => self.restored(restored),
            EventBody::MessageUser(message) => self.message(event, message),
            EventBody::MessageAssistant(reply) => self.assistant(event, reply),
            EventBody::MessageWithdrawn(withdrawn) => self.withdrawn(withdrawn),
            EventBody::ToolResult(result) => self.result(event, result),
            EventBody::ApprovalRequested(requested) => self.approval_requested(requested),
            EventBody::ApprovalDecided(decided) => self.approval_decided(event, decided),
            EventBody::QuestionAsked(asked) => {
                self.questions
                    .insert(asked.call_id, asked.questions.clone());
            }
            EventBody::QuestionAnswered(answered) => self.answered(event, answered),
            EventBody::ContextCompacted(compacted) => self.compacted(event, compacted),
            EventBody::CompactionPaused(paused) => self.paused(event, paused),
            EventBody::ModelCalled(called) => self.called(event, called),
            EventBody::JobReported(reported) => self.job_reported(event, reported),
            EventBody::ChildReported(reported) => self.child_reported(event, reported),
            EventBody::PeerIdle(idle) => self.peer(event, idle),
            _ => {}
        }
        self.track(event);
        self.drain()
    }

    /// 喂一条瞬时的事件，交回它引起的变化。
    pub fn transient(&mut self, transient: &Transient) -> Vec<Change> {
        self.now = transient.at;
        match &transient.body {
            TransientBody::ModelDelta(delta) => self.delta(transient.at, delta),
            TransientBody::CompactionProgress(progress) => self.progress(transient.at, progress),
            TransientBody::CompactionDone(done) => self.compaction_done(done),
            TransientBody::ModelChanged(changed) => self.model_changed(transient.at, changed),
            _ => {}
        }
        self.track_transient(transient);
        self.drain()
    }

    /// 换一套字（施工 9-8 下）：连接的 `ui.language` 改了，从下一条变化起照新的；已经交出去的不重算。
    pub fn retext(&mut self, texts: Arc<Texts>) {
        self.texts = texts;
    }

    /// 翻页时，这一页之前的日志：只学派出去的后台任务（标题、命令），不出条目（施工 9-8 中）。这一页里报完了、派在更早的
    /// 任务照它写那一行；不学的话，那一行没有标题、命令。会话状态的任务表也照它补上更早派出、了结的（施工 9-8 补上）。
    pub fn learn(&mut self, earlier: &[Event]) {
        self.learn_jobs(earlier);
        self.learn_rows(earlier);
    }

    /// 现在的全部条目，照显示的先后。
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// 编号是 `id` 的那一条。
    #[must_use]
    pub fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.at.get(id).map(|&i| &self.entries[i])
    }

    /// 在末尾加一条。
    fn add(&mut self, entry: Entry) {
        let id = entry.id.clone();
        if self.at.contains_key(&id) {
            return;
        }
        self.at.insert(id.clone(), self.entries.len());
        self.entries.push(entry);
        self.ops.push(Op::Add(id));
    }

    /// 改编号是 `id` 的那一条；没有的不改。
    fn touch(&mut self, id: &EntryId, change: impl FnOnce(&mut Entry)) {
        if let Some(&i) = self.at.get(id) {
            change(&mut self.entries[i]);
            self.ops.push(Op::Touch(id.clone()));
        }
    }

    /// 改编号是 `id` 的那一条：`change` 交回真改了才算动过。
    fn touch_if(&mut self, id: &EntryId, change: impl FnOnce(&mut Entry) -> bool) {
        if let Some(&i) = self.at.get(id)
            && change(&mut self.entries[i])
        {
            self.ops.push(Op::Touch(id.clone()));
        }
    }

    /// 把编号是 `id` 的那一条挪到末尾，再照 `change` 改。
    fn move_to_end(&mut self, id: &EntryId, change: impl FnOnce(&mut Entry)) {
        let Some(i) = self.at.get(id).copied() else {
            return;
        };
        let mut entry = self.entries.remove(i);
        change(&mut entry);
        self.entries.push(entry);
        self.reindex(i);
        self.ops.push(Op::Move(id.clone()));
    }

    /// 拿掉编号是 `id` 的那一条：流式时开了、落了盘却没有的块，被打断的压缩。
    fn remove(&mut self, id: &EntryId) {
        let Some(i) = self.at.remove(id) else {
            return;
        };
        self.entries.remove(i);
        self.reindex(i);
        self.ops.push(Op::Remove(id.clone()));
    }

    /// 第 `from` 条起的位置变了，重记。
    fn reindex(&mut self, from: usize) {
        for (i, entry) in self.entries.iter().enumerate().skip(from) {
            self.at.insert(entry.id.clone(), i);
        }
    }

    /// 正在写的字接在后面。
    fn append(
        &mut self,
        id: &EntryId,
        text: &str,
        field: impl FnOnce(&mut Body) -> Option<&mut String>,
    ) {
        if text.is_empty() {
            return;
        }
        if let Some(&i) = self.at.get(id)
            && let Some(slot) = field(&mut self.entries[i].body)
        {
            slot.push_str(text);
            self.ops.push(Op::Append(id.clone(), text.to_string()));
        }
    }

    /// 这几条藏起来、显示回来。
    fn hide(&mut self, ids: Vec<EntryId>, hidden: bool) {
        let mut changed = Vec::new();
        for id in ids {
            if let Some(&i) = self.at.get(&id)
                && self.entries[i].hidden != hidden
            {
                self.entries[i].hidden = hidden;
                changed.push(id);
            }
        }
        if !changed.is_empty() {
            self.ops.push(Op::Hidden(changed, hidden));
        }
    }

    /// 插进一条旁白：前面在进行的那一段先收起（终端蓝图「时间线」第 21 条）。
    fn notice(&mut self, id: EntryId, at: Timestamp, notice: Notice) {
        self.close_group();
        let turn = self.turn.as_ref().map(|t| t.id);
        self.add(Entry {
            id,
            body: Body::Notice(notice),
            turn,
            hidden: false,
            at,
        });
    }

    /// 只在视图流里有的旁白的编号。
    fn fresh_transient(&mut self) -> EntryId {
        self.last.1 += 1;
        EntryId::transient(self.last.0, self.last.1)
    }

    /// 收起在进行的那一段：照最后的样子算收起那一行、用时。
    fn close_group(&mut self) {
        if let Some(id) = self.group.take() {
            self.refresh_group(&id, false);
        }
    }

    /// 重算一段的收起那一行和用时；`open` 是它还在不在进行。
    fn refresh_group(&mut self, id: &EntryId, open: bool) {
        let Some(&i) = self.at.get(id) else {
            return;
        };
        let Body::Group(group) = &self.entries[i].body else {
            return;
        };
        let steps = group.steps.clone();
        let counted: Vec<_> = steps
            .iter()
            .filter_map(|step| self.get(step))
            .map(|entry| tools::counted(entry, &self.texts))
            .collect();
        let starts = steps
            .iter()
            .filter_map(|s| self.get(s))
            .map(|e| e.at.unix_millis());
        let first = starts.min();
        let last = steps
            .iter()
            .filter_map(|s| self.ended.get(s))
            .map(|t| t.unix_millis())
            .max();
        let took = match (first, last) {
            (Some(first), Some(last)) if !open => Some(u64::try_from(last - first).unwrap_or(0)),
            _ => None,
        };
        let shown_took = took.unwrap_or(0);
        let start = first.and_then(Timestamp::from_unix_millis);
        let next = Group {
            steps,
            open,
            summary: summary::line(&counted, shown_took, self.texts.local.as_ref()),
            summary_en: summary::line(&counted, shown_took, self.texts.english.as_ref()),
            failed: summary::failed(&counted),
            took_ms: took,
        };
        let at = start.unwrap_or(self.entries[i].at);
        if self.entries[i].body != Body::Group(next.clone()) || self.entries[i].at != at {
            self.touch(id, |entry| {
                entry.body = Body::Group(next);
                entry.at = at;
            });
        }
    }

    /// 照最后的样子交出这一批的变化。
    fn drain(&mut self) -> Vec<Change> {
        let ops = std::mem::take(&mut self.ops);
        let added: BTreeSet<EntryId> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Add(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        let removed: BTreeSet<EntryId> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Remove(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        // 这一批整条交过的（加的、换的）不再接字：整条里已经有了。
        let whole: BTreeSet<EntryId> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Add(id) | Op::Touch(id) | Op::Move(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        let moved: BTreeSet<EntryId> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Move(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        let mut done: BTreeSet<EntryId> = BTreeSet::new();
        let mut out = Vec::new();
        for op in ops {
            match op {
                Op::Add(id) if !removed.contains(&id) && done.insert(id.clone()) => {
                    if let Some(entry) = self.get(&id) {
                        out.push(Change::Add {
                            entry: entry.clone(),
                            after: self.before(&id),
                        });
                    }
                }
                Op::Touch(id) | Op::Move(id)
                    if !added.contains(&id)
                        && !removed.contains(&id)
                        && done.insert(id.clone()) =>
                {
                    if let Some(entry) = self.get(&id) {
                        let after = moved.contains(&id).then(|| self.before(&id));
                        out.push(Change::Update {
                            entry: entry.clone(),
                            after,
                        });
                    }
                }
                Op::Append(id, text) if !whole.contains(&id) => {
                    out.push(Change::Append { id, text })
                }
                Op::Hidden(ids, hidden) => {
                    let ids: Vec<_> = ids.into_iter().filter(|id| !added.contains(id)).collect();
                    if !ids.is_empty() {
                        out.push(Change::Hidden { ids, hidden });
                    }
                }
                Op::Remove(id) if !added.contains(&id) => out.push(Change::Remove { id }),
                _ => {}
            }
        }
        out
    }

    /// 排在 `id` 前面的那一条；它是第一条的是 `None`。
    fn before(&self, id: &EntryId) -> Option<EntryId> {
        let i = *self.at.get(id)?;
        i.checked_sub(1).map(|j| self.entries[j].id.clone())
    }
}
