//! 编辑供应商、模型、池的悬浮窗（蓝图「配置页」第 14 到 19 条）：一行一项，名字、值、来源。开窗时照草稿、个人层、
//! 资料填好；「确定」照 `apply.rs` 记进草稿。

mod apply;

pub use apply::apply;
use serde_json::Value;

use super::data::{Data, Model};
use super::draft::Draft;
use super::keys;
use crate::input::Editor;

/// 改的是哪一样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 供应商：已有的带编号，新加的是 `None`。
    Provider(Option<String>),
    /// 模型：供应商编号；已有的带模型名。
    Model(String, Option<String>),
    /// 池：已有的带名字。
    Pool(Option<String>),
}

/// 一项是什么（名字、空着时写的照它在 `fields`、`hints` 里取）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Id,
    Name,
    BaseUrl,
    Driver,
    Auth,
    Key,
    Model,
    Inputs,
    Window,
    Effort,
    Temperature,
    Price,
    Input,
    Output,
    CacheRead,
    CacheWrite,
    Currency,
    Multiplier,
    Pool,
    Strategy,
    Members,
    Member,
}

impl Field {
    /// 在字里的名字。
    pub fn name(self) -> &'static str {
        match self {
            Field::Id => "id",
            Field::Name => "name",
            Field::BaseUrl => "base_url",
            Field::Driver => "driver",
            Field::Auth => "auth",
            Field::Key => "key",
            Field::Model => "model",
            Field::Inputs => "inputs",
            Field::Window => "window",
            Field::Effort => "effort",
            Field::Temperature => "temperature",
            Field::Price => "price",
            Field::Input => "input",
            Field::Output => "output",
            Field::CacheRead => "cache_read",
            Field::CacheWrite => "cache_write",
            Field::Currency => "currency",
            Field::Multiplier => "multiplier",
            Field::Pool => "pool",
            Field::Strategy => "strategy",
            Field::Members => "members",
            Field::Member => "member",
        }
    }
}

/// 一项的值。
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    /// 一段字（`secret` 的打码）。
    Text {
        /// 字。
        text: String,
        /// 打码。
        secret: bool,
    },
    /// 几个里选一个：选项值，选了第几个（`None` 是没写、照继承的）。
    Choice {
        /// 选项值。
        options: Vec<String>,
        /// 选了第几个。
        at: Option<usize>,
    },
    /// 几个里勾几个（`None` 是没写、照继承的）。
    Multi {
        /// 选项值。
        options: Vec<String>,
        /// 每个勾没勾。
        on: Option<Vec<bool>>,
    },
    /// 池的一个成员：引用、能收图、已经不存在。
    Member {
        /// 引用。
        reference: String,
        /// 能收图。
        sees: bool,
        /// 不存在了。
        gone: bool,
    },
    /// 只读的字。
    Fixed(String),
    /// 一段的段名（价格、成员）。
    Section,
}

/// 一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// 是什么。
    pub field: Field,
    /// 值。
    pub val: Val,
    /// 没写时暗着显示的继承来的值。
    pub inherit: Option<String>,
    /// 继承来的值从哪来（`catalog` 这些）。
    pub source: Option<String>,
}

impl Row {
    fn new(field: Field, val: Val) -> Self {
        Self {
            field,
            val,
            inherit: None,
            source: None,
        }
    }

    fn text(field: Field, text: impl Into<String>) -> Self {
        Self::new(
            field,
            Val::Text {
                text: text.into(),
                secret: false,
            },
        )
    }

    fn inherit(mut self, value: Option<String>, source: Option<String>) -> Self {
        self.inherit = value;
        self.source = source;
        self
    }

    /// 能停在这一行（段名、只读的除外）。
    pub fn stops(&self) -> bool {
        !matches!(self.val, Val::Section | Val::Fixed(_))
    }

    /// 自己写了（不是继承的）。
    pub fn own(&self) -> bool {
        match &self.val {
            Val::Text { text, .. } => !text.is_empty(),
            Val::Choice { at, .. } => at.is_some(),
            Val::Multi { on, .. } => on.is_some(),
            _ => false,
        }
    }
}

/// 焦点在哪：一行，还是下面的按钮（0 确定、1 取消）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// 第几行。
    Row(usize),
    /// 第几个按钮。
    Button(usize),
}

/// 一个编辑窗。
#[derive(Debug)]
pub struct Form {
    /// 改的是哪一样。
    pub target: Target,
    /// 一行行。
    pub rows: Vec<Row>,
    /// 焦点。
    pub focus: Focus,
    /// 正在改一项文字：输入框。
    pub editing: Option<Editor>,
    /// 多选那一行停在第几个选项上。
    pub cursor: usize,
    /// 池的成员照勾的先后。
    pub members: Vec<String>,
    /// 开窗时的样子：「确定」时比一比哪几项动了。
    pub initial: Vec<Row>,
    /// 「确定」不收时的原因。
    pub error: Option<String>,
}

