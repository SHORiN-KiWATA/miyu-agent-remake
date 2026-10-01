//! 分层合出最终值和来源（`docs/blueprint/config.md`「最终值和来源」、「怎么走」第二条第 5 条、第三条第 4 条，G2、G3，
//! 施工 8-2）。
//!
//! 从下往上，上面的盖掉下面的：默认值、系统配置、个人设置、项目配置，最后是环境变量（只对带 `env` 的项）。
//!
//! - 项目配置信任过才算；算了也只认收紧的：和下面几层合出来的比，宽的不算、报 `not_tightening`，一样的收下。还没问过
//!   的报一条 `untrusted_project`（警告），选了不信任的不报。
//! - 环境变量设了、不是空的、读得懂的才盖上去（[`crate::Kind::from_env`]）；读不懂的当没设，交回去由核心记一条 `WARN`。
//! - 一份文件里写错的项已经在解析时丢掉了（[`crate::parse`]），这里只合写对了的。
//! - 键里有人起的名字那一段的项（施工 8-6）：哪几层写了哪几个真的键，每个各合各的；没有默认值的项哪一层都没写，最终值里
//!   就没有它。

use std::collections::BTreeMap;

use crate::item::{Item, Layer};
use crate::parse::Parsed;
use crate::problem::{Code, Problem};
use crate::value::{Value, Values};

/// 一个值从哪来（协议上的「来源」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// 默认值。
    Default,
    /// 一层配置文件里的第几行：这一项的键所在的那一行。
    File {
        /// 哪一层。
        layer: Layer,
        /// 第几行，从 1 数。
        line: usize,
    },
    /// 这一次启动的环境变量。
    Env(&'static str),
}

impl Origin {
    /// 协议上 `layer` 的写法：`default`、`system`、`personal`、`project`、`env`。
    pub fn layer_name(&self) -> &'static str {
        match self {
            Origin::Default => "default",
            Origin::File { layer, .. } => layer.as_str(),
            Origin::Env(_) => "env",
        }
    }
}

/// 项目配置信不信任（第三条第 2 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// 记着信任，版本一样：算。
    Trusted,
    /// 记着不信任，版本一样：不算，也不再提醒。
    Distrusted,
    /// 没有记录，或者内容变了：不算，等人答。
    Unknown,
}

/// 要合的几层：没有的那一层是空的。
#[derive(Debug, Clone, Copy, Default)]
pub struct Layers<'a> {
    /// 系统配置。
    pub system: Option<&'a Parsed>,
    /// 个人设置。
    pub personal: Option<&'a Parsed>,
    /// 项目配置，和它信不信任。
    pub project: Option<(&'a Parsed, Trust)>,
}

/// 合出来的最终值。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    entries: BTreeMap<String, (Value, Origin)>,
    /// 合的时候发现的问题：项目配置的 `not_tightening`、`untrusted_project`。
    pub problems: Vec<Problem>,
    /// 读不懂、当没设的环境变量：名字和原值。
    pub ignored_env: Vec<(&'static str, String)>,
}

impl Resolved {
    /// 真的键 `key` 的最终值和来源；清单里没有的、没写又没有默认值的是空的。
    pub fn get(&self, key: &str) -> Option<(&Value, &Origin)> {
        self.entries.get(key).map(|(value, origin)| (value, origin))
    }

    /// 最终值：设置类型从它变过来。
    pub fn values(&self) -> Values {
        let mut values = Values::default();
        for (key, (value, _)) in &self.entries {
            values.set(key, value.clone());
        }
        values
    }

    /// 全部真的键，照字节排。
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// 一层里写了这一项的一行（`config.get` 的 `all`，`miyu config explain` 用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// 从哪来。
    pub origin: Origin,
    /// 写的值。
    pub value: Value,
    /// 是不是它生效。
    pub used: bool,
    /// 写了、不算的原因：项目配置还没信任、不比下面宽、不能写在这一层。
    pub problem: Option<Code>,
}

/// 照清单 `items` 合 `layers`，环境变量照 `env` 读（名字到值）。
pub fn merge(items: &[Item], layers: &Layers, env: &dyn Fn(&str) -> Option<String>) -> Resolved {
    let mut resolved = Resolved::default();
    if let Some((_, Trust::Unknown)) = layers.project {
        resolved
            .problems
            .push(Problem::file(Code::UntrustedProject, Layer::Project, None));
    }
    for (item, key) in concrete(items, layers) {
        let mut top = below(item, &key, layers, Layer::Project);
        if let Some((project, Trust::Trusted)) = layers.project
            && let Some(entry) = project.entries.get(&key).filter(|entry| entry.counts)
        {
            match (item.tighten, &top) {
                (Some(tighten), Some((below, _))) if tighten.looser(&entry.value, below) => {
                    let mut problem = Problem::item(
                        Code::NotTightening,
                        Layer::Project,
                        &key,
                        entry.at,
                        &entry.raw,
                    );
                    problem.current = Some(below.clone());
                    resolved.problems.push(problem);
                }
                _ => top = Some(file(Layer::Project, entry)),
            }
        }
        if let Some(name) = item.env
            && let Some(raw) = env(name).filter(|raw| !raw.trim().is_empty())
        {
            match item.kind.from_env(&raw) {
                Some(value) => top = Some((value, Origin::Env(name))),
                None => resolved.ignored_env.push((name, raw)),
            }
        }
        if let Some(top) = top {
            resolved.entries.insert(key, top);
        }
    }
    resolved
}

