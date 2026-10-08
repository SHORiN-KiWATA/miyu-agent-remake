//! 下架的模型移出池（施工 8-23，`docs/construction/8-23-下架的模型移出池（要补）.md`）：路由确认一个池的成员下架了，交到这里
//! （[`miyu_session::Retirement`]），系统配置、个人设置里写着它的池（`pools.<名字>.models`）都拿掉它，经写配置的那一条路写
//! （手改保护、推 `config.changed`、记日志），改的人记成内核。删空了的池留着。直接写在 `models.chat` 的不动。

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use serde_json::{Value as Json, json};

use miyu_config::{Layer, Value};
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;
use miyu_session::Retirement;

use crate::Core;
use crate::config::set::{SetParams, set_by};
use crate::hello::Peer;
use crate::refusal::Locale;

/// 运行日志的目标。
const TARGET: &str = "miyu::models";

/// 池的成员那一项的样子。
const MEMBERS: &str = "pools.<id>.models";

/// 核心的那一头：核心没了的什么都不做。
struct Retiring(Weak<Core>);

impl Core {
    /// 核心起来时调一次（施工 8-23）：模型资料里装上下架的模型的端口，路由确认了的从池里拿掉。
    pub fn start_retirement(self: &Arc<Self>) {
        self.model_data
            .on_retirement(Arc::new(Retiring(Arc::downgrade(self))));
    }
}

impl Retirement for Retiring {
    fn concluded(&self, provider: &str, model: &str, gone: bool) {
        let Some(core) = self.0.upgrade() else {
            return;
        };
        if gone {
            retire(&core, &format!("{provider}/{model}"));
        }
    }
}

/// 从两层里写着 `reference` 的池拿掉它：一层一次写。
fn retire(core: &Core, reference: &str) {
    let mut by_layer: BTreeMap<&'static str, Vec<Json>> = BTreeMap::new();
    let mut pools = Vec::new();
    {
        let config = core.config();
        for (layer, name) in [(Layer::System, "system"), (Layer::Personal, "personal")] {
            for (key, entry) in &config.file(layer).parsed.entries {
                let Value::List(members) = &entry.value else {
                    continue;
                };
                if entry.item != MEMBERS
                    || !members.iter().any(|member| text(member) == Some(reference))
                {
                    continue;
                }
                let kept: Vec<&str> = members
                    .iter()
                    .filter_map(text)
                    .filter(|member| *member != reference)
                    .collect();
                by_layer
                    .entry(name)
                    .or_default()
                    .push(json!({"key": key, "value": kept}));
                pools.push(key.clone());
            }
        }
    }
    if by_layer.is_empty() {
        return;
    }
    tracing::info!(target: TARGET, model = reference, pools = pools.join(",").as_str(), "model retired");
    let peer = Peer {
        locale: Locale::En,
        language: "en",
        input: false,
    };
    for (layer, changes) in by_layer {
        let written =
            serde_json::from_value::<SetParams>(json!({"layer": layer, "changes": changes}))
                .map_err(|_| crate::refusal::Refusal::INTERNAL)
                .and_then(|params| {
                    let cause = CommandId::parse(&format!("retire:{reference}"))
                        .map_err(|_| crate::refusal::Refusal::INTERNAL)?;
                    set_by(core, peer, &cause, params, By::Kernel)
                });
        if let Err(refusal) = written {
            tracing::warn!(target: TARGET, model = reference, layer, reason = refusal.reason, "model not retired");
        }
    }
}

/// 一个成员的字；不是字的没有。
fn text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text),
        _ => None,
    }
}
