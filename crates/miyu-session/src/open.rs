//! 造会话、载入（`docs/designs/07-存储.md` 第四、七节，施工 3-6 上的策略快照）：备好磁盘上的，交给
//! 内核造会话、或者从日志重建，再起 actor。磁盘上的事都在阻塞线程里做。

use std::fmt;
use std::io;
use std::path::Path;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::{Body, Permission, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{LoadError as Broken, Session};
use miyu_policy::{BuildError, Snapshot, SnapshotError, ToolEntry, compose};
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::log::{OpenError, SEGMENT_LIMIT, SessionLog};
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use crate::TARGET;
use crate::actor::{self, Actor};
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::guard::Guard;
use crate::handle::Handle;
use crate::port::{ForSession, Models};
use crate::tools::ToolKit;

/// 造一个会话要的。
pub struct Create<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 资源目录：人格的原文从这里读。
    pub resources: &'a ResourceRoot,
    /// 会话编号，照 [`crate::new_id`] 造。
    pub id: SessionId,
    /// 照哪个人格造。
    pub persona: &'a str,
    /// 在哪个场所。
    pub venue: VenueId,
    /// 会话的属主：会话、blob 都在他的家目录里。
    pub owner: AccountId,
    /// 开始时的权限。
    pub permission: Permission,
    /// 有没有人能确认（`02-内核.md` 第六节「确认怎么走」第 2 条）。
    pub attended: bool,
    /// 一次性的：`miyu ask` 开的（`22-命令行.md` O2，施工 3-9 下）。
    pub oneshot: bool,
    /// 会话所在的环境：时区、工作目录。
    pub environment: Environment,
    /// 造会话的那个命令的编号：`session.created` 的 `cause`。
    pub command: CommandId,
    /// 谁发的造会话。
    pub by: By,
    /// 给会话造请求模型的端口：驱动的占位取自这个会话的策略快照。
    pub models: &'a dyn Models,
    /// 工具目录：照它存下这个会话的工具面（施工 4-1），以后一直照快照发。
    pub tools: &'a Catalog,
    /// 系统的家目录：权限策略照它换 `~`、找工具链目录（施工 4-3 下）。读不出来的是空的。
    pub home: Option<&'a Path>,
}

/// 载入一个会话要的。
pub struct Load<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 会话的属主。
    pub owner: AccountId,
    /// 会话编号。
    pub id: SessionId,
    /// 会话所在的环境：时区、工作目录。
    pub environment: Environment,
    /// 给会话造请求模型的端口：驱动的占位取自这个会话的策略快照。
    pub models: &'a dyn Models,
    /// 工具目录：执行工具时照名字在这里找（施工 4-2）。工具面照快照，不照它。
    pub tools: &'a Catalog,
    /// 系统的家目录：权限策略照它换 `~`、找工具链目录（施工 4-3 下）。读不出来的是空的。
    pub home: Option<&'a Path>,
}

/// 造不成。
#[derive(Debug)]
pub enum CreateError {
    /// 人格读不出来：编号不合写法，或者哪一份文件读不了。
    Persona(SourceError),
    /// 随核心附带的字造不出策略：安装坏了。
    Policy(BuildError),
    /// 存不下快照、建不了会话目录和日志。
    Disk(io::Error),
    /// 造会话那一条没落盘，会话就停了。
    Stopped,
}

/// 载入不了。
#[derive(Debug)]
pub enum LoadError {
    /// 日志打不开：没有这个会话，或者日志坏了。
    Log(OpenError),
    /// 日志里没有造会话那一条。
    NotCreated,
    /// 策略快照取不出来。
    Blob(BlobError),
    /// 策略快照读不懂。
    Snapshot(SnapshotError),
    /// 快照造不出策略。
    Policy(BuildError),
    /// 内核载入不了：日志过不了账本。
    Kernel(Broken),
}

