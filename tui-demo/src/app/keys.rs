//! 按键：列表开着时先归列表，别的给输入框；两下 `Esc`、翻正文、清屏、斜杠命令的执行（`tui.md`「按键」）。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::App;
use crate::commands::{Line, Run, Spec};
use crate::core::Command;
use crate::input::{Action, Draft};

impl App {
    /// 按了要按两下的 `Esc`：在回答时打断（排着队的接着发），没在回答时清空输入框。
    /// 上一下还在时限里就做，不然只提示再按一次。
    pub(super) fn esc(&mut self) {
        let window = Duration::from_millis(self.config.layout.esc_window_ms);
        let running = self.transcript.running.is_some();
        match self.esc_at.take() {
            Some(at) if at.elapsed() <= window => {
                self.notice = None;
                if running {
                    self.core.send(Command::Interrupt { send: true });
                } else {
                    self.input.editor.take();
                }
            }
            _ => {
                self.esc_at = Some(Instant::now());
                let text = &self.config.text;
                let hint = if running {
                    &text.esc_hint
                } else {
                    &text.esc_clear_hint
                };
                self.hint(hint.clone(), false);
            }
        }
    }

    /// 换下一套主题，只管这一次启动（`tui.md`「主题」第 3 条）。
    pub(super) fn next_theme(&mut self) {
        let themes = &self.config.themes;
        let at = themes
            .iter()
            .position(|(name, _)| *name == self.theme)
            .unwrap_or(0);
        if let Some((name, palette)) = themes.get((at + 1) % themes.len().max(1)) {
            crate::theme::set(palette.clone());
            self.theme = name.clone();
            let text = self.config.text.theme_changed.replace("{name}", name);
            self.hint(text, true);
        }
    }

    /// 换下一套图标，只管这一次启动（`tui.md`「图标」第 4 条）。排好的时间线照图标那一套的名字重排。
    pub(super) fn next_icons(&mut self) {
        let sets = &self.config.icon_sets;
        let at = sets
            .iter()
            .position(|s| s.name == self.config.icons.name)
            .unwrap_or(0);
        if let Some(next) = sets.get((at + 1) % sets.len().max(1)).cloned() {
            let text = self.config.text.icons_changed.replace("{name}", &next.name);
            self.config.icons = next;
            self.hint(text, true);
        }
    }

