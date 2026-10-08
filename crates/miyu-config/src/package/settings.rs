//! 清单里的 `[settings]`（施工 9-1 下，`docs/blueprint/packages.md`「配置项」，`05-内核接口.md` 第二节）：一项一张表
//! `[settings.<名字>]`，读成 [`Setting`]；核心起来时照它们拼成配置项（[`items`]），键是 `<包的编号>.<名字>`，挂在设置页的
//! 「软件包」那一页、这个包那一组。
//!
//! 读的时候不留 `'static` 的字：`miyu check` 每次都照磁盘读。只有核心起来时拼配置项那一次把字留在进程里，一直用到退出
//! （配置清单的项是编译期常量的样子，包的配置项装卸要重启，留下的量有界）。

use toml_edit::{Item as Node, TableLike};

use super::reader::Reader;
use super::{Code, Problem};
use crate::item::{Applies, Control, Item, Kind, Layer, Ui};
use crate::parse;
use crate::phrases::Phrases;
use crate::value::Value;

/// 包的配置项都在设置页的这一页。
pub const PAGE: &str = "packages";

/// 文字不写 `max` 的，最多几个字符。
const TEXT_MAX: usize = 200;

/// 读好的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// 名字：键的最后一段。
    pub name: String,
    /// 类型。
    pub kind: SettingKind,
    /// 默认值；没写的没有。
    pub default: Option<Value>,
    /// 能写在哪几层：系统配置、个人设置。
    pub layers: Vec<Layer>,
    /// 什么时候生效；不写是这个程序下次启动时（[`Applies::HeadStart`]）。
    pub applies: Applies,
    /// 给人看的名字。
    pub label: Phrases,
    /// 给人看的说明；没写的是空的。
    pub description: Phrases,
    /// 设置页不画。
    pub hidden: bool,
}

/// 一项的类型：配置清单的几种里包能用的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingKind {
    /// 开关。
    Bool,
    /// 整数，`min` 到 `max`。
    Int {
        /// 最小。
        min: i64,
        /// 最大。
        max: i64,
    },
    /// 选项：列出的几个之一，至少两个。
    Option(Vec<String>),
    /// 文字，最多 `max` 个字符。
    Text {
        /// 最多几个字符。
        max: usize,
    },
    /// 名字。
    Name,
    /// 网址。
    Url,
    /// 密钥的引用：没有默认值。
    Secret,
    /// 列表：每一个照元素的类型，元素不能再是列表（施工 9-1 补）。
    List(Box<SettingKind>),
}

impl SettingKind {
    /// 不带借来的字的那几种，换成配置清单的类型；选项、列表没有（要 `'static` 的字、元素，[`items`] 那一次才换）。
    fn plain(&self) -> Option<Kind> {
        Some(match self {
            SettingKind::Bool => Kind::Bool,
            SettingKind::Int { min, max } => Kind::Int {
                min: *min,
                max: *max,
            },
            SettingKind::Text { max } => Kind::Text { max: *max },
            SettingKind::Name => Kind::Name,
            SettingKind::Url => Kind::Url,
            SettingKind::Secret => Kind::Secret,
            SettingKind::Option(_) | SettingKind::List(_) => return None,
        })
    }

    /// 密钥、密钥的列表：不能写默认值。
    fn holds_secrets(&self) -> bool {
        match self {
            SettingKind::Secret => true,
            SettingKind::List(element) => element.holds_secrets(),
            _ => false,
        }
    }

    /// 设置页用的控件。
    fn control(&self) -> Control {
        match self {
            SettingKind::Bool => Control::Toggle,
            SettingKind::Int { .. } => Control::Number,
            SettingKind::Option(_) => Control::Select,
            SettingKind::List(_) => Control::List,
            _ => Control::Text,
        }
    }
}

/// 读 `[settings]`：照写的先后。
pub(super) fn read(reader: &Reader<'_>, node: &Node) -> Result<Vec<Setting>, Problem> {
    let Some(table) = node.as_table_like() else {
        return Ok(Vec::new());
    };
    let mut settings = Vec::new();
    for (name, item) in table.iter() {
        if !setting_name(name) {
            return Err(reader.problem(
                Some(item),
                Code::BadSettingName,
                &format!("settings.{name}"),
                format!("settings.{name}: a setting name starts with a lowercase letter and uses only lowercase letters, digits and _"),
            ));
        }
        let at = format!("settings.{name}");
        let Some(fields) = item.as_table_like() else {
            return Err(reader.problem(
                Some(item),
                Code::NotATable,
                &at,
                format!("{at} must be a table"),
            ));
        };
        settings.push(one(reader, name, &at, fields, item)?);
    }
    Ok(settings)
}

