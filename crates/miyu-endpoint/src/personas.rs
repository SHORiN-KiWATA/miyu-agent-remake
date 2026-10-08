//! 人格（施工 P-1 上，`docs/blueprint/personas.md`）：造会话时照「开会话时指定、个人设置、系统配置」找人格、几层叠好；
//! 记忆归哪个账号；`persona.list`、`persona.get`。找哪几层、怎么叠在 `miyu_store::personas`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::personas::{Found, Layer, Origin, PersonaError, Personas};

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::settings::PersonaSettings;

const TARGET: &str = "miyu::endpoint";

/// 这个核心的几层人格：出厂、系统区、管理员的家目录。
pub(crate) fn personas(core: &Core) -> Personas {
    Personas::new(&core.resources, &core.root, &core.admin)
}

/// 找新会话的人格：指定了的是它，不然照这时的 `persona.default`（个人设置压着系统配置，都没写的是出厂的 `engineer`）。
/// 在阻塞线程里读盘。
pub(crate) async fn resolve(core: &Core, wanted: Option<&str>) -> Result<Found, Refusal> {
    let id = match wanted {
        Some(id) => id.to_string(),
        None => PersonaSettings::from(&core.config().resolved().values()).default,
    };
    let personas = personas(core);
    let found = tokio::task::spawn_blocking(move || personas.find(&id)).await;
    match found {
        Ok(Ok(found)) => Ok(found),
        Ok(Err(error)) => Err(refusal(&error)),
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 找人格出的错照协议说：编号不合写法的参数不对，哪一层都没有的 `unknown_persona`，写错了的 `persona_invalid`（`data.problem`
/// 写明哪一层、哪个文件第几行），读不了的是内部出错。
fn refusal(error: &PersonaError) -> Refusal {
    match error {
        PersonaError::BadId(_) => Refusal::BAD_PARAMS,
        PersonaError::NotFound(_) => Refusal::UNKNOWN_PERSONA,
        PersonaError::Invalid(..)
        | PersonaError::BaseCycle(_)
        | PersonaError::BaseMissing(..)
        | PersonaError::BaseInvalid(..) => Refusal::persona_invalid(error.to_string()),
        PersonaError::Unreadable(..) => {
            tracing::warn!(target: TARGET, error = %error, "persona unreadable");
            Refusal::INTERNAL
        }
    }
}

/// 造会话时记忆归哪个账号：和载入时的 [`Personas::memory_account`] 同一条规则（`personas.md`「怎么走」第 5 条），叠好的人格
/// 已经知道它住在谁家，不再看一遍目录。属主是系统账号的归管理员，随 O-4。
pub(crate) fn memory_account(found: &Found, owner: &AccountId) -> AccountId {
    found.home.clone().unwrap_or_else(|| owner.clone())
}

/// `persona.list`：几层里所有的人格，照编号排。每个带名字、说明（照这个连接的语言挑）、来自哪几层；写错了的带 `problem`、
/// 不带名字和说明。
pub(crate) async fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let personas = personas(core);
    let read = tokio::task::spawn_blocking(move || {
        personas
            .ids()
            .into_iter()
            .map(|id| {
                let found = personas.find(&id);
                (id, found)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let listed: Vec<Value> = read
        .into_iter()
        .map(|(id, found)| match found {
            Ok(found) => json!({
                "persona": id,
                "name": pick(&found.file.name, peer.language),
                "summary": pick(&found.file.summary, peer.language),
                "layers": layers(&found.layers),
            }),
            Err(error) => json!({"persona": id, "problem": error.to_string()}),
        })
        .collect();
    Ok(json!({"personas": listed}))
}

/// `persona.get` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    persona: String,
}

/// `persona.get`：叠好的样子。名字、说明的几种语言原样给，人设、示范对话、角色扮演提示来自哪一层（没有的是 `null`，来自底的
/// 写成 `base:<编号>/<层>`），示范对话几轮；写了底的带 `base`（施工 P-3 上）。
/// 提示词原文不经协议给。
pub(crate) async fn get(core: &Core, params: GetParams) -> Result<Value, Refusal> {
    let found = resolve(core, Some(&params.persona)).await?;
    let from = |origin: &Option<Origin>| origin.as_ref().map(|origin| origin_of(&found.id, origin));
    let mut reply = json!({
        "persona": found.id,
        "name": found.file.name,
        "summary": found.file.summary,
        "layers": layers(&found.layers),
        "prompts": {
            "persona": from(&found.persona_from),
            "examples": from(&found.examples_from),
            "reminders": from(&found.reminders_from),
        },
        "examples": found.texts.examples.len(),
    });
    if let Some(base) = &found.base {
        reply["base"] = json!(base);
    }
    Ok(reply)
}

/// 一份字来自哪儿，`persona.get` 的写法：人格 `id` 自己的是那一层，来自底的是 `base:<编号>/<层>`。
fn origin_of(id: &str, origin: &Origin) -> String {
    if origin.persona == id {
        origin.layer.as_str().to_string()
    } else {
        format!("base:{}/{}", origin.persona, origin.layer.as_str())
    }
}

fn layers(layers: &[Layer]) -> Vec<&'static str> {
    layers.iter().map(|layer| layer.as_str()).collect()
}

/// 照连接的语言挑一句；这种语言没写的照 `en`、`zh`、`ja` 的先后挑，都没写的是 `null`。
pub(crate) fn pick(phrases: &miyu_store::personas::Phrases, language: &str) -> Option<String> {
    [language, "en", "zh", "ja"]
        .iter()
        .find_map(|language| phrases.get(*language).cloned())
}

#[cfg(test)]
mod tests;
