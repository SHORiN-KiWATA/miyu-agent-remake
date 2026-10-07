//! 会话表（`docs/designs/07-存储.md` 第七节「会话按需载入」）：照编号找会话；这次运行里没在跑的，从磁盘
//! 载入；停了的拿掉，下次用到再载入。删会话连子会话在 `sessions/delete.rs`（施工 3-8 三补）。
//!
//! 表拿 tokio 的锁护着，载入期间一直拿着：两个连接同时说给同一个没在跑的会话，只载入一次、只起一个
//! actor（一个会话只能有一个写者，`07-存储.md` 第三节）。

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use miyu_kernel::event::{Body, Level, Permission};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CommandId, SessionId, VenueId};
use miyu_kernel::origin::{By, Person, Session};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::{Child, Create, Handle, create, new_id};
use miyu_store::log::first_event;

use crate::Core;
use crate::personas;
use crate::refusal::Refusal;
use crate::settings::PermissionSettings;
use crate::spawn;

mod delete;
mod found;
mod orphans;
#[cfg(test)]
mod tests;

/// 记住最近多少个造会话的命令编号：断线重发的造会话不再造一个新的（`04-核心协议.md` 第六节第 1 条）。核心重启以后
/// 第一次造会话时，从最新的这么多个会话的 `session.created` 里补回来（施工 4-9 再补三上）。
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
    /// 最近造会话的命令编号，和它造出的会话，照从旧到新。
    created: VecDeque<(CommandId, SessionId)>,
    /// 这次运行里补回过去重的编号没有：第一次造会话时补一次。
    recalled: bool,
}

#[derive(Debug)]
struct Running {
    handle: Handle,
    /// 头报上来的工作目录：下次报来的和它比。
    cwd: String,
    /// 实际在哪个目录里干活：`cwd` 太宽的，是账号的工作区。
    workspace: String,
    /// 加进来的目录（施工 5-10 上）：头下次报来的和它比。
    dirs: Vec<String>,
}

/// 造好的会话：编号，和它实际在哪个目录里干活（施工 4-5 下）；这个目录的项目配置还没问过信不信任的，它在哪（施工 8-2）。
#[derive(Debug)]
pub(crate) struct Created {
    pub(crate) id: SessionId,
    pub(crate) cwd: String,
    pub(crate) untrusted: Option<String>,
}

/// 找到的会话：把手，和它这会儿实际在哪个目录里干活（施工 4-5 下）。
#[derive(Debug)]
pub(crate) struct Found {
    pub(crate) handle: Handle,
    pub(crate) cwd: String,
}

