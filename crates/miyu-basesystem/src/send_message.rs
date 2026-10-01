//! `send_message`（`docs/blueprint/tools/send_message.md`，施工 7-7、C-5）：给自己派的、还没被停掉的子代理（`to` 写它的
//! 任务编号）留言，给自己的父（`to: parent`），或者给不在这棵树上的别的会话（`to` 写它的会话编号，施工 C-5）。留言作为
//! 这个会话发来的话送过去，对方落了盘就返回，不等它回答：对方在跑，下一步看到；闲着，开一轮；是没人看着的一次性会话，
//! 只记下（`cross-session.md` 第三条第 4 款）。
//!
//! 施工 C-5 从 `message_agent` 改名（以前的名字照样认，[`Tool::formerly`]）：`to` 不是 `parent`、也不是任务编号写法的，
//! 当会话编号认，和 `history` 的 `session` 同一个认法（[`find_session`]，`cross-session.md` 第三条第 1 款）：在这个会话
//! 看得到的主会话（加她自己）里找，找不到、对得上不止一个、是她自己、这个会话不能发给别的会话的，各一句拒绝，不去送。
//! `message` 超过 `peers.message_chars`（出厂值不进快照，照 `jobs.output_chars` 的放法）的，发出去之前拒；父子之间的
//! 留言也照它。
//!
//! 施工 C-6 多一格 `notify_when_idle`（`cross-session.md` 第六条第 1、2 款）：`to` 是别的会话的，那次调用报一样效果
//! `peer.watch`，结果接一句「空下来时告诉你」。工具自己不去订：效果落了盘，执行器照内核算的在等的去订（`peers.rs`）。
//! 带 `message` 的先发话，送到了（`sent`、`held`、一模一样的都算）再订，发话被拒、送不到的整次不订。`message` 可以不写：
//! 只订不发，那边不开轮。`to` 是子代理、`parent` 的整次拒，留言也不发（照 Claude Code）。两样都没有的，参数不对。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::{JobMessaged, PeerWatch};
use miyu_kernel::id::{JobId, SessionId};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{
    Call, Delivered, Done, Effect, Found, NotSent, Progress, Recipient, Running, SEND_MESSAGE,
    SEND_MESSAGE_FORMERLY, Spec, Tool, find_session,
};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// `to` 写这个是发给父会话。
const PARENT: &str = "parent";

/// 一句话最多几个字（策略数据 `peers.message_chars` 的出厂值，`cross-session.md` 第五条第 4 款）：和 `jobs.output_chars`
/// 一样放在代码里，不进快照；配置那一步能改。
const MESSAGE_CHARS: usize = 100_000;

/// `send_message`。
pub(crate) struct SendMessage {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/send_message/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    sent: Template,
    held: Template,
    no_parent: Template,
    not_yours: Template,
    stopped: Template,
    not_sent: Template,
    no_session: Template,
    ambiguous: Template,
    itself: Template,
    not_here: Template,
    too_long: Template,
    too_many: Template,
    duplicate: Template,
    inbox_full: Template,
    watching: Template,
    watch_peers_only: Template,
}

/// 她给的参数。别的参数不认，也不报错。`null` 当没写。
#[derive(Deserialize)]
struct Args {
    to: String,
    /// 只订不发的可以不写（施工 C-6）。
    #[serde(default)]
    message: Option<String>,
    /// 订「空了告诉我」（施工 C-6）。
    #[serde(default)]
    notify_when_idle: Option<bool>,
}

impl SendMessage {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读，和 `subagent` 一样：留言什么都不改，只读
    /// 开着也发得出去；一步里给几个会话留言，连着的一起发。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<SendMessage, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, SEND_MESSAGE, name, fields);
        Ok(SendMessage {
            spec: load::spec(resources, SEND_MESSAGE, Access::Read)?,
            texts: Texts {
                common,
                sent: text("sent", &["to"])?,
                held: text("held", &["to"])?,
                no_parent: text("no-parent", &[])?,
                not_yours: text("not-yours", &["to"])?,
                stopped: text("stopped", &["to"])?,
                not_sent: text("not-sent", &[])?,
                no_session: text("no-session", &["to"])?,
                ambiguous: text("ambiguous", &["to"])?,
                itself: text("self", &["to"])?,
                not_here: text("not-here", &[])?,
                too_long: text("too-long", &["chars", "limit"])?,
                too_many: text("too-many", &["to"])?,
                duplicate: text("duplicate", &["to"])?,
                inbox_full: text("inbox-full", &["to"])?,
                watching: text("watching", &["to"])?,
                watch_peers_only: text("watch-peers-only", &[])?,
            },
        })
    }
}

