//! 出错分类（`docs/designs/05-内核接口.md` 第七节「出错怎么分」）：HTTP 状态、响应头、响应体，分成
//! 内核的六类，附带要等多久和原话。
//!
//! 照 opencode 的先后，先对上的算：上下文超长、被内容策略拦截、认证失败（额度用完也算）、限速、
//! 可重试、其他。说法的清单用 opencode、pi 收集的并集，全部小写以后找子串：纯逻辑层没有正则。

use miyu_kernel::event::{CallError, ErrorClass};
use serde_json::Value;

/// 出了错的一次请求：状态、响应头、响应体。流里报的错没有状态，也没有头。
#[derive(Debug, Clone, Copy)]
pub struct Failure<'a> {
    /// HTTP 状态；连接断了、流里报的错，没有。
    pub status: Option<u16>,
    /// 响应头，名字不分大小写。
    pub headers: &'a [(&'a str, &'a str)],
    /// 响应体。
    pub body: &'a [u8],
}

impl<'a> Failure<'a> {
    /// 流里报的错：只有一段 JSON。
    pub fn stream(body: &'a [u8]) -> Failure<'a> {
        Failure {
            status: None,
            headers: &[],
            body,
        }
    }
}

/// 分好的类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    /// 分类和原话，记进 `model.called`。
    pub error: CallError,
    /// 供应商说了要等多久，毫秒。没说的没有，由执行器退避。
    pub retry_after_ms: Option<u64>,
}

/// 原话最长多少字节：出错页可能是一整页 HTML。
pub const MESSAGE_LIMIT: usize = 2000;

/// 超长的错误码。
const OVERFLOW_CODES: [&str; 3] = [
    "context_length_exceeded",
    "model_context_window_exceeded",
    "request_too_large",
];

/// 超长的说法。
const OVERFLOW_PHRASES: [&str; 22] = [
    "prompt is too long",
    "prompt too long",
    "input is too long",
    "too large for model",
    "exceeds the context window",
    "context window exceeds",
    "maximum context length",
    "context length exceeded",
    "context_length_exceeded",
    "context length is only",
    "greater than the context length",
    "longer than the model",
    "exceeds the available context size",
    "the configured context size",
    "exceeded model token limit",
    "token limit exceeded",
    "too many tokens",
    "tokens in request more than max tokens allowed",
    "reduce the length of the messages",
    "maximum prompt length is",
    "maximum allowed input length",
    "range of input length should be",
];

/// 限速的说法：先排除它们，再认超长。
const RATE_PHRASES: [&str; 5] = [
    "rate limit",
    "rate_limit",
    "too many requests",
    "throttling",
    "service unavailable",
];

/// 内容策略的错误码。
const POLICY_CODES: [&str; 8] = [
    "content_filter",
    "responsibleaipolicyviolation",
    "content_policy_violation",
    "image_content_policy_violation",
    "refusal",
    "cyber_policy",
    "bio_policy",
    "misalignment_policy_violation",
];

/// 内容策略的说法。
const POLICY_PHRASES: [&str; 7] = [
    "violating our usage policy",
    "blocked by content filtering policy",
    "content policy",
    "content-policy",
    "content_policy",
    "contentpolicy",
    "rejected as a result of our safety system",
];

/// 额度用完的错误码和说法。
const QUOTA_PHRASES: [&str; 9] = [
    "insufficient_quota",
    "insufficient quota",
    "insufficient balance",
    "insufficient_balance",
    "exceeded your current quota",
    "quota exceeded",
    "billing_hard_limit_reached",
    "credit balance is too low",
    "usagelimiterror",
];

