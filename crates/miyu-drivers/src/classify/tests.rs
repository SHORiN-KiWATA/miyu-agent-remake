//! 出错分类：各家的写法对应到六类和要等多久。响应体照各家文档和调研里的原话造。

use super::*;

fn failure<'a>(
    status: Option<u16>,
    headers: &'a [(&'a str, &'a str)],
    body: &'a str,
) -> Failure<'a> {
    Failure {
        status,
        headers,
        body: body.as_bytes(),
    }
}

fn class(status: Option<u16>, headers: &[(&str, &str)], body: &str) -> (ErrorClass, Option<u64>) {
    let got = classify(&failure(status, headers, body));
    (got.error.class, got.retry_after_ms)
}

#[test]
fn context_too_long() {
    for (status, body) in [
        // OpenAI：错误码。
        (
            400,
            r#"{"error":{"message":"This model's maximum context length is 128000 tokens. However, your messages resulted in 130000 tokens.","type":"invalid_request_error","code":"context_length_exceeded"}}"#,
        ),
        // DeepSeek：错误码是笼统的，靠原话。
        (
            400,
            r#"{"error":{"message":"This model's maximum context length is 65536 tokens. However, you requested 70000 tokens. Please reduce the length of the messages or completion.","type":"invalid_request_error","param":null,"code":"invalid_request_error"}}"#,
        ),
        // 经网关的 Anthropic。
        (
            400,
            r#"{"error":{"message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#,
        ),
        // 本机的 llama.cpp。
        (
            400,
            r#"{"error":{"code":400,"message":"the request exceeds the available context size, try increasing it","type":"exceed_context_size_error"}}"#,
        ),
        // 请求体太大。
        (413, "Request Entity Too Large"),
    ] {
        assert_eq!(
            class(Some(status), &[], body),
            (ErrorClass::ContextTooLong, None),
            "{body}"
        );
    }
    // 流里报的，没有状态。
    assert_eq!(
        class(
            None,
            &[],
            r#"{"error":{"message":"Input is too long for requested model."}}"#
        )
        .0,
        ErrorClass::ContextTooLong
    );
}

#[test]
fn a_rate_limit_that_mentions_tokens_is_not_too_long() {
    let body = r#"{"error":{"message":"Rate limit reached for gpt-4o in organization org-x on tokens per min (TPM): Limit 30000, Used 29000, Requested 2000. Please try again in 2.5s. Too many tokens.","type":"tokens","code":"rate_limit_exceeded"}}"#;
    assert_eq!(
        class(Some(429), &[], body),
        (ErrorClass::RateLimited, Some(2500))
    );
    // Bedrock 的限速是 400，也不当超长。
    let body = "Throttling error: Too many tokens, please wait before trying again.";
    assert_eq!(class(Some(400), &[], body).0, ErrorClass::RateLimited);
}

#[test]
fn content_policy() {
    for body in [
        r#"{"error":{"message":"Your request was rejected as a result of our safety system.","type":"invalid_request_error","code":"content_policy_violation"}}"#,
        r#"{"error":{"message":"The response was filtered due to the prompt triggering Azure OpenAI's content management policy.","code":"content_filter"}}"#,
    ] {
        assert_eq!(
            class(Some(400), &[], body).0,
            ErrorClass::ContentPolicy,
            "{body}"
        );
    }
}

#[test]
fn auth_and_quota() {
    let bad_key = r#"{"error":{"message":"Authentication Fails (no such user)","type":"authentication_error","param":null,"code":"invalid_request_error"}}"#;
    assert_eq!(class(Some(401), &[], bad_key).0, ErrorClass::Auth);
    assert_eq!(class(Some(403), &[], "Forbidden").0, ErrorClass::Auth);
    // 额度用完：402，或者 429 带着额度的说法。重试没用。
    let balance = r#"{"error":{"message":"Insufficient Balance","type":"unknown_error","param":null,"code":"invalid_request_error"}}"#;
    assert_eq!(class(Some(402), &[], balance).0, ErrorClass::Auth);
    let quota = r#"{"error":{"message":"You exceeded your current quota, please check your plan and billing details.","type":"insufficient_quota","code":"insufficient_quota"}}"#;
    assert_eq!(class(Some(429), &[], quota).0, ErrorClass::Auth);
    let zen = r#"{"error":{"type":"FreeUsageLimitError","message":"Free usage limit reached."}}"#;
    assert_eq!(class(Some(429), &[], zen).0, ErrorClass::Auth);
}

