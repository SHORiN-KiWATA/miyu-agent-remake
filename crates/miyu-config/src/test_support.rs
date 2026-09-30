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
    ];
    Fake {
        items: items
            .iter()
            .map(|item| {
                let Kind::Option(options) = item.kind;
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