/// 分类。
pub fn classify(failure: &Failure<'_>) -> Classified {
    let raw = String::from_utf8_lossy(failure.body);
    let parsed: Option<Value> = serde_json::from_slice(failure.body).ok();
    let said = Said::read(parsed.as_ref(), &raw);
    let status = failure.status.or(said.status);
    let text = said.text.to_lowercase();
    let codes = [said.code.to_lowercase(), said.kind.to_lowercase()];
    let is_code = |list: &[&str]| codes.iter().any(|code| list.contains(&code.as_str()));
    let says = |list: &[&str]| list.iter().any(|phrase| text.contains(phrase));
    let client_side = status.is_none_or(|status| (400..500).contains(&status));
    let should_retry = header(failure.headers, "x-should-retry");
    let overflow = client_side
        && !says(&RATE_PHRASES)
        && (is_code(&OVERFLOW_CODES) || says(&OVERFLOW_PHRASES));
    let class = if overflow || status == Some(413) {
        ErrorClass::ContextTooLong
    } else if client_side && (is_code(&POLICY_CODES) || says(&POLICY_PHRASES)) {
        ErrorClass::ContentPolicy
    } else if status == Some(402) || says(&QUOTA_PHRASES) || matches!(status, Some(401 | 403)) {
        ErrorClass::Auth
    } else if status == Some(429) || (client_side && status.is_some() && says(&RATE_PHRASES)) {
        ErrorClass::RateLimited
    } else if let Some(retry) = should_retry {
        if retry.eq_ignore_ascii_case("true") {
            ErrorClass::Retryable
        } else {
            ErrorClass::Unclassified
        }
    } else {
        match status {
            None | Some(408 | 409 | 500..) => ErrorClass::Retryable,
            Some(_) => ErrorClass::Unclassified,
        }
    };
    let message = match failure.status {
        Some(status) => format!("HTTP {status}: {}", said.message),
        None => said.message,
    };
    Classified {
        error: CallError {
            class,
            message: clip(&message, MESSAGE_LIMIT).to_string(),
        },
        retry_after_ms: retry_after(failure.headers, &text),
    }
}

/// 响应体里说的：错误码、类型、原话，和找说法用的全部的字。
struct Said {
    code: String,
    kind: String,
    /// 响应体里的 `error.code` 是数字、又像 HTTP 状态的（流里报的错）。
    status: Option<u16>,
    message: String,
    text: String,
}

impl Said {
    fn read(parsed: Option<&Value>, raw: &str) -> Said {
        let error = parsed.and_then(|value| value.get("error"));
        let field = |name: &str| {
            error
                .and_then(|error| error.get(name))
                .or_else(|| parsed.and_then(|value| value.get(name)))
        };
        let string = |value: Option<&Value>| match value {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => String::new(),
        };
        let code = string(field("code"));
        let kind = string(field("type"));
        let status = field("code")
            .and_then(Value::as_u64)
            .and_then(|code| u16::try_from(code).ok())
            .filter(|code| (400..600).contains(code));
        let message = [field("message"), error, field("detail")]
            .into_iter()
            .find_map(|value| match value {
                Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_else(|| raw.trim().to_string());
        let text = format!("{message}\n{code}\n{kind}\n{raw}");
        Said {
            code,
            kind,
            status,
            message,
            text,
        }
    }
}

/// 名字不分大小写地找一个头。
fn header<'a>(headers: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

/// 要等多久：`retry-after-ms`、`retry-after` 的秒数、原话里的 `try again in …`，先有的算。
fn retry_after(headers: &[(&str, &str)], text: &str) -> Option<u64> {
    header(headers, "retry-after-ms")
        .and_then(|value| millis(value, 1.0))
        .or_else(|| header(headers, "retry-after").and_then(|value| millis(value, 1000.0)))
        .or_else(|| try_again_in(text))
}

/// 一个非负的数乘上倍数，成了毫秒。
fn millis(number: &str, scale: f64) -> Option<u64> {
    let value: f64 = number.parse().ok()?;
    (value.is_finite() && value >= 0.0).then(|| (value * scale).round() as u64)
}

/// 原话里的 `try again in 6.5s`、`try again in 500ms`、`try again in 20 seconds`。
fn try_again_in(text: &str) -> Option<u64> {
    let (_, rest) = text.split_once("try again in")?;
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    let (number, unit) = rest.split_at(end);
    let unit = unit.trim_start();
    if unit.starts_with("ms") {
        millis(number, 1.0)
    } else if unit.starts_with('s') {
        millis(number, 1000.0)
    } else {
        None
    }
}

/// 截到最多 `limit` 字节，截在字的边界上。
fn clip(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests;
