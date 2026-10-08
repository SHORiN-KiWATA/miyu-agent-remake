//! 回应（`web-ui.md`「怎么走」第一条第 5 款）：正文的类型，没有内容的回应，一律带的两个安全响应头。从来不设 cookie
//! （`web-module.md`「起草时定的」第 13 条）。

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{self, HeaderValue};
use hyper::{Response, StatusCode};

/// 回应的正文：整段的，或者一块块给的（网页软件的 `/media`）。
pub type Body = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

/// 整段的正文。
pub fn full(bytes: impl Into<Bytes>) -> Body {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

/// 没有内容的回应，带两个一律有的头。
pub fn empty(status: StatusCode) -> Response<Body> {
    let mut response = Response::new(full(Bytes::new()));
    *response.status_mut() = status;
    secure(response.headers_mut());
    response
}

/// 一律带的：不猜类型、不带 Referer。从来不设 cookie（「起草时定的」第 13 条）。
pub fn secure(headers: &mut hyper::HeaderMap) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

/// 字写成头的值；写不成的（有控制字符）是空的。
pub fn value(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap_or_else(|_| HeaderValue::from_static(""))
}
