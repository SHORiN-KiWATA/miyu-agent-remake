//! 发一次请求（`05-内核接口.md` 第七节「HTTP 执行器」）：发、读、空闲超时、出错、打断。
//!
//! 1. 先报「发出去了」，带上请求字节的哈希；再 `POST <地址><路径>`，请求体一个字节不改。
//! 2. 不是 2xx 的，读最多 64 KiB 的响应体，连同状态、响应头交给驱动分类。
//! 3. 2xx 的，一片一片地读，每一片都套上空闲超时，交给解码器，解出来的增量马上交出去；解码器
//!    说不用再读了就停。读完了（或者读到一半断了）由解码器收尾：它知道说没说完。
//! 4. 打断：`cancel` 一完成就停，丢掉连接，交回 [`Outcome::Cancelled`]，不再报任何东西。

use std::error::Error;
use std::future::Future;
use std::time::Duration;

use miyu_drivers::Driver;
use miyu_drivers::classify::{Classified, Failure};
use miyu_kernel::accumulate::Delta;
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::ContentHash;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use tokio::time::timeout;

use crate::Endpoint;

/// 出错时的响应体最多读多少：分类用不着那么多，原话本来也只留 2000 字节。
const ERROR_BODY_LIMIT: usize = 64 * 1024;

/// 发一次请求要的。
#[derive(Clone, Copy)]
pub struct Attempt<'a> {
    /// HTTP 客户端，连接跨请求复用。
    pub client: &'a reqwest::Client,
    /// 发给谁。
    pub endpoint: &'a Endpoint,
    /// 哪一家的接口：路径、解码器、分类。
    pub driver: &'a dyn Driver,
    /// 驱动编码好的请求字节。
    pub body: &'a [u8],
    /// 多久没收到新的字节就算断了。
    pub idle: Duration,
}

/// 读的过程中交出去的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// 发出去了：请求字节的哈希，记进 `model.called`。
    Sent {
        /// 请求字节的 SHA-256。
        request: ContentHash,
    },
    /// 解出来的一段增量。
    Delta(Delta),
}

/// 这一次请求怎么收场的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 说完了，或者出错了。
    Ended {
        /// 用量。供应商没报的，没有。
        usage: Option<Usage>,
        /// 出错的分类、原话、要等多久；正常说完的，没有。
        error: Option<Classified>,
    },
    /// 被叫停了：之后什么都不报。
    Cancelled,
}

/// 发一次请求，读到说完、出错或者被叫停。增量和「发出去了」一有就交给 `on`。
pub async fn send(
    attempt: Attempt<'_>,
    cancel: impl Future<Output = ()> + Send,
    mut on: impl FnMut(Progress) + Send,
) -> Outcome {
    tokio::pin!(cancel);
    let url = format!(
        "{}{}",
        attempt.endpoint.base_url.trim_end_matches('/'),
        attempt.driver.path()
    );
    let mut request = attempt
        .client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {}", attempt.endpoint.key()))
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "text/event-stream")
        .body(attempt.body.to_vec());
    for (name, value) in &attempt.endpoint.headers {
        request = request.header(name.as_str(), value.as_str());
    }
    on(Progress::Sent {
        request: ContentHash::of(attempt.body),
    });
    let waited = tokio::select! {
        biased;
        () = &mut cancel => return Outcome::Cancelled,
        waited = timeout(attempt.idle, request.send()) => waited,
    };
    let mut response = match waited {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return failed(attempt.driver, &chain(&error)),
        Err(_) => return idle(attempt.idle),
    };
    let status = response.status();
    if !status.is_success() {
        let headers = headers(response.headers());
        let mut body = Vec::new();
        while body.len() < ERROR_BODY_LIMIT {
            let next = tokio::select! {
                biased;
                () = &mut cancel => return Outcome::Cancelled,
                next = timeout(attempt.idle, response.chunk()) => next,
            };
            match next {
                Ok(Ok(Some(bytes))) => body.extend_from_slice(&bytes),
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
            }
        }
        body.truncate(ERROR_BODY_LIMIT);
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let classified = attempt.driver.classify(&Failure {
            status: Some(status.as_u16()),
            headers: &pairs,
            body: &body,
        });
        return Outcome::Ended {
            usage: None,
            error: Some(classified),
        };
    }
    let mut decoder = attempt.driver.decoder();
    let mut broken = None;
    loop {
        let next = tokio::select! {
            biased;
            () = &mut cancel => return Outcome::Cancelled,
            next = timeout(attempt.idle, response.chunk()) => next,
        };
        match next {
            Ok(Ok(Some(bytes))) => {
                for delta in decoder.feed(&bytes) {
                    on(Progress::Delta(delta));
                }
                if decoder.done() {
                    break;
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(error)) => {
                broken = Some(chain(&error));
                break;
            }
            Err(_) => return idle(attempt.idle),
        }
    }
    let ending = decoder.finish();
    let error = match (ending.error, broken) {
        // 读到一半断了、又没说完：原话写连接怎么断的，比「流断了」有用。
        (Some(error), Some(broken)) if error.class == ErrorClass::Retryable => Some(CallError {
            class: ErrorClass::Retryable,
            message: format!("连接断了：{broken}"),
        }),
        (error, _) => error,
    };
    if error.is_none() {
        for delta in ending.deltas {
            on(Progress::Delta(delta));
        }
    }
    Outcome::Ended {
        usage: ending.usage,
        error: error.map(|error| Classified {
            error,
            retry_after_ms: None,
        }),
    }
}

/// 连不上、发不出去：没有状态，交给驱动分类（可重试）。
fn failed(driver: &dyn Driver, why: &str) -> Outcome {
    Outcome::Ended {
        usage: None,
        error: Some(driver.classify(&Failure {
            status: None,
            headers: &[],
            body: why.as_bytes(),
        })),
    }
}

/// 空闲超时：多久没收到新的字节，算可重试的错。
fn idle(idle: Duration) -> Outcome {
    Outcome::Ended {
        usage: None,
        error: Some(Classified {
            error: CallError {
                class: ErrorClass::Retryable,
                message: format!("空闲超时：{} 秒没有收到新的内容", idle.as_secs_f64()),
            },
            retry_after_ms: None,
        }),
    }
}

/// 一个错误连同它的来由，一层一层接起来：reqwest 的最外层只说「发请求出错」，里面才有
/// 「连接被拒绝」。
fn chain(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}

/// 响应头写成字符串对；不是 UTF-8 的值照替换字符写。
fn headers(map: &HeaderMap) -> Vec<(String, String)> {
    map.iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect()
}
