//! `model_or` 那种项（「配置页」第 21 条：默认模型页第三行「语义模型」，`models.embedding`，4a 的 R-5 补）：能写
//! `options` 那几个字，或者一个 `<供应商>/<模型>`。选择窗列那几个字的名字，最后一行「填一个模型…」开编辑窗；存照「通用」
//! 页的规矩当场存，写错的照核心的原话说。不从 `model.list` 里挑：那里多是对话模型，没有嵌入模型。

use serde_json::{Value, json};

use super::choose::{Choice, Choose, LineEdit};
use super::schema::Opt;
use super::target::Target;
use super::{choices, current};
use crate::settings::popup::Popup;
use crate::settings::{Settings, Texts, Tone};

/// 「填一个模型…」那一行的值：配置里不会有这样的值。
fn custom() -> Value {
    json!({"custom_model": true})
}

/// 这个值是一个模型（`<供应商>/<模型>`），不是 `options` 里的字。
fn is_model(value: &Value) -> bool {
    value.as_str().is_some_and(|s| s.contains('/'))
}

impl Settings {
    /// 开选择窗：停在现在的值上，写的是模型的停在「填一个模型…」；核心不认这一项的（旧核心）说一句。
    pub(in crate::settings) fn open_model_or(&mut self, key: &str, texts: &Texts) {
        let item = self
            .more
            .schema
            .as_ref()
            .and_then(|s| s.items.iter().find(|i| i.key == key))
            .cloned();
        let (Some(item), Some(data)) = (item, self.data.as_ref()) else {
            return self.say(texts.more.no_editor.clone(), Tone::Note);
        };
        let (value, _) = current(&item, data);
        let at = if is_model(value) {
            custom()
        } else {
            value.clone()
        };
        let mut list = choices(&item, None, &texts.more);
        list.push(Choice {
            value: custom(),
            label: texts.more.custom_model.clone(),
            note: None,
            off: None,
        });
        let mut choose = Choose::open(key, &item.name, list, &at);
        choose.target = Target::ModelOr;
        self.popup = Some(Popup::Choose(choose));
    }

    /// 选择窗选好了：「填一个模型…」开编辑窗（原来写的模型留着），交回 `true`；别的交回 `false`，照旧写个人设置。
    pub(in crate::settings) fn model_or_write(
        &mut self,
        target: &Target,
        key: &str,
        value: &Value,
    ) -> bool {
        if *target != Target::ModelOr || *value != custom() {
            return false;
        }
        let item = self
            .more
            .schema
            .as_ref()
            .and_then(|s| s.items.iter().find(|i| i.key == key))
            .cloned();
        let (Some(item), Some(data)) = (item, self.data.as_ref()) else {
            return true;
        };
        let (now, _) = current(&item, data);
        let written = if is_model(now) {
            now.clone()
        } else {
            Value::Null
        };
        self.popup = Some(Popup::Line(LineEdit::open(
            key, &item.name, false, &written,
        )));
        true
    }
}

/// 现在的值写成给人看的：`options` 里的写名字，没写的写 `local` 的名字（不写就是照它），模型照写。
pub fn shown(options: &[Opt], value: &Value) -> (String, bool) {
    let name = |v: &str| {
        options
            .iter()
            .find(|o| o.value == v)
            .map(|o| o.name.clone())
    };
    match value {
        Value::Null => (name("local").unwrap_or_default(), true),
        Value::String(s) => (name(s).unwrap_or_else(|| s.clone()), false),
        other => (other.to_string(), false),
    }
}
