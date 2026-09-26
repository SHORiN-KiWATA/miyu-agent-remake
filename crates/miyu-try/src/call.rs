//! 请求模型（`docs/construction/3-5-试玩台（补）.md`）：驱动编码好，交给 HTTP 执行器在另一个任务里发；
//! 「发出去了」、每一段增量、说完了，都带着 `seen` 送回会话那一队（`02-内核.md` 第四节的表）。
//!
//! 故意掐断（`/cut`）：读到这么多段思考或者回复就停下，照「读到一半断了」报可以重试的错，内核自己
//! 再来。只为实测接着说。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use miyu_drivers::{Call, Driver};
use miyu_http::{Attempt, Endpoint, Outcome, Progress, send};
use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Input;
use tokio::sync::{Notify, mpsc, oneshot};

use crate::now;

/// 故意掐断时报的原话。
const CUT_MESSAGE: &str = "cut on purpose by the trial bench (/cut)";

/// `/cut` 怎么写。
const CUT_USAGE: &str = "/cut [think|text] [第几段]";

/// 在哪里掐断、读到第几段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    /// 掐在思考里，还是回复里。
    pub within: Within,
    /// 那一块读到第几段增量就停。
    pub after: u32,
}

/// 掐在哪一块里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Within {
    /// 思考。
    Thinking,
    /// 回复。
    Text,
}

impl Cut {
    /// 认 `/cut` 这一行：`/cut` 掐在思考的第 30 段，`/cut text` 掐在回复的第 8 段，后面再写一个数
    /// 就是第几段。不是 `/cut` 开头的，没有；写法不对的，交回怎么写。
    pub fn parse(line: &str) -> Option<Result<Cut, &'static str>> {
        let mut words = line.split_whitespace();
        if words.next() != Some("/cut") {
            return None;
        }
        let mut cut = Cut {
            within: Within::Thinking,
            after: 30,
        };
        for word in words {
            match word {
                "think" => cut.within = Within::Thinking,
                "text" => {
                    cut.within = Within::Text;
                    cut.after = 8;
                }
                number => match number.parse::<u32>() {
                    Ok(after) if after > 0 => cut.after = after,
                    _ => return Some(Err(CUT_USAGE)),
                },
            }
        }
        Some(Ok(cut))
    }

    /// 写给人看：在哪里掐断。
    pub fn describe(&self) -> String {
        let within = match self.within {
            Within::Thinking => "思考",
            Within::Text => "回复",
        };
        format!("在{within}的第 {} 段掐断", self.after)
    }
}

/// 数到了该掐断的那一段没有。
#[derive(Debug)]
struct Counter {
    cut: Cut,
    kinds: BTreeMap<usize, Kind>,
    counted: u32,
}

impl Counter {
    /// 看一段增量：是要掐的那种块里的一段字，数一下；刚好数到的，交回真。
    fn reached(&mut self, delta: &Delta) -> bool {
        match delta {
            Delta::Start { index, kind } => {
                self.kinds.insert(*index, kind.clone());
                false
            }
            Delta::Text { index, .. } => {
                let target = match self.cut.within {
                    Within::Thinking => Kind::Reasoning,
                    Within::Text => Kind::Text,
                };
                if self.kinds.get(index) != Some(&target) {
                    return false;
                }
                self.counted += 1;
                self.counted == self.cut.after
            }
            Delta::Private { .. } | Delta::End { .. } => false,
        }
    }
}

/// 发请求要的，一个会话一份。
pub struct Caller {
    /// HTTP 客户端，连接跨请求复用。
    pub client: reqwest::Client,
    /// 发给谁。
    pub endpoint: Arc<Endpoint>,
    /// 哪一家的接口。
    pub driver: Arc<dyn Driver>,
    /// 用哪个模型、输出上限。
    pub call: Call,
    /// 记进 `model.called` 的供应商和模型。
    pub model: Model,
    /// 多久没收到新的字节就算断了。
    pub idle: Duration,
}

