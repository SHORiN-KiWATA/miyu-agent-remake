//! 规则能设的属性（`docs/blueprint/chat.md` 第一条「对外的样子」那张属性表，`docs/designs/18-通讯平台.md` 第四节）：名字和
//! 类型。拼错的名字找离得最近的那一个，和配置共用 `miyu_config::problem::nearest`（施工 O-12）。
//!
//! 能照配置清单的类型认的，用 [`Kind`]，读和查都照配置（`miyu_config::parse::read`、[`Kind::check`]），原因码一样；配置清单
//! 没有的三种写法（限流、睡眠、管理员的身份）在 [`super::forms`] 里认（施工时定的第 3 条）。群聊内核自带的参数是几张表，
//! 声明在 `params/items.rs`，读法在 `rules/tables.rs`（第八条，施工 O-15）；插件自己的参数随插件那一步加。

use miyu_config::problem::Code;
use miyu_config::{Kind, Value};
use toml_edit::Value as TomlValue;

use super::forms;

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
