//! 好几个会话（蓝图 `tui.md`「后台命令、子代理和侧边栏」「切进子会话」「别处来的话」）：正在看的会话照旧在 `App`
//! 身上（正文、任务表、视口），别的停放着——切进子会话时的主会话、子代理的会话、`/new` 以后还有任务在跑的旧会话。
//! 核心推来的照会话分：任务的几种记进那个会话的任务表，别处来的话画进那个会话的正文，别的交给那个会话的正文。

use std::collections::HashMap;
use std::time::Instant;

use super::App;
use crate::body_view::BodyView;
use crate::config::JobTexts;
use crate::core::{Bill, Command, JobStart, Push, Sender, Update, Usage};
use crate::jobs::{Board, Job, JobKind};
use crate::transcript::Transcript;

mod notes;
use notes::{Relation, announce, doing, tokens, untrusted};

/// 停放着的一个会话。
#[derive(Debug, Default)]
pub struct Parked {
    /// 正文。
    pub transcript: Transcript,
    /// 任务表和待办。
    pub board: Board,
    /// 视口：切回来时照旧停在原处。
    pub view: BodyView,
}

/// 停放着的会话，照编号。
pub type Lot = HashMap<String, Parked>;

impl App {
    /// 核心推来的是哪个会话的：正在看的会话的交回来照常办；别的交给停放着的那一份，交回 `None`。
    pub(super) fn route(&mut self, update: Update) -> Option<Update> {
        match update {
            Update::Elsewhere { session, update } => {
                if self.transcript.session.as_deref() == Some(session.as_str()) {
                    return Some(*update);
                }
                self.park_update(&session, *update);
                None
            }
            // 切进了子会话：主会话推来的交给停放着的主会话。
            Update::Push(_) | Update::Limits(_) if self.home.is_some() => {
                let main = self.home.clone().unwrap_or_default();
                self.park_update(&main, update);
                None
            }
            update => Some(update),
        }
    }

    /// 正在看的会话推来的任务、别处来的话：照任务表、正文先办，交回 `true`（不再交给正文）。
    pub(super) fn visible_push(&mut self, push: &Push) -> bool {
        let Some(session) = self.transcript.session.clone() else {
            return false;
        };
        self.special(&session, push, true)
    }

    /// 停放着的会话推来的。
    fn park_update(&mut self, session: &str, update: Update) {
        match update {
            Update::Push(push) => {
                let replayed = matches!(push, Push::Clock(None));
                if !self.special(session, &push, true)
                    && let Some(parked) = self.parked.get_mut(session)
                {
                    parked
                        .transcript
                        .update(Update::Push(push), &self.config.text);
                }
                self.refresh_agent(session);
                // 切走时还忙着的：这一轮做完了、空下来就退订（「会话列表」第 4 条）。
                self.prune();
                // 切过去的补完了：换上来（第 5 条）。
                if replayed && self.opening(session) {
                    self.opened();
                }
            }
            update => {
                if let Some(parked) = self.parked.get_mut(session) {
                    parked.transcript.update(update, &self.config.text);
                }
            }
        }
    }

