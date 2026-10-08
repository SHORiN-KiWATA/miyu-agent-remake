//! 发给会话的命令（`02-内核.md` 第三节）：意图、回答、打断时排着的怎么办。施工 9-7 上从 `input.rs` 挪出来（那一份到了 500 行）。

use crate::block::Block;
use crate::event::{ChildReported, Decision, Level, Response};
use crate::id::{CallId, TurnId};

/// 发给会话的意图（`02-内核.md` 第三节）。结局只有两种：被接受并产生事件，或被拒绝并附原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `session.send`：发一条消息。会话空闲时，还会开一个回合。
    Send {
        /// 消息的内容块。
        blocks: Vec<Block>,
        /// 急着插话：这一步还没跑的工具跳过，这句话马上进下一步（`02-内核.md` 第六节
        /// 「打断和急着插话」）。
        urgent: bool,
    },
    /// `session.set_permission_level`：开关只读，或者改常用的那一级，改哪样写哪样
    /// （`02-内核.md` 第六节「权限级别怎么切」）。
    SetPermission {
        /// 常用的那一级；不改就没有。
        level: Option<Level>,
        /// 只读开关；不改就没有。
        read_only: Option<bool>,
    },
    /// `session.configure`：换模型（施工 8-10），`configure.rs`。下一个回合开始时生效；和现在的一样的不记。
    Configure {
        /// 换成的引用：模型或 `@池`，协议那一头已经查过。内核只存字，不解读。
        model: String,
    },
    /// `session.set_meta`：改标题、置顶，改哪样写哪样（施工 3-8 三补，`meta.rs`）。
    SetMeta {
        /// 新的标题，照 `session.meta_changed` 的写法：空的是去掉标题；不改就没有。去掉前后空白、量长短是协议端点的事
        /// （`docs/blueprint/protocol.md` 的 `session.set_meta`），内核照原样比、照原样记。
        title: Option<String>,
        /// 置顶还是取消置顶；不改就没有。
        pinned: Option<bool>,
    },
    /// `session.set_workspace`：换会话在哪个目录干活（施工 9-7 上，`workspace.rs`）。协议那一头判过太不太宽、换成了实际用的；
    /// 和现在一样的不记。
    SetWorkspace {
        /// 新的工作目录。
        cwd: String,
        /// 新的加进来的目录；不换就没有。
        dirs: Option<Vec<String>>,
    },
    /// 记下人用了一个斜杠命令（施工 O-6，`command.run`）：命令本身已经照它自己的编号执行了，这一条只记 `command.ran`。
    Ran {
        /// 人打的原文。
        text: String,
        /// 执行了哪个命令的正名。
        command: String,
    },
    /// `session.interrupt`：打断正在进行的回合。
    Interrupt {
        /// 排着队的消息怎么办（`02-内核.md` 第六节「排队的消息」）。
        queued: Queued,
    },
    /// `session.answer`：回答一次权限确认，或者一组题（`02-内核.md` 第六节「确认怎么走」第 3 条、
    /// 「提问怎么走」第 3 条）。
    Answer {
        /// 回答的是哪一次调用的请求或者题目。
        call_id: CallId,
        /// 回答。
        answer: Answer,
    },
    /// `session.revert`：从这一轮起撤销，它和它以后的全撤（`02-内核.md` 第六节「撤销与恢复」）。
    Revert {
        /// 从哪一轮起。不写的，撤还在有效历史里的最后一轮（施工 4-7 下）。
        turn: Option<TurnId>,
    },
    /// `session.unrevert`：恢复最近一次撤销，在下一轮开始、压缩之前。
    Unrevert,
    /// `session.redo`：重做最后一轮（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补）：撤掉它，把开它的那几句
    /// 人的话再发一次，开新的一轮。最后一轮不是人的话开的、一轮都没有的，拒绝，`not_redoable`。
    Redo {
        /// 开这一轮的那一句里的字换成这几块（原来的文字块全换掉，空的是不要字）；`None` 是字照原来的。
        text: Option<Vec<Block>>,
        /// 开这一轮的那一句里的附件换成这几块（原来文字以外的块全换掉，空的是不要附件）；`None` 是附件照原来的。两样都是
        /// `None` 的原样重发。
        attachments: Option<Vec<Block>>,
    },
    /// `session.compact`：手动压缩，空闲时才收，单开一轮只做压缩（`compaction.md` 第七条，施工 6-8）。
    Compact {
        /// 人附的要求，原样；`None` 是没附。只有空白的也当没附。
        instructions: Option<String>,
    },
    /// `session.clear`：清空上下文，空闲时才收，单开一轮压成一个空的检查点，不请求模型（`compaction.md` 第十四条，
    /// 施工 6-8 补）。
    Clear,
    /// `session.recap`：要一句回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）。有回合在进行时照收，照这一刻
    /// 落了盘的有效历史；不开回合、不进她的上下文。
    Recap,
    /// 子会话交来的回报（施工 7-2，`agents.md` 第二条第 5 条）：子会话的执行器经端口交，发命令的一方就是子会话。记一条
    /// `child.reported`，不带回合编号。对不上一个还会报的子代理的，拒绝，`unknown_job`。
    Report(ChildReported),
    /// 被等的会话交来的「空了」（施工 C-6，`docs/blueprint/cross-session.md` 第六条第 6、7 款）：发命令的一方就是它。这边
    /// 在等它的，记一条 `peer.idle`（`idle`），不带回合编号，照回报的规矩叫不叫醒她；不在等的拒绝，`unknown_watch`。
    PeerIdle {
        /// 它最近结束的那一轮最后一条有字的回复的第一行，被等的那一边截好的；一个字都没说的没有。
        status: Option<String>,
    },
}

/// 一次回答：回答确认的，或者回答一组题的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// 回答确认。
    Approval {
        /// 选了哪一项。
        decision: Decision,
        /// 拒绝的理由；只有拒绝能带，空的当没写。
        reason: Option<String>,
    },
    /// 回答一组题：照题目的先后，每道题选了哪几项、自己写了什么。
    Questions(Vec<Response>),
}

/// 打断时，排着队的消息怎么办。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queued {
    /// 接着发：打断以后马上开一轮，由最后一条触发。
    Send,
    /// 退回：撤回来，交还给头，放回输入框。
    Return,
    /// 留着（施工 O-6，`/stop` 全停）：不撤回，也不接着开一轮；照样在历史里，下一句话开的那一轮看得到。
    Keep,
}
