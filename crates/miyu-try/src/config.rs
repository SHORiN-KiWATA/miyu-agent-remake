//! key 文件（`docs/construction/3-5-试玩台（补）.md`「我定的」）：仓库外、只有本人能读的一个文件，
//! 三行 `名字=值`：
//!
//! ```text
//! MIYU_TRY_BASE_URL=https://api.deepseek.com
//! MIYU_TRY_MODEL=deepseek-flash
//! MIYU_TRY_KEY=…
//! ```
//!
//! key 不打印：读不了的时候只说是哪个文件、第几行、少了哪一行，不带值。key 怎么存是 M8 的事，
//! 这里只是试玩台用的一个文件。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// key 文件里的三样。
#[derive(Clone, PartialEq, Eq)]
pub struct KeyFile {
    /// 供应商的地址，例如 `https://api.deepseek.com`。
    pub base_url: String,
    /// 默认的模型，`--model` 可以换。
    pub model: String,
    /// key。
    pub key: String,
}

/// 打印出来 key 是 `***`。
impl fmt::Debug for KeyFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyFile")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &"***")
            .finish()
    }
}

/// key 文件读不了。
#[derive(Debug)]
pub enum KeyFileError {
    /// 读不到这个文件。
    Read(PathBuf, io::Error),
    /// 第几行不是 `名字=值`，也不是空行、`#` 开头的注释。
    Malformed(PathBuf, usize),
    /// 少了哪一行，或者那一行是空的。
    Missing(PathBuf, &'static str),
}

impl fmt::Display for KeyFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyFileError::Read(path, error) => {
                write!(f, "读不到 key 文件 {}：{error}", path.display())
            }
            KeyFileError::Malformed(path, line) => write!(
                f,
                "key 文件 {} 的第 {line} 行不是「名字=值」",
                path.display()
            ),
            KeyFileError::Missing(path, name) => {
                write!(f, "key 文件 {} 里没有 {name}，或者它是空的", path.display())
            }
        }
    }
}

impl std::error::Error for KeyFileError {}

/// 三行的名字。
const BASE_URL: &str = "MIYU_TRY_BASE_URL";
const MODEL: &str = "MIYU_TRY_MODEL";
const KEY: &str = "MIYU_TRY_KEY";

impl KeyFile {
    /// 默认的位置：家目录下的 `.config/miyu-try/deepseek.env`，三个平台一样。找不到家目录的，没有。
    pub fn default_path() -> Option<PathBuf> {
        std::env::home_dir().map(|home| home.join(".config").join("miyu-try").join("deepseek.env"))
    }

    /// 读 `path`。
    ///
    /// # Errors
    ///
    /// 读不到文件；有一行写法不对；三行里少了一行。
    pub fn read(path: &Path) -> Result<KeyFile, KeyFileError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| KeyFileError::Read(path.into(), error))?;
        KeyFile::parse(&text).map_err(|problem| match problem {
            Problem::Malformed(line) => KeyFileError::Malformed(path.into(), line),
            Problem::Missing(name) => KeyFileError::Missing(path.into(), name),
        })
    }

    /// 照文本读：空行、`#` 开头的注释跳过，别的名字不认识的也跳过；Windows 的换行照样认。
    fn parse(text: &str) -> Result<KeyFile, Problem> {
        let (mut base_url, mut model, mut key) = (None, None, None);
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let (name, value) = line.split_once('=').ok_or(Problem::Malformed(number + 1))?;
            let value = Some(value.trim().to_string()).filter(|value| !value.is_empty());
            match name.trim() {
                BASE_URL => base_url = value,
                MODEL => model = value,
                KEY => key = value,
                _ => {}
            }
        }
        Ok(KeyFile {
            base_url: base_url.ok_or(Problem::Missing(BASE_URL))?,
            model: model.ok_or(Problem::Missing(MODEL))?,
            key: key.ok_or(Problem::Missing(KEY))?,
        })
    }
}

/// 照文本读出的毛病，还没带上是哪个文件。
#[derive(Debug, PartialEq, Eq)]
enum Problem {
    Malformed(usize),
    Missing(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_lines_are_read_and_the_rest_skipped() {
        let text = "# 试玩台\r\nMIYU_TRY_BASE_URL=https://api.example.com\r\n\r\nOTHER=1\r\nMIYU_TRY_MODEL= flash \r\nMIYU_TRY_KEY=sk-a=b\r\n";
        let file = KeyFile::parse(text).unwrap();
        assert_eq!(file.base_url, "https://api.example.com");
        assert_eq!(file.model, "flash");
        assert_eq!(file.key, "sk-a=b", "值里的等号照留");
    }

    #[test]
    fn a_missing_or_empty_line_is_named() {
        let text =
            "MIYU_TRY_BASE_URL=https://api.example.com\nMIYU_TRY_MODEL=flash\nMIYU_TRY_KEY=\n";
        assert_eq!(KeyFile::parse(text), Err(Problem::Missing(KEY)));
        assert_eq!(
            KeyFile::parse("MIYU_TRY_KEY=sk\nno equals sign\n"),
            Err(Problem::Malformed(2))
        );
    }

    #[test]
    fn the_key_is_never_printed() {
        let file = KeyFile {
            base_url: "https://api.example.com".to_string(),
            model: "flash".to_string(),
            key: "sk-secret".to_string(),
        };
        assert!(!format!("{file:?}").contains("sk-secret"));
        let error = KeyFileError::Missing(PathBuf::from("/x/deepseek.env"), KEY);
        assert_eq!(
            error.to_string(),
            "key 文件 /x/deepseek.env 里没有 MIYU_TRY_KEY，或者它是空的"
        );
    }
}
