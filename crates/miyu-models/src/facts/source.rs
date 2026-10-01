//! 一格资料的来源（`docs/blueprint/models.md`「模型的资料」「来源写成一个对象」那张表，施工 8-7）：头照它说一句。

use miyu_config::Layer;
use miyu_kernel::time::Timestamp;
use serde_json::{Map, Value, json};

use crate::matching::Matched;

/// 从哪来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// 手写的：哪一层的配置、第几行。
    Config {
        /// 哪一层。
        layer: Layer,
        /// 第几行，从 1 数。
        line: usize,
    },
    /// 用出来的。
    Learned {
        /// 什么时候记下的。
        at: Timestamp,
    },
    /// 供应商的模型列表。
    Provider {
        /// 列表什么时候拉的。
        fetched: Timestamp,
    },
    /// models.dev 的目录。
    Catalog {
        /// 目录里的哪一个：`<供应商>/<模型>`。
        entry: String,
        /// 第几层对上的。
        layer: u8,
        /// 目录什么时候拉的：`meta` 里的原样。
        fetched: String,
    },
    /// 本机的服务：价格当 0。
    Local,
    /// 驱动的保守默认。
    Default,
}

impl Source {
    /// 对上的目录条目 `matched`，目录是 `fetched` 拉的。
    pub fn catalog(matched: &Matched, fetched: &str) -> Source {
        Source::Catalog {
            entry: format!("{}/{}", matched.provider, matched.model),
            layer: matched.layer,
            fetched: fetched.to_string(),
        }
    }

    /// 写成协议上的样子：`from` 和另带的几格。手写的写成哪份文件，由 `file` 照层给（数据根里的相对路径）。
    pub fn json(&self, file: &dyn Fn(Layer) -> String) -> Map<String, Value> {
        let value = match self {
            Source::Config { layer, line } => {
                json!({"from": "config", "file": file(*layer), "line": line})
            }
            Source::Learned { at } => json!({"from": "learned", "at": at}),
            Source::Provider { fetched } => json!({"from": "provider", "fetched": fetched}),
            Source::Catalog {
                entry,
                layer,
                fetched,
            } => json!({"from": "catalog", "entry": entry, "layer": layer, "fetched": fetched}),
            Source::Local => json!({"from": "local"}),
            Source::Default => json!({"from": "default"}),
        };
        match value {
            Value::Object(map) => map,
            _ => Map::new(),
        }
    }
}
