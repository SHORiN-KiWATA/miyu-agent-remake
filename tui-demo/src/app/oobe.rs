//! 第一次打开的引导和 App 接在一起（蓝图 `tui.md`「第一次打开的引导」第 1–4、27 条）：连上以后读 `ui.welcomed`，没走过
//! 的开引导；开着时按键、粘贴、鼠标都归它，要发的请求经 `Command::Ask` 交给核心，回应照编号交还；换语言、换图标、开编辑器
//! 由这里办；走完关掉，回到平常的首页。照 `tui.icons` 换图标那一套在 `head_settings.rs`。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::layout::Position;
use serde_json::json;

use super::App;
use crate::core::{Command, Update};
use crate::oobe::asker::TAG_BASE;
use crate::oobe::pick::Pick;
use crate::oobe::{Effect, Oobe};
use crate::transcript::Link;

/// 读「引导走过了没有」的请求编号：在引导自己的编号下面，和配置页的也分开。
const WELCOMED_TAG: u64 = TAG_BASE - 1;

impl App {
    /// 开引导，从欢迎页起（没走过的、`--page oobe` 起来的）。
    pub fn open_oobe(&mut self) {
        let online = matches!(self.transcript.link, Link::Ready);
        let look = self.config.oobe.clone();
        let icons = self.config.icons.name.clone();
        self.oobe = Some(Oobe::new(
            Instant::now(),
            look,
            self.language_pick(),
            &icons,
            online,
        ));
        self.send_oobe_asks();
    }

    /// 语言那一步的几行：第一行跟随系统（写照系统认出来的那种），下面一种一行；选着现在用的那一档。
    fn language_pick(&self) -> Pick {
        let table = &self.config.language_table;
        let auto = self
            .config
            .text
            .oobe
            .language
            .auto
            .replace("{name}", table.name(&self.system_language));
        let rows = std::iter::once(auto)
            .chain(table.languages.iter().map(|l| l.name.clone()))
            .collect();
        let selected = if self.config.auto {
            0
        } else {
            table.position(&self.config.language).map_or(0, |i| i + 1)
        };
        Pick::new(rows, selected)
    }

    /// 引导攒着的请求发出去。
    fn send_oobe_asks(&mut self) {
        let Some(oobe) = self.oobe.as_mut() else {
            return;
        };
        for ask in oobe.take_asks() {
            self.core.send(Command::Ask {
                tag: ask.tag,
                method: ask.method,
                params: ask.params,
            });
        }
    }

