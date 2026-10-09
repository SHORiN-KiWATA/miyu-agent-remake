//! 协议上的 `config.schema`、`config.get`、`config.check`（`docs/blueprint/config.md`「协议」，施工 8-2）：都是查询，
//! 不改什么。说成话、挑出几项的几个小函数 `config.set`、`config.trust` 也用（施工 8-3）。给人看的字（名字、说明、报错的话）照这个连接的语言（握手时定的）。
//!
//! 写了清单里没有的键：`unknown_config_key`，`data.problems` 里每个不认识的一条，带离得最近的键名。

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::merge::{Layers, Trust, below, explain, merge};
use miyu_config::parse::parse;
use miyu_config::problem::{Code, Problem, Told, Using, nearest, tell};
use miyu_config::{Item, Kind, Layer, Words};
use miyu_session::EMBED_LOCAL;
use miyu_store::human::{FALLBACK, Human};

use super::file::File;
use super::{Config, Project, TARGET, wire};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 语义模型那一项的键（`miyu_models::settings::UseSettings` 的 `embedding`）。
const EMBEDDING: &str = "models.embedding";

/// `config.schema` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct SchemaParams {
    /// 只要这几项；不写是全部。
    #[serde(default)]
    keys: Option<Vec<String>>,
}

/// `config.get` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct GetParams {
    /// 只要这几项；不写是全部。
    #[serde(default)]
    keys: Option<Vec<String>>,
    /// 照这个目录找项目配置；不写不算项目配置。
    #[serde(default)]
    cwd: Option<String>,
    /// 每一项再列出写了它的每一层。
    #[serde(default)]
    all: bool,
}

/// `config.check` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct CheckParams {
    /// 当成哪一层的文件查。
    layer: LayerParam,
    /// 要查的字。
    text: String,
}

/// 能查的三层。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LayerParam {
    System,
    Personal,
    Project,
}

impl LayerParam {
    fn layer(self) -> Layer {
        match self {
            LayerParam::System => Layer::System,
            LayerParam::Personal => Layer::Personal,
            LayerParam::Project => Layer::Project,
        }
    }
}

/// `config.schema`：配置清单，名字和说明照这个连接的语言。
pub(crate) fn schema(core: &Core, peer: Peer, params: SchemaParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let config = &*core.config();
    let mut items: Vec<&Item> = Vec::new();
    for (item, _) in selected(config, params.keys.as_deref(), &words)? {
        if !items.iter().any(|seen| seen.key == item.key) {
            items.push(item);
        }
    }
    let fallback = || Human::load(&core.resources, FALLBACK).ok();
    let english = if items.iter().all(|item| words.item(item.key).is_some()) {
        None
    } else {
        fallback()
    };
    let mut pages: Vec<Value> = Vec::new();
    let mut groups: Vec<Value> = Vec::new();
    let mut listed = Vec::new();
    let local = core
        .memory
        .vectors()
        .and_then(|vectors| vectors.local_name());
    for item in items {
        let said = words
            .item(item.key)
            .or_else(|| english.as_ref().and_then(|english| english.item(item.key)));
        let notes = |option: &str| note(item.key, option, local.as_deref());
        listed.push(schema_item(item, said, &notes));
        let page = item.ui.page;
        if !pages.iter().any(|seen| seen["id"] == page) {
            let name = config_name(&words, english.as_ref(), "pages", page);
            pages.push(json!({"id": page, "name": name}));
        }
        let group = item.ui.group;
        if !groups.iter().any(|seen| seen["id"] == group) {
            let name = config_name(&words, english.as_ref(), "groups", group);
            groups.push(json!({"id": group, "name": name, "page": page}));
        }
    }
    Ok(json!({"groups": groups, "items": listed, "pages": pages}))
}

/// 选项后面暗字写的（施工 R-5 再补，`config.md`「协议」）：只有核心查得出的才有，不进给人看的字。现在只有语义模型
/// （`UseSettings` 的 `embedding`）的「内置模型」：本机清单的模型名 `local`，本机的那一路用不上的没有。
fn note(key: &str, option: &str, local: Option<&str>) -> Option<String> {
    match (key, option) {
        (EMBEDDING, EMBED_LOCAL) => local.map(str::to_string),
        _ => None,
    }
}

