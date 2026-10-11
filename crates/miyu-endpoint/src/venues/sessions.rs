//! `venue.sessions {}`（施工 O-32 前，`docs/blueprint/venues.md`「列场所会话」）：系统账号名下的场所会话，每个场所一个，回应
//! `{"sessions": [{"session", "venue"}]}`。通讯平台的桥起来时照它订阅，不用等每个场所来一条消息才找。
//!
//! 一个场所只看最新的那一个（和 `venue.session` 照「场所加属主」找回的是同一个；删了的挪进了回收处，本来就不在），属主是这两种
//! 的列：调用的系统账号；场所编号的平台前缀（第一个 `:` 前面那一段）是这个包清单 `[connection] platform` 的，属主是对应表里这个
//! 平台的身份对着的本机账号（终端管理员的私聊，2026-10-11 核心定）。别的不列：属主换过、旧的留在系统账号名下的（再来一句找的是
//! 新的那一个），别的平台的，对应表里已经没有这个平台的身份对着它的。平台只认前缀，对应表的键也是这个写法，核心不解读场所编号
//! 别的部分。只给系统账号的连接，别的回 `no_system_account`。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, SessionId};

use super::mains;
use crate::Core;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// `venue.sessions` 的参数：没有格，多写的是参数不对。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionsParams {}

/// `venue.sessions`：见模块的说明。从新到旧（会话编号是 UUIDv7，照编号排就是照造的先后），不分页：一个系统账号名下的场所
/// 是它进过的群和私聊，桥起来时一次要全。
pub(crate) async fn sessions(
    core: &Arc<Core>,
    caller: &AccountId,
    _params: SessionsParams,
) -> Result<Value, Refusal> {
    if !core.is_system(caller) {
        return Err(Refusal::NO_SYSTEM_ACCOUNT);
    }
    let owners = owners(core, caller);
    // 场所 → 最新的会话和它的属主：场所会话的属主只会是管理员（对应表里的）或者系统账号，核心认的账号都看。
    let mut newest: BTreeMap<String, (SessionId, AccountId)> = BTreeMap::new();
    for account in core.accounts() {
        for listed in mains(core, &account, |row| row.venue != LOCAL, None).await? {
            let newer = newest
                .get(&listed.venue)
                .is_none_or(|(found, _)| listed.id.as_str() > found.as_str());
            if newer {
                newest.insert(listed.venue, (listed.id, account.clone()));
            }
        }
    }
    let mut mine: Vec<(SessionId, String)> = newest
        .into_iter()
        .filter(|(venue, (_, owner))| owner == caller || owners.take(venue, owner))
        .map(|(venue, (session, _))| (session, venue))
        .collect();
    mine.sort_by(|a, b| b.0.as_str().cmp(a.0.as_str()));
    let sessions: Vec<Value> = mine
        .iter()
        .map(|(session, venue)| json!({"session": session.as_str(), "venue": venue}))
        .collect();
    Ok(json!({"sessions": sessions}))
}

/// 系统账号 `caller` 名下以外还列谁的：它的包清单 `[connection] platform` 是哪个平台（没写的是空的，只列它自己名下的），对应表里
/// 这个平台的身份对着的本机账号（不存在的账号不算）。
///
/// 清单照「系统账号的编号就是包的编号」找（施工 O-4 下的约定）：以后系统账号改了名、不再和包同名，这里要跟着改。
struct Owners {
    platform: Option<String>,
    bound: BTreeSet<AccountId>,
}

/// 照这时的清单、配置算 [`Owners`]。
fn owners(core: &Core, caller: &AccountId) -> Owners {
    let packages = core.packages();
    let platform = packages
        .iter()
        .find(|found| found.id == caller.as_str())
        .and_then(|found| found.read.as_ref().ok())
        .and_then(|manifest| manifest.connection.as_ref())
        .map(|connection| connection.platform.clone());
    let bound = match &platform {
        Some(platform) => super::written(core)
            .into_iter()
            .filter(|(id, _)| platform_of(id) == Some(platform.as_str()))
            .filter_map(|(_, account)| AccountId::parse(&account).ok())
            .filter(|account| *account == core.admin)
            .collect(),
        None => BTreeSet::new(),
    };
    Owners { platform, bound }
}

impl Owners {
    /// 场所 `venue` 最新的会话属主是 `owner`、不是调用的系统账号的，列不列：平台对得上、属主是对应表里这个平台的身份对着的。
    fn take(&self, venue: &str, owner: &AccountId) -> bool {
        self.platform.is_some()
            && platform_of(venue) == self.platform.as_deref()
            && self.bound.contains(owner)
    }
}

/// 场所编号、平台身份的平台前缀：第一个 `:` 前面那一段；没有 `:` 的是空的。
fn platform_of(text: &str) -> Option<&str> {
    text.split_once(':').map(|(prefix, _)| prefix)
}