impl Tool for SendMessage {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    /// 改名以前叫 `message_agent`：以前造的会话快照里冻着它，她照它调（施工 C-5）。
    fn formerly(&self) -> &'static [&'static str] {
        &[SEND_MESSAGE_FORMERLY]
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let watch = args.notify_when_idle == Some(true);
            if args.message.is_none() && !watch {
                return texts.common.bad_args(&"missing field `message`");
            }
            let to = args.to.as_str();
            let recipient = match recipient(&texts, &call, to).await {
                Ok(recipient) => recipient,
                Err(done) => return *done,
            };
            // 只能等别的会话空下来（`cross-session.md` 第六条第 1 款）：订子代理、父会话的整次拒，留言也不发。
            let watched = match (&recipient, watch) {
                (Recipient::Session(session), true) => Some(session.clone()),
                (_, true) => return watch_peers_only(&texts),
                (_, false) => None,
            };
            let sent = match &args.message {
                Some(message) => match deliver(&texts, &call, recipient.clone(), to, message).await
                {
                    Ok(done) => Some(done),
                    Err(refused) => return refused,
                },
                None => None,
            };
            match (sent, watched) {
                (sent, Some(session)) => watching(&texts, sent, to, session),
                // 给子代理的留言记一样效果：它欠一份回报，这个会话照它等（`agents.md` 第二条第 2 条）。别的会话、父会话
                // 不欠她什么，不报。
                (Some(done), None) => match recipient {
                    Recipient::Child(job) => done.effect(Effect::JobMessaged(JobMessaged { job })),
                    Recipient::Parent | Recipient::Session(_) => done,
                },
                // 两样都没有的上面已经交回参数不对，这里照同一句，不 panic。
                (None, None) => texts.common.bad_args(&"missing field `message`"),
            }
        })
    }
}

/// 发话：长度上限、交给端口。送到了的（`sent`、`held`、一模一样的）交回那一句；拒绝、送不到的交回出错的那一句。
async fn deliver(
    texts: &Texts,
    call: &Call,
    recipient: Recipient,
    to: &str,
    message: &str,
) -> Result<Done, Done> {
    let chars = message.chars().count();
    if chars > MESSAGE_CHARS {
        return Err(too_long(texts, chars));
    }
    let Some(port) = &call.messages else {
        return Err(refused(texts, NotSent::Undelivered, to));
    };
    match port.send(recipient, message).await {
        Ok(delivered) => Ok(delivered_ok(texts, delivered, to)),
        Err(NotSent::Duplicate) => Ok(refused(texts, NotSent::Duplicate, to)),
        Err(why) => Err(refused(texts, why, to)),
    }
}

/// 订了（施工 C-6）：报效果 `peer.watch`，`session` 是认出来的整个编号；结果接一句 `watching.txt`。先发过话的接在那一句
/// 后面，给人看的说法照发话的；只订不发的，给人看的说法是「空下来时告诉她」。
fn watching(texts: &Texts, sent: Option<Done>, to: &str, session: SessionId) -> Done {
    let line = say(&texts.watching, &[("to", to)]);
    let done = match sent {
        Some(mut done) => {
            if let Some(miyu_kernel::block::Block::Text(text)) = done.blocks.first_mut() {
                text.text.push_str(&line);
            }
            done
        }
        None => Done::ok(line).said(said("send_message/watching").with("to", to)),
    };
    done.effect(Effect::PeerWatch(PeerWatch { session }))
}

/// 订子代理、父会话的：整次拒（施工 C-6）。
fn watch_peers_only(texts: &Texts) -> Done {
    Done::error(say(&texts.watch_peers_only, &[])).said(said("send_message/watch-peers-only"))
}