/// 一项在 `config.schema` 里的样子：选项后面暗字写的照 `notes` 查。
fn schema_item(
    item: &Item,
    said: Option<&miyu_config::ItemWords>,
    notes: &dyn Fn(&str) -> Option<String>,
) -> Value {
    let mut map = Map::new();
    map.insert("key".to_string(), json!(item.key));
    map.insert("type".to_string(), json!(item.kind.as_str()));
    // 选项的列表（施工 8-7：模型能收哪些输入）也列出能选的几个；几个字之一或者一个模型的（`model_or`，施工 R-5 补）列出
    // 那几个字，别的照模型的写法手写。
    let options = match item.kind {
        Kind::Option(options) | Kind::List(&Kind::Option(options)) | Kind::ModelOr(options) => {
            Some(options)
        }
        _ => None,
    };
    if let Some(options) = options {
        let named: Vec<Value> = options
            .iter()
            .map(|option| {
                let name = said
                    .and_then(|said| said.options.get(*option))
                    .map_or(*option, String::as_str);
                let mut named = json!({"name": name, "value": option});
                if let Some(note) = notes(option) {
                    named["note"] = json!(note);
                }
                named
            })
            .collect();
        map.insert("options".to_string(), json!(named));
    }
    if let Some(default) = &item.default {
        map.insert("default".to_string(), default.json());
    }
    match item.kind {
        Kind::Int { min, max } | Kind::Float { min, max } => {
            map.insert("min".to_string(), json!(min));
            map.insert("max".to_string(), json!(max));
        }
        // 时长的范围写成秒（施工 8-7）。
        Kind::Duration { min, max } => {
            map.insert("min".to_string(), json!(min));
            map.insert("max".to_string(), json!(max));
        }
        Kind::Text { max } | Kind::English { max } => {
            map.insert("max".to_string(), json!(max));
        }
        Kind::List(inner) => {
            map.insert("element".to_string(), json!(inner.as_str()));
        }
        _ => {}
    }
    let layers: Vec<&str> = item.layers.iter().map(|layer| layer.as_str()).collect();
    map.insert("layers".to_string(), json!(layers));
    if let Some(tighten) = item.tighten {
        map.insert("tighten".to_string(), json!(tighten.as_str()));
    }
    if let Some(env) = item.env {
        map.insert("env".to_string(), json!(env));
    }
    map.insert("applies".to_string(), json!(item.applies.as_str()));
    map.insert(
        "name".to_string(),
        json!(said.map_or(item.key, |said| said.name.as_str())),
    );
    map.insert(
        "description".to_string(),
        json!(said.map_or("", |said| said.description.as_str())),
    );
    map.insert("page".to_string(), json!(item.ui.page));
    map.insert("group".to_string(), json!(item.ui.group));
    map.insert("common".to_string(), json!(item.ui.common));
    map.insert("control".to_string(), json!(item.ui.control.as_str()));
    if item.ui.hidden {
        map.insert("hidden".to_string(), json!(true));
    }
    Value::Object(map)
}

/// 页、组的名字：这种语言没有的照英文，都没有的照编号。
fn config_name(words: &Human, english: Option<&Human>, what: &str, id: &str) -> String {
    let find = |human: &Human| match what {
        "pages" => human.page(id).map(str::to_string),
        _ => human.group(id).map(str::to_string),
    };
    find(words)
        .or_else(|| english.and_then(find))
        .unwrap_or_else(|| id.to_string())
}

/// `config.get`：最终值，每个值附上来源；每一份文件的位置、版本；这几份文件现在的全部问题。
pub(crate) fn get(core: &Core, peer: Peer, params: GetParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let config = &*core.config();
    let project = params.cwd.as_deref().and_then(|cwd| config.project(cwd));
    let layers = config.layers(project.as_ref());
    let resolved = merge(config.items(), &layers, &|name| {
        config.env.get(name).cloned()
    });
    let shown = |layer: Layer| match layer {
        Layer::System => Some(config.system.shown.clone()),
        Layer::Personal => Some(config.personal.shown.clone()),
        Layer::Project => project.as_ref().map(|project| project.file.shown.clone()),
    };
    let wanted = selected_in(config, &resolved, params.keys.as_deref(), &words)?;
    let mut listed = Map::new();
    for (item, key) in wanted {
        let Some((value, origin)) = resolved.get(&key) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("origin".to_string(), wire::origin(origin, &shown));
        entry.insert("value".to_string(), value.json());
        if params.all {
            let rows: Vec<Value> = explain(item, &key, &layers, &resolved)
                .iter()
                .map(|row| {
                    let mut map = Map::new();
                    map.insert("origin".to_string(), wire::origin(&row.origin, &shown));
                    map.insert("value".to_string(), row.value.json());
                    map.insert("used".to_string(), json!(row.used));
                    if let Some(problem) = row.problem {
                        map.insert("problem".to_string(), json!(problem.as_str()));
                    }
                    Value::Object(map)
                })
                .collect();
            entry.insert("layers".to_string(), json!(rows));
        }
        listed.insert(key, Value::Object(entry));
    }
    let mut problems = Vec::new();
    for file in [&config.system, &config.personal] {
        let missing = config.missing(&file.parsed, file.layer);
        for problem in file.problems().chain(&missing) {
            problems.push(said(config, &layers, problem, Some(file), &words)?);
        }
    }
    let secrets = &config.secrets;
    for problem in secrets.problems() {
        let place = Some((secrets.shown.as_str(), secrets.last_good));
        problems.push(said_at(config, &layers, problem, place, &words)?);
    }
    if let Some(project) = &project {
        let merged = resolved.problems.iter();
        let missing = config.missing(&project.file.parsed, Layer::Project);
        for problem in project.file.problems().chain(merged).chain(&missing) {
            problems.push(said(config, &layers, problem, Some(&project.file), &words)?);
        }
    }
    Ok(json!({
        "files": files(config, project.as_ref()),
        "items": listed,
        "problems": problems,
    }))
}

