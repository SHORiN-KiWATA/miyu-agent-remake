//! 发话，执行器这一头（`docs/blueprint/agents.md` 第六条、`docs/blueprint/cross-session.md` 第三条，`session/tools.md`
//! 「父子之间留言」，施工 7-7、C-5）：`to` 认出是哪个会话，把话作为这个会话发来的话（`by` 是这个会话）经会话表的端口
//! （[`SessionPort`]）送过去，对方落了盘就交回。
//!
//! 每一次调用照内核这一刻交的这个会话派出去的子代理（[`Session::subagents`]）造一个端口：那以后才派的，她还不知道编号；
//! 送出去之前被停掉的，送过去它照样收（它的会话还在），父会话不会再认它的回报，和她停它之前刚发出去一样。
//!
//! 施工 C-5：`to` 不是 `parent`、也不认不出任务编号的，交给 `send_message` 认成会话编号（`cross-session.md` 第三条
//! 第 1 款，照 [`miyu_tool::find_session`]），这里只拿认出来的整个编号去送。送到了以后多问一句对方是不是没人看着的
//! 一次性会话（[`SessionPort::held`]），照它交回 [`Delivered::Sent`] 还是 [`Delivered::Held`]。拒绝的原因码（防刷屏的
//! 三种）译成 [`NotSent`] 的那几种；别的原因码（这几条规矩之外的，内核不会真的给，留着兜底）照送不到算。
//!
//! [`Session::subagents`]: miyu_kernel::session::Session::subagents
//! [`SessionPort`]: crate::spawn::SessionPort
//! [`SessionPort::held`]: crate::spawn::SessionPort::held

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{CallId, CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome, Reason, Subagent};
use miyu_tool::{Delivered, MessagePort, NotSent, Recipient, Sending};

use crate::TARGET;
use crate::agents::Agents;

/// 交给一次调用的发话端口：这个会话的父会话、它派出去的子代理照 `subagents`，命令编号照调用 `call`。
pub(crate) fn for_call(
    agents: &Arc<Agents>,
    call: CallId,
    subagents: BTreeMap<JobId, Subagent>,
) -> Arc<dyn MessagePort> {
    Arc::new(Messenger {
        agents: Arc::clone(agents),
        call,
        subagents,
    })
}

/// 一次调用的发话端口。
struct Messenger {
    agents: Arc<Agents>,
    call: CallId,
    subagents: BTreeMap<JobId, Subagent>,
}

impl MessagePort for Messenger {
    fn send<'a>(&'a self, to: Recipient, message: &'a str) -> Sending<'a> {
        Box::pin(async move {
            let (session, label) = self.recipient(to)?;
            let agents = &self.agents;
            let by = By::Session(Session {
                id: agents.session.clone(),
            });
            let send = Command::Send {
                blocks: vec![Block::Text(Text {
                    text: message.to_string(),
                })],
                urgent: false,
            };
            let outcome = agents
                .port
                .command(session.clone(), self.id(), by, send)
                .await;
            match outcome {
                Ok(Outcome::Accepted { .. } | Outcome::Recapped { .. }) => {
                    tracing::info!(target: TARGET, to = label.as_str(), "message sent");
                    // 送到了，再看一眼对方这时是不是没人看着的一次性会话（施工 C-5）：会话已经经 `command` 载入过了。
                    let held = agents.port.held(session).await;
                    Ok(if held {
                        Delivered::Held
                    } else {
                        Delivered::Sent
                    })
                }
                Ok(Outcome::Rejected { reason }) => {
                    let why = format!("refused: {}", reason.code());
                    tracing::warn!(target: TARGET, to = label.as_str(), error = why.as_str(), "message not delivered");
                    Err(match reason {
                        Reason::TooManyMessages => NotSent::TooMany,
                        Reason::DuplicateMessage => NotSent::Duplicate,
                        Reason::InboxFull => NotSent::InboxFull,
                        _ => NotSent::Undelivered,
                    })
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, to = label.as_str(), error = error.as_str(), "message not delivered");
                    Err(NotSent::Undelivered)
                }
            }
        })
    }
}

impl Messenger {
    /// 发给哪个会话，和运行日志里怎么写它：父会话写 `parent`，子代理写它的编号，别的会话（施工 C-5）写短编号。
    fn recipient(&self, to: Recipient) -> Result<(SessionId, String), NotSent> {
        match to {
            Recipient::Parent => {
                let parent = self.agents.parent.clone().ok_or(NotSent::NoParent)?;
                Ok((parent, "parent".to_string()))
            }
            Recipient::Child(job) => match self.subagents.get(&job) {
                None => Err(NotSent::NotYours),
                Some(subagent) if subagent.stopped => Err(NotSent::Stopped),
                Some(subagent) => Ok((subagent.session.clone(), job.to_string())),
            },
            Recipient::Session(session) => {
                let label = session.short().to_string();
                Ok((session, label))
            }
        }
    }

    /// 命令编号：`<这个会话>/message/<调用编号>`。会话编号整个数据根里不重，调用编号一个会话里不重，所以它在哪儿都不重；
    /// 一次调用只送一次。
    fn id(&self) -> CommandId {
        CommandId::parse(&format!("{}/message/{}", self.agents.session, self.call))
            .unwrap_or_else(|e| unreachable!("会话编号、调用编号都短，合命令编号的写法：{e}"))
    }
}
