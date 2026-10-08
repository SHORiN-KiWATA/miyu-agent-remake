//! 人格（施工 P-1 上，`docs/blueprint/personas.md`）：造会话时照「开会话时指定、个人设置、系统配置」找人格、几层叠好；
//! 记忆归哪个账号；`persona.list`、`persona.get`。找哪几层、怎么叠在 `miyu_store::personas`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_config::phrases::Label;
use miyu_kernel::id::AccountId;
use miyu_store::human::Human;
use miyu_store::personas::{Found, Layer, PersonaError, Personas};

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::settings::PersonaSettings;

pub(crate) mod write;

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
        PersonaError::Invalid(..) => Refusal::persona_invalid(error.to_string()),
        PersonaError::Unreadable(..) => {
            tracing::warn!(target: TARGET, error = %error, "persona unreadable");
            Refusal::INTERNAL
        }
    }
}

/// 同 [`refusal`]，写错了的多带一句照 `words` 的语言的 `message` 和第几行（施工 P-3 补）：`persona.get`、`persona.set`、
/// `persona.read` 用，头照它当场告诉人哪里写错了。
pub(crate) fn told(error: &PersonaError, words: Option<&Human>) -> Refusal {
    let refused = refusal(error);
    match (error, words) {
        (PersonaError::Invalid(_, problem), Some(words)) => {
            let key = format!("persona-problems/{}", problem.code.as_str());
            let message = Words::sentence(words, &key, &[("detail", problem.detail.as_str())]);
            refused.telling(message, problem.line)
        }
        _ => refused,
    }
}

/// 删了会怎样（施工 P-3 补，`*.get` 的 `remove`）：有家目录那一层、下面还有的是 `restore`（回到出厂的样子），只有家目录那
/// 一层的是 `delete`（就没了），没有家目录那一层的没什么可删。
pub(crate) fn remove(layers: &[Layer]) -> Option<&'static str> {
    match (layers.contains(&Layer::Home), layers.len()) {
        (false, _) => None,
        (true, 1) => Some("delete"),
        (true, _) => Some("restore"),
    }
}

/// 造会话时记忆归哪个账号：和载入时的 [`Personas::memory_account`] 同一条规则（`personas.md`「怎么走」第 5 条），叠好的人格
/// 已经知道它住在谁家，不再看一遍目录。属主是系统账号的归管理员，随 O-4。
pub(crate) fn memory_account(found: &Found, owner: &AccountId) -> AccountId {
    found.home.clone().unwrap_or_else(|| owner.clone())
}

/// `persona.list`：几层里所有的人格，照编号排。每个带名字、说明（照这个连接的语言挑）；写错了的带 `problem`、不带名字和
/// 说明。来自哪几层不往外给（施工 P-3 补：人看的是名字）。
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
                "name": label(found.file.name.as_ref(), peer.language),
                "summary": label(found.file.summary.as_ref(), peer.language),
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

/// `persona.get`：叠好的样子（施工 P-3 补：只给人要看的）。名字、说明照这个连接的语言挑；人设、角色扮演提示写没写；示范
/// 对话几轮；删了会怎样（`remove`）。提示词原文照 `persona.read` 给，来自哪一层不往外给。
pub(crate) async fn get(core: &Core, peer: Peer, params: GetParams) -> Result<Value, Refusal> {
    let said = words(core, peer.language).ok();
    let personas = personas(core);
    let found = tokio::task::spawn_blocking(move || personas.find(&params.persona))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|error| told(&error, said.as_ref()))?;
    Ok(describe(&found, peer.language))
}

/// 叠好的一个人格照 `persona.get` 写（`persona.set` 的回应也是它，施工 P-3 下），名字、说明照语言 `language` 挑。
fn describe(found: &Found, language: &str) -> Value {
    json!({
        "persona": found.id,
        "name": label(found.file.name.as_ref(), language),
        "summary": label(found.file.summary.as_ref(), language),
        "prompts": {
            "persona": !found.texts.persona.trim().is_empty(),
            "reminders": !found.texts.reminders.trim().is_empty(),
        },
        "examples": found.texts.examples.len(),
        "remove": remove(&found.layers),
    })
}

/// 人格、预设的名字、说明照语言挑一句（施工 P-3 补）：一句字的就是它，以前的语言表照 [`pick`] 的先后挑，没写的是 `null`。
pub(crate) fn label(label: Option<&Label>, language: &str) -> Option<String> {
    label
        .and_then(|label| label.pick(language))
        .map(str::to_string)
}

/// 照连接的语言挑一句（软件包清单的语言表）；这种语言没写的照 `en`、`zh`、`ja` 的先后挑，都没写的是 `null`。
pub(crate) fn pick(phrases: &miyu_store::personas::Phrases, language: &str) -> Option<String> {
    [language, "en", "zh", "ja"]
        .iter()
        .find_map(|language| phrases.get(*language).cloned())
}

#[cfg(test)]
mod tests;
