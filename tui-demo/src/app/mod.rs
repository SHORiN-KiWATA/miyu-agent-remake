//! 程序的状态，和把终端事件、核心的消息分给各块。按键在 `keys.rs`，鼠标在 `mouse.rs`。

mod drawer;
mod jobs;

pub use jobs::Panel;
mod help;
mod keys;
mod language;
mod mascot;
mod mention;
mod mouse;
mod notify;
mod output;
mod paste;
mod redo;
mod session;
mod sessions;
mod switch;
mod takeback;
mod updates;

use std::cell::RefCell;
use std::time::{Duration, Instant};

use miyu_store::human::Human;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::layout::Position;

use crate::body_view::BodyView;
use crate::commands::{self, Spec};
use crate::config::Config;
use crate::core::Core;
use crate::drawer::Drawers;
use crate::figures::Figures;
use crate::focus::Focus;
use crate::history::History;
use crate::input::{Action, Draft, InputBox, PasteRule};
use crate::jobs::{Board, Feed};
use crate::mascot::{Gaze, Idle, Perch};
use crate::menu::Menu;
use crate::notify::Notifier;
use crate::pulse::Pulse;
use crate::side_select::SideSelect;
use crate::tips::Tips;
use crate::transcript::Transcript;
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

/// 最近发出去的一句，没发出去时撤回来用（「输入框」第 12、13 条）。
struct Unsent {
    /// 放回输入框的字：说的话、编辑上一句改过的；`/redo` 原样重来的没有。
    draft: Option<Draft>,
    /// 编辑上一句的，改之前的那句：放回去以后接着编辑。
    original: Option<Draft>,
    /// 重做先藏掉的那一轮：显示回来。
    turn: Option<u64>,
}

/// 整个程序的状态。
pub struct App {
    /// 界面上的字和布局的数值。
    pub config: Config,
    /// 系统通知、报给 herdr（蓝图「系统通知」）。
    pub notifier: Notifier,
    /// 输入框。
    pub input: InputBox,
    /// 停放着的会话：切进子会话时的主会话、子代理的会话、`/new` 以后还有任务在跑的旧会话（`sessions.rs`）。
    parked: sessions::Lot,
    /// 切进了子会话：主会话的编号（它停放着）；看着主会话是 `None`。
    home: Option<String>,
    /// `/new` 以后还有任务在跑、照样订阅着的旧会话：任务都报完了退订。
    left: Vec<String>,
    /// 切过去、还在补发的会话，和原来那个要不要接着订阅：补完了再换上来（`switch.rs`）。
    opening: Option<(String, bool)>,
    /// 上次交回的会话列表：再开框先照它画（`switch.rs`）。
    sessions_seen: Option<Vec<crate::core::SessionInfo>>,
    /// 最近发出去的一句：没发出去时撤回来（`redo.rs`）。
    unsent: Option<Unsent>,
    /// `Ctrl+V` 贴的截图暂存在哪（「输入框」第 12 条）；找不到缓存目录的是 `None`，贴不了图。
    staging: Option<crate::clipboard::Staging>,
    /// 会话：正文、在不在跑、用量。
    pub transcript: Transcript,
    /// 连着核心的一头。
    core: Core,
    /// 斜杠命令列表。
    pub menu: Menu,
    /// `@` 文件列表。
    pub mention: crate::mention::Mention,
    /// 输入历史列表（`Ctrl+R`）。
    pub history: History,
    /// 运行状态行正在写的词。
    pub pulse: Pulse,
    /// 首页的吉祥物朝哪看（`tui.md`「空会话的首页」第 6、7 条）。
    pub gaze: Gaze,
    /// 首页吉祥物的待机小动作。
    pub idle: Idle,
    /// 首页吉祥物看鼠标还是看输入光标：最近一次动鼠标、按键的时刻。
    pub attention: crate::mascot::Attention,
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
    /// `/sessions` 开着时的会话列表（`switch.rs`）。
    pub session_list: Option<crate::session_list::SessionList>,
    /// 确认和提问的抽屉：现在这一个和排着的（蓝图「确认和提问的抽屉」）。
    pub drawers: Drawers,
    /// 抽屉每一行是第几项（点哪一行点中哪一项）；上一帧排出来的。
    pub drawer_rows: Vec<Option<usize>>,
    /// 这一帧的光标停在哪、显不显示；画的时候填，画完 `main` 写出去（蓝图「每一帧」）。
    pub caret: crate::caret::Caret,
    /// 启动时照系统认出来的界面语言：`/language` 选回自动用它，框的第一行写它（蓝图「界面语言」）。
    pub system_language: crate::language::Language,
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
    /// Ctrl+C 打断时要退回排着的话，走到哪一步了（`takeback.rs`）。
    takeback: Option<takeback::Takeback>,
    /// 吉祥物的嘴（`tui.md`「空会话的首页」第 9 条）。
    pub mouth: crate::mascot::Mouth,
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
    /// 压缩那一行的进度条一顿一顿地追，停多久、追几格要的随机数。
    rng: crate::rng::Rng,
    /// 该退出了。
    pub quit: bool,
    /// 按了 Ctrl+Z，主循环该把程序挂起到后台了。
    pub suspend: bool,
    /// 本机的文本文件用哪个编辑器开（`$VISUAL`、`$EDITOR`）；没有的交给系统（`editor.rs`）。
    editor: Option<String>,
    /// 点了本机的文本文件：主循环让出终端给编辑器，编辑器和文件。
    pub edit: Option<(String, std::path::PathBuf)>,
}

