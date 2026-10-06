//! 造会话、载入（`docs/designs/07-存储.md` 第四、七节，施工 3-6 上的策略快照）：备好磁盘上的，交给
//! 内核造会话、或者从日志重建，再起 actor。磁盘上的事都在阻塞线程里做。

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::{Body, Event, SessionCreated};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::session::{Input, Session};
use miyu_models::provider::chat;
use miyu_policy::{Snapshot, compose};
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, abandon};
use miyu_store::usage::{UsageIndex, Who};
use miyu_tool::{Log, Seen};

use crate::TARGET;
use crate::actor::{self, Actor, JobKit};
use crate::agents::{Agents, job_in};
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::config::Turning;
use crate::effects;
use crate::guard::Guard;
use crate::handle::Handle;
use crate::job_ids::JobIds;
use crate::jobs::Roster;
use crate::port::ForSession;
use crate::report::{Reporter, Upstream, wake_children};
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;
use crate::usage::Ledger;

mod error;
mod setup;

pub use error::{CreateError, LoadError};
pub use setup::{Create, Load};

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
        index,
        usage,
        configs,
        model,
    } = setup;
    let span = actor::span(&id);
    let config = Turning::start(configs, environment.cwd.clone()).await;
    // 没指定的照这时的 `models.chat`：记进 `session.created`，以后照它（施工 8-8）。
    let reference = model.or_else(|| chat(&config.current().resolved.values()));
    let (resources, name) = (resources.clone(), persona.to_string());
    // 工具面照这时的配置拼：`subagent` 能选哪几个池（施工 8-8 补），以后照快照、载入不重拼。
    let face = Agents::face(
        tools,
        &venue,
        lineage.as_ref(),
        &config.current().resolved.values(),
    );
    let pools = Agents::pools_in(&face);
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
        let lines = resources.core_lines().map_err(CreateError::Persona)?;
        let snapshot = snapshot.with_core_lines(&lines);
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
    models.ready().await;
    let model = models.port(ForSession {
        id: id.clone(),
        owner: owner.clone(),
        config: Arc::clone(config.current()),
        texts,
        blobs,
        reference: reference.clone(),
        sent: None,
    });
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
            pools,
        })
    });
    let created = SessionCreated {
        oneshot,
        cwd: Some(environment.cwd.clone()),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
        depth: lineage.as_ref().map(|lineage| lineage.depth),
        model: reference,
        ..snapshot.session_created(owner.clone(), venue.clone(), permission)
    };
    let (mut session, first) = Session::create(
        id.clone(),
        command.clone(),
        by,
        clock.now(),
        created,
        policy,
        environment,
    );
    // 模型的限额在别的输入之前交（施工 6-3 上）：什么动作都不出。给头看的那一份由 actor 当场要，`Handle` 和它共用（施工
    // 6-3 补；施工 8-9 起会变）。
    session.handle(Input::Limits(model.limits()));
    // 子会话领的号带上它在父会话里的编号，照造它的命令读回（施工 7-1 补）。
    let prefix = lineage
        .as_ref()
        .and_then(|lineage| job_in(&lineage.parent, &command));
    let job_ids = Arc::new(JobIds::starting_after(prefix, session.last_job_number()));
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
    let who = Who {
        owner: owner.clone(),
        venue: venue.clone(),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
    };
    let ledger = ledger_of(usage.as_ref(), &id, &owner);
    let mut actor = Actor::new(
        session,
        Box::new(Indexed::new(
            log,
            &id,
            index,
            usage.map(|usage| (usage, who)),
        )),
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
            ledger,
        },
        jobs,
        guard,
        mailbox,
        clock,
        config,
    );
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
    let (reply, answer) = oneshot::channel();
    actor.wait_for(command, reply);
    span.in_scope(|| {
        tracing::info!(target: TARGET, persona, venue = venue.as_str(), tools = count, "created");
    });
    actor::spawn(actor, first, span);
    match answer.await {
        Ok(_) => Ok(Handle::new(id, inbox, busy, oneshot, watched, shown)),
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
        index,
        usage,
        configs,
    } = setup;
    let span = actor::span(&id);
    let config = Turning::start(configs, environment.cwd.clone()).await;
    let dir = root.session_dir(&owner, &id);
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (log, events, (created, command), (attended, pools), policy, texts, run, guard) =
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
            // 能选的池照快照读回（施工 8-8 补）：造会话时拼的那一份，不重拼。
            let chosen = (snapshot.attended, Agents::pools_in(&snapshot.tools));
            Ok((
                log,
                events,
                (created, command),
                chosen,
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
    let who = Who::of(&created);
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
            pools,
        })
    });
    let kept = blobs.clone();
    models.ready().await;
    // 系统时间比日志里最后一条还早（往回拨过），照最后一条的：时刻不往回走。
    let mut clock = events
        .last()
        .map_or_else(Clock::default, |event| Clock::since(event.at));
    let count = events.len();
    // 她看过的文件（施工 4-6 上）、派出去的任务（施工 7-4）、最近发给了谁（施工 8-8）从日志里重建：内核收走日志之前。
    let seen = effects::seen_in(&events);
    let roster = Roster::from_events(&events);
    let sent = last_sent(&events);
    let (mut session, first) = Session::load(id.clone(), events, clock.now(), policy, environment)
        .map_err(LoadError::Kernel)?;
    // 路由照内核从日志算的引用造（施工 8-10）：换过模型的是换过以后的。
    let model = models.port(ForSession {
        id: id.clone(),
        owner: owner.clone(),
        config: Arc::clone(config.current()),
        texts,
        blobs,
        reference: session.reference().map(str::to_string),
        sent,
    });
    // 重启以后接着干的那一轮，发主请求之前就知道限额（施工 6-3 上）；给头看的限额同上（施工 6-3 补）。检查点重读过的
    // 文件，内核在载入吐出来的动作里第一个要回原文（施工 6-9），actor 起来先做它。
    session.handle(Input::Limits(model.limits()));
    // 子会话领的号带上它在父会话里的编号，照 `session.created` 的 `cause` 读回（施工 7-1 补）。
    let prefix = created
        .parent
        .as_ref()
        .zip(command.as_ref())
        .and_then(|(parent, command)| job_in(parent, command));
    let job_ids = Arc::new(JobIds::starting_after(prefix, session.last_job_number()));
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
    let ledger = ledger_of(usage.as_ref(), &id, &owner);
    let mut actor = Actor::new(
        session,
        Box::new(Indexed::new(
            log,
            &id,
            index,
            usage.map(|usage| (usage, who)),
        )),
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
            ledger,
        },
        jobs,
        guard,
        mailbox,
        clock,
        config,
    );
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
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
    Ok(Handle::new(
        id,
        inbox,
        busy,
        created.oneshot,
        watched,
        shown,
    ))
}

/// 用量汇总里的这个会话（施工 8-15）：`session_usage` 的端口照它造。没开汇总的没有。
fn ledger_of(usage: Option<&Arc<UsageIndex>>, id: &SessionId, owner: &AccountId) -> Option<Ledger> {
    usage.map(|index| Ledger {
        index: Arc::clone(index),
        session: id.clone(),
        owner: owner.clone(),
    })
}

/// 最近一条发出去了的 `model.called` 发给了谁（施工 8-8）：钉住的池载入时照它认钉着的成员（「起草时定的」第 2 条）。
fn last_sent(events: &[Event]) -> Option<Model> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::ModelCalled(called) => Some(Model {
            endpoint: called.endpoint.clone()?,
            model: called.model.clone()?,
        }),
        _ => None,
    })
}

#[cfg(test)]
mod tests;
