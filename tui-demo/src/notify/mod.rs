//! 系统通知（蓝图 `tui.md`「系统通知」）：终端不在前台时，回答好了、出错了、在等你确认或回答，弹一条通知、响一声；
//! 在 herdr 里把在做、在等、空闲报给它。
//!
//! 弹不弹、怎么弹、响不响由 [`plan`] 定（纯函数，好测）；[`Notifier`] 记着终端在不在前台，照它做。要写给终端的转义
//! 序列攒在 [`Notifier::outbox`]，主循环画完一帧再写出去。

pub mod herdr;
mod osc;
mod sound;
mod system;

use std::path::PathBuf;

use crate::config::{NotifyLook, NotifyTexts};
pub use herdr::State;

/// 通知从哪条路出去（第 4 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// kitty 的 OSC 99：点一下跳回窗口，前台不前台 kitty 自己判。
    Kitty,
    /// iTerm2、WezTerm、ghostty 认的 OSC 9。
    Osc9,
    /// 交给系统：`notify-send`、`osascript`。
    System,
}

impl Route {
    /// 照环境变量认。在 tmux、herdr 里 OSC 过不去（tmux 不转，herdr 吞掉），交给系统。
    pub fn detect(var: &impl Fn(&str) -> Option<String>, osc9_programs: &[String]) -> Self {
        let has = |name: &str| var(name).is_some_and(|v| !v.is_empty());
        if has("TMUX") || var("HERDR_ENV").as_deref() == Some("1") {
            return Route::System;
        }
        if var("TERM").as_deref() == Some("xterm-kitty") || has("KITTY_WINDOW_ID") {
            return Route::Kitty;
        }
        let program = var("TERM_PROGRAM").unwrap_or_default();
        if osc9_programs.contains(&program) {
            return Route::Osc9;
        }
        Route::System
    }
}

/// 要通知的一件事（第 1、3 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 一轮回答完了。
    Replied,
    /// 这一轮出错了。
    Failed,
    /// 在等你确认。
    Approving,
    /// 在等你回答。
    Asking,
}

/// 这一次怎么做。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// 弹通知走哪条路；不弹是 `None`。
    pub route: Option<Route>,
    /// 响一声。
    pub sound: bool,
}

/// 弹不弹、怎么弹、响不响（第 2、4、5 条）。`focused` 是终端报的在不在前台，一次都没报过是 `None`（当在前台）。
/// kitty 不管在不在前台都交给它（`o=unfocused` 它自己判），别的只在报过「离开」时弹；响只在报过「离开」时，
/// 在 herdr 里仍弹桌面通知，声音由 herdr 发出。
pub fn plan(look: &NotifyLook, route: Route, focused: Option<bool>, in_herdr: bool) -> Plan {
    if !look.enabled {
        return Plan {
            route: None,
            sound: false,
        };
    }
    let away = focused == Some(false);
    Plan {
        route: (route == Route::Kitty || away).then_some(route),
        sound: look.sound && away && !in_herdr,
    }
}

/// 界面这一边的通知：记着终端在不在前台，照 [`plan`] 弹、响，报给 herdr。
pub struct Notifier {
    look: NotifyLook,
    texts: NotifyTexts,
    route: Route,
    focused: Option<bool>,
    herdr: Option<herdr::Herdr>,
    /// 提示音写到哪个目录；找不到缓存目录的不响。
    sounds: Option<PathBuf>,
    /// kitty 通知的编号：同一个界面的新通知顶掉旧的。
    id: String,
    outbox: Vec<String>,
}

impl Notifier {
    /// 照环境变量认终端和 herdr。`sounds` 是提示音写到哪个目录。
    pub fn new(
        look: NotifyLook,
        texts: NotifyTexts,
        var: impl Fn(&str) -> Option<String>,
        sounds: Option<PathBuf>,
    ) -> Self {
        let route = Route::detect(&var, &look.osc9_programs);
        Self {
            route,
            focused: None,
            herdr: herdr::Herdr::detect(&var),
            sounds,
            id: format!("miyu-{}", std::process::id()),
            outbox: Vec::new(),
            look,
            texts,
        }
    }

    /// 终端报了在不在前台。
    pub fn focus(&mut self, focused: bool) {
        self.focused = Some(focused);
    }

    /// 报给 herdr（不在 herdr 里什么都不做）。
    pub fn state(&mut self, state: State) {
        if let Some(herdr) = self.herdr.as_mut() {
            herdr.report(state);
        }
    }

    /// 主会话变了：更新 herdr 恢复命令，空会话清除旧绑定。
    pub fn session(&mut self, session: Option<&str>) {
        if let Some(herdr) = self.herdr.as_mut() {
            herdr.session(session);
        }
    }

    /// 出了一件要通知的事。
    pub fn tell(&mut self, event: Event) {
        let plan = plan(&self.look, self.route, self.focused, self.herdr.is_some());
        let body = match event {
            Event::Replied => self.texts.replied.clone(),
            Event::Failed => self.texts.failed.clone(),
            Event::Approving => self.texts.approving.clone(),
            Event::Asking => self.texts.asking.clone(),
        };
        let title = &self.texts.title;
        match plan.route {
            Some(Route::Kitty) => {
                let seq = osc::kitty(&self.id, &self.look.app, title, &body);
                self.outbox.push(seq);
            }
            Some(Route::Osc9) => self.outbox.push(osc::osc9(title, &body)),
            Some(Route::System) => {
                let (program, args) = system::command(&self.look.app, title, &body);
                system::spawn(&program, &args);
            }
            None => {}
        }
        if plan.sound
            && let Some(file) = self.sounds.as_deref().and_then(sound::materialize)
        {
            sound::play(self.look.players.here(), &file);
        }
    }

    /// 攒着的、要写给终端的转义序列：主循环画完一帧写出去。
    pub fn outbox(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outbox)
    }
}

#[cfg(test)]
mod tests;
