//! 造会话、载入要交进来的几样（施工 8-8 从 `open.rs` 挪来：那一页长过了行数的上限）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_policy::PersonaTexts;
use miyu_policy::features::Features;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::Chosen;
use miyu_store::index::SessionIndex;
use miyu_store::personas::Personas;
use miyu_store::presets::Presets;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_store::usage::UsageIndex;
use miyu_tool::Shelf;

use crate::config::Configs;
use crate::jobs::Jobs;
use crate::port::Models;
use crate::sandbox::SandboxCache;
use crate::spawn::{Lineage, SessionPort};

/// 预设的几层和这台机器上装了的功能（施工 P-2 下；施工 F-3 上起是功能）：回合开始时照它看预设的文件改了没有。
#[derive(Debug, Clone)]
pub struct PresetPlaces {
    /// 几层。
    pub presets: Presets,
    /// 装了的功能（设计 `30-插件框架.md` 第三节）。
    pub features: Features,
}

/// 造一个会话要的。
pub struct Create<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 资源目录：随核心附带的字从这里读。
    pub resources: &'a ResourceRoot,
    /// 会话编号，照 [`crate::new_id`] 造。
    pub id: SessionId,
    /// 照哪个人格造：编号。无人格的没有（施工 P-4 上）：`persona_texts` 是空的，记忆不生效。
    pub persona: Option<&'a str>,
    /// 这个人格的字，几层叠好的（施工 P-1 上，`miyu_store::personas`）：造快照用。
    pub persona_texts: PersonaTexts,
    /// 人格的几层（施工 P-1 再补）：回合开始时照它看人格的文件改了没有。
    pub personas: Personas,
    /// 记忆归哪个账号（施工 P-1 上，`Personas::memory_account`）：回合库、记忆日志照它和人格开。
    pub memory_account: AccountId,
    /// 记忆的范围（施工 R-3 下，`memory.md`「范围」）：记进快照，以后照它。子会话不管交的是什么，一律 `off`。
    pub memory_scope: MemoryScope,
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
    /// 工具目录的架子：照现在的那一份存下这个会话的工具面（施工 4-1），以后照快照发；提供者的包换了，下一个回合换上
    /// （施工 O-2 中）。执行工具时照现在的那一份找。
    pub tools: &'a Shelf,
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
    /// 属主的会话列表的索引（施工 3-8 七补）：日志每落一批，顺手更新这个会话的那一行。没有的（测试里自己造的）不更新。
    pub index: Option<Arc<SessionIndex>>,
    /// 用量汇总（施工 8-15）：日志每落一批，顺手写这一批发出去了的请求；`session_usage` 照它查。没有的（测试里自己造的）
    /// 不写，`session_usage` 照什么都没花答。
    pub usage: Option<Arc<UsageIndex>>,
    /// 从哪取配置（施工 8-4）：回合开始时照它冻结这一轮的配置。没有配置服务的（测试里）给 [`crate::fixed`] 的一份。
    pub configs: Configs,
    /// 核心一份的记忆（施工 R-2 上、R-3 中，`memory.md`）：主会话每落一批，把结束了的人开的回合放进这个人格的回合库；三件
    /// 工具的端口照它造。没有的（测试里自己造的）不放、三件工具说记忆没开。
    pub memory: Option<Arc<crate::Memory>>,
    /// 会话用哪个模型（施工 8-8）：已经查过的引用，模型或 `@池`（协议的 `session.create` 的 `model`、派子代理时照 `pool`
    /// 或父会话的）。没有的照这时的 `models.chat`。记进 `session.created` 的 `model`。
    pub model: Option<String>,
    /// 会话用哪个预设（施工 P-2 上、中）：已经找好的，编号记进 `session.created` 的 `preset`；子会话照父会话的。工具面照它
    /// 筛，记忆没开的范围一律 `off`，记进快照（[`miyu_policy::PresetPin`]）。测试里自己造的可以没有：全开。
    pub preset: Option<Chosen>,
    /// 预设的几层和装了的软件（施工 P-2 下）：改了预设的文件下一个回合换上。没有的（测试里自己造的）不看。
    pub presets: Option<PresetPlaces>,
    /// 是不是群会话（施工 O-13 中）：`venue.session` 的 `kind` 是 `group` 的。是的 system 接上群聊的格式说明，快照钉下这时的
    /// 时区，群里的人说的渲染成一行一条。
    pub group: bool,
    /// 属主是不是管理员（施工 5-12）：场所会话、属主不是管理员的（群会话、陌生人和成员的私聊）只碰得到自己的工作区
    /// （`11-权限与沙盒.md` 第三节「外部身份」）。
    pub owner_is_admin: bool,
}

/// 照最后一次记下的工作目录（没有的是空的）定实际在哪干活。
pub type Pick<'a> = Box<dyn FnOnce(Option<&str>) -> String + Send + 'a>;

/// 载入的会话在哪干活（施工 V-2 三补）。
pub enum Workplace<'a> {
    /// 给定的：时区、工作目录、加进来的目录。
    Given(Environment),
    /// 照日志里最后一次记下的：载入读日志时顺手认出最后一次记下的工作目录（都没记下的是空的）、加进来的目录，交给 `pick`
    /// 定实际在哪干活（太宽的、落在数据根里的怎么退是会话表的事）。原来会话表在载入之前另读一遍整份日志认它。
    Remembered {
        /// 时区。
        offset: UtcOffset,
        /// 照最后一次记下的工作目录定实际在哪干活。
        pick: Pick<'a>,
    },
}

/// 载入一个会话要的。
pub struct Load<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 会话的属主。
    pub owner: AccountId,
    /// 人格的几层（施工 P-1 上）：读出快照里的人格以后，照 [`Personas::memory_account`] 算记忆归哪个账号；回合开始时照它
    /// 看人格的文件改了没有（施工 P-1 再补）。
    pub personas: Personas,
    /// 资源目录（施工 P-1 再补）：人格的文件改了，照它重拼快照。
    pub resources: &'a ResourceRoot,
    /// 会话编号。
    pub id: SessionId,
    /// 会话在哪干活（施工 V-2 三补）：给定的，或者照日志里最后一次记下的挑。
    pub place: Workplace<'a>,
    /// 给会话造请求模型的端口：驱动的占位取自这个会话的策略快照。
    pub models: &'a dyn Models,
    /// 工具目录的架子：执行工具时照名字在现在的那一份里找（施工 4-2）。工具面照快照；提供者的包换了，下一个回合换上
    /// （施工 O-2 中）。
    pub tools: &'a Shelf,
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
    /// 同 [`Create::index`]。
    pub index: Option<Arc<SessionIndex>>,
    /// 同 [`Create::usage`]。
    pub usage: Option<Arc<UsageIndex>>,
    /// 同 [`Create::configs`]。
    pub configs: Configs,
    /// 同 [`Create::memory`]：载入时照整份事件补上回合库落下的。
    pub memory: Option<Arc<crate::Memory>>,
    /// 同 [`Create::presets`]。
    pub presets: Option<PresetPlaces>,
    /// 同 [`Create::owner_is_admin`]。
    pub owner_is_admin: bool,
}