    /// 任务的几种、别处来的话在会话 `session` 里办了，交回 `true`；别的交回 `false`（照常交给正文）。`live` 是假的：
    /// 按页读更早的一页（`pages.rs`），只记进任务表、画进正文，不另订阅子代理的会话、不动待办、不退订。
    pub(super) fn special(&mut self, session: &str, push: &Push, live: bool) -> bool {
        let now = Instant::now();
        let words = self.config.text.jobs.clone();
        match push {
            // 前台跑的子代理不是后台任务：结果到的时候已经做完了，不进任务条（核心 T-1 下）。
            Push::JobStarted(start) | Push::JobEarlier(start) if start.foreground => {}
            Push::JobStarted(start) if live => self.started(session, start, now, &words),
            // 以前派的、页里带的跨页任务：只记进任务表，结束的那一行照它的标题写。
            Push::JobStarted(start) | Push::JobEarlier(start) => {
                let command = self.command_of(session, &start.call_id);
                if let Some((_, board)) = self.slot(session)
                    && board.by_job_mut(&start.job).is_none()
                {
                    board.start(start, command, now);
                }
            }
            Push::JobMessaged(job) => {
                if let Some((_, board)) = self.slot(session) {
                    board.messaged(job);
                }
            }
            // 以前的待办不算：现在的照订阅的回应。
            Push::Todos { .. } if !live => {}
            // 待办换了（核心 D-3）：整份换上，空的是清空了。
            Push::Todos { todos, done } => {
                let linger = std::time::Duration::from_millis(self.config.layout.todo_linger_ms);
                if let Some((_, board)) = self.slot(session) {
                    board.set_todos(todos, done, now, linger);
                }
            }
            Push::JobEnded(end) => {
                let mut command = false;
                if let Some((transcript, board)) = self.slot(session)
                    && let Some(id) = board.end(end, now)
                {
                    announce(board, transcript, id, now, &words);
                    command = job_of(board, id).is_some_and(|j| j.kind == JobKind::Shell);
                }
                // 命令结束了：读它全部的输出，正文里那一行点开看（第 5 条）。
                if command {
                    self.fetch_final(session, &end.job);
                }
                if live {
                    self.prune();
                }
            }
            Push::PeerIdle {
                session: peer,
                reason,
                status,
            } => {
                let known = self.session_title(peer);
                let (mark, text) =
                    notes::peer_note(peer, known.as_deref(), reason, status.as_deref(), &words);
                if let Some((transcript, _)) = self.slot(session) {
                    transcript.job(mark, text, status.clone().unwrap_or_default());
                }
            }
            Push::Foreign(said) => {
                let from = self.label(session, &said.from, &words);
                if let Some((transcript, _)) = self.slot(session) {
                    transcript.foreign(Some(said.seq), from, said.text.clone());
                }
            }
            _ => return false,
        }
        true
    }

    /// 派它的那一步的命令本身（照时间线里这一步的参数）；拿不到的是 `None`。
    fn command_of(&mut self, session: &str, call_id: &str) -> Option<String> {
        let (transcript, _) = self.slot(session)?;
        transcript.call_args(call_id)?["command"]
            .as_str()
            .map(str::to_string)
    }

    /// 派出去一个：记进任务表；子代理另订阅它的会话，停放一份它的正文，开头画上交代的活（「切进子会话」第 1 条）。
    pub(super) fn started(
        &mut self,
        session: &str,
        start: &JobStart,
        now: Instant,
        words: &JobTexts,
    ) {
        let args = self
            .slot(session)
            .and_then(|(t, _)| t.call_args(&start.call_id).cloned());
        let command = args
            .as_ref()
            .and_then(|a| a["command"].as_str())
            .map(str::to_string);
        let task = args
            .as_ref()
            .and_then(|a| a["prompt"].as_str())
            .unwrap_or_default()
            .to_string();
        if let Some((_, board)) = self.slot(session) {
            board.start(start, command, now);
        }
        let Some(child) = start.session.clone() else {
            return;
        };
        self.core.send(Command::Watch(child.clone()));
        // 交代的活是派它的会话发来的：来处和核心推来的那一句算得一样，才认得出是同一句（`transcript/foreign.rs`）。
        let from = self.label(&child, &Sender::Session(session.to_string()), words);
        let transcript = self.transcript.child(child.clone(), from, task);
        self.parked.insert(
            child,
            Parked {
                transcript,
                ..Parked::default()
            },
        );
    }

    /// 最近一次会话列表里这个会话的标题；没见过的顺手要一次列表（「别处来的话」第 2 条）。
    pub(super) fn session_title(&self, id: &str) -> Option<String> {
        let known = self
            .sessions_seen
            .iter()
            .flatten()
            .find(|s| s.session == id);
        if known.is_none() {
            self.core.send(Command::ListSessions);
        }
        known.and_then(|s| s.title.clone())
    }