/// 认 `to`（`cross-session.md` 第三条第 1 款）：`parent`、任务编号写法的，照旧；别的当会话编号认，和 `history` 的
/// `session` 同一个认法——在这个会话看得到的主会话（加她自己）里找，找不到、对得上不止一个、是她自己、没有列会话的
/// 端口（子会话、场所会话）：各一句拒绝，不去送，一条日志都不读、一次会话都不开。
async fn recipient(texts: &Texts, call: &Call, to: &str) -> Result<Recipient, Box<Done>> {
    if to == PARENT {
        return Ok(Recipient::Parent);
    }
    if let Ok(job) = JobId::parse(to) {
        return Ok(Recipient::Child(job));
    }
    let Some(sessions) = &call.sessions else {
        return Err(Box::new(not_here(texts)));
    };
    let others = match sessions.list(&call.stop).await {
        Ok(others) => others,
        Err(_) => return Err(Box::new(refused(texts, NotSent::Undelivered, to))),
    };
    let among = std::iter::once(sessions.this()).chain(others.iter().map(|other| &other.id));
    match find_session(to, among) {
        Found::None => Err(Box::new(no_session(texts, to))),
        Found::Many => Err(Box::new(ambiguous(texts, to))),
        Found::One(id) if &id == sessions.this() => Err(Box::new(itself(texts, to))),
        Found::One(id) => Ok(Recipient::Session(id)),
    }
}

/// 送到了：是不是落进了一个没人看着的一次性会话（施工 C-5）。两种都不算出错。
fn delivered_ok(texts: &Texts, delivered: Delivered, to: &str) -> Done {
    match delivered {
        Delivered::Sent => {
            Done::ok(say(&texts.sent, &[("to", to)])).said(said("send_message/sent").with("to", to))
        }
        Delivered::Held => {
            Done::ok(say(&texts.held, &[("to", to)])).said(said("send_message/held").with("to", to))
        }
    }
}

/// 没送出去：照为什么说那一句，发给谁的写进去。一模一样的那一句不算出错：那句话已经在那边了。
fn refused(texts: &Texts, why: NotSent, to: &str) -> Done {
    let (template, key) = match why {
        NotSent::NoParent => (&texts.no_parent, "no-parent"),
        NotSent::NotYours => (&texts.not_yours, "not-yours"),
        NotSent::Stopped => (&texts.stopped, "stopped"),
        NotSent::Undelivered => (&texts.not_sent, "not-sent"),
        NotSent::TooMany => (&texts.too_many, "too-many"),
        NotSent::Duplicate => (&texts.duplicate, "duplicate"),
        NotSent::InboxFull => (&texts.inbox_full, "inbox-full"),
    };
    let text = say(template, &[("to", to)]);
    let human = said(&format!("send_message/{key}")).with("to", to);
    if matches!(why, NotSent::Duplicate) {
        Done::ok(text).said(human)
    } else {
        Done::error(text).said(human)
    }
}

/// 这个会话不能发给别的会话、不能订别的会话（`cross-session.md` 第九条）：不去找，直接拒。
fn not_here(texts: &Texts) -> Done {
    Done::error(say(&texts.not_here, &[])).said(said("send_message/not-here"))
}

/// 找不到会话 `to`。
fn no_session(texts: &Texts, to: &str) -> Done {
    Done::error(say(&texts.no_session, &[("to", to)]))
        .said(said("send_message/no-session").with("to", to))
}

/// `to` 对得上不止一个会话。
fn ambiguous(texts: &Texts, to: &str) -> Done {
    Done::error(say(&texts.ambiguous, &[("to", to)]))
        .said(said("send_message/ambiguous").with("to", to))
}

/// `to` 就是这个会话自己。
fn itself(texts: &Texts, to: &str) -> Done {
    Done::error(say(&texts.itself, &[("to", to)])).said(said("send_message/self").with("to", to))
}

/// `message` 太长了：`chars` 个字，上限是 [`MESSAGE_CHARS`]。
fn too_long(texts: &Texts, chars: usize) -> Done {
    let chars = chars.to_string();
    let limit = MESSAGE_CHARS.to_string();
    Done::error(say(
        &texts.too_long,
        &[("chars", chars.as_str()), ("limit", limit.as_str())],
    ))
    .said(
        said("send_message/too-long")
            .with("chars", chars.as_str())
            .with("limit", limit.as_str()),
    )
}
