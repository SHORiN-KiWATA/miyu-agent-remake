//! 人经协议碰记忆（施工 R-3 补，`docs/blueprint/memory.md`「协议」、第二条、第九条；`protocol.md` 的 `memory.*`）：列、搜、
//! 记、改、忘和清空。怎么记、怎么挑在会话那一层的 [`Keeper`]，她的工具也用它；这里只管找哪一间、读参数、写回应。现在就整理
//! （`memory.dream`、`/dream`，施工 R-7 补）在 `memory/dream.rs`。
//!
//! - 哪一间：`persona` 是这个人格那一间（记忆账号照 `personas.md` 第 5 条，属主是这个连接的账号）；`session` 是那个会话用的
//!   那一间（照会话交出来的 [`Handle::memory_room`]）；都不写照默认人格，没有默认人格的没有哪一间（不带人格记忆不生效）。
//! - 听众是这个连接的人：本机的令牌、网页登录都是管理员。代表外部的人（`as`）不收，随 O 线。
//! - 碰记忆日志、检索库的在阻塞线程里做。

pub(crate) mod dream;
mod params;

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_recall::{CLASSES, Entry, MemoryId};
use miyu_session::{
    ConfigSource, EmbedSetup, Embedder, Filter, Handle, Keeper, Memory, Stamp, Turn, Using, Vectors,
};
use miyu_store::recall::Room;
use miyu_tool::{Refused, Remember, TEXT_CHARS};

use crate::Core;
use crate::list::LOCAL;
use crate::personas;
use crate::refusal::Refusal;
use crate::sessions::{admin, now};

use params::{Forget, List, Search, Update, Where};

impl Core {
    /// 同一份家底，接上照意思找的那一路（施工 R-5 下、补，`recall.md` 第四条）：本机的照 `local` 算（没装内置语义模型的是
    /// 空的，装卸以后照新的清单换，`Vectors::replace_local`），远程的照模型资料里的供应商发。核心起来时接一次，接在
    /// [`Core::with_model_data`] 后面（远程的照那一份查供应商、记账）；不接的（测试里）只照关键词找。（施工 R-10 从 `lib.rs`
    /// 挪过来。）
    #[must_use]
    pub fn with_vectors(self, local: Option<EmbedSetup>) -> Core {
        let vectors = Vectors::new(local.map(Embedder::new), Arc::clone(&self.model_data));
        self.memory.give_vectors(Arc::new(vectors));
        self
    }

    /// 账号 `owner` 的会话用的记忆的登记，交给造的、载入的会话（施工 R-2 上、R-3 中）：管理员和系统账号的会话有（施工 O-4
    /// 下）；记忆归哪个账号另照 [`Core::memory_owner`] 算，系统账号的归管理员。
    pub(crate) fn memory_for(&self, owner: &AccountId) -> Option<Arc<Memory>> {
        self.knows(owner).then(|| Arc::clone(&self.memory))
    }
}

/// `memory.list` 不写 `limit` 时给几条。
const LIST: usize = 50;
/// `memory.search` 不写 `limit` 时给几条。
const SEARCH: usize = 10;
/// `limit` 最多写几。
const LIMIT: usize = 500;