    /// 别处来的话写的来处（「别处来的话」第 2 条）：自己派的子代理、派这个子代理的会话，别的都是别的主会话发来的
    /// （核心 C-5），照最近一次会话列表写标题，没见过的先只写短编号、顺手要一次列表。
    fn label(&self, session: &str, from: &Sender, words: &JobTexts) -> String {
        match from {
            Sender::Person | Sender::Other => words.from_person.clone(),
            Sender::Harness(name) => words.from_harness.replace("{name}", &untrusted(name)),
            Sender::Session(id) => {
                let receiver_is_child = self.agent_job(session).is_some();
                match notes::relation(self.agent_job(id), receiver_is_child) {
                    Relation::Agent(job) => words.from_agent.replace("{job}", &job),
                    Relation::Parent => words.from_main.clone(),
                    Relation::Other => {
                        notes::from_session(id, self.session_title(id).as_deref(), words)
                    }
                }
            }
        }
    }

    /// 子会话 `child` 的子代理的任务编号（在哪个会话的任务表里都找）。
    fn agent_job(&self, child: &str) -> Option<String> {
        self.boards().find_map(|b| {
            b.jobs
                .iter()
                .find(|j| j.session.as_deref() == Some(child))
                .map(|j| j.job.clone())
        })
    }

    /// 会话 `session`（正在看的、停放着的）的正文和任务表。
    pub(super) fn slot(&mut self, session: &str) -> Option<(&mut Transcript, &mut Board)> {
        if self.transcript.session.as_deref() == Some(session) {
            return Some((&mut self.transcript, &mut self.board));
        }
        self.parked
            .get_mut(session)
            .map(|p| (&mut p.transcript, &mut p.board))
    }

    /// 子会话 `child` 的子代理叫什么（在哪个会话的任务表里都找）。
    fn agent_title(&self, child: &str) -> Option<String> {
        self.boards().find_map(|b| {
            b.jobs
                .iter()
                .find(|j| j.session.as_deref() == Some(child))
                .map(|j| j.title.clone())
        })
    }

    /// 每一份任务表：正在看的、停放着的。
    fn boards(&self) -> impl Iterator<Item = &Board> {
        std::iter::once(&self.board).chain(self.parked.values().map(|p| &p.board))
    }

    /// 子会话 `child` 推来了东西：照它的正文更新它那一行正在做什么、用了多少（「后台命令、子代理和侧边栏」）。
    pub(super) fn refresh_agent(&mut self, child: &str) {
        let transcript = if self.transcript.session.as_deref() == Some(child) {
            Some(&self.transcript)
        } else {
            self.parked.get(child).map(|p| &p.transcript)
        };
        let Some((doing, tokens)) =
            transcript.map(|t| (doing(t, &self.human, &self.config.text.jobs), tokens(t)))
        else {
            return;
        };
        let boards =
            std::iter::once(&mut self.board).chain(self.parked.values_mut().map(|p| &mut p.board));
        for board in boards {
            if let Some(job) = board.agent_mut(child) {
                job.doing = doing;
                job.tokens = tokens;
                return;
            }
        }
    }

    /// `/new` 以后停放着的旧会话，任务都报完了：退订它和它的子代理，不再停放。
    pub(super) fn prune(&mut self) {
        let done: Vec<String> = self
            .left
            .iter()
            .filter(|s| {
                self.parked
                    .get(*s)
                    .is_none_or(|p| !p.board.busy() && p.transcript.running.is_none())
            })
            .cloned()
            .collect();
        for session in done {
            self.left.retain(|s| *s != session);
            if let Some(parked) = self.parked.remove(&session) {
                self.drop_children(&parked.board);
            }
            self.core.send(Command::Unwatch(session));
        }
    }

    /// 退订这份任务表里的子代理，不再停放它们的会话。
    pub(super) fn drop_children(&mut self, board: &Board) {
        for child in board.jobs.iter().filter_map(|j| j.session.clone()) {
            self.parked.remove(&child);
            self.core.send(Command::Unwatch(child));
        }
    }

