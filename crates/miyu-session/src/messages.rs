//! 父子之间留言，执行器这一头（`docs/blueprint/agents.md` 第六条，`session/tools.md`「父子之间留言」，施工 7-7）：`to` 认出
//! 是哪个会话，把留言作为这个会话发来的话（`by` 是这个会话）经会话表的端口（[`SessionPort`]）送过去，对方落了盘就交回。
//!
//! 每一次调用照内核这一刻交的这个会话派出去的子代理（[`Session::subagents`]）造一个端口：那以后才派的，她还不知道编号；
//! 送出去之前被停掉的，送过去它照样收（它的会话还在），父会话不会再认它的回报，和她停它之前刚发出去一样。
//!
//! [`Session::subagents`]: miyu_kernel::session::Session::subagents
//! [`SessionPort`]: crate::spawn::SessionPort

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{CallId, CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome, Subagent};
use miyu_tool::{MessagePort, NotSent, Recipient, Sending};

use crate::TARGET;
use crate::agents::Agents;

/// 交给一次调用的留言端口：这个会话的父会话、它派出去的子代理照 `subagents`，命令编号照调用 `call`。
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

/// 一次调用的留言端口。
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
            let why = match agents.port.command(session, self.id(), by, send).await {
                Ok(Outcome::Accepted { .. } | Outcome::Recapped { .. }) => {
                    tracing::info!(target: TARGET, to = label.as_str(), "message sent");
                    return Ok(());
                }
                Ok(Outcome::Rejected { reason }) => format!("refused: {}", reason.code()),
                Err(error) => error,
            };
            tracing::warn!(target: TARGET, to = label.as_str(), error = why.as_str(), "message not delivered");
            Err(NotSent::Undelivered)
        })
    }
}

impl Messenger {
    /// 发给哪个会话，和运行日志里怎么写它：父会话写 `parent`，子代理写它的编号。
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
        }
    }

    /// 命令编号：`<这个会话>/message/<调用编号>`。会话编号整个数据根里不重，调用编号一个会话里不重，所以它在哪儿都不重；
    /// 一次调用只送一次。
    fn id(&self) -> CommandId {
        CommandId::parse(&format!("{}/message/{}", self.agents.session, self.call))
            .unwrap_or_else(|e| unreachable!("会话编号、调用编号都短，合命令编号的写法：{e}"))
    }
}
