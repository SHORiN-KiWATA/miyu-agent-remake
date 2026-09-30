//! 几个模块的测试共用的：一份假的给人看的字，几项手写的清单。

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::item::{Applies, Control, Item, Kind, Layer, Ui};
use crate::value::Value;
use crate::words::{ItemWords, Words};

/// 假的一种语言：几句话照 `{字段}` 换，缺了的就是缺了。
#[derive(Debug, Clone, Default)]
pub(crate) struct Fake {
    pub(crate) items: BTreeMap<String, ItemWords>,
    pub(crate) sentences: BTreeMap<String, String>,
}

impl Words for Fake {
    fn item(&self, key: &str) -> Option<&ItemWords> {
        self.items.get(key)
    }

    fn sentence(&self, key: &str, fields: &[(&str, &str)]) -> Option<String> {
        let mut text = self.sentences.get(key)?.clone();
        for (field, value) in fields {
            text = text.replace(&format!("{{{field}}}"), value);
        }
        Some(text)
    }
}

/// 一项手写的：选项、默认值、几层。
pub(crate) fn item(
    key: &'static str,
    options: &'static [&'static str],
    default: &'static str,
) -> Item {
    Item {
        key,
        kind: Kind::Option(options),
        default: Value::Text(Cow::Borrowed(default)),
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::Now,
        ui: Ui {
            page: "general",
            group: "display",
            common: false,
            control: Control::Select,
        },
    }
}

/// 只能放在系统配置里的一项。
pub(crate) fn system_only(item: Item) -> Item {
    Item {
        layers: &[Layer::System],
        ..item
    }
}

/// 一项的字：名字、说明，选项的名字照选项本身大写。
pub(crate) fn said(name: &str, description: &str, options: &[&str]) -> ItemWords {
    ItemWords {
        name: name.to_string(),
        description: description.to_string(),
        options: options
            .iter()
            .map(|option| (option.to_string(), option.to_uppercase()))
            .collect(),
    }
}

/// 一份假的字，句子照中文那一份的样子，项照 `items` 给名字、说明。
pub(crate) fn words(items: &[Item]) -> Fake {
    let sentences = [
        ("config/reference-header", "头一行。"),
        ("config/reference-where", "头两行。"),
        ("config/reference-item", "{name}：{description}"),
        (
            "config/facts",
            "能写：{values}。只能写在{layers}里。{applies}。",
        ),
        ("config/schema-description", "{description}{facts}"),
        ("config/layer/system", "系统配置"),
        ("config/layer/personal", "个人设置"),
        ("config/applies/now", "当场生效"),
        ("config/or", "{rest}或{last}"),
        ("config/or-values", "{rest} 或 {last}"),
        ("config/list", "{rest}、{next}"),
        ("config/layer/default", "默认值"),
        ("config/layer/project", "项目配置"),
        ("config/unreadable", "读不了这份文件：{why}"),
        ("config/too-big", "这份文件超过 1 MiB，不读"),
        ("config/not-utf8", "这份文件不是 UTF-8"),
        ("config/syntax", "TOML 写法不对：{why}"),
        (
            "config/unknown-key",
            "没有 {key} 这一项。是不是想写 {suggest}？",
        ),
        ("config/unknown-key-plain", "没有 {key} 这一项"),
        ("config/wrong-type", "{key} 要写 {expected}，写的是 {got}"),
        (
            "config/not-an-option",
            "{key} 只能是 {options}，写的是 {got}",
        ),
        (
            "config/wrong-layer",
            "{key} 只能写在{layers}里，写在{layer}里不算",
        ),
        (
            "config/not-tightening",
            "项目配置只能让限制更严。{key} 现在是 {current}，这里写的 {got} 更宽，不算",
        ),
        ("config/untrusted-project", "这份项目配置还没信任，先不用"),
        ("config/fix-example", "改成其中一个，例如 {example}"),
        ("config/fix-write", "改成 {example}"),
        ("config/fix-move", "挪到{layers}里去"),
        ("config/kept", "这一行先不管，原样留着"),
        ("config/using-value", "这一项先照 {value} 用着（{from}）"),
        ("config/using-last-good", "这份文件先照上一次读进来的用着"),
        ("config/using-nothing", "这份文件先不用"),
        ("config/expected/bool", "true 或 false"),
        ("config/expected/table", "一张表"),
        ("config/sentence", "{text}。"),
        ("config/then", "{rest}{next}"),
        ("config/stops", "。？！"),
    ];
    Fake {
        items: items
            .iter()
            .map(|item| {
                let options: &[&str] = match item.kind {
                    Kind::Option(options) => options,
                    Kind::Bool => &[],
                };
                let name = format!("{} 的名字", item.key);
                let description = format!("{} 的说明。", item.key);
                (item.key.to_string(), said(&name, &description, options))
            })
            .collect(),
        sentences: sentences
            .iter()
            .map(|(key, text)| (key.to_string(), text.to_string()))
            .collect(),
    }
}
