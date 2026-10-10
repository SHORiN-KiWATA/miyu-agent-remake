//! 包带的功能（施工 F-1，设计 `30-插件框架.md` 第三节，`docs/blueprint/packages.md`「格式」）：`[features.<编号>]` 读成
//! [`Feature`]；没写 `[features]` 的内置包、扩展包整个算一个（[`Manifest::features_of`]）。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, line_of};
use super::{Code, Manifest, PackageKind, Problem};
use crate::phrases::Phrases;
use crate::secret::valid_name;

/// 工具名最长多少个字符：同工具目录的（`miyu-tool` 的 `Catalog`）。
const TOOL_CHARS: usize = 64;

/// 一个功能：预设开关的那一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// 编号，写法同包的编号，全局不重（两个包撞了的在 `miyu-store` 认）。
    pub id: String,
    /// 名字。
    pub name: Phrases,
    /// 一句说明；没写的是空的。
    pub summary: Phrases,
    /// 下面的工具，照写的先后；没写的是空的。包只有一个功能的，没列的工具也都归它（F-3）。
    pub tools: Vec<String>,
    /// 编号写在第几行：两个包撞了，报在这一行。照包算的那一个没有。
    pub line: Option<usize>,
}

impl Manifest {
    /// 这个包带的功能，`id` 是包的编号：写了 `[features]` 的照写的（空表就是一个都没有）；没写的内置包、扩展包整个算一个，
    /// 编号、名字、说明照包的；界面、小程序没有。
    pub fn features_of(&self, id: &str) -> Vec<Feature> {
        match (&self.features, self.kind) {
            (Some(features), _) => features.clone(),
            (None, PackageKind::Builtin | PackageKind::Process) => vec![Feature {
                id: id.to_string(),
                name: self.name.clone(),
                summary: self.summary.clone(),
                tools: Vec::new(),
                line: None,
            }],
            (None, PackageKind::Ui | PackageKind::Worker) => Vec::new(),
        }
    }
}

/// `[features]`：一个功能一张表，照写的先后；同一个包里一件工具只能列一次。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<Feature>, Problem> {
    let Some(table) = node.as_table_like() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "features",
            "features must be a table".to_string(),
        ));
    };
    let mut features: Vec<Feature> = Vec::new();
    for (id, item) in table.iter() {
        if !valid_name(id) {
            return Err(reader.problem(
                Some(item),
                Code::BadFeature,
                id,
                format!(
                    "feature {id:?} must start with a lowercase letter and use only lowercase letters, digits, - and _"
                ),
            ));
        }
        let Some(fields) = item.as_table_like() else {
            return Err(reader.problem(
                Some(item),
                Code::NotATable,
                &format!("features.{id}"),
                format!("features.{id} must be a table"),
            ));
        };
        let feature = one(reader, id, fields, item)?;
        if let Some(tool) = feature
            .tools
            .iter()
            .find(|tool| features.iter().any(|seen| seen.tools.contains(tool)))
        {
            return Err(listed_twice(reader, fields, tool));
        }
        features.push(feature);
    }
    Ok(features)
}

/// 一个功能的表：`name` 必写，`summary`、`tools` 可以不写。
fn one(
    reader: &Reader<'_>,
    id: &str,
    fields: &dyn TableLike,
    at: &Item,
) -> Result<Feature, Problem> {
    let path = format!("features.{id}");
    reader.only(fields, &path, &["name", "summary", "tools"])?;
    let name = reader.phrases(
        reader.required(fields, at, &path, "name")?,
        &format!("{path}.name"),
    )?;
    let summary = match fields.get("summary") {
        Some(item) => reader.phrases(item, &format!("{path}.summary"))?,
        None => Phrases::new(),
    };
    let tools = reader.texts(fields, &path, "tools")?;
    let mut seen: Vec<&String> = Vec::new();
    for tool in &tools {
        if !tool_name(tool) {
            return Err(reader.problem(
                fields.get("tools"),
                Code::BadTool,
                tool,
                format!("{path}.tools: {tool:?} must be 1 to 64 ASCII letters, digits, _ or -"),
            ));
        }
        if seen.contains(&tool) {
            return Err(listed_twice(reader, fields, tool));
        }
        seen.push(tool);
    }
    Ok(Feature {
        id: id.to_string(),
        name,
        summary,
        tools,
        line: at.span().map(|span| line_of(reader.text, span.start)),
    })
}

/// 一件工具在这个包里列了两次：报在后列的那一格。
fn listed_twice(reader: &Reader<'_>, fields: &dyn TableLike, tool: &str) -> Problem {
    reader.problem(
        fields.get("tools"),
        Code::BadTool,
        tool,
        format!("tool {tool:?} is listed twice in this package"),
    )
}

/// 工具名：英文字母、数字、`_`、`-`，1 到 64 个字符（同工具目录登记时查的）。
fn tool_name(name: &str) -> bool {
    (1..=TOOL_CHARS).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[cfg(test)]
mod tests;
