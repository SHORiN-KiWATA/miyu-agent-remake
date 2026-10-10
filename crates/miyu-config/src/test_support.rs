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
        default: Some(Value::Text(Cow::Borrowed(default))),
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::Now,
        ui: Ui {
            page: "general",
            group: "display",
            common: false,
            control: Control::Select,
            hidden: false,
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

/// 类型是密钥的一项（施工 8-5）：默认照环境变量 `EXAMPLE_KEY` 取，能放进系统配置、个人设置。
pub(crate) fn secret_item(key: &'static str) -> Item {
    Item {
        kind: Kind::Secret,
        default: Some(Value::Secret(crate::secret::Reference::Env(
            "EXAMPLE_KEY".to_string(),
        ))),
        ..item(key, &[], "")
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
        ("config/applies/now", "立即生效"),
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
        ("config/untrusted-project", "项目配置未信任，已忽略"),
        ("config/fix-example", "改成其中一个，例如 {example}"),
        ("config/fix-write", "改成 {example}"),
        ("config/fix-move", "挪到{layers}里去"),
        ("config/kept", "已忽略此行"),
        ("config/using-value", "暂用 {value}（{from}）"),
        ("config/using-last-good", "暂用上次的有效配置"),
        ("config/using-nothing", "已忽略此文件"),
        ("config/expected/bool", "true 或 false"),
        ("config/expected/table", "一张表"),
        ("config/sentence", "{text}。"),
        ("config/applies/new_session", "新会话生效"),
        ("config/applies/next_turn", "下一个回合开始时生效"),
        ("config/expected/int", "{min} 到 {max} 之间的整数"),
        ("config/expected/url", "http:// 或 https:// 开头的网址"),
        ("config/expected/name", "名字"),
        ("config/expected/reference", "<供应商>/<模型> 或 @<池>"),
        ("config/expected/model", "<供应商>/<模型>"),
        ("config/expected/list", "{item} 的列表"),
        ("config/expected/id", "小写字母开头的编号"),
        ("config/expected/model-name", "1 到 128 个字节的模型名"),
        ("config/bad-format", "{key} 要写 {expected}，写的是 {got}"),
        (
            "config/out-of-range",
            "{key} 要在 {min} 到 {max} 之间，写的是 {got}",
        ),
        (
            "config/bad-segment",
            "{key} 里的 {name} 不能当名字：要写{expected}",
        ),
        ("config/then", "{rest}{next}"),
        ("config/stops", "。？！"),
        ("config/unknown-secret", "{key} 引用的密钥 {name} 还没设"),
        (
            "config/env-not-set",
            "{key} 引用的环境变量 {name} 核心起来时没有设",
        ),
        (
            "config/bad-secret-name",
            "{key} 不能当密钥的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符",
        ),
        (
            "config/bad-secret-value",
            "{key} 的值要写成带引号的字，不能是空的",
        ),
        ("config/no-provider", "{key} 指的供应商 {name} 没有配"),
        ("config/no-pool", "{key} 指的池 {name} 没有配"),
        (
            "config/unknown-effort",
            "{key} 写的 {name} 不是这个模型现在有的一档：请求照没写发",
        ),
        (
            "config/temperature-unsupported",
            "{key}：这个模型不收温度，请求照没写发",
        ),
        (
            "config/temperature-too-high",
            "{key} 写的 {got} 超过了这个模型的上限 {max}：请求照没写发",
        ),
    ];
    Fake {
        items: items
            .iter()
            .map(|item| {
                let options: &[&str] = match item.kind {
                    Kind::Option(options) | Kind::ModelOr(options) => options,
                    _ => &[],
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
