//! 组装请求的接口：给一段有效历史，出一份统一的请求（`docs/designs/05-内核接口.md`
//! 第五节「组装请求」，`08-上下文投影.md` 第四节「默认的组装怎么写」）。
//!
//! 这是一个独占的挂接点：一个会话只用一种组装，按策略选定。内核只定这个接口，默认的做法
//! 在第 2 层的 `miyu-assemble` 里，换一种组装不用改内核。

use crate::block::{Block, Image};
use crate::history::History;
use crate::id::{Seq, TurnId};
use crate::request::Request;
use crate::time::Timestamp;

/// 一段话（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：人这边说的一句，或者她一轮最后一条有正文的回答，照组装器的
/// 写法渲染成字。抽取的那一段照它取，取法和回顾的请求同一份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spoken {
    /// 日志里第几条。
    pub seq: Seq,
    /// 哪一轮；不在回合里的没有。
    pub turn: Option<TurnId>,
    /// 什么时候。
    pub at: Timestamp,
    /// 是不是她的回答。
    pub assistant: bool,
    /// 字，去掉了前后空白，不是空的。
    pub text: String,
}

/// 组装请求：给一段有效历史，出一份统一的请求。
///
/// 必须是同步的纯函数：同一个组装器，同样的有效历史，出来的请求字节一定一样（内核不变量 3）。
/// 冻结在会话上的策略，例如工具面、system、出厂的英文，在造组装器的时候交进来，
/// 一个会话一个（内核 K3），所以这里只收有效历史。
pub trait Assembler {
    /// 从有效历史组装出发给模型的请求。
    fn assemble(&self, history: &History) -> Request;

    /// 压缩的摘要请求：有效历史到第 `upto` 条为止的投影，最后是摘要指令（`compaction.md` 第三条第 3 条）。
    /// 同样是纯函数。`cut` 是截短重试截到第几条（施工 6-6 中，第三条第 10 条）：检查点后面第 `cut` 条及以前的不要，
    /// 截过的要照组装器的写法标出来；没有是不截。`instructions` 是手动压缩时人附的要求，接进摘要指令（第七条第 3 条，
    /// 施工 6-8）；`None` 是没附。
    fn summarize(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request;

    /// 隔离式的摘要请求（施工 6-6 下，`compaction.md` 第四条）：和 [`Assembler::summarize`] 一样的消息，system 换成隔离式
    /// 那一句，工具面空的。fork 式的摘要回复里调了工具时改发它。手动压缩附的要求照样接进指令（施工 6-8）。
    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request;

    /// 从摘要请求的回复里取出摘要；取不出来的（空的）是 `None`（`compaction.md` 第三条第 6 条）。指令和取法是
    /// 一对，所以都归组装。
    fn summary(&self, reply: &[Block]) -> Option<String>;

    /// 回顾的请求（施工 3-8 四补，`docs/blueprint/kernel/request.md`「回顾的请求」）：单独的一次辅助请求，不接主对话的前缀，
    /// 只喂 `history` 里最近几轮人说的话和她的回答正文。交回请求，和它照到的那一条：喂进去的最新那一条消息的序号。
    /// `history` 是这一刻落了盘的有效历史。同样是纯函数。
    ///
    /// 没有能回顾的（她一个带正文的回复都没有）是 `None`。默认的是 `None`：不做回顾的组装。
    fn recap(&self, history: &History) -> Option<(Request, Seq)> {
        let _ = history;
        None
    }

    /// 起标题的请求（施工 3-8 五补，`docs/blueprint/kernel/request.md`「起标题的请求」）：和回顾一样单独的一次辅助请求，只喂
    /// `history` 里的第一轮：第一个回答以前人这边的话，和那个回答的正文。交回请求，和它照到的那一条：那个回答的序号。
    /// `history` 是这一刻落了盘的有效历史。同样是纯函数。
    ///
    /// 她一个带正文的回复都没有的是 `None`。默认的是 `None`：不做起标题的组装。
    fn title(&self, history: &History) -> Option<(Request, Seq)> {
        let _ = history;
        None
    }

    /// `history` 里第 `after` 条以后的几段话（施工 R-6 上）：人这边的话照主请求里的写法渲染，她每一轮只取最后一条有正文的
    /// 回答，只要字；照投影的先后，撤掉的回合不算。`history` 可以是留着一切的那一份（[`History::whole`]）：压缩替代掉的也在。
    /// 同样是纯函数。默认的是空的：不做这一种渲染。
    fn spoken(&self, history: &History, after: Seq) -> Vec<Spoken> {
        let _ = (history, after);
        Vec::new()
    }

    /// 转述一张图的请求（施工 8-17，`docs/blueprint/kernel/request.md`「替它看的图」）：主对话的模型看不了图，内核把 `image`
    /// 交给 `models.vision` 转成字之前组装。`said` 是人这一轮最近说的那一句，内核找好交进来，没有的是 `None`。同样是纯函数。
    ///
    /// 快照里没有转述的字的是 `None`：不转述，照旧写占位。默认的是 `None`。
    fn describe(&self, image: &Image, said: Option<&str>) -> Option<Request> {
        let _ = (image, said);
        None
    }
}
