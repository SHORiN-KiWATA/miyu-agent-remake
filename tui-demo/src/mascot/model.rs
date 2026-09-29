//! 吉祥物的模型和数值（`resources/mascot.json`，蓝图 `tui.md`「空会话的首页」第 5–7 条）。
//! 长度都以头的半径为 1：x 往右、y 往上、z 朝着看的人。

use serde::Deserialize;

/// 吉祥物的一块：头、耳朵、鳍，和画在脸上的眼睛。颜色照它取。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    /// 头，连脸上的锯齿线。
    Head,
    /// 头顶的耳朵。
    Ear,
    /// 底下的鳍。
    Fin,
    /// 眼睛。
    Eye,
}

/// 一个椭球：头、一只耳朵或一片鳍。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    /// 是哪一块。
    pub part: Part,
    /// 中心。
    pub center: [f64; 3],
    /// 三个方向的半径。
    pub radii: [f64; 3],
    /// 绕 z 轴歪多少，弧度，逆时针为正。
    pub tilt: f64,
    /// 左右各一个：照 x 镜像再放一个，歪的方向也反过来。
    #[serde(default)]
    pub mirror: bool,
    /// 跟着头转几成：头、耳朵 1；鳍长在身上，只跟一点（不写是 1）。
    #[serde(default = "whole")]
    pub follow: f64,
}

fn whole() -> f64 {
    1.0
}

/// 嘴：圆角矩形的洞。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mouth {
    /// 中心的 x、y。
    pub center: [f64; 2],
    /// 半宽、半高。
    pub half: [f64; 2],
    /// 圆角的半径。
    pub round: f64,
}

/// 脸：画在头的正面，位置用头朝前时的 x、y。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    /// 眼睛：每只一条线段，两头的 x、y。
    pub eyes: Vec<[[f64; 2]; 2]>,
    /// 眼睛的半宽。
    pub eye_width: f64,
    /// 眼睛写哪个字。
    pub eye_mark: char,
    /// 锯齿线：一串点连成折线。
    pub line: Vec<[f64; 2]>,
    /// 锯齿线的半宽。
    pub line_width: f64,
    /// 锯齿线写哪个字。
    pub line_mark: char,
    /// 嘴。
    pub mouth: Mouth,
}

/// 转头的数值。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gaze {
    /// 左右最多转几度。
    pub max_yaw: f64,
    /// 上下最多转几度。
    pub max_pitch: f64,
    /// 虚拟的距离（列）：目标横着离脸多远，转 `atan(远近 ÷ 它)`。
    pub distance: f64,
    /// 缓动：每过这么多毫秒走剩下的一半。
    pub half_life_ms: u64,
    /// 转着的时候隔多少毫秒画一帧。
    pub frame_ms: u64,
}

/// 待机小动作的数值（`tui.md`「空会话的首页」第 8 条）。`[最小, 最大]` 的每次在里面随机。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idle {
    /// 多久没按键、没动鼠标算没人动。
    pub after_ms: u64,
    /// 抖耳朵时隔多少毫秒画一帧。
    pub frame_ms: u64,
    /// 隔多久眨一次眼。
    pub blink_every_ms: [u64; 2],
    /// 闭眼多久。
    pub blink_ms: [u64; 2],
    /// 连眨两下的机会（0–1）。
    pub double_blink: f64,
    /// 连眨两下时中间睁开多久。
    pub double_gap_ms: u64,
    /// 摇到一个角度停多久。
    pub glance_hold_ms: [u64; 2],
    /// 摇头左右最多几度。
    pub glance_yaw: f64,
    /// 摇头上下最多几度。
    pub glance_pitch: f64,
    /// 转回正前方的机会（0–1）。
    pub glance_home: f64,
    /// 摇过去的缓动：每过这么多毫秒走剩下的一半（比跟光标慢）。
    pub glance_half_life_ms: u64,
    /// 隔多久抖一次耳朵。
    pub twitch_every_ms: [u64; 2],
    /// 抖一次多久。
    pub twitch_ms: u64,
    /// 抖到最外面，耳朵多歪多少（弧度）。
    pub twitch_tilt: [f64; 2],
}

/// 被列表顶上去以后走回原处的数值（`tui.md`「空会话的首页」第 4 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Perch {
    /// 列表关了先停多久。
    pub settle_ms: u64,
    /// 往下走一行多少毫秒。
    pub row_ms: u64,
}

/// 整个吉祥物（`resources/mascot.json`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 画出来占几列。
    pub cols: u16,
    /// 画出来占几行。
    pub rows: u16,
    /// 头的半径占几列。
    pub radius: f64,
    /// 一格高是宽的几倍。
    pub cell_aspect: f64,
    /// 头的中心在第几行（从上往下，带小数）；左右总在正中。
    pub center_row: f64,
    /// 从暗到亮挑的字，第一个是空格。
    pub ramp: String,
    /// 光从哪个方向来。
    pub light: [f64; 3],
    /// 背光的面也有这么亮（0–1）。
    pub ambient: f64,
    /// 头、耳朵、鳍。
    pub shapes: Vec<Shape>,
    /// 脸。
    pub face: Face,
    /// 待机小动作。
    pub idle: Idle,
    /// 被列表顶上去以后怎么走回来。
    pub perch: Perch,
    /// 转头。
    pub gaze: Gaze,
}
