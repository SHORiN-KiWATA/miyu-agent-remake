//! 配置清单（`docs/blueprint/config.md`「怎么走」第一条，施工 8-1）：各模块在自己的 crate 里声明自己的几项，这里
//! 登记成一张表；核心起来时照它生成两份 JSON Schema 和参考文件，放在 `state/config/`。
//!
//! 生成的三份是派生的：一样的不重写，写不成的记一条 `WARN config schema not written`，照样起来，缺了只是编辑器没有
//! 补全。字照管理员的 `ui.language` 的最终值；这一步还不读配置，最终值就是默认值 `auto`，照核心所在系统的语言。

use miyu_config::{Item, Layer, Missing, Values, Words, reference, schema};
use miyu_endpoint::settings::UiSettings;
use miyu_log::settings::LogSettings;
use miyu_store::generated;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 运行日志的目标（`config.md`「出错」）。
const TARGET: &str = "miyu::config";

/// 登记的模块，照这个先后，一个模块里照声明的先后。加一个模块只加一行。
const MODULES: [&[Item]; 2] = [UiSettings::ITEMS, LogSettings::ITEMS];

/// 生成的三份放在状态区的这个目录里：`state/config/`。
const DIR: &str = "config";

/// 登记的全部配置项。核心起来时合成一次，之后不变。
pub fn items() -> Vec<Item> {
    MODULES
        .iter()
        .flat_map(|items| items.iter().cloned())
        .collect()
}

/// 生成的三份的文件名，和 [`render`] 交回的先后一样：系统配置的 Schema、个人设置的 Schema、参考文件。
pub const FILES: [&str; 3] = [
    "config.schema.json",
    "settings.schema.json",
    "reference.toml",
];

/// 照清单 `items`、字 `words` 生成的三份，先后照 [`FILES`]。系统配置的 Schema 只有能放进系统配置的项，个人设置的
/// 同理，参考文件是全部。
pub fn render(items: &[Item], words: &dyn Words) -> [Result<String, Missing>; 3] {
    [
        schema::render(items, Layer::System, words),
        schema::render(items, Layer::Personal, words),
        reference::render(items, words),
    ]
}

/// 核心起来时写生成的三份：字照 `ui.language` 的最终值，`auto` 的照系统的语言 `locale`。写不成的一份记一条 `WARN`，
/// 不影响起不起得来。
pub fn generate(root: &DataRoot, resources: &ResourceRoot, locale: Option<&str>) {
    let items = items();
    let ui = UiSettings::from(&Values::defaults(&items));
    let texts = match Human::load(resources, ui.language_for(locale)) {
        Ok(words) => render(&items, &words).map(|text| text.map_err(|error| error.to_string())),
        Err(error) => FILES.map(|_| Err(error.to_string())),
    };
    let dir = root.state().join(DIR);
    for (name, text) in FILES.into_iter().zip(texts) {
        let written = text.and_then(|text| {
            generated::write(&dir.join(name), text.as_bytes()).map_err(|error| error.to_string())
        });
        if let Err(error) = written {
            tracing::warn!(
                target: TARGET,
                file = %format!("state/{DIR}/{name}"),
                error = %error,
                "config schema not written"
            );
        }
    }
}

#[cfg(test)]
mod tests;
