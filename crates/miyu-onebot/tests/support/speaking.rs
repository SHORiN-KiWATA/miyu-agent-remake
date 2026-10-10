//! 她照台词说（施工 O-25 上，出站链的测试用）：会话的端口照一句句台词回，一句是一次回复。和剧本（`Script`）不同的两样：
//!
//! - 一次回复里可以既说一句、又调一件工具（[`Line::calls`]）：内核答完工具再请求一次，同一回合里她就说了两句，测得到去重。
//!   调的是不存在的 [`NO_TOOL`]：内核当场答「没有这件工具」，接着请求（`02-内核.md`），什么都不用装。
//! - 可以等测试放行再说（[`Line::released_by`]）：引用、@ 看「她回的那条之后群里来了几条」「过了多久」，先压着，群里说完了、
//!   等够了再放；去重看「这一回合已经发出去的」（O-25 中起照入队算，桥入队记成了就算上，`onebot.md`「施工时定的」第 112 条）。
//!   不靠谁快。
//!
//! 起标题这类辅助请求不回：在路上不碍事。每一次主请求记下来（[`Lines::requests`]，施工 O-25 下：看退信那一块进没进她下一次
//! 请求）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::oneshot;

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::event::Usage;
use miyu_kernel::id::{ModelName, ProviderId, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports, TurnConfig};

/// 她调的工具：不存在。
pub const NO_TOOL: &str = "no_such_tool";

/// 一句台词：一次回复。
pub struct Line {
    /// 说的字。
    text: &'static str,
    /// 说完接着调 [`NO_TOOL`]。
    call: bool,
    /// 等它放行再说；放行的一头放下了也照说，不卡住。
    release: Option<oneshot::Receiver<()>>,
}

impl Line {
    /// 说一句，说完：这一轮完了。
    pub fn says(text: &'static str) -> Line {
        Line {
            text,
            call: false,
            release: None,
        }
    }

    /// 说一句，同一次回复里调一件不存在的工具：这一轮接着往下走。
    pub fn calls(text: &'static str) -> Line {
        Line {
            call: true,
            ..Line::says(text)
        }
    }

    /// 同一句，等 `release` 放行了再说。
    pub fn released_by(mut self, release: oneshot::Receiver<()>) -> Line {
        self.release = Some(release);
        self
    }
}

/// 照台词回的端口：几个会话共用一份台词，照请求的先后一句句拿。
#[derive(Clone)]
pub struct Lines {
    lines: Arc<Mutex<VecDeque<Line>>>,
    /// 交来的每一次主请求，照先后。
    seen: Arc<Mutex<Vec<Request>>>,
}

impl Lines {
    /// 照 `lines` 一句句回。
    pub fn new(lines: impl IntoIterator<Item = Line>) -> Lines {
        Lines {
            lines: Arc::new(Mutex::new(lines.into_iter().collect())),
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 交来的每一次主请求，照先后（起标题这类辅助请求不算）。
    pub fn requests(&self) -> Vec<Request> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Models for Lines {
    fn port(&self, _session: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Lines {
    fn model(&self) -> Model {
        Model {
            endpoint: ProviderId::parse("deepseek").expect("端点合写法"),
            model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
        }
    }

    fn call(
        &self,
        _seen: Seq,
        request: Request,
        _config: &TurnConfig,
        reports: Reports,
        _cancel: Cancel,
    ) {
        if reports.purpose().is_some() {
            return;
        }
        let line = self
            .lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .expect("台词里排了这一次说什么");
        let (model, hash) = (self.model(), request.hash());
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
        tokio::spawn(async move {
            if let Some(release) = line.release
                && release.await.is_err()
            {
                // 放行的一头放下了：照说，不卡住。
            }
            reports.sent(model, hash);
            let mut deltas = vec![
                Delta::Start {
                    index: 0,
                    kind: Kind::Text,
                },
                Delta::Text {
                    index: 0,
                    text: line.text.to_string(),
                },
            ];
            let mut ends = vec![Delta::End { index: 0 }];
            if line.call {
                let kind = Kind::ToolCall {
                    name: NO_TOOL.to_string(),
                };
                deltas.push(Delta::Start { index: 1, kind });
                deltas.push(Delta::Text {
                    index: 1,
                    text: "{}".to_string(),
                });
                ends.push(Delta::End { index: 1 });
            }
            // 几块都等流完了才一起收，和驱动一样。
            for delta in deltas.into_iter().chain(ends) {
                reports.delta(delta);
            }
            let usage = Usage {
                uncached: 60,
                cache_read: 40,
                cache_write: 0,
                output: 10,
                reasoning: None,
            };
            reports.ended(Some(usage), None, None, None);
        });
    }
}
