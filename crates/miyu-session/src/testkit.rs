//! 测试用的请求模型的端口（施工 3-7 中写的，施工 3-8 上挪到这里）：照剧本回，记下交给它的每一次请求和
//! 被叫停的请求。它自己也造端口（[`Models`]），造出来的和手里这一份共用剧本和记录。
//!
//! 只在 `testkit` 开关打开时编进去：会话自己的测试、上层 crate（协议端点）的测试都用它，在各自的
//! dev-dependencies 里打开（照内核执行器替身的做法，施工 2-9）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::{ModelName, ProviderId, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;

use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};

/// 剧本里的一次回复。
#[derive(Debug, Clone)]
pub enum Play {
    /// 说一句，说完。用量：60 没命中、40 命中、10 输出。
    Says(&'static str),
    /// 出错：分类，供应商说要等多久。
    Fails {
        /// 出错的分类。
        class: ErrorClass,
        /// 供应商说要等多久，毫秒。
        wait_ms: Option<u64>,
    },
    /// 开了个头就停住，等叫停。
    Holds,
    /// 端口自己的 bug：一叫它就 panic。
    Panics,
}

/// 照剧本回的请求模型的端口。它自己也造端口：造出来的和手里这一份共用剧本和记录。
#[derive(Clone)]
pub struct Script {
    model: Model,
    plays: Arc<Mutex<VecDeque<Play>>>,
    requests: Arc<Mutex<Vec<(Seq, Request)>>>,
    cancelled: Arc<Mutex<Vec<Seq>>>,
}

impl Script {
    /// 照 `plays` 一次次回：deepseek 的 deepseek-v4。
    ///
    /// # Panics
    ///
    /// 实际不会：端点、模型名都合写法。
    pub fn new(plays: impl IntoIterator<Item = Play>) -> Script {
        Script {
            model: Model {
                endpoint: ProviderId::parse("deepseek").expect("端点合写法"),
                model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
            },
            plays: Arc::new(Mutex::new(plays.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 交给它的每一次请求，照先后：看到了第几条为止，和请求本身。
    pub fn requests(&self) -> Vec<(Seq, Request)> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 被叫停的请求，照先后。
    pub fn cancelled(&self) -> Vec<Seq> {
        self.cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Models for Script {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Script {
    fn model(&self) -> &Model {
        &self.model
    }

    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel) {
        let hash = request.hash();
        let asked = {
            let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
            requests.push((seen, request));
            requests.len()
        };
        let play = self
            .plays
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| panic!("剧本里没排第 {asked} 次请求说什么"));
        if matches!(play, Play::Panics) {
            panic!("端口自己的 bug");
        }
        let model = self.model.clone();
        let cancelled = Arc::clone(&self.cancelled);
        tokio::spawn(async move {
            reports.sent(model, hash);
            match play {
                Play::Says(text) => {
                    for delta in text_block(text, true) {
                        reports.delta(delta);
                    }
                    reports.ended(
                        Some(Usage {
                            uncached: 60,
                            cache_read: 40,
                            cache_write: 0,
                            output: 10,
                        }),
                        None,
                        None,
                    );
                }
                Play::Fails { class, wait_ms } => reports.ended(
                    None,
                    Some(CallError {
                        class,
                        message: "HTTP 429: slow down".to_string(),
                    }),
                    wait_ms,
                ),
                Play::Holds => {
                    for delta in text_block("…", false) {
                        reports.delta(delta);
                    }
                    cancel.wait().await;
                    cancelled
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(seen);
                }
                Play::Panics => unreachable!("上面已经 panic 了"),
            }
        });
    }
}

/// 一块正文：开始、全文；`ends` 的再收全。
fn text_block(text: &str, ends: bool) -> Vec<Delta> {
    let mut deltas = vec![
        Delta::Start {
            index: 0,
            kind: Kind::Text,
        },
        Delta::Text {
            index: 0,
            text: text.to_string(),
        },
    ];
    if ends {
        deltas.push(Delta::End { index: 0 });
    }
    deltas
}
