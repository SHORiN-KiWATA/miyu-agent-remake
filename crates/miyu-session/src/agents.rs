//! 派子代理，执行器这一头（`docs/blueprint/agents.md` 第一条，`session/tools.md`「派子代理」，施工 7-5）：照父会话这一刻的
//! 样子填好一个子会话，经会话表的端口（[`SessionPort`]）造出来，再把交代作为父会话发来的话送进去，开它的第一轮。
//!
//! 一个会话一份 [`Agents`]：属主、场所、第几层、父会话、有没有人能确认，造会话、载入时定。每一次调用照这一轮的工作目录、
//! 加进来的目录、派出去那一刻的权限造一个端口交给工具（[`Agents::for_call`]），编号从会话共用的那一串里领；父子之间留言的
//! 端口也照它造（`crate::messages`，施工 7-7）。
//!
//! 子会话用哪个模型（施工 8-8，`models.md`「怎么走」第三条第 4 条）：她写了池的（施工 8-8 补，只能是这个会话列着的），子会话
//! 记 `@<池>`；没写的，用父会话这时生效的引用（[`Inherit`]）。记进子会话 `session.created` 的 `model`。
//!
//! 列着哪几个池（施工 8-8 补，`models.md`「工具」）：造会话时照那时的配置拼进工具面上 `subagent` 的 `pool`（[`Agents::face`]），
//! 造会话、载入时再照快照读回（[`Agents::pools_in`]），存在 [`Agents::pools`]，整个会话不变。

use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Permission;
use miyu_kernel::id::{AccountId, CommandId, JobId, SessionId, VenueId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::PresetFile;
use miyu_policy::{Choice, JOB_DEPTH, ToolEntry};
use miyu_tool::{
    ASK_USER, AgentPort, BASESYSTEM, Catalog, FORGET, MEMORY_SEARCH, NotSpawned, Order, REMEMBER,
    SEND_MESSAGE, SESSION_USAGE, SESSIONS, SUBAGENT, Spawned, Spawning, TODOWRITE, is_subagent,
};

use crate::TARGET;
use crate::job_ids::JobIds;
use crate::spawn::{Child, Lineage, SessionPort};

/// 本机这个场所（`protocol.md` 的 `session.create` 第 2 条）：只有它的会话能派子代理；回合索引也只收它的（施工 R-2 再补，
/// `memory.rs`）。
pub(crate) const LOCAL: &str = "local";

/// 会话在哪：场所，和是不是群会话（施工 O-2 上：提供者的工具照它挑给私聊还是给群）。
#[derive(Clone, Copy)]
pub(crate) struct Site<'a> {
    pub(crate) venue: &'a VenueId,
    pub(crate) group: bool,
}

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
    /// 派子代理能选的池（施工 8-8 补）：快照里 `subagent` 的 `pool` 的 `enum`（[`Agents::pools_in`]），没有的是空的。
    pub(crate) pools: Vec<String>,
    /// 这个会话的预设（施工 P-2 上）：`session.created` 的，子会话没选的照抄（C5 的默认）。以前的会话没有。
    pub(crate) preset: Option<String>,
    /// 派子代理能选的人格（施工 P-2 补）：同 `pools`，快照里 `subagent` 的 `persona` 的 `enum`（[`Agents::personas_in`]）。
    pub(crate) personas: Vec<String>,
}

/// 工具面上 `subagent` 那两个参数能选的（施工 8-8 补、P-2 补）：池照这时的配置，人格照这台机器上这时有的。
pub(crate) struct Offers {
    pub(crate) pools: Vec<Choice>,
    pub(crate) personas: Vec<Choice>,
}

impl Offers {
    /// 照这时的配置 `values`、人格的编号 `personas`（照编号排）。人格只填名字，不带说明：说明是人格的作者写的，不一定是英文。
    pub(crate) fn of(values: &Values, personas: Vec<String>) -> Offers {
        Offers {
            pools: choices(values),
            personas: personas
                .into_iter()
                .map(|name| Choice {
                    name,
                    description: None,
                })
                .collect(),
        }
    }
}

