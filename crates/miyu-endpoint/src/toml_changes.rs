//! 没有清单的 TOML 文件（预设、人格的 `persona.toml`）照 `changes` 改（施工 P-3 中、下）：参数照 `config.set` 的 `changes`
//! 查，`expect` 照这一层现在的值比（数字照数值比），一项项在原来的字上改、只动那一项（`miyu_config::edit::apply`）。

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{Map, Value as Json, json};

use miyu_config::Value;
use miyu_config::edit::{self, Change};
use miyu_config::plain;

use crate::config::set::same;
use crate::refusal::Refusal;

/// `changes` 的一项：`value`、`unset` 正好写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChangeParams {
    key: String,
    #[serde(default)]
    value: Option<Json>,
    #[serde(default)]
    unset: Option<bool>,
    #[serde(default)]
    expect: Option<Map<String, Json>>,
}

/// 查过参数的一项：改成的值（`None` 是删掉），和 `expect`（外面一层是写没写，里面是「这一项现在应当没写」）。
pub(crate) struct Wanted {
    key: String,
    value: Option<Value>,
    expect: Option<Option<Json>>,
}

impl Wanted {
    /// 这一项是写一个值（不是删）：新建的至少要有一项（施工 P-3 补）。
    pub(crate) fn writes(&self) -> bool {
        self.value.is_some()
    }
}

/// 查参数：同一个键不写两次，`value`、`unset` 正好一个，值是字、开关、数，`expect` 是 `{"value": …}` 或 `{}`。空的由调用的
/// 一方判。
pub(crate) fn wanted(changes: Vec<ChangeParams>) -> Result<Vec<Wanted>, Refusal> {
    let mut keys = BTreeSet::new();
    changes
        .into_iter()
        .map(|change| {
            if !keys.insert(change.key.clone()) {
                return Err(Refusal::BAD_PARAMS);
            }
            let value = match (change.value, change.unset) {
                (Some(value), None) => Some(plain::from_json(&value).ok_or(Refusal::BAD_PARAMS)?),
                (None, Some(true)) => None,
                _ => return Err(Refusal::BAD_PARAMS),
            };
            let expect = match change.expect {
                None => None,
                Some(expect) if expect.is_empty() => Some(None),
                Some(mut expect) if expect.len() == 1 => {
                    Some(Some(expect.remove("value").ok_or(Refusal::BAD_PARAMS)?))
                }
                Some(_) => return Err(Refusal::BAD_PARAMS),
            };
            Ok(Wanted {
                key: change.key,
                value,
                expect,
            })
        })
        .collect()
}

/// 每一项的 `expect` 照 `text`（这一层现在的字）比：对不上的拒绝，原因码是 `conflict`，`data.current` 是这一项现在的样子。
pub(crate) fn check_expect(
    text: &str,
    wanted: &[Wanted],
    conflict: &'static str,
) -> Result<(), Refusal> {
    for want in wanted {
        if let Some(expected) = &want.expect {
            let now = plain::json_at(text, &want.key);
            if !same(now.as_ref(), expected.as_ref()) {
                let shown = now.map_or_else(|| json!({}), |value| json!({ "value": value }));
                return Err(Refusal::conflict(conflict, Some(shown)));
            }
        }
    }
    Ok(())
}

/// 去掉 `text` 里 P-3 上写进去的「以谁为底」那一格（施工 P-3 再补）：`table` 是 `persona`、`preset`。没有的、去不掉的照原样。
pub(crate) fn without_base(text: &str, table: &str) -> String {
    let key = format!("{table}.{}", miyu_policy::persona::BASE);
    edit::apply(text, Change::Unset(&key)).unwrap_or_else(|_| text.to_string())
}

/// 在 `text` 上一项项改：这一层本来就是这个值的不动（写法不同的也不动，例如单引号）；本来就没写又要删的，`edit::apply` 自己
/// 不动。放不进去的（那一组写成了别的东西）照 `invalid` 拒绝，说是 `file` 的哪一项。
pub(crate) fn edited(
    text: &str,
    wanted: &[Wanted],
    file: &str,
    invalid: fn(String) -> Refusal,
) -> Result<String, Refusal> {
    let mut edited = text.to_string();
    for want in wanted {
        let now = plain::json_at(&edited, &want.key);
        let change = match &want.value {
            Some(value) if now.as_ref() == Some(&value.json()) => continue,
            Some(value) => Change::Set(&want.key, value),
            None => Change::Unset(&want.key),
        };
        edited = edit::apply(&edited, change)
            .map_err(|blocked| invalid(format!("{file}: cannot write {}", blocked.0)))?;
    }
    Ok(edited)
}
