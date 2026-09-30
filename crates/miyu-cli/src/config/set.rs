//! `miyu config set <键> <值>`、`unset <键>`（`config.md` 第十条第 4、5 条，施工 8-3）：`config.set`，`layer` 照 `--system` 是
//! `system`，不写是 `personal`；不带 `expect`（后写的算，第五条第 11 条）。
//!
//! - `set` 把人敲的值原样交给核心（`input`），由它照这一项的类型读。成了：标准错误上印一行灰字，写进了哪一层、什么时候
//!   生效；上面一层压着的说用的还是哪一层写的。这一层本来就是的说没改。
//! - `unset`：成了说现在是哪一层的什么；这一层本来就没写的说一句，退出码 0。
//! - `set --project`：项目配置只能手改，参数不对，退出码 2：连核心以前就拦下了（`config.rs` 的 `early`）。

use serde_json::{Value, json};

use super::Talk;
use super::render::toml;
use crate::exit;
use crate::shown::{Line, write};

/// `set`：改一项，交回退出码。
pub(super) async fn set(talk: &mut Talk<'_>, key: &str, value: &str, system: bool) -> u8 {
    let language = talk.plan.language;
    let layer = layer(system);
    let params = json!({"layer": layer, "changes": [{"key": key, "input": value}]});
    let result = match talk.ask("config.set", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let said = match result["keys"].get(key) {
        Some(entry) => {
            let value = toml(&entry["value"]);
            let effective = toml(&entry["effective"]);
            match entry["origin"]["layer"].as_str() {
                Some(above) if above != layer => {
                    language.saved_below(key, &value, layer, above, &effective)
                }
                _ => {
                    let applies = entry["applies"].as_str().unwrap_or_default();
                    language.saved(key, &value, layer, applies)
                }
            }
        }
        None => {
            let params = json!({"keys": [key], "all": true});
            let got = match talk.ask("config.get", params).await {
                Ok(result) => result,
                Err(code) => return code,
            };
            language.already(&toml(&written(&got["items"][key], layer)))
        }
    };
    gray(talk, said);
    exit::OK
}

/// `unset`：从这一层删掉一项，交回退出码。
pub(super) async fn unset(talk: &mut Talk<'_>, key: &str, system: bool) -> u8 {
    let language = talk.plan.language;
    let layer = layer(system);
    let params = json!({"layer": layer, "changes": [{"key": key, "unset": true}]});
    let result = match talk.ask("config.set", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let said = match result["keys"].get(key) {
        Some(entry) => language.removed(
            key,
            layer,
            &toml(&entry["effective"]),
            entry["origin"]["layer"].as_str().unwrap_or("default"),
        ),
        None => language.not_there(key, layer),
    };
    gray(talk, said);
    exit::OK
}

/// 改哪一层。
fn layer(system: bool) -> &'static str {
    match system {
        true => "system",
        false => "personal",
    }
}

/// `config.get` 带 `all` 的一项里 `layer` 那一层写的值；没有的照最终值。
fn written(item: &Value, layer: &str) -> Value {
    item["layers"]
        .as_array()
        .and_then(|layers| {
            layers
                .iter()
                .find(|row| row["origin"]["layer"] == layer)
                .map(|row| row["value"].clone())
        })
        .unwrap_or_else(|| item["value"].clone())
}

/// 标准错误上一行灰字。
fn gray(talk: &mut Talk<'_>, said: String) {
    write(talk.err, &Line::gray(said).paint(talk.plan.gray));
}