impl Form {
    fn new(target: Target, rows: Vec<Row>, members: Vec<String>) -> Self {
        let focus = rows
            .iter()
            .position(Row::stops)
            .map_or(Focus::Button(0), Focus::Row);
        Self {
            target,
            initial: rows.clone(),
            rows,
            focus,
            editing: None,
            cursor: 0,
            members,
            error: None,
        }
    }

    /// 这一项（`field`）现在的字。
    pub fn text(&self, field: Field) -> &str {
        self.rows
            .iter()
            .find(|r| r.field == field)
            .and_then(|r| match &r.val {
                Val::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }

    /// 这一项选的选项值；没写的是 `None`。
    pub fn choice(&self, field: Field) -> Option<&str> {
        self.rows
            .iter()
            .find(|r| r.field == field)
            .and_then(|r| match &r.val {
                Val::Choice { options, at } => at.map(|i| options[i].as_str()),
                _ => None,
            })
    }

    /// 这一项比开窗时动了。
    pub fn moved(&self, field: Field) -> bool {
        let find = |rows: &[Row]| {
            rows.iter()
                .find(|r| r.field == field)
                .map(|r| r.val.clone())
        };
        find(&self.rows) != find(&self.initial)
    }
}

/// 编辑一家供应商（`None` 是新加）。
pub fn provider(view: &Data, data: &Data, draft: &Draft, id: Option<&str>) -> Form {
    let p = id.and_then(|id| view.provider(id));
    let table = id.map(keys::provider).unwrap_or_default();
    let written = |item: &str| draft.value(&format!("{table}.{item}"), data).cloned();
    let id_row = match id {
        Some(id) => Row::new(Field::Id, Val::Fixed(id.to_string())),
        None => Row::text(Field::Id, ""),
    };
    let name = written("name").and_then(|v| v.as_str().map(str::to_string));
    let base_url = written("base_url").map(|v| match v.get("env").and_then(Value::as_str) {
        Some(env) => format!("env:{env}"),
        None => v.as_str().unwrap_or_default().to_string(),
    });
    let drivers = ["", "openai-chat", "anthropic", "openai-responses"];
    let driver = written("driver").and_then(|v| {
        let v = v.as_str()?.to_string();
        drivers.iter().position(|d| *d == v)
    });
    // 认证：照这一家的 key 的引用（一家一个，核心 8-25）；没有的当 API Key。
    let first = p.and_then(|p| p.key.as_ref());
    let env = first.and_then(|(r, _)| r.strip_prefix("env:"));
    let auth = usize::from(env.is_some());
    let key_set = first.is_some_and(|(r, set)| r.starts_with("secret:") && *set)
        || draft.has_secret(id.unwrap_or(""));
    let mut key = Row::new(
        Field::Key,
        Val::Text {
            text: env.unwrap_or_default().to_string(),
            secret: env.is_none(),
        },
    );
    key.inherit = Some(if key_set { "key_set" } else { "key_unset" }.to_string());
    let rows = vec![
        id_row,
        Row::text(Field::Name, name.unwrap_or_default())
            .inherit(p.map(|p| p.shown().to_string()), None),
        Row::text(Field::BaseUrl, base_url.unwrap_or_default()).inherit(
            p.and_then(|p| p.base_url.as_str().map(str::to_string)),
            None,
        ),
        Row::new(
            Field::Driver,
            Val::Choice {
                options: drivers.iter().map(|d| d.to_string()).collect(),
                at: Some(driver.unwrap_or(0)),
            },
        ),
        Row::new(
            Field::Auth,
            Val::Choice {
                options: vec!["secret".into(), "env".into()],
                at: Some(auth),
            },
        ),
        key,
    ];
    Form::new(Target::Provider(id.map(str::to_string)), rows, Vec::new())
}

/// 编辑一个模型（`model` 是 `None` 的新加）。
pub fn model(view: &Data, data: &Data, draft: &Draft, provider: &str, model: Option<&str>) -> Form {
    let found = model.and_then(|m| view.model(&format!("{provider}/{m}")).map(|(_, m)| m));
    let empty = Model::default();
    let m = found.unwrap_or(&empty);
    let table = model.map_or_else(String::new, |name| keys::model_table(provider, name));
    let written = |item: &str| draft.value(&format!("{table}.{item}"), data).cloned();
    let text_of = |item: &str| written(item).map(|v| plain(&v)).unwrap_or_default();
    let fact = |name: &str| Some(plain(m.fact(name))).filter(|s| !s.is_empty());
    let from = |name: &str| Some(m.from(name));
    // 先后照核心配置项的选项（8-27 起多了音频、视频；驱动现在只真发图、PDF）。
    let inputs = ["text", "image", "pdf", "audio", "video"];
    let on = written("inputs").map(|v| {
        let have: Vec<String> = v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        inputs.iter().map(|i| have.iter().any(|h| h == i)).collect()
    });
    let mut levels = vec![String::new()];
    levels.extend(
        m.fact("reasoning")
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l.as_str().map(str::to_string)),
    );
    let effort = written("effort")
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|v| levels.iter().position(|l| *l == v));
    let mut rows = vec![
        match model {
            Some(name) => Row::new(Field::Model, Val::Fixed(name.to_string())),
            None => Row::text(Field::Model, ""),
        },
        Row::new(
            Field::Inputs,
            Val::Multi {
                options: inputs.iter().map(|i| i.to_string()).collect(),
                on: on.or_else(|| {
                    model
                        .is_none()
                        .then(|| vec![true, false, false, false, false])
                }),
            },
        )
        .inherit(Some(m.inputs().join(",")), from("inputs")),
        Row::text(Field::Window, text_of("window"))
            .inherit(m.window().map(window_text), from("window")),
        Row::new(
            Field::Effort,
            Val::Choice {
                options: levels,
                at: effort,
            },
        )
        .inherit(None, from("effort")),
    ];
    let temperature = if m.fact("takes_temperature") == &Value::Bool(false) {
        Row::new(Field::Temperature, Val::Fixed("no_temperature".into()))
    } else {
        Row::text(Field::Temperature, text_of("temperature"))
    };
    rows.push(temperature);
    rows.push(Row::new(Field::Price, Val::Section));
    let price = m.fact("price");
    for (field, item) in [
        (Field::Input, "input"),
        (Field::Output, "output"),
        (Field::CacheRead, "cache_read"),
        (Field::CacheWrite, "cache_write"),
    ] {
        let inherit = Some(plain(&price[item])).filter(|s| !s.is_empty());
        rows.push(
            Row::text(field, text_of(&format!("price.{item}"))).inherit(inherit, from("price")),
        );
    }
    let currency = Some(plain(&price["currency"])).filter(|s| !s.is_empty());
    rows.push(
        Row::text(Field::Currency, text_of("price.currency")).inherit(currency, from("price")),
    );
    rows.push(
        Row::text(Field::Multiplier, text_of("price_multiplier"))
            .inherit(fact("multiplier"), from("multiplier")),
    );
    Form::new(
        Target::Model(provider.to_string(), model.map(str::to_string)),
        rows,
        Vec::new(),
    )
}