/// 造一个会话：先把策略快照存成 blob（先落 blob，再写引用它的事件），再建会话目录和日志，交给内核
/// 造会话；`session.created` 落了盘，才交回 [`Handle`]。
///
/// # Errors
///
/// 人格读不出来、策略造不出来、磁盘上建不成；造会话那一条没落盘。
pub async fn create(setup: Create<'_>) -> Result<Handle, CreateError> {
    let Create {
        root,
        resources,
        id,
        persona,
        venue,
        owner,
        permission,
        attended,
        oneshot,
        environment,
        command,
        by,
        models,
        tools,
        home,
    } = setup;
    let span = actor::span(&id);
    let (resources, name) = (resources.clone(), persona.to_string());
    let face = face(tools);
    let count = face.len();
    let dir = root.session_dir(&owner, &id);
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (snapshot, policy, texts, run, guard, log) = blocking(move || {
        let sources = resources.sources(&name).map_err(CreateError::Persona)?;
        let snapshot = compose(&name, sources, attended).with_tools(face);
        let policy = snapshot.policy().map_err(CreateError::Policy)?;
        let texts = snapshot.driver_texts().map_err(CreateError::Policy)?;
        let run = snapshot.run_texts().map_err(CreateError::Policy)?;
        let guard = snapshot.guard_texts().map_err(CreateError::Policy)?;
        store.put(&snapshot.to_bytes()).map_err(CreateError::Disk)?;
        let log = SessionLog::create(&dir, SEGMENT_LIMIT).map_err(CreateError::Disk)?;
        Ok((snapshot, policy, texts, run, guard, log))
    })
    .await?;
    let model = models.port(ForSession { texts, blobs });
    let mut clock = Clock::default();
    let created = SessionCreated {
        oneshot,
        ..snapshot.session_created(owner, venue.clone(), permission)
    };
    let (session, first) = Session::create(
        command.clone(),
        by,
        clock.now(),
        created,
        policy,
        environment,
    );
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        guard,
    );
    let mut actor = Actor::new(
        session,
        Box::new(log),
        model,
        ToolKit {
            catalog: tools.clone(),
            texts: run,
            home: home.map(Path::to_path_buf),
        },
        guard,
        mailbox,
        clock,
    );
    let busy = actor.busy();
    let (reply, answer) = oneshot::channel();
    actor.wait_for(command, reply);
    span.in_scope(|| {
        tracing::info!(target: TARGET, persona, venue = venue.as_str(), tools = count, "created");
    });
    actor::spawn(actor, first, span);
    match answer.await {
        Ok(_) => Ok(Handle::new(id, inbox, busy)),
        Err(_) => Err(CreateError::Stopped),
    }
}

/// 目录里每件工具的规格，换成快照里的写法。
fn face(tools: &Catalog) -> Vec<ToolEntry> {
    tools
        .specs()
        .map(|spec| ToolEntry {
            name: spec.name.clone(),
            description: spec.description.clone(),
            parameters: spec.parameters.clone(),
            access: spec.access.clone(),
        })
        .collect()
}

/// 从磁盘载入一个会话：打开日志（自检、截尾），照第 1 条的策略哈希取快照、造策略，交给内核载入。
/// 内核吐出来的动作照样回：有计划的重启打断了的一轮，接着干。
///
/// # Errors
///
/// 日志打不开或者坏了、快照取不出来或者读不懂、内核载入不了。
pub async fn load(setup: Load<'_>) -> Result<Handle, LoadError> {
    let Load {
        root,
        owner,
        id,
        environment,
        models,
        tools,
        home,
    } = setup;
    let span = actor::span(&id);
    let dir = root.session_dir(&owner, &id);
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (log, events, policy, texts, run, guard) = blocking(move || {
        let (log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).map_err(LoadError::Log)?;
        let hash = match events.first().map(|event| &event.body) {
            Some(Body::SessionCreated(created)) => created.policy.clone(),
            _ => return Err(LoadError::NotCreated),
        };
        let bytes = store.get(&hash).map_err(LoadError::Blob)?;
        let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
        let policy = snapshot.policy().map_err(LoadError::Policy)?;
        let texts = snapshot.driver_texts().map_err(LoadError::Policy)?;
        let run = snapshot.run_texts().map_err(LoadError::Policy)?;
        let guard = snapshot.guard_texts().map_err(LoadError::Policy)?;
        Ok((log, events, policy, texts, run, guard))
    })
    .await?;
    let model = models.port(ForSession { texts, blobs });
    // 系统时间比日志里最后一条还早（往回拨过），照最后一条的：时刻不往回走。
    let mut clock = events
        .last()
        .map_or_else(Clock::default, |event| Clock::since(event.at));
    let count = events.len();
    let (session, first) =
        Session::load(events, clock.now(), policy, environment).map_err(LoadError::Kernel)?;
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        guard,
    );
    let actor = Actor::new(
        session,
        Box::new(log),
        model,
        ToolKit {
            catalog: tools.clone(),
            texts: run,
            home: home.map(Path::to_path_buf),
        },
        guard,
        mailbox,
        clock,
    );
    let busy = actor.busy();
    span.in_scope(|| {
        tracing::info!(target: TARGET, events = count, "loaded");
    });
    actor::spawn(actor, first, span);
    Ok(Handle::new(id, inbox, busy))
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateError::Persona(error) => write!(f, "人格读不出来：{error}"),
            CreateError::Policy(error) => write!(f, "造不出策略：{error}"),
            CreateError::Disk(error) => write!(f, "磁盘上建不成会话：{error}"),
            CreateError::Stopped => write!(f, "造会话那一条没落盘，会话停了"),
        }
    }
}

impl std::error::Error for CreateError {}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Log(error) => write!(f, "会话日志打不开：{error}"),
            LoadError::NotCreated => write!(f, "会话日志里没有造会话那一条"),
            LoadError::Blob(error) => write!(f, "策略快照取不出来：{error}"),
            LoadError::Snapshot(error) => write!(f, "策略快照读不懂：{error}"),
            LoadError::Policy(error) => write!(f, "快照造不出策略：{error}"),
            LoadError::Kernel(error) => write!(f, "载入不了：{error}"),
        }
    }
}

impl std::error::Error for LoadError {}
