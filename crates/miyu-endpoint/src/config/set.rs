//! 协议上的 `config.set`（`docs/blueprint/config.md`「协议」、「怎么走」第五、六条，G4、G5，施工 8-3）：在系统配置或个人
//! 设置里改一项或几项、恢复默认，或者整份换掉（`miyu config edit` 用）。
//!
//! 1. 参数不对的（`changes`、`text` 不是正好一个，一项里 `value`、`input`、`unset` 不是正好一个，同一个键写了两次，整份换的
//!    没写 `version`）：`bad_params`。不认识的键：`unknown_config_key`。
//! 2. 一次的几项一起查、一起写：有一项不对（类型、选项、层），整条不收，`config_invalid`。
//! 3. 拿着配置服务的锁，先把这一层的文件重读一遍（手改过的在新的字上改）。改几项的，文件读不进来：`config_file_broken`；
//!    `expect` 对不上：`config_conflict` 带 `current`。整份换的，版本对不上：`config_conflict` 带 `version`；新的字里有错误：
//!    `config_invalid`。
//! 4. 只改那几项（`miyu_config::edit`），先写临时文件再替换；替换前发现这一瞬间有人手改了，从第 3 步重来，最多三次，还不行
//!    的回 `config_conflict`。写不成：`internal_error`，记一条 `WARN config not written`。
//! 5. 写成了：换上新的最终值，记日志（第六条），记一条 `INFO config changed`，推 `config.changed`（施工 8-4），回应。先落盘，
//!    后回应。
//!
//! 第 3 步的重读和监视看到手改走同一条路（`observe.rs`）：手改过的先当手改推、记，再改。

use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value as Json, json};

use miyu_config::edit::{self, Change};
use miyu_config::parse::parse;
use miyu_config::problem::{Code, Problem, Severity, got};
use miyu_config::{Item, Layer, Value};
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, ConfigText, WriteError};
use miyu_store::human::Human;

use super::file::File;
use super::hub::Pushing;
use super::methods::{said, selected, told, words};
use super::observe::{differences, observe, record};
use super::push::{self, Via};
use super::{Config, TARGET, wire};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 替换前发现有人手改，最多重来几次（第五条第 6 条）。
const TRIES: usize = 3;

/// `config.set` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct SetParams {
    /// 改哪一层。项目配置只手改，不收。
    layer: WriteLayer,
    /// 改几项。
    #[serde(default)]
    changes: Option<Vec<ChangeParam>>,
    /// 整份换成这段字。
    #[serde(default)]
    text: Option<String>,
    /// 读到的这份文件的版本，`null` 是那时还没有：写了 `text` 的必写，所以分得出没写和 `null`。
    #[serde(default, deserialize_with = "present")]
    version: Option<Option<String>>,
}

/// 能改的两层。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WriteLayer {
    System,
    Personal,
}

/// `changes` 的一项。
#[derive(Debug, Deserialize)]
struct ChangeParam {
    /// 哪一项。
    key: String,
    /// 改成它，JSON 的写法。写成 `null` 的也算写了（类型不对）。
    #[serde(default, deserialize_with = "present")]
    value: Option<Json>,
    /// 人敲的字，照这一项的类型读。
    #[serde(default)]
    input: Option<String>,
    /// `true`：从这一层删掉。
    #[serde(default)]
    unset: Option<bool>,
    /// 这一层里这一项现在应当是什么：`{"value": …}` 写着这个值，`{}` 没写。
    #[serde(default)]
    expect: Option<Map<String, Json>>,
}

/// 写了这一格（哪怕是 `null`）就是有。
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// 这一次改什么。
enum Plan {
    /// 改几项：每一项要改成的（`None` 是删掉），和 `expect`。
    Changes(Vec<Wanted>),
    /// 整份换：新的字，和人读到的版本。
    Text(String, Option<String>),
}

/// 改的一项。
struct Wanted {
    item: Item,
    value: Option<Value>,
    expect: Option<Option<Json>>,
}