impl Sessions {
    /// 造一个会话：属主是管理员，在本机；有没有人能确认照 `attended`；`miyu ask` 开的是一次性的。人格是 `persona`，
    /// 没写的照默认人格找（施工 P-1 上，`personas.rs`）。同一个命令编号重发，交回上一次造的那一个，人格不再找。
    pub(crate) async fn create(
        &self,
        core: &Arc<Core>,
        command: CommandId,
        persona: Option<&str>,
        cwd: String,
        dirs: Vec<String>,
        who: Opening,
    ) -> Result<Created, Refusal> {
        check_dirs(core, &dirs)?;
        let mut open = self.open.lock().await;
        if !open.recalled {
            open.recalled = true;
            let earlier = recall(core).await;
            for pair in earlier.into_iter().rev() {
                open.created.push_front(pair);
            }
            while open.created.len() > REMEMBERED {
                open.created.pop_front();
            }
        }
        let workspace = workspace(core, &cwd);
        // 开局只读照这个会话实际干活的目录算，带上信任着的项目配置（`config.md` 第二条第 9 条）。
        let (resolved, project) = core.config().with_project(&workspace);
        let untrusted = project.and_then(|project| project.untrusted());
        if let Some((_, session)) = open.created.iter().find(|(id, _)| *id == command) {
            let id = session.clone();
            return Ok(Created {
                id,
                cwd: workspace,
                untrusted,
            });
        }
        let read_only = PermissionSettings::from(&resolved.values()).start_read_only;
        let persona = personas::resolve(core, persona).await?;
        let id = new_id(now());
        let created = create(Create {
            root: &core.root,
            resources: &core.resources,
            id: id.clone(),
            persona: &persona.id,
            persona_texts: persona.texts.clone(),
            memory_account: personas::memory_account(&persona, &core.admin),
            venue: who.venue.clone().unwrap_or_else(local),
            owner: core.admin.clone(),
            permission: Permission {
                level: Level::Workspace,
                read_only,
            },
            attended: who.attended,
            oneshot: who.oneshot,
            environment: environment(workspace.clone(), dirs.clone()),
            command: command.clone(),
            by: admin(core),
            models: &*core.models,
            tools: &core.tools,
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            sandbox_cache: core.sandbox_cache_of(&core.admin),
            lineage: None,
            sessions: Some(spawn::port(core)),
            jobs: &core.jobs,
            index: core.index_for(&core.admin),
            usage: core.usage_for(&core.admin),
            memory: core.memory_for(&core.admin),
            configs: core.hub.configs(),
            model: who.model,
        })
        .await;
        let handle = match created {
            Ok(handle) => handle,
            // 人格已经找好了：`core/` 下哪一份读不了（安装坏了），是内部出错（施工 4-9 再补三上）。
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", error = %error, "create failed");
                return Err(Refusal::INTERNAL);
            }
        };
        open.running.insert(
            id.clone(),
            Running {
                handle,
                cwd,
                workspace: workspace.clone(),
                dirs,
            },
        );
        open.created.push_back((command, id.clone()));
        if open.created.len() > REMEMBERED {
            open.created.pop_front();
        }
        Ok(Created {
            id,
            cwd: workspace,
            untrusted,
        })
    }

    /// 找会话 `id`：在跑的直接交回；没在跑的从磁盘载入。头报上来的工作目录 `cwd`、加进来的目录 `dirs`（施工 5-10
    /// 上）和会话现在的不一样，先送进会话；`dirs` 里有太宽的，整条命令都不收。
    pub(crate) async fn get(
        &self,
        core: &Arc<Core>,
        id: &SessionId,
        cwd: Option<&str>,
        dirs: Option<&[String]>,
    ) -> Result<Found, Refusal> {
        if let Some(dirs) = dirs {
            check_dirs(core, dirs)?;
        }
        let mut open = self.open.lock().await;
        open.found(core, id, cwd, dirs).await
    }

    /// 造一个子会话（施工 7-5，`agents.md` 第一条）：照执行器填好的 `child`，由父会话造（`by` 是它）。放进表里，和头造的
    /// 一样照编号找得到：一个会话只起一个 actor。造不成的交回原因，由执行器记进运行日志。
    pub(crate) async fn spawn(&self, core: &Arc<Core>, child: Child) -> Result<SessionId, String> {
        let mut open = self.open.lock().await;
        // 父会话被删了（施工 3-8 三补）：它停下之前在派的不再造，不留下没有父会话的子会话。
        if !open.running.contains_key(&child.lineage.parent) {
            return Err("the parent session is gone".to_string());
        }
        let id = new_id(now());
        let parent = By::Session(Session {
            id: child.lineage.parent.clone(),
        });
        let persona = personas::resolve(core, Some(&child.persona))
            .await
            .map_err(|refusal| format!("persona {}: {}", child.persona, refusal.reason))?;
        let handle = create(Create {
            root: &core.root,
            resources: &core.resources,
            id: id.clone(),
            persona: &persona.id,
            persona_texts: persona.texts.clone(),
            memory_account: personas::memory_account(&persona, &child.owner),
            venue: child.venue,
            sandbox_cache: core.sandbox_cache_of(&child.owner),
            index: core.index_for(&child.owner),
            usage: core.usage_for(&child.owner),
            memory: core.memory_for(&child.owner),
            configs: core.hub.configs(),
            owner: child.owner,
            permission: child.permission,
            attended: child.attended,
            oneshot: false,
            environment: environment(child.cwd.clone(), child.dirs.clone()),
            command: child.command,
            by: parent,
            models: &*core.models,
            tools: &core.tools,
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            lineage: Some(child.lineage),
            sessions: Some(spawn::port(core)),
            jobs: &core.jobs,
            model: child.model,
        })
        .await
        .map_err(|error| error.to_string())?;
        // 工作目录是父会话这一轮实际干活的那一个，已经定过宽不宽。
        let running = Running {
            handle,
            cwd: child.cwd.clone(),
            workspace: child.cwd,
            dirs: child.dirs,
        };
        open.running.insert(id.clone(), running);
        Ok(id)
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

    /// 这时忙着的会话（施工 C-3）：在表里、有回合在进行，和 [`Sessions::busy`] 看的是同一样。列会话时照它写忙不忙。
    pub(crate) async fn busy_ids(&self) -> BTreeSet<SessionId> {
        let open = self.open.lock().await;
        open.running
            .iter()
            .filter(|(_, running)| running.handle.busy())
            .map(|(id, _)| id.clone())
            .collect()
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

/// 造会话时要记下的几样：有没有人能确认，是不是一次性的，用哪个模型。
#[derive(Debug, Clone)]
pub(crate) struct Opening {
    /// 有没有人能确认：头握手时报的。
    pub(crate) attended: bool,
    /// 一次性的：`miyu ask` 开的（施工 3-9 下）。
    pub(crate) oneshot: bool,
    /// 用哪个模型（施工 8-8）：`session.create` 的 `model` 照这时的配置解析好的引用，模型或 `@池`；没写的是空的，照这时的
    /// `models.chat`。
    pub(crate) model: Option<String>,
    /// 场所（施工 O-3）：通讯平台的场所会话写它，本机的是空的（`local`）。
    pub(crate) venue: Option<VenueId>,
}

/// 管理员：本机连上来的都是他（`06-多用户与身份.md` 第二节）。
pub(crate) fn admin(core: &Core) -> By {
    By::Person(Person::new(core.admin.clone()))
}

/// 核心重启以后补回去重的编号（施工 4-9 再补三上）：最新的 [`REMEMBERED`] 个会话，`session.created` 的 `cause` 就是
/// 造会话的命令编号。读不了的跳过。在阻塞线程里读，交回的照从旧到新。
async fn recall(core: &Core) -> Vec<(CommandId, SessionId)> {
    let root = core.root.clone();
    let admin = core.admin.clone();
    tokio::task::spawn_blocking(move || {
        let Ok(ids) = root.sessions(&admin) else {
            return Vec::new();
        };
        let mut found: Vec<(CommandId, SessionId)> = ids
            .into_iter()
            .take(REMEMBERED)
            .filter_map(|id| {
                let event = first_event(&root.session_dir(&admin, &id)).ok()?;
                match (&event.body, event.cause) {
                    (Body::SessionCreated(_), Some(cause)) => Some((cause, id)),
                    _ => None,
                }
            })
            .collect();
        found.reverse();
        found
    })
    .await
    .unwrap_or_default()
}

/// 本机这个场所。
fn local() -> VenueId {
    VenueId::parse("local").unwrap_or_else(|e| unreachable!("「local」合场所的写法：{e}"))
}

/// 会话所在的环境：核心所在的机器现在的时区，实际干活的目录（[`workspace`] 定的），加进来的目录（施工 5-10 上）。
fn environment(workspace: String, dirs: Vec<String>) -> Environment {
    Environment {
        offset: offset(),
        cwd: workspace,
        dirs,
    }
}

/// 加进来的目录里有太宽的：整条命令都不收（施工 5-10 上）。
fn check_dirs(core: &Core, dirs: &[String]) -> Result<(), Refusal> {
    if dirs.iter().any(|dir| dir_too_wide(core, dir)) {
        Err(Refusal::DIR_TOO_WIDE)
    } else {
        Ok(())
    }
}

/// 加进来的一个目录太不太宽：和工作目录同一套（`~` 本身、系统的家目录、根目录、包含数据根的），另外落在数据根里的
/// 一律算太宽，账号的工作区也不例外：工作目录太宽时有地方可退，加进来的目录没有。换不成真实位置的照原样，边界表里
/// 那一片不算。
fn dir_too_wide(core: &Core, dir: &str) -> bool {
    if dir.trim() == "~" {
        return true;
    }
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), dir) else {
        return false;
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    miyu_fs::within(&real, &data_root)
        || miyu_fs::too_wide(&real, home.as_deref(), &data_root, &data_root)
}

/// 拿头报上来的 `cwd` 当工作区。太宽的（`~` 本身、系统的家目录、根目录，包含数据根或者落在数据根里），退回
/// 管理员的工作区 `home/<账号>/workspace/`（`11-权限与沙盒.md` 第四节，施工 4-3 下）。换不成真实位置的照原样：
/// 说不清它宽不宽，用到时工具自己报错。
fn workspace(core: &Core, cwd: &str) -> String {
    let own = core.root.workspace(&core.admin);
    let fallback = || {
        // 建家目录时就建了；老的数据根里可能还没有，补上。建不了的照样退回：用到时工具自己报错。
        if let Err(error) = core.root.prepare_home(&core.admin) {
            tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "workspace not prepared");
        }
        own.to_string_lossy().into_owned()
    };
    if cwd.trim() == "~" {
        return fallback();
    }
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd) else {
        return cwd.to_string();
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    let own_real = std::fs::canonicalize(&own).unwrap_or_else(|_| own.clone());
    if miyu_fs::too_wide(&real, home.as_deref(), &data_root, &own_real) {
        fallback()
    } else {
        cwd.to_string()
    }
}

/// 核心所在的机器现在的时区偏移，到分钟。读不出来的当 UTC。
pub(crate) fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes)
        .or_else(|| UtcOffset::from_minutes(0))
        .unwrap_or_else(|| unreachable!("UTC 在偏移的范围里"))
}

/// 现在：造会话编号用，编号的前 48 位是它；配置的日志也照它记时刻（施工 8-3）。
pub(crate) fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0));
    Timestamp::from_unix_millis(millis)
        .or_else(|| Timestamp::from_unix_millis(0))
        .unwrap_or_else(|| unreachable!("1970 年在时刻的范围里"))
}
