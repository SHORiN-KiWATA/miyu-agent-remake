//! `config.schema` 里选项的两样另说的（施工 R-5 再补、三补，`docs/blueprint/config.md`「协议」）：后面暗字写什么（`note`）、现在
//! 用不用得了（`available`）。只有核心查得出的才有，不进给人看的字；现在只有语义模型的「内置模型」。

use miyu_session::EMBED_LOCAL;

/// 语义模型那一项的键（`miyu_models::settings::UseSettings` 的 `embedding`）。
const EMBEDDING: &str = "models.embedding";

/// 选项后面暗字写的（施工 R-5 再补，`config.md`「协议」）：只有核心查得出的才有，不进给人看的字。现在只有语义模型
/// （`UseSettings` 的 `embedding`）的「内置模型」：本机清单的模型名 `local`，本机的那一路用不上的没有。
pub(super) fn note(key: &str, option: &str, local: Option<&str>) -> Option<String> {
    match (key, option) {
        (EMBEDDING, EMBED_LOCAL) => local.map(str::to_string),
        _ => None,
    }
}

/// 选项现在用不用得了（施工 R-5 三补，`config.md`「协议」）：只有核心查得出用不了的才说用不了。现在只有语义模型的「内置模型」：
/// 本机的那一路没有（没装内置模型那个小程序包、程序不在、模型清单读不了）的用不了，头画成灰的、选不了。
pub(super) fn usable(key: &str, option: &str, local: bool) -> bool {
    !matches!((key, option), (EMBEDDING, EMBED_LOCAL)) || local
}