/// 要合的每一个真的键和它是清单里的哪一项：写死的键一项一个；键里有人起的名字的，哪一层写了哪个就是哪个（施工 8-6）。
fn concrete<'a>(items: &'a [Item], layers: &Layers) -> Vec<(&'a Item, String)> {
    let files = [
        layers.system,
        layers.personal,
        layers.project.map(|(parsed, _)| parsed),
    ];
    let mut keys = Vec::new();
    for item in items {
        if !item.is_pattern() {
            keys.push((item, item.key.to_string()));
            continue;
        }
        let written: std::collections::BTreeSet<&String> = files
            .iter()
            .flatten()
            .flat_map(|parsed| parsed.entries.iter())
            .filter(|(_, entry)| entry.item == item.key)
            .map(|(key, _)| key)
            .collect();
        keys.extend(written.into_iter().map(|key| (item, key.clone())));
    }
    keys
}

/// 真的键 `key`（清单里的 `item`）在 `layer` 下面那几层合出来的值和来源（不算环境变量）：一项写错了，丢掉它以后照什么用
/// （「报错」的 `using`）；项目配置和它比收不收紧。下面几层都没写、又没有默认值的是空的。
pub fn below(item: &Item, key: &str, layers: &Layers, layer: Layer) -> Option<(Value, Origin)> {
    let mut top = item
        .default
        .clone()
        .map(|default| (default, Origin::Default));
    let files = [
        (Layer::System, layers.system),
        (Layer::Personal, layers.personal),
    ];
    for (at, parsed) in files {
        if at >= layer {
            break;
        }
        if let Some(entry) = parsed
            .and_then(|parsed| parsed.entries.get(key))
            .filter(|entry| entry.counts)
        {
            top = Some(file(at, entry));
        }
    }
    top
}

/// 一层里写的一项：值和来源。
fn file(layer: Layer, entry: &crate::parse::Entry) -> (Value, Origin) {
    (
        entry.value.clone(),
        Origin::File {
            layer,
            line: entry.line,
        },
    )
}

/// 真的键 `key`（清单里的 `item`）每一层写的，从上往下，默认值在最后（有的话）；`resolved` 是同样几层合出来的（照它定
/// 哪一个生效）。
pub fn explain(item: &Item, key: &str, layers: &Layers, resolved: &Resolved) -> Vec<Written> {
    let used = resolved.get(key).map(|(_, origin)| origin.clone());
    let mut written = Vec::new();
    if let Some(Origin::Env(name)) = &used
        && let Some((value, _)) = resolved.get(key)
    {
        written.push(Written {
            origin: Origin::Env(name),
            value: value.clone(),
            used: true,
            problem: None,
        });
    }
    let files = [
        (Layer::Project, layers.project.map(|(parsed, _)| parsed)),
        (Layer::Personal, layers.personal),
        (Layer::System, layers.system),
    ];
    for (layer, parsed) in files {
        let Some(entry) = parsed.and_then(|parsed| parsed.entries.get(key)) else {
            continue;
        };
        let (_, origin) = file(layer, entry);
        let looser = |tighten: crate::item::Tighten| {
            below(item, key, layers, Layer::Project)
                .is_some_and(|(below, _)| tighten.looser(&entry.value, &below))
        };
        let problem = match (layer, layers.project) {
            _ if !entry.counts => Some(Code::WrongLayer),
            (Layer::Project, Some((_, Trust::Trusted))) if item.tighten.is_some_and(looser) => {
                Some(Code::NotTightening)
            }
            (Layer::Project, Some((_, Trust::Unknown | Trust::Distrusted))) => {
                Some(Code::UntrustedProject)
            }
            _ => None,
        };
        written.push(Written {
            used: used.as_ref() == Some(&origin),
            origin,
            value: entry.value.clone(),
            problem,
        });
    }
    if let Some(default) = &item.default {
        written.push(Written {
            used: used == Some(Origin::Default),
            origin: Origin::Default,
            value: default.clone(),
            problem: None,
        });
    }
    written
}

#[cfg(test)]
mod tests;
