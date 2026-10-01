//! 默认的请求组装：从有效历史出一份统一的请求（`docs/designs/08-上下文投影.md` 第四节
//! 「默认的组装怎么写」）。
//!
//! 第 2 层的纯逻辑，实现内核的 [`Assembler`] 接口。请求照这个先后排：
//!
//! 1. 稳定区：工具面（照名字排好）、system、示范对话；
//! 2. 检查点：最近一次压缩的摘要，套上包装；
//! 3. 历史：照有效历史排好的先后，每种事件渲染成对应的消息或内容块，任务的两种回报是带标签的事实（`jobs.rs`），别的
//!    harness 发来的话包一层带名字的标签（`harness.rs`），别的会话发来的话包一层带短编号的标签（`peers.rs`）；
//! 4. 人这一边挨着的块合成一条 user 消息：检查点最前，事实其次，人的消息最后。
//!
//! 压缩的摘要请求也在这里组装：截到第 N 条照平常组装，最后接摘要指令（`summary.rs`）。回顾的请求也是（`recap.rs`，施工 3-8
//! 四补），它不接稳定区，只喂最近几轮的对话正文；起标题的请求照它的写法只喂第一轮（`title.rs`，施工 3-8 五补）。
//!
//! 冻结在会话上的东西，也就是稳定区和给模型看的几句固定的字，在造组装器的时候交进来，
//! 一个会话一个（内核 K3）。这里不读文件：出厂的字由执行器从资源目录读好交进来。

mod harness;
mod jobs;
mod peers;
mod recap;
mod render;
mod summary;
mod tag;
mod texts;
mod title;

#[cfg(test)]
mod test_support;

pub use texts::{
    HarnessTexts, IdleTexts, JobTexts, PeerTexts, Recap, RestoredWrap, Texts, Title, TurnEndedTexts,
};

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::Block;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request, ToolSpec};

/// 稳定区：每次请求都一样、排在最前面的部分（`08-上下文投影.md` 第三节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stable {
    /// 工具面。造组装器时照名字排好，交进来时的先后不算数。
    pub tools: Vec<ToolSpec>,
    /// 系统提示词，已经照 `26-提示词.md` 第四节的顺序拼好（施工 3-6）。组装时不再拆开。
    pub system: String,
    /// 示范对话，排在 system 之后、历史之前。
    pub demos: Vec<Message>,
}

/// 默认的组装器。一个会话一个，造好以后不再变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultAssembler {
    /// 稳定区，工具面已经照名字排好。
    stable: Stable,
    /// 检查点的包装、回合没走完的那几句。
    texts: Texts,
}

impl DefaultAssembler {
    /// 用这个会话冻结的稳定区和固定的字造一个组装器。工具面在这里照名字排好：
    /// 名字的字节序，同名的保持交进来的先后。
    pub fn new(mut stable: Stable, texts: Texts) -> DefaultAssembler {
        stable.tools.sort_by(|a, b| a.name.cmp(&b.name));
        DefaultAssembler { stable, texts }
    }
}

impl Assembler for DefaultAssembler {
    fn assemble(&self, history: &History) -> Request {
        let mut messages = self.stable.demos.clone();
        messages.extend(render::render(history, &self.texts));
        Request {
            tools: self.stable.tools.clone(),
            system: self.stable.system.clone(),
            messages,
            stable: self.stable.demos.len(),
            continuation: render::continues(history),
        }
    }

    /// 有效历史截到第 `upto` 条，照平常组装，摘要指令接在最后；最后是人这边的指令，不接着写（`summary.rs`）。截短重试
    /// 的（施工 6-6 中）：检查点后面第 `cut` 条及以前的不要，留下的第一条是助手的，前面补一条 user（`truncated.txt`）。
    /// 手动压缩附了要求的，要求夹在指令里（`Texts::instruction`，施工 6-8）。
    fn summarize(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        let kept = history.until(upto);
        let kept = match cut {
            Some(cut) => kept.after(cut),
            None => kept,
        };
        let mut request = self.assemble(&kept);
        if cut.is_some() {
            summary::mark_truncated(&mut request.messages, request.stable, &self.texts.truncated);
        }
        summary::instruct(&mut request.messages, &self.texts.instruction(instructions));
        request.continuation = false;
        request
    }

    /// 隔离式（施工 6-6 下）：和 fork 式一样的消息，system 换成那一句，工具面空的；稳定区的示范对话照留在消息里。
    /// 手动压缩附的要求照 fork 式的接（施工 6-8）。
    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        let mut request = self.summarize(history, upto, cut, instructions);
        request.system = self.texts.summarize_system.clone();
        request.tools = Vec::new();
        request
    }

    fn summary(&self, reply: &[Block]) -> Option<String> {
        summary::extract(reply)
    }

    /// 回顾的请求（施工 3-8 四补，`recap.rs`）：不接稳定区，一条 user，指令接对话记录。快照里没有回顾的字的，没有。
    fn recap(&self, history: &History) -> Option<(Request, Seq)> {
        recap::request(history, &self.texts)
    }

    /// 起标题的请求（`title.rs`，施工 3-8 五补）：只喂第一轮，一条 user，不接稳定区。
    fn title(&self, history: &History) -> Option<(Request, Seq)> {
        title::request(history, &self.texts)
    }
}

#[cfg(test)]
mod tests;
