//! 第一次打开的引导（蓝图 `tui.md`「第一次打开的引导」）：欢迎 → 语言 → 图标 → 接模型 → 建人格 → 选预设 → 好了。
//! 这里是走到哪、按键、等的回应交给哪一步；数值在 `look.rs`，动的东西在 `motion/`，画在 `ui/oobe/`，和 App 接在
//! `app/oobe.rs`。

pub mod asker;
pub mod field;
mod focus;
pub mod look;
pub mod model;
pub mod motion;
pub mod nav;
pub mod persona;
pub mod pick;
pub mod preset;
pub mod sheet;
mod texts;
pub mod wrap;

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::{Value, json};

pub use texts::Texts;

use crate::core::Refusal;
use asker::{Ask, Asker, Waiting};
use motion::{Intro, React, Slide, Stars};
use pick::Pick;

/// 第几步，照先后。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 欢迎页。
    Welcome,
    /// 界面语言。
    Language,
    /// 图标。
    Icons,
    /// 接模型。
    Model,
    /// 建人格。
    Persona,
    /// 选预设。
    Preset,
    /// 好了。
    Done,
}

const ORDER: [Step; 7] = [
    Step::Welcome,
    Step::Language,
    Step::Icons,
    Step::Model,
    Step::Persona,
    Step::Preset,
    Step::Done,
];

impl Step {
    fn at(self) -> usize {
        ORDER.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// 下一步（好了以后没有）。
    pub fn next(self) -> Self {
        ORDER.get(self.at() + 1).copied().unwrap_or(self)
    }

    /// 上一步（欢迎页前面没有）。
    pub fn previous(self) -> Self {
        ORDER[self.at().saturating_sub(1)]
    }

    /// 在进度那一行里是第几个；欢迎、好了不画进度（第 6 条）。
    pub fn bar(self) -> Option<usize> {
        matches!(
            self,
            Step::Language | Step::Icons | Step::Model | Step::Persona | Step::Preset
        )
        .then(|| self.at() - 1)
    }
}

/// 一步按了键、收到回应以后往哪走。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// 留在这一步。
    Stay,
    /// 下一步。
    Next,
    /// 上一步。
    Back,
}

/// 吉祥物该有的反应（第 10 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    /// 抬头等着。
    Wait,
    /// 连上了。
    Happy,
    /// 没连上。
    Sad,
    /// 不等了、也不高兴不失望。
    Calm,
}

/// 交给 App 办的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 换界面语言：语言那一步第几行（第 0 行跟随系统）。
    Language(usize),
    /// 换图标那一套。
    Icons(String),
    /// 把这段字交给 `$EDITOR`（建人格时 `Ctrl+G`）。
    Editor(String),
}

/// 好了那一屏的三样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// 模型。
    pub model: String,
    /// 人格的名字。
    pub persona: String,
    /// 预设的名字。
    pub preset: String,
}

/// 整个引导。
#[derive(Debug)]
pub struct Oobe {
    /// 在第几步。
    pub step: Step,
    /// 正在滑：从哪一步滑过来。
    pub sliding: Option<(Step, Slide)>,
    /// 欢迎页的开场。
    pub intro: Intro,
    /// 好了那一屏的时间线从这一刻起。
    pub done_at: Option<Instant>,
    /// 回车走了：从这一刻起散开。
    pub leaving: Option<Instant>,
    /// 写标记的回应回来了：`Some(None)` 成了，`Some(Some(原话))` 没写成。
    pub marked: Option<Option<String>>,
    /// 星点。
    pub stars: Stars,
    /// 吉祥物的反应。
    pub react: React,
    /// 名字写到吉祥物脚下：名字、从哪一刻起写。
    pub badge: Option<(String, Instant)>,
    /// 语言那一步。
    pub language: Pick,
    /// 图标那一步：第 0 行 `nerd`、第 1 行 `plain`。
    pub icons: Pick,
    /// 接模型。
    pub model: model::ModelStep,
    /// 建人格。
    pub persona: persona::PersonaStep,
    /// 选预设。
    pub preset: preset::PresetStep,
    /// 连着核心。
    pub online: bool,
    /// 数值（`oobe.json`）。
    pub look: look::Look,
    /// 引导从这一刻起：流光照它算。
    pub born: Instant,
    asker: Asker,
    seed: u64,
}

/// 图标那一步两行各是哪一套。
pub const ICON_SETS: [&str; 2] = ["nerd", "plain"];

