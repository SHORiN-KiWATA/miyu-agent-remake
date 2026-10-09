//! 参数的声明（`docs/blueprint/chat.md` 第八条「对外的样子」那张表、「怎么走」第 1 条）：五张表，每一项一个 `表.项` 的
//! 名字、一个类型（配置清单的 [`Kind`]，范围写在里面）、落在 [`Params`] 的哪一格。只此一份：出厂文件、场所规则、
//! [`Params::at`] 都照它查，名字和类型不在别处另写（第七条第 5 条）。
//!
//! 范围照「能算出有意义的结果」定（第八条施工时定的第 7 条）：只挡算不出意思的、会算坏的，不替人挑值。

use miyu_config::problem::Code;
use miyu_config::{Kind, Value, duration};
use toml_edit::Value as TomlValue;

use super::Params;

/// 分、权重、加分、门槛、冷静的上限和 `k`：0 到 10。分的量级是 0 到 1，加起来能过 1；负的没有意义。
const SCORE: Kind = Kind::Float { min: 0, max: 10 };

/// 千分比：0 到 1000（抽样、base64 可打印的比例）。
const PER_MILLE: Kind = Kind::Int { min: 0, max: 1000 };

/// 百分比：1 到 100（去重的相似度）。0 什么都算重复。
const PERCENT: Kind = Kind::Int { min: 1, max: 100 };

/// 字符数：1 到 100000（base64 至少多长、最多看多少）。看 0 个字符什么都留不下。上限只防写错。
const CHARS: Kind = Kind::Int { min: 1, max: LIMIT };

/// 字符数，0 有意思的：0 到 100000（理由不留、不拆）。
const CHARS_OR_NONE: Kind = Kind::Int { min: 0, max: LIMIT };

/// 条数：0 到 1000（隔几条才引用）。
const COUNT: Kind = Kind::Int { min: 0, max: 1000 };

/// 判官看几条记录：1 到 100，和核心的 `venue.records` 收的一样（`venues.md`「判官看的群聊记录」第 1 条，施工 O-23 下改）。
const RECORDS: Kind = Kind::Int { min: 1, max: 100 };

/// 违规的严重程度：1 到 10。0 是判官一报严重程度就回。
const SEVERITY: Kind = Kind::Int { min: 1, max: 10 };

/// 判官最多输出：1 到 100000 个 token。
const TOKENS: Kind = Kind::Int { min: 1, max: LIMIT };

/// 重试：0 到 10 次。
const RETRIES: Kind = Kind::Int { min: 0, max: 10 };

/// 至少几个两字组才比相似度：1 到 10000。0 会拿空集比。
const BIGRAMS: Kind = Kind::Int {
    min: 1,
    max: 10_000,
};

/// 不可见字符：文字的列表，每项最多 1 个字符；配置的文字不收空的，也就是正好一个（施工时定的第 19 条）。
const ONE_CHAR_EACH: Kind = Kind::List(&Kind::Text { max: 1 });

/// 漏进来的工具调用的标记：文字的列表，每项 1 到 64 个字符（同触发词的上限，只防写错）。空的不收：空的开头、收尾会让清理
/// 停不下来（施工时定的第 20 条）。
const MARKERS: Kind = Kind::List(&Kind::Text { max: 64 });

/// 窗口、半衰期、@ 的间隔：1 秒到 1 天，同场所规则 `rate` 的时长。时长不收 0（配置的时长写法）。
const WINDOW: Kind = Kind::Duration { min: 1, max: DAY };

/// 判官的超时：1 秒到 1 小时。
const TIMEOUT: Kind = Kind::Duration { min: 1, max: HOUR };

/// 字符数、token 数的上限：只防写错，不替人挑值。
const LIMIT: i64 = 100_000;

/// 一天的秒数。
const DAY: u64 = 24 * HOUR;

/// 一小时的秒数。
const HOUR: u64 = 60 * 60;

/// 照位置一一对上的两份：漏进来的工具调用的开头和收尾（第八条「怎么走」第 7 条，施工时定的第 18 条）。同一张表里要一起写、
/// 一样长，读表的时候查（`rules/tables.rs`）；[`Params::at`] 套完不成对的照套之前的。
pub(crate) const PAIRED: (&str, &str) = ("outbound.leak_open", "outbound.leak_close");

/// 一项参数的声明。
pub(crate) struct Item {
    /// 名字：`表.项`，例如 `chatty.probability`。场所规则套出来的结果（[`Resolved`](crate::Resolved)）、`venue show`、
    /// 问题的键都照它。
    pub(crate) key: &'static str,
    /// 类型：配置清单的一种，范围写在里面。
    kind: Kind,
    /// 只收大于 0 的：配置的小数范围只能写整数，写不出开区间（施工时定的第 11 条）。
    positive: bool,
    /// 可以不写：出厂文件缺了它不报（只有 `judge.model`，施工时定的第 6 条）。
    pub(crate) optional: bool,
    /// 查过的值放进 [`Params`] 的哪一格（施工时定的第 12 条）。
    put: fn(&mut Params, &Value),
}

