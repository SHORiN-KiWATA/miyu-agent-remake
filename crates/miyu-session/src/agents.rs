//! 派子代理，执行器这一头（`docs/blueprint/agents.md` 第一条，`session/tools.md`「派子代理」，施工 7-5）：照父会话这一刻的
//! 样子填好一个子会话，经会话表的端口（[`SessionPort`]）造出来，再把交代作为父会话发来的话送进去，开它的第一轮。
//!
//! 一个会话一份 [`Agents`]：属主、场所、第几层、父会话、有没有人能确认，造会话、载入时定。每一次调用照这一轮的工作目录、
//! 加进来的目录、派出去那一刻的权限造一个端口交给工具（[`Agents::for_call`]），编号从会话共用的那一串里领；父子之间留言的
//! 端口也照它造（`crate::messages`，施工 7-7）。

use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Permission;
use miyu_kernel::id::{AccountId, CommandId, JobId, SessionId, VenueId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_policy::{JOB_DEPTH, ToolEntry};
use miyu_tool::{AGENT, AgentPort, Catalog, MESSAGE_AGENT, NotSpawned, Spawned, Spawning};

use crate::TARGET;
use crate::job_ids::JobIds;
use crate::spawn::{Child, Lineage, SessionPort};

/// 子代理的人格：软件工程师（`agents.md` 第一条第 1 条）。挑人格随配置和预设那一步。
const PERSONA: &str = "engineer";

/// 本机这个场所（`protocol.md` 的 `session.create` 第 2 条）：只有它的会话能派子代理。
const LOCAL: &str = "local";

/// 一个会话派子代理要的：造子会话的端口，和子会话照抄的、会话里不变的几样。
pub(crate) struct Agents {
    /// 会话表交进来的端口。
    pub(crate) port: Arc<dyn SessionPort>,
    /// 这个会话：子会话的父。
    pub(crate) session: SessionId,
    /// 属主。
    pub(crate) owner: AccountId,
    /// 场所。
    pub(crate) venue: VenueId,
    /// 这个会话是第几层：主会话是 0。
    pub(crate) depth: u32,
    /// 父会话：子会话才有，留言发给它（施工 7-7）。
    pub(crate) parent: Option<SessionId>,
    /// 有没有人能确认：快照里的。
    pub(crate) attended: bool,
    /// 回报的正文怎么截：快照里的（施工 7-4）。停掉子代理时交回的回报照它截，和子会话自己向上回报的一样。
    pub(crate) reports: miyu_kernel::session::Reports,
}

impl Agents {
    /// 会话能不能派子代理（`agents.md` 第一条第 5、6 条）：在本机，还没到深度上限（[`JOB_DEPTH`]）。场所会话（群）里不能
    /// 派，外部身份只从场所会话进来，也就派不了；到了上限的，子会话再往下就超了。造会话时照它定工具面里有没有 `agent`。
    pub(crate) fn allowed(venue: &VenueId, lineage: Option<&Lineage>) -> bool {
        venue.as_str() == LOCAL && Agents::depth_of(lineage) < JOB_DEPTH
    }

    /// 造会话时定的工具面（施工 7-5、7-7）：目录里每件工具的规格换成快照里的写法。不能派子代理的会话不给 `agent`；场所
    /// 会话（群）不给 `message_agent`：它没有父，也派不了子代理。到了深度上限的子会话照样有 `message_agent`，只能发给父。
    /// 工具面造会话时定，一个会话里不变，给了只会被拒的不给（`agents.md` 第一条第 6 条）。
    pub(crate) fn face(
        tools: &Catalog,
        venue: &VenueId,
        lineage: Option<&Lineage>,
    ) -> Vec<ToolEntry> {
        let spawns = Agents::allowed(venue, lineage);
        let local = venue.as_str() == LOCAL;
        tools
            .specs()
            .filter(|spec| spawns || spec.name != AGENT)
            .filter(|spec| local || spec.name != MESSAGE_AGENT)
            .map(|spec| ToolEntry {
                name: spec.name.clone(),
                description: spec.description.clone(),
                parameters: spec.parameters.clone(),
                access: spec.access.clone(),
            })
            .collect()
    }

    /// 第几层照 `lineage` 算：主会话没有，是 0。
    pub(crate) fn depth_of(lineage: Option<&Lineage>) -> u32 {
        lineage.map_or(0, |lineage| lineage.depth)
    }

    /// 交给一次调用的端口：编号从 `ids` 领，子会话在这一轮的工作目录 `cwd`、加进来的目录 `dirs` 里，权限照派出去那一刻
    /// 实际的 `permission`。
    pub(crate) fn for_call(
        self: &Arc<Agents>,
        ids: Arc<JobIds>,
        cwd: String,
        dirs: Vec<String>,
        permission: Permission,
    ) -> Arc<dyn AgentPort> {
        Arc::new(Spawner {
            agents: Arc::clone(self),
            ids,
            cwd,
            dirs,
            permission,
        })
    }
}

/// 一次调用的派子代理的端口。
struct Spawner {
    agents: Arc<Agents>,
    ids: Arc<JobIds>,
    cwd: String,
    dirs: Vec<String>,
    permission: Permission,
}

impl AgentPort for Spawner {
    /// 标题不交给子会话：它只给头看，记在 `job.started` 里（工具报）。
    fn spawn<'a>(&'a self, _description: &'a str, prompt: &'a str) -> Spawning<'a> {
        Box::pin(async move {
            let job = self.ids.next();
            let agents = &self.agents;
            let parent = &agents.session;
            let child = Child {
                lineage: Lineage {
                    parent: parent.clone(),
                    depth: agents.depth + 1,
                },
                command: command_id(parent, job, ""),
                persona: PERSONA.to_string(),
                owner: agents.owner.clone(),
                venue: agents.venue.clone(),
                permission: self.permission.clone(),
                attended: agents.attended,
                cwd: self.cwd.clone(),
                dirs: self.dirs.clone(),
            };
            let job_text = job.to_string();
            let session = agents.port.create(child).await.map_err(|error| {
                tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent not created");
                NotSpawned
            })?;
            // 交代原样送进去，不加包装：子会话的场所说明已经说了它来自父会话（第一条第 2 条）。
            let by = By::Session(Session { id: parent.clone() });
            let send = Command::Send {
                blocks: vec![Block::Text(Text {
                    text: prompt.to_string(),
                })],
                urgent: false,
            };
            let sent = agents
                .port
                .command(
                    session.clone(),
                    command_id(parent, job, "/prompt"),
                    by,
                    send,
                )
                .await;
            let why = match sent {
                Ok(Outcome::Accepted { .. }) => {
                    tracing::info!(target: TARGET, job = job_text.as_str(), child = session.as_str(), "subagent started");
                    return Ok(Spawned { job, session });
                }
                Ok(Outcome::Rejected { reason }) => {
                    format!("the task was refused: {}", reason.code())
                }
                Err(error) => error,
            };
            tracing::warn!(target: TARGET, job = job_text.as_str(), child = session.as_str(), error = why.as_str(), "subagent not given its task");
            Err(NotSpawned)
        })
    }
}

/// 父会话发给子会话的命令编号：`<父会话>/<任务编号><后缀>`。父会话的编号整个数据根里不重，任务编号一个会话里不重，
/// 所以它在哪儿都不重。
pub(crate) fn command_id(parent: &SessionId, job: JobId, suffix: &str) -> CommandId {
    CommandId::parse(&format!("{parent}/{job}{suffix}"))
        .unwrap_or_else(|e| unreachable!("会话编号、任务编号都短，合命令编号的写法：{e}"))
}

/// 子会话在父会话里的任务编号（施工 7-6）：从造它的命令编号 `<父会话>/<编号>`（`session.created` 的 `cause`）读回来。
/// 不是这个样子的（不是派出来的）没有。会话表删一个子会话时也照它认出父会话里的那个任务（施工 3-8 三补）。
pub fn job_in(parent: &SessionId, command: &CommandId) -> Option<JobId> {
    let job = command
        .as_str()
        .strip_prefix(parent.as_str())?
        .strip_prefix('/')?;
    JobId::parse(job).ok()
}