#[test]
fn rate_limited_and_how_long_to_wait() {
    let body = r#"{"error":{"message":"Rate limit exceeded"}}"#;
    assert_eq!(
        class(Some(429), &[("Retry-After", "20")], body),
        (ErrorClass::RateLimited, Some(20_000))
    );
    // retry-after-ms 在前。
    assert_eq!(
        class(
            Some(429),
            &[("retry-after", "2"), ("retry-after-ms", "1500")],
            body
        ),
        (ErrorClass::RateLimited, Some(1500))
    );
    // 秒数可以带小数；原话里的也认；都没有就不写。
    assert_eq!(
        class(Some(429), &[("retry-after", "0.5")], body).1,
        Some(500)
    );
    assert_eq!(
        class(
            Some(429),
            &[],
            r#"{"error":{"message":"Please try again in 500ms."}}"#
        )
        .1,
        Some(500)
    );
    assert_eq!(
        class(Some(429), &[], "slow down, try again in 20 seconds").1,
        Some(20_000)
    );
    assert_eq!(class(Some(429), &[], body).1, None);
}

#[test]
fn retryable_and_other() {
    for status in [408, 409, 500, 502, 503, 529] {
        assert_eq!(
            class(Some(status), &[], "upstream error").0,
            ErrorClass::Retryable,
            "{status}"
        );
    }
    // 503 带着要等多久。
    assert_eq!(
        class(Some(503), &[("retry-after", "5")], "Service Unavailable"),
        (ErrorClass::Retryable, Some(5000))
    );
    // 连接断了、流里报的错：没有状态，可重试。
    assert_eq!(
        class(
            None,
            &[],
            r#"{"error":{"message":"upstream connect error"}}"#
        )
        .0,
        ErrorClass::Retryable
    );
    // 流里报的错带着数字的 code，当状态用。
    assert_eq!(
        class(
            None,
            &[],
            r#"{"error":{"message":"Provider returned error","code":429}}"#
        )
        .0,
        ErrorClass::RateLimited
    );
    // 别的 4xx：请求本身不对，重试没用。
    let invalid = r#"{"error":{"message":"Invalid 'messages[1].content': string too long.","type":"invalid_request_error"}}"#;
    assert_eq!(class(Some(400), &[], invalid).0, ErrorClass::Unclassified);
}

#[test]
fn the_should_retry_header_decides() {
    assert_eq!(
        class(Some(500), &[("x-should-retry", "false")], "").0,
        ErrorClass::Unclassified
    );
    assert_eq!(
        class(Some(400), &[("X-Should-Retry", "true")], "").0,
        ErrorClass::Retryable
    );
}

#[test]
fn the_message_is_the_providers_words() {
    let got = classify(&failure(
        Some(401),
        &[],
        r#"{"error":{"message":"Authentication Fails","type":"authentication_error"}}"#,
    ));
    assert_eq!(got.error.message, "HTTP 401: Authentication Fails");
    // 不是 JSON 的，用响应体本身；最长 2000 字节，截在字的边界上。
    let page = format!("<html>{}</html>", "错".repeat(1000));
    let got = classify(&failure(Some(502), &[], &page));
    assert!(got.error.message.starts_with("HTTP 502: <html>错"));
    assert!(got.error.message.len() <= MESSAGE_LIMIT);
    assert!(got.error.message.ends_with('错'));
}
