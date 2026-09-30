//! 造会话、载入（`docs/designs/07-存储.md` 第四、七节，施工 3-6 上的策略快照）：备好磁盘上的，交给
//! 内核造会话、或者从日志重建，再起 actor。磁盘上的事都在阻塞线程里做。

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::{Body, Event, Permission, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Input, LoadError as Broken, Session};
use miyu_policy::{BuildError, Snapshot, SnapshotError, compose};
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::log::{OpenError, SEGMENT_LIMIT, SessionLog, abandon};
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_store::root::DataRoot;
use miyu_tool::{Catalog, Log, Seen};

use crate::TARGET;
use crate::actor::{self, Actor, JobKit};
use crate::agents::Agents;
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::effects;
use crate::guard::Guard;
use crate::handle::Handle;
use crate::job_ids::JobIds;
use crate::jobs::{Jobs, Roster};
use crate::port::{ForSession, Models};
use crate::report::{Reporter, Upstream, wake_children};
use crate::sandbox::SandboxCache;
use crate::spawn::{Lineage, SessionPort};
use crate::store::LogDir;
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
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。权限策略照它判执行命令，执行器照它
    /// 给每次调用写沙盒。
    pub sandbox: Option<&'a Path>,
    /// 沙盒的缓存：属主的那一份在哪、你的 cargo 目录在哪（施工 5-4 下）。核心算不出缓存目录的没有，沙盒里不设工具链的
    /// 变量。
    pub sandbox_cache: Option<SandboxCache>,
    /// 父会话和第几层（施工 7-5）：子会话才有，写进 `session.created`；system 接上子会话的场所说明；到了深度上限的，
    /// 工具面里不给 `agent`。
    pub lineage: Option<Lineage>,
    /// 造子会话、给别的会话发命令的端口（施工 7-5）：会话表交进来，派子代理经它。没有的（测试里自己造的），`agent` 照派
    /// 不了出错。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 执行器的任务表，核心里一张（施工 7-3）：后台命令交给它。
    pub jobs: &'a Arc<Jobs>,
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
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。权限策略照它判执行命令，执行器照它
    /// 给每次调用写沙盒。
    pub sandbox: Option<&'a Path>,
    /// 沙盒的缓存：属主的那一份在哪、你的 cargo 目录在哪（施工 5-4 下）。核心算不出缓存目录的没有，沙盒里不设工具链的
    /// 变量。
    pub sandbox_cache: Option<SandboxCache>,
    /// 造子会话、给别的会话发命令的端口（施工 7-5）：同 [`Create::sessions`]。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 执行器的任务表，核心里一张（施工 7-3）：后台命令交给它，任务编号照日志往后数。
    pub jobs: &'a Arc<Jobs>,
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
/// 造会话；`session.created` 落了盘，才交回 [`Handle`]。子会话（带着 [`Create::lineage`]）的 system 接上场所说明
/// （施工 7-5）。
///
/// # Errors
///
/// 人格、子会话的场所说明读不出来，策略造不出来、磁盘上建不成；造会话那一条没落盘。
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
        sandbox,
        sandbox_cache,
        lineage,
        sessions,
        jobs,
    } = setup;
    let span = actor::span(&id);
    let (resources, name) = (resources.clone(), persona.to_string());
    let face = Agents::face(tools, &venue, lineage.as_ref());
    let child = lineage.is_some();
    let count = face.len();
    let dir = root.session_dir(&owner, &id);
    let abandoned = dir.clone();
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (snapshot, policy, texts, run, guard, log) = blocking(move || {
        let sources = resources.sources(&name).map_err(CreateError::Persona)?;
        let mut snapshot = compose(&name, sources, attended).with_tools(face);
        if child {
            let venue = resources.subagent_venue().map_err(CreateError::Persona)?;
            snapshot = snapshot.with_venue(&venue);
        }
        let policy = snapshot.policy().map_err(CreateError::Policy)?;
        let texts = snapshot.driver_texts().map_err(CreateError::Policy)?;
        let run = snapshot.run_texts().map_err(CreateError::Policy)?;
        let guard = snapshot.guard_texts().map_err(CreateError::Policy)?;
        store.put(&snapshot.to_bytes()).map_err(CreateError::Disk)?;
        let log = SessionLog::create(&dir, SEGMENT_LIMIT).map_err(CreateError::Disk)?;
        Ok((snapshot, policy, texts, run, guard, log))
    })
    .await?;
    let kept = blobs.clone();
    let model = models.port(ForSession { texts, blobs });
    let mut clock = Clock::default();
    let upstream = Upstream::of(
        sessions.as_ref(),
        lineage.as_ref().map(|lineage| &lineage.parent),
        Some(&command),
        &id,
    );
    let agents = sessions.map(|port| {
        Arc::new(Agents {
            port,
            session: id.clone(),
            owner: owner.clone(),
            venue: venue.clone(),
            depth: Agents::depth_of(lineage.as_ref()),
            parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
            attended,
            reports: policy.reports.clone(),
        })
    });
    let created = SessionCreated {
        oneshot,
        cwd: Some(environment.cwd.clone()),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
        depth: lineage.as_ref().map(|lineage| lineage.depth),
        ..snapshot.session_created(owner, venue.clone(), permission)
    };
    let (mut session, first) = Session::create(
        command.clone(),
        by,
        clock.now(),
        created,
        policy,
        environment,
    );
    // 模型的限额在别的输入之前交（施工 6-3 上）：什么动作都不出。给头看的那一份当场要，`Handle` 带着（施工 6-3 补）。
    session.handle(Input::Limits(model.limits()));
    let limits = session.context_limits();
    let job_ids = Arc::new(JobIds::starting_after(session.last_job_number()));
    let jobs = JobKit {
        table,
        dir: jobs_dir,
        blobs: kept.clone(),
        ids: Arc::clone(&job_ids),
        roster: Roster::default(),
        agents: agents.clone(),
    };
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        guard,
        sandbox.is_some(),
    );
    let mut actor = Actor::new(
        session,
        Box::new(log),
        model,
        ToolKit {
            catalog: tools.clone(),
            texts: run,
            home: home.map(Path::to_path_buf),
            data_root: root.path().to_path_buf(),
            blobs: kept,
            seen: Seen::new(),
            sandbox: sandbox.map(Path::to_path_buf),
            sandbox_cache,
            log: Log::new(log_dir),
            offset,
            job_ids,
            agents,
        },
        jobs,
        guard,
        mailbox,
        clock,
    );
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    let busy = actor.busy();
    let (reply, answer) = oneshot::channel();
    actor.wait_for(command, reply);
    span.in_scope(|| {
        tracing::info!(target: TARGET, persona, venue = venue.as_str(), tools = count, "created");
    });
    actor::spawn(actor, first, span);
    match answer.await {
        Ok(_) => Ok(Handle::new(id, inbox, busy, limits)),
        Err(_) => {
            // 造会话那一条没落盘：只剩空的第一段的会话目录删掉；快照的 blob 留着，按内容存，别的会话可能也在用
            // （施工 4-9 再补四下：原来都留在磁盘上）。
            if let Err(error) = blocking(move || abandon(&abandoned)).await {
                tracing::warn!(
                    target: TARGET,
                    session = id.as_str(),
                    error = %error,
                    "abandoned session not removed"
                );
            }
            Err(CreateError::Stopped)
        }
    }
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
        sandbox,
        sandbox_cache,
        sessions,
        jobs,
    } = setup;
    let span = actor::span(&id);
    let dir = root.session_dir(&owner, &id);
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (log, events, (created, command), attended, policy, texts, run, guard) =
        blocking(move || {
            let (log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).map_err(LoadError::Log)?;
            let (created, command) = match events.first() {
                Some(Event {
                    body: Body::SessionCreated(created),
                    cause,
                    ..
                }) => (created.clone(), cause.clone()),
                _ => return Err(LoadError::NotCreated),
            };
            let bytes = store.get(&created.policy).map_err(LoadError::Blob)?;
            let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
            let policy = snapshot.policy().map_err(LoadError::Policy)?;
            let texts = snapshot.driver_texts().map_err(LoadError::Policy)?;
            let run = snapshot.run_texts().map_err(LoadError::Policy)?;
            let guard = snapshot.guard_texts().map_err(LoadError::Policy)?;
            let attended = snapshot.attended;
            Ok((
                log,
                events,
                (created, command),
                attended,
                policy,
                texts,
                run,
                guard,
            ))
        })
        .await?;
    let upstream = Upstream::of(
        sessions.as_ref(),
        created.parent.as_ref(),
        command.as_ref(),
        &id,
    );
    let port = sessions.clone();
    let agents = sessions.map(|port| {
        Arc::new(Agents {
            port,
            session: id.clone(),
            owner: owner.clone(),
            venue: created.venue,
            depth: created.depth.unwrap_or(0),
            parent: created.parent.clone(),
            attended,
            reports: policy.reports.clone(),
        })
    });
    let kept = blobs.clone();
    let model = models.port(ForSession { texts, blobs });
    // 系统时间比日志里最后一条还早（往回拨过），照最后一条的：时刻不往回走。
    let mut clock = events
        .last()
        .map_or_else(Clock::default, |event| Clock::since(event.at));
    let count = events.len();
    // 她看过的文件（施工 4-6 上）、派出去的任务（施工 7-4）从日志里重建：内核收走日志之前。
    let seen = effects::seen_in(&events);
    let roster = Roster::from_events(&events);
    let (mut session, first) =
        Session::load(events, clock.now(), policy, environment).map_err(LoadError::Kernel)?;
    // 重启以后接着干的那一轮，发主请求之前就知道限额（施工 6-3 上）；给头看的限额同上（施工 6-3 补）。检查点重读过的
    // 文件，内核在载入吐出来的动作里第一个要回原文（施工 6-9），actor 起来先做它。
    session.handle(Input::Limits(model.limits()));
    let limits = session.context_limits();
    let job_ids = Arc::new(JobIds::starting_after(session.last_job_number()));
    let jobs = JobKit {
        table,
        dir: jobs_dir,
        blobs: kept.clone(),
        ids: Arc::clone(&job_ids),
        roster,
        agents: agents.clone(),
    };
    let waiting = session.waiting_children();
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        guard,
        sandbox.is_some(),
    );
    let mut actor = Actor::new(
        session,
        Box::new(log),
        model,
        ToolKit {
            catalog: tools.clone(),
            texts: run,
            home: home.map(Path::to_path_buf),
            data_root: root.path().to_path_buf(),
            blobs: kept,
            seen,
            sandbox: sandbox.map(Path::to_path_buf),
            sandbox_cache,
            log: Log::new(log_dir),
            offset,
            job_ids,
            agents,
        },
        jobs,
        guard,
        mailbox,
        clock,
    );
    let busy = actor.busy();
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    span.in_scope(|| {
        tracing::info!(target: TARGET, events = count, "loaded");
    });
    if let Some(port) = &port {
        wake_children(port, waiting, &span);
    }
    actor::spawn(actor, first, span);
    Ok(Handle::new(id, inbox, busy, limits))
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateError::Persona(error) => write!(f, "persona not readable: {error}"),
            CreateError::Policy(error) => write!(f, "policy not built: {error}"),
            CreateError::Disk(error) => write!(f, "session not created on disk: {error}"),
            CreateError::Stopped => write!(f, "session.created not stored; the session stopped"),
        }
    }
}

impl std::error::Error for CreateError {}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Log(error) => write!(f, "session log not opened: {error}"),
            LoadError::NotCreated => write!(f, "the session log has no session.created"),
            LoadError::Blob(error) => write!(f, "policy snapshot not fetched: {error}"),
            LoadError::Snapshot(error) => write!(f, "policy snapshot not understood: {error}"),
            LoadError::Policy(error) => write!(f, "policy not built from the snapshot: {error}"),
            LoadError::Kernel(error) => write!(f, "not loaded: {error}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[cfg(test)]
mod tests;
