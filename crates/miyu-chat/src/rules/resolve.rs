//! 套到一个场所上（`docs/blueprint/chat.md` 第一条「怎么走」第 4、6、7 条）：匹配条件 [`Match`] 怎么读、怎么配；照文件、
//! 规则的先后，匹配的规则写的每一项盖掉之前的，来处跟着换（[`Rules::resolve`]）。

use std::collections::BTreeMap;

use miyu_config::problem::Code;
use miyu_config::{Kind, Value};
use toml_edit::Value as TomlValue;

use super::{Rules, Source, forms};

/// 匹配条件的键，照 `chat.md` 那张表的先后：拼错的找离得最近的那一个。
pub(super) const CONDITIONS: &[&str] = &["platform", "kind", "group", "user"];

/// 一个场所：通讯平台上的一个群或者一个私聊（`18-通讯平台.md` 第四节）。由桥照驱动报上来的填。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Venue {
    /// 平台，例如 `qq`：和规则的 `match.platform` 照字比。
    pub platform: String,
    /// 群还是私聊。
    pub kind: VenueKind,
    /// 平台里的编号：群是群号，私聊是对方的号。照十进制写成字，不带平台前缀；和规则的 `match.group`、`match.user` 照字比。
    pub id: String,
}

/// 场所是群还是私聊。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VenueKind {
    /// 群：`match.kind = "group"`，`match.group` 只配它。
    Group,
    /// 私聊：`match.kind = "private"`，`match.user` 只配它。
    Private,
}

/// 套到一个场所上的结果：每一项属性的值和来处。没有规则设到的项不在里面（「怎么走」第 7 条）：默认值是出厂的规则文件，
/// 代码里不写死。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    /// 属性的名字（`persona`、`rate`……）到它的值和来处，照名字排。
    pub entries: BTreeMap<&'static str, Entry>,
}

/// 一项属性最后的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 值，过了校验。名字、选项、文字、限流、睡眠是字（限流、睡眠照原文，写法的规矩在 `rules/forms.rs`），开关、整数照原样，触发词、
    /// 管理员是字的列表。
    pub value: Value,
    /// 最后设它的那一条规则。
    pub origin: Origin,
}

/// 来处：一项是哪一条规则设的（`miyu onebot venue show` 照它写「出厂」或「系统」和文件名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// 哪一份：出厂还是系统。
    pub source: Source,
    /// 文件名。
    pub file: String,
    /// 这个文件里第几条规则，从 1 数；写错没用上的规则也占一个号，和文件里看到的对得上。
    pub rule: usize,
    /// 这一项的键在第几行，从 1 数。
    pub line: usize,
}

/// 一条规则的匹配条件：写了的都要满足，没写的不管（「对外的样子」那张表）。
#[derive(Debug, Clone, Default)]
pub(super) struct Match {
    /// 平台。
    platform: Option<String>,
    /// 群还是私聊。
    kind: Option<VenueKind>,
    /// 群号的列表：场所是群，并且群号在里面。
    group: Option<Vec<String>>,
    /// 对方的号的列表：场所是私聊，并且对方的号在里面。
    user: Option<Vec<String>>,
}

impl Match {
    /// 记下一个条件 `name = value`。`value` 是空的：写成了表头（`[rule.match.group]`），不是值。
    ///
    /// # Errors
    ///
    /// 不认识的键 `unknown_key`；写成别的类型 `wrong_type`；平台不合名字的写法、写成字的编号不合 [`forms::id`]
    /// `bad_format`；种类不是 `group`、`private` `not_an_option`。
    pub(super) fn set(&mut self, name: &str, value: Option<&TomlValue>) -> Result<(), Code> {
        if !CONDITIONS.contains(&name) {
            return Err(Code::UnknownKey);
        }
        let value = value.ok_or(Code::WrongType)?;
        match name {
            "platform" => {
                let platform =
                    miyu_config::parse::read(Kind::Name, value).ok_or(Code::WrongType)?;
                Kind::Name.check(&platform)?;
                self.platform = Some(String::from(&platform));
            }
            "kind" => {
                self.kind = Some(match value.as_str().ok_or(Code::WrongType)? {
                    "group" => VenueKind::Group,
                    "private" => VenueKind::Private,
                    _ => return Err(Code::NotAnOption),
                });
            }
            "group" => self.group = Some(ids(value)?),
            _ => self.user = Some(ids(value)?),
        }
        Ok(())
    }

    /// 这个场所满不满足写了的每一个条件。`group`、`user` 都写了的没有场所满足；空的列表谁也不配。
    fn matches(&self, venue: &Venue) -> bool {
        let listed = |ids: &Option<Vec<String>>, kind| {
            ids.as_ref()
                .is_none_or(|ids| venue.kind == kind && ids.contains(&venue.id))
        };
        self.platform
            .as_ref()
            .is_none_or(|platform| *platform == venue.platform)
            && self.kind.is_none_or(|kind| kind == venue.kind)
            && listed(&self.group, VenueKind::Group)
            && listed(&self.user, VenueKind::Private)
    }
}

/// 编号的列表：整数照十进制写成字（`0x7B` 是 `123`），字原样收（`"0123"` 不是 `123`），都照 [`forms::id`] 查。
fn ids(value: &TomlValue) -> Result<Vec<String>, Code> {
    let array = value.as_array().ok_or(Code::WrongType)?;
    array
        .iter()
        .map(|id| match id {
            TomlValue::Integer(number) => Ok(number.value().to_string()),
            TomlValue::String(text) if forms::id(text.value()) => Ok(text.value().clone()),
            TomlValue::String(_) => Err(Code::BadFormat),
            _ => Err(Code::WrongType),
        })
        .collect()
}

impl Rules {
    /// 套到场所 `venue` 上（「怎么走」第 6 条）：照文件的先后、文件里规则的先后，一条一条看；匹配的规则，它写的每一项都盖掉
    /// 之前的，来处跟着换。没有规则设到的项不在结果里。
    pub fn resolve(&self, venue: &Venue) -> Resolved {
        let mut resolved = Resolved::default();
        for rule in self.rules.iter().filter(|rule| rule.matcher.matches(venue)) {
            for (name, (value, line)) in &rule.attrs {
                let origin = Origin {
                    source: rule.source,
                    file: rule.file.clone(),
                    rule: rule.number,
                    line: *line,
                };
                let entry = Entry {
                    value: value.clone(),
                    origin,
                };
                resolved.entries.insert(name, entry);
            }
        }
        resolved
    }
}

#[cfg(test)]
mod tests;