    /// 按键：列表开着时，上下、Tab、Enter、Esc 归列表，别的照旧给输入框。
    pub(super) fn key(&mut self, key: KeyEvent) -> Action {
        // Ctrl+V：读剪贴板，照粘贴处理；列表、抽屉开着时也一样（`tui.md`「按键」）。
        if is_paste(&key) {
            self.paste_clipboard();
            return Action::None;
        }
        // 输入历史列表开着时，按键先归它（`tui.md`「输入历史列表」）。
        if self.history.open {
            self.history_key(key);
            return Action::None;
        }
        // 确认和提问的抽屉开着时按键先归它（`tui.md`「确认和提问的抽屉」第 4 条）。
        if self.drawer_key(key) {
            return Action::None;
        }
        // Tab、Shift+Tab（终端报成 BackTab）轮换权限级别；命令列表、`@` 文件列表开着时 Tab 归列表（`tui.md`「按键」）。
        let menu_open = self.menu_matches().is_some() || self.mention_found().is_some();
        // 整屏看输出、后台面板、后台按钮先拿按键（`tui.md`「后台命令、子代理和侧边栏」）。
        if self.jobs_key(key, menu_open) {
            return Action::None;
        }
        if cycles_level(&key, menu_open) {
            self.cycle_level();
            return Action::None;
        }
        // 正文里有选区：Ctrl+C、Ctrl+Shift+C 复制它，Esc 取消它；都先于输入框（`tui.md`「按键」）。
        let ctrl = key
            .modifiers
            .contains(ratatui::crossterm::event::KeyModifiers::CONTROL);
        // 侧边栏有选区：Ctrl+C 复制它，Esc 取消它（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
        if self.side_select.active() {
            if ctrl && matches!(key.code, KeyCode::Char('c' | 'C')) {
                let text = self.side_select.text();
                self.copy(&text);
                return Action::None;
            }
            if key.code == KeyCode::Esc {
                self.side_select.clear();
                return Action::None;
            }
        }
        if self.view.select.is_some() {
            if ctrl && matches!(key.code, KeyCode::Char('c' | 'C')) {
                let text = self.view.selected_text();
                self.copy(&text);
                return Action::None;
            }
            if key.code == KeyCode::Esc {
                self.view.select = None;
                return Action::None;
            }
        }
        // Ctrl+L：清屏，把视口顶空，往回滚内容还在；回答进行中不清（新的字马上又冒出来）。
        if ctrl && key.code == KeyCode::Char('l') {
            if self.transcript.running.is_some() {
                self.hint(self.config.text.no_clear_running.clone(), false);
            } else {
                self.view.clear();
            }
            return Action::None;
        }
        // PgUp、PgDn：翻正文。
        if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
            self.view.page(key.code == KeyCode::PageDown);
            return Action::None;
        }
        // Ctrl+R：调出输入历史列表；恢复撤销只用 /restore（`tui.md`「按键」）。
        if ctrl && key.code == KeyCode::Char('r') {
            if self.input.sent().is_empty() {
                self.hint(self.config.text.history.none.clone(), false);
            } else {
                self.history.open();
            }
            return Action::None;
        }
        // Ctrl+End：回到正文最底下，跟着最新的（`tui.md`「按键」，2026-10-02 项目主人要）。
        if ctrl && key.code == KeyCode::End {
            self.view.follow();
            return Action::None;
        }
        // Ctrl+G：用编辑器写输入框里的话（`compose.rs`，`tui.md`「按键」）。
        if ctrl && key.code == KeyCode::Char('g') {
            self.compose();
            return Action::None;
        }
        // Ctrl+Z：挂起到后台；主循环照 `suspend` 还原终端、发信号（`tui.md`「按键」）。Windows 没有作业控制。
        if ctrl && key.code == KeyCode::Char('z') {
            self.suspend = cfg!(unix);
            return Action::None;
        }
        // Ctrl+Enter：输入框有字先发（在回答时排进队），在回答的当场打断、排着的马上发（`tui.md`「按键」，
        // 2026-10-02 项目主人要；不提示）。没在回答时同 Enter。
        if ctrl && key.code == KeyCode::Enter && self.menu_matches().is_none() {
            let running = self.transcript.running.is_some();
            let plain = KeyEvent::new(
                KeyCode::Enter,
                ratatui::crossterm::event::KeyModifiers::NONE,
            );
            let action = self.input.key(plain);
            if running {
                if let Action::Submit(text) = action {
                    self.submit(text);
                }
                // 没有排着的（输入框也是空的）：什么都不做，不当打断用。
                if self.transcript.entries.iter().any(|e| e.queued) {
                    self.core.send(Command::Interrupt { send: true });
                }
                return Action::None;
            }
            return action;
        }
        // `@` 文件列表开着：选、进目录、收成块、关（「`@` 文件列表」第 5 条）；别的键照常打字。
        if let Some(found) = self.mention_found()
            && let Some(action) = self.mention_key(key, &found)
        {
            return action;
        }
        let Some(matches) = self.menu_matches() else {
            return self.input.key(key);
        };
        let selected = matches.get(self.menu.selected).cloned();
        match (key.code, selected) {
            (KeyCode::Up, _) => self.menu.step(false, matches.len()),
            (KeyCode::Down, _) => self.menu.step(true, matches.len()),
            (KeyCode::Tab, Some(spec)) => {
                self.input.editor.take();
                self.input.paste(&format!("/{}", spec.name));
            }
            (KeyCode::Enter, Some(spec)) => {
                self.input.editor.take();
                self.run(&spec, None);
            }
            (KeyCode::Esc, _) => self.menu.dismiss(self.input.editor.text()),
            _ => return self.input.key(key),
        }
        Action::None
    }

    /// 回车发出去的字：是命令就执行；像命令又没有这个命令的弹提示「命令不存在」，字留在输入框里（`tui.md`
    /// 「斜杠命令列表」第 4 条）；别的发给她。
    pub(super) fn submit(&mut self, draft: Draft) {
        let text = draft.text.clone();
        // 在编辑上一句：打了命令就不编辑了；打错了命令、发不出去的，接着编辑（「输入框」第 13 条）。
        let edit = self.input.take_edit();
        match self.config.commands.read(&text) {
            Line::Command(spec, words) => {
                let (spec, words) = (spec.clone(), words.map(str::to_string));
                self.run(&spec, words.as_deref());
                return;
            }
            Line::Unknown => {
                let note = self.config.text.unknown_command.clone();
                self.hint(note, false);
                self.input.editor.set_draft(draft);
                if let Some(original) = edit {
                    self.input.resume_edit(original);
                }
                return;
            }
            Line::Talk => {}
        }
        // 连不上核心：发不出去，字留在输入框里（「连核心」第 8 条）。
        if !self.reachable() {
            self.input.editor.set_draft(draft);
            if let Some(original) = edit {
                self.input.resume_edit(original);
            }
            return;
        }
        if let Some(original) = edit {
            self.submit_edit(original, draft);
            return;
        }
        self.view.follow();
        // 输入框里的粘贴块发出去换回原文；输入历史和正文里照输入框的样子（`tui.md`「输入框」第 11 条）。附件（图）的
        // 块照留 `[图片 1]`，文件跟着发（第 12 条）。
        let full = draft.expand();
        let chips = super::paste::chips(&draft);
        let files = draft.attachments();
        self.unsent = Some(super::Unsent {
            draft: Some(draft.clone()),
            original: None,
            turn: None,
        });
        self.input.remember(draft);
        self.input.forget_cleared();
        self.transcript.user(text, chips);
        self.core.send(Command::Send { text: full, files });
    }

    /// 执行一条命令；`words` 是名字后面的字（能带参数的命令才有，蓝图「斜杠命令列表」第 5 条）。
    pub(super) fn run(&mut self, spec: &Spec, words: Option<&str>) {
        // 命令也记进输入历史：从列表里回车、点的，和整条打出来回车的一样，带参数的记整条（`tui.md`「按键」↑、↓）。
        let line = match words {
            Some(words) => format!("/{} {words}", spec.name),
            None => format!("/{}", spec.name),
        };
        self.input.remember(Draft::plain(&line));
        self.view.follow();
        match spec.run {
            Run::Revert
            | Run::Unrevert
            | Run::Compact
            | Run::Clear
            | Run::Redo
            | Run::Edit
            | Run::Recap
            | Run::Rename
                if !self.reachable() =>
            {
                self.input.editor.set_draft(Draft::plain(&line));
            }
            Run::Revert
            | Run::Unrevert
            | Run::Compact
            | Run::Clear
            | Run::Redo
            | Run::Edit
            | Run::Recap
            | Run::Rename
                if self.not_opened() =>
            {
                self.nothing_yet(spec.run);
            }
            Run::Redo => self.redo(),
            Run::Edit => self.edit_last(),
            Run::Clear => self.core.send(Command::Clear),
            Run::Recap => {
                self.core.send(Command::Recap);
                let note = self.config.text.recap.working.clone();
                self.hint(note, false);
            }
            Run::Rename => self.rename(words),
            Run::Sessions => self.open_sessions(),
            Run::Model => self.open_models(),
            Run::Config => self.open_settings(false),
            Run::Connect => self.open_connect(),
            Run::Effort => self.open_effort(),
            Run::Copy => self.copy_reply(),
            Run::New => self.new_session(),
            Run::Revert => self.core.send(Command::Revert),
            Run::Unrevert => self.core.send(Command::Unrevert),
            Run::Compact => self.core.send(Command::Compact(words.map(str::to_string))),
            Run::Theme => self.next_theme(),
            Run::Icons => self.next_icons(),
            Run::Quit => self.quit = true,
            Run::Help => self.open_help(),
            Run::Language => self.open_languages(),
            Run::Fake => {
                // 只弹提示框，不写进正文（2026-10-01 项目主人）。
                let note = self.config.text.fake_command.replace("{name}", &spec.name);
                self.hint(note, false);
            }
            Run::DemoTodo => self.demo(spec.run),
            Run::DemoAsk | Run::DemoApprove => self.demo_drawer(spec.run),
        }
    }

    /// `/copy`：复制她上一轮的回答，照拖选以后 Ctrl+C 那一套；还没有回答的弹一句（蓝图「斜杠命令」`/copy`）。
    fn copy_reply(&mut self) {
        match self.transcript.last_reply() {
            Some(text) => self.copy(&text),
            None => {
                let note = self.config.text.nothing_to_copy.clone();
                self.hint(note, false);
            }
        }
    }

    /// 输入历史列表开着时的按键：`↑`、`Ctrl+R` 往更早的走，`↓` 往更新的走，打字进搜索，
    /// `Enter` 挑中放进输入框，`Tab` 展开全文，`Esc` 关掉（`tui.md`「输入历史列表」第 2–4、7 条）。
    fn history_key(&mut self, key: KeyEvent) {
        use ratatui::crossterm::event::KeyModifiers;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !ctrl && !key.modifiers.contains(KeyModifiers::ALT);
        let count = self.history.matches(self.input.sent()).len();
        match key.code {
            KeyCode::Up => self.history.older(count),
            KeyCode::Char('r') if ctrl => self.history.older(count),
            KeyCode::Down => self.history.newer(),
            KeyCode::Enter => self.pick_history(),
            KeyCode::Tab => {
                let selected = self.history.selected;
                let found = self.history.matches(self.input.sent());
                if let Some(at) = found.get(selected).map(|s| s.at) {
                    self.history.toggle_full(at);
                }
            }
            KeyCode::Esc => self.history.close(),
            KeyCode::Backspace => self.history.backspace(),
            KeyCode::Char(c) if plain => self.history.type_text(&c.to_string()),
            _ => {}
        }
    }

    /// 挑中选中的那一条：放进输入框，关掉列表。
    pub(super) fn pick_history(&mut self) {
        let picked = self
            .history
            .matches(self.input.sent())
            .get(self.history.selected)
            .map(|sent| sent.draft.text.clone());
        self.history.close();
        if let Some(text) = picked {
            self.input.pick(&text);
        }
    }
}

