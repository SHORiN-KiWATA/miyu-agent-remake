//! 后台命令、子代理、待办（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：演示命令起假的、到点推一步、
//! 结束的在正文里出通知；焦点往下走、后台面板、待办面板的按键和鼠标。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::App;
use crate::commands::Run;
use crate::focus::{Button, Focus, Stops};
use crate::jobs::{JobKind, JobState};
use crate::meter;
use crate::transcript::JobMark;

/// 开着的面板：和命令列表同一个位置（第 3 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// 后台面板：选中第几条、点开了哪一条（编号）。
    Background {
        /// 选中第几条。
        selected: usize,
        /// 点开的那一条的编号。
        open: Option<u64>,
    },
}

impl App {
    /// 演示命令：起一条假的后台命令、派一个假的子代理、推一份假的待办。
    pub(super) fn demo(&mut self, run: Run) {
        let now = Instant::now();
        let (script, board) = (&self.config.fake, &mut self.board);
        match run {
            Run::DemoShell => self.feed.start_shell(script, board, now),
            Run::DemoAgent => self.feed.start_agent(script, board, now),
            Run::DemoTodo => self.feed.start_todo(script, board, now),
            _ => {}
        }
    }

    /// 到点的推一步；结束了的出通知。
    pub(super) fn advance_jobs(&mut self) {
        let now = Instant::now();
        let finished = self.feed.advance(&self.config.fake, &mut self.board, now);
        for id in finished {
            self.announce(id, now);
        }
    }

    /// 下一次该推、该重画的时刻：假数据源的下一步；有在跑的，每秒重画一次（用时在走）。
    pub(super) fn jobs_deadline(&self) -> Option<Instant> {
        let running = self.board.jobs.iter().any(|j| j.running());
        let tick = running.then(|| Instant::now() + Duration::from_secs(1));
        self.feed.next_at().into_iter().chain(tick).min()
    }

    /// 停掉一条在跑的后台任务，出一条通知。
    fn stop_job(&mut self, id: u64) {
        let now = Instant::now();
        if self.board.stop(id, now) {
            self.announce(id, now);
        }
    }

    /// 一条后台任务结束了：正文末尾一行，记号有颜色、字是原色；子代理的报告能点开看（第 5 条）。
    fn announce(&mut self, id: u64, now: Instant) {
        let Some(job) = self.board.jobs.iter().find(|j| j.id == id) else {
            return;
        };
        let words = &self.config.text.jobs;
        let elapsed = meter::clock(job.elapsed(now).as_secs());
        let (mark, text) = match (job.kind, job.state) {
            (JobKind::Agent, _) => (JobMark::Done, words.agent_note.clone()),
            (_, JobState::Failed(code)) => (
                JobMark::Failed,
                words.failed_note.replace("{code}", &code.to_string()),
            ),
            (_, JobState::Stopped) => (JobMark::Stopped, words.stopped_note.clone()),
            _ => (JobMark::Done, words.done_note.clone()),
        };
        let text = text
            .replace("{title}", &job.title)
            .replace("{elapsed}", &elapsed);
        // 点开看的全文：子代理是报告，命令是它的输出（第 5 条）。
        let detail = match job.kind {
            JobKind::Agent => job.report.clone(),
            JobKind::Shell => job.output.join("\n"),
        };
        self.transcript.job(mark, text, detail);
    }

    /// 框下面那一行现在有哪些按钮（从左到右）：有在跑的命令时的后台按钮。
    pub fn footer_buttons(&self) -> Vec<Button> {
        (self.board.shells_running() > 0)
            .then_some(Button::Background)
            .into_iter()
            .collect()
    }

    /// 子代理状态行里的子代理有几行。
    fn agent_stops(&self) -> usize {
        self.board.agents().len().min(self.config.layout.agent_rows)
    }

    /// 焦点现在在哪：按钮、行没了就退回（第 2 条）。
    pub fn focus(&self) -> Focus {
        let buttons = self.footer_buttons();
        let stops = Stops {
            buttons: &buttons,
            agents: self.agent_stops(),
        };
        self.focus.settle(stops)
    }

    /// 打开按钮的面板。
    fn open(&mut self, button: Button) {
        self.panel = Some(match button {
            Button::Background => Panel::Background {
                selected: 0,
                open: None,
            },
        });
    }

    /// 按键先归面板、框下面那一行的按钮、子代理状态行（第 2–6 条）；归了它们返回 `true`。
    /// 输入框空着时按 `↓`，焦点往下走。
    pub(super) fn jobs_key(&mut self, key: KeyEvent, menu_open: bool) -> bool {
        if let Some(panel) = self.panel {
            self.panel_key(panel, key);
            return true;
        }
        let buttons = self.footer_buttons();
        let stops = Stops {
            buttons: &buttons,
            agents: self.agent_stops(),
        };
        let focus = self.focus.settle(stops);
        self.focus = match (focus, key.code) {
            (Focus::Input, KeyCode::Down) if self.input.editor.is_empty() && !menu_open => {
                focus.down(stops)
            }
            (Focus::Input, _) => return false,
            (_, KeyCode::Down) => focus.down(stops),
            (_, KeyCode::Up) => focus.up(stops),
            (_, KeyCode::Left) => focus.side(stops, false),
            (_, KeyCode::Right) => focus.side(stops, true),
            (_, KeyCode::Esc) => Focus::Input,
            (Focus::Footer(b), KeyCode::Enter) => {
                self.open(b);
                focus
            }
            (Focus::Agent(_), KeyCode::Enter) => {
                let text = self.config.text.jobs.switch_todo.clone();
                self.hint(text, false);
                focus
            }
            // 别的键：焦点回输入框，字照打进去。
            _ => {
                self.focus = Focus::Input;
                return false;
            }
        };
        focus != Focus::Input || self.focus != Focus::Input
    }

