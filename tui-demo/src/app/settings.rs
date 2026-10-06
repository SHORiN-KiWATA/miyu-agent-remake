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

    /// 关配置页、退出程序。
    fn settings_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => {}
            Outcome::Back => self.settings = None,
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
        self.send_settings_asks();
        mine
    }
}

fn arrow(code: KeyCode) -> ratatui::crossterm::event::KeyEvent {
    ratatui::crossterm::event::KeyEvent::new(code, KeyModifiers::NONE)
}
