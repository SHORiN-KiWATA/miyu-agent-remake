//! 新页面的几种悬浮窗（蓝图「配置页」第 33、34、36 条）：下拉的选择窗、填一行字的编辑窗、人格的详情窗。

use serde_json::Value;

use super::personas::Detail;
use super::target::Target;
use crate::input::Editor;

/// 选择窗里的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// 选了写进配置的值。
    pub value: Value,
    /// 写什么。
    pub label: String,
    /// 后面暗着写的（人格的说明）。
    pub note: Option<String>,
    /// 选不了的原因（写错的人格）；能选的是 `None`。
    pub off: Option<String>,
}

/// 下拉的选择窗：停在现在的值上，选了当场存。
#[derive(Debug, Clone, PartialEq)]
pub struct Choose {
    /// 改的是哪一项。
    pub key: String,
    /// 标题（这一项的名字）。
    pub title: String,
    /// 一行行。
    pub choices: Vec<Choice>,
    /// 选中第几行。
    pub sel: usize,
    /// 选好了写到哪（配置项、预设的一个键、新建预设的一步）。
    pub target: Target,
}

impl Choose {
    /// 开窗：停在值是 `current` 的那一行，没有的停在第一行能选的。
    pub fn open(key: &str, title: &str, choices: Vec<Choice>, current: &Value) -> Self {
        let sel = choices
            .iter()
            .position(|c| &c.value == current && c.off.is_none())
            .or_else(|| choices.iter().position(|c| c.off.is_none()))
            .unwrap_or(0);
        Self {
            key: key.to_string(),
            title: title.to_string(),
            choices,
            sel,
            target: Target::Setting,
        }
    }

    /// 上下挪：跳过选不了的，到头停。
    pub fn step(&mut self, down: bool) {
        let mut at = self.sel;
        loop {
            at = if down {
                if at + 1 >= self.choices.len() {
                    return;
                }
                at + 1
            } else {
                match at.checked_sub(1) {
                    Some(up) => up,
                    None => return,
                }
            };
            if self.choices[at].off.is_none() {
                self.sel = at;
                return;
            }
        }
    }

    /// 选中的值；选不了的是 `None`。
    pub fn chosen(&self) -> Option<&Value> {
        self.choices
            .get(self.sel)
            .filter(|c| c.off.is_none())
            .map(|c| &c.value)
    }
}

/// 填一行字的编辑窗（`text`、`number`）：原来的字整段选中，回车、`s` 存。
#[derive(Debug)]
pub struct LineEdit {
    /// 改的是哪一项。
    pub key: String,
    /// 标题（这一项的名字）。
    pub title: String,
    /// 是数：存的时候照整数、小数读，读不出的不存、写原因。
    pub number: bool,
    /// 输入框。
    pub editor: Editor,
    /// 没存成的原因。
    pub error: Option<String>,
    /// 存的时候写到哪（同 [`Choose::target`]）。
    pub target: Target,
}

impl LineEdit {
    /// 开窗：原来的值整段选中，一敲就换掉。
    pub fn open(key: &str, title: &str, number: bool, current: &Value) -> Self {
        let text = match current {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        let mut editor = Editor::default();
        editor.set(&text);
        editor.select_all();
        Self {
            key: key.to_string(),
            title: title.to_string(),
            number,
            editor,
            error: None,
            target: Target::Setting,
        }
    }

    /// 存的时候写到 `target`（默认写个人设置）。
    pub fn to(self, target: Target) -> Self {
        Self { target, ..self }
    }

    /// 写进配置的值：字照原样（去掉前后空白），数照整数、小数读；读不出的数是 `None`。
    pub fn value(&self) -> Option<Value> {
        let text = self.editor.text().trim().to_string();
        if !self.number {
            return Some(Value::String(text));
        }
        if let Ok(n) = text.parse::<i64>() {
            return Some(Value::from(n));
        }
        text.parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }
}

/// 只看的人格详情窗；没读回来以前是 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaView {
    /// 看哪一个。
    pub id: String,
    /// 读回来的详情。
    pub detail: Option<Detail>,
    /// 读不了的原因。
    pub error: Option<String>,
    /// 光标在第几行能改的（[`super::persona_edit::FIELDS`]）。
    pub cursor: usize,
}

/// 只看的预设详情窗；没读回来以前是 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetView {
    /// 看哪一个。
    pub id: String,
    /// 读回来的详情。
    pub detail: Option<super::presets::Detail>,
    /// 读不了的原因。
    pub error: Option<String>,
    /// 光标在第几行能改的（[`super::preset_edit::fields`]）。
    pub cursor: usize,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Choice, Choose, LineEdit};

    fn choice(v: &str, off: bool) -> Choice {
        Choice {
            value: json!(v),
            label: v.to_string(),
            note: None,
            off: off.then(|| "坏了".to_string()),
        }
    }

    #[test]
    fn the_chooser_starts_on_the_current_value_and_skips_what_cannot_be_chosen() {
        let mut c = Choose::open(
            "persona.default",
            "默认人格",
            vec![choice("a", false), choice("b", true), choice("c", false)],
            &json!("c"),
        );
        assert_eq!(c.sel, 2);
        c.step(false);
        assert_eq!(c.sel, 0, "跳过写错的");
        c.step(false);
        assert_eq!(c.sel, 0, "到头停");
        assert_eq!(c.chosen(), Some(&json!("a")));
        let broken = Choose::open(
            "k",
            "t",
            vec![choice("x", true), choice("y", false)],
            &json!("x"),
        );
        assert_eq!(broken.sel, 1, "现在的值选不了的停在能选的");
    }

    #[test]
    fn numbers_are_read_as_numbers_and_bad_ones_are_not_saved() {
        let mut edit = LineEdit::open("k", "t", true, &json!(30));
        assert_eq!(edit.value(), Some(json!(30)));
        edit.editor.set("1.5");
        assert_eq!(edit.value(), Some(json!(1.5)));
        edit.editor.set("快");
        assert_eq!(edit.value(), None);
        let text = LineEdit::open("k", "t", false, &json!(" hi "));
        assert_eq!(text.value(), Some(json!("hi")));
    }
}