impl Caller {
    /// 编码 `request`，在另一个任务里发；回报送进 `inputs`。交回叫停它用的那一头：送一下就停，
    /// 之后什么都不报。
    ///
    /// # Errors
    ///
    /// 编码不成：交回这次请求怎么没成的，执行器照「没发出去就失败了」报。
    pub fn call(
        &self,
        seen: Seq,
        request: &Request,
        cut: Option<Cut>,
        inputs: mpsc::UnboundedSender<Input>,
    ) -> Result<oneshot::Sender<()>, CallError> {
        let encoded = self
            .driver
            .encode(request, &self.call, &BTreeMap::new())
            .map_err(|error| CallError {
                class: ErrorClass::Unclassified,
                message: error.to_string(),
            })?;
        let (stop, stopped) = oneshot::channel::<()>();
        let client = self.client.clone();
        let endpoint = Arc::clone(&self.endpoint);
        let driver = Arc::clone(&self.driver);
        let model = self.model.clone();
        let idle = self.idle;
        tokio::spawn(async move {
            let cutter = Arc::new(Notify::new());
            let cancelled = {
                let cutter = Arc::clone(&cutter);
                async move {
                    tokio::select! {
                        _ = stopped => {}
                        () = cutter.notified() => {}
                    }
                }
            };
            let mut counter = cut.map(|cut| Counter {
                cut,
                kinds: BTreeMap::new(),
                counted: 0,
            });
            let mut was_cut = false;
            let attempt = Attempt {
                client: &client,
                endpoint: &endpoint,
                driver: &*driver,
                body: &encoded.body,
                idle,
            };
            let outcome = send(attempt, cancelled, |progress| match progress {
                Progress::Sent { request } => report(
                    &inputs,
                    Input::RequestSent {
                        at: now(),
                        seen,
                        model: model.clone(),
                        request,
                    },
                ),
                Progress::Delta(delta) => {
                    if counter
                        .as_mut()
                        .is_some_and(|counter| counter.reached(&delta))
                    {
                        was_cut = true;
                        cutter.notify_one();
                    }
                    report(
                        &inputs,
                        Input::ModelDelta {
                            at: now(),
                            seen,
                            delta,
                        },
                    );
                }
            })
            .await;
            let ended = match outcome {
                Outcome::Ended { usage, error } => Input::ModelEnded {
                    at: now(),
                    seen,
                    usage,
                    wait_ms: error.as_ref().and_then(|error| error.retry_after_ms),
                    error: error.map(|error| error.error),
                },
                Outcome::Cancelled if was_cut => Input::ModelEnded {
                    at: now(),
                    seen,
                    usage: None,
                    error: Some(CallError {
                        class: ErrorClass::Retryable,
                        message: CUT_MESSAGE.to_string(),
                    }),
                    wait_ms: None,
                },
                // 内核叫停的：之后什么都不报。
                Outcome::Cancelled => return,
            };
            report(&inputs, ended);
        });
        Ok(stop)
    }
}

/// 送回会话那一队。那一头已经退出了的，回报没人要，丢下不要紧。
pub fn report(inputs: &mpsc::UnboundedSender<Input>, input: Input) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "会话那一头退出了，回报没人要，丢下不要紧"
    )]
    let _ = inputs.send(input);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_lines_are_read() {
        let think = |after| Cut {
            within: Within::Thinking,
            after,
        };
        assert_eq!(Cut::parse("/cut"), Some(Ok(think(30))));
        assert_eq!(Cut::parse("/cut 50"), Some(Ok(think(50))));
        assert_eq!(
            Cut::parse("/cut text"),
            Some(Ok(Cut {
                within: Within::Text,
                after: 8
            }))
        );
        assert_eq!(
            Cut::parse("/cut text 3"),
            Some(Ok(Cut {
                within: Within::Text,
                after: 3
            }))
        );
        assert!(matches!(Cut::parse("/cut 0"), Some(Err(_))));
        assert!(matches!(Cut::parse("/cut later"), Some(Err(_))));
        assert_eq!(Cut::parse("/cutx"), None);
        assert_eq!(Cut::parse("你好"), None);
    }

    #[test]
    fn the_counter_counts_only_the_kind_it_cuts() {
        let mut counter = Counter {
            cut: Cut {
                within: Within::Text,
                after: 2,
            },
            kinds: BTreeMap::new(),
            counted: 0,
        };
        let text = |index: usize| Delta::Text {
            index,
            text: "x".to_string(),
        };
        assert!(!counter.reached(&Delta::Start {
            index: 0,
            kind: Kind::Reasoning
        }));
        assert!(!counter.reached(&text(0)));
        assert!(!counter.reached(&text(0)), "思考里的不数");
        assert!(!counter.reached(&Delta::Start {
            index: 1,
            kind: Kind::Text
        }));
        assert!(!counter.reached(&text(1)));
        assert!(counter.reached(&text(1)), "回复的第 2 段");
        assert!(!counter.reached(&text(1)), "只报一次");
    }
}
