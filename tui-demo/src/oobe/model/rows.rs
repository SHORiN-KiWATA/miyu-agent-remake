//! 选一家那一屏的一行行（「第一次打开的引导」第 16 条）：照 `provider.catalog {"featured": true}` 列常用的几家，
//! `provider.detect` 的 `keys` 对上的标「已找到 key」，配置里有的标「已配好」；本机跑着的服务另一段；最后「更多供应商…」
//! 「自定义…」。

use serde_json::Value;

/// 一家供应商。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provider {
    /// 目录里的编号（`provider.test` 的 `catalog`）。
    pub catalog: String,
    /// 名字，照核心给的。
    pub name: String,
    /// 核心的环境里找到的 key 的变量名。
    pub env: Option<String>,
    /// 配置里已经有的那一家的编号。
    pub configured: Option<String>,
    /// 本机的服务的地址。
    pub base_url: Option<String>,
    /// 现在接得上（有驱动、有地址）；接不上的暗着、选不了。
    pub supported: bool,
}

impl Provider {
    /// 不用填密钥、选了直接试：配好了的、找到 key 的、本机的。
    pub fn ready(&self) -> bool {
        self.configured.is_some() || self.env.is_some() || self.base_url.is_some()
    }
}

/// 段名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// 本机。
    Local,
    /// 其他。
    Other,
}

/// 一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 段名：选不了。
    Section(Section),
    /// 一家。
    Provider(Provider),
    /// 「更多供应商…」：开浮窗搜全目录（第 16a 条）。
    More,
    /// 「自定义…」。
    Custom,
}

impl Row {
    /// 光标停得上。
    pub fn selectable(&self) -> bool {
        match self {
            Row::Section(_) => false,
            Row::Provider(p) => p.supported,
            Row::More | Row::Custom => true,
        }
    }
}

/// `provider.catalog` 回来的几家，标上 `provider.detect` 找到的 key、配置里有的（「更多供应商…」浮窗也照它读）。
pub fn providers(catalog: &Value, detect: &Value, configured: &[String]) -> Vec<Provider> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    let keys: Vec<&Value> = detect["keys"].as_array().into_iter().flatten().collect();
    catalog["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = text(&p["id"])?;
            let found = keys.iter().find(|k| k["provider"].as_str() == Some(&id));
            let in_config = configured.contains(&id).then(|| id.clone());
            Some(Provider {
                name: text(&p["name"]).unwrap_or_else(|| id.clone()),
                env: found.and_then(|k| text(&k["env"])),
                configured: in_config.or_else(|| found.and_then(|k| text(&k["configured"]))),
                base_url: None,
                supported: p["supported"].as_bool().unwrap_or(false),
                catalog: id,
            })
        })
        .collect()
}

/// 照三份回应排好一行行。`configured` 是 `model.list` 里配置的那几家的编号。
pub fn rows(catalog: &Value, detect: &Value, configured: &[String]) -> Vec<Row> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    let mut out: Vec<Row> = providers(catalog, detect, configured)
        .into_iter()
        .map(Row::Provider)
        .collect();
    let local: Vec<Row> = detect["local"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let id = text(&l["provider"])?;
            Some(Row::Provider(Provider {
                name: text(&l["name"]).unwrap_or_else(|| id.clone()),
                env: None,
                configured: text(&l["configured"]),
                base_url: text(&l["base_url"]),
                supported: true,
                catalog: id,
            }))
        })
        .collect();
    if !local.is_empty() {
        out.push(Row::Section(Section::Local));
        out.extend(local);
    }
    out.push(Row::Section(Section::Other));
    out.push(Row::More);
    out.push(Row::Custom);
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Row, Section, rows};

    #[test]
    fn featured_first_marked_by_found_keys_and_config_then_local_then_custom() {
        let catalog = json!({"providers": [
            {"id": "deepseek", "name": "DeepSeek", "supported": true},
            {"id": "anthropic", "name": "Anthropic", "supported": false},
            {"id": "opencode", "name": "opencode Zen", "supported": true}]});
        let detect = json!({
            "keys": [{"env": "DEEPSEEK_API_KEY", "provider": "deepseek", "supported": true}],
            "local": [{"provider": "ollama", "name": "Ollama", "base_url": "http://127.0.0.1:11434/v1", "models": []}]});
        let got = rows(&catalog, &detect, &["opencode".to_string()]);
        let Row::Provider(deepseek) = &got[0] else {
            panic!("{got:?}")
        };
        assert_eq!(deepseek.env.as_deref(), Some("DEEPSEEK_API_KEY"));
        assert!(deepseek.ready() && got[0].selectable());
        assert!(!got[1].selectable(), "接不上的选不了");
        let Row::Provider(zen) = &got[2] else {
            panic!()
        };
        assert_eq!(zen.configured.as_deref(), Some("opencode"));
        assert_eq!(got[3], Row::Section(Section::Local));
        let Row::Provider(ollama) = &got[4] else {
            panic!()
        };
        assert!(ollama.ready() && ollama.base_url.is_some());
        assert_eq!(
            got[5..],
            [Row::Section(Section::Other), Row::More, Row::Custom]
        );
        let bare = rows(&catalog, &json!({}), &[]);
        assert!(
            !bare.contains(&Row::Section(Section::Local)),
            "本机没有服务就不列这一段"
        );
    }
}