/// `config.set`：改好了交回改了的几项和新的版本。`cause` 是这条命令的编号，记进日志。
pub(crate) fn set(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: SetParams,
) -> Result<Json, Refusal> {
    let words = words(core, peer.language)?;
    let layer = match params.layer {
        WriteLayer::System => Layer::System,
        WriteLayer::Personal => Layer::Personal,
    };
    let mut config = core.config();
    let plan = plan(&config, layer, params, &words)?;
    let via = match &plan {
        Plan::Changes(_) => Via::Set,
        Plan::Text(..) => Via::Edit,
    };
    for _ in 0..TRIES {
        observe(core, &mut config, layer);
        let file = config.file(layer);
        let Some(text) = edited(&config, file, &plan, &words)? else {
            return Ok(json!({"keys": {}, "version": file.version}));
        };
        let bytes = config_file::bytes(&text, file.bom);
        match config_file::write(&file.path, &bytes, file.version.as_deref()) {
            Ok(()) => {
                let written = ConfigText {
                    version: config_file::version(&bytes),
                    text,
                    bom: file.bom,
                };
                return Ok(done(core, &mut config, layer, written, via, cause));
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Io(error)) => {
                tracing::warn!(target: TARGET, file = %file.shown, error = %error, "config not written");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    observe(core, &mut config, layer);
    Err(Refusal::config_conflict_version(
        config.file(layer).version.clone(),
    ))
}

/// 查参数、认键、照类型读值，交回这一次改什么。
fn plan(config: &Config, layer: Layer, params: SetParams, words: &Human) -> Result<Plan, Refusal> {
    match (params.changes, params.text) {
        (Some(changes), None) if !changes.is_empty() => {
            let mut keys: Vec<&str> = changes.iter().map(|change| change.key.as_str()).collect();
            keys.sort_unstable();
            if keys.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(Refusal::BAD_PARAMS);
            }
            let named: Vec<String> = changes.iter().map(|change| change.key.clone()).collect();
            selected(config, Some(named.as_slice()), words)?;
            wanted(config, layer, changes, words).map(Plan::Changes)
        }
        (None, Some(text)) => match params.version {
            Some(version) => Ok(Plan::Text(text, version)),
            None => Err(Refusal::BAD_PARAMS),
        },
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 每一项照类型读成值、查能不能写在这一层；有一项不对整条不收（`config_invalid`，每一处一条）。
fn wanted(
    config: &Config,
    layer: Layer,
    changes: Vec<ChangeParam>,
    words: &Human,
) -> Result<Vec<Wanted>, Refusal> {
    let mut wanted = Vec::new();
    let mut problems = Vec::new();
    for change in changes {
        let Some(item) = config.items().iter().find(|item| item.key == change.key) else {
            return Err(Refusal::BAD_PARAMS);
        };
        let value = match (change.value, change.input, change.unset) {
            (Some(json), None, None) => {
                Some(Some((edit::from_json(item.kind, &json), json.to_string())))
            }
            (None, Some(input), None) => Some(Some((edit::input(item.kind, &input), input))),
            (None, None, Some(true)) => Some(None),
            _ => None,
        };
        let Some(value) = value else {
            return Err(Refusal::BAD_PARAMS);
        };
        let expect = change.expect.map(|expect| expect.get("value").cloned());
        let value = match value {
            None => None,
            Some((read, raw)) => {
                let code = match &read {
                    _ if !item.layers.contains(&layer) => Some(Code::WrongLayer),
                    None => Some(Code::WrongType),
                    Some(read) if !item.kind.accepts(read) => Some(Code::NotAnOption),
                    Some(_) => None,
                };
                if let Some(code) = code {
                    problems.push(request_problem(config, code, layer, item, &raw, words)?);
                }
                read
            }
        };
        wanted.push(Wanted {
            item: item.clone(),
            value,
            expect,
        });
    }
    match problems.is_empty() {
        true => Ok(wanted),
        false => Err(Refusal::config_invalid(problems)),
    }
}

/// 请求里写错的一项说成一条问题：没有文件、没有行列，也不说「先照什么用着」（什么都没变）。
fn request_problem(
    config: &Config,
    code: Code,
    layer: Layer,
    item: &Item,
    raw: &str,
    words: &Human,
) -> Result<Json, Refusal> {
    let problem = Problem {
        code,
        layer,
        at: None,
        key: Some(item.key.to_string()),
        got: Some(got(raw)),
        why: None,
        suggest: None,
        current: None,
        name: None,
    };
    let told = told(&problem, config.items(), None, words)?;
    Ok(wire::problem(&problem, None, &told, None))
}

/// 在这一层现在的字上照 `plan` 改，交回新的字；什么都没变的是空的。改几项的：文件读不进来、`expect` 对不上、放不进去的拒绝；
/// 整份换的：版本对不上、有错误的拒绝。
fn edited(
    config: &Config,
    file: &File,
    plan: &Plan,
    words: &Human,
) -> Result<Option<String>, Refusal> {
    match plan {
        Plan::Changes(wanted) => changed(config, file, wanted, words),
        Plan::Text(text, version) => {
            if version != &file.version {
                return Err(Refusal::config_conflict_version(file.version.clone()));
            }
            let layers = config.layers(None);
            let problems = match parse(config.items(), file.layer, text) {
                Err(problem) => vec![said(config, &layers, &problem, None, words)?],
                Ok(parsed) => {
                    let mut problems = Vec::new();
                    for problem in parsed
                        .problems
                        .iter()
                        .filter(|problem| problem.severity() == Severity::Error)
                    {
                        problems.push(said(config, &layers, problem, None, words)?);
                    }
                    problems
                }
            };
            if !problems.is_empty() {
                return Err(Refusal::config_invalid(problems));
            }
            Ok((file.version.is_none() || text != &file.text).then(|| text.clone()))
        }
    }
}

/// 改几项：查文件读不读得进来、`expect`，再一项项改；这一层本来就是这样的项不动。
fn changed(
    config: &Config,
    file: &File,
    wanted: &[Wanted],
    words: &Human,
) -> Result<Option<String>, Refusal> {
    let broken = || -> Result<Refusal, Refusal> {
        let layers = config.layers(None);
        let mut problems = Vec::new();
        for problem in file.problems() {
            problems.push(said(config, &layers, problem, Some(file), words)?);
        }
        Ok(Refusal::config_file_broken(problems))
    };
    if file.broken.is_some() {
        return Err(broken()?);
    }
    let current = |key: &str| {
        file.parsed
            .entries
            .get(key)
            .filter(|entry| entry.counts)
            .map(|entry| &entry.value)
    };
    for want in wanted {
        if let Some(expected) = &want.expect {
            let now = current(want.item.key).map(Value::json);
            if &now != expected {
                let shown = now.map_or_else(|| json!({}), |value| json!({ "value": value }));
                return Err(Refusal::config_conflict_current(shown));
            }
        }
    }
    let start = match file.version {
        Some(_) => file.text.clone(),
        None => edit::new_file(Config::schema(file.layer)),
    };
    let mut text = start.clone();
    for want in wanted {
        let change = match &want.value {
            Some(value) if current(want.item.key) == Some(value) => continue,
            Some(value) => Change::Set(want.item.key, value),
            None => Change::Unset(want.item.key),
        };
        text = match edit::apply(&text, change) {
            Ok(text) => text,
            Err(_) => return Err(broken()?),
        };
    }
    Ok((text != start).then_some(text))
}

/// 写成了：换上新的一份、重算最终值，记日志和运行日志，推给订阅着配置的头（发这一条的连接先见推送、后见回应，施工 8-4），
/// 交回回应。
fn done(
    core: &Core,
    config: &mut Config,
    layer: Layer,
    written: ConfigText,
    via: Via,
    cause: &CommandId,
) -> Json {
    let old = config.file(layer).clone();
    let new = File::written(config.items(), &old, written);
    let version = new.version.clone();
    let changes = differences(&old, &new);
    config.replace(new);
    let by = By::Person(Person {
        account: config.places.account.clone(),
    });
    record(config, layer, via, by.clone(), Some(cause), &changes);
    let keys: Vec<&'static str> = changes.iter().map(|(key, _, _)| *key).collect();
    let listed = push::keys(config, layer, &keys);
    core.hub.publish(
        config,
        Some(Pushing {
            layer,
            via,
            by: Some(by),
            keys,
        }),
    );
    json!({"keys": listed, "version": version})
}
