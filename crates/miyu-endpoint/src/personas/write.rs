//! 新建、改、删人格，读提示词的原文（施工 P-3 下，`docs/blueprint/personas.md`「改」，16 第四、九节）：只写管理员家目录那一层
//! 的人格目录。改出厂的、系统区的就是建同名覆盖，只写改了的；不写编号的是新建，编号由核心起（施工 P-3 补）。
//!
//! - `persona.toml` 照 `changes` 改（`crate::toml_changes`，同预设），提示词整份换、`unset` 删掉你那一层的；示范对话也能给一对
//!   一对的（`pairs`，施工 P-3 补），核心写成文件的写法。几样一起查（改完的一份连同叠好以后），有错什么都不写。
//! - 提示词防覆盖：`persona.read` 给你那一层这一份的版本，`persona.set` 带着它（`expect`），对不上 `persona_conflict`。
//! - 写盘照配置文件的规矩（`miyu_store::config_file`），一份一份写；只试一次：几份里写到一半撞上有人手改的，前面写了的不撤，
//!   回 `persona_conflict`，头重读再来（重来时前面那几份已经是改完的样子，不再写）。
//! - 删：整个目录挪进回收处（`miyu_store::trash::personas`），留 7 天。

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer};
use serde_json::{Value as Json, json};

use miyu_config::Words;
use miyu_policy::persona::{self, Demo};
use miyu_store::config_file::{self, ConfigText, WriteError};
use miyu_store::human::Human;
use miyu_store::layers::valid;
use miyu_store::personas::{Edits, Found, PERSONA_MD, Personas, REMINDERS_MD};
use miyu_store::trash;

use super::{TARGET, describe, personas, told};
use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::sessions::now;
use crate::toml_changes::{ChangeParams, Wanted, check_expect, edited, wanted, without_base};

/// 提示词的名字到人格目录里的位置。
fn prompt_file(name: &str) -> Option<&'static str> {
    match name {
        "persona" => Some(PERSONA_MD),
        "examples" => Some(persona::EXAMPLES),
        "reminders" => Some(REMINDERS_MD),
        _ => None,
    }
}

/// `persona.set` 的参数：`changes`、`prompts` 至少写一样。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    #[serde(default)]
    persona: Option<String>,
    #[serde(default)]
    changes: Vec<ChangeParams>,
    #[serde(default)]
    prompts: BTreeMap<String, PromptParams>,
}

/// `prompts` 的一份：`text`、`unset`、`pairs`（只给示范对话）正好写一个；`expect` 是 `persona.read` 给的版本，`null` 是
/// 「你那一层还没有」。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptParams {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    unset: Option<bool>,
    #[serde(default)]
    pairs: Option<Vec<PairParams>>,
    #[serde(default, deserialize_with = "written")]
    expect: Option<Option<String>>,
}

/// `pairs` 的一对：人说的、她答的。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairParams {
    user: String,
    assistant: String,
}

/// 写了的（连 `null` 在内）是 `Some`，没写的是 `None`（`#[serde(default)]`）。
fn written<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

/// 查过参数的一份提示词。
struct Prompt {
    file: &'static str,
    text: Option<String>,
    expect: Option<Option<String>>,
}

/// 要写的一份：内容（`None` 是删掉），和读到时的版本（替换、删之前再比一次）。
struct Pending {
    path: PathBuf,
    content: Option<Vec<u8>>,
    version: Option<String>,
}

/// `persona.set`：改家目录那一层，交回改完叠好的样子（同 `persona.get`）。不写 `persona` 的是新建（施工 P-3 补）：编号由
/// 核心起，至少要写一样东西。
pub(crate) async fn set(core: &Core, peer: Peer, params: SetParams) -> Result<Json, Refusal> {
    let empty = params.changes.is_empty() && params.prompts.is_empty();
    if empty || params.persona.as_deref().is_some_and(|id| !valid(id)) {
        return Err(Refusal::BAD_PARAMS);
    }
    let said = words(core, peer.language).ok();
    let wanted = wanted(params.changes)?;
    let prompts = prompts(params.prompts, said.as_ref())?;
    let writes =
        wanted.iter().any(Wanted::writes) || prompts.iter().any(|prompt| prompt.text.is_some());
    if params.persona.is_none() && !writes {
        return Err(Refusal::BAD_PARAMS);
    }
    let personas = personas(core);
    let found = tokio::task::spawn_blocking(move || match params.persona {
        Some(id) => write(&personas, &id, &wanted, &prompts, said.as_ref()),
        None => create(&personas, &wanted, &prompts, said.as_ref()),
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    Ok(describe(&found, peer.language))
}

/// 新建：挑一个没用过的编号，先建它的目录占住（别处同时建了同一个的，换下一个），再照常写；写不成的把空目录删掉。
fn create(
    personas: &Personas,
    wanted: &[Wanted],
    prompts: &[Prompt],
    said: Option<&Human>,
) -> Result<Found, Refusal> {
    for _ in 0..3 {
        let id = personas.unused_id();
        let dir = personas.home_dir(&id);
        if let Some(parent) = dir.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(target: TARGET, kind = ?error.kind(), "persona not created");
            return Err(Refusal::INTERNAL);
        }
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "persona not created");
                return Err(Refusal::INTERNAL);
            }
        }
        let written = write(personas, &id, wanted, prompts, said);
        if written.is_err()
            && let Err(error) = std::fs::remove_dir_all(&dir)
        {
            tracing::warn!(target: TARGET, kind = ?error.kind(), "persona dir not removed");
        }
        return written;
    }
    Err(Refusal::conflict("persona_conflict", None))
}

