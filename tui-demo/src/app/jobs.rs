//! 后台命令、子代理、待办（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：演示命令起假的、到点推一步、
//! 结束的在正文里出通知；焦点往下走、后台面板、待办面板的按键和鼠标。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::App;
use crate::commands::Run;
use crate::core::Command;
use crate::focus::{Button, Focus, Stops};
use crate::jobs::PanelItem;

/// 开着的面板：和命令列表同一个位置（第 3 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// 后台面板：选中第几行、点开了哪一条（编号）、结束了的收起来的展开了没有。
    Background {
        /// 选中第几行（[`Board::panel_items`] 的第几个）。
        selected: usize,
        /// 点开的那一条的编号。
        open: Option<u64>,
        /// 结束了的多于一条时收起来的，展开了。
        all: bool,
    },
    /// 帮助（`/help`）：往下滚了几行。
    Help {
        /// 往下滚了几行。
        scroll: usize,
    },
    /// 界面语言（`/language`）：选中第几种。
    Language {
        /// 选中语言表里的第几种。
        selected: usize,
    },
    /// 会话列表（`/sessions`）：列表的状态在 [`App::session_list`](super::App) 上。
    Sessions,
    /// 换模型（`/model`）：框的状态在 [`App::model_list`](super::App) 上。
    Models,
    /// 思考强度（`/effort`）：选中第几行（第 0 行默认）；几级在 [`App::efforts`](super::App) 上。
    Effort {
        /// 选中第几行。
        selected: usize,
    },
}

impl App {
    /// 演示命令：推一份假的待办（后台命令、子代理已经接核心）。
    pub(super) fn demo(&mut self, run: Run) {
        if run == Run::DemoTodo {
            let now = Instant::now();
            self.feed
                .start_todo(&self.config.fake, &mut self.board, now);
        }
    }

    /// 到点的推一步（假的待办）。
    pub(super) fn advance_jobs(&mut self) {
        self.feed
            .advance(&self.config.fake, &mut self.board, Instant::now());
        self.poll_output();
    }

    /// 下一次该推、该重画的时刻：假待办的下一步；有在跑的后台任务，每秒重画一次（用时在走）。
    pub(super) fn jobs_deadline(&self) -> Option<Instant> {
        let running = self.board.busy() || self.tree().busy();
        let tick = running.then(|| Instant::now() + Duration::from_secs(1));
        self.feed.next_at().into_iter().chain(tick).min()
    }

    /// 停掉一条在跑的后台任务：交给核心（`job.stop`），结束的回报来了再出通知。
    fn stop_job(&mut self, id: u64) {
        if let Some(job) = super::sessions::job_of(&self.board, id).filter(|j| j.running()) {
            self.core.send(Command::Stop(job.job.clone()));
        }
    }

    /// 框下面那一行现在有哪些按钮（从左到右）：有在跑的命令时的后台按钮。
    pub fn footer_buttons(&self) -> Vec<Button> {
        (self.board.shells_running() > 0)
            .then_some(Button::Background)
            .into_iter()
            .collect()
    }