    /// 引导开着时的一个终端事件。
    pub(super) fn oobe_event(&mut self, event: Event) {
        let now = Instant::now();
        let texts = self.config.text.oobe.clone();
        let Some(oobe) = self.oobe.as_mut() else {
            return;
        };
        let effects = match event {
            Event::Key(key) if key.kind == KeyEventKind::Release => Vec::new(),
            // Ctrl+C：退出程序，不写标记（下次从头来，第 2 条）。
            Event::Key(key)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'd')) =>
            {
                self.quit = true;
                return;
            }
            Event::Key(key) => oobe.key(key, now, &texts),
            Event::Paste(text) => {
                oobe.paste(&text);
                Vec::new()
            }
            Event::Mouse(mouse)
                if matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) =>
            {
                self.pointer = Some(Position::new(mouse.column, mouse.row));
                Vec::new()
            }
            _ => Vec::new(),
        };
        for effect in effects {
            self.oobe_effect(effect);
        }
        self.send_oobe_asks();
    }

    /// 引导交来办的。
    fn oobe_effect(&mut self, effect: Effect) {
        match effect {
            Effect::Language(index) => {
                self.switch_language(index, false);
                let pick = self.language_pick();
                if let Some(oobe) = self.oobe.as_mut() {
                    oobe.language = pick;
                }
            }
            Effect::Icons(name) => self.use_icons(&name),
            Effect::Editor(text) => {
                let editor = self
                    .editor
                    .clone()
                    .or_else(|| cfg!(unix).then(|| "vi".to_string()));
                let file =
                    std::env::temp_dir().join(format!("miyu-oobe-{}.md", std::process::id()));
                match editor.map(|e| (e, std::fs::write(&file, &text))) {
                    Some((editor, Ok(()))) => {
                        self.oobe_editing = Some(file.clone());
                        self.edit = Some((editor, file));
                    }
                    _ => {
                        if let Some(oobe) = self.oobe.as_mut() {
                            oobe.edited(None);
                        }
                    }
                }
            }
        }
    }

    /// 编辑器退出了：是引导开的就读回去、删掉临时文件。交回是不是它。
    pub(super) fn oobe_edited(&mut self) -> bool {
        let Some(file) = self.oobe_editing.take() else {
            return false;
        };
        let text = std::fs::read_to_string(&file).ok();
        drop(std::fs::remove_file(&file));
        if let Some(oobe) = self.oobe.as_mut() {
            oobe.edited(text);
        }
        true
    }

    /// 换成 `name` 那一套图标（`nerd`、`plain`）；没有这一套的不换。
    pub(super) fn use_icons(&mut self, name: &str) {
        if let Some(set) = self
            .config
            .icon_sets
            .iter()
            .find(|s| s.name == name)
            .cloned()
        {
            self.config.icons = set;
        }
    }

    /// 还在等 `ui.welcomed`：问到以前、最多 `oobe.json` 的 `welcome_wait_ms`，不画首页（「第一次打开的引导」第 1 条，
    /// 2026-10-10 项目主人报：进引导前闪一下空会话）。
    pub fn welcome_pending(&self, now: Instant) -> bool {
        !self.welcome_known && now < self.welcome_until()
    }

    /// 等 `ui.welcomed` 等到什么时候。
    fn welcome_until(&self) -> Instant {
        self.started + Duration::from_millis(self.config.oobe.welcome_wait_ms)
    }

    /// 还在等 `ui.welcomed` 的，到点醒来画首页。
    pub(super) fn welcome_deadline(&self, now: Instant) -> Option<Instant> {
        self.welcome_pending(now).then(|| self.welcome_until())
    }

    /// 核心那边的消息先给引导看：引导、这里自己发的请求的回应归这里（交回 `true`）；连上、断开告诉引导；头的配置读到了
    /// （连上、配置变了）就去读 `ui.welcomed`（只第一次）、`tui.icons`。
    pub(super) fn oobe_update(&mut self, update: &Update) -> bool {
        match update {
            Update::Answer { tag, result } if *tag == WELCOMED_TAG => {
                self.welcome_known = true;
                let welcomed = result
                    .as_ref()
                    .map(|got| got["items"]["ui.welcomed"]["value"] == true);
                // 核心不认这个键（旧核心）、读不成的当走过了（第 1 条）。
                let standalone = self.settings.as_ref().is_some_and(|s| s.standalone);
                if welcomed == Ok(false) && self.oobe.is_none() && !standalone {
                    self.open_oobe();
                }
                true
            }
            Update::Answer { tag, result } if *tag >= TAG_BASE => {
                let texts = self.config.text.oobe.clone();
                if let Some(oobe) = self.oobe.as_mut() {
                    oobe.answer(*tag, result.clone(), Instant::now(), &texts);
                }
                self.send_oobe_asks();
                true
            }
            Update::HeadConfig(_) => {
                if !std::mem::replace(&mut self.welcome_checked, true) {
                    let params = json!({"keys": ["ui.welcomed"]});
                    self.core.send(Command::Ask {
                        tag: WELCOMED_TAG,
                        method: "config.get",
                        params,
                    });
                }
                false
            }
            Update::Ready(_) | Update::Reconnected => {
                if let Some(oobe) = self.oobe.as_mut() {
                    oobe.link(true);
                }
                false
            }
            Update::Disconnected | Update::Failed(_) | Update::NoCoreBin | Update::Missing(_) => {
                self.welcome_known = true;
                if let Some(oobe) = self.oobe.as_mut() {
                    oobe.link(false);
                }
                false
            }
            _ => false,
        }
    }

    /// 走完了：散开的动画播完、写标记的回来了就关掉；没写成的弹一句（下次打开还会进引导）。
    pub(super) fn oobe_tick(&mut self) {
        let Some(oobe) = self.oobe.as_ref() else {
            return;
        };
        let leave = Duration::from_millis(self.config.oobe.done.leave_ms);
        let Some(marked) = oobe.closing(Instant::now(), leave) else {
            return;
        };
        let (persona, preset) = oobe.picks();
        self.oobe = None;
        // 紧接着的新会话照引导里刚选的开，不再弹两个框（第 11 条，2026-10-09 项目主人定）。
        self.adopt_picks(persona, preset);
        // 引导里建的人格可能带头像：重读人格列表（「空会话的首页」第 10 条）。
        self.core.send(Command::ListPersonas);
        if let Some(message) = marked {
            let text = self
                .config
                .text
                .oobe
                .unmarked
                .replace("{message}", &message);
            self.hint(text, false);
        }
        // 底栏照新配的模型写（「第一次打开的引导」第 11 条）。
        self.refresh_effort();
    }

    /// 引导开着时下一次要自己醒来的时刻：有东西在动照 `frame_ms`，只剩星点闪照星点的节拍。
    pub(super) fn oobe_deadline(&self, now: Instant) -> Option<Instant> {
        let oobe = self.oobe.as_ref()?;
        let look = &self.config.oobe;
        let texts = &self.config.text.oobe;
        let ms = Duration::from_millis;
        let chars = texts.welcome.title.chars().count();
        let busy = !oobe.intro.finished(now, &look.intro, chars)
            || oobe
                .sliding
                .is_some_and(|(_, s)| s.offsets(now, ms(look.slide_ms), 1).is_some())
            || oobe.react.busy(now, &look.react)
            || oobe.react.waiting
            || oobe.stars.moving(now, &look.stars)
            || oobe.leaving.is_some()
            || (oobe.done_at.is_some() && !oobe.finale(now, texts).ready)
            || oobe.badge.as_ref().is_some_and(|(name, at)| {
                now < *at
                    + ms(look.react.name_ms) * u32::try_from(name.chars().count()).unwrap_or(0)
            })
            || self.gaze.moving();
        let calm = matches!(
            oobe.step,
            crate::oobe::Step::Welcome | crate::oobe::Step::Done
        );
        let beat = if busy {
            look.frame_ms
        } else if calm {
            look.stars.frame_ms
        } else {
            let idle = crate::ui::oobe_idle_look(&self.config.mascot, &self.config.oobe.idle);
            return self.idle.wake(now, &idle);
        };
        Some(now + ms(beat))
    }
}
