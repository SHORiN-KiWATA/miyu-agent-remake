//! 老会话按页读，界面这一头（蓝图 `tui.md`「会话列表 `/sessions`」第 5 条「按页读」，核心 9-6）：订阅回应里的累计
//! 用量、权限、还在跑的照会话换上；最上面那一行「正在读更早的…」露进视口就要更早的一页；更早的一页在一份临时的正文和
//! 任务表里照补发的读一遍，拼在前面，视口钉在原来看的那一行。

use std::collections::HashMap;
use std::time::Instant;

use super::App;
use super::drawer::report_into;
use crate::body_view::BodyView;
use crate::core::{Asking, Command, Push, Snapshot, Update};
use crate::drawer::{Answered, Approval, Asked, Decided, Drawer, Outcome};
use crate::jobs::Board;
use crate::transcript::Transcript;

impl App {
    /// 按页读的几样：订阅回应的三格、更早的一页。办了的交回 `None`，别的原样交回。
    pub(super) fn page_update(&mut self, update: Update) -> Option<Update> {
        match update {
            Update::Elsewhere { session, update } => match *update {
                Update::Snapshot(snapshot) => {
                    self.snapshot(&session, snapshot);
                    None
                }
                Update::Paged(more) => {
                    self.paged(&session, more);
                    None
                }
                Update::Workspace { cwd, joined } => {
                    self.workspace(&session, cwd, joined);
                    None
                }
                Update::Older {
                    pushes,
                    more,
                    failed,
                } => {
                    self.older(&session, pushes, more, failed);
                    None
                }
                update => Some(Update::Elsewhere {
                    session,
                    update: Box::new(update),
                }),
            },
            // 开会话、重连时订阅主会话的回应（`core/serve.rs`、`core/spawn.rs`）：工作区。
            Update::Workspace { cwd, joined } => {
                if let Some(main) = self.main_session() {
                    self.workspace(&main, cwd, joined);
                }
                None
            }
            // 重连时重新订阅主会话的回应（`core/spawn.rs`）：断开那一段漏掉的用量补回来。
            Update::Snapshot(snapshot) => {
                if let Some(main) = self.main_session() {
                    self.snapshot(&main, snapshot);
                }
                None
            }
            update => Some(update),
        }
    }

    /// 每一帧：正在看的会话最上面那一行露进了视口（上一帧画过、第一行就是它），要更早的一页。
    pub(super) fn ask_older(&mut self) {
        let Some(session) = self.transcript.session.clone() else {
            return;
        };
        let shown = !self.view.rows.is_empty() && self.view.first == 0;
        if shown && self.transcript.wants_older() {
            self.transcript.asked_older();
            self.core.send(Command::Older(session));
        }
    }

    /// 最新一页读进来了：更早的还有的，最上面那一行在。起标题、改名的事件可能在更早的页里：页里没给标题的，照会话列表里
    /// 它的标题（核心 9-5 推着，一直是新的）。
    fn paged(&mut self, session: &str, more: bool) {
        let listed = self.session_title(session);
        let texts = &self.config.text;
        let found = place(
            &mut self.transcript,
            &mut self.board,
            &mut self.view,
            &mut self.parked,
            session,
        );
        if let Some((transcript, ..)) = found {
            transcript.paged(more, texts);
            if transcript.title.is_none() {
                transcript.title = listed;
            }
        }
    }

    /// 订阅回应里会话的工作区（核心 9-7 上）：记下，接上老会话时终端在别的目录的写一句（`transcript/workspace.rs`）。
    fn workspace(&mut self, session: &str, cwd: String, joined: bool) {
        let texts = &self.config.text;
        let here = &self.cwd;
        let found = place(
            &mut self.transcript,
            &mut self.board,
            &mut self.view,
            &mut self.parked,
            session,
        );
        if let Some((transcript, ..)) = found {
            transcript.joined_workspace(cwd, joined, here, texts);
        }
    }

    /// 订阅回应里的三格：任务表里还没有的在跑的记上（子代理照派出去的办），累计用量、权限换上。
    fn snapshot(&mut self, session: &str, snapshot: Snapshot) {
        let now = Instant::now();
        let words = self.config.text.jobs.clone();
        for job in snapshot.jobs.iter().flatten() {
            let known = self
                .slot(session)
                .is_some_and(|(_, board)| board.by_job_mut(&job.job).is_some());
            if !known {
                self.started(session, job, now, &words);
            }
        }
        if let Some((transcript, _)) = self.slot(session) {
            transcript.snapshot(snapshot.usage.as_ref(), snapshot.level);
        }
    }

