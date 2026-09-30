//! 几个会话共用的替身模型（施工 7-9，`agents.rs` 用）：主会话、子会话同时请求模型，谁先到不一定，照请求里人这边的那句话分给各自的剧本；
//! 带闸的剧本，测试放行一次才回一次，子代理什么时候回报由测试定。

use std::sync::Arc;

use tokio::sync::Semaphore;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Purpose;
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};

/// 一份剧本：请求里人这边有 `key` 这一句的归它；有闸的等放行。
struct Route {
    key: &'static str,
    script: Script,
    gate: Option<Arc<Semaphore>>,
}

/// 照请求分给各自剧本的替身模型。
#[derive(Clone)]
pub struct Router(Arc<Vec<Route>>);

/// 一道闸：放行一次，那份剧本回一次。
#[derive(Clone)]
pub struct Gate(Arc<Semaphore>);

impl Gate {
    /// 放行一次。
    pub fn open(&self) {
        self.0.add_permits(1);
    }
}

impl Router {
    /// 主会话的剧本：人说的那一句是 `key`，一问就回。
    pub fn new(key: &'static str, plays: impl IntoIterator<Item = Play>) -> Router {
        Router(Arc::new(vec![Route {
            key,
            script: Script::new(plays),
            gate: None,
        }]))
    }

    /// 加一份带闸的剧本：交代是 `key` 的子代理，交回它的闸。
    pub fn gated(&mut self, key: &'static str, plays: impl IntoIterator<Item = Play>) -> Gate {
        let gate = Arc::new(Semaphore::new(0));
        Arc::get_mut(&mut self.0).expect("还没交出去").push(Route {
            key,
            script: Script::new(plays),
            gate: Some(Arc::clone(&gate)),
        });
        Gate(gate)
    }

    fn route(&self, request: &Request) -> &Route {
        let said = |key: &str| {
            request.messages.iter().any(|message| match message {
                Message::User { blocks, .. } => blocks.contains(&Block::Text(Text {
                    text: key.to_string(),
                })),
                _ => false,
            })
        };
        self.0
            .iter()
            .find(|route| said(route.key))
            .unwrap_or_else(|| panic!("没有哪份剧本认这次请求"))
    }
}

impl Models for Router {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Router {
    fn model(&self) -> &Model {
        self.0[0].script.model()
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &miyu_session::TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        // 起标题的请求（施工 3-8 五补）不带哪一份的原话：不回，一直在路上，不碍这里测的。
        if reports.purpose() == Some(&Purpose::Title) {
            return;
        }
        let route = self.route(&request);
        let script = route.script.clone();
        match route.gate.clone() {
            None => script.call(seen, request, config, reports, cancel),
            Some(gate) => {
                let config = std::sync::Arc::clone(config);
                tokio::spawn(async move {
                    // 闸不会关：拿不到就是测试结束了，这一次也不用回。
                    if let Ok(permit) = gate.acquire().await {
                        permit.forget();
                        script.call(seen, request, &config, reports, cancel);
                    }
                });
            }
        }
    }
}
