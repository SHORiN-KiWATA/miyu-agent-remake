//! `/human`（施工 O-16，`onebot.md` 第二条「对外的样子」）：页面登录以前要的字。登录以前页面还没和核心握手，调不了
//! `human.get`；桥照自己的语言（握手回的）读 `software/onebot/human/<语言>.json`（`Human::load`，和 `human.get` 同一个读法），
//! 只交 `software/onebot/web/` 开头的那些，样子和 `human.get` 的 `said` 一样。登录以后页面照 `human.get` 换一遍。不要登录：
//! 里面没有秘密，和页面文件一样。

use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use serde_json::json;
use std::collections::BTreeMap;

use miyu_store::human::Human;
use miyu_webserve::respond::{Body, empty, full, secure};

use super::Web;
use crate::TARGET;

/// 页面的字的编号前缀。
const PREFIX: &str = "software/onebot/web/";

/// `GET /human`：`{"language": …, "said": {编号: 模板}}`。别的方法 405；字读不懂 500，记一行。
pub(super) async fn get<B>(request: &Request<B>, web: &Web) -> Response<Body> {
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let resources = web.resources.clone();
    let language = web.language.clone();
    let wanted = language.clone();
    let human = match tokio::task::spawn_blocking(move || Human::load(&resources, &wanted)).await {
        Ok(Ok(human)) => human,
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "human not read");
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(error) => {
            tracing::error!(target: TARGET, error = %error, "human panicked");
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let said: BTreeMap<&str, &str> = human
        .said_entries()
        .filter(|(key, _)| key.starts_with(PREFIX))
        .collect();
    let mut response = Response::new(full(
        json!({"language": language, "said": said}).to_string(),
    ));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    secure(headers);
    response
}
