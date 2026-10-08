//! 提供者的一件工具（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 3 条）：执行时照包查提供者表，没有连接的交回「暂时
//! 不可用」；有的，反向调用 `tool.call {session, call_id, tool, args, cwd}`，照回应 `{blocks, error}` 交回结果。回应是错误的、
//! 写法不对的，算出错，原话写进结果；等着时连接断了，也是暂时不可用。叫停时丢掉等待（O-2 下发 `tool.cancel`）。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool, Venues};

use super::Provided;

/// 提供者的一件工具。
pub(crate) struct RemoteTool {
    spec: Spec,
    venues: Venues,
    package: String,
    provided: Arc<Provided>,
    /// 暂时不可用那一句的模板（`core/tool-results/unavailable.txt`），字段 `name`。
    unavailable: String,
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
        spec: Spec,
        venues: Venues,
        package: &str,
        provided: Arc<Provided>,
        unavailable: String,
    ) -> RemoteTool {
        RemoteTool {
            spec,
            venues,
            package: package.to_string(),
            provided,
            unavailable,
        }
    }

    /// 暂时不可用：说法同执行器替目录里没有的工具写的那一句。
    fn unavailable(&self) -> Done {
        let name = self.spec.name.as_str();
        let text = Template::parse(&self.unavailable)
            .ok()
            .and_then(|template| template.render(&BTreeMap::from([("name", name)])).ok())
            .unwrap_or_default();
        Done {
            human: Some(Said::new("core/tool-results/unavailable").with("name", name)),
            ..failed(text)
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
            let params = json!({
                "session": ids.map(|ids| ids.session.as_str()),
                "call_id": ids.map(|ids| ids.call.to_string()),
                "tool": self.spec.name,
                "args": args,
                "cwd": call.cwd,
            });
            match peer.call("tool.call", params).await {
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
                Err(_) => self.unavailable(),
            }
        })
    }
}

/// 出错，交回一段字。
fn failed(text: String) -> Done {
    Done {
        error: true,
        blocks: vec![Block::Text(Text { text })],
        ..Done::ok("")
    }
}