/// 一项平常的参数：出厂文件里得写，照 `kind` 查。
const fn item(key: &'static str, kind: Kind, put: fn(&mut Params, &Value)) -> Item {
    Item {
        key,
        kind,
        positive: false,
        optional: false,
        put,
    }
}

/// 只收大于 0 的一项。
const fn positive(key: &'static str, kind: Kind, put: fn(&mut Params, &Value)) -> Item {
    Item {
        positive: true,
        ..item(key, kind, put)
    }
}

/// 出厂文件里可以不写的一项。
const fn optional(key: &'static str, kind: Kind, put: fn(&mut Params, &Value)) -> Item {
    Item {
        optional: true,
        ..item(key, kind, put)
    }
}

/// 五张表的每一项，照表的先后、表里照第八条那张表的先后；同一张表的挨在一起（[`tables`] 照它数表）。
pub(crate) const ITEMS: &[Item] = &[
    item("inbound.base64_min_chars", CHARS, |p, v| {
        p.base64.min_chars = count(v)
    }),
    item("inbound.base64_max_chars", CHARS, |p, v| {
        p.base64.max_chars = count(v)
    }),
    item("inbound.base64_printable", PER_MILLE, |p, v| {
        p.base64.printable = count(v)
    }),
    item("chatty.probability", PER_MILLE, |p, v| {
        p.chatty.probability = count(v)
    }),
    item("chatty.base", SCORE, |p, v| p.chatty.base = float(v)),
    // 五维的权重各一项，照相关、意愿、社交、时机、连贯的先后填进 `weights`（施工时定的第 1 条）。
    item("chatty.relevance", SCORE, |p, v| {
        p.chatty.weights[0] = float(v)
    }),
    item("chatty.willingness", SCORE, |p, v| {
        p.chatty.weights[1] = float(v)
    }),
    item("chatty.social", SCORE, |p, v| {
        p.chatty.weights[2] = float(v)
    }),
    item("chatty.timing", SCORE, |p, v| {
        p.chatty.weights[3] = float(v)
    }),
    item("chatty.continuity", SCORE, |p, v| {
        p.chatty.weights[4] = float(v)
    }),
    item("chatty.adjust", SCORE, |p, v| p.chatty.adjust = float(v)),
    item("chatty.direct", SCORE, |p, v| p.chatty.direct = float(v)),
    item("chatty.continuation", SCORE, |p, v| {
        p.chatty.continuation.bonus = float(v)
    }),
    item("chatty.continuation_window", WINDOW, |p, v| {
        p.chatty.continuation.window = millis(v)
    }),
    item("chatty.after_speaking", SCORE, |p, v| {
        p.chatty.after_speaking.bonus = float(v)
    }),
    item("chatty.after_speaking_window", WINDOW, |p, v| {
        p.chatty.after_speaking.window = millis(v)
    }),
    item("chatty.restraint", Kind::Bool, |p, v| {
        p.chatty.restraint.on = flag(v)
    }),
    item("chatty.restraint_half_life", WINDOW, |p, v| {
        p.chatty.restraint.half_life = millis(v)
    }),
    item("chatty.restraint_cap", SCORE, |p, v| {
        p.chatty.restraint.cap = float(v)
    }),
    // 0 会算出 NaN：`p = 0` 时 `p³ ÷ (p³ + k³)` 是 0 ÷ 0。
    positive("chatty.restraint_k", SCORE, |p, v| {
        p.chatty.restraint.k = float(v)
    }),
    item("chatty.severity_min", SEVERITY, |p, v| {
        p.chatty.severity_min = count(v)
    }),
    item("dispatch.supersede_window", WINDOW, |p, v| {
        p.supersede_window = millis(v)
    }),
    optional("judge.model", Kind::Reference, |p, v| {
        p.judge.model = Some(String::from(v))
    }),
    item("judge.records", RECORDS, |p, v| p.judge.records = count(v)),
    item("judge.max_tokens", TOKENS, |p, v| {
        p.judge.max_tokens = count(v)
    }),
    item("judge.timeout", TIMEOUT, |p, v| p.judge.timeout = millis(v)),
    item("judge.moderation_timeout", TIMEOUT, |p, v| {
        p.judge.moderation_timeout = millis(v)
    }),
    item("judge.retries", RETRIES, |p, v| p.judge.retries = count(v)),
    item("judge.reason_chars", CHARS_OR_NONE, |p, v| {
        p.judge.reason_chars = count(v)
    }),
    item("outbound.quote_after", COUNT, |p, v| {
        p.outbound.quote_after = count(v)
    }),
    item("outbound.mention_after", WINDOW, |p, v| {
        p.outbound.mention_after = millis(v)
    }),
    item("outbound.min_bigrams", BIGRAMS, |p, v| {
        p.outbound.min_bigrams = count(v)
    }),
    item("outbound.similar", PERCENT, |p, v| {
        p.outbound.similar = count(v)
    }),
    item("outbound.split_chars", CHARS_OR_NONE, |p, v| {
        p.split_chars = count(v)
    }),
    // 清理的两份名单（O-15 下）。两份标记照位置对上（[`PAIRED`]）。
    item("outbound.invisible", ONE_CHAR_EACH, |p, v| {
        p.outbound.invisible = texts(v).iter().filter_map(|c| c.chars().next()).collect()
    }),
    item("outbound.leak_open", MARKERS, |p, v| {
        p.outbound.leak_open = texts(v)
    }),
    item("outbound.leak_close", MARKERS, |p, v| {
        p.outbound.leak_close = texts(v)
    }),
];