impl Agents {
    /// 会话能不能派子代理（`agents.md` 第一条第 5、6 条）：在本机，还没到深度上限（[`JOB_DEPTH`]）。场所会话（群）里不能
    /// 派，外部身份只从场所会话进来，也就派不了；到了上限的，子会话再往下就超了。造会话时照它定工具面里有没有 `subagent`。
    pub(crate) fn allowed(venue: &VenueId, lineage: Option<&Lineage>) -> bool {
        venue.as_str() == LOCAL && Agents::depth_of(lineage) < JOB_DEPTH
    }

    /// 会话能不能列别的会话（施工 C-3，`cross-session.md` 第九条）：本机的主会话才能。子会话的事经它的父会话，群里的人不可信，
    /// 不能经她看人的会话。造会话时照它定工具面里有没有 `sessions`，每次调用照它给不给列会话的端口。
    pub(crate) fn lists_sessions(venue: &VenueId, parent: Option<&SessionId>) -> bool {
        venue.as_str() == LOCAL && parent.is_none()
    }

    /// 会话能不能问人（施工 D-2）：有人能回答（`attended`）的本机主会话。子会话问父会话，`miyu ask`、群里没有提问的界面。
    /// 造会话时照它定工具面里有没有 `ask_user`，执行器照它给不给提问的端口。
    pub(crate) fn asks(venue: &VenueId, parent: Option<&SessionId>, attended: bool) -> bool {
        attended && Agents::lists_sessions(venue, parent)
    }

    /// 造会话时定的工具面（施工 7-5、7-7、C-3、C-5）：目录里每件工具的规格换成快照里的写法。不能派子代理的会话不给
    /// `subagent`；场所会话（群）不给 `send_message`：它没有父，也派不了子代理，给了只会被 `not-here.txt` 拒。到了
    /// 深度上限的子会话照样有 `send_message`，能发给父、能发给别的会话，只是不能派子代理。只有本机的主会话有 `sessions`
    /// （[`Agents::lists_sessions`]）。`session_usage` 只给本机的会话（施工 8-15）：群里的人不可信，花了多少钱是属主的事。
    /// 工具面造会话时定，一个会话里不变，给了只会被拒的不给（`agents.md` 第一条第 6 条）。
    /// `subagent` 的 `pool` 照这时的配置 `values` 填上能选的池（施工 8-8 补：`miyu_models::pools::offered`，一个都没有的拿掉
    /// 这个参数）；`persona` 照 `offers` 填上这台机器上有的人格（施工 P-2 补）。`ask_user` 只给能问人的会话（[`Agents::asks`]，施工 D-2）。`todowrite` 只给本机的会话（施工 D-3）：群里没人
    /// 看她的清单。有预设的照它筛（施工 P-2 中）：工具所在的包没开的、单件关掉的不给（[`PresetFile::keeps`]）；目录里没记包的
    /// 当基础系统。
    pub(crate) fn face(
        tools: &Catalog,
        site: Site<'_>,
        lineage: Option<&Lineage>,
        offers: &Offers,
        attended: bool,
        memory: MemoryScope,
        preset: Option<&PresetFile>,
    ) -> Vec<ToolEntry> {
        let venue = site.venue;
        let spawns = Agents::allowed(venue, lineage);
        let local = venue.as_str() == LOCAL;
        let asks = Agents::asks(venue, lineage.map(|lineage| &lineage.parent), attended);
        let lists = Agents::lists_sessions(venue, lineage.map(|lineage| &lineage.parent));
        tools
            .specs()
            .filter(|spec| spawns || spec.name != SUBAGENT)
            .filter(|spec| local || spec.name != SEND_MESSAGE)
            .filter(|spec| lists || spec.name != SESSIONS)
            .filter(|spec| local || spec.name != SESSION_USAGE)
            .filter(|spec| asks || spec.name != ASK_USER)
            .filter(|spec| local || spec.name != TODOWRITE)
            // 记忆的三件只给本机、记忆开着的会话（施工 R-3 中、下，`memory.md`「工具」）：子会话的范围一律 `off`（17 第二节），
            // 群随 O 线。
            .filter(|spec| {
                (local && memory != MemoryScope::Off)
                    || ![REMEMBER, FORGET, MEMORY_SEARCH].contains(&spec.name.as_str())
            })
            // 提供者的工具照它给哪种会话挑（施工 O-2 上）。
            .filter(|spec| {
                tools
                    .get(&spec.name)
                    .and_then(|tool| tool.venues())
                    .is_none_or(|venues| {
                        if local {
                            venues.local
                        } else if site.group {
                            venues.group
                        } else {
                            venues.private
                        }
                    })
            })
            .filter(|spec| {
                preset.is_none_or(|preset| {
                    let package = tools.package_of(&spec.name).unwrap_or(BASESYSTEM);
                    preset.keeps(package, &spec.name)
                })
            })
            .map(|spec| {
                let mut entry = ToolEntry {
                    name: spec.name.clone(),
                    description: spec.description.clone(),
                    parameters: spec.parameters.clone(),
                    access: spec.access.clone(),
                };
                // 以前的名字也填：只在测试里拿改名以前的目录造会话时碰得到，照同一个规矩，两张工具面只差名字。
                if is_subagent(&spec.name) {
                    entry.offer(POOL, &offers.pools);
                    entry.offer(PERSONA_PARAMETER, &offers.personas);
                }
                entry
            })
            .collect()
    }

