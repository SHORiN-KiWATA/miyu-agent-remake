//! 软件后台页（`web-ui.md`「怎么走」第四条，施工 F-6 下；`package-pages.md`「网页软件怎么给后台页」）：页面带登录令牌
//! `POST /page` 换一张票据，`GET /p/<票据>/<包>/<路径>` 拿这个包后台页里的一份文件，放进页面里隔离的框。内容由核心照
//! `package.file` 一块 512 KiB 地给，读一块写一块。框的地址里只有票据、不带登录令牌：框里的页面读得到自己的地址。

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use http_body_util::{BodyExt, Limited, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::TARGET;
use crate::media::link::Failed;
use crate::media::tickets::{self, Owned, Tickets};
use crate::media::{MOST_BODY, bearer, insert};
use crate::serve::{Body, Site, empty, full};
use crate::settings::Settings;

/// 一张票据管的：谁换的、哪个包的后台页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page {
    /// 换票据的登录令牌：给的时候照它连核心。
    login: String,
    /// 包的编号。
    package: String,
}

impl Owned for Page {
    fn login(&self) -> &str {
        &self.login
    }
}

/// 网页软件里管软件后台页的：票据（多久作废、最多几张同 `/media`）。
pub(crate) struct Backstage {
    tickets: Mutex<Tickets<Page>>,
}

impl Backstage {
    pub(crate) fn new(settings: &Settings) -> Backstage {
        Backstage {
            tickets: Mutex::new(Tickets::new(
                Duration::from_secs(settings.ticket_idle_seconds),
                settings.most_tickets,
            )),
        }
    }

    fn tickets(&self) -> std::sync::MutexGuard<'_, Tickets<Page>> {
        self.tickets.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 定时打扫：过期的票据。
    pub(crate) fn sweep(&self) {
        self.tickets().sweep();
    }
}

