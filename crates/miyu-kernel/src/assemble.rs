//! 组装请求的接口：给一段有效历史，出一份统一的请求（`docs/designs/05-内核接口.md`
//! 第五节「组装请求」，`08-上下文投影.md` 第四节「默认的组装怎么写」）。
//!
//! 这是一个独占的挂接点：一个会话只用一种组装，按策略选定。内核只定这个接口，默认的做法
//! 在第 2 层的 `miyu-assemble` 里，换一种组装不用改内核。

use crate::block::Block;
use crate::history::History;
use crate::id::Seq;
use crate::request::Request;

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
}