    /// 更早的一页到了：换一份临时的正文和任务表进去读一遍，再换回来、拼在前面。读不成的去掉最上面那一行、弹原因。
    fn older(&mut self, session: &str, pushes: Vec<Push>, more: bool, failed: Option<String>) {
        if let Some(reason) = failed {
            let hint = self.config.text.older_failed.replace("{reason}", &reason);
            self.hint(hint, false);
        }
        let Some((transcript, board, _)) = self.place(session) else {
            return;
        };
        let temp = transcript.history();
        let newer = std::mem::replace(transcript, temp);
        let newer_board = std::mem::take(board);
        let mut asked = HashMap::new();
        for push in pushes {
            self.history_push(session, push, &mut asked);
        }
        let texts = self.config.text.clone();
        let Some((transcript, board, view)) = self.place(session) else {
            return;
        };
        let older = std::mem::replace(transcript, newer);
        let older_board = std::mem::replace(board, newer_board);
        board.prepend(older_board);
        view.pin_top(&transcript.entries);
        transcript.prepend(older, more, &texts);
    }

    /// 更早的一页里的一条：确认、提问不开抽屉，答了的照了结写一行；任务的几种只记进任务表；别的交给那份临时的正文。
    fn history_push(&mut self, session: &str, push: Push, asked: &mut HashMap<String, Drawer>) {
        if let Push::Asking(asking) = &push {
            self.history_asking(session, asking, asked);
            return;
        }
        if self.special(session, &push, false) {
            return;
        }
        let texts = &self.config.text;
        if let Some((transcript, ..)) = place(
            &mut self.transcript,
            &mut self.board,
            &mut self.view,
            &mut self.parked,
            session,
        ) {
            transcript.update(Update::Push(push), texts);
        }
    }

    /// 以前的确认、提问：问的记着，答了的在正文里写结果（「确认和提问的抽屉」第 6 条）。
    fn history_asking(
        &mut self,
        session: &str,
        asking: &Asking,
        asked: &mut HashMap<String, Drawer>,
    ) {
        let outcome = match asking {
            Asking::Asked(body) => {
                if let Ok(a) = serde_json::from_value::<Asked>(body.clone()) {
                    let drawer = Drawer::question(None, a);
                    asked.insert(drawer.call_id.clone(), drawer);
                }
                return;
            }
            Asking::Approval(body) => {
                if let Ok(a) = serde_json::from_value::<Approval>(body.clone()) {
                    let drawer = Drawer::approval(None, a);
                    asked.insert(drawer.call_id.clone(), drawer);
                }
                return;
            }
            Asking::Answered(body) => serde_json::from_value::<Answered>(body.clone())
                .ok()
                .map(|a| (a.call_id.clone(), Outcome::Answered(a))),
            Asking::Decided(body) => serde_json::from_value::<Decided>(body.clone())
                .ok()
                .map(|d| (d.call_id.clone(), Outcome::Decided(d))),
        };
        let Some((call, outcome)) = outcome else {
            return;
        };
        let Some(drawer) = asked.remove(&call) else {
            return;
        };
        let texts = &self.config.text.drawer;
        if let Some((transcript, ..)) = place(
            &mut self.transcript,
            &mut self.board,
            &mut self.view,
            &mut self.parked,
            session,
        ) {
            report_into(transcript, &drawer, &outcome, texts);
        }
    }

    /// 会话 `session`（正在看的、停放着的）的正文、任务表和视口。
    fn place(&mut self, session: &str) -> Option<(&mut Transcript, &mut Board, &mut BodyView)> {
        place(
            &mut self.transcript,
            &mut self.board,
            &mut self.view,
            &mut self.parked,
            session,
        )
    }
}

/// 照会话找正文、任务表和视口：正在看的是外面那一份，别的在停放着的里面。拆成几格传，借用不和别的字段打架。
fn place<'a>(
    transcript: &'a mut Transcript,
    board: &'a mut Board,
    view: &'a mut BodyView,
    parked: &'a mut super::sessions::Lot,
    session: &str,
) -> Option<(&'a mut Transcript, &'a mut Board, &'a mut BodyView)> {
    if transcript.session.as_deref() == Some(session) {
        return Some((transcript, board, view));
    }
    parked
        .get_mut(session)
        .map(|p| (&mut p.transcript, &mut p.board, &mut p.view))
}
