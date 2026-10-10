//! 引导的数值（`resources/oobe.json`，蓝图 `tui.md`「第一次打开的引导」第 8–12 条）：吉祥物多大、内容多宽、
//! 开场的时间线、星点、换一步滑多久、吉祥物的反应。

use serde::Deserialize;

/// 欢迎页上的吉祥物。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    /// 最多放大几倍。
    pub scale: f64,
    /// 最多占窗口高度的几成。
    pub height: f64,
    /// 吉祥物和下面的字之间空几行。
    pub gap: u16,
}

/// 中间几步的版面。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    /// 内容最宽几列。
    pub content_width: u16,
    /// 吉祥物、内容这一整组两边各至少留几列；放不下就收掉吉祥物（第 10 条，2026-10-09 项目主人：「如果窗口宽度不够
    /// 应该自动隐藏吉祥物」）。
    pub margin: u16,
    /// 吉祥物和内容之间空几列。
    pub gap: u16,
    /// 内容最高几行（窗口更高的上下居中）。
    pub max_height: u16,
}

/// 开场的时间线（第 8 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intro {
    /// 转一整圈多久。
    pub spin_ms: u64,
    /// 转圈的曲线：先往反方向蓄力、最后冲过头的力度（0 是不蓄不冲，`motion::swing`）。
    pub spin_swing: f64,
    /// 从黑到亮多久。
    pub fade_ms: u64,
    /// 转完以后到开始打字多久：这中间抖耳朵、连眨两下眼。
    pub settle_ms: u64,
    /// 抖耳朵多久。
    pub twitch_ms: u64,
    /// 耳朵往外歪多少（弧度）。
    pub twitch_tilt: f64,
    /// 眨一下闭多久。
    pub blink_ms: u64,
    /// 连眨两下中间睁开多久。
    pub blink_gap_ms: u64,
    /// 打一个字多久。
    pub type_ms: u64,
    /// 说话时嘴张多大。
    pub talk: f64,
    /// 说明淡出来多久。
    pub sub_ms: u64,
}

/// 星点（第 8、11 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stars {
    /// 开场冒出几颗。
    pub count: usize,
    /// 聚到中间一共多久。
    pub gather_ms: u64,
    /// 每颗晚出发最多多久（越晚的走得越快）。
    pub lead_ms: u64,
    /// 聚完以后留几颗在四周闪。
    pub linger: usize,
    /// 一闪的周期，`[最短, 最长]`。
    pub twinkle_ms: [u64; 2],
    /// 散开多久。
    pub scatter_ms: u64,
    /// 从暗到亮挑的字。
    pub marks: Vec<String>,
    /// 只剩一闪一闪时，多久画一帧。
    pub frame_ms: u64,
}

/// 吉祥物的反应（第 10 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct React {
    /// 跳一下多久。
    pub hop_ms: u64,
    /// 跳多高（行）。
    pub hop_rows: u16,
    /// 跳的时候嘴张多大。
    pub hop_mouth: f64,
    /// 摇头多久。
    pub shake_ms: u64,
    /// 摇几下。
    pub shakes: u32,
    /// 摇到多少度。
    pub shake_yaw: f64,
    /// 耷拉耳朵往外歪多少（弧度）。
    pub droop: f64,
    /// 试连接时抬头多少度（往上为负）。
    pub look_up: f64,
    /// 眨一下闭多久。
    pub blink_ms: u64,
    /// 连眨两下中间睁开多久。
    pub blink_gap_ms: u64,
    /// 名字写到脚下，一个字多久。
    pub name_ms: u64,
    /// 跳的时候耳朵扑多少（弧度）。
    pub hop_ear: f64,
    /// 打字时头歪多少度（往右为正：歪向右边的内容）。
    pub tilt: f64,
    /// 停手以后头歪着再留多久。
    pub tilt_hold_ms: u64,
    /// 歪过去、回正各要多久。
    pub tilt_ease_ms: u64,
    /// 打一个字耳朵抖多少（弧度）、抖多久。
    pub type_ear: f64,
    /// 耳朵抖多久。
    pub type_ear_ms: u64,
}

/// 引导里吉祥物闲着时比首页勤（第 10 条）：盖在 `mascot.json` 的待机小动作上的几项，闲着时摇头顺带歪一点头。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idle {
    /// 多久没人动算闲着。
    pub after_ms: u64,
    /// 摇到一个角度停多久（最短、最长）。
    pub glance_hold_ms: [u64; 2],
    /// 左右最多摇多少度。
    pub glance_yaw: f64,
    /// 隔多久抖一下耳朵（最短、最长）。
    pub twitch_every_ms: [u64; 2],
    /// 摇头时跟着歪头：左右转一度歪几度。
    pub glance_roll: f64,
}

/// 好了那一屏（第 11 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Done {
    /// 从左边走回中间多久。
    pub walk_ms: u64,
    /// 原地转一圈多久。
    pub spin_ms: u64,
    /// 回车以后散开、走到首页多久。
    pub leave_ms: u64,
}

/// 浮在一步上面的窗（第 21a 条）：大编辑浮窗、示范对话。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Popup {
    /// 占窗口宽度的几成。
    pub width: f64,
    /// 占窗口高度的几成。
    pub height: f64,
    /// 最宽几列。
    pub max_width: u16,
}

/// 整份 `oobe.json`。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 欢迎页的吉祥物。
    pub welcome: Welcome,
    /// 中间几步的版面。
    pub stage: Stage,
    /// 开场。
    pub intro: Intro,
    /// 星点。
    pub stars: Stars,
    /// 换一步滑多久。
    pub slide_ms: u64,
    /// 吉祥物的反应。
    pub react: React,
    /// 吉祥物闲着时。
    pub idle: Idle,
    /// 好了那一屏。
    pub done: Done,
    /// 浮窗。
    pub popup: Popup,
    /// 动着的时候多久画一帧。
    pub frame_ms: u64,
    /// 图标那一步摆出来的几件工具的图标（照 `nerd` 那一套）。
    pub sample_icons: Vec<String>,
    /// 启动时问 `ui.welcomed` 最多等多久不画首页（第 1 条：免得进引导前闪一下）。
    pub welcome_wait_ms: u64,
    /// 「更多供应商…」拿全目录时 `provider.catalog` 的 `limit`（核心不写时只给 50 家）。
    pub catalog_limit: u64,
    /// 建人格时头像的小图几行高（第 21 条）。
    pub avatar_rows: u16,
}
