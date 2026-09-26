//! 默认的请求组装：从有效历史出一份统一的请求（`docs/designs/08-上下文投影.md` 第四节
//! 「默认的组装怎么写」）。
//!
//! 第 2 层的纯逻辑，实现内核的 [`Assembler`] 接口。请求照这个先后排：
//!
//! 1. 稳定区：工具面（照名字排好）、system、示范对话；
//! 2. 检查点：最近一次压缩的摘要，套上包装；
//! 3. 历史：照有效历史排好的先后，每种事件渲染成对应的消息或内容块；
//! 4. 人这一边挨着的块合成一条 user 消息：检查点最前，事实其次，人的消息最后。
//!
//! 冻结在会话上的东西，也就是稳定区和给模型看的几句固定的字，在造组装器的时候交进来，
//! 一个会话一个（内核 K3）。这里不读文件：出厂的字由执行器从资源目录读好交进来。

mod render;
mod texts;

#[cfg(test)]
mod test_support;

pub use texts::{Texts, TurnEndedTexts};

use miyu_kernel::assemble::Assembler;
use miyu_kernel::history::History;
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
        }
    }
}

#[cfg(test)]
mod tests;