impl Item {
    /// 在哪张表：`表.项` 的前一段。
    pub(crate) fn table(&self) -> &'static str {
        split(self.key).0
    }

    /// 表里叫什么：`表.项` 的后一段。
    pub(crate) fn name(&self) -> &'static str {
        split(self.key).1
    }

    /// 照这一项的类型读一个 TOML 的值，读出来的照 [`Item::check`] 查过。
    ///
    /// # Errors
    ///
    /// 写成了别的类型 `wrong_type`；别的照 [`Item::check`]。
    pub(crate) fn read(&self, value: &TomlValue) -> Result<Value, Code> {
        let value = miyu_config::parse::read(self.kind, value).ok_or(Code::WrongType)?;
        self.check(&value)?;
        Ok(value)
    }

    /// 一个值合不合这一项：先照配置的 [`Kind::check`]，只收正数的再查大于 0（`0.0`、`-0.0` 都不收）。
    ///
    /// # Errors
    ///
    /// 照 [`Kind::check`] 的原因码；只收正数的不大于 0，`out_of_range`。
    pub(crate) fn check(&self, value: &Value) -> Result<(), Code> {
        self.kind.check(value)?;
        match self.positive && float(value) <= 0.0 {
            true => Err(Code::OutOfRange),
            false => Ok(()),
        }
    }

    /// 把查过的值放进 `params` 的那一格。
    pub(crate) fn put(&self, params: &mut Params, value: &Value) {
        (self.put)(params, value);
    }
}

/// 照名字（`表.项`）找一项。
pub(crate) fn find(key: &str) -> Option<&'static Item> {
    ITEMS.iter().find(|item| item.key == key)
}

/// 表名 `name` 是不是声明里的一张表：是就交回声明里的那个名字（活得和程序一样长）。
pub(crate) fn table(name: &str) -> Option<&'static str> {
    tables().into_iter().find(|table| *table == name)
}

/// 五张表的名字，照声明的先后。
pub(crate) fn tables() -> Vec<&'static str> {
    let mut tables: Vec<&'static str> = ITEMS.iter().map(Item::table).collect();
    tables.dedup();
    tables
}

/// 在表 `table` 里照项名找一项。
pub(crate) fn in_table(table: &str, name: &str) -> Option<&'static Item> {
    ITEMS
        .iter()
        .find(|item| item.table() == table && item.name() == name)
}

/// 表 `table` 里的项名：拼错的找离得最近的那一个。
pub(crate) fn names(table: &str) -> impl Iterator<Item = &'static str> {
    ITEMS
        .iter()
        .filter(move |item| item.table() == table)
        .map(Item::name)
}

/// `表.项` 在第一个 `.` 处切开。声明里的名字都带一个 `.`（测试守着）；万一没带，整个当表名。
fn split(key: &'static str) -> (&'static str, &'static str) {
    key.split_once('.').unwrap_or((key, ""))
}

// 下面几个把查过的值换成 `Params` 那一格的类型。值都照声明查过：类型对不上、装不下的走不到，走到了照 0，同配置的
// `Setting`（「最终值都校验过，类型对不上的情形只在手写的值里有」）。

/// 整数换成那一格的整数类型（范围都装得下）。
fn count<T: TryFrom<i64> + Default>(value: &Value) -> T {
    match value {
        Value::Int(number) => T::try_from(*number).unwrap_or_default(),
        _ => T::default(),
    }
}

/// 开关。
fn flag(value: &Value) -> bool {
    matches!(value, Value::Bool(true))
}

/// 小数。
fn float(value: &Value) -> f64 {
    match value {
        Value::Float(number) => number.get(),
        _ => 0.0,
    }
}

/// 列表有几项；不是列表的当 0。
pub(crate) fn length(value: &Value) -> usize {
    match value {
        Value::List(values) => values.len(),
        _ => 0,
    }
}

/// 文字的列表：每项照原样。
fn texts(value: &Value) -> Vec<String> {
    match value {
        Value::List(values) => values.iter().map(String::from).collect(),
        _ => Vec::new(),
    }
}

/// 时长（照配置的写法，`15s`、`3m`）换成毫秒。
fn millis(value: &Value) -> i64 {
    match value {
        Value::Text(text) => duration(text)
            .and_then(|length| i64::try_from(length.as_millis()).ok())
            .unwrap_or_default(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
