//! `command.catalog`（施工 O-6 补，`docs/blueprint/protocol.md`「`command.catalog`」）：核心认的斜杠命令列给头的命令菜单、
//! `/help`。不写会话的列全部；写了的只列这个连接在这个会话里打了不会被拒的，和 `command.run` 同一份判法（[`super::allowed`]）。
//! 只收本机的会话：场所会话的命令由桥代表外部的人说，桥自己知道。列表不推送，头换会话、打开菜单时问。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;

use super::{Said, Slash, allowed, words};
use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;
use crate::sessions::admin;

/// `command.catalog` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogParams {
    #[serde(default)]
    session: Option<String>,
}

/// `command.catalog`：交回 `{"commands": [{name, aliases, summary, argument?}]}`，照名字排。
pub(crate) async fn catalog(
    core: &Arc<Core>,
    peer: &Peer,
    params: CatalogParams,
) -> Result<Value, Refusal> {
    let listed: Vec<Slash> = match params.session {
        None => Slash::ALL.to_vec(),
        Some(text) => {
            let session = SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS)?;
            let found = core.sessions.get(core, &session).await?;
            let handle = &found.handle;
            if handle.venue().as_str() != LOCAL {
                return Err(Refusal::VENUE_SESSION);
            }
            let by = admin(core);
            Slash::ALL
                .into_iter()
                .filter(|slash| allowed(core, *slash, handle, &by).is_ok())
                .collect()
        }
    };
    let mut commands = Vec::with_capacity(listed.len());
    for slash in listed {
        let summary = words(core, peer, &Said::plain(summary_key(slash))).await;
        let mut item =
            json!({"name": slash.name(), "aliases": slash.aliases(), "summary": summary});
        if slash.takes_text() {
            item["argument"] = json!(words(core, peer, &Said::plain(argument_key(slash))).await);
        }
        commands.push(item);
    }
    Ok(json!({ "commands": commands }))
}

/// 这个命令是做什么的那一句：`core/human/<语言>.json` 的键。
fn summary_key(slash: Slash) -> &'static str {
    match slash {
        Slash::Clear => "commands/summary/clear",
        Slash::Stop => "commands/summary/stop",
        Slash::Workspace => "commands/summary/workspace",
        Slash::Remember => "commands/summary/remember",
    }
}

/// 名字后面跟什么的提示：同上。只有 [`Slash::takes_text`] 的有。
fn argument_key(slash: Slash) -> &'static str {
    match slash {
        Slash::Workspace => "commands/argument/workspace",
        _ => "commands/argument/remember",
    }
}
