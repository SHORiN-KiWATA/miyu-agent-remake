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

/// 超长的带着超了多少（施工 6-6 中）：解析得出的有，解析不出来的、别的分类没有。
#[test]
fn too_long_says_how_many_tokens_over() {
    let excess = |status: u16, body: &str| classify(&failure(Some(status), &[], body)).excess;
    assert_eq!(
        excess(
            400,
            r#"{"error":{"message":"This model's maximum context length is 65536 tokens. However, you requested 70000 tokens. Please reduce the length of the messages or completion.","type":"invalid_request_error","code":"invalid_request_error"}}"#
        ),
        Some(4_464)
    );
    assert_eq!(
        excess(
            400,
            r#"{"error":{"message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#
        ),
        Some(10_000)
    );
    assert_eq!(excess(413, "Request Entity Too Large"), None);
    // 限速的原话里也有两个数，不是超长就不解析。
    assert_eq!(
        excess(
            429,
            r#"{"error":{"message":"maximum context length is 1000 tokens, you requested 2000 tokens per minute","code":"rate_limit_exceeded"}}"#
        ),
        None
    );
}

/// 超长的报了上限的交出它（施工 8-7）：说了上限、说不出超了多少的也交；限速、别的分类不交。
#[test]
fn too_long_gives_the_stated_limit() {
    let limit = |status: u16, body: &str| classify(&failure(Some(status), &[], body)).limit;
    assert_eq!(
        limit(
            400,
            r#"{"error":{"message":"This model's maximum context length is 65536 tokens. However, you requested 70000 tokens.","code":"context_length_exceeded"}}"#
        ),
        Some(65_536)
    );
    assert_eq!(
        limit(
            400,
            r#"{"error":{"message":"This model's maximum context length is 32,768 tokens."}}"#
        ),
        Some(32_768)
    );
    assert_eq!(
        limit(
            400,
            r#"{"error":{"message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#
        ),
        Some(200_000)
    );
    assert_eq!(limit(413, "Request Entity Too Large"), None);
    assert_eq!(
        limit(
            429,
            r#"{"error":{"message":"maximum context length is 1000 tokens, you requested 2000 tokens per minute","code":"rate_limit_exceeded"}}"#
        ),
        None
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

/// HTTP 状态码另记一格（施工 3-5 三补）：每一类的状态码都带上，原话开头的 `HTTP <状态>: ` 照留；分类照旧（404 是
/// `other`，402 是 `auth`）。连不上的、流里报的没有，流里的 `code` 只拿来分类。
#[test]
fn the_http_status_is_kept_in_its_own_field() {
    for (status, class) in [
        (400, ErrorClass::Unclassified),
        (401, ErrorClass::Auth),
        (402, ErrorClass::Auth),
        (403, ErrorClass::Auth),
        (404, ErrorClass::Unclassified),
        (408, ErrorClass::Retryable),
        (413, ErrorClass::ContextTooLong),
        (429, ErrorClass::RateLimited),
        (500, ErrorClass::Retryable),
        (503, ErrorClass::Retryable),
    ] {
        let got = classify(&failure(Some(status), &[], "Not Found"));
        assert_eq!(
            (&got.error.class, got.error.status),
            (&class, Some(status)),
            "{status}"
        );
        assert_eq!(got.error.message, format!("HTTP {status}: Not Found"));
    }
    let refused = classify(&failure(
        None,
        &[],
        "error sending request: Connection refused",
    ));
    assert_eq!(refused.error.status, None);
    let streamed = classify(&Failure::stream(
        br#"{"error":{"message":"Provider returned error","code":429}}"#,
    ));
    assert_eq!(
        (streamed.error.class, streamed.error.status),
        (ErrorClass::RateLimited, None)
    );
}

#[test]
fn anthropic_error_bodies() {
    // 施工 8-12：Anthropic 的错误体 `{"type":"error","error":{"type":…,"message":…}}`，照官方文档的几类各一个。
    let body = |kind: &str, message: &str| {
        format!(
            r#"{{"type":"error","error":{{"type":"{kind}","message":"{message}"}},"request_id":"req_1"}}"#
        )
    };
    let cases = [
        (
            400,
            body(
                "invalid_request_error",
                "prompt is too long: 210000 tokens > 200000 maximum",
            ),
            ErrorClass::ContextTooLong,
        ),
        (
            400,
            body(
                "invalid_request_error",
                "input length and `max_tokens` exceed context limit: 197000 + 8192 > 200000, decrease input length or `max_tokens` and try again",
            ),
            ErrorClass::ContextTooLong,
        ),
        (
            400,
            body(
                "invalid_request_error",
                "Output blocked by content filtering policy",
            ),
            ErrorClass::ContentPolicy,
        ),
        (
            400,
            body(
                "invalid_request_error",
                "messages.1.content.0: unexpected `tool_use_id` found",
            ),
            ErrorClass::Unclassified,
        ),
        (
            401,
            body("authentication_error", "invalid x-api-key"),
            ErrorClass::Auth,
        ),
        (
            402,
            body("billing_error", "Your credit balance is too low"),
            ErrorClass::Auth,
        ),
        (
            403,
            body("permission_error", "Your API key does not have permission"),
            ErrorClass::Auth,
        ),
        (
            404,
            body("not_found_error", "model: claude-nope"),
            ErrorClass::Unclassified,
        ),
        (
            413,
            body(
                "request_too_large",
                "Request exceeds the maximum allowed number of bytes",
            ),
            ErrorClass::ContextTooLong,
        ),
        (
            429,
            body(
                "rate_limit_error",
                "Number of request tokens has exceeded your per-minute rate limit",
            ),
            ErrorClass::RateLimited,
        ),
        (
            500,
            body("api_error", "Internal server error"),
            ErrorClass::Retryable,
        ),
        (
            529,
            body("overloaded_error", "Overloaded"),
            ErrorClass::Retryable,
        ),
    ];
    for (status, body, want) in cases {
        let got = classify(&failure(Some(status), &[], &body));
        assert_eq!(got.error.class, want, "{status} {body}");
        assert!(
            got.error.message.starts_with(&format!("HTTP {status}: ")),
            "{}",
            got.error.message
        );
        assert!(
            !got.error.message.contains("request_id"),
            "原话是 error.message：{}",
            got.error.message
        );
    }
    // 超长的老说法：超了多少、上限不解析。
    let old = classify(&failure(
        Some(400),
        &[],
        &body(
            "invalid_request_error",
            "input length and `max_tokens` exceed context limit: 197000 + 8192 > 200000",
        ),
    ));
    assert_eq!((old.excess, old.limit), (None, None));
    // 流里的错没有状态：过载可重试。
    let stream = classify(&Failure::stream(
        body("overloaded_error", "Overloaded").as_bytes(),
    ));
    assert_eq!(stream.error.class, ErrorClass::Retryable);
    assert_eq!(stream.error.message, "Overloaded");
}
