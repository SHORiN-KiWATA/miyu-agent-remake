//! 场所会话与外部身份（施工 O-3，`docs/blueprint/venues.md`）：主人对应表 `external.bindings` 怎么查；`venue.session` 找回或者造
//! 一个场所的主线会话；`session.send` 的 `as` 记成谁。核心不认识 QQ：场所编号不解读，是不是私聊、对方是谁，桥照实报。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, CommandId, ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_store::index::Row;
use miyu_tool::Stop;

use crate::Core;
use crate::list::scan;
use crate::refusal::Refusal;
use crate::sessions::Opening;

mod message;

pub(crate) use message::{VenueMessageParams, platform_id};

/// 主人对应表在配置里的样子（`settings::EXTERNAL_BINDINGS`）。
const BINDINGS: &str = "external.bindings.<external>";

/// `venue.session` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VenueParams {
    venue: String,
    kind: Kind,
    #[serde(default)]
    peer: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    /// 新造的会话用哪个人格（施工 P-1 上）：桥照场所规则算好交来，核心不读场所规则；不写的照默认人格，写 `null` 的明着
    /// 无人格（施工 P-4 上）。找回已有的会话时不看。
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    /// 新造的会话用哪个预设（施工 P-2 上）：同 `persona`，桥照场所规则算好交来；不写的照默认预设。找回已有的会话时不看。
    #[serde(default)]
    preset: Option<String>,
}

/// 场所是私聊还是群。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Private,
    Group,
}

/// `session.send` 的 `as`：代表通讯平台上的哪个人，他在场所里是什么身份。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsParams {
    external: String,
    #[serde(default)]
    role: Option<Role>,
}

/// 平台身份 `id` 在主人对应表里对着的本机账号。没写的、写了不存在的账号（现在只有管理员）当没写：后者记一行运行日志。
pub(crate) fn bound(core: &Core, id: &ExternalId) -> Option<AccountId> {
    let values = core.config().resolved().values();
    let found = values.keys().find_map(|key| {
        let segments = miyu_config::key::split(key)?;
        match miyu_config::key::fit(BINDINGS, &segments) {
            miyu_config::key::Fit::Yes(names)
                if names.first().map(String::as_str) == Some(id.as_str()) =>
            {
                match values.get(key) {
                    Some(miyu_config::Value::Text(account)) => Some(account.to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    })?;
    let account = AccountId::parse(&found).ok()?;
    if account != core.admin {
        tracing::warn!(target: "miyu::endpoint", external = id.as_str(), account = account.as_str(), "binding to an unknown account ignored");
        return None;
    }
    Some(account)
}

/// `venue.session`：找回或者造场所 `venue` 的主线会话（`venues.md`「对外的样子」）。属主：私聊、对方在对应表里的是那个本机
/// 账号；别的归系统账号：连接 `caller` 是系统账号的（核心拉起的、声明了系统账号的包的扩展）归它，别的连接回
/// `no_system_account`（施工 O-4 下）。照「场所加属主」找：属主的主会话里场所是它的、最新的那一个。回应带属主 `account`。
pub(crate) async fn session(
    core: &Arc<Core>,
    caller: &AccountId,
    command: CommandId,
    params: VenueParams,
) -> Result<Value, Refusal> {
    let venue = VenueId::parse(&params.venue).map_err(|_| Refusal::BAD_PARAMS)?;
    let peer = match (params.kind, params.peer) {
        (Kind::Private, Some(peer)) => {
            Some(ExternalId::parse(&peer).map_err(|_| Refusal::BAD_PARAMS)?)
        }
        (Kind::Group, None) => None,
        _ => return Err(Refusal::BAD_PARAMS),
    };
    let owner = match peer.as_ref().and_then(|peer| bound(core, peer)) {
        Some(account) => account,
        None if core.is_system(caller) => caller.clone(),
        None => return Err(Refusal::NO_SYSTEM_ACCOUNT),
    };
    // 同一个场所同时来两次：一个找、一个造，排着来，免得造出两个主线会话。
    let _one_at_a_time = core.venues.lock().await;
    if let Some(found) = find(core, &owner, &venue).await? {
        return Ok(json!({"session": found.as_str(), "created": false, "account": owner.as_str()}));
    }
    let who = Opening {
        owner: owner.clone(),
        attended: false,
        oneshot: false,
        model: None,
        venue: Some(venue),
        memory: None,
        preset: params.preset,
    };
    let cwd = params
        .cwd
        .unwrap_or_else(|| crate::list::NO_CWD.to_string());
    let created = core
        .sessions
        .create(
            core,
            command,
            params.persona.as_ref().map(Option::as_deref),
            cwd,
            Vec::new(),
            who,
        )
        .await?;
    Ok(json!({"session": created.id.as_str(), "created": true, "account": owner.as_str()}))
}

/// 属主 `owner` 的主会话里，场所是 `venue` 的最新的那一个：照会话列表的索引找，和 `session.list` 同一个读法。
async fn find(
    core: &Arc<Core>,
    owner: &AccountId,
    venue: &VenueId,
) -> Result<Option<miyu_kernel::id::SessionId>, Refusal> {
    let root = core.root.clone();
    let index = core.index_for(owner);
    let (owner, venue) = (owner.clone(), venue.as_str().to_string());
    let found = tokio::task::spawn_blocking(move || {
        let pick = |row: &Row| row.parent.is_none() && row.venue == venue;
        scan(
            &root,
            &owner,
            index.as_deref(),
            &Default::default(),
            pick,
            Some(1),
            &Stop::default(),
        )
    })
    .await;
    match found {
        Ok(Ok(listed)) => Ok(listed.into_iter().next().map(|listed| listed.id)),
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "venue session not found");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// `session.send` 的 `as` 记成谁（`venues.md`「`session.send` 的 `as`」）：场所 `venue` 的会话，属主是 `owner`。对应表里对着属主
/// 的号是本人（带 `via`）；别的是外部身份，对应表里有的另记账号。
pub(crate) fn speaker(
    core: &Core,
    venue: &VenueId,
    owner: &AccountId,
    params: AsParams,
) -> Result<By, Refusal> {
    let id = ExternalId::parse(&params.external).map_err(|_| Refusal::BAD_PARAMS)?;
    let account = bound(core, &id);
    if account.as_ref() == Some(owner) {
        let mut person = Person::new(owner.clone());
        person.via = Some(id);
        return Ok(By::Person(person));
    }
    Ok(By::External(External {
        venue: venue.clone(),
        id,
        account,
        role: Some(params.role.unwrap_or(Role::Member)),
    }))
}
