//! 悬浮窗「确定」：照窗里的值记进草稿（蓝图「配置页」第 16 到 19 条、第 24 条）。只记动了的项；价格动了一项就五项一起记
//! （配置里价格是整份的）。不收的交回出错的名字（`errors` 里的键）和要换进去的字。

use serde_json::{Value, json};

use super::{Field, Form, Target, Val, window_value};
use crate::settings::data::Data;
use crate::settings::draft::Draft;
use crate::settings::keys;

/// 不收：出错的名字、`{name}` 换成什么。
pub type Refused = (&'static str, String);

/// 记进草稿；成了交回这一样的编号（选中它用）：供应商编号、`供应商/模型`、池名。
pub fn apply(form: &Form, view: &Data, data: &Data, draft: &mut Draft) -> Result<String, Refused> {
    match &form.target {
        Target::Provider(id) => provider(form, id.as_deref(), view, data, draft),
        Target::Model(p, m) => model(form, p, m.as_deref(), view, data, draft),
        Target::Pool(name) => pool(form, name.as_deref(), view, data, draft),
    }
}

fn provider(
    form: &Form,
    id: Option<&str>,
    view: &Data,
    data: &Data,
    draft: &mut Draft,
) -> Result<String, Refused> {
    let id = match id {
        Some(id) => id.to_string(),
        None => {
            let id = form.text(Field::Id).trim().to_string();
            if !name_ok(&id) {
                return Err(("bad_id", id));
            }
            if view.provider(&id).is_some() {
                return Err(("taken", id));
            }
            id
        }
    };
    let table = keys::provider(&id);
    let new = view.provider(&id).is_none();
    let key = |item: &str| format!("{table}.{item}");
    let auth = form.choice(Field::Auth).unwrap_or("secret");
    let pasted = form.text(Field::Key).trim().to_string();
    if new && auth == "env" && pasted.is_empty() {
        return Err(("need_key", String::new()));
    }
    if new {
        draft.new_providers.push(id.clone());
    }
    if new || form.moved(Field::Name) {
        text_or_unset(draft, data, &key("name"), form.text(Field::Name));
    }
    if new || form.moved(Field::BaseUrl) {
        let url = form.text(Field::BaseUrl).trim();
        match url.strip_prefix("env:") {
            Some(env) => draft.set(&key("base_url"), json!({"env": env.trim()}), data),
            None => text_or_unset(draft, data, &key("base_url"), url),
        }
    }
    if form.moved(Field::Driver) {
        match form.choice(Field::Driver).filter(|d| !d.is_empty()) {
            Some(d) => draft.set(&key("driver"), json!(d), data),
            None => draft.unset(&key("driver"), data),
        }
    }
    if !pasted.is_empty() {
        if auth == "env" {
            draft.set(&key("key"), json!({"env": pasted}), data);
        } else {
            draft.set_secret(&id, pasted);
        }
    }
    Ok(id)
}

fn model(
    form: &Form,
    provider: &str,
    name: Option<&str>,
    view: &Data,
    data: &Data,
    draft: &mut Draft,
) -> Result<String, Refused> {
    let name = match name {
        Some(n) => n.to_string(),
        None => {
            let n = form.text(Field::Model).trim().to_string();
            if n.is_empty() || n.len() > 128 || n.chars().any(char::is_control) {
                return Err(("need_name", n));
            }
            if view.model(&format!("{provider}/{n}")).is_some() {
                return Err(("taken", n));
            }
            n
        }
    };
    let new = form.target == Target::Model(provider.to_string(), None);
    let table = keys::model_table(provider, &name);
    let key = |item: &str| format!("{table}.{item}");
    // 先查完再写：有一项不收，草稿一点不动。
    let number = |field: Field| -> Result<Option<f64>, Refused> {
        let t = form.text(field).trim();
        if t.is_empty() {
            return Ok(None);
        }
        t.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(Some)
            .ok_or(("bad_number", field.name().to_string()))
    };
    let window = match form.text(Field::Window).trim() {
        "" => None,
        t => Some(window_value(t).ok_or(("bad_number", "window".to_string()))?),
    };
    let temperature = number(Field::Temperature)?;
    let multiplier = number(Field::Multiplier)?;
    let prices = [
        Field::Input,
        Field::Output,
        Field::CacheRead,
        Field::CacheWrite,
    ];
    let mut price = Vec::new();
    for field in prices {
        price.push(number(field)?);
    }
    if new {
        draft.new_models.push((provider.to_string(), name.clone()));
    }
    if new || form.moved(Field::Inputs) {
        let row = form.rows.iter().find(|r| r.field == Field::Inputs);
        match row.map(|r| &r.val) {
            Some(Val::Multi {
                options,
                on: Some(on),
            }) if on.iter().any(|b| *b) => {
                let picked: Vec<&String> = options
                    .iter()
                    .zip(on)
                    .filter(|(_, b)| **b)
                    .map(|(o, _)| o)
                    .collect();
                draft.set(&key("inputs"), json!(picked), data);
            }
            _ => draft.unset(&key("inputs"), data),
        }
    }
    if form.moved(Field::Window) {
        number_or_unset(draft, data, &key("window"), window.map(|w| json!(w)));
    }
    if form.moved(Field::Effort) {
        match form.choice(Field::Effort).filter(|e| !e.is_empty()) {
            Some(e) => draft.set(&key("effort"), json!(e), data),
            None => draft.unset(&key("effort"), data),
        }
    }
    if form.moved(Field::Temperature) {
        number_or_unset(
            draft,
            data,
            &key("temperature"),
            temperature.map(|t| json!(t)),
        );
    }
    if form.moved(Field::Multiplier) {
        number_or_unset(
            draft,
            data,
            &key("price_multiplier"),
            multiplier.map(|m| json!(m)),
        );
    }
    let price_moved = prices
        .iter()
        .chain([&Field::Currency])
        .any(|f| form.moved(*f));
    if price_moved {
        write_price(form, &price, &key("price"), data, draft);
    }
    Ok(format!("{provider}/{name}"))
}

/// 价格整份写：窗里自己写了的照写，没写的照继承来的（显示的那个值）补上；四项都空的整份删掉、回到继承的。
fn write_price(form: &Form, typed: &[Option<f64>], table: &str, data: &Data, draft: &mut Draft) {
    let items = ["input", "output", "cache_read", "cache_write"];
    let fields = [
        Field::Input,
        Field::Output,
        Field::CacheRead,
        Field::CacheWrite,
    ];
    let all_empty =
        typed.iter().all(Option::is_none) && form.text(Field::Currency).trim().is_empty();
    for (i, item) in items.iter().enumerate() {
        let key = format!("{table}.{item}");
        let inherited = form
            .rows
            .iter()
            .find(|r| r.field == fields[i])
            .and_then(|r| r.inherit.as_deref())
            .and_then(|t| t.parse::<f64>().ok());
        match typed[i].or(inherited) {
            Some(v) if !all_empty => draft.set(&key, json!(v), data),
            _ => draft.unset(&key, data),
        }
    }
    let key = format!("{table}.currency");
    let currency = form.text(Field::Currency).trim().to_uppercase();
    let inherited = form
        .rows
        .iter()
        .find(|r| r.field == Field::Currency)
        .and_then(|r| r.inherit.clone());
    match Some(currency).filter(|c| !c.is_empty()).or(inherited) {
        Some(c) if !all_empty => draft.set(&key, json!(c), data),
        _ => draft.unset(&key, data),
    }
}

fn pool(
    form: &Form,
    name: Option<&str>,
    view: &Data,
    data: &Data,
    draft: &mut Draft,
) -> Result<String, Refused> {
    let name = match name {
        Some(n) => n.to_string(),
        None => {
            let n = form.text(Field::Pool).trim().to_string();
            if !name_ok(&n) {
                return Err(("bad_id", n));
            }
            if view.pool(&n).is_some() {
                return Err(("taken", n));
            }
            draft.new_pools.push(n.clone());
            n
        }
    };
    let table = keys::pool(&name);
    let new = name_new(form);
    let original = view
        .pool(&name)
        .map(|p| p.members.clone())
        .unwrap_or_default();
    if new || form.members != original {
        draft.set(&format!("{table}.models"), json!(form.members), data);
    }
    if (new || form.moved(Field::Strategy))
        && let Some(s) = form.choice(Field::Strategy)
    {
        draft.set(&format!("{table}.strategy"), json!(s), data);
    }
    Ok(name)
}

fn name_new(form: &Form) -> bool {
    form.target == Target::Pool(None)
}

/// 名字的写法（`config.md`）：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。
fn name_ok(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn text_or_unset(draft: &mut Draft, data: &Data, key: &str, text: &str) {
    match text.trim() {
        "" => draft.unset(key, data),
        t => draft.set(key, json!(t), data),
    }
}

fn number_or_unset(draft: &mut Draft, data: &Data, key: &str, value: Option<Value>) {
    match value {
        Some(v) => draft.set(key, v, data),
        None => draft.unset(key, data),
    }
}