/// 一项。
fn one(
    reader: &Reader<'_>,
    name: &str,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
) -> Result<Setting, Problem> {
    let kind = kind(reader, at, fields, node)?;
    let default = match fields.get("default") {
        None => None,
        Some(item) => Some(default(reader, at, &kind, item)?),
    };
    let layers = match fields.get("layers") {
        None => vec![Layer::System, Layer::Personal],
        Some(item) => layers(reader, at, item)?,
    };
    let applies = match fields.get("applies") {
        None => Applies::HeadStart,
        Some(item) => match item.as_str() {
            Some("now") => Applies::Now,
            Some("new_session") => Applies::NewSession,
            Some("next_turn") => Applies::NextTurn,
            Some("program_start") => Applies::HeadStart,
            _ => {
                return Err(reader.problem(
                    Some(item),
                    Code::BadApplies,
                    &format!("{at}.applies"),
                    format!("{at}.applies must be now, new_session, next_turn or program_start"),
                ));
            }
        },
    };
    let hidden = match fields.get("hidden") {
        None => false,
        Some(item) => item.as_bool().ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::NotBool,
                &format!("{at}.hidden"),
                format!("{at}.hidden must be true or false"),
            )
        })?,
    };
    let label = reader.phrases(
        reader.required(fields, node, at, "name")?,
        &format!("{at}.name"),
    )?;
    let description = match fields.get("description") {
        None => Phrases::new(),
        Some(item) => reader.phrases(item, &format!("{at}.description"))?,
    };
    Ok(Setting {
        name: name.to_string(),
        kind,
        default,
        layers,
        applies,
        label,
        description,
        hidden,
    })
}

/// `type` 和它带的几格：`min`、`max`（整数）、`choices`（选项）、`max`（文字）；列表多 `element`，这几格照元素的类型收。
/// 别的类型写了这几格的报不认识的键。
fn kind(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
) -> Result<SettingKind, Problem> {
    let item = reader.required(fields, node, at, "type")?;
    let (kind, extra, list) = if item.as_str() == Some("list") {
        let element = reader.required(fields, node, at, "element")?;
        let bad = || {
            reader.problem(
                Some(element),
                Code::BadElement,
                &format!("{at}.element"),
                format!("{at}.element must be bool, int, option, text, name, url or secret"),
            )
        };
        let name = element.as_str().ok_or_else(bad)?;
        let (inner, extra) = scalar(reader, at, fields, node, name)?.ok_or_else(bad)?;
        (SettingKind::List(Box::new(inner)), extra, true)
    } else {
        let scalar = match item.as_str() {
            Some(name) => scalar(reader, at, fields, node, name)?,
            None => None,
        };
        let (kind, extra) = scalar.ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadType,
                &format!("{at}.type"),
                format!("{at}.type must be bool, int, option, text, name, url, secret or list"),
            )
        })?;
        (kind, extra, false)
    };
    let common = [
        "type",
        "default",
        "layers",
        "applies",
        "name",
        "description",
        "hidden",
    ];
    let element: &[&str] = if list { &["element"] } else { &[] };
    let known: Vec<&str> = common.iter().chain(extra).chain(element).copied().collect();
    reader.only(fields, at, &known)?;
    Ok(kind)
}

