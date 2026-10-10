//! 吉祥物包的模型（蓝图 `tui.md`「吉祥物包」第 4 条，2026-10-11 项目主人定长相、颜色、小动作都能改）：格式同内置的
//! `resources/mascot.json`，每一格都可以不写，没写的照内置的；`face`、`mouth`、`idle`、`gaze` 里一格一格地盖。
//! 终端格子的长宽比（`cell_aspect`）、被顶上去走下来的节拍（`perch`）不归包管。写错的整份不用，交回第一处错（英文短句，
//! 进运行日志）。

use serde_json::Value;

use super::Look;

/// 内置的那份。
const BUILTIN: &str = include_str!("../../resources/mascot.json");

/// 包能写的格。
const ALLOWED: [&str; 13] = [
    "cols",
    "rows",
    "radius",
    "center_row",
    "ramp",
    "light",
    "ambient",
    "shapes",
    "face",
    "mouth",
    "idle",
    "gaze",
    "colors",
];

/// 一格一格地盖的。
const MERGED: [&str; 4] = ["face", "mouth", "idle", "gaze"];

/// 最多占几列：放得进侧边栏（`layout.json` 的 `sidebar_width` 减两边）。
const MAX_COLS: u16 = 36;

/// 最多占几行。
const MAX_ROWS: u16 = 18;

/// 最多几个椭球。
const MAX_SHAPES: usize = 32;

/// 模型文件最大几个字节（核心 `miyu check` 也照它查）。
pub const MAX_BYTES: usize = 256 * 1024;

/// 读一份模型：盖在内置的上面、查过的样子，或者第一处错。
pub fn read(bytes: &[u8]) -> Result<Look, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("model is larger than {MAX_BYTES} bytes"));
    }
    let got: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("model is not JSON: {e}"))?;
    let Value::Object(got) = got else {
        return Err("model is not a JSON object".into());
    };
    let mut base: Value = serde_json::from_str(BUILTIN).map_err(|e| e.to_string())?;
    let Some(fields) = base.as_object_mut() else {
        return Err("built-in mascot is not a JSON object".into());
    };
    for (key, value) in got {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(format!("`{key}` cannot be set by a mascot package"));
        }
        match (fields.get_mut(&key), value) {
            (Some(Value::Object(old)), Value::Object(new)) if MERGED.contains(&key.as_str()) => {
                old.extend(new);
            }
            (_, value) => {
                fields.insert(key, value);
            }
        }
    }
    let look: Look = serde_json::from_value(base).map_err(|e| e.to_string())?;
    check(&look)?;
    Ok(look)
}

/// 查上限和画得出来的数。
fn check(look: &Look) -> Result<(), String> {
    if !(1..=MAX_COLS).contains(&look.cols) {
        return Err(format!("`cols` must be 1 to {MAX_COLS}"));
    }
    if !(1..=MAX_ROWS).contains(&look.rows) {
        return Err(format!("`rows` must be 1 to {MAX_ROWS}"));
    }
    if look.shapes.is_empty() || look.shapes.len() > MAX_SHAPES {
        return Err(format!("`shapes` must have 1 to {MAX_SHAPES} items"));
    }
    if !(look.radius.is_finite() && look.radius > 0.0) {
        return Err("`radius` must be a positive number".into());
    }
    if look.ramp.chars().count() < 2 {
        return Err("`ramp` needs at least two characters".into());
    }
    let numbers = look
        .shapes
        .iter()
        .flat_map(|s| s.center.iter().chain(&s.radii).chain([&s.tilt, &s.follow]))
        .chain(&look.light)
        .chain([&look.ambient, &look.center_row]);
    if numbers.into_iter().any(|n| !n.is_finite()) {
        return Err("numbers must be finite".into());
    }
    if look
        .shapes
        .iter()
        .any(|s| s.radii.iter().any(|r| *r <= 0.0))
    {
        return Err("`radii` must be positive".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
