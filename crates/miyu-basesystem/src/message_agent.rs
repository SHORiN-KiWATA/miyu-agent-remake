//! `message_agent`（`docs/blueprint/tools/message_agent.md`，施工 7-7）：父子之间留言。`to` 写自己派的子代理的任务编号，
//! 或者 `parent`；`message` 原样送过去。经 [`Call::messages`] 交给执行器，对方落了盘就返回，不等它回答：回答自己会作为
//! 对方的留言、回报送回来（`agents.md` 第六条）。给子代理的留言报一样效果 `job.messaged`：它欠一份回报。
//!
//! 只在树上相邻的两层之间（2026-09-29 项目主人定）：`to` 只认这两种，别的都拒，拒的时候说清为什么。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::JobMessaged;
use miyu_kernel::id::JobId;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{
    Call, Done, Effect, MESSAGE_AGENT, NotSent, Progress, Recipient, Running, Spec, Tool,
};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// `to` 写这个是发给父会话。
const PARENT: &str = "parent";

/// `message_agent`。
pub(crate) struct MessageAgent {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/message_agent/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    sent: Template,
    no_parent: Template,
    not_yours: Template,
    stopped: Template,
    not_sent: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    to: String,
    message: String,
}

impl MessageAgent {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读，和 `agent` 一样：留言什么都不改，只读开着
    /// 也发得出去；一步里给几个子代理留言，连着的一起发。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<MessageAgent, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, MESSAGE_AGENT, name, fields);
        Ok(MessageAgent {
            spec: load::spec(resources, MESSAGE_AGENT, Access::Read)?,
            texts: Texts {
                common,
                sent: text("sent", &["to"])?,
                no_parent: text("no-parent", &[])?,
                not_yours: text("not-yours", &["to"])?,
                stopped: text("stopped", &["to"])?,
                not_sent: text("not-sent", &[])?,
            },
        })
    }
}

impl Tool for MessageAgent {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let to = args.to.as_str();
            // 不是 `parent`、也不是任务编号的写法的，不会是她派的：端口不问。
            let recipient = match (to, JobId::parse(to)) {
                (PARENT, _) => Recipient::Parent,
                (_, Ok(job)) => Recipient::Child(job),
                (_, Err(_)) => return texts.refused(NotSent::NotYours, to),
            };
            let Some(port) = call.messages else {
                return texts.refused(NotSent::Undelivered, to);
            };
            if let Err(why) = port.send(recipient, &args.message).await {
                return texts.refused(why, to);
            }
            let sent = Done::ok(say(&texts.sent, &[("to", to)]))
                .said(said("message_agent/sent").with("to", to));
            // 给子代理的留言记一样效果：它欠一份回报，这个会话照它等（`agents.md` 第二条第 2 条）。
            match recipient {
                Recipient::Child(job) => sent.effect(Effect::JobMessaged(JobMessaged { job })),
                Recipient::Parent => sent,
            }
        })
    }
}

impl Texts {
    /// 没送出去：照为什么说那一句，发给谁的写进去。
    fn refused(&self, why: NotSent, to: &str) -> Done {
        let (template, key) = match why {
            NotSent::NoParent => (&self.no_parent, "no-parent"),
            NotSent::NotYours => (&self.not_yours, "not-yours"),
            NotSent::Stopped => (&self.stopped, "stopped"),
            NotSent::Undelivered => (&self.not_sent, "not-sent"),
        };
        Done::error(say(template, &[("to", to)]))
            .said(said(&format!("message_agent/{key}")).with("to", to))
    }
}