/// 不是列表的那几种：`name` 是哪一种、它带的几格；不认识的是 `None`。
fn scalar(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
    name: &str,
) -> Result<Option<(SettingKind, &'static [&'static str])>, Problem> {
    Ok(Some(match name {
        "bool" => (SettingKind::Bool, &[]),
        "int" => {
            let bound = |key: &str, otherwise: i64| match fields.get(key) {
                None => Ok(otherwise),
                Some(item) => item.as_integer().ok_or_else(|| {
                    reader.problem(
                        Some(item),
                        Code::BadRange,
                        &format!("{at}.{key}"),
                        format!("{at}.{key} must be an integer"),
                    )
                }),
            };
            let (min, max) = (bound("min", i64::MIN)?, bound("max", i64::MAX)?);
            if min > max {
                return Err(reader.problem(
                    fields.get("min"),
                    Code::BadRange,
                    &format!("{at}.min"),
                    format!("{at}.min must not be above {at}.max"),
                ));
            }
            (SettingKind::Int { min, max }, &["min", "max"])
        }
        "option" => {
            let bad = |item: Option<&Node>| {
                reader.problem(
                    item.or(Some(node)),
                    Code::BadChoices,
                    &format!("{at}.choices"),
                    format!("{at}.choices must list at least two different texts"),
                )
            };
            let choices: Vec<String> = fields
                .get("choices")
                .and_then(Node::as_array)
                .and_then(|array| {
                    array
                        .iter()
                        .map(|value| value.as_str().map(str::to_string))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| bad(fields.get("choices")))?;
            let mut distinct = choices.clone();
            distinct.sort();
            distinct.dedup();
            if distinct.len() < 2 || distinct.len() != choices.len() {
                return Err(bad(fields.get("choices")));
            }
            (SettingKind::Option(choices), &["choices"])
        }
        "text" => {
            let max = match fields.get("max") {
                None => TEXT_MAX,
                Some(item) => item
                    .as_integer()
                    .and_then(|max| usize::try_from(max).ok())
                    .filter(|max| *max > 0)
                    .ok_or_else(|| {
                        reader.problem(
                            Some(item),
                            Code::BadRange,
                            &format!("{at}.max"),
                            format!("{at}.max must be a positive integer"),
                        )
                    })?,
            };
            (SettingKind::Text { max }, &["max"])
        }
        "name" => (SettingKind::Name, &[]),
        "url" => (SettingKind::Url, &[]),
        "secret" => (SettingKind::Secret, &[]),
        _ => return Ok(None),
    }))
}

/// 默认值：照类型查；选项照列出的几个查；列表照元素一个个查；密钥、密钥的列表不能有。
fn default(
    reader: &Reader<'_>,
    at: &str,
    kind: &SettingKind,
    item: &Node,
) -> Result<Value, Problem> {
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadDefault,
            &format!("{at}.default"),
            format!("{at}.default does not fit its type"),
        )
    };
    let value = item.as_value().ok_or_else(bad)?;
    if kind.holds_secrets() {
        return Err(bad());
    }
    fits(kind, value).ok_or_else(bad)
}

/// 一个值合不合这个类型，合的读成配置的值。
fn fits(kind: &SettingKind, value: &toml_edit::Value) -> Option<Value> {
    match kind {
        SettingKind::Option(choices) => value
            .as_str()
            .filter(|text| choices.iter().any(|choice| choice == text))
            .map(|text| Value::Text(text.to_string().into())),
        SettingKind::List(element) => value
            .as_array()?
            .iter()
            .map(|value| fits(element, value))
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        plain => {
            let kind = plain.plain()?;
            parse::read(kind, value).filter(|value| kind.accepts(value))
        }
    }
}

/// `layers`：`system`、`personal` 里的一个或两个，不重复。
fn layers(reader: &Reader<'_>, at: &str, item: &Node) -> Result<Vec<Layer>, Problem> {
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadLayers,
            &format!("{at}.layers"),
            format!("{at}.layers must list system, personal or both"),
        )
    };
    let array = item.as_array().ok_or_else(bad)?;
    let mut layers = Vec::new();
    for value in array {
        let layer = match value.as_str() {
            Some("system") => Layer::System,
            Some("personal") => Layer::Personal,
            _ => return Err(bad()),
        };
        if layers.contains(&layer) {
            return Err(bad());
        }
        layers.push(layer);
    }
    if layers.is_empty() {
        return Err(bad());
    }
    layers.sort();
    Ok(layers)
}

/// 名字：小写字母开头，只有小写字母、数字、`_`，最多 64 个。
fn setting_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.len() <= 64
}

/// 包 `package` 的配置项拼成配置清单的项：键 `<包>.<名字>`，挂在「软件包」那一页、这个包那一组。字留在进程里，一直用到
/// 退出：只在核心起来时调一次。
pub fn items(package: &str, settings: &[Setting]) -> Vec<Item> {
    let group: &'static str = leak(package);
    settings
        .iter()
        .map(|setting| Item {
            key: leak(&format!("{package}.{}", setting.name)),
            kind: leaked(&setting.kind),
            default: setting.default.clone(),
            layers: Box::leak(setting.layers.clone().into_boxed_slice()),
            tighten: None,
            env: None,
            applies: setting.applies,
            ui: Ui {
                page: PAGE,
                group,
                common: false,
                control: setting.kind.control(),
                hidden: setting.hidden,
            },
        })
        .collect()
}

/// 换成配置清单的类型：选项的字、列表的元素留在进程里。
fn leaked(kind: &SettingKind) -> Kind {
    match kind {
        SettingKind::Option(choices) => Kind::Option(Box::leak(
            choices
                .iter()
                .map(|choice| leak(choice))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )),
        SettingKind::List(element) => Kind::List(Box::leak(Box::new(leaked(element)))),
        plain => plain.plain().unwrap_or(Kind::Bool),
    }
}

/// 一段字留在进程里。
fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests;
