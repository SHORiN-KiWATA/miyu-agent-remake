//! `/media`（`web-ui.md`「怎么走」第三条，施工 W-10）：页面带登录令牌 `POST /media` 换一张票据，`GET /media/<票据>` 拿一个
//! blob 或者一份本机文件，能带 `Range`。内容由核心照 `blob.get`、`fs.read` 一块 512 KiB 地给，读一块写一块，不整个读进内存。
//! 回应关在一个空的来源里（`sandbox`）：有人直接打开这个地址，它碰不到页面。

pub(crate) mod link;
mod range;
#[cfg(test)]
mod tests;
pub(crate) mod tickets;

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
use crate::serve::{Body, Site, empty, full};
use crate::settings::Settings;
use link::{Cores, Failed};
use range::Span;
use tickets::{Source, Tickets, Wanted};

/// 一块最多多少字节：`blob.get`、`fs.read` 一次最多给这么多。
const CHUNK: u64 = 512 * 1024;

/// 换票据的正文最长多少字节。
pub(crate) const MOST_BODY: usize = 64 * 1024;

/// 给媒体时的 `Content-Security-Policy`：一个空的来源。
const SANDBOX: &str =
    "sandbox; default-src 'none'; img-src data:; media-src data:; style-src 'unsafe-inline'";

/// 表里没有的类型。
const OCTET: &str = "application/octet-stream";

/// 网页软件里管媒体的：票据、连着的核心。
pub(crate) struct Media {
    tickets: Mutex<Tickets<Wanted>>,
    /// 照登录令牌连着的核心：软件后台页（`backstage`）也用这一份。
    pub(crate) cores: Cores,
}

impl Media {
    pub(crate) fn new(settings: &Settings) -> Media {
        Media {
            tickets: Mutex::new(Tickets::new(
                Duration::from_secs(settings.ticket_idle_seconds),
                settings.most_tickets,
            )),
            cores: Cores::default(),
        }
    }

    fn tickets(&self) -> std::sync::MutexGuard<'_, Tickets<Wanted>> {
        self.tickets.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 定时打扫：过期的票据、没人用的核心连接。
    pub(crate) fn sweep(&self) {
        self.tickets().sweep();
        self.cores.sweep();
    }
}

