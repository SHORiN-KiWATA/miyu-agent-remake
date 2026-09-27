//! 执行一次调用（`05-内核接口.md` 第六节「执行这一步」，施工 4-2）：交给工具的、工具交回的、执行中的
//! 输出推给谁。

use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use miyu_kernel::block::{Block, Text};

/// 一次调用交给工具的：修正过的参数、这一轮的工作目录、系统的家目录、Miyu 的数据根。别的（会话、身份、沙盒范围）
/// 用到时再加。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// 修正过的参数：一个 JSON 对象的原文。
    pub args: String,
    /// 这一轮的工作目录：回合开始时的那一个。
    pub cwd: String,
    /// 系统的家目录（施工 4-4 上）：路径里的 `~` 照它换，和权限策略换的一样，碰到的才是同一个文件。读不出来的是空的。
    pub home: Option<PathBuf>,
    /// Miyu 的数据根（施工 4-4 下）：哪一件工具都不能碰（`11-权限与沙盒.md` A9）。权限策略只核对工具报出的路径，
    /// 往下走目录的工具（`glob`、`grep`）从上面搜下来会走进去，走到这里要自己跳过。不知道的是空的。
    pub data_root: Option<PathBuf>,
}

/// 一次调用要碰的一条路径（施工 4-3 下）：她给的原样，和是读是写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// 她给的路径，原样：相对的照这一轮的工作目录算，`~` 开头的照家目录算。
    pub path: String,
    /// 要写：新建、改、删。不是就是读。
    pub write: bool,
}

/// 一次调用的结局：给模型看的内容块，出没出错。用时由执行器量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    /// 出错了没有：工具执行了，但是出了错。错在哪，写在内容块里给她看。
    pub error: bool,
    /// 给模型看的内容。
    pub blocks: Vec<Block>,
}

impl Done {
    /// 成功，交回一段字。
    pub fn ok(text: impl Into<String>) -> Done {
        Done {
            error: false,
            blocks: vec![Block::Text(Text { text: text.into() })],
        }
    }

    /// 出错，交回一段字：错在哪，写给她看，让她下一步自己改对。
    pub fn error(text: impl Into<String>) -> Done {
        Done {
            error: true,
            blocks: vec![Block::Text(Text { text: text.into() })],
        }
    }
}

/// 执行中的输出推给谁：一段段推给头，不落盘（`03-事件模型.md` 第五节）。
pub struct Progress(Box<dyn Fn(String) + Send + Sync>);

impl Progress {
    /// 每一段交给 `push`。
    pub fn new(push: impl Fn(String) + Send + Sync + 'static) -> Progress {
        Progress(Box::new(push))
    }

    /// 推一段。
    pub fn push(&self, text: impl Into<String>) {
        (self.0)(text.into());
    }
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Progress")
    }
}

/// 跑着的一次调用：交回结局的 future。叫停就是丢掉它，工具手里的东西随之收拾，不另给取消信号。
pub type Running<'a> = Pin<Box<dyn Future<Output = Done> + Send + 'a>>;
