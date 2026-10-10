//! 引导里的吉祥物（「第一次打开的引导」第 8–11 条）：转头、眨眼、待机小动作照首页（`ui/mascot_view.rs`），看的是人正在
//! 填的那一格；开场、反应、好了那一屏叠上去的（转一圈、从黑到亮、张嘴、耷拉耳朵、摇头、抬头）照 [`Extra`]。

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::app::App;
use crate::mascot::{self, Look, Pose};
use crate::ui::mascot_view;

/// 叠上去的样子。
#[derive(Debug, Clone, Copy, Default)]
pub struct Extra {
    /// 整个身子转多少度。
    pub spin: f64,
    /// 暗多少。
    pub dark: f64,
    /// 嘴张多大。
    pub mouth: f64,
    /// 耳朵往外多歪多少。
    pub ear: f64,
    /// 闭着眼。
    pub blink: bool,
    /// 左右多转多少度。
    pub yaw: f64,
    /// 上下看哪，盖过转头。
    pub pitch: Option<f64>,
    /// 歪头多少度（打字时）。
    pub roll: f64,
}

/// 照 `look` 的大小画在 `rect`：鼠标在动看鼠标，不然看 `target`（人正在填的那一格），都没有的看正前方。`reach` 是看的东西
/// 横着最远在几列外：虚拟距离放远到那里刚好转到头，内容一直在右边，不然头一直转到头（同侧边栏，`mascot::reaching`）。
pub fn draw(
    frame: &mut Frame,
    rect: Rect,
    look: &Look,
    app: &mut App,
    (target, reach): (Option<Position>, f64),
    extra: Extra,
) {
    let now = Instant::now();
    let face = (
        f64::from(rect.x) + f64::from(look.cols) / 2.0,
        f64::from(rect.y) + look.center_row,
    );
    let settle = Duration::from_millis(look.gaze.pointer_settle_ms);
    let target = if app.attention.on_pointer(now, settle) {
        app.pointer.or(target)
    } else {
        target.or(app.pointer)
    };
    let idle = idle_look(look, &app.config.oobe.idle);
    let glance_roll = app.config.oobe.idle.glance_roll;
    let idling = app.idle.pose(now, &idle);
    let half_life = if idling.glance.is_some() {
        look.idle.glance_half_life_ms
    } else {
        look.gaze.half_life_ms
    };
    let aim = match idling.glance {
        Some(angles) => angles,
        None => target.map_or((0.0, 0.0), |p| {
            let at = (f64::from(p.x) + 0.5, f64::from(p.y) + 0.5);
            mascot::toward(
                face,
                at,
                look.cell_aspect,
                &mascot::reaching(&look.gaze, reach),
            )
        }),
    };
    app.gaze.aim(aim);
    app.gaze.step(now, Duration::from_millis(half_life));
    let (yaw, pitch) = app.gaze.angles();
    // 闲着四处看时顺带歪一点头。
    let wander = if idling.glance.is_some() {
        yaw * glance_roll
    } else {
        0.0
    };
    let pose = Pose {
        yaw: yaw + extra.yaw,
        pitch: extra.pitch.unwrap_or(pitch),
        roll: extra.roll + wander,
        blink: idling.blink || extra.blink,
        ear: idling.ear + extra.ear,
        mouth: extra.mouth,
        spin: extra.spin,
        dark: extra.dark,
    };
    mascot_view::paint(frame, rect, look, &pose);
}

/// 引导里闲着时比首页勤（第 10 条）：首页的待机小动作盖上 `oobe.json` 的几项。画和定下一次醒来都照它。
pub fn idle_look(look: &Look, tune: &crate::oobe::look::Idle) -> crate::mascot::IdleLook {
    let mut idle = look.idle.clone();
    idle.after_ms = tune.after_ms;
    idle.glance_hold_ms = tune.glance_hold_ms;
    idle.glance_yaw = tune.glance_yaw;
    idle.twitch_every_ms = tune.twitch_every_ms;
    idle
}
