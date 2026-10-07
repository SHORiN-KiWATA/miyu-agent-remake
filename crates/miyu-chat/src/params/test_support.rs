//! 测试共用的几样：仓库里的出厂文件（`include_str!` 读进来：数据改了，测试跟着变），包成一份 [`File`]、读成 [`Params`]。

use crate::Params;
use crate::rules::{File, Source};

/// 仓库里的出厂文件。
pub(crate) const DEFAULTS: &str =
    include_str!("../../../../resources/software/onebot/defaults.toml");

/// 出厂文件的名字：问题里的 `file` 照交进来的。
pub(crate) const NAME: &str = "defaults.toml";

/// 一份出厂文件，字是 `text`。
pub(crate) fn file(text: &str) -> File {
    File {
        source: Source::Factory,
        name: NAME.to_string(),
        text: text.to_string(),
    }
}

/// 仓库里的出厂文件读出来的参数，断定读得出。
pub(crate) fn defaults() -> Params {
    Params::read(&file(DEFAULTS)).expect("仓库里的出厂文件读得出")
}