impl App {
    /// 刚启动时的样子：输入框空着，正文空着，核心在连。
    pub fn new(config: Config, core: Core, human: Human, figures: Figures) -> Self {
        // 启动时的配置照系统语言读的（自动）：记下它，选回自动时用。
        let system_language = config.language.clone();
        let md_keep = config.layout.markdown_cache;
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
        input.set_attach_rule(paste::attach_rule(&config));
        // 提示音、暂存的截图放机器缓存目录（「系统通知」第 5 条、「输入框」第 12 条）；找不到的不响、贴不了图。
        let cache = miyu_store::root::cache_root(&miyu_store::env::Env::current()).ok();
        let sounds = cache.as_ref().map(|root| root.join("tui").join("sounds"));
        let staging = cache
            .as_ref()
            .map(|root| crate::clipboard::Staging::new(&root.join("tui").join("pasted")));
        let mut notifier = Notifier::new(
            config.notify.clone(),
            config.text.notify.clone(),
            |name| std::env::var(name).ok(),
            sounds,
        );
        let mention = crate::mention::Mention::new(config.mention.clone());
        // 界面一开就报空闲：herdr 侧栏上马上看得到（「系统通知」第 6 条）。
        notifier.state(crate::notify::State::Idle);
        Self {
            config,
            notifier,
            input,
            staging,
            unsent: None,
            parked: Default::default(),
            home: None,
            left: Vec::new(),
            opening: None,
            sessions_seen: None,
            transcript: Transcript::default(),
            core,
            menu: Menu::default(),
            mention,
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
            session_list: None,
            drawers: Drawers::default(),
            drawer_rows: Vec::new(),
            caret: crate::caret::Caret::default(),
            system_language,
            demo_drawers: (0, 0),
            todo_full: false,
            panel_rows: Vec::new(),
            focus: Focus::Input,
            agents_hover: None,
            perch: Perch::default(),
            takeback: None,
            mouth: crate::mascot::Mouth::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64 >> 5),
            ),
            attention: crate::mascot::Attention::default(),
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
            md_cache: RefCell::new(MdCache::new(md_keep)),
            row_cache: RefCell::new(RowCache::default()),
            figures: RefCell::new(figures),
            started: Instant::now(),
            grab: None,
            theme,
            esc_at: None,
            rng: crate::rng::Rng::from_clock(),
            quit: false,
            suspend: false,
            editor: crate::editor::command(|name| std::env::var(name).ok()),
            edit: None,
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
        // 看鼠标还是看输入光标：记下最近一次动鼠标、按键的时刻（第 6 条）。
        match &event {
            Event::Key(_) | Event::Paste(_) => self.attention.typed(Instant::now()),
            Event::Mouse(m)
                if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) =>
            {
                self.attention.pointed(Instant::now());
            }
            _ => {}
        }
        let menu_open = self.menu_matches().is_some() || self.mention_found().is_some();
        let action = match event {
            // Windows 上松开键也报一次，只认按下和按住。
            Event::Key(key) if key.kind == KeyEventKind::Release => Action::None,
            // 在子会话里看：输入框空着时 Esc 回主会话（「切进子会话」第 4 条）；打断它按 Ctrl+C。
            Event::Key(key)
                if key.code == KeyCode::Esc
                    && self.home.is_some()
                    && self.esc_for_input(menu_open)
                    && self.input.editor.is_empty() =>
            {
                self.leave_child();
                Action::None
            }
            // 编辑上一句时一下就取消（「输入框」第 13 条），不照下面有字时两下清空的规矩。
            Event::Key(key)
                if key.code == KeyCode::Esc
                    && self.input.editing()
                    && self.esc_for_input(menu_open)
                    && self.transcript.running.is_none() =>
            {
                self.input.cancel_edit();
                Action::None
            }
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
            // 终端报的在不在前台：系统通知照它（「系统通知」第 2 条）。
            Event::FocusGained | Event::FocusLost => {
                self.notifier.focus(matches!(event, Event::FocusGained));
                Action::None
            }
            _ => Action::None,
        };
        match action {
            Action::None => {}
            Action::Submit(text) => self.submit(text),
            Action::Copy(text) => self.copy(&text),
            Action::Quit => self.quit = true,
            // Ctrl+C 分级：输入框空着时，在回答就打断（排着队的退回输入框），不在回答才提示用 Ctrl+D 退出。
            Action::ExitHint if self.transcript.running.is_some() => self.interrupt(),
            Action::ExitHint => self.hint(self.config.text.exit_hint.clone(), false),
            Action::Cleared => self.hint(self.config.text.input_cleared.clone(), false),
            Action::Open(path) => self.open_link(&path),
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
        notice
            .into_iter()
            .chain(clock)
            .chain(spin)
            .chain(self.mascot_deadline(Instant::now()))
            .chain(self.jobs_deadline())
            .chain(self.drawer_deadline())
            // 整份重排没排完的：下一帧接着排（蓝图「正文」第 8 条）。
            .chain((self.row_cache.borrow().stale > 0).then(Instant::now))
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
        self.renumber_attachments();
        // 压缩那一行的进度条追一下（`tui.md`「正文」第 9 条）。
        let layout = &self.config.layout;
        self.transcript.climb(
            Instant::now(),
            layout.bar.width,
            &layout.compaction,
            &mut self.rng,
        );
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
        // 翻输入历史翻出来的命令还没改过：不弹，`↑` `↓` 接着翻（蓝图「按键」`↑`、`↓`）。
        if self.input.recalled() {
            return None;
        }
        let text = self.input.editor.text();
        let typed = commands::menu_typed(text);
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
}
