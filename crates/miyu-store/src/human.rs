//! 给人看的字（`docs/designs/26-提示词.md` 第三节「写法」里的「双槽」、第八节，施工 4-5 上）。
//!
//! 内核和每个软件包各有一份 `human/<语言>.json`：内核的在 `core/human/`，软件包的在 `software/<软件包>/human/`。
//! 每份里两样：
//!
//! - `tools`：每件工具给人看的显示名 `name`，显示名后面跟哪一个参数的值 `subject`，写在最前面的符号 `icon`，标题
//!   下面还印一块什么 `block`（施工 4-11）；
//! - `said`：每一种说法的字，模板照 `{字段}` 写，编号照这一份所在的地方往下写，例如内核那一份里的
//!   `tool-results/unattended` 就是说法 `core/tool-results/unattended`。
//!
//! 头照一次调用的说法（`tool.result` 的 `human`）换成字。这些字不进请求，所以换进去的字段不转义成 JSON 的样子，
//! 只把控制字符换成 `�`（[`clean`]）：路径、参数是她给的，里面要是混着终端的控制序列，原样印出来会把终端弄乱。
//!
//! 哪一份没有这种语言，照英文那一份；英文也没有的，那一处就没有给人看的字，头照工具名、状态写最泛的。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;

use crate::resources::ResourceRoot;

/// 找不到别的语言时用的那一种。
pub const FALLBACK: &str = "en";

/// 读好的一种语言的字。
#[derive(Debug, Clone, Default)]
pub struct Human {
    tools: BTreeMap<String, Face>,
    said: BTreeMap<String, Template>,
}

/// 一件工具给人看的样子。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    /// 显示名，例如「读取」。
    pub name: String,
    /// 显示名后面跟哪一个参数的值，例如 `file_path`。没有的只写显示名。
    #[serde(default)]
    pub subject: Option<String>,
    /// 写在最前面的符号，例如 `→`（施工 4-11）。没有的由头定。
    #[serde(default)]
    pub icon: Option<String>,
    /// 标题下面还印一块什么（施工 4-11）。没有的只印标题。
    #[serde(default)]
    pub block: Option<Block>,
}

/// 标题下面的那一块。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    /// 执行命令：标题写成 `$ 命令`，不写显示名；下面印工具自己写的结果。
    Command,
    /// 改动：下面印参数 `edits` 里每一处改掉的、改成的。
    Edits,
}

/// `human/<语言>.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    tools: BTreeMap<String, Face>,
    #[serde(default)]
    said: BTreeMap<String, String>,
}

/// 一份给人看的字读不懂。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanError {
    /// 哪一份。
    pub file: PathBuf,
    /// 为什么。
    pub why: String,
}

impl fmt::Display for HumanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for HumanError {}

impl Human {
    /// 照资源目录 `root` 读 `language` 这一种：内核的一份，每个软件包各一份。
    ///
    /// # Errors
    ///
    /// 有一份读得到、却读不懂（不是 JSON、写法不对、模板坏了），说是哪一份。读不到的不算错：没有这种语言的照
    /// 英文，英文也没有的，那一处就没有字。
    pub fn load(root: &ResourceRoot, language: &str) -> Result<Human, HumanError> {
        let mut human = Human::default();
        human.add(&root.path().join("core"), "core", language)?;
        let software = root.path().join("software");
        let mut packages: Vec<String> = match std::fs::read_dir(&software) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        };
        packages.sort();
        for package in packages {
            human.add(
                &software.join(&package),
                &format!("software/{package}"),
                language,
            )?;
        }
        Ok(human)
    }

    /// 读 `dir` 下 `human/` 里 `language` 那一份，没有就读英文那一份；说法的编号前面加上 `prefix`。
    fn add(&mut self, dir: &Path, prefix: &str, language: &str) -> Result<(), HumanError> {
        let found = [language, FALLBACK].iter().find_map(|language| {
            let file = dir.join("human").join(format!("{language}.json"));
            std::fs::read_to_string(&file).ok().map(|text| (file, text))
        });
        let Some((file, text)) = found else {
            return Ok(());
        };
        let bad = |why: String| HumanError {
            file: file.clone(),
            why,
        };
        let parsed: File = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
        for (key, source) in parsed.said {
            let template =
                Template::parse(&source).map_err(|error| bad(format!("{key}: {error}")))?;
            self.said.insert(format!("{prefix}/{key}"), template);
        }
        self.tools.extend(parsed.tools);
        Ok(())
    }

    /// 叫 `name` 的工具给人看的样子；没有的是空的。
    pub fn tool(&self, name: &str) -> Option<&Face> {
        self.tools.get(name)
    }

    /// 照说法换成一句话，字段先过一遍 [`clean`]。没有这一句、或者少了字段的，是空的。
    pub fn say(&self, said: &Said) -> Option<String> {
        let template = self.said.get(&said.key)?;
        let fields: BTreeMap<&str, &str> = said
            .fields
            .iter()
            .map(|(field, value)| (field.as_str(), value.as_str()))
            .collect();
        template.fill(&fields, clean).ok()
    }

    /// 说法 `key` 这一句要哪些字段；没有这一句的是空的。
    pub fn fields(&self, key: &str) -> Option<Vec<&str>> {
        self.said.get(key).map(Template::fields)
    }
}

/// 给人看的字段：控制字符换成 `�`，别的照原样。
pub fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}
