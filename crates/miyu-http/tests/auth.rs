//! 认证头照驱动（`docs/blueprint/models.md`「驱动要守的约定」第 2 条，施工 8-6）：HTTP 执行器不自己写 `Bearer`，驱动交回
//! 什么头就带什么头；没有 key 的端点一个认证头都不带；运行日志、调试输出里没有 key。

use std::collections::BTreeSet;
use std::time::Duration;

use crate::support::{BODY, driver, sample};
use miyu_drivers::classify::{Classified, Failure};
use miyu_drivers::{BlobBytes, Call, Decode, Driver, EncodeError, Encoded, OpenAiChat};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Endpoint, Outcome, Proxy, client, send};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;

/// 一个照 Anthropic 那样带 `x-api-key` 和版本头的驱动：别的照 OpenAI 兼容的。
struct ApiKey(OpenAiChat);

impl Driver for ApiKey {
    fn family(&self) -> &'static str {
        self.0.family()
    }

    fn blobs_needed(&self, request: &Request, call: &Call) -> BTreeSet<ContentHash> {
        self.0.blobs_needed(request, call)
    }

    fn encode(
        &self,
        request: &Request,
        call: &Call,
        blobs: &dyn BlobBytes,
    ) -> Result<Encoded, EncodeError> {
        self.0.encode(request, call, blobs)
    }

    fn decoder(&self) -> Box<dyn Decode> {
        self.0.decoder()
    }

    fn classify(&self, failure: &Failure<'_>) -> Classified {
        self.0.classify(failure)
    }

    fn auth(&self, key: &str) -> Vec<(String, String)> {
        vec![
            ("x-api-key".to_string(), key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ]
    }

    fn models_path(&self) -> &'static str {
        self.0.models_path()
    }

    fn parse_models(&self, bytes: &[u8]) -> Result<Vec<miyu_drivers::Listed>, String> {
        self.0.parse_models(bytes)
    }
}

/// 照 `driver` 发一次给假服务器，带 key `key`（没有的不带），交回服务器收到的那一个请求的头。
async fn headers_sent(driver: &dyn Driver, key: Option<&str>) -> Vec<(String, String)> {
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(sample(
        "openai-text",
    ))])])
    .await;
    let endpoint = match key {
        Some(key) => Endpoint::new(&server.base_url, key),
        None => Endpoint::keyless(&server.base_url),
    };
    let client = client(Proxy::Off).expect("造得出客户端");
    let outcome = send(
        Attempt {
            client: &client,
            endpoint: &endpoint,
            driver,
            body: BODY,
            path: "/chat/completions",
            idle: Duration::from_secs(5),
        },
        std::future::pending(),
        |_| {},
    )
    .await;
    assert!(
        matches!(outcome, Outcome::Ended { error: None, .. }),
        "{outcome:?}"
    );
    server.received()[0].headers.clone()
}

fn named<'a>(headers: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .collect()
}

#[tokio::test]
async fn the_auth_headers_are_what_the_driver_says() {
    let keyed = Some("sk-test");
    let bearer = headers_sent(&driver(), keyed).await;
    assert_eq!(named(&bearer, "authorization"), ["Bearer sk-test"]);
    assert!(named(&bearer, "x-api-key").is_empty());

    let api_key = headers_sent(&ApiKey(driver()), keyed).await;
    assert!(
        named(&api_key, "authorization").is_empty(),
        "不自己写 Bearer"
    );
    assert_eq!(named(&api_key, "x-api-key"), ["sk-test"]);
    assert_eq!(named(&api_key, "anthropic-version"), ["2023-06-01"]);
}

#[tokio::test]
async fn an_endpoint_without_a_key_sends_no_auth_header() {
    let keyless = Endpoint::keyless("http://unused.invalid");
    for driver in [&driver() as &dyn Driver, &ApiKey(driver())] {
        let headers = headers_sent(driver, None).await;
        assert!(named(&headers, "authorization").is_empty(), "{headers:?}");
        assert!(named(&headers, "x-api-key").is_empty(), "{headers:?}");
    }
    assert_eq!(
        format!("{keyless:?}"),
        r#"Endpoint { host: "unused.invalid", key: None, headers: [] }"#
    );
    assert!(!format!("{:?}", Endpoint::new("http://a.invalid", "sk-secret")).contains("sk-secret"));
}
