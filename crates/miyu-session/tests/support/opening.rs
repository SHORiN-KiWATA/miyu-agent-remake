//! 造会话时可以换的几样（从 `mod.rs` 挪出来，那边放不下了）：权限、有没有人能确认、工作目录、沙盒（施工 4-3 下），场所、
//! 父会话、记忆的范围、预设、群会话这些线上的（施工 7-5 起）。

use super::*;

/// 造会话时可以换的几样（施工 4-3 下）。
pub struct Opening {
    /// 开始时的权限。
    pub permission: Permission,
    /// 有没有人能确认。
    pub attended: bool,
    /// 工作目录。
    pub cwd: String,
    /// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
    pub dirs: Vec<String>,
    /// 沙盒的助手：有的当这台机器上的沙盒能用（施工 5-4 上）。假工具不起它，随便一条路径就行；真的起命令的用
    /// [`miyu_sandbox::testkit::built_helper`]。
    pub sandbox: Option<PathBuf>,
    /// 沙盒的缓存（施工 5-4 下）：没有的沙盒里不设工具链的变量。
    pub sandbox_cache: Option<SandboxCache>,
}

/// 造会话时另外可以换的几样：派子代理用的三样（施工 7-5），一次性的（施工 7-9）。
pub struct Lines {
    /// 场所：默认在本机。
    pub venue: VenueId,
    /// 父会话和第几层：子会话才有。
    pub lineage: Option<Lineage>,
    /// 造子会话、给别的会话发命令的端口：没有的派不了子代理。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 造会话的命令编号：默认 `cmd-0`；子会话向上回报时照它读回任务编号（`<父会话>/<编号>`，施工 7-6）。
    pub command: Option<CommandId>,
    /// 一次性的（`miyu ask` 开的那种）：没有头订阅着时回报只记下（施工 7-9）。默认不是。
    pub oneshot: bool,
    /// 用哪个模型（施工 8-8）：解析好的引用；默认没有，照这时的 `models.chat`。
    pub model: Option<String>,
    /// 记忆的范围（施工 R-3 下）：默认跟着人格。
    pub memory: MemoryScope,
    /// 记忆归哪个账号（施工 P-1 上）：默认是属主 alice。
    pub memory_account: AccountId,
    /// 预设（施工 P-2 上、中）：默认没有，全开。
    pub preset: Option<miyu_policy::preset::Chosen>,
    /// 核心交不交记忆（施工 R-2 下）：不交的是以前的版本造的会话，回合库里一条都没有。默认交。
    pub indexed: bool,
    /// 群会话（施工 O-13 中）：默认不是。
    pub group: bool,
}

impl Default for Lines {
    /// 本机的主会话，派不了子代理。
    fn default() -> Lines {
        Lines {
            venue: VenueId::parse("local").expect("场所合写法"),
            lineage: None,
            sessions: None,
            command: None,
            oneshot: false,
            model: None,
            memory: MemoryScope::Persona,
            memory_account: alice_account(),
            preset: None,
            indexed: true,
            group: false,
        }
    }
}

impl Default for Opening {
    /// 工作区这一级，有人能确认，工作目录照 [`environment`]，沙盒用不了。
    fn default() -> Opening {
        Opening {
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
            attended: true,
            cwd: environment().cwd,
            dirs: Vec::new(),
            sandbox: None,
            sandbox_cache: None,
        }
    }
}