/// 这一下是不是 `Ctrl+V`（认得 kitty 键盘协议的终端里按着 Shift 报成大写的 V，不算：那是终端自己的粘贴）。
fn is_paste(key: &KeyEvent) -> bool {
    use ratatui::crossterm::event::KeyModifiers;
    key.code == KeyCode::Char('v') && key.modifiers == KeyModifiers::CONTROL
}

/// 这一下是不是轮换权限级别：`Shift+Tab` 什么时候都是；`Tab` 在命令列表没开时是（开着时补全命令）。
fn cycles_level(key: &KeyEvent, menu_open: bool) -> bool {
    key.code == KeyCode::BackTab
        || (key.code == KeyCode::Tab && key.modifiers.is_empty() && !menu_open)
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{cycles_level, is_paste};

    #[test]
    fn tab_and_shift_tab_cycle_the_level_unless_the_menu_wants_tab() {
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let back = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert!(cycles_level(&tab, false), "Tab 轮换");
        assert!(cycles_level(&back, false), "Shift+Tab 留着");
        assert!(!cycles_level(&tab, true), "命令列表开着：Tab 补全命令");
        assert!(cycles_level(&back, true));
    }

    #[test]
    fn ctrl_v_reads_the_clipboard_but_ctrl_shift_v_is_the_terminals() {
        let key = |code, m| KeyEvent::new(code, m);
        assert!(is_paste(&key(KeyCode::Char('v'), KeyModifiers::CONTROL)));
        // 按着 Shift 的是终端自己的粘贴（走括号粘贴），不读剪贴板。
        assert!(!is_paste(&key(
            KeyCode::Char('V'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )));
        assert!(!is_paste(&key(KeyCode::Char('v'), KeyModifiers::NONE)));
    }
}
