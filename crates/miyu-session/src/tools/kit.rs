//! 执行工具要的那一份（施工 4-4 上起；从 `tools.rs` 挪出来，施工 O-2 三补）：开会话、载入会话时拼好，交给 [`super::Tools::new`]。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_store::blob::Blobs;
use miyu_tool::{Log, Seen, Shelf};

use crate::agents::Agents;
use crate::job_ids::JobIds;
use crate::lettering::Lettering;
use crate::sandbox::SandboxCache;
use crate::usage::Ledger;

/// 执行工具要的：工具目录、替工具写的两句、系统的家目录（施工 4-4 上，交给每次调用）。
pub(crate) struct ToolKit {
    /// 这个会话（施工 O-2 上）：每次调用带上，提供者的工具照它发 `tool.call`。
    pub(crate) session: SessionId,
    /// 会话的属主（施工 O-2 三补）：每次调用带上，提供者的工具交回的图照它拷进属主名下。
    pub(crate) owner: AccountId,
    /// 外部身份的会话（施工 5-12 下）：命令的沙盒读写都只限工作区。
    pub(crate) confined: bool,
    /// 工具目录的架子：执行时照现在的那一份找（施工 O-2 中）。
    pub(crate) catalog: Shelf,
    /// 替工具写的两句，和权限策略同一份（施工 P-1 三补：换快照时跟着换）。
    pub(crate) lettering: Arc<Lettering>,
    /// 系统的家目录。
    pub(crate) home: Option<PathBuf>,
    /// Miyu 的数据根：交给工具，往下走目录的走到这里跳过（施工 4-4 下）。
    pub(crate) data_root: PathBuf,
    /// 这个会话的 blob：效果里改前改后的内容存进这里（施工 4-6 上）。
    pub(crate) blobs: Blobs,
    /// 她看过的文件：新会话是空的，载入的从日志里重建（施工 4-6 上）。
    pub(crate) seen: Seen,
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。
    pub(crate) sandbox: Option<PathBuf>,
    /// 沙盒的缓存：工具链的缓存用沙盒自己的一份（施工 5-4 下）。核心算不出缓存目录的没有。
    pub(crate) sandbox_cache: Option<SandboxCache>,
    /// 这个会话日志的只读入口：交给每次调用，`history` 用（施工 6-4）。
    pub(crate) log: Log,
    /// 会话的时区：开会话时的环境里的（施工 6-4）。
    pub(crate) offset: UtcOffset,
    /// 这个会话的任务编号（施工 7-5）：从日志里用过的最大编号往下数，几次调用一起跑的各领各的。
    pub(crate) job_ids: Arc<JobIds>,
    /// 派子代理要的（施工 7-5）：会话表交进来了端口才有。
    pub(crate) agents: Option<Arc<Agents>>,
    /// 用量汇总里的这个会话（施工 8-15）：`session_usage` 的端口照它造。没开汇总的没有。
    pub(crate) ledger: Option<Ledger>,
    /// 能不能问人（施工 D-2，[`Agents::asks`]）：能的每次调用给一个提问的端口。
    pub(crate) asks: bool,
    /// 记忆（施工 R-3 中）：主会话、核心交了记忆的才有，每次调用照它造记忆的端口。
    pub(crate) memory: Option<crate::memory::Calls>,
}