/// 照方法名办一条 `memory.*`，命令编号是 `id`；不是这五个的没有这个方法。
pub(crate) async fn call(
    core: &Arc<Core>,
    method: &str,
    id: &CommandId,
    params: &Value,
) -> Result<Value, Refusal> {
    if !installed(core) {
        return Err(Refusal::MEMORY_NOT_INSTALLED);
    }
    match method {
        "memory.list" => list(core, read(params)?).await,
        "memory.search" => search(core, read(params)?).await,
        "memory.remember" => remember(core, stamp(core, id), read(params)?).await,
        "memory.update" => update(core, stamp(core, id), read(params)?).await,
        "memory.forget" => forget(core, stamp(core, id), read(params)?).await,
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 会话 `handle` 里人记一条（`/remember`，施工 R-3 补）：记进这个会话那一间，类 `user`，出处空，听众是 `by`，命令编号是
/// `id`（同一个编号再来交回头一次的）。交回记下的那句话（去掉了前后空白的）：回执照它说，不露编号（施工 R-3 四补）。
///
/// # Errors
///
/// 这个会话没有记忆（范围 `off`、场所会话）：`memory_unavailable`；空的：`bad_params`；超长：`memory_too_long`。
pub(crate) async fn remember_in(
    core: &Arc<Core>,
    handle: &Handle,
    id: &CommandId,
    by: By,
    text: &str,
) -> Result<String, Refusal> {
    let keeper = keeper_of(core, handle, by.clone())?;
    let text = checked(text)?;
    let remember = Remember {
        class: "user".to_string(),
        text: text.clone(),
        replaces: None,
    };
    let stamp = Stamp {
        at: now(),
        by,
        cause: Some(id.clone()),
    };
    blocking({
        let keeper = keeper.clone();
        move || keeper.save(stamp, remember, Vec::new())
    })
    .await?;
    fill(core, keeper).await;
    Ok(text)
}

async fn list(core: &Arc<Core>, params: List) -> Result<Value, Refusal> {
    let filter = Filter {
        class: params.class,
        from: params.from.as_deref().map(session_id).transpose()?,
        forgotten: params.forgotten,
        limit: limit(params.limit, LIST)?,
    };
    let keeper = find(core, params.at).await?;
    let found = blocking(move || keeper.list(&filter).map_err(Refused::Failed)).await?;
    Ok(json!({"memories": found.iter().map(shown).collect::<Vec<Value>>()}))
}

async fn search(core: &Arc<Core>, params: Search) -> Result<Value, Refusal> {
    if params.query.trim().is_empty() {
        return Err(Refusal::BAD_PARAMS);
    }
    let limit = limit(params.limit, SEARCH)?;
    let keeper = find(core, params.at).await?;
    let (query, forgotten) = (params.query, params.forgotten);
    // 照意思找（施工 R-5 下）照这时的配置：先算问句的向量（`off` 的、等不到的只走关键词），搜完在后台补这一间缺的。远程的
    // 用量记在管理员名下（M8 只有他，和 `model.call` 一样）。
    let using = using(core);
    let near = match keeper.vectors() {
        Some(vectors) => vectors.query(&using, &query).await,
        None => None,
    };
    let searched = keeper.clone();
    let found = blocking(move || {
        searched
            .search(&query, forgotten, limit, near.as_ref())
            .map_err(Refused::Failed)
    })
    .await?;
    if keeper.vectors().is_some() {
        blocking(move || {
            keeper.fill(&using);
            Ok(())
        })
        .await?;
    }
    Ok(json!({"memories": found.iter().map(shown).collect::<Vec<Value>>()}))
}

async fn remember(
    core: &Arc<Core>,
    stamp: Stamp,
    params: params::Remember,
) -> Result<Value, Refusal> {
    if !CLASSES.contains(&params.class.as_str()) {
        return Err(Refusal::BAD_PARAMS);
    }
    let remember = Remember {
        class: params.class,
        text: checked(&params.text)?,
        replaces: None,
    };
    let keeper = find(core, params.at).await?;
    let id = blocking({
        let keeper = keeper.clone();
        move || keeper.save(stamp, remember, Vec::new())
    })
    .await?;
    fill(core, keeper).await;
    Ok(json!({"id": id.to_string()}))
}

async fn update(core: &Arc<Core>, stamp: Stamp, params: Update) -> Result<Value, Refusal> {
    let id = memory_id(&params.id)?;
    let text = checked(&params.text)?;
    let keeper = find(core, params.at).await?;
    let id = blocking({
        let keeper = keeper.clone();
        move || keeper.update(stamp, id, text)
    })
    .await?;
    fill(core, keeper).await;
    Ok(json!({"id": id.to_string()}))
}

async fn forget(core: &Arc<Core>, stamp: Stamp, params: Forget) -> Result<Value, Refusal> {
    match (params.id, params.clear.as_deref()) {
        (Some(id), None) => {
            let id = memory_id(&id)?;
            let why = params.why.unwrap_or_default();
            let keeper = find(core, params.at).await?;
            blocking(move || keeper.retire(stamp, id, why)).await?;
            Ok(json!({}))
        }
        (None, Some(clear @ ("session" | "me"))) => {
            // 清会话的：那个会话就是写的 `session`；会话那一间的就是整间。
            let session = match clear {
                "session" => Some(session_id(
                    params.at.session.as_deref().ok_or(Refusal::BAD_PARAMS)?,
                )?),
                _ => None,
            };
            let keeper = find(core, params.at).await?;
            let session = session.filter(|_| matches!(keeper.room(), Room::Persona { .. }));
            let cleared = blocking(move || keeper.clear(stamp, session)).await?;
            Ok(json!({"cleared": cleared}))
        }
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 人经协议写的：此刻、管理员、这条命令的编号。
fn stamp(core: &Core, id: &CommandId) -> Stamp {
    Stamp {
        at: now(),
        by: admin(core),
        cause: Some(id.clone()),
    }
}

/// 照参数找哪一间，听众是这个连接的人（管理员）。
async fn find(core: &Arc<Core>, at: Where) -> Result<Keeper, Refusal> {
    let hearer = admin(core);
    match (at.persona, at.session, at.as_external) {
        (_, _, Some(_)) | (Some(_), Some(_), None) => Err(Refusal::BAD_PARAMS),
        (persona, None, None) => {
            // 不带人格的会话记忆不生效（17 L17）：不写、默认人格也没设的，没有哪一间。
            let found = personas::resolve(core, persona.as_deref().map(Some))
                .await?
                .ok_or(Refusal::MEMORY_UNAVAILABLE)?;
            let account = personas::memory_account(Some(&found), &core.admin);
            let room = Room::persona(&account, &found.id);
            Ok(Keeper::new(&memory(core)?, room, vec![hearer]))
        }
        (None, Some(session), None) => {
            let session = session_id(&session)?;
            let found = core.sessions.get(core, &session).await?;
            keeper_of(core, &found.handle, hearer)
        }
    }
}

/// 这个会话能不能记（施工 O-6 补，`/remember` 在 `command.catalog` 里列不列）：同 [`keeper_of`] 的判法。
pub(crate) fn remembers(core: &Core, handle: &Handle) -> bool {
    room(handle).is_some() && memory(core).is_ok()
}

/// 会话 `handle` 用的那一间，听众是 `hearer`：本机的、记忆开着的才有。
fn keeper_of(core: &Core, handle: &Handle, hearer: By) -> Result<Keeper, Refusal> {
    let room = room(handle).ok_or(Refusal::MEMORY_UNAVAILABLE)?;
    Ok(Keeper::new(&memory(core)?, room.clone(), vec![hearer]))
}

/// 会话 `handle` 用的那一间：本机的、记忆开着的才有。
fn room(handle: &Handle) -> Option<&Room> {
    handle
        .memory_room()
        .filter(|_| handle.venue().as_str() == LOCAL)
}

/// 照意思找的那一路照这时的配置（施工 R-5 下）：远程的用量记在管理员名下（M8 只有他，和 `model.call` 一样）。
fn using(core: &Core) -> Using {
    let config = core.config_now().borrow().clone();
    let resolved = config.resolved().clone();
    let source: Arc<dyn ConfigSource> = config;
    Using {
        config: Arc::new(Turn::new(resolved, source)),
        owner: core.admin.clone(),
    }
}

/// 记下、改了以后在后台补这一间缺的向量（施工 R-5 五补：不等下一次搜）。没接向量的、`off` 的什么都不做。
async fn fill(core: &Core, keeper: Keeper) {
    if keeper.vectors().is_none() {
        return;
    }
    let using = using(core);
    // 记下已经成了，回应照常：补不成的 `blocking` 记了 `WARN memory failed`，补本身的出错 `Keeper::fill` 自己记。
    let _filled: Result<(), Refusal> = blocking(move || {
        keeper.fill(&using);
        Ok(())
    })
    .await;
}

/// 人格记忆这个软件包这时装着没有（施工 R-10）：核心照清单设在记忆上（`Memory::set_installed`）。没有记忆的核心照装着算，
/// 由后面的检查说这里没有记忆。
pub(crate) fn installed(core: &Core) -> bool {
    core.memory_for(&core.admin)
        .is_none_or(|memory| memory.installed())
}

/// 核心一份的记忆：管理员的才有（多用户以后照账号）。人格记忆没装的当没有（施工 R-10）。
fn memory(core: &Core) -> Result<Arc<miyu_session::Memory>, Refusal> {
    core.memory_for(&core.admin)
        .filter(|memory| memory.installed())
        .ok_or(Refusal::MEMORY_UNAVAILABLE)
}

/// 一条在回应里的样子。
fn shown(entry: &Entry) -> Value {
    let by = serde_json::to_value(&entry.by)
        .ok()
        .and_then(|by| by.get("kind").cloned())
        .unwrap_or(Value::Null);
    json!({
        "id": entry.id.to_string(),
        "class": entry.class,
        "text": entry.text,
        "at": entry.at.to_string(),
        "by": by,
        "sources": entry.sources,
        "retired": entry.retired,
    })
}

/// 正文：去掉前后空白，空的参数不对，超过 [`TEXT_CHARS`] 个字的太长。
fn checked(text: &str) -> Result<String, Refusal> {
    let text = text.trim();
    let chars = text.chars().count();
    match chars {
        0 => Err(Refusal::BAD_PARAMS),
        _ if chars > TEXT_CHARS => Err(Refusal::memory_too_long(chars, TEXT_CHARS)),
        _ => Ok(text.to_string()),
    }
}

/// `limit`：不写的照 `default`，1 到 [`LIMIT`]，别的参数不对。
fn limit(limit: Option<usize>, default: usize) -> Result<usize, Refusal> {
    match limit.unwrap_or(default) {
        limit @ 1..=LIMIT => Ok(limit),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

fn memory_id(text: &str) -> Result<MemoryId, Refusal> {
    MemoryId::parse(text).ok_or(Refusal::BAD_PARAMS)
}

fn session_id(text: &str) -> Result<SessionId, Refusal> {
    SessionId::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}

fn read<T: DeserializeOwned>(params: &Value) -> Result<T, Refusal> {
    serde_json::from_value(params.clone()).map_err(|_| Refusal::BAD_PARAMS)
}

/// 在阻塞线程里做，[`Refused`] 写成拒绝：没有的、听众不合的 `unknown_memory`，不算了的 `memory_not_current`，磁盘上出的错
/// 记一行、是内部出错。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refused> + Send + 'static,
) -> Result<T, Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(Refused::NoSuch(_))) => Err(Refusal::UNKNOWN_MEMORY),
        Ok(Err(Refused::NotCurrent(_))) => Err(Refusal::MEMORY_NOT_CURRENT),
        Ok(Err(Refused::Failed(error))) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "memory failed");
            Err(Refusal::INTERNAL)
        }
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "memory failed");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 抽取要的几样（施工 R-6 上，`memory.md` 第六条）：请求模型的照一次性入口（照剧本回的端口没有，不抽）、资源目录里的字和 key 的
/// 写法、管理员的 blob（抽取的请求只有字，用不上）。读不出来的记一行，没有：这个核心不抽，别的照常。
pub(crate) fn extraction(
    models: &dyn miyu_session::Models,
    resources: &miyu_store::resources::ResourceRoot,
    root: &miyu_store::root::DataRoot,
    admin: &miyu_kernel::id::AccountId,
) -> Option<miyu_session::Extraction> {
    let ask = models.one_shot()?;
    let texts = miyu_session::ExtractTexts::load(resources.path())
        .and_then(|texts| Ok((texts, miyu_session::MergeTexts::load(resources.path())?)))
        .map_err(|error| error.to_string());
    let shapes = resources
        .memory_secrets()
        .map_err(|error| error.to_string())
        .and_then(|text| miyu_recall::redact::KeyShapes::parse(&text));
    match texts.and_then(|texts| shapes.map(|shapes| (texts, shapes))) {
        Ok(((texts, merge), shapes)) => Some(miyu_session::Extraction {
            texts,
            shapes,
            ask,
            blobs: miyu_store::blob::Blobs::new(root.blobs(admin)),
            idle: None,
            merge,
        }),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "memory extraction unavailable");
            None
        }
    }
}
