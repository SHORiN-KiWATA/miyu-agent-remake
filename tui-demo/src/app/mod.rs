//! 程序的状态，和把终端事件、核心的消息分给各块。按键在 `keys.rs`，鼠标在 `mouse.rs`。

mod drawer;
mod jobs;

pub use jobs::Panel;
mod keys;
mod mouse;

use std::cell::RefCell;
use std::time::{Duration, Instant};

use miyu_store::human::Human;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::Position;

use crate::body_view::BodyView;
use crate::clipboard;
use crate::commands::{self, Spec};
use crate::config::Config;
use crate::core::{Block, Command, Core, Push, Update};
use crate::drawer::Drawers;
use crate::figures::Figures;
use crate::focus::Focus;
use crate::history::History;
use crate::input::{Action, Draft, InputBox, PasteRule};
use crate::jobs::{Board, Feed};
use crate::mascot::{Gaze, Idle, Perch};
use crate::menu::Menu;
use crate::pulse::Pulse;
use crate::side_select::SideSelect;
use crate::tips::Tips;
use crate::transcript::{Kind, Transcript};
use crate::ui::Areas;
use crate::ui::row_cache::RowCache;
use crate::ui::rows::MdCache;

/// 按下鼠标时落在哪一块：拖动、松开都归它，拖出了那一块也一样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grab {
    Menu,
    Input,
    Body,
}

/// 一条会自己消失的提示。
#[derive(Debug)]
pub struct Notice {
    /// 显示的字。
    pub text: String,
    /// 什么时候消失。
    pub until: Instant,
    /// 是好消息（复制成了）：绿；别的提示暗。
    pub good: bool,
}

/// 整个程序的状态。
pub struct App {
    /// 界面上的字和布局的数值。
    pub config: Config,
    /// 输入框。
    pub input: InputBox,
    /// 会话：正文、在不在跑、用量。
    pub transcript: Transcript,
    /// 连着核心的一头。
    core: Core,
    /// 斜杠命令列表。
    pub menu: Menu,
    /// 输入历史列表（`Ctrl+R`）。
    pub history: History,
    /// 运行状态行正在写的词。
    pub pulse: Pulse,
    /// 首页的吉祥物朝哪看（`tui.md`「空会话的首页」第 6、7 条）。
    pub gaze: Gaze,
    /// 首页吉祥物的待机小动作。
    pub idle: Idle,
    /// 工作目录，家目录写成 `~`：侧边栏照它写。
    pub cwd: String,
    /// 输入框空着时写哪条提示。
    pub tips: Tips,
    /// 侧边栏的选字（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
    pub side_select: SideSelect,
    /// 后台任务表和待办（现在由假数据源推）。
    pub board: Board,
    /// 演示用的假数据源。
    feed: Feed,
    /// 开着的面板（后台）。
    pub panel: Option<Panel>,
    /// 确认和提问的抽屉：现在这一个和排着的（蓝图「确认和提问的抽屉」）。
    pub drawers: Drawers,
    /// 抽屉每一行是第几项（点哪一行点中哪一项）；上一帧排出来的。
    pub drawer_rows: Vec<Option<usize>>,
    /// `/demo-ask`、`/demo-approve` 各出到第几个。
    demo_drawers: (usize, usize),
    /// 待办点开了，列出全部（`tui.md`「后台命令、子代理和侧边栏」第 4 条）。
    pub todo_full: bool,
    /// 后台面板每一行是第几条命令（点哪一行点中哪一条）；上一帧排出来的。
    pub panel_rows: Vec<Option<usize>>,
    /// 焦点在哪：输入框、框下面那一行的按钮、子代理状态行。
    focus: Focus,
    /// 鼠标悬停在子代理状态行的第几行（第 0 行是上面的空行）。
    pub agents_hover: Option<usize>,
    /// 首页吉祥物被列表顶上去以后待在哪。
    pub perch: Perch,
    /// 鼠标最后在哪一格；还没动过是 `None`。
    pub pointer: Option<Position>,
    /// 框下面那一行的临时提示。
    pub notice: Option<Notice>,
    /// 上一帧各块的位置。
    pub areas: Areas,
    /// 正文区：滚到哪、悬在哪、选了哪。
    pub view: BodyView,
    /// 工具给人看的显示名（仓库 `resources/software/basesystem/human/`）。
    pub human: Human,
    /// 回答排好的行的缓存（`ui/rows.rs`）。
    pub md_cache: RefCell<MdCache>,
    /// 正文按条缓存排好的行（`ui/row_cache`）。
    pub row_cache: RefCell<RowCache>,
    /// 正文里做好的图（蓝图「图片、公式和 mermaid 图」）。
    pub figures: RefCell<Figures>,
    /// 程序启动的时刻：转圈照它算第几帧。
    pub started: Instant,
    /// 按着鼠标时是哪一块的。
    grab: Option<Grab>,
    /// 当前主题的名字。
    theme: String,
    /// 在回答时第一下 `Esc` 的时刻：时限里再按一下才打断。
    esc_at: Option<Instant>,
    /// 该退出了。
    pub quit: bool,
    /// 按了 Ctrl+Z，主循环该把程序挂起到后台了。
    pub suspend: bool,
}

