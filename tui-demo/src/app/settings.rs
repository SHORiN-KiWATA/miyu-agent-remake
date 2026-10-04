//! App 与全屏配置页的边界：不改会话、聊天草稿或滚动；命令和回复异步发送。

use super::App;
use crate::core::Update;
use crate::settings::{Hit, Settings};
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use ratatui::layout::Position;

impl App {
    /// 打开配置页，独立模式退出回 shell。
    pub fn open_settings(&mut self, standalone: bool) {
        let connected = matches!(self.transcript.link, crate::transcript::Link::Ready);
        self.settings = Some(Settings::new(standalone, connected));
        self.flush_settings();
    }

    fn flush_settings(&mut self) {
        if let Some(settings) = &mut self.settings {
            for command in settings.outgoing() {
                self.core.send(command);
            }
        }
    }

    /// 配置更新先归页面；别的推送继续交给原会话。
    pub(super) fn settings_update(&mut self, update: &mut Option<Update>) -> bool {
        let Some(incoming) = update.as_ref() else {
            return false;
        };
        match incoming {
            Update::SettingsRpc { .. } => {
                if let Some(Update::SettingsRpc { tag, result }) = update.take()
                    && let Some(settings) = &mut self.settings
                {
                    settings.replied(tag, result, &self.config.text.settings);
                }
                self.flush_settings();
                true
            }
            Update::SettingsChanged => {
                if let Some(settings) = &mut self.settings {
                    settings.fetch(false);
                }
                update.take();
                self.flush_settings();
                true
            }
            Update::Ready(_) | Update::Reconnected => {
                if let Some(settings) = &mut self.settings {
                    settings.connected = true;
                    settings.fetch(false);
                }
                self.flush_settings();
                false
            }
            Update::Disconnected | Update::Failed(_) | Update::NoCoreBin | Update::Missing(_) => {
                if let Some(settings) = &mut self.settings {
                    settings.connected = false;
                    settings.note = connection_note(incoming, &self.config.text);
                }
                false
            }
            _ => false,
        }
    }

    /// 打开的配置页接管输入，焦点通知仍发给通知模块。
    pub(super) fn settings_event(&mut self, event: &Event) -> bool {
        let Some(settings) = &mut self.settings else {
            return false;
        };
        let texts = &self.config.text.settings;
        let mut close = false;
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                close = settings.key(*key, texts)
            }
            Event::Paste(value) => settings.paste(value),
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(ratatui::crossterm::event::MouseButton::Left)
                {
                    let pos = Position::new(mouse.column, mouse.row);
                    let hit = settings
                        .hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains(pos))
                        .map(|(_, h)| *h);
                    if let Some(hit) = hit {
                        match hit {
                            Hit::Tab(tab) => settings.tab(tab, texts),
                            Hit::Provider(p)
                                if settings.form.is_none() && settings.editing.is_none() =>
                            {
                                settings.provider = p;
                                settings.column = 0;
                                settings.selected = 0;
                            }
                            Hit::Row(row) if settings.editing.is_none() => {
                                settings.selected = row;
                                if settings.tab == 0 && settings.form.is_none() {
                                    settings.column = 1;
                                }
                                if settings.form.is_some() || settings.tab != 0 {
                                    settings.open(texts);
                                }
                            }
                            Hit::Option(index) => {
                                let multi = settings.editing.as_mut().map(|e| {
                                    e.selected = index;
                                    e.checked.is_some()
                                });
                                if let Some(multi) = multi {
                                    close = settings.key(
                                        KeyEvent::new(
                                            if multi {
                                                KeyCode::Char(' ')
                                            } else {
                                                KeyCode::Enter
                                            },
                                            KeyModifiers::NONE,
                                        ),
                                        texts,
                                    );
                                }
                            }
                            Hit::Action(code) => {
                                close = settings.key(KeyEvent::new(code, KeyModifiers::NONE), texts)
                            }
                            _ => {}
                        }
                    }
                } else if matches!(
                    mouse.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                ) {
                    close = settings.key(
                        KeyEvent::new(
                            if mouse.kind == MouseEventKind::ScrollDown {
                                KeyCode::Down
                            } else {
                                KeyCode::Up
                            },
                            KeyModifiers::NONE,
                        ),
                        texts,
                    );
                }
            }
            Event::FocusGained | Event::FocusLost => {
                self.notifier.focus(matches!(event, Event::FocusGained))
            }
            _ => {}
        }
        if close && self.settings.take().is_some_and(|s| s.standalone) {
            self.quit = true;
        }
        self.flush_settings();
        true
    }
}

fn connection_note(update: &Update, texts: &crate::config::Texts) -> String {
    match update {
        Update::Failed(reason) => texts.core_failed.replace("{reason}", reason),
        Update::Missing(path) => texts.missing_core.replace("{path}", path),
        Update::NoCoreBin => texts.no_core_bin.clone(),
        Update::Disconnected => texts.reconnecting.clone(),
        _ => crate::settings::text(&texts.settings, "not_connected").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_connection_failures_keep_the_original_reason_or_missing_program_path() {
        let config = crate::config::Config::builtin().unwrap();
        assert!(
            connection_note(&Update::Failed("核心拒绝握手".into()), &config.text)
                .contains("核心拒绝握手")
        );
        assert!(
            connection_note(&Update::Missing("/not-installed/miyu".into()), &config.text)
                .contains("/not-installed/miyu")
        );
        assert_eq!(
            connection_note(&Update::NoCoreBin, &config.text),
            config.text.no_core_bin
        );
    }
}