impl Oobe {
    /// 从欢迎页开场起。`language` 是语言那一步的几行和选着的，`icons` 是现在用的那一套。
    pub fn new(now: Instant, look: look::Look, language: Pick, icons: &str, online: bool) -> Self {
        let seed = crate::rng::Rng::from_clock().next();
        let icon_row = ICON_SETS.iter().position(|s| *s == icons).unwrap_or(0);
        Self {
            step: Step::Welcome,
            sliding: None,
            intro: Intro::new(now),
            done_at: None,
            leaving: None,
            marked: None,
            stars: Stars::gather(now, seed),
            react: React::default(),
            badge: None,
            language,
            // 两行的字照界面语言画（「看得到图标」「是方块或者乱码」），这里记哪一套。
            icons: Pick::new(
                ICON_SETS.iter().map(|s| (*s).to_string()).collect(),
                icon_row,
            ),
            model: model::ModelStep::new(look.catalog_limit),
            persona: persona::PersonaStep::default(),
            preset: preset::PresetStep::default(),
            online,
            look,
            born: now,
            asker: Asker::default(),
            seed,
        }
    }

    /// 攒着要发的请求，App 交给核心。
    pub fn take_asks(&mut self) -> Vec<Ask> {
        self.asker.take()
    }

    /// 按了一个键。`chars` 是欢迎页标题有几个字（跳过开场要用）。
    pub fn key(&mut self, key: KeyEvent, now: Instant, texts: &Texts) -> Vec<Effect> {
        self.react.poke();
        if self.leaving.is_some() {
            return Vec::new();
        }
        // 吉祥物的反应（第 10 条）：在格子里打字歪着头看。
        let typing = self.typing() && focus::is_typed(key);
        let mut effects = Vec::new();
        let moved = match self.step {
            Step::Welcome => {
                if self.intro_running(now, texts) {
                    self.intro.skip();
                    Move::Stay
                } else if key.code == KeyCode::Enter {
                    Move::Next
                } else {
                    Move::Stay
                }
            }
            Step::Language => self.pick_key(Step::Language, key, &mut effects),
            Step::Icons => self.pick_key(Step::Icons, key, &mut effects),
            Step::Model => self.model.key(key, &mut self.asker, &texts.model),
            Step::Persona => self
                .persona
                .key(key, &mut self.asker, &texts.persona, &mut effects),
            Step::Preset => self.preset.key(key, &mut self.asker, &texts.preset),
            Step::Done => {
                if !self.finale(now, texts).ready {
                    self.done_at = now.checked_sub(Duration::from_secs(3600));
                } else if key.code == KeyCode::Enter {
                    self.leave(now);
                }
                Move::Stay
            }
        };
        self.moved(moved, now);
        if typing {
            self.react.typed(now, &self.look.react);
        }
        effects
    }

    /// 粘贴：给光标那一格。
    pub fn paste(&mut self, text: &str) {
        match self.step {
            Step::Model => self.model.paste(text),
            Step::Persona => self.persona.paste(text),
            Step::Preset => self.preset.paste(text),
            _ => {}
        }
    }

    /// 编辑器退出了：写回建人格那一格。
    pub fn edited(&mut self, text: Option<String>) {
        self.persona.edited(text);
    }

    /// 等的回应回来了：是引导的交回 `true`。
    pub fn answer(
        &mut self,
        tag: u64,
        result: Result<Value, Refusal>,
        now: Instant,
        texts: &Texts,
    ) -> bool {
        let Some(waiting) = self.asker.settle(tag) else {
            return false;
        };
        let moved = match &waiting {
            Waiting::Icons => Move::Stay,
            Waiting::Welcomed => {
                self.marked = Some(
                    result
                        .err()
                        .map(|r| crate::settings::pages::target::said(&r)),
                );
                Move::Stay
            }
            Waiting::Models
            | Waiting::Detect
            | Waiting::Catalog
            | Waiting::AllProviders
            | Waiting::Pools
            | Waiting::Test
            | Waiting::Secret
            | Waiting::SaveModel => {
                self.model
                    .answer(&waiting, result, &mut self.asker, &texts.model)
            }
            Waiting::PersonaDefault
            | Waiting::PersonaGet(_)
            | Waiting::PersonaRead(..)
            | Waiting::PersonaSave
            | Waiting::AvatarPut
            | Waiting::PersonaPick(_) => {
                self.persona
                    .answer(&waiting, result, &mut self.asker, &texts.persona)
            }
            Waiting::Presets
            | Waiting::PresetDefault
            | Waiting::PresetGet(_)
            | Waiting::PresetSave
            | Waiting::PresetPick(_) => {
                self.preset
                    .answer(&waiting, result, &mut self.asker, &texts.preset)
            }
        };
        // 人格建好了：名字写到吉祥物脚下（第 10 条）。
        if let Some(name) = self.persona.badge.take() {
            self.badge = Some((name, now));
        }
        self.moved(moved, now);
        true
    }