    /// 快照的工具面 `face` 上派子代理能选的池（施工 8-8 补）：`subagent`（以前的名字也算）的 `pool` 的 `enum`。没有这件、
    /// 没有这一格的（以前造的会话、不能派的会话）是空的。
    pub(crate) fn pools_in(face: &[ToolEntry]) -> Vec<String> {
        face.iter()
            .find(|entry| is_subagent(&entry.name))
            .map(|entry| entry.offered(POOL))
            .unwrap_or_default()
    }

    /// 快照的工具面 `face` 上派子代理能选的人格（施工 P-2 补）：同 [`Agents::pools_in`]，`persona` 的 `enum`。
    pub(crate) fn personas_in(face: &[ToolEntry]) -> Vec<String> {
        face.iter()
            .find(|entry| is_subagent(&entry.name))
            .map(|entry| entry.offered(PERSONA_PARAMETER))
            .unwrap_or_default()
    }

    /// 第几层照 `lineage` 算：主会话没有，是 0。
    pub(crate) fn depth_of(lineage: Option<&Lineage>) -> u32 {
        lineage.map_or(0, |lineage| lineage.depth)
    }

    /// 交给一次调用的端口：编号从 `ids` 领，子会话在这一轮的工作目录 `cwd`、加进来的目录 `dirs` 里，权限照派出去那一刻
    /// 实际的 `permission`，模型照 `inherit`（施工 8-8）。
    pub(crate) fn for_call(
        self: &Arc<Agents>,
        ids: Arc<JobIds>,
        (cwd, dirs): (String, Vec<String>),
        permission: Permission,
        inherit: Inherit,
    ) -> Arc<dyn AgentPort> {
        Arc::new(Spawner {
            agents: Arc::clone(self),
            ids,
            cwd,
            dirs,
            permission,
            inherit,
        })
    }
}

/// 子会话用哪个模型要的（施工 8-8）：父会话这时生效的引用（她没写池时子会话记下它）。
#[derive(Debug)]
pub(crate) struct Inherit {
    /// 父会话这时生效的引用：模型或 `@池`；没有的是空的（子会话照它那时的 `models.chat`）。
    pub(crate) reference: Option<String>,
}

impl Inherit {
    /// 照会话请求模型的端口 `model`（它这时生效的引用）抄一份。
    pub(crate) fn of(model: &dyn crate::port::ModelPort) -> Inherit {
        Inherit {
            reference: model.reference(),
        }
    }

    /// 子会话记下的引用：写了池 `pool` 的是 `@<池>`（施工 8-8 补：这时还解析不解析得出不查，解析不出的子会话照它的路由退回
    /// `models.chat`），没写的是父会话的。
    fn model(&self, pool: Option<&str>) -> Option<String> {
        match pool {
            Some(pool) => Some(format!("@{pool}")),
            None => self.reference.clone(),
        }
    }
}

/// `subagent` 上填池的那个参数（施工 8-8 补）。
const POOL: &str = "pool";

