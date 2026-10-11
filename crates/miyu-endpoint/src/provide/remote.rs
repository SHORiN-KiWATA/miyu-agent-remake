//! 提供者的一件工具（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 3 条）：执行时照包查提供者表，没有连接的交回「暂时
//! 不可用」；有的，反向调用 `tool.call {session, call_id, tool, args, cwd, by, owner}`，照回应 `{blocks, error}` 交回结果。回应是
//! 错误的、写法不对的，算出错，原话写进结果；等着时连接断了，也是暂时不可用。到了登记时写的时限没回的，交回「没在 N 秒内答完」；
//! 超时、叫它停、掐掉（这个 future 被丢掉）时发一次 `tool.cancel {session, call_id}`，回应先到的不发（施工 O-2 下）。
//!
//! 交回的图、文件引用的 blob 是扩展经 `blob.put` 传进管理员名下的，她照会话的属主读：回应到了先拷一份进属主名下，块原样不动；
//! 拷不成的（管理员名下也没有这个 blob、存不下）照暂时不可用（施工 O-2 三补，同 `session.send` 的附件，`attach::copy_over`）。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Said;
use miyu_kernel::id::AccountId;
use miyu_kernel::template::Template;
use miyu_store::root::DataRoot;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool, Venues};

use super::{Checked, Provided};
use crate::reverse::{Gone, Peer};

/// 叫它停了没有，隔多久看一眼旗。
const STOP_POLL: Duration = Duration::from_millis(100);

/// 提供者的工具替自己写的两句的模板。
#[derive(Clone)]
pub(super) struct Texts {
    /// 暂时不可用（`core/tool-results/unavailable.txt`），字段 `name`。
    pub(super) unavailable: String,
    /// 没在时限里答完（`core/tool-results/timed-out.txt`），字段 `name`、`seconds`。
    pub(super) timed_out: String,
}

/// 扩展传上来的 blob 在哪（施工 O-2 三补）：数据根和管理员，`blob.put` 都存在管理员名下。
#[derive(Clone)]
pub(crate) struct Stores {
    pub(crate) root: DataRoot,
    pub(crate) admin: AccountId,
}

/// 提供者的一件工具。
pub(crate) struct RemoteTool {
    spec: Spec,
    venues: Venues,
    timeout: Duration,
    package: String,
    provided: Arc<Provided>,
    texts: Texts,
    stores: Stores,
}

/// 回应里的结果。
#[derive(Deserialize)]
struct Answer {
    blocks: Vec<Block>,
    #[serde(default)]
    error: bool,
}

impl RemoteTool {
    /// 包 `package` 提供的这一件。
    pub(super) fn new(
        checked: Checked,
        package: &str,
        provided: Arc<Provided>,
        texts: Texts,
        stores: Stores,
    ) -> RemoteTool {
        RemoteTool {
            spec: checked.spec,
            venues: checked.venues,
            timeout: checked.timeout,
            package: package.to_string(),
            provided,
            texts,
            stores,
        }
    }

    /// 暂时不可用：说法同执行器替目录里没有的工具写的那一句。
    fn unavailable(&self) -> Done {
        let name = self.spec.name.as_str();
        Done {
            human: Some(Said::new("core/tool-results/unavailable").with("name", name)),
            ..failed(render(&self.texts.unavailable, &[("name", name)]))
        }
    }

    /// 没在时限里答完：秒数往上取整。
    fn timed_out(&self) -> Done {
        let name = self.spec.name.as_str();
        let seconds = self.timeout.as_millis().div_ceil(1000).to_string();
        Done {
            human: Some(
                Said::new("core/tool-results/timed-out")
                    .with("name", name)
                    .with("seconds", &seconds),
            ),
            ..failed(render(
                &self.texts.timed_out,
                &[("name", name), ("seconds", &seconds)],
            ))
        }
    }

    /// 照回应交回结果。
    fn answered(&self, outcome: Result<Result<Value, Value>, Gone>) -> Done {
        match outcome {
            Ok(Ok(result)) => match serde_json::from_value::<Answer>(result.clone()) {
                Ok(answer) => Done {
                    error: answer.error,
                    blocks: answer.blocks,
                    ..Done::ok("")
                },
                Err(_) => failed(result.to_string()),
            },
            Ok(Err(error)) => failed(
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .map_or_else(|| error.to_string(), str::to_string),
            ),
            Err(Gone) => self.unavailable(),
        }
    }