    /// 面板开着时的按键。
    fn panel_key(&mut self, panel: Panel, key: KeyEvent) {
        let Panel::Background { selected, open } = panel;
        let count = self.board.shells().len();
        let at = |selected| Some(Panel::Background { selected, open });
        match key.code {
            KeyCode::Up => self.panel = at(selected.saturating_sub(1)),
            KeyCode::Down => self.panel = at((selected + 1).min(count.saturating_sub(1))),
            KeyCode::Enter => self.toggle_job(selected),
            KeyCode::Char('x') => {
                if let Some(id) = self.board.shells().get(selected).map(|j| j.id) {
                    self.stop_job(id);
                }
            }
            KeyCode::Esc => self.panel = None,
            _ => {}
        }
    }

    /// 后台面板里点开、收起第几条（一次只开一条）。
    fn toggle_job(&mut self, index: usize) {
        let id = self.board.shells().get(index).map(|j| j.id);
        if let Some(Panel::Background { open, .. }) = self.panel {
            let open = if open == id { None } else { id };
            self.panel = Some(Panel::Background {
                selected: index,
                open,
            });
        }
    }

    /// 鼠标先归框下面那一行的按钮、面板、子代理状态行；归了它们返回 `true`。
    pub(super) fn jobs_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        let press = matches!(mouse.kind, MouseEventKind::Down(_));
        let areas = self.areas;
        if self.sidebar_mouse(mouse, at) {
            return true;
        }
        self.agents_hover = areas
            .agents
            .contains(at)
            .then(|| usize::from(at.y - areas.agents.y));
        // 侧边栏的短编号：点了复制完整的会话编号（第 7 条）。
        if areas.session_id.contains(at) {
            if press && let Some(session) = self.transcript.session.clone() {
                self.copy(&session);
            }
            return true;
        }
        // 待办那一块：点了展开全部，再点收起（第 4 条）。
        if areas.todo.contains(at) {
            if press {
                self.todo_full = !self.todo_full;
            }
            return true;
        }
        for (rect, button) in [(areas.button, Button::Background)] {
            if rect.contains(at) {
                if press {
                    self.open(button);
                }
                return true;
            }
        }
        if let Some(row) = self.agents_hover {
            // 第 0 行是空行，第 1 行是主会话，子代理从第 2 行起。
            if press && row >= 2 {
                self.focus = Focus::Agent(row - 2);
                let text = self.config.text.jobs.switch_todo.clone();
                self.hint(text, false);
            }
            return true;
        }
        if let Some(Panel::Background { open, .. }) = self.panel
            && areas.menu.contains(at)
        {
            // 点在框的边上不算点中哪一条（「斜杠命令列表」第 3 条）。
            let row = at.y.checked_sub(areas.menu_text.y).map(usize::from);
            let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
            if let Some(index) = index.filter(|_| areas.menu_text.contains(at)) {
                self.panel = Some(Panel::Background {
                    selected: index,
                    open,
                });
                if press {
                    self.toggle_job(index);
                }
            }
            return true;
        }
        false
    }

    /// 侧边栏里按下、拖、松开：拖了是选字，只点不拖的照点的来（点编号复制完整编号、点待办展开收起）；
    /// 在别处按下，侧边栏的选区作废（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
    fn sidebar_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        let areas = self.areas;
        let inside = areas.sidebar.contains(at);
        match mouse.kind {
            MouseEventKind::Down(_) if inside => {
                self.side_select.press(at.x, at.y);
                true
            }
            MouseEventKind::Down(_) => {
                self.side_select.clear();
                false
            }
            MouseEventKind::Drag(_) if self.side_select.pressing() => {
                self.side_select.drag(at.x, at.y);
                true
            }
            MouseEventKind::Up(_) if self.side_select.pressing() => {
                if self.side_select.release() {
                    if areas.session_id.contains(at)
                        && let Some(session) = self.transcript.session.clone()
                    {
                        self.copy(&session);
                    } else if areas.todo.contains(at) {
                        self.todo_full = !self.todo_full;
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// 鼠标该不该是手：悬停在链接、框下面那一行的按钮、子代理那几行上。
    pub fn pointing(&self) -> bool {
        let over = |r: ratatui::layout::Rect| self.pointer.is_some_and(|p| r.contains(p));
        // 正文里能点的（收起那一行、一步、撤销那一行、能点开的通知）也算（`tui.md`「鼠标」）。
        self.view.hover_link.is_some()
            || self.view.hover.is_some()
            || over(self.areas.button)
            || over(self.areas.session_id)
            || over(self.areas.todo)
            || self.agents_hover.is_some_and(|r| r >= 2)
    }
}