/// `POST /media`：换一张票据。
pub(crate) async fn post(request: Request<Incoming>, site: Arc<Site>) -> Response<Body> {
    if request.method() != Method::POST {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(login) = bearer(&request) else {
        return empty(StatusCode::UNAUTHORIZED);
    };
    let Some((source, kind, name, download)) = read_body(request).await else {
        return empty(StatusCode::BAD_REQUEST);
    };
    // 先问核心有没有、能不能读。
    if let Err(failed) = size(&site, &login, &source).await {
        return refused(&site, &login, &failed);
    }
    let wanted = Wanted {
        login,
        source,
        kind,
        name,
        download,
    };
    let mut fresh = tickets::fresh();
    let ticket = site
        .media
        .tickets()
        .issue(wanted, || fresh.take().unwrap_or_default());
    if ticket.is_empty() {
        tracing::warn!(target: TARGET, "no random bytes for a ticket");
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut response = Response::new(full(json!({"url": format!("/media/{ticket}")}).to_string()));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    crate::serve::secure(headers);
    response
}

/// `GET /media/<票据>`：照票据给，能带 `Range`。
pub(crate) async fn get(request: Request<Incoming>, site: Arc<Site>) -> Response<Body> {
    let busy = site.busy();
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let ticket = request.uri().path().trim_start_matches("/media/");
    let Some(wanted) = site.media.tickets().find(ticket) else {
        return empty(StatusCode::NOT_FOUND);
    };
    // 文件可能变了：照这时的大小算（`web-ui.md`「施工时定的」第 14 条）。
    let size = match size(&site, &wanted.login, &wanted.source).await {
        Ok(size) => size,
        Err(failed) => return refused(&site, &wanted.login, &failed),
    };
    let asked = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let (status, start, end) = match range::span(asked, size) {
        Span::Beyond => {
            let mut response = empty(StatusCode::RANGE_NOT_SATISFIABLE);
            insert(
                response.headers_mut(),
                header::CONTENT_RANGE,
                &format!("bytes */{size}"),
            );
            return response;
        }
        Span::Whole => (StatusCode::OK, 0, size),
        Span::Part { start, end } => (StatusCode::PARTIAL_CONTENT, start, end + 1),
    };
    let (out, frames) = mpsc::channel::<Result<Frame<Bytes>, std::io::Error>>(2);
    let stream = futures_util::stream::unfold(frames, |mut frames| async move {
        frames.recv().await.map(|frame| (frame, frames))
    });
    let mut response = Response::new(BodyExt::boxed(StreamBody::new(stream)));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    insert(
        headers,
        header::CONTENT_TYPE,
        &content_type(&site.settings, &wanted),
    );
    insert(headers, header::CONTENT_LENGTH, &(end - start).to_string());
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if status == StatusCode::PARTIAL_CONTENT {
        insert(
            headers,
            header::CONTENT_RANGE,
            &format!("bytes {start}-{}/{size}", end - 1),
        );
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(SANDBOX),
    );
    if wanted.download {
        let name = wanted.name.as_deref().or(match &wanted.source {
            Source::Path(path) => Some(path.as_str()),
            Source::Blob(_) => None,
        });
        insert(
            headers,
            header::CONTENT_DISPOSITION,
            &range::attachment(name),
        );
    }
    crate::serve::secure(headers);
    tokio::spawn(pour(site, wanted, start, end, out, busy));
    response
}

/// 一块块问核心、一块块写给浏览器，从 `start` 到 `end`（不含）。浏览器走了就停；核心那头断了、给得比说的少（文件变短
/// 了），交一个错，连接照 HTTP 的规矩断掉。
async fn pour(
    site: Arc<Site>,
    wanted: Wanted,
    start: u64,
    end: u64,
    out: mpsc::Sender<Result<Frame<Bytes>, std::io::Error>>,
    _busy: crate::serve::Busy,
) {
    let mut offset = start;
    while offset < end {
        let length = CHUNK.min(end - offset);
        let bytes = match read(&site, &wanted, offset, length).await {
            Ok(bytes) if !bytes.is_empty() => bytes,
            Ok(_) => {
                tracing::warn!(target: TARGET, offset, error = "shorter than said", "media cut short");
                return cut(&out).await;
            }
            Err(failed) => {
                tracing::warn!(target: TARGET, offset, error = ?failed, "media cut short");
                return cut(&out).await;
            }
        };
        // 核心一次最多给要的那么多（`blob.get`、`fs.read` 第 3 条）；多写的 hyper 也照 `Content-Length` 截掉。
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
        .send(Err(std::io::Error::other("media cut short")))
        .await
        .is_err()
    {
        // 浏览器已经走了。
    }
}

/// 问一块：交回这一段的字节。
async fn read(site: &Site, wanted: &Wanted, offset: u64, length: u64) -> Result<Vec<u8>, Failed> {
    let answer = ask(site, &wanted.login, &wanted.source, offset, length).await?;
    STANDARD
        .decode(answer["data"].as_str().unwrap_or_default())
        .map_err(|error| Failed::Unreachable(format!("bad data: {error}")))
}

/// 问大小（`length` 写 0）。
async fn size(site: &Site, login: &str, source: &Source) -> Result<u64, Failed> {
    let answer = ask(site, login, source, 0, 0).await?;
    answer["size"]
        .as_u64()
        .ok_or_else(|| Failed::Unreachable("no size".into()))
}

/// 照来源问 `blob.get` 或者 `fs.read`。
async fn ask(
    site: &Site,
    login: &str,
    source: &Source,
    offset: u64,
    length: u64,
) -> Result<Value, Failed> {
    let (method, params) = match source {
        Source::Blob(blob) => (
            "blob.get",
            json!({"blob": blob, "offset": offset, "length": length}),
        ),
        Source::Path(path) => (
            "fs.read",
            json!({"path": path, "offset": offset, "length": length}),
        ),
    };
    site.media
        .cores
        .call(&site.root, &site.core, login, method, params)
        .await
}

/// 没问成写成状态码；握手被拒的，这个令牌的票据一起作废（`web-ui.md`「施工时定的」第 15 条）。
fn refused(site: &Site, login: &str, failed: &Failed) -> Response<Body> {
    let status = match failed {
        Failed::BadLogin => {
            site.media.tickets().revoke(login);
            StatusCode::UNAUTHORIZED
        }
        Failed::Unreachable(_) => StatusCode::BAD_GATEWAY,
        Failed::Refused(reason) => match reason.as_str() {
            "unknown_blob" | "path_unreadable" => StatusCode::NOT_FOUND,
            "path_forbidden" => StatusCode::FORBIDDEN,
            "bad_params" => StatusCode::BAD_REQUEST,
            _ => StatusCode::BAD_GATEWAY,
        },
    };
    empty(status)
}

/// `Authorization: Bearer <登录令牌>`。
pub(crate) fn bearer(request: &Request<Incoming>) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then(|| token.to_string())
}

/// 换票据的正文：`blob`、`path` 正好一个；`type`、`name` 是字符串，`download` 是布尔，都可以不写。
async fn read_body(
    request: Request<Incoming>,
) -> Option<(Source, Option<String>, Option<String>, bool)> {
    let bytes = Limited::new(request.into_body(), MOST_BODY)
        .collect()
        .await
        .ok()?
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).ok()?;
    let body = body.as_object()?;
    let text = |key: &str| match body.get(key) {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(text)) => Some(Some(text.clone())),
        Some(_) => None,
    };
    let source = match (text("blob")?, text("path")?) {
        (Some(blob), None) => Source::Blob(blob),
        (None, Some(path)) => Source::Path(path),
        _ => return None,
    };
    let download = match body.get("download") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(download)) => *download,
        Some(_) => return None,
    };
    Some((source, text("type")?, text("name")?, download))
}

/// 媒体类型：页面说的在 `web.json` 的表里出现过的照它；本机文件照扩展名查表；都没有的 `application/octet-stream`。
fn content_type(settings: &Settings, wanted: &Wanted) -> String {
    if let Some(kind) = &wanted.kind
        && settings.types.values().any(|known| known == kind)
    {
        return kind.clone();
    }
    match &wanted.source {
        Source::Path(path) => settings.type_of(Path::new(path)).to_string(),
        Source::Blob(_) => OCTET.to_string(),
    }
}

/// 写一个头；写不成的（有控制字符）不写。
pub(crate) fn insert(headers: &mut hyper::HeaderMap, name: header::HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}
