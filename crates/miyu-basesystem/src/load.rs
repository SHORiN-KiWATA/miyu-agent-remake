//! 从资源目录读工具的字（`26-提示词.md` 第八节）：`software/basesystem/tools/<工具>.json` 是给模型看的说明和
//! 参数格式，`software/basesystem/<工具>/<名字>.txt` 是输出里给她看的几句。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use miyu_kernel::raw::RawJson;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::Spec;

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

/// 读工具 `name` 的说明和参数格式，访问类别是 `access`。
pub(crate) fn spec(resources: &Path, name: &str, access: Access) -> Result<Spec, LoadError> {
    let file = resources
        .join("software")
        .join("basesystem")
        .join("tools")
        .join(format!("{name}.json"));
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

/// 读工具 `tool` 输出里的一句 `name`，拿 `fields` 里的每个字段试换一次。
pub(crate) fn text(
    resources: &Path,
    tool: &str,
    name: &str,
    fields: &[&str],
) -> Result<Template, LoadError> {
    let file = resources
        .join("software")
        .join("basesystem")
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

/// 读一份文件。
fn read(file: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(file).map_err(|error| LoadError {
        file: file.to_path_buf(),
        why: error.to_string(),
    })
}
