//! 配置键怎么写（`config.md`「键」）：人起的名字里有点、斜杠这些的加引号；判断一个键在不在某张表下面。

/// 一段名字放进键里：只有字母、数字、`-`、`_` 的照写，别的加双引号（里面的 `"`、`\` 转义）。
pub fn segment(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if bare {
        name.to_string()
    } else {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

/// 一家供应商的表：`providers.<编号>`。
pub fn provider(id: &str) -> String {
    format!("providers.{}", segment(id))
}

/// 一个模型的表：`providers.<编号>.models."<模型>"`。
pub fn model_table(provider_id: &str, model: &str) -> String {
    format!("{}.models.{}", provider(provider_id), segment(model))
}

/// 一个池的表：`pools.<名字>`。
pub fn pool(name: &str) -> String {
    format!("pools.{}", segment(name))
}

/// `key` 是 `prefix` 本身，或者在它下面（后面紧跟着 `.`）。
pub fn under(key: &str, prefix: &str) -> bool {
    key == prefix
        || key
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::{model_table, pool, segment, under};

    #[test]
    fn names_with_dots_or_slashes_are_quoted_in_keys() {
        assert_eq!(segment("dev"), "dev");
        assert_eq!(segment("deepseek-v4.1-flash"), "\"deepseek-v4.1-flash\"");
        assert_eq!(
            model_table("relay", "cline/deepseek-v4"),
            "providers.relay.models.\"cline/deepseek-v4\""
        );
        assert_eq!(segment("a\"b"), "\"a\\\"b\"");
        assert_eq!(pool("daily"), "pools.daily");
    }

    #[test]
    fn a_key_is_under_a_table_only_at_a_dot() {
        assert!(under("providers.dev.keys", "providers.dev"));
        assert!(under("providers.dev", "providers.dev"));
        assert!(!under("providers.dev2.keys", "providers.dev"));
    }
}
