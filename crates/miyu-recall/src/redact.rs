//! 发出去以前、记下以前把 key 遮掉（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 7 款）：配置里引用的密钥的原文，和
//! 常见的 key 写法（前缀加够长的一串），换成 `[REDACTED]`。写法是数据：资源目录的 `core/memory/secrets.toml`，这里只认
//! 写法、照它遮。纯逻辑。

#[cfg(test)]
mod tests;

use toml_edit::Document;

/// 换进去的那几个字。
pub const REDACTED: &str = "[REDACTED]";

/// 密钥的原文短于这么多的不遮：太短的（`1`、`ok`）会把正常的字遮掉。
const SHORTEST_SECRET: usize = 8;

/// 常见的 key 写法：前缀，和前缀后面至少几个字符。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyShapes {
    /// 前缀，区分大小写（`sk-`、`ghp_`、`AKIA`）。
    pub prefixes: Vec<String>,
    /// 前缀后面至少几个字母、数字、`-`、`_`。
    pub min_tail: usize,
}

impl KeyShapes {
    /// 读 `secrets.toml` 的原文：`prefixes` 是字的列表（不能有空的），`min_tail` 是正整数。
    ///
    /// # Errors
    ///
    /// 读不成 TOML；少了、多了格；写法不对：说是哪一格。
    pub fn parse(text: &str) -> Result<KeyShapes, String> {
        let document = Document::parse(text).map_err(|error| format!("secrets.toml: {error}"))?;
        if let Some((key, _)) = document
            .iter()
            .find(|(key, _)| !["prefixes", "min_tail"].contains(key))
        {
            return Err(format!("secrets.toml: unknown field {key}"));
        }
        let prefixes = document
            .get("prefixes")
            .and_then(|item| item.as_array())
            .ok_or_else(|| "secrets.toml: prefixes must be a list of words".to_string())?
            .iter()
            .map(|value| match value.as_str() {
                Some(prefix) if !prefix.is_empty() => Ok(prefix.to_string()),
                _ => Err("secrets.toml: prefixes must be a list of words".to_string()),
            })
            .collect::<Result<Vec<String>, String>>()?;
        let min_tail = document
            .get("min_tail")
            .and_then(|item| item.as_integer())
            .and_then(|count| usize::try_from(count).ok())
            .filter(|count| *count > 0)
            .ok_or_else(|| "secrets.toml: min_tail must be a positive integer".to_string())?;
        Ok(KeyShapes { prefixes, min_tail })
    }
}

/// 把 `text` 里的 `secrets`（密钥的原文，短于 8 个字节的不管）和照 `shapes` 认得出的 key 换成 [`REDACTED`]。
pub fn redact(text: &str, secrets: &[String], shapes: &KeyShapes) -> String {
    let mut text = text.to_string();
    for secret in secrets
        .iter()
        .filter(|secret| secret.len() >= SHORTEST_SECRET)
    {
        text = text.replace(secret.as_str(), REDACTED);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find(|c: char| is_key_char(c)) {
        out.push_str(&rest[..start]);
        let run = &rest[start..];
        let end = run.find(|c: char| !is_key_char(c)).unwrap_or(run.len());
        let word = &run[..end];
        let looks_like_key = shapes.prefixes.iter().any(|prefix| {
            word.strip_prefix(prefix.as_str())
                .is_some_and(|tail| tail.chars().count() >= shapes.min_tail)
        });
        out.push_str(if looks_like_key { REDACTED } else { word });
        rest = &run[end..];
    }
    out.push_str(rest);
    out
}

/// key 里会有的字符：ASCII 的字母、数字、`-`、`_`。
fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}
