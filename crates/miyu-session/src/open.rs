//! 造会话、载入（`docs/designs/07-存储.md` 第四、七节，施工 3-6 上的策略快照）：备好磁盘上的，交给
//! 内核造会话、或者从日志重建，再起 actor。磁盘上的事都在阻塞线程里做。载入在 `load.rs`（施工 R-3 下挪出去）。

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::SessionCreated;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::session::{Input, Session};
use miyu_models::provider::chat;
use miyu_policy::preset::{Chosen, MEMORY};
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, abandon};
use miyu_store::usage::{UsageIndex, Who};
use miyu_tool::{Log, Seen};

use crate::TARGET;
use crate::actor::persona::Refresh;
use crate::actor::{self, Actor, JobKit};
use crate::agents::{Agents, Offers, job_in};
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::config::Turning;
use crate::guard::Guard;
use crate::handle::Handle;
use crate::job_ids::JobIds;
use crate::jobs::Roster;
use crate::memory::{self, connect};
use crate::port::ForSession;
use crate::report::{Reporter, Upstream};
use crate::snapshot::{Parts, build};
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;
use crate::usage::Ledger;

mod error;
mod load;
mod setup;

pub use error::{CreateError, LoadError};
pub use load::load;
pub use setup::{Create, Load, PresetPlaces};

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
        persona_texts,
        personas,
        memory_account,
        memory_scope,
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
        memory,
        preset,
        presets,
    } = setup;
    let span = actor::span(&id);
    let config = Turning::start(configs, environment.cwd.clone()).await;
    // 没指定的照这时的 `models.chat`：记进 `session.created`，以后照它（施工 8-8）。
    let reference = model.or_else(|| chat(&config.current().resolved.values()));
    let (resources, name) = (resources.clone(), persona.to_string());
    let shipped = resources.clone();
    // 预设没开记忆的，范围一律 `off`（施工 P-2 中，走查 E2：开不开记忆归预设）。
    let opened = preset
        .as_ref()
        .is_none_or(|chosen| chosen.file.opens(MEMORY));
    let scope = memory::scope(lineage.is_some(), opened, memory_scope);
    // 工具面照这时的配置拼：`subagent` 能选哪几个池（施工 8-8 补）、哪几个人格（施工 P-2 补），以后照快照、载入不重拼。
    // 照预设筛（施工 P-2 中）。
    let offers = Offers::of(&config.current().resolved.values(), personas.ids());
    let face = Agents::face(
        tools,
        &venue,
        lineage.as_ref(),
        &offers,
        attended,
        scope,
        preset.as_ref().map(|chosen| &chosen.file),
    );
    let pin = preset.as_ref().map(Chosen::pin);
    let preset = preset.map(|chosen| chosen.id);
    let asks = Agents::asks(
        &venue,
        lineage.as_ref().map(|lineage| &lineage.parent),
        attended,
    );
    let pools = Agents::pools_in(&face);
    let agents_personas = Agents::personas_in(&face);
    let child = lineage.is_some();
    let count = face.len();
    let dir = root.session_dir(&owner, &id);
    let abandoned = dir.clone();
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (owner_of, id_of) = (owner.clone(), id.clone());
    let (snapshot, policy, texts, run, guard, log, (turns, calls)) = blocking(move || {
        let parts = Parts {
            name: name.clone(),
            texts: persona_texts,
            attended,
            face,
            memory: Some(scope.as_str().to_string()),
            child,
            preset: pin,
        };
        let snapshot = build(&resources, parts).map_err(CreateError::Persona)?;
        let policy = snapshot.policy().map_err(CreateError::Policy)?;
        let texts = snapshot.driver_texts().map_err(CreateError::Policy)?;
        let run = snapshot.run_texts().map_err(CreateError::Policy)?;
        let guard = snapshot.guard_texts().map_err(CreateError::Policy)?;
        store.put(&snapshot.to_bytes()).map_err(CreateError::Disk)?;
        let log = SessionLog::create(&dir, SEGMENT_LIMIT).map_err(CreateError::Disk)?;
        let turns = connect(
            memory.as_ref(),
            scope,
            &memory_account,
            &owner_of,
            &name,
            &id_of,
            &[],
        );
        Ok((snapshot, policy, texts, run, guard, log, turns))
    })
    .await?;
    let kept = blobs.clone();
    let stored = blobs.clone();
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
            preset: preset.clone(),
            personas: agents_personas,
        })
    });
    let created = SessionCreated {
        oneshot,
        cwd: Some(environment.cwd.clone()),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
        depth: lineage.as_ref().map(|lineage| lineage.depth),
        model: reference,
        preset,
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
            turns,
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
            asks,
            memory: calls,
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
    let (resources, blobs) = (shipped, stored);
    actor.watch_persona(Refresh {
        personas,
        resources,
        blobs,
        snapshot,
        child,
        presets,
        tools: tools.clone(),
        venue: venue.clone(),
        lineage: lineage.clone(),
    });
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
        Ok(_) => Ok(Handle::new(id, venue, inbox, busy, oneshot, watched, shown)),
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

/// 用量汇总里的这个会话（施工 8-15）：`session_usage` 的端口照它造。没开汇总的没有。
fn ledger_of(usage: Option<&Arc<UsageIndex>>, id: &SessionId, owner: &AccountId) -> Option<Ledger> {
    usage.map(|index| Ledger {
        index: Arc::clone(index),
        session: id.clone(),
        owner: owner.clone(),
    })
}

#[cfg(test)]
mod tests;