    /// 连上、断开。断开时等着的都不会回来了：在试、在存的退回去。
    pub fn link(&mut self, online: bool) {
        self.online = online;
        if !online {
            self.asker.forget();
        }
    }

    /// 走完了：回车以后散开的动画播完、写标记的回应也回来了。交回写标记没成时核心的原话。
    pub fn closing(&self, now: Instant, leave: Duration) -> Option<Option<String>> {
        let left = self.leaving.is_some_and(|at| now >= at + leave);
        left.then(|| self.marked.clone()).flatten()
    }

    fn pick_key(&mut self, step: Step, key: KeyEvent, effects: &mut Vec<Effect>) -> Move {
        let pick = if step == Step::Language {
            &mut self.language
        } else {
            &mut self.icons
        };
        if pick.key(key) {
            return Move::Stay;
        }
        match key.code {
            KeyCode::Esc => Move::Back,
            KeyCode::Enter if step == Step::Language => {
                effects.push(Effect::Language(pick.selected));
                Move::Next
            }
            KeyCode::Enter => {
                let set = ICON_SETS[pick.selected.min(ICON_SETS.len() - 1)].to_string();
                let params =
                    json!({"layer": "personal", "changes": [{"key": "tui.icons", "value": set}]});
                self.asker.ask("config.set", params, Waiting::Icons);
                effects.push(Effect::Icons(set));
                Move::Next
            }
            _ => Move::Stay,
        }
    }

    /// 照这一步交回的走：滑过去、吉祥物跳一下；进了哪一步要读的发出去；反应照那一步记下的。
    fn moved(&mut self, moved: Move, now: Instant) {
        let mood = self.model.mood.take().or(self.persona.mood.take());
        match mood {
            Some(Mood::Wait) => self.react.waiting = true,
            Some(Mood::Happy) => self.react.happy(now),
            Some(Mood::Sad) => self.react.sad(now),
            Some(Mood::Calm) => self.react.waiting = false,
            None => {}
        }
        let to = match moved {
            Move::Stay => return,
            Move::Next => self.step.next(),
            Move::Back => self.step.previous(),
        };
        if to == self.step {
            return;
        }
        self.sliding = Some((self.step, Slide::new(now, moved == Move::Back)));
        self.step = to;
        self.react.hop(now);
        self.react.waiting = false;
        match to {
            Step::Welcome => {
                self.intro.skip();
                self.stars = Stars::quiet(now, self.seed);
                self.sliding = None;
            }
            Step::Model => self.model.enter(&mut self.asker),
            Step::Persona => self.persona.enter(&mut self.asker),
            Step::Preset => self.preset.enter(&mut self.asker),
            Step::Done => {
                self.done_at = Some(now);
                self.stars = Stars::gather(now, self.seed.wrapping_add(1));
                self.sliding = None;
            }
            Step::Language | Step::Icons => {}
        }
    }

    /// 好了那一屏回车：写标记，星点散开。
    fn leave(&mut self, now: Instant) {
        self.leaving = Some(now);
        self.stars.scatter(now);
        if self.online {
            let params =
                json!({"layer": "personal", "changes": [{"key": "ui.welcomed", "value": true}]});
            self.asker.ask("config.set", params, Waiting::Welcomed);
        } else {
            self.marked = Some(Some(String::new()));
        }
    }

    /// 欢迎页的开场还在播。
    fn intro_running(&self, now: Instant, texts: &Texts) -> bool {
        let chars = texts.welcome.title.chars().count();
        self.step == Step::Welcome && !self.intro.finished(now, &self.look.intro, chars)
    }

    /// 好了那一屏这一刻的样子（第 11 条）。
    pub fn finale(&self, now: Instant, texts: &Texts) -> motion::Finale {
        let chars = texts.done.title.chars().count();
        let start = self.done_at.unwrap_or(now);
        motion::finale(start, now, &self.look, chars)
    }

    /// 引导里写成默认的人格、预设的编号：紧接着的新会话照它们开，不再问（第 11 条）。
    pub fn picks(&self) -> (Option<String>, Option<String>) {
        (
            self.persona.chosen_id.clone(),
            self.preset.chosen_id.clone(),
        )
    }

    /// 好了那一屏三样。
    pub fn summary(&self) -> Summary {
        Summary {
            model: self.model.chosen.clone().unwrap_or_default(),
            persona: self.persona.chosen.clone().unwrap_or_default(),
            preset: self.preset.chosen.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests;