    /// 子代理状态行里焦点能停的几行：主会话那一行加上列着的子代理（`sessions.rs` 的 `listed_agents`）；
    /// 没有列着的子代理，这一块不在，一行都没有。
    fn agent_stops(&self) -> usize {
        match self
            .listed_agents()
            .len()
            .min(self.config.layout.agent_rows)
        {
            0 => 0,
            n => n + 1,
        }
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
                all: false,
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
            // 第 0 行回主会话，别的切进那个子代理的会话（「切进子会话」第 1、4 条）；焦点留在这一行
            // （2026-09-30 项目主人：原来切完回输入框）。
            (Focus::Agent(0), KeyCode::Enter) => {
                self.leave_child();
                focus
            }
            (Focus::Agent(i), KeyCode::Enter) => {
                self.enter_agent(i - 1);
                focus
            }
            // 选中的子代理按 x：停掉它（主会话那一行、报完了的不管）。
            (Focus::Agent(i), KeyCode::Char('x')) if i > 0 => {
                let listed = self.listed_agents();
                let job = listed
                    .get(i - 1)
                    .filter(|j| j.running())
                    .map(|j| j.job.clone());
                if let Some(job) = job {
                    self.core.send(Command::Stop(job));
                }
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
        let (selected, open, all) = match panel {
            Panel::Background {
                selected,
                open,
                all,
            } => (selected, open, all),
            Panel::Help { scroll } => return self.help_key(scroll, key),
            Panel::Language { selected } => return self.language_key(selected, key),
            Panel::Sessions => return self.sessions_key(key),
            Panel::Models => return self.models_key(key),
            Panel::Effort { selected } => return self.effort_key(selected, key),
        };
        let items = self.board.panel_items(all);
        let at = |selected| {
            Some(Panel::Background {
                selected,
                open,
                all,
            })
        };
        match key.code {
            KeyCode::Up => self.panel = at(selected.saturating_sub(1)),
            KeyCode::Down => self.panel = at((selected + 1).min(items.len().saturating_sub(1))),
            KeyCode::Enter => self.pick_item(selected),
            KeyCode::Char('x') => {
                if let Some(PanelItem::Job(id)) = items.get(selected) {
                    self.stop_job(*id);
                }
            }
            KeyCode::Esc => self.panel = None,
            _ => {}
        }
    }

    /// 后台面板里选了第几行：一条命令点开、收起（一次只开一条，点开时马上读一次输出）；收起来的那一行展开，展开以后
    /// 最后那一行收起，选中的跟着它走。
    fn pick_item(&mut self, index: usize) {
        let Some(Panel::Background { open, all, .. }) = self.panel else {
            return;
        };
        let item = self.board.panel_items(all).get(index).copied();
        let (open, all) = match item {
            Some(PanelItem::Job(id)) => ((open != Some(id)).then_some(id), all),
            Some(PanelItem::More(_) | PanelItem::Less) => (open, !all),
            None => return,
        };
        let selected = match item {
            Some(PanelItem::Job(_)) => index,
            _ => self.board.panel_items(all).len().saturating_sub(1),
        };
        self.panel = Some(Panel::Background {
            selected,
            open,
            all,
        });
        self.poll_output();
    }

    /// 鼠标先归框下面那一行的按钮、面板、子代理状态行；归了它们返回 `true`。
    pub(super) fn jobs_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        if self.help_mouse(mouse, at)
            || self.language_mouse(mouse, at)
            || self.sessions_mouse(mouse, at)
            || self.models_mouse(mouse, at)
            || self.effort_mouse(mouse, at)
        {
            return true;
        }
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
            // 第 0 行是空行，第 1 行是主会话，子代理从第 2 行起：点了切过去（「切进子会话」第 1、4 条）。
            if press && row == 1 {
                self.leave_child();
            } else if press && row >= 2 {
                self.enter_agent(row - 2);
            }
            return true;
        }
        if let Some(Panel::Background { open, all, .. }) = self.panel
            && areas.menu.contains(at)
        {
            // 点在框的边上不算点中哪一条（「斜杠命令列表」第 3 条）。
            let row = at.y.checked_sub(areas.menu_text.y).map(usize::from);
            let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
            if let Some(index) = index.filter(|_| areas.menu_text.contains(at)) {
                self.panel = Some(Panel::Background {
                    selected: index,
                    open,
                    all,
                });
                if press {
                    self.pick_item(index);
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

    /// 鼠标该不该是手：悬停在链接、输入框里的附件块、框下面那一行的按钮、子代理那几行上。
    pub fn pointing(&self) -> bool {
        let over = |r: ratatui::layout::Rect| self.pointer.is_some_and(|p| r.contains(p));
        // 正文里能点的（收起那一行、一步、撤销那一行、能点开的通知）也算（`tui.md`「鼠标」）。
        self.view.hover_link.is_some()
            || self.view.hover.is_some()
            || self.input.hovered_attachment().is_some()
            || over(self.areas.button)
            || over(self.areas.session_id)
            || over(self.areas.todo)
            || over(self.areas.bottom)
            || self.agents_hover.is_some_and(|r| r >= 2)
    }
}
