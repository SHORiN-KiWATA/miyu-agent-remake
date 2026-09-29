//! 一张图在 DeepSeek 上算多少 token（施工 6-3 上，`docs/blueprint/compaction.md`「对外的样子」模型的资料）：官方文档
//! 「Token 用量」里图片计算器的 v41 配置，照 dsh（`deepseek-harness`，2026-09-28 的版本）的移植一行行搬过来。
//!
//! 图片总像素小于 544×544 的先放大；补齐成 14 像素一块的网格，每个方向 3 块合成一格；一行末尾多一个分隔，另加两个首尾；
//! 超过 1024 的，解出不超过 1024、保持长宽比的最大网格；反复投影到不再变。算出来的只是估算，真值由供应商报。
//!
//! 数都照 JavaScript 的写法用 `f64` 算，取整的地方和原文一样，才和官方计算器的对照数据一字不差。

use miyu_kernel::estimate::ImagePrice;

/// 一块的边长，像素。
const PATCH_SIZE: f64 = 14.0;
/// 每个方向几块合成一格。
const DOWNSAMPLE_RATIO: f64 = 3.0;
/// 一张图最多多少 token。
const MAX_IMAGE_TOKENS: f64 = 1024.0;
/// 总像素的下限：小的先放大到这么多。
const MIN_PIXELS: f64 = 544.0 * 544.0;
/// 一格覆盖多少像素（每个方向）。
const CELL_SIZE: f64 = PATCH_SIZE * DOWNSAMPLE_RATIO;
/// 反复投影最多几遍：每一遍都是投影，第二遍一样就停，官方的计算器也设了这个上限。
const PASSES: usize = 10;

/// DeepSeek 的图片算法：官方计算器的 v41 配置。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeepSeekImages;

impl ImagePrice for DeepSeekImages {
    fn tokens(&self, width: u32, height: u32) -> u64 {
        deepseek_image_tokens(width, height)
    }
}

/// 一次缩放、补齐、投影的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
struct Grid {
    grid_height: f64,
    grid_width: f64,
    best_height: f64,
    best_width: f64,
    tokens: f64,
}

fn int_div(value: f64, divisor: f64) -> f64 {
    (value / divisor).floor()
}

fn ceil_div(value: f64, divisor: f64) -> f64 {
    ((value + divisor - 1.0) / divisor).floor()
}

/// 一个网格多少 token：每一行多一个分隔，另加两个首尾。
fn grid_tokens(grid_height: f64, grid_width: f64) -> f64 {
    grid_height * (grid_width + 1.0) + 2.0
}

/// 补齐过的一边有几格。
fn grid_cells(padded: f64) -> f64 {
    ceil_div(int_div(padded, PATCH_SIZE), DOWNSAMPLE_RATIO)
}

/// 在 `budget` 个 token 以内，保持长宽比的最大网格。
fn solve(height: f64, width: f64, budget: f64) -> Grid {
    let aspect = height / width;
    let ideal_width = ((budget - 2.0) / aspect + 0.25).sqrt() - 0.5;
    let ideal_height = ideal_width * aspect;
    let (best_width, best_height) = if ideal_width < 1.0 {
        let cells_high = int_div(budget - 2.0, 2.0);
        (CELL_SIZE, cells_high * CELL_SIZE)
    } else if ideal_height < 1.0 {
        let cells_wide = int_div(budget - 2.0, 1.0) - 1.0;
        (cells_wide * CELL_SIZE, CELL_SIZE)
    } else {
        let cells_wide = ideal_width.trunc();
        let cells_high = ideal_height.trunc();
        let scale = (cells_wide * CELL_SIZE / width).min(cells_high * CELL_SIZE / height);
        (
            (width * scale / PATCH_SIZE).trunc() * PATCH_SIZE,
            (height * scale / PATCH_SIZE).trunc() * PATCH_SIZE,
        )
    };
    let grid_height = grid_cells(best_height);
    let grid_width = grid_cells(best_width);
    Grid {
        grid_height,
        grid_width,
        best_height,
        best_width,
        tokens: grid_tokens(grid_height, grid_width),
    }
}

/// 补齐过的尺寸投影到预算以内最大的网格。
fn fit(height: f64, width: f64, padded_height: f64, padded_width: f64) -> Grid {
    let grid_height = grid_cells(padded_height);
    let grid_width = grid_cells(padded_width);
    let direct = Grid {
        grid_height,
        grid_width,
        best_height: padded_height,
        best_width: padded_width,
        tokens: grid_tokens(grid_height, grid_width),
    };
    if direct.tokens <= MAX_IMAGE_TOKENS {
        return direct;
    }
    solve(height, width, MAX_IMAGE_TOKENS)
}

/// 缩放、补齐、投影一遍。
fn once(width: f64, height: f64) -> Grid {
    let (mut width, mut height) = (width, height);
    let pixels = width * height;
    if pixels < MIN_PIXELS && pixels > 0.0 {
        let scale = (MIN_PIXELS / pixels).sqrt();
        width = (width * scale).trunc();
        height = (height * scale).trunc();
    }
    let padded_width = ceil_div(width, PATCH_SIZE) * PATCH_SIZE;
    let padded_height = ceil_div(height, PATCH_SIZE) * PATCH_SIZE;
    fit(height, width, padded_height, padded_width)
}

/// 宽 `width`、高 `height` 的一张图在 DeepSeek 上算多少 token，至多 1024。反复投影不收敛（官方的计算器在这里报错，
/// 实际不会出现）的，照上限算。
pub fn deepseek_image_tokens(width: u32, height: u32) -> u64 {
    let mut grid = once(f64::from(width), f64::from(height));
    for _ in 1..PASSES {
        let next = once(grid.best_width, grid.best_height);
        if next == grid {
            return as_tokens(grid.tokens);
        }
        grid = next;
    }
    as_tokens(MAX_IMAGE_TOKENS)
}

/// 网格的 token 数是不大的非负整数，写成 `u64`。
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "网格的 token 数是 0 到 1024 之间的整数"
)]
fn as_tokens(tokens: f64) -> u64 {
    tokens.clamp(0.0, MAX_IMAGE_TOKENS) as u64
}

#[cfg(test)]
mod tests;
