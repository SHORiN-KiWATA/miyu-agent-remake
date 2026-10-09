//! 在判的（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 11、12 条）：交给判官的一条记在桥的内存里（`Judging`，
//! `chat.md` 第七条第 4 条：只在桥的内存里，桥重启就丢，不补判），问判官的任务在这里起、在这里收。
//!
//! - 交给判官（[`Judges::start`]）：记下在判的、发一张号（`ticket`），起一个任务（`ask.rs`）。
//! - 顶替要放下的（[`Judges::cancel`]）：拿掉在判的那一条；任务不掐，回来的回答没人认，丢掉（施工单「要定的」第 1 条）。
//! - 判官回来了（[`Judges::take`]）：照号拿回在判的那一条；被放下的拿不到。
//! - 群会话用的人格（施工 O-23 补，第 1 条、第 12 条第 3 款）：订阅上了照回应的 `persona` 记下（[`Judges::subscribed`]），
//!   交给判官时照它带（[`Judges::persona`]）；原文另记在 `persona.rs`，几个任务共用。

use std::collections::HashMap;
use std::sync::Arc;

use miyu_chat::{Chatty, JudgeTexts, Pending, Standing};
use tokio::task::JoinSet;

use super::ask::{Answer, Asking, Slots, ask};
use super::decide::Decision;
use super::persona::Personas;
use crate::core::Caller;

/// 判的那几条里最后一条的：判断、开一轮的命令编号照它拼，运行日志照它记。
#[derive(Debug, Clone)]
pub(super) struct Tag {
    /// 命令编号（「群消息」第 6 条）。
    pub(super) id: String,
    /// 平台的消息编号：只记运行日志。
    pub(super) number: i64,
    /// 场所编号：只记运行日志。
    pub(super) venue: String,
}

/// 交给判官的一条：判官回来时算分、记判断要的，都在交出去的那一刻定下（「施工时定的」第 100 条）。
#[derive(Debug, Clone)]
pub(super) struct Judging {
    /// 群会话的编号。
    pub(super) session: String,
    /// 顶替看的样子：`status` 是 `Judging`，`msg` 是最后一条、`absorbed` 是前面几条，条件是合起来的。
    pub(super) pending: Pending,
    /// 判的几条的正文，照先后：再顶替时几条一起解 base64。
    pub(super) texts: Vec<String>,
    /// 到问判官那一步的判断：进站链、线路规程、条件、顶替、走的路。
    pub(super) decision: Decision,
    /// 发的人是谁。
    pub(super) standing: Standing,
    /// 最后一条的命令编号、平台编号、场所。
    pub(super) tag: Tag,
    /// 交出去时套的那一份参数：算分照它。
    pub(super) chatty: Chatty,
}

/// 一个任务问完了：号和回答；核心断开了的没有回答。
pub(super) type Asked = (u64, Option<Answer>);

/// 在判的和问判官的任务。
pub(super) struct Judges {
    /// 并着发的调用口。
    caller: Caller,
    /// 判官的说明。
    texts: Arc<JudgeTexts>,
    /// 全局的名额。
    slots: Slots,
    /// 读到的人格原文，几个任务共用。
    personas: Personas,
    /// 群会话编号 → 它用的人格（订阅回应的 `persona`）：无人格的不在里面。
    used: HashMap<String, String>,
    /// 在跑的任务：放下 `Route` 时一起掐掉（桥在停）。
    pub(super) running: JoinSet<Asked>,
    /// 在判的，照交出去的先后，带着号。
    judging: Vec<(u64, Judging)>,
    /// 下一张号。
    next: u64,
}

impl Judges {
    /// 经 `caller` 问、照 `texts` 拼请求、全局照 `slots` 排队，人格的原文照 `personas` 记。
    pub(super) fn new(
        caller: Caller,
        texts: Arc<JudgeTexts>,
        slots: Slots,
        personas: Personas,
    ) -> Judges {
        Judges {
            caller,
            texts,
            slots,
            personas,
            used: HashMap::new(),
            running: JoinSet::new(),
            judging: Vec::new(),
            next: 0,
        }
    }

    /// 群会话 `session` 订阅上了，回应说它用人格 `persona`（没有这一格的是无人格）：有的记下。会话的人格造的时候就定了
    /// （照日志第一条 `session.created`，`protocol.md` 的 `subscribe`），掉了队再订阅也是同一个，不会从有变成没有。
    pub(super) fn subscribed(&mut self, session: &str, persona: Option<&str>) {
        if let Some(persona) = persona {
            self.used.insert(session.to_string(), persona.to_string());
        }
    }

    /// 群会话 `session` 用的人格；无人格的、没订阅上的是空的。
    pub(super) fn persona(&self, session: &str) -> Option<&str> {
        self.used.get(session).map(String::as_str)
    }

    /// 群会话 `session` 里在判的，交给顶替看。
    pub(super) fn pendings(&self, session: &str) -> impl Iterator<Item = &Pending> {
        self.judging
            .iter()
            .filter(move |(_, judging)| judging.session == session)
            .map(|(_, judging)| &judging.pending)
    }

    /// 交给判官：记下 `judging`，照 `asking` 起一个任务。
    pub(super) fn start(&mut self, judging: Judging, asking: Asking) {
        self.next += 1;
        let ticket = self.next;
        self.judging.push((ticket, judging));
        let (caller, texts, slots, personas) = (
            self.caller.clone(),
            Arc::clone(&self.texts),
            self.slots.clone(),
            self.personas.clone(),
        );
        self.running
            .spawn(async move { (ticket, ask(asking, caller, texts, slots, personas).await) });
    }

    /// 顶替要放下群会话 `session` 里挂在序号 `msg` 上的那一次：拿掉在判的、交回它，回来的回答没人认。
    pub(super) fn cancel(&mut self, session: &str, msg: u64) -> Option<Judging> {
        let at = self.judging.iter().position(|(_, judging)| {
            judging.session == session && judging.pending.msg.get() == msg
        })?;
        Some(self.judging.remove(at).1)
    }

    /// 号是 `ticket` 的那一次回来了：拿回在判的；被放下的是空的。
    pub(super) fn take(&mut self, ticket: u64) -> Option<Judging> {
        let at = self.judging.iter().position(|(held, _)| *held == ticket)?;
        Some(self.judging.remove(at).1)
    }
}