    /// 交回的图、文件引用的 blob 拷一份进会话的属主 `owner` 名下（施工 O-2 三补）。没有引用的、不知道是哪个会话的（测试里的
    /// 假调用）原样交回；拷不成的记一行，照暂时不可用。
    async fn handed(&self, done: Done, owner: Option<AccountId>) -> Done {
        let hashes = crate::attach::blobs_of(&done.blocks);
        let Some(owner) = owner.filter(|_| !hashes.is_empty()) else {
            return done;
        };
        let stores = self.stores.clone();
        let copied = tokio::task::spawn_blocking(move || {
            crate::attach::copy_over(&stores.root, &stores.admin, &owner, &hashes)
        })
        .await;
        match copied {
            Ok(Ok(())) => done,
            Ok(Err(_)) | Err(_) => {
                tracing::warn!(target: "miyu::endpoint", tool = self.spec.name.as_str(), "tool blobs not handed over");
                self.unavailable()
            }
        }
    }
}

impl Tool for RemoteTool {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn venues(&self) -> Option<Venues> {
        Some(self.venues)
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let Some(peer) = self.provided.peer(&self.package) else {
                return self.unavailable();
            };
            let args: Value = serde_json::from_str(&call.args).unwrap_or_else(|_| json!({}));
            let ids = call.ids.as_ref();
            let asked = ids.and_then(|ids| ids.asked.as_ref());
            let session = ids.map(|ids| ids.session.as_str());
            let owner = ids.map(|ids| ids.owner.clone());
            let call_id = ids.map(|ids| ids.call.to_string());
            let mut params = json!({
                "session": session,
                "call_id": call_id,
                "tool": self.spec.name,
                "args": args,
                "cwd": call.cwd,
                "owner": asked.is_some_and(|by| by.is_owner()),
            });
            if let Some(by) = asked {
                params["by"] = json!(by);
            }
            let mut cancel = Cancel {
                peer: peer.clone(),
                params: json!({"session": session, "call_id": call_id}),
                state: Sending::Armed,
            };
            let deadline = tokio::time::Instant::now() + self.timeout;
            let answer = peer.call("tool.call", params);
            tokio::pin!(answer);
            loop {
                tokio::select! {
                    outcome = &mut answer => {
                        cancel.state = Sending::Done;
                        return self.handed(self.answered(outcome), owner).await;
                    }
                    // 超时：交回那一句，`tool.cancel` 由 `cancel` 丢掉时发。
                    () = tokio::time::sleep_until(deadline) => return self.timed_out(),
                    // 叫它停（打断改东西的调用）：告诉提供者，照样等它回，内核到点掐掉。
                    () = tokio::time::sleep(STOP_POLL), if cancel.state == Sending::Armed => {
                        if call.stop.stopped() {
                            cancel.send();
                        }
                    }
                }
            }
        })
    }
}

/// `tool.cancel` 发到哪一步了。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sending {
    /// 还没发，丢掉时发。
    Armed,
    /// 发过了。
    Sent,
    /// 回应到了，不用发。
    Done,
}

/// 发 `tool.cancel` 的：叫它停时发；回应没到就不等了的（超时、这次调用的 future 被丢掉）丢掉它时发。
struct Cancel {
    peer: Peer,
    params: Value,
    state: Sending,
}

impl Cancel {
    /// 还没发的发一次。
    fn send(&mut self) {
        if self.state == Sending::Armed {
            self.state = Sending::Sent;
            if !self.peer.notify("tool.cancel", self.params.clone()) {
                tracing::warn!(target: "miyu::endpoint", "tool.cancel not sent");
            }
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        self.send();
    }
}

/// 照模板 `template` 换进字段 `fields`；模板坏了的交回空的。
fn render(template: &str, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    Template::parse(template)
        .ok()
        .and_then(|template| template.render(&fields).ok())
        .unwrap_or_default()
}

/// 出错，交回一段字。
fn failed(text: String) -> Done {
    Done {
        error: true,
        blocks: vec![Block::Text(Text { text })],
        ..Done::ok("")
    }
}