impl App {
    /// 刚启动时的样子：输入框空着，正文空着，核心在连。
    pub fn new(config: Config, core: Core, human: Human, figures: Figures) -> Self {
        // 照配置设主题；没有这一套的用出厂的第一套。
        let chosen = config
            .themes
            .iter()
            .find(|(name, _)| *name == config.layout.theme)
            .or(config.themes.first())
            .cloned();
        let theme = chosen.map_or_else(String::new, |(name, palette)| {
            crate::theme::set(palette);
            name
        });
        let config_tips = config.text.tips.len();
        let layout = &config.layout;
        let mut input = InputBox::new(
            layout.max_rows,
            Duration::from_millis(layout.double_click_ms),
        );
        input.set_paste_rule(PasteRule {
            lines: layout.paste_fold_lines,
            chars: layout.paste_fold_chars,
            label: config.text.paste_label.clone(),
        });
        Self {
            config,
            input,
            transcript: Transcript::default(),
            core,
            menu: Menu::default(),
            history: History::default(),
            // 挑词的随机数照启动的时刻起头，每次启动不一样。
            pulse: Pulse::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64),
            ),
            gaze: Gaze::default(),
            side_select: SideSelect::default(),
            cwd: std::env::current_dir()
                .map(|d| crate::local::home_short(&d.display().to_string()))
                .unwrap_or_default(),
            tips: Tips::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64 >> 3),
                config_tips,
            ),
            board: Board::default(),
            feed: Feed::default(),
            panel: None,
            drawers: Drawers::default(),
            drawer_rows: Vec::new(),
            demo_drawers: (0, 0),
            todo_full: false,
            panel_rows: Vec::new(),
            focus: Focus::Input,
            agents_hover: None,
            perch: Perch::default(),
            idle: Idle::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64),
            ),
            pointer: None,
            notice: None,
            areas: Areas::default(),
            view: BodyView::default(),
            human,
            md_cache: RefCell::new(MdCache::new()),
            row_cache: RefCell::new(RowCache::default()),
            figures: RefCell::new(figures),
            started: Instant::now(),
            grab: None,
            theme,
            esc_at: None,
            quit: false,
            suspend: false,
        }
    }

    /// 这一下 Esc 归输入框（打断、清空）：列表（命令列表、输入历史列表）、后台面板、抽屉开着的，焦点在别处的，
    /// 有选区的，都先归它们（`13-终端界面.md` 第十节「由近及远」，`tui.md`「按键」Esc）。
    fn esc_for_input(&self, menu_open: bool) -> bool {
        !menu_open
            && !self.history.open
            && self.panel.is_none()
            && self.focus == Focus::Input
            && !self.drawers.open()
            && self.input.editor.selection().is_none()
            && self.view.select.is_none()
    }

    /// 处理一个终端事件。
    pub fn handle(&mut self, event: Event) {
        // 有人按键、动鼠标、粘贴：吉祥物停下待机的晃（`tui.md`「空会话的首页」第 8 条）。
        if matches!(event, Event::Key(_) | Event::Mouse(_) | Event::Paste(_)) {
            self.idle.poke(Instant::now());
        }
        let menu_open = self.menu_matches().is_some();
        let action = match event {
            // Windows 上松开键也报一次，只认按下和按住。
            Event::Key(key) if key.kind == KeyEventKind::Release => Action::None,
            // Esc 由近及远（`13-终端界面.md` 第十节）：列表、选区归 `key`；在回答时第一下只提示，
            // 一小会儿以内再按一下才打断；没在回答、有字时，两下清空。
            Event::Key(key)
                if key.code == KeyCode::Esc
                    && self.esc_for_input(menu_open)
                    && (self.transcript.running.is_some() || !self.input.editor.is_empty()) =>
            {
                self.esc();
                Action::None
            }
            Event::Key(key) => self.key(key),
            Event::Paste(text) => {
                self.paste(&text);
                Action::None
            }
            Event::Mouse(mouse) => self.mouse(mouse),
            _ => Action::None,
        };
        match action {
            Action::None => {}
            Action::Submit(text) => self.submit(text),
            Action::Copy(text) => self.copy(&text),
            Action::Quit => self.quit = true,
            // Ctrl+C 分级：输入框空着时，在回答就打断（排着队的退回输入框），不在回答才提示用 Ctrl+D 退出。
            Action::ExitHint if self.transcript.running.is_some() => {
                self.core.send(Command::Interrupt { send: false });
            }
            Action::ExitHint => self.hint(self.config.text.exit_hint.clone(), false),
        }
    }

    /// 粘贴进来的字（终端送来的、`Ctrl+V` 读剪贴板的）：历史列表开着的进「历史：」，抽屉开着的进抽屉里写的字，
    /// 别的进输入框（大段的收成一块，`tui.md`「输入框」第 11 条）。
    fn paste(&mut self, text: &str) {
        if self.history.open {
            self.history.type_text(&text.replace(['\r', '\n'], " "));
        } else if self.drawers.open() {
            self.drawer_paste(text);
        } else {
            self.input.paste(text);
        }
    }

    /// `Ctrl+V`：读系统剪贴板里的字，照粘贴处理；读不到、是空的，提示一句（`tui.md`「按键」）。
    fn paste_clipboard(&mut self) {
        match clipboard::read() {
            Ok(text) if text.is_empty() => {
                self.hint(self.config.text.clipboard_empty.clone(), false);
            }
            Ok(text) => self.paste(&text),
            Err(_) => self.hint(self.config.text.clipboard_unreadable.clone(), false),
        }
    }

    /// 在输入框左上方写一条提示，新的顶掉旧的。
    fn hint(&mut self, text: String, good: bool) {
        self.notice = Some(Notice {
            text,
            until: Instant::now() + Duration::from_millis(self.config.layout.notice_ms),
            good,
        });
    }

    /// 收一条核心那边的消息。撤销成了、排队的消息被退回了，字放回输入框（`tui.md`「输入框」第 7、8 条）。
    pub fn core(&mut self, update: Update) {
        let undone = matches!(update, Update::Undone { restore: false, .. });
        if matches!(update, Update::Undone { restore: true, .. }) {
            self.input.take_back();
        }
        // 限制了进行中那一段的高度就不放开视口（`tui.md`「正文」第 1 条、「时间线」第 20 条）。
        // 限制着的，一轮结束运行状态行收起时也按住不往下落。
        let release = crate::ui::release_on_fold(&self.config.timeline);
        if matches!(update, Update::Push(crate::core::Push::TurnEnded(_))) {
            if release {
                self.view.settle();
            } else {
                self.view.hold();
            }
        }
        // 她开始下一步：长正文替人停着的，回到最底下接着跟（`tui.md`「正文」第 1 条）。
        if let Update::Push(Push::BlockStart { block, .. }) = &update
            && *block != Block::Text
        {
            self.view.resume();
        }
        let folds = self.transcript.folds();
        self.transcript.update(update, &self.config.text);
        // 一段刚收起（她开口、一轮结束）：放开一次视口，收起留下的空白由上面的行补满（`tui.md`「正文」第 1 条）。
        if release && self.transcript.folds() > folds {
            self.view.settle();
        }
        // 被退回的排队消息连同粘贴块放回输入框，一条之间空一行，接在已有的字前面（`tui.md`「输入框」第 8、11 条）。
        let returned = self.transcript.take_returned();
        if !returned.is_empty() {
            let mut draft = Draft::default();
            for (text, pasted) in returned {
                draft.append(Draft::from_pasted(&text, &pasted), "\n\n");
            }
            if !self.input.editor.is_empty() {
                draft.append(self.input.draft(), "\n\n");
            }
            self.input.editor.set_draft(draft);
        }
        if undone {
            let said = self
                .transcript
                .entries
                .iter()
                .rev()
                .find(|e| e.kind == Kind::Undo);
            if let Some(said) = said.map(|e| e.text.clone()) {
                self.input.put_back(&said);
            }
        }
    }

    /// 下一次要自己醒来的时刻：提示到点消失；在跑时每秒走一下用时。没有就是 `None`，一直等事件。
    pub fn deadline(&self) -> Option<Instant> {
        let notice = self.notice.as_ref().map(|n| n.until);
        let clock = self.transcript.running.map(|start| {
            let next = start.elapsed().as_secs() + 1;
            start + Duration::from_secs(next)
        });
        // 有步骤在转圈、运行状态行的流光在走，照转圈的节拍重画。
        let moving = self.transcript.busy() || self.transcript.running.is_some();
        let spin =
            moving.then(|| Instant::now() + Duration::from_millis(self.config.timeline.spinner_ms));
        // 首页的吉祥物在转头：照它的节拍画，转到了就停（`tui.md`「空会话的首页」第 7 条）。
        let now = Instant::now();
        let turning = (self.mascot_shown() && self.gaze.moving())
            .then(|| now + Duration::from_millis(self.config.mascot.gaze.frame_ms));
        // 待机小动作：眨眼、晃、抖耳朵的下一刻（第 8 条）。
        let idling = self
            .mascot_shown()
            .then(|| self.idle.wake(now, &self.config.mascot.idle))
            .flatten();
        let walking = self
            .home()
            .then(|| self.perch.wake(now, &self.config.mascot.perch))
            .flatten();
        notice
            .into_iter()
            .chain(clock)
            .chain(spin)
            .chain(turning)
            .chain(idling)
            .chain(walking)
            .chain(self.jobs_deadline())
            .chain(self.drawer_deadline())
            .min()
    }

    /// 吉祥物画着：在首页或宽屏的侧边栏里，而且那里的开关开着（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
    fn mascot_shown(&self) -> bool {
        let layout = &self.config.layout;
        (self.home() && layout.mascot_home)
            || (self.areas.sidebar.width > 0 && layout.mascot_sidebar)
    }

    /// 是首页：正文里一条都没有，也没在回答（`tui.md`「空会话的首页」第 1 条）。
    pub fn home(&self) -> bool {
        self.transcript.entries.is_empty() && self.transcript.running.is_none()
    }

    /// 到点了：收掉过期的提示。
    pub fn tick(&mut self) {
        self.advance_jobs();
        self.drawer_tick();
        if self
            .notice
            .as_ref()
            .is_some_and(|n| n.until <= Instant::now())
        {
            self.notice = None;
        }
    }

    /// 列表开着时筛出来的命令；没开是 `None`。每次按输入框里现在的字重新筛，顺手定开不开。
    pub fn menu_matches(&mut self) -> Option<Vec<Spec>> {
        let text = self.input.editor.text();
        let typed = commands::typed(text);
        let matches: Vec<Spec> = typed
            .map(|t| {
                self.config
                    .commands
                    .filter(t)
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.menu
            .sync(text, typed, matches.len())
            .then_some(matches)
    }

    fn copy(&mut self, text: &str) {
        let words = &self.config.text;
        let (message, good) = match clipboard::copy(text) {
            Ok(()) => (
                words
                    .copied
                    .replace("{count}", &text.chars().count().to_string()),
                true,
            ),
            Err(e) => (words.copy_failed.replace("{reason}", &e.to_string()), false),
        };
        self.hint(message, good);
    }
}
