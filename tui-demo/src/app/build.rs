//! 刚启动时的 [`App`]：照配置摆好各块、读回输入历史、起输入法那一头（从 `mod.rs` 分出来）。

use super::*;

impl App {
    /// 刚启动时的样子：输入框空着，正文空着，核心在连。
    pub fn new(config: Config, core: Core, human: Human, figures: Figures) -> Self {
        // 启动时的配置照系统语言读的（自动）：记下它，选回自动时用。
        let system_language = config.language.clone();
        let md_keep = config.layout.markdown_cache;
        let ime = crate::ime::Ime::start(&config.layout.ime, |name| std::env::var(name).ok());
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
        // 输入历史记在数据根的 `state/tui/history.jsonl`：所有会话一起、重启以后还在（「输入历史列表」第 8 条）。
        // 单元测试不记：数据根是人在用的。
        let saved = miyu_store::root::DataRoot::locate(&miyu_store::env::Env::current())
            .ok()
            .filter(|_| !cfg!(test))
            .map(|root| root.state().join("tui").join("history.jsonl"));
        input.keep_history(crate::input::Saved::at(saved), layout.history_keep);
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
            model_list: None,
            settings: None,
            ime,
            asks: Default::default(),
            efforts: None,
            cards: std::cell::RefCell::new(crate::link_cards::LinkCards::cached()),
            diagrams: std::cell::RefCell::default(),
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
            composing: None,
            currency: "USD".to_string(),
            usage: None,
            edit: None,
        }
    }
}
