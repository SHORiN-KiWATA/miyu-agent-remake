//! 结果写成两份（`23-性能预算.md` 第一节「原始数据入库」）：`.json` 是原始数据（`report/raw.rs`），`.md` 是条件和
//! 对照预算的表。预算改了，照原始数据读回来（`report/load.rs`）重出表，不用再量。表里每一行照哪个预算比、量的是哪一段，写在 `docs/blueprint/perf.md`。

mod gate;
mod load;
mod raw;
mod rows;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::args::Args;
use crate::budget::Budget;
use crate::machine::Machine;
use crate::measure::large::Large;
use crate::measure::sessions::Hot;
use crate::measure::size::Size;
use crate::measure::startup::Cold;

pub use gate::enforce;
pub use load::results as load;
pub use rows::ITEMS;

/// 量到的一切。
pub struct Results {
    /// 什么时候量的：`YYYY-MM-DD`（UTC）。
    pub date: String,
    /// 量的条件。
    pub machine: Machine,
    /// 量了多少次、多大。
    pub args: Args,
    /// 二进制大小。
    pub sizes: Vec<Size>,
    /// 冷启动。
    pub cold: Cold,
    /// 热启动和每多一个会话。
    pub hot: Hot,
    /// 大会话。
    pub large: Large,
    /// 追加并同步，每一条的毫秒数。
    pub appends: Vec<f64>,
}

/// 写进 `out`：`<日期>-<系统>-<架构>.json` 和同名的 `.md`。交回 `.md` 的路径。
///
/// # Errors
///
/// 建不了目录、写不进；预算表里缺了要比的一项。
pub fn write(
    results: &Results,
    budgets: &BTreeMap<String, Budget>,
    out: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("建不了 {}：{e}", out.display()))?;
    let json = out.join(format!("{}.json", name(results)));
    std::fs::write(&json, format!("{}\n", raw::json(results)))
        .map_err(|e| format!("写不进 {}：{e}", json.display()))?;
    write_markdown(results, budgets, out)
}

/// 只写同名的 `.md`：照原始数据重出表时用，原始数据不动。交回它的路径。
///
/// # Errors
///
/// 建不了目录、写不进；预算表里缺了要比的一项。
pub fn write_markdown(
    results: &Results,
    budgets: &BTreeMap<String, Budget>,
    out: &Path,
) -> Result<PathBuf, String> {
    let name = name(results);
    std::fs::create_dir_all(out).map_err(|e| format!("建不了 {}：{e}", out.display()))?;
    let markdown = out.join(format!("{name}.md"));
    std::fs::write(&markdown, rows::markdown(results, budgets, &name)?)
        .map_err(|e| format!("写不进 {}：{e}", markdown.display()))?;
    Ok(markdown)
}

/// 文件名：`<日期>-<系统>-<架构>`。
fn name(results: &Results) -> String {
    format!(
        "{}-{}-{}",
        results.date, results.machine.os, results.machine.arch
    )
}

/// 从 1970-01-01 算起的秒数写成 `YYYY-MM-DD`（UTC，公历）。
pub fn date_of(seconds: u64) -> String {
    // Howard Hinnant 的 civil_from_days：按 400 年一轮算，三月起算一年，闰日落在年末。
    let days = seconds / 86_400 + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests;
