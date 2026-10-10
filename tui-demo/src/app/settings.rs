//! 配置页和 App 接在一起（蓝图「配置页」第 1、8、25 条）：`/config` 打开、`--page config` 一起来就打开；开着时按键、鼠标、
//! 粘贴都归它，要发的请求经 `Command::Ask` 交给核心，回应、配置变了、断开连上转给它；关了回到对话，原样不动。

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::Position;

use super::App;
use crate::core::{Command, Update};
use crate::settings::{Outcome, Settings};
use crate::transcript::Link;

impl App {
    /// 打开配置页；`standalone` 是照 `--page config` 起来的（主菜单 `Esc` 退出程序）。
    pub fn open_settings(&mut self, standalone: bool) {
        let online = matches!(self.transcript.link, Link::Ready);
        self.settings = Some(Settings::open(standalone, online));
        self.send_settings_asks();
    }

    /// `/connect`：直接进「供应商和模型」。
    pub(super) fn open_connect(&mut self) {
        let online = matches!(self.transcript.link, Link::Ready);
        self.settings = Some(Settings::connect(online));
        self.send_settings_asks();
    }

    /// 配置页攒着的请求发出去。
    fn send_settings_asks(&mut self) {
        let Some(page) = self.settings.as_mut() else {
            return;
        };
        for (tag, method, params) in page.take_asks() {
            self.core.send(Command::Ask {
                tag,
                method,
                params,
            });
        }
    }

    /// 配置页开着时的一个终端事件。
    pub(super) fn settings_event(&mut self, event: Event) {
        let Some(page) = self.settings.as_mut() else {
            return;
        };
        let texts = &self.config.text.settings;
        let outcome = match event {
            Event::Key(key) if key.kind == KeyEventKind::Release => Outcome::Stay,
            // Ctrl+C：照 `--page config` 起来的退出，对话里打开的回对话（草稿丢掉）。
            Event::Key(key)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'd')) =>
            {
                if page.standalone {
                    Outcome::Quit
                } else {
                    Outcome::Back
                }
            }
            Event::Key(key) => page.key(key, texts),
            Event::Paste(text) => {
                page.paste(&text);
                Outcome::Stay
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let at = Position::new(mouse.column, mouse.row);
                    let window =
                        std::time::Duration::from_millis(self.config.layout.double_click_ms);
                    page.click(at, window, texts)
                }
                MouseEventKind::ScrollDown => page.key(arrow(KeyCode::Down), texts),
                MouseEventKind::ScrollUp => page.key(arrow(KeyCode::Up), texts),
                _ => Outcome::Stay,
            },
            _ => Outcome::Stay,
        };
        self.settings_outcome(outcome);
        self.send_settings_asks();
    }

    /// 配置页读到了要交给编辑器的提示词（「配置页」第 40 条）：写临时文件、让出终端，同 Ctrl+G（`compose.rs`）。编辑器
    /// 照 `$VISUAL`、`$EDITOR`，都没有的用 `vi`；Windows 上没有的不开。
    fn open_prompt_editor(&mut self) {
        let Some(page) = self.settings.as_mut() else {
            return;
        };
        let Some(text) = page.take_prompt() else {
            return;
        };
        let editor = self
            .editor
            .clone()
            .or_else(|| cfg!(unix).then(|| "vi".to_string()));
        let file = std::env::temp_dir().join(format!("miyu-persona-{}.md", std::process::id()));
        match editor.map(|e| (e, std::fs::write(&file, &text))) {
            Some((editor, Ok(()))) => {
                self.settings_prompt = Some(file.clone());
                self.edit = Some((editor, file));
            }
            _ => page.prompt_edited(None),
        }
    }

    /// 编辑器退出了：是配置页的提示词就读回来交还配置页、删掉临时文件。交回是不是它。
    pub(super) fn settings_prompt_edited(&mut self) -> bool {
        let Some(file) = self.settings_prompt.take() else {
            return false;
        };
        let text = std::fs::read_to_string(&file).ok();
        drop(std::fs::remove_file(&file));
        if let Some(page) = self.settings.as_mut() {
            page.prompt_edited(text);
        }
        self.send_settings_asks();
        true
    }

    /// 关配置页、退出程序。
    fn settings_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => {}
            Outcome::Back => {
                self.settings = None;
                // 配置页里可能换了人格的头像：重读人格列表，照新的版本换（「空会话的首页」第 10 条）。
                self.core.send(Command::ListPersonas);
            }
            Outcome::Quit => self.quit = true,
        }
    }

    /// 核心那边的消息先给配置页看：它的回应归它（交回 `true`），配置变了、断开、连上告诉它，别的照旧往下走。
    pub(super) fn settings_update(&mut self, update: &Update) -> bool {
        let Some(page) = self.settings.as_mut() else {
            return matches!(update, Update::Answer { .. });
        };
        let texts = &self.config.text.settings;
        let mine = match update {
            Update::Answer { tag, result } => {
                page.answer(*tag, result.clone(), texts);
                true
            }
            Update::ConfigChanged => {
                page.changed();
                false
            }
            Update::Ready(_) | Update::Reconnected => {
                page.connected();
                false
            }
            Update::Disconnected | Update::Failed(_) | Update::NoCoreBin | Update::Missing(_) => {
                page.disconnected(texts);
                false
            }
            _ => false,
        };
        self.open_prompt_editor();
        self.send_settings_asks();
        mine
    }
}

fn arrow(code: KeyCode) -> ratatui::crossterm::event::KeyEvent {
    ratatui::crossterm::event::KeyEvent::new(code, KeyModifiers::NONE)
}