/// `subagent` 上填人格的那个参数（施工 P-2 补）。
const PERSONA_PARAMETER: &str = "persona";

/// 照这时的配置 `values`，派子代理能选的池写成工具面要的样子。
fn choices(values: &Values) -> Vec<Choice> {
    miyu_models::pools::offered(values)
        .into_iter()
        .map(|offer| Choice {
            name: offer.name,
            description: offer.description,
        })
        .collect()
}

/// 一次调用的派子代理的端口。
struct Spawner {
    agents: Arc<Agents>,
    ids: Arc<JobIds>,
    cwd: String,
    dirs: Vec<String>,
    permission: Permission,
    inherit: Inherit,
}

impl AgentPort for Spawner {
    /// 标题不交给子会话：它只给头看，记在 `job.started` 里（工具报）。人格没挑的是无人格（施工 P-4 上：出厂不带人格，原来是
    /// 软件工程师，走查 C5），预设照父会话的。
    fn spawn<'a>(&'a self, order: Order<'a>) -> Spawning<'a> {
        let Order {
            prompt,
            pool,
            persona,
            ..
        } = order;
        Box::pin(async move {
            let job = self.ids.next();
            let agents = &self.agents;
            let parent = &agents.session;
            let child = Child {
                lineage: Lineage {
                    parent: parent.clone(),
                    depth: agents.depth + 1,
                },
                command: command_id(parent, &job, ""),
                persona: persona.map(str::to_string),
                owner: agents.owner.clone(),
                venue: agents.venue.clone(),
                permission: self.permission.clone(),
                attended: agents.attended,
                cwd: self.cwd.clone(),
                dirs: self.dirs.clone(),
                model: self.inherit.model(pool),
                preset: agents.preset.clone(),
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
                venue: None,
            };
            let sent = agents
                .port
                .command(
                    session.clone(),
                    command_id(parent, &job, "/prompt"),
                    by,
                    send,
                )
                .await;
            let why = match sent {
                Ok(Outcome::Accepted { .. } | Outcome::Recapped { .. }) => {
                    tracing::info!(target: TARGET, job = job_text.as_str(), child = session.as_str(), "subagent started");
                    return Ok(Spawned { job, session });
                }
                Ok(Outcome::Rejected { reason, .. }) => {
                    format!("the task was refused: {}", reason.code())
                }
                Err(error) => error,
            };
            tracing::warn!(target: TARGET, job = job_text.as_str(), child = session.as_str(), error = why.as_str(), "subagent not given its task");
            Err(NotSpawned)
        })
    }

    fn pools(&self) -> &[String] {
        &self.agents.pools
    }

    fn personas(&self) -> &[String] {
        &self.agents.personas
    }
}

/// 父会话发给子会话的命令编号：`<父会话>/<任务编号><后缀>`。父会话的编号整个数据根里不重，任务编号一个会话里不重，
/// 所以它在哪儿都不重。任务编号带着前缀（施工 7-1 补）也还短：段数有深度上限管着（出厂的上限下，子代理的编号至多两段），
/// 每一段是派过的第几个，离命令编号的 128 个字节远着。
pub(crate) fn command_id(parent: &SessionId, job: &JobId, suffix: &str) -> CommandId {
    CommandId::parse(&format!("{parent}/{job}{suffix}"))
        .unwrap_or_else(|e| unreachable!("会话编号、任务编号都短，合命令编号的写法：{e}"))
}

/// 子会话在父会话里的任务编号（施工 7-6）：从造它的命令编号 `<父会话>/<编号>`（`session.created` 的 `cause`）读回来。
/// 不是这个样子的（不是派出来的）没有。向上回报照它补上任务编号（施工 7-6），会话表删一个子会话时也照它认出父会话里的
/// 那个任务（施工 3-8 三补），它领的号照它带前缀（施工 7-1 补，`JobIds`）。
pub fn job_in(parent: &SessionId, command: &CommandId) -> Option<JobId> {
    let job = command
        .as_str()
        .strip_prefix(parent.as_str())?
        .strip_prefix('/')?;
    JobId::parse(job).ok()
}

#[cfg(test)]
mod tests;
