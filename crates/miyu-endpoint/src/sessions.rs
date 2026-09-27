//! 会话表（`docs/designs/07-存储.md` 第七节「会话按需载入」）：照编号找会话；这次运行里没在跑的，从磁盘
//! 载入；停了的拿掉，下次用到再载入。
//!
//! 表拿 tokio 的锁护着，载入期间一直拿着：两个连接同时说给同一个没在跑的会话，只载入一次、只起一个
//! actor（一个会话只能有一个写者，`07-存储.md` 第三节）。

use std::collections::{BTreeMap, VecDeque};

use tokio::sync::Mutex;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CommandId, SessionId, VenueId};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::{Create, CreateError, Handle, Load, LoadError, create, load, new_id};
use miyu_store::log::OpenError;
use miyu_store::resources::SourceError;

use crate::Core;
use crate::refusal::Refusal;

/// 记住最近多少个造会话的命令编号：断线重发的造会话不再造一个新的（`04-核心协议.md` 第六节第 1 条）。
const REMEMBERED: usize = 1024;

/// 会话表。
#[derive(Debug, Default)]
pub(crate) struct Sessions {
    open: Mutex<Open>,
}

#[derive(Debug, Default)]
struct Open {
    /// 在跑的会话，和它现在的工作目录。
    running: BTreeMap<SessionId, Running>,
    /// 最近造会话的命令编号，和它造出的会话。
    created: VecDeque<(CommandId, SessionId)>,
}

#[derive(Debug)]
struct Running {
    handle: Handle,
    cwd: String,
}

impl Sessions {
    /// 造一个会话：属主是管理员，在本机；有没有人能确认照 `attended`；`miyu ask` 开的是一次性的。
    /// 同一个命令编号重发，交回上一次造的那一个。
    pub(crate) async fn create(
        &self,
        core: &Core,
        command: CommandId,
        persona: &str,
        cwd: String,
        who: Opening,
    ) -> Result<SessionId, Refusal> {
        let mut open = self.open.lock().await;
        if let Some((_, session)) = open.created.iter().find(|(id, _)| *id == command) {
            return Ok(session.clone());
        }
        let id = new_id(now());
        let created = create(Create {
            root: &core.root,
            resources: &core.resources,
            id: id.clone(),
            persona,
            venue: local(),
            owner: core.admin.clone(),
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
            attended: who.attended,
            oneshot: who.oneshot,
            environment: environment(&cwd),
            command: command.clone(),
            by: admin(core),
            models: &*core.models,
            tools: &core.tools,
        })
        .await;
        let handle = match created {
            Ok(handle) => handle,
            Err(CreateError::Persona(SourceError::Persona(_))) => return Err(Refusal::BAD_PARAMS),
            Err(CreateError::Persona(SourceError::Read { .. })) => {
                return Err(Refusal::UNKNOWN_PERSONA);
            }
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", error = %error, "create failed");
                return Err(Refusal::INTERNAL);
            }
        };
        open.running.insert(id.clone(), Running { handle, cwd });
        open.created.push_back((command, id.clone()));
        if open.created.len() > REMEMBERED {
            open.created.pop_front();
        }
        Ok(id)
    }

    /// 找会话 `id`：在跑的直接交回；没在跑的从磁盘载入。头报上来的工作目录 `cwd` 和会话现在的不一样，
    /// 先送进会话。
    pub(crate) async fn get(
        &self,
        core: &Core,
        id: &SessionId,
        cwd: Option<&str>,
    ) -> Result<Handle, Refusal> {
        let mut open = self.open.lock().await;
        if let Some(running) = open.running.get_mut(id) {
            if let Some(cwd) = cwd
                && cwd != running.cwd
            {
                if running.handle.environment(environment(cwd)).is_err() {
                    open.running.remove(id);
                    return Err(Refusal::STOPPED);
                }
                running.cwd = cwd.to_string();
            }
            return Ok(running.handle.clone());
        }
        let cwd = cwd.unwrap_or("~").to_string();
        let loaded = load(Load {
            root: &core.root,
            owner: core.admin.clone(),
            id: id.clone(),
            environment: environment(&cwd),
            models: &*core.models,
        })
        .await;
        let handle = match loaded {
            Ok(handle) => handle,
            Err(LoadError::Log(OpenError::Missing(_))) => return Err(Refusal::NOT_FOUND),
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "load failed");
                return Err(Refusal::BROKEN);
            }
        };
        open.running.insert(
            id.clone(),
            Running {
                handle: handle.clone(),
                cwd,
            },
        );
        Ok(handle)
    }

    /// 会话 `id` 停了：从表里拿掉，下次用到再载入。
    pub(crate) async fn forget(&self, id: &SessionId) {
        self.open.lock().await.running.remove(id);
    }

    /// 有没有在跑的回合：哪个在跑的会话还忙着，就是有（施工 3-9 上）。
    pub(crate) async fn busy(&self) -> bool {
        let open = self.open.lock().await;
        open.running.values().any(|running| running.handle.busy())
    }

    /// 有计划地停下全部在跑的会话：跑到一半的回合记成「重启了」，下次载入接着干（施工 3-9 上）。
    pub(crate) async fn stop_all(&self) {
        let running = std::mem::take(&mut self.open.lock().await.running);
        for (id, running) in running {
            if running.handle.stop().await.is_err() {
                tracing::debug!(target: "miyu::endpoint", session = id.as_str(), "already stopped");
            }
        }
    }
}

/// 造会话时要记下的两样：有没有人能确认，是不是一次性的。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Opening {
    /// 有没有人能确认：头握手时报的。
    pub(crate) attended: bool,
    /// 一次性的：`miyu ask` 开的（施工 3-9 下）。
    pub(crate) oneshot: bool,
}

/// 管理员：本机连上来的都是他（`06-多用户与身份.md` 第二节）。
pub(crate) fn admin(core: &Core) -> By {
    By::Person(Person {
        account: core.admin.clone(),
    })
}

/// 本机这个场所。
fn local() -> VenueId {
    VenueId::parse("local").unwrap_or_else(|e| unreachable!("「local」合场所的写法：{e}"))
}

/// 会话所在的环境：核心所在的机器现在的时区，头报上来的工作目录。
fn environment(cwd: &str) -> Environment {
    Environment {
        offset: offset(),
        cwd: cwd.to_string(),
    }
}

/// 核心所在的机器现在的时区偏移，到分钟。读不出来的当 UTC。
fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes)
        .or_else(|| UtcOffset::from_minutes(0))
        .unwrap_or_else(|| unreachable!("UTC 在偏移的范围里"))
}

/// 现在：造会话编号用，编号的前 48 位是它。
fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0));
    Timestamp::from_unix_millis(millis)
        .or_else(|| Timestamp::from_unix_millis(0))
        .unwrap_or_else(|| unreachable!("1970 年在时刻的范围里"))
}