/// `files`：每一层的文件在哪、版本；项目配置多一格信不信任；密钥文件只说在哪（施工 8-5：它的版本是整份密钥的哈希，
/// 不交出去）。
fn files(config: &Config, project: Option<&Project>) -> Value {
    let mut files = Map::new();
    for file in [&config.system, &config.personal] {
        files.insert(
            file.layer.as_str().to_string(),
            json!({"file": file.shown, "version": file.version}),
        );
    }
    files.insert("secrets".to_string(), json!({"file": config.secrets.shown}));
    if let Some(project) = project {
        let trusted = match project.trust {
            Trust::Trusted => json!(true),
            Trust::Distrusted => json!(false),
            Trust::Unknown => Value::Null,
        };
        files.insert(
            "project".to_string(),
            json!({"file": project.file.shown, "trusted": trusted, "version": project.file.version}),
        );
    }
    Value::Object(files)
}

/// `config.check`：把 `text` 当成一层的文件查，不生效。项目配置照「收紧」和另外几层合出来的比，不看信没信任。
pub(crate) fn check(core: &Core, peer: Peer, params: CheckParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let problems = check_text(core, &words, params.layer.layer(), &params.text)?;
    Ok(json!({ "problems": problems }))
}

/// 把 `text` 当成 `layer` 那一层的文件查，交回给人看的问题（`config.check`；施工 8-30 起 `check` 也用它）。
pub(crate) fn check_text(
    core: &Core,
    words: &Human,
    layer: Layer,
    text: &str,
) -> Result<Vec<Value>, Refusal> {
    let config = &*core.config();
    let parsed = match parse(config.items(), layer, text) {
        Ok(parsed) => parsed,
        Err(problem) => {
            let layers = config.layers(None);
            return Ok(vec![said(config, &layers, &problem, None, words)?]);
        }
    };
    let mut layers = config.layers(None);
    let missing = config.missing_if(&parsed, layer);
    let mut found: Vec<&Problem> = parsed.problems.iter().chain(&missing).collect();
    let tightening;
    if layer == Layer::Project {
        layers.project = Some((&parsed, Trust::Trusted));
        let merged = merge(config.items(), &layers, &|_| None);
        tightening = merged
            .problems
            .into_iter()
            .filter(|problem| problem.code == Code::NotTightening)
            .collect::<Vec<_>>();
        found.extend(tightening.iter());
    }
    let mut problems = Vec::new();
    for problem in found {
        problems.push(said(config, &layers, problem, None, words)?);
    }
    Ok(problems)
}

/// 一条问题写成协议上的样子，话照 `words`。现在照什么用着：一项的问题照这一项在它那一层下面几层合出来的；整份的问题
/// 是手里用着的文件 `file` 的，照上一次读好的用（`last_good`，施工 8-4）或者先不用（`nothing`）；查一段字（没有 `file`）
/// 时不说。
pub(super) fn said(
    config: &Config,
    layers: &Layers,
    problem: &Problem,
    file: Option<&File>,
    words: &Human,
) -> Result<Value, Refusal> {
    let place = file.map(|file| (file.shown.as_str(), file.last_good));
    said_at(config, layers, problem, place, words)
}