/// 编辑一个池（`None` 是新建）：所有模型一行一个，已经不存在的成员排在最前。
pub fn pool(view: &Data, name: Option<&str>) -> Form {
    let pool = name.and_then(|n| view.pool(n));
    let members = pool.map(|p| p.members.clone()).unwrap_or_default();
    let strategies = vec!["rotate".to_string(), "pin".to_string()];
    let at = pool
        .and_then(|p| p.strategy.as_deref())
        .and_then(|s| strategies.iter().position(|x| x == s))
        .or(Some(0));
    let mut rows = vec![
        match name {
            Some(n) => Row::new(Field::Pool, Val::Fixed(n.to_string())),
            None => Row::text(Field::Pool, ""),
        },
        Row::new(
            Field::Strategy,
            Val::Choice {
                options: strategies,
                at,
            },
        ),
        Row::new(Field::Members, Val::Section),
    ];
    for gone in members.iter().filter(|r| view.model(r).is_none()) {
        rows.push(member(gone, false, true));
    }
    for p in &view.providers {
        for m in &p.models {
            rows.push(member(&m.reference, m.sees(), false));
        }
    }
    Form::new(Target::Pool(name.map(str::to_string)), rows, members)
}

fn member(reference: &str, sees: bool, gone: bool) -> Row {
    Row::new(
        Field::Member,
        Val::Member {
            reference: reference.to_string(),
            sees,
            gone,
        },
    )
}

/// 一个值写成字：字照写，数照写，别的（`null`、表）是空的。
pub fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// 窗口写成人看的：和用量那一行一样写短（`128k`、`1M`、`1.4M`）。
pub fn window_text(n: u64) -> String {
    crate::meter::short(n)
}

/// 人敲的窗口读成数：`128k`、`1M`、`131072`；读不懂的是 `None`。
pub fn window_value(text: &str) -> Option<u64> {
    let t = text.trim().to_lowercase();
    let (digits, times) = match t.strip_suffix('m') {
        Some(d) => (d, 1_000_000),
        None => t.strip_suffix('k').map_or((t.as_str(), 1), |d| (d, 1_000)),
    };
    let n: f64 = digits.trim().parse().ok()?;
    (n > 0.0).then(|| (n * f64::from(times)).round() as u64)
}

#[cfg(test)]
mod tests;