/// `POST /page`：这个包有后台页的，换一张票据。
pub(crate) async fn post(request: Request<Incoming>, site: Arc<Site>) -> Response<Body> {
    if request.method() != Method::POST {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(login) = bearer(&request) else {
        return empty(StatusCode::UNAUTHORIZED);
    };
    let Some(package) = read_body(request).await else {
        return empty(StatusCode::BAD_REQUEST);
    };
    // 先问核心这个包有没有后台页：`package.list` 的那一项 `page` 是真的。
    let listed = match ask(&site, &login, "package.list", json!({})).await {
        Ok(listed) => listed,
        Err(failed) => return refused(&site, &login, &failed),
    };
    let has_page = listed["packages"].as_array().is_some_and(|all| {
        all.iter()
            .any(|p| p["package"] == json!(package) && p["page"] == json!(true))
    });
    if !has_page {
        return empty(StatusCode::NOT_FOUND);
    }
    let mut fresh = tickets::fresh();
    let ticket = site.backstage.tickets().issue(
        Page {
            login,
            package: package.clone(),
        },
        || fresh.take().unwrap_or_default(),
    );
    if ticket.is_empty() {
        tracing::warn!(target: TARGET, "no random bytes for a ticket");
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut response = Response::new(full(
        json!({"url": format!("/p/{ticket}/{package}/")}).to_string(),
    ));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    crate::serve::secure(headers);
    response
}

/// `GET /p/<票据>/<包>/<路径>`：照票据给这个包后台页里的一份文件。
pub(crate) async fn get(request: Request<Incoming>, site: Arc<Site>) -> Response<Body> {
    let busy = site.busy();
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some((ticket, package, path)) = split(request.uri().path()) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some(page) = site.backstage.tickets().find(&ticket) else {
        return empty(StatusCode::NOT_FOUND);
    };
    if page.package != package {
        return empty(StatusCode::NOT_FOUND);
    }
    // 先读第一块：知道一共多大、有没有这份文件，再写响应头。
    let first = match read(&site, &page, &path, 0).await {
        Ok(first) => first,
        Err(failed) => return refused(&site, &page.login, &failed),
    };
    let (out, frames) = mpsc::channel::<Result<Frame<Bytes>, std::io::Error>>(2);
    let stream = futures_util::stream::unfold(frames, |mut frames| async move {
        frames.recv().await.map(|frame| (frame, frames))
    });
    let mut response = Response::new(BodyExt::boxed(StreamBody::new(stream)));
    let headers = response.headers_mut();
    insert(
        headers,
        header::CONTENT_TYPE,
        content_type(&site.settings, &path),
    );
    insert(headers, header::CONTENT_LENGTH, &first.size.to_string());
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    insert(
        headers,
        header::CONTENT_SECURITY_POLICY,
        &site.settings.backstage_csp,
    );
    // 框的来源是空的（不给 `allow-same-origin`）：ES 模块脚本、字体照跨源取，不给这一格取不到；票据就是凭据。
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    crate::serve::secure(headers);
    tokio::spawn(pour(site, page, path, first, out, busy));
    response
}

/// `package.file` 给的一块：这一段、整份多大、读到头了没有。
struct Chunk {
    bytes: Vec<u8>,
    size: u64,
    eof: bool,
}

/// 一块块问核心、一块块写给浏览器，第一块已经读了。浏览器走了就停；核心那头断了、给得比说的少，交一个错，连接照 HTTP 的
/// 规矩断掉。
async fn pour(
    site: Arc<Site>,
    page: Page,
    path: String,
    first: Chunk,
    out: mpsc::Sender<Result<Frame<Bytes>, std::io::Error>>,
    _busy: crate::serve::Busy,
) {
    let (size, mut eof) = (first.size, first.eof);
    let mut offset = first.bytes.len() as u64;
    if out
        .send(Ok(Frame::data(Bytes::from(first.bytes))))
        .await
        .is_err()
    {
        return;
    }
    while offset < size {
        if eof {
            tracing::warn!(target: TARGET, offset, error = "shorter than said", "page cut short");
            return cut(&out).await;
        }
        let bytes = match read(&site, &page, &path, offset).await {
            Ok(chunk) if !chunk.bytes.is_empty() => {
                eof = chunk.eof;
                chunk.bytes
            }
            Ok(_) => {
                tracing::warn!(target: TARGET, offset, error = "shorter than said", "page cut short");
                return cut(&out).await;
            }
            Err(failed) => {
                tracing::warn!(target: TARGET, offset, error = ?failed, "page cut short");
                return cut(&out).await;
            }
        };
        offset += bytes.len() as u64;
        if out.send(Ok(Frame::data(Bytes::from(bytes)))).await.is_err() {
            // 浏览器走了：不再问核心。
            return;
        }
    }
}

/// 没给完：交一个错，hyper 照 HTTP 的规矩断掉这个连接（浏览器知道没收全）。
async fn cut(out: &mpsc::Sender<Result<Frame<Bytes>, std::io::Error>>) {
    if out
        .send(Err(std::io::Error::other("page cut short")))
        .await
        .is_err()
    {
        // 浏览器已经走了。
    }
}

/// 问一块。
async fn read(site: &Site, page: &Page, path: &str, offset: u64) -> Result<Chunk, Failed> {
    let answer = ask(
        site,
        &page.login,
        "package.file",
        json!({"package": page.package, "path": path, "offset": offset}),
    )
    .await?;
    let bytes = STANDARD
        .decode(answer["data"].as_str().unwrap_or_default())
        .map_err(|error| Failed::Unreachable(format!("bad data: {error}")))?;
    let size = answer["size"]
        .as_u64()
        .ok_or_else(|| Failed::Unreachable("no size".into()))?;
    Ok(Chunk {
        bytes,
        size,
        eof: answer["eof"].as_bool().unwrap_or(true),
    })
}

/// 照登录令牌问核心一次（和 `/media` 共用连着的核心）。
async fn ask(site: &Site, login: &str, method: &str, params: Value) -> Result<Value, Failed> {
    site.media
        .cores
        .call(&site.root, &site.core, login, method, params)
        .await
}

/// 没问成写成状态码；握手被拒的，这个令牌的票据一起作废（同 `/media`）。
fn refused(site: &Site, login: &str, failed: &Failed) -> Response<Body> {
    let status = match failed {
        Failed::BadLogin => {
            site.backstage.tickets().revoke(login);
            StatusCode::UNAUTHORIZED
        }
        Failed::Unreachable(_) => StatusCode::BAD_GATEWAY,
        Failed::Refused(reason) => status_of(reason),
    };
    empty(status)
}

/// 核心拒的原因码换成状态码。
fn status_of(reason: &str) -> StatusCode {
    match reason {
        "not_found" | "no_page" | "unknown_package" => StatusCode::NOT_FOUND,
        "bad_params" => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// 换票据的正文：`{"package"}`，包的编号是非空的字符串。
async fn read_body(request: Request<Incoming>) -> Option<String> {
    let bytes = Limited::new(request.into_body(), MOST_BODY)
        .collect()
        .await
        .ok()?
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).ok()?;
    let package = body.as_object()?.get("package")?.as_str()?;
    valid_package(package).then(|| package.to_string())
}

/// 包的编号的写法：字母、数字、`-`、`_`、`.`，1 到 64 个（放进地址里不用转义；`miyu web --package` 也照它查）。
pub fn valid_package(package: &str) -> bool {
    (1..=64).contains(&package.len())
        && package
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

/// `/p/<票据>/<包>/<路径>` 拆成三段：路径的 `%xx` 换回来，以 `/` 结尾的补上 `index.html`，空的照核心认成 `index.html`。
/// 带 `..`、`\` 这些的由核心拒（`package.file` 第 1 条）。
fn split(path: &str) -> Option<(String, String, String)> {
    let rest = path.strip_prefix("/p/")?;
    let (ticket, rest) = rest.split_once('/')?;
    let (package, file) = rest.split_once('/').unwrap_or((rest, ""));
    if ticket.is_empty() || !valid_package(package) {
        return None;
    }
    let mut file = miyu_webserve::pages::decode(file)?;
    if file.ends_with('/') {
        file.push_str("index.html");
    }
    Some((ticket.to_string(), package.to_string(), file))
}

/// 媒体类型照扩展名查 `web.json` 的表；空路径是 `index.html`；查不到的 `application/octet-stream`。
fn content_type<'a>(settings: &'a Settings, path: &str) -> &'a str {
    settings.type_of(Path::new(if path.is_empty() { "index.html" } else { path }))
}