/// 同 [`said`]，文件只给在哪、是不是照上一次读好的用着（`place`）：密钥文件不是一层配置（施工 8-5）。
pub(crate) fn said_at(
    config: &Config,
    layers: &Layers,
    problem: &Problem,
    place: Option<(&str, bool)>,
    words: &Human,
) -> Result<Value, Refusal> {
    let key = problem.key.as_deref().unwrap_or_default();
    let item = miyu_config::key::item_of(config.items(), key);
    let using = match (problem.code, item) {
        (code, _) if code.whole_file() => place.map(|(_, last_good)| match last_good {
            true => Using::LastGood,
            false => Using::Nothing,
        }),
        (
            Code::WrongType
            | Code::NotAnOption
            | Code::OutOfRange
            | Code::BadFormat
            | Code::WrongLayer
            | Code::NotTightening,
            Some(item),
        ) => below(item, key, layers, problem.layer)
            .map(|(value, origin)| Using::Value(value, origin)),
        _ => None,
    };
    let told = told(problem, config.items(), using.as_ref(), words)?;
    let shown = place.map(|(shown, _)| shown);
    Ok(wire::problem(problem, shown, &told, using.as_ref()))
}

/// 说成话；要用的字缺了是装坏了：内部出错，记一条 `WARN`。
pub(super) fn told(
    problem: &Problem,
    items: &[Item],
    using: Option<&Using>,
    words: &dyn Words,
) -> Result<Told, Refusal> {
    tell(problem, items, using, words).map_err(|missing| {
        tracing::warn!(target: TARGET, error = %missing, "config words missing");
        Refusal::INTERNAL
    })
}

/// 这个连接的语言的字。读不懂是装坏了：内部出错。
pub(crate) fn words(core: &Core, language: &str) -> Result<Human, Refusal> {
    Human::load(&core.resources, language)
        .map(|human| human.with_packages(crate::packages::manifests(&core.packages), language))
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "resource unreadable");
            Refusal::INTERNAL
        })
}

/// 请求里的 `keys` 挑出的几项和真的键，照清单的先后；不写的是全部写死的项（键里有人起的名字的项，`config.schema` 用它的样子）。
/// 一个真的键对得上一项的样子就算（施工 8-6）。有不认识的：`unknown_config_key`。
pub(super) fn selected<'a>(
    config: &'a Config,
    keys: Option<&[String]>,
    words: &Human,
) -> Result<Vec<(&'a Item, String)>, Refusal> {
    let items = config.items();
    let Some(keys) = keys else {
        return Ok(items
            .iter()
            .map(|item| (item, item.key.to_string()))
            .collect());
    };
    let mut unknown = Vec::new();
    let mut found = Vec::new();
    for key in keys {
        match miyu_config::key::item_of(items, key) {
            Some(item) => found.push((item, key.clone())),
            None => {
                let problem = Problem {
                    code: Code::UnknownKey,
                    layer: Layer::Personal,
                    at: None,
                    key: Some(key.clone()),
                    got: None,
                    why: None,
                    suggest: nearest(items.iter().map(|item| item.key), key),
                    current: None,
                    name: None,
                };
                let told = told(&problem, items, None, words)?;
                // 请求里写错的键是这一条请求的错：级别写错误（文件里不认识的键才是警告）。
                let mut entry = wire::problem(&problem, None, &told, None);
                entry["level"] = json!("error");
                unknown.push(entry);
            }
        }
    }
    if !unknown.is_empty() {
        return Err(Refusal::unknown_config_key(unknown));
    }
    found.sort_by_key(|(item, key)| {
        let at = items.iter().position(|listed| listed.key == item.key);
        (at, key.clone())
    });
    found.dedup_by(|a, b| a.1 == b.1);
    Ok(found)
}

/// `config.get` 挑的：写了 `keys` 的照 [`selected`]；没写的是全部写死的项，加上最终值 `resolved` 里键里有人起的名字的
/// 每一个真的键（施工 8-6）。
fn selected_in<'a>(
    config: &'a Config,
    resolved: &miyu_config::merge::Resolved,
    keys: Option<&[String]>,
    words: &Human,
) -> Result<Vec<(&'a Item, String)>, Refusal> {
    if keys.is_some() {
        return selected(config, keys, words);
    }
    let items = config.items();
    let mut found: Vec<(&Item, String)> = items
        .iter()
        .filter(|item| !item.is_pattern())
        .map(|item| (item, item.key.to_string()))
        .collect();
    for key in resolved.keys() {
        if let Some(item) = miyu_config::key::item_of(items, key).filter(|item| item.is_pattern()) {
            found.push((item, key.to_string()));
        }
    }
    Ok(found)
}