    /// 子代理状态行照的那份任务表：主会话的（切进了子会话的是停放着的主会话的）。
    pub fn tree(&self) -> &Board {
        match &self.home {
            Some(main) => self.parked.get(main).map_or(&self.board, |p| &p.board),
            None => &self.board,
        }
    }

    /// 子代理状态行列的子代理：主会话任务表里在跑的，加上正在看的那个（报完了也留着，「切进子会话」第 5 条）。
    pub fn listed_agents(&self) -> Vec<&Job> {
        self.tree().listed(self.viewing())
    }

    /// 正在看的子会话的子代理叫什么（输入框右上角写，「切进子会话」第 2 条）；看着主会话是 `None`。
    pub fn viewing_title(&self) -> Option<String> {
        self.viewing().and_then(|child| self.agent_title(child))
    }

    /// 子会话 `child` 自己又派了几个在跑的子代理（状态行里写「（+N）」）。
    pub fn nested_agents(&self, child: &str) -> usize {
        if self.transcript.session.as_deref() == Some(child) {
            return self.board.agents().len();
        }
        self.parked.get(child).map_or(0, |p| p.board.agents().len())
    }

    /// 正在看的会话连同它派出去的子代理（一层层往下）一共用了多少：框下面那一行、侧边栏写（2026-09-30 项目主人：
    /// 子代理用的也算进主会话）。子代理的照它停放着的正文，订阅上以前用的看不到。
    pub fn usage_total(&self) -> Usage {
        notes::tree_usage(self.transcript.total, &self.board, &self.parked)
    }

    /// 花了多少：连同子代理的，和 [`App::usage_total`] 一个算法（蓝图「配置与模型」第 5 条）。
    pub fn bill_total(&self) -> Bill {
        notes::tree_bill(&self.transcript.bill, &self.board, &self.parked)
    }

    /// 花了多少写成字（`$0.42 + ¥1.30`，一笔都算不出的是 `None`）：侧边栏、框下面那一行用。
    pub fn spent(&self) -> Option<String> {
        let bill = self.bill_total();
        crate::money::amounts(&bill, &self.currency, &self.config.layout.currencies)
    }

    /// 正在看的子会话；看着主会话是 `None`。
    pub fn viewing(&self) -> Option<&str> {
        self.home.as_ref().and(self.transcript.session.as_deref())
    }

    /// 切进子代理状态行里第 `index` 个子代理的会话（「切进子会话」第 1 条）。
    pub(super) fn enter_agent(&mut self, index: usize) {
        let child = self
            .listed_agents()
            .get(index)
            .and_then(|j| j.session.clone());
        if let Some(child) = child {
            self.switch(child);
        }
    }

    /// 回主会话（「切进子会话」第 4 条）。
    pub(super) fn leave_child(&mut self) {
        if let Some(main) = self.home.clone() {
            self.switch(main);
        }
    }

    /// 换成看 `session`：正在看的停放起来，它从停放的里拿出来；命令跟着对着它。
    fn switch(&mut self, session: String) {
        let Some(current) = self.transcript.session.clone() else {
            return;
        };
        if current == session {
            return;
        }
        let Some(mut next) = self.parked.remove(&session) else {
            return;
        };
        next.transcript.link = self.transcript.link.clone();
        let old = Parked {
            transcript: std::mem::replace(&mut self.transcript, next.transcript),
            board: std::mem::replace(&mut self.board, next.board),
            view: std::mem::replace(&mut self.view, next.view),
        };
        self.parked.insert(current.clone(), old);
        let main = self.home.clone().unwrap_or(current);
        self.home = (session != main).then_some(main);
        let target = self.home.is_some().then_some(session);
        self.core.send(Command::View(target));
        // 行缓存照条目编号认，两份正文的编号会撞：换了就重排。
        *self.row_cache.borrow_mut() = Default::default();
        self.panel = None;
    }
}

/// 界面里的编号对上的那一个（停掉用）。
pub(super) fn job_of(board: &Board, id: u64) -> Option<&Job> {
    board.jobs.iter().find(|j| j.id == id)
}