/// 查 `prompts`：名字只认那三种，`text`、`unset`、`pairs` 正好一个、`unset` 只能是 `true`；`pairs` 只给示范对话，每一句去掉
/// 前后空白不能是空的，空的 `pairs` 等于删掉，写成文件的写法读不回原样的 `persona_invalid`（施工 P-3 补）。
fn prompts(
    params: BTreeMap<String, PromptParams>,
    said: Option<&Human>,
) -> Result<Vec<Prompt>, Refusal> {
    params
        .into_iter()
        .map(|(name, prompt)| {
            let file = prompt_file(&name).ok_or(Refusal::BAD_PARAMS)?;
            let text = match (prompt.text, prompt.unset, prompt.pairs) {
                (Some(text), None, None) => Some(text),
                (None, Some(true), None) => None,
                (None, None, Some(pairs)) if file == persona::EXAMPLES => examples(&pairs, said)?,
                _ => return Err(Refusal::BAD_PARAMS),
            };
            Ok(Prompt {
                file,
                text,
                expect: prompt.expect,
            })
        })
        .collect()
}

/// 一对一对的示范对话写成文件的写法；空的是删掉。
fn examples(pairs: &[PairParams], said: Option<&Human>) -> Result<Option<String>, Refusal> {
    if pairs.is_empty() {
        return Ok(None);
    }
    let blank = |text: &str| text.trim().is_empty();
    if pairs
        .iter()
        .any(|pair| blank(&pair.user) || blank(&pair.assistant))
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let demos: Vec<Demo> = pairs
        .iter()
        .map(|pair| Demo {
            user: pair.user.clone(),
            assistant: pair.assistant.clone(),
        })
        .collect();
    persona::write_examples(&demos).map(Some).map_err(|n| {
        let pair = n.to_string();
        let message = said.and_then(|words| {
            Words::sentence(
                words,
                "persona-problems/pair_clash",
                &[("pair", pair.as_str())],
            )
        });
        Refusal::persona_invalid(format!(
            "{}: pair {n}: a line starts with user: or assistant:",
            persona::EXAMPLES
        ))
        .telling(message, None)
    })
}

