//! 规则能设的属性（`docs/blueprint/chat.md` 第一条「对外的样子」那张属性表，`docs/designs/18-通讯平台.md` 第四节）：名字和
//! 类型；拼错的名字找离得最近的那一个。
//!
//! 能照配置清单的类型认的，用 [`Kind`]，读和查都照配置（`miyu_config::parse::read`、[`Kind::check`]），原因码一样；配置清单
//! 没有的三种写法（限流、睡眠、管理员的身份）在 [`super::forms`] 里认（施工时定的第 3 条）。插件的参数随各插件那一步加。

use miyu_config::problem::Code;
use miyu_config::{Kind, Value};
use toml_edit::Value as TomlValue;

use super::forms;

/// 拼错的名字离得最近的那一个最远差几个字：照配置清单（`miyu_config::problem::nearest`）。
const NEAREST: usize = 3;

/// 线路规程的几种（`18-通讯平台.md` 第七节）。
const DISCIPLINES: &[&str] = &["every-message", "when-called", "chatty", "wake"];

/// 一项属性的写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Form {
    /// 配置清单的一种类型。
    Kind(Kind),
    /// 限流：字，照 [`forms::rate`]。
    Rate,
    /// 睡眠：字，照 [`forms::sleep`]。
    Sleep,
    /// 管理员：字的列表，每一个照 [`forms::manager`]。
    Managers,
}

/// 规则能设的属性：名字和写法，照属性表的先后。
pub(super) const ATTRS: &[(&str, Form)] = &[
    ("persona", Form::Kind(Kind::Name)),
    ("preset", Form::Kind(Kind::Name)),
    ("discipline", Form::Kind(Kind::Option(DISCIPLINES))),
    ("allow", Form::Kind(Kind::Bool)),
    ("keywords", Form::Kind(Kind::List(&Kind::Text { max: 64 }))),
    ("rate", Form::Rate),
    ("parallel", Form::Kind(Kind::Int { min: 0, max: 3 })),
    ("sleep", Form::Sleep),
    ("managers", Form::Managers),
    ("show_ids", Form::Kind(Kind::Bool)),
    ("workspace", Form::Kind(Kind::Text { max: 4096 })),
    ("extra_prompt", Form::Kind(Kind::Text { max: 2000 })),
];

/// 照名字找一项属性：交回表里的名字（活得和程序一样长，结果的键用它）和写法。
pub(super) fn find(name: &str) -> Option<(&'static str, Form)> {
    ATTRS.iter().copied().find(|(known, _)| *known == name)
}

impl Form {
    /// 照这种写法读一个 TOML 的值，读出来的过一遍校验。限流、睡眠照原文收成字，管理员收成字的列表（[`super::forms`] 的
    /// 模块注释说为什么不换形状）。
    ///
    /// # Errors
    ///
    /// 写成了别的类型 `wrong_type`；别的照 [`Kind::check`] 和 [`super::forms`] 的原因码。
    pub(super) fn read(self, value: &TomlValue) -> Result<Value, Code> {
        match self {
            Form::Kind(kind) => {
                let value = miyu_config::parse::read(kind, value).ok_or(Code::WrongType)?;
                kind.check(&value)?;
                Ok(value)
            }
            Form::Rate => checked_text(value, forms::rate),
            Form::Sleep => checked_text(value, forms::sleep),
            Form::Managers => value
                .as_array()
                .ok_or(Code::WrongType)?
                .iter()
                .map(|value| checked_text(value, forms::manager))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List),
        }
    }
}

/// 一个写成字的值，照 `check` 查过收成字。不是字的 `wrong_type`。
fn checked_text(value: &TomlValue, check: fn(&str) -> Result<(), Code>) -> Result<Value, Code> {
    let text = value.as_str().ok_or(Code::WrongType)?;
    check(text)?;
    Ok(Value::Text(text.to_string().into()))
}

/// `names` 里和写的 `written` 离得最近的名字：照配置清单的算法和门槛（编辑距离，插入、删除、替换一个字，相邻两个字换位，
/// 都算 1；不超过 3、也不超过 `written` 长度的三分之一的才给；几个一样近的取前面的）。
///
/// 配置的 `nearest` 只认清单里的项（`&[Item]`），这里的名字不是配置项，所以照它的样子写一个小的，不去开配置的私有函数。
pub(super) fn nearest<'a>(
    names: impl IntoIterator<Item = &'a str>,
    written: &str,
) -> Option<&'a str> {
    let written: Vec<char> = written.chars().collect();
    let mut best: Option<(usize, &str)> = None;
    for name in names {
        let distance = distance(&written, &name.chars().collect::<Vec<_>>());
        let close = distance <= NEAREST && distance * 3 <= written.len();
        if close && best.is_none_or(|(so_far, _)| distance < so_far) {
            best = Some((distance, name));
        }
    }
    best.map(|(_, name)| name)
}

/// 编辑距离，相邻两个字换位算一次（optimal string alignment）。
fn distance(a: &[char], b: &[char]) -> usize {
    let width = b.len() + 1;
    let mut table = vec![0usize; (a.len() + 1) * width];
    let at = |i: usize, j: usize| i * width + j;
    for i in 0..=a.len() {
        table[at(i, 0)] = i;
    }
    for j in 0..=b.len() {
        table[at(0, j)] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (table[at(i - 1, j)] + 1)
                .min(table[at(i, j - 1)] + 1)
                .min(table[at(i - 1, j - 1)] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(table[at(i - 2, j - 2)] + 1);
            }
            table[at(i, j)] = best;
        }
    }
    table[at(a.len(), b.len())]
}
