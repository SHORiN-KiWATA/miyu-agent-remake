//! 从资源目录读工具的字（`26-提示词.md` 第八节）：`software/<软件包>/tools/<工具>.json` 是给模型看的说明和
//! 参数格式，`software/<软件包>/<工具>/<名字>.txt` 是输出里给她看的几句（施工 4-4 下）。原来在基础系统里、软件包名写死，
//! 施工 R-3 中挪到这里、多一个软件包名：记忆这个软件包照同一套读（同一件事不写两遍）。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Spec;
use miyu_kernel::raw::RawJson;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;

/// 一份字读不出来，或者写法不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// 哪一份。
    pub file: PathBuf,
    /// 为什么。
    pub why: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for LoadError {}

/// `tools/<工具>.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecFile {
    description: String,
    parameters: RawJson,
}

/// 读软件包 `package` 的工具 `name` 的说明和参数格式，访问类别是 `access`。
///
/// # Errors
///
/// 读不出来；不是 `{description, parameters}` 的样子。
pub fn spec(
    resources: &Path,
    package: &str,
    name: &str,
    access: Access,
) -> Result<Spec, LoadError> {
    let file = spec_file(resources, package, name);
    let text = read(&file)?;
    let parsed: SpecFile = serde_json::from_str(&text).map_err(|error| LoadError {
        file: file.clone(),
        why: error.to_string(),
    })?;
    Ok(Spec {
        name: name.to_string(),
        description: parsed.description,
        parameters: parsed.parameters,
        access,
    })
}

/// 同 [`spec`]，说明是一段模板，换进 `fields`：说明里要写这台机器上的东西的（`shell` 写用的是哪种 shell，施工
/// 4-8）。核心起来时换一次，会话里不变。别的工具的说明不当模板读：里面的花括号是字面的。
///
/// # Errors
///
/// 同 [`spec`]；说明不是合写法的模板，或者要了没给的字段。
pub fn spec_filled(
    resources: &Path,
    package: &str,
    name: &str,
    access: Access,
    fields: &[(&str, &str)],
) -> Result<Spec, LoadError> {
    let mut spec = spec(resources, package, name, access)?;
    let bad = |why: String| LoadError {
        file: spec_file(resources, package, name),
        why,
    };
    let template = Template::parse(&spec.description).map_err(|error| bad(error.to_string()))?;
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    spec.description = template
        .render(&fields)
        .map_err(|error| bad(error.to_string()))?;
    Ok(spec)
}

/// 读软件包 `package` 的工具 `tool` 输出里的一句 `name`，拿 `fields` 里的每个字段试换一次。
///
/// # Errors
///
/// 读不出来；不是合写法的模板，或者要了 `fields` 以外的字段。
pub fn text(
    resources: &Path,
    package: &str,
    tool: &str,
    name: &str,
    fields: &[&str],
) -> Result<Template, LoadError> {
    let file = resources
        .join("software")
        .join(package)
        .join(tool)
        .join(format!("{name}.txt"));
    let source = read(&file)?;
    let bad = |why: String| LoadError {
        file: file.clone(),
        why,
    };
    let template = Template::parse(&source).map_err(|error| bad(error.to_string()))?;
    let trial: BTreeMap<&str, &str> = fields.iter().map(|field| (*field, "")).collect();
    template
        .render(&trial)
        .map_err(|error| bad(error.to_string()))?;
    Ok(template)
}

/// 照模板 `template` 换进字段 `fields`。
///
/// # Panics
///
/// 实际不会：造的时候试换过，字段都有。
pub fn say(template: &Template, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    template.render(&fields).expect("造的时候试换过，字段都有")
}

/// 软件包 `package` 的工具 `name` 的说明在哪。
fn spec_file(resources: &Path, package: &str, name: &str) -> PathBuf {
    resources
        .join("software")
        .join(package)
        .join("tools")
        .join(format!("{name}.json"))
}

/// 读一份文件。
fn read(file: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(file).map_err(|error| LoadError {
        file: file.to_path_buf(),
        why: error.to_string(),
    })
}
