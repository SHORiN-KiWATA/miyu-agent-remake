//! 测试用的请求模型的端口（施工 3-7 中写的，施工 3-8 上挪到这里）：照剧本回，记下交给它的每一次请求和
//! 被叫停的请求。它自己也造端口（[`Models`]），造出来的和手里这一份共用剧本和记录。
//!
//! 只在 `testkit` 开关打开时编进去：会话自己的测试、上层 crate（协议端点）的测试都用它，在各自的
//! dev-dependencies 里打开（照内核执行器替身的做法，施工 2-9）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::event::{CallError, ErrorClass, Purpose, Usage};
use miyu_kernel::id::{ModelName, ProviderId, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;

use crate::config::TurnConfig;
use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};

/// 剧本里的一次回复。
#[derive(Debug, Clone)]
pub enum Play {
    /// 说一句，说完。用量：60 没命中、40 命中、10 输出。
    Says(&'static str),
    /// 先想 `thinking`，再说 `text`，说完。用量同 [`Play::Says`]（施工 3-9 下）。增量的先后照驱动真实的：
    /// 两块都等流完了才一起收（`samples/drivers/openai-chat/streams/deepseek-reasoning-tools.txt`）。
    Thinks {
        /// 思考。
        thinking: &'static str,
        /// 回答。
        text: &'static str,
    },
    /// 一口气推 `n` 段增量，每段一个字，再说完：好测读得慢的订阅者掉队。
    Floods(usize),
    /// 调这几件工具，每件写工具名和参数原文，照先后放在一次回复里（施工 4-2）。块都等流完了才一起收，
    /// 和驱动一样。用量同 [`Play::Says`]。用 [`Play::calls`] 造。
    Calls(Vec<(String, String)>),
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

impl Play {
    /// 调这几件工具：工具名和参数原文（施工 4-2）。
    pub fn calls(calls: &[(&str, &str)]) -> Play {
        Play::Calls(
            calls
                .iter()
                .map(|(name, args)| ((*name).to_string(), (*args).to_string()))
                .collect(),
        )
    }
}

/// 照剧本回的请求模型的端口。它自己也造端口：造出来的和手里这一份共用剧本和记录。
#[derive(Clone)]
pub struct Script {
    model: Model,
    plays: Arc<Mutex<VecDeque<Play>>>,
    requests: Arc<Mutex<Vec<(Seq, Request)>>>,
    /// 每一次主请求交来的这一轮的配置，和 `requests` 一一对上（施工 8-4）。
    configs: Arc<Mutex<Vec<TurnConfig>>>,
    cancelled: Arc<Mutex<Vec<Seq>>>,
    /// 交给内核的窗口；没有的不主动压（施工 6-3 上）。
    window: Option<u64>,
    /// 起标题的请求另排的剧本，和交来的起标题的请求（施工 3-8 五补）：它在每一轮答完以后自己来，不占主剧本、不算在
    /// [`Script::requests`] 里；没排的只记下、不回（一直在路上），不管标题的测试不用替它排。
    titles: Arc<Mutex<VecDeque<Play>>>,
    titled: Arc<Mutex<Vec<(Seq, Request)>>>,
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
            configs: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(Vec::new())),
            window: None,
            titles: Arc::new(Mutex::new(VecDeque::new())),
            titled: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 起标题的请求照先后这样回（施工 3-8 五补）。
    #[must_use]
    pub fn titles(self, plays: impl IntoIterator<Item = Play>) -> Script {
        lock(&self.titles).extend(plays);
        self
    }

    /// 交来的起标题的请求，照先后：照到第几条为止，和请求本身。
    pub fn titled(&self) -> Vec<(Seq, Request)> {
        lock(&self.titled).clone()
    }

    /// 同一份剧本，模型的窗口是 `window`：会话照它算压缩线（施工 6-3 上）。
    #[must_use]
    pub fn window(mut self, window: u64) -> Script {
        self.window = Some(window);
        self
    }

    /// 交给它的每一次请求，照先后：看到了第几条为止，和请求本身。
    pub fn requests(&self) -> Vec<(Seq, Request)> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 每一次主请求交来的这一轮的配置，照先后，和 [`Script::requests`] 一一对上（施工 8-4）。
    pub fn configs(&self) -> Vec<TurnConfig> {
        lock(&self.configs).clone()
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
    fn model(&self) -> Model {
        self.model.clone()
    }

    fn limits(&self) -> Limits {
        Limits {
            model: self.model.clone(),
            window: self.window,
            max_output: None,
            images: None,
        }
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        let hash = request.hash();
        let play = if reports.purpose() == Some(&Purpose::Title) {
            lock(&self.titled).push((seen, request));
            match lock(&self.titles).pop_front() {
                Some(play) => play,
                None => return,
            }
        } else {
            let asked = {
                let mut requests = lock(&self.requests);
                requests.push((seen, request));
                lock(&self.configs).push(Arc::clone(config));
                requests.len()
            };
            lock(&self.plays)
                .pop_front()
                .unwrap_or_else(|| panic!("剧本里没排第 {asked} 次请求说什么"))
        };
        if matches!(play, Play::Panics) {
            panic!("端口自己的 bug");
        }
        let model = self.model.clone();
        let cancelled = Arc::clone(&self.cancelled);
        tokio::spawn(async move {
            reports.sent(model, hash);
            match play {
                Play::Says(text) => says(reports, text, 1),
                Play::Thinks { thinking, text } => {
                    let mut thought = block(0, Kind::Reasoning, thinking, 2);
                    let mut said = block(1, Kind::Text, text, 2);
                    // 两块都等流完了才一起收，和驱动一样。
                    let ends = [thought.pop(), said.pop()];
                    for delta in thought
                        .into_iter()
                        .chain(said)
                        .chain(ends.into_iter().flatten())
                    {
                        reports.delta(delta);
                    }
                    reports.ended(Some(usage()), None, None, None);
                }
                Play::Floods(n) => says(reports, &"字".repeat(n), n),
                Play::Calls(calls) => {
                    let mut ends = Vec::new();
                    for (index, (name, args)) in calls.iter().enumerate() {
                        let kind = Kind::ToolCall { name: name.clone() };
                        let mut deltas = block(index, kind, args, 1);
                        ends.extend(deltas.pop());
                        for delta in deltas {
                            reports.delta(delta);
                        }
                    }
                    for end in ends {
                        reports.delta(end);
                    }
                    reports.ended(Some(usage()), None, None, None);
                }
                Play::Fails { class, wait_ms } => reports.ended(
                    None,
                    Some(CallError {
                        class,
                        message: "HTTP 429: slow down".to_string(),
                        status: None,
                    }),
                    wait_ms,
                    None,
                ),
                Play::Holds => {
                    for delta in text_block("…", 1).into_iter().take(2) {
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

/// 拿锁：拿着锁的线程 panic 了，照样拿（剧本、记录只是测试的记账）。
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 说完一句：正文分成 `pieces` 段交出去，报用量：60 没命中、40 命中、10 输出。
fn says(reports: Reports, text: &str, pieces: usize) {
    for delta in text_block(text, pieces) {
        reports.delta(delta);
    }
    reports.ended(Some(usage()), None, None, None);
}

/// 剧本报的用量：60 没命中、40 命中、10 输出。
fn usage() -> Usage {
    Usage {
        uncached: 60,
        cache_read: 40,
        cache_write: 0,
        output: 10,
    }
}

/// 一块正文：开始，全文分成 `pieces` 段（按字切），收全。
fn text_block(text: &str, pieces: usize) -> Vec<Delta> {
    block(0, Kind::Text, text, pieces)
}

/// 第 `index` 块，种类是 `kind`：开始，全文分成 `pieces` 段（按字切），收全。
fn block(index: usize, kind: Kind, text: &str, pieces: usize) -> Vec<Delta> {
    let chars: Vec<char> = text.chars().collect();
    let size = chars.len().div_ceil(pieces.max(1)).max(1);
    let mut deltas = vec![Delta::Start { index, kind }];
    for piece in chars.chunks(size) {
        deltas.push(Delta::Text {
            index,
            text: piece.iter().collect(),
        });
    }
    deltas.push(Delta::End { index });
    deltas
}
