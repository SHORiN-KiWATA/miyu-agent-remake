//! 载入（`docs/designs/07-存储.md` 第七节）：打开日志，照第一条的策略哈希取回快照，交给内核从日志重建，再起 actor。换过
//! 快照的（施工 P-1 再补）照最近换上的那一份。

use std::path::Path;
use std::sync::Arc;

use tokio::sync::mpsc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;
use miyu_kernel::session::{Input, Session};
use miyu_policy::Snapshot;
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::usage::Who;
use miyu_tool::Log;

use crate::TARGET;
use crate::actor::persona::Refresh;
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
use crate::memory::{self, connect};
use crate::port::ForSession;
use crate::report::{Reporter, Upstream, wake_children};
use crate::spawn::Lineage;
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;

use super::{Load, LoadError, ledger_of};

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
        personas,
        resources,
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
        memory,
        presets,
    } = setup;
    let span = actor::span(&id);
    let config = Turning::start(configs, environment.cwd.clone()).await;
    let dir = root.session_dir(&owner, &id);
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (watching, shipped, stored) = (personas.clone(), resources.clone(), blobs.clone());
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (owner_of, id_of) = (owner.clone(), id.clone());
    let (log, events, (created, command), (snapshot, pools), policy, texts, run, guard, wired) =
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
            let bytes = store
                .get(current_policy(&events).unwrap_or(&created.policy))
                .map_err(LoadError::Blob)?;
            let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
            let policy = snapshot.policy().map_err(LoadError::Policy)?;
            let texts = snapshot.driver_texts().map_err(LoadError::Policy)?;
            let run = snapshot.run_texts().map_err(LoadError::Policy)?;
            let guard = snapshot.guard_texts().map_err(LoadError::Policy)?;
            // 能选的池照快照读回（施工 8-8 补）：造会话时拼的那一份，不重拼。
            let pools = Agents::pools_in(&snapshot.tools);
            // 快照里的范围已经照预设算过（施工 P-2 中），这里只再管子会话。
            let scope = memory::scope(created.parent.is_some(), true, snapshot.memory_scope());
            let turns = connect(
                memory.as_ref(),
                scope,
                &personas.memory_account(&snapshot.persona, &owner_of),
                &owner_of,
                &snapshot.persona,
                &id_of,
                &events,
            );
            Ok((
                log,
                events,
                (created, command),
                (snapshot, pools),
                policy,
                texts,
                run,
                guard,
                turns,
            ))
        })
        .await?;
    let (turns, calls) = wired;
    let attended = snapshot.attended;
    let upstream = Upstream::of(
        sessions.as_ref(),
        created.parent.as_ref(),
        command.as_ref(),
        &id,
    );
    let port = sessions.clone();
    let who = Who::of(&created);
    let venue = created.venue.clone();
    // 换预设时重新筛工具面要的（施工 P-2 下）：场所、父会话和第几层，照 `session.created`。
    let refresh_venue = created.venue.clone();
    let refresh_lineage = created.parent.clone().map(|parent| Lineage {
        parent,
        depth: created.depth.unwrap_or(1),
    });
    let asks = Agents::asks(&created.venue, created.parent.as_ref(), attended);
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
            preset: created.preset.clone(),
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
            turns,
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
            asks,
            memory: calls,
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
    actor.watch_persona(Refresh {
        personas: watching,
        resources: shipped,
        blobs: stored,
        snapshot,
        child: created.parent.is_some(),
        presets,
        tools: tools.clone(),
        venue: refresh_venue,
        lineage: refresh_lineage,
    });
    span.in_scope(|| {
        tracing::info!(target: TARGET, events = count, "loaded");
    });
    if let Some(port) = &port {
        wake_children(port, waiting, &span);
    }
    actor::spawn(actor, first, span);
    Ok(Handle::new(
        id,
        venue,
        inbox,
        busy,
        created.oneshot,
        watched,
        shown,
    ))
}

/// 现在的快照（施工 P-1 再补）：整份日志里最近一条带 `policy` 的 `session.policy_changed`，撤掉的回合里的也算（换快照不是
/// 对话的一部分）；没换过的没有，照 `session.created` 的。
fn current_policy(events: &[Event]) -> Option<&ContentHash> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed.policy.as_ref(),
        _ => None,
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