/// 读、查 `expect`、改、查改完的、写。什么都没变的不写，照盘上的找。
fn write(
    personas: &Personas,
    id: &str,
    wanted: &[Wanted],
    prompts: &[Prompt],
    said: Option<&Human>,
) -> Result<Found, Refusal> {
    let mut edits = Edits::new();
    let mut pending = Vec::new();
    let toml_path = personas.home_file(id, persona::TOML);
    let toml = read(&toml_path)?;
    let (text, bom, version) = parts(toml);
    check_expect(&text, wanted, "persona_conflict")?;
    let label = format!("home {}", persona::TOML);
    let new = edited(
        &without_base(&text, "persona"),
        wanted,
        &label,
        Refusal::persona_invalid,
    )?;
    if new != text {
        pending.push(Pending {
            path: toml_path,
            content: Some(config_file::bytes(&new, bom)),
            version,
        });
        edits.insert(persona::TOML, Some(new));
    }
    for prompt in prompts {
        let path = personas.home_file(id, prompt.file);
        let now = read(&path)?;
        let version = now.as_ref().map(|now| now.version.clone());
        if prompt
            .expect
            .as_ref()
            .is_some_and(|expected| *expected != version)
        {
            return Err(Refusal::conflict("persona_conflict", Some(json!(version))));
        }
        let content = match (&prompt.text, &now) {
            (Some(text), Some(now)) if now.text == *text => continue,
            (None, None) => continue,
            (Some(text), now) => Some(config_file::bytes(
                text,
                now.as_ref().is_some_and(|now| now.bom),
            )),
            (None, Some(_)) => None,
        };
        pending.push(Pending {
            path,
            content,
            version,
        });
        edits.insert(prompt.file, prompt.text.clone());
    }
    if pending.is_empty() {
        return personas.find(id).map_err(|error| told(&error, said));
    }
    let found = personas
        .find_with(id, &edits)
        .map_err(|error| told(&error, said))?;
    for one in &pending {
        match apply(one) {
            Ok(()) => {}
            Err(WriteError::Changed) => return Err(Refusal::conflict("persona_conflict", None)),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "persona not saved");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    tracing::info!(target: TARGET, persona = %id, "persona saved");
    Ok(found)
}

/// 写一份、删一份：删之前也再读一次，和读到时的版本对不上的放弃。
fn apply(one: &Pending) -> Result<(), WriteError> {
    match &one.content {
        Some(content) => config_file::write(&one.path, content, one.version.as_deref()),
        None => {
            let now = config_file::read(&one.path)
                .map_err(|error| WriteError::Io(io::Error::other(error.to_string())))?
                .map(|now| now.version);
            if now != one.version {
                return Err(WriteError::Changed);
            }
            std::fs::remove_file(&one.path).map_err(WriteError::Io)
        }
    }
}

/// 读一份：没有的是空的；读不了的是内部出错。
fn read(path: &std::path::Path) -> Result<Option<ConfigText>, Refusal> {
    config_file::read(path).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "persona unreadable");
        Refusal::INTERNAL
    })
}

/// 读到的一份拆成字、有没有 BOM、版本；没有的是空的字、没有版本。
fn parts(read: Option<ConfigText>) -> (String, bool, Option<String>) {
    match read {
        Some(read) => (read.text, read.bom, Some(read.version)),
        None => (String::new(), false, None),
    }
}

/// `persona.read` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    persona: String,
    prompt: String,
}

/// `persona.read`：叠好的那一份提示词的原文，和你那一层这一份的版本：`{"text", "version"}`，没有的是 `null`；示范对话另带
/// 一对一对的 `pairs`（施工 P-3 补，界面照它编）。来自哪一层不往外给。
pub(crate) async fn read_prompt(
    core: &Core,
    peer: Peer,
    params: ReadParams,
) -> Result<Json, Refusal> {
    let file = prompt_file(&params.prompt).ok_or(Refusal::BAD_PARAMS)?;
    let said = words(core, peer.language).ok();
    let personas = personas(core);
    let id = params.persona;
    tokio::task::spawn_blocking(move || {
        let found = personas
            .find(&id)
            .map_err(|error| told(&error, said.as_ref()))?;
        let origin = match file {
            PERSONA_MD => &found.persona_from,
            REMINDERS_MD => &found.reminders_from,
            _ => &found.examples_from,
        };
        let text = match origin {
            Some(layer) => {
                let path = personas
                    .file_of(&id, *layer, file)
                    .ok_or(Refusal::INTERNAL)?;
                read(&path)?.map(|read| read.text)
            }
            None => None,
        };
        let version = read(&personas.home_file(&id, file))?.map(|read| read.version);
        let mut reply = json!({"text": text, "version": version});
        if file == persona::EXAMPLES {
            reply["pairs"] = json!(found.texts.examples);
        }
        Ok(reply)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// `persona.delete` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteParams {
    persona: String,
}

/// `persona.delete`：家目录那一层这个人格的目录挪进回收处，交回 `{"remains"}`：下面几层还有没有。你那一层本来就没有的
/// `nothing_to_delete`。
pub(crate) async fn delete(core: &Core, params: DeleteParams) -> Result<Json, Refusal> {
    if !valid(&params.persona) {
        return Err(Refusal::BAD_PARAMS);
    }
    let personas = personas(core);
    let (root, account, at) = (core.root.clone(), core.admin.clone(), now());
    let id = params.persona;
    tokio::task::spawn_blocking(move || {
        let dir = personas.home_dir(&id);
        if !dir.is_dir() {
            return Err(Refusal::NOTHING_TO_DELETE);
        }
        if let Err(error) = trash::personas::discard(&root, &account, &dir, &id, at) {
            tracing::warn!(target: TARGET, kind = ?error.kind(), "persona not deleted");
            return Err(Refusal::INTERNAL);
        }
        tracing::info!(target: TARGET, persona = %id, "persona deleted");
        Ok(json!({"remains": personas.exists(&id)}))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}
