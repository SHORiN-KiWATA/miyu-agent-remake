//! 哪些模型能出向量（施工 R-5 再补，`docs/blueprint/models.md`「协议」`model.list` 的「能出向量的」）：设置页的「语义模型」
//! 从这几个里挑。目录不标 embedding 的输出种类，照名字认；规矩是数据，住在资源目录的 `models/embedding.toml`，核心读好原文
//! 交进来，这里只认写法、照它认。

use toml_edit::Document;

/// 认得的三格。
const FIELDS: [&str; 3] = ["contains", "starts", "except"];

/// 认能出向量的模型的规矩。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbeddingNames {
    /// 模型名、`family` 含其中一个的算。
    pub contains: Vec<String>,
    /// 模型名最后一段（`/` 后面）、`family` 以其中一个开头的算。
    pub starts: Vec<String>,
    /// 含其中一个的不算，压过上面两条。
    pub except: Vec<String>,
}

impl EmbeddingNames {
    /// 读 `embedding.toml` 的原文：三格都是字的列表，可以不写；字转成小写。
    ///
    /// # Errors
    ///
    /// 读不成 TOML；有别的格；哪一格不是字的列表、有空的字：说是哪一格。
    pub fn parse(text: &str) -> Result<EmbeddingNames, String> {
        let document = Document::parse(text).map_err(|error| format!("embedding.toml: {error}"))?;
        if let Some((key, _)) = document.iter().find(|(key, _)| !FIELDS.contains(key)) {
            return Err(format!("embedding.toml: unknown field {key}"));
        }
        let words = |key: &str| -> Result<Vec<String>, String> {
            let Some(item) = document.get(key) else {
                return Ok(Vec::new());
            };
            let wrong = || format!("embedding.toml: {key} must be a list of words");
            let list = item.as_array().ok_or_else(wrong)?;
            list.iter()
                .map(|value| match value.as_str() {
                    Some(word) if !word.trim().is_empty() => Ok(word.to_lowercase()),
                    _ => Err(wrong()),
                })
                .collect()
        };
        Ok(EmbeddingNames {
            contains: words("contains")?,
            starts: words("starts")?,
            except: words("except")?,
        })
    }

    /// 模型 `model`（目录里对上的条目的 `family` 是 `family`）能不能出向量。
    pub fn matches(&self, model: &str, family: Option<&str>) -> bool {
        let model = model.to_lowercase();
        let family = family.unwrap_or_default().to_lowercase();
        let last = model.rsplit('/').next().unwrap_or_default();
        let has = |words: &[String]| {
            words
                .iter()
                .any(|word| model.contains(word.as_str()) || family.contains(word.as_str()))
        };
        let begins = self
            .starts
            .iter()
            .any(|word| last.starts_with(word.as_str()) || family.starts_with(word.as_str()));
        !has(&self.except) && (has(&self.contains) || begins)
    }
}

#[cfg(test)]
mod tests;
