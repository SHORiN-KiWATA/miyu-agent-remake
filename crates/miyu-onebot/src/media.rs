//! 取到手、存下来（施工 O-33，`onebot.md` 第一条「平台工具（二）」第 4、5 条）：NapCat 说东西在哪（`onebot::media::sources`），
//! 这里照先后拿到字节；视频、文件写进会话工作区的 `qq-files/`。
//!
//! - 拿字节（[`bytes`]）：先下 `http`、`https` 的地址（核心的 HTTP 客户端 `miyu-http`：连接的时限照它，整个最多 `fetch_seconds`；
//!   回环地址不走代理，别的照环境变量），再读本机路径（NapCat 和桥在同一台机器上才有），再解 base64（NapCat 开了
//!   `enableLocalFile2Url` 才给）；都不行的交回每一样为什么（2026-10-11 主会话定：很多人的 NapCat 跑在 Docker 里，路径读不到）。
//!   不设大小上限（2026-10-11 项目主人定）：整个读进内存，大的视频吃内存（施工单「风险」）。
//! - 存（[`save`]）：路径从核心给的 `cwd` 来，`~` 自己换成家目录；只写进它下面的 `qq-files/`，文件名去掉路径分隔符、开头的点；
//!   先写临时文件再改名（同一个文件取两次照新的换掉，读的一方看不到写了一半的）；`qq-files` 和文件落到的真实位置都要还在
//!   工作区里，不跟链接走（`qq-files` 是链接的不写，文件名那里本来是链接的被换掉、不写进它指的地方）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use miyu_http::{Get, Got, Proxy};
use serde_json::Value;

use crate::listen::bots::Link;
use crate::onebot::media::{Source, get_media, get_msg, sources};
use crate::onebot::{CallError, Fetch, said};

/// 没取到。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Missed {
    /// NapCat 回了失败：它说的（去掉首尾空白、截到 200 个字符，同出站队列的 `rejected`）。
    Failed(String),
    /// 等不到 NapCat 回。
    Unanswered,
    /// 那个机器人号没连着、连接断了。
    Unreachable,
    /// NapCat 回了，东西拿不到手（[`bytes`] 说的为什么）。
    Unfetched(String),
}

impl Missed {
    /// 记运行日志、答她的那一句的名字（`tool-results/<名字>.txt`）。
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Missed::Failed(_) => "failed",
            Missed::Unanswered => "unanswered",
            Missed::Unreachable => "unreachable",
            Missed::Unfetched(_) => "unfetched",
        }
    }
}

/// NapCat 那一次调用没成算成哪一种。
fn missed(error: CallError) -> Missed {
    match error {
        CallError::Failed(reply) => Missed::Failed(said(&reply)),
        CallError::Timeout => Missed::Unanswered,
        CallError::Closed => Missed::Unreachable,
    }
}

/// 经连接 `link` 取平台编号是 `msg` 的那一条消息（`get_msg`），交回回应的 `data`；等多久照一次 OneBot 调用。
///
/// # Errors
///
/// NapCat 回了失败（没有这一条、过期了）、等不到、没连着。
pub(crate) async fn message(link: &Link, msg: &str) -> Result<Value, Missed> {
    let (action, params) = get_msg(msg);
    let reply = link
        .calls
        .call(&link.out, action, params)
        .await
        .map_err(missed)?;
    Ok(reply["data"].clone())
}

/// 经连接 `link` 照 `fetch` 取编号是 `id` 的那一样（`get_image`、`get_file`），再照回应说的地方拿到字节（[`bytes`]）。NapCat
/// 回应、下载各最多等 `wait`。交回字节和回应的 `data`（文件名在里面）。
///
/// # Errors
///
/// NapCat 回了失败、等不到、没连着；回了但哪儿都拿不到。
pub(crate) async fn take(
    link: &Link,
    fetch: Fetch,
    id: &str,
    wait: Duration,
) -> Result<(Vec<u8>, Value), Missed> {
    let (action, params) = get_media(fetch, id);
    let pending = link
        .calls
        .begin(&link.out, action, params)
        .await
        .map_err(missed)?;
    let reply = pending.wait_for(wait).await.map_err(missed)?;
    let data = reply["data"].clone();
    let bytes = bytes(&sources(&data), wait)
        .await
        .map_err(Missed::Unfetched)?;
    Ok((bytes, data))
}

/// 工作区里放取来的视频、文件的目录（施工单「要定的」第 3 条）。
pub(crate) const QQ_FILES: &str = "qq-files";

/// 文件名里名字那一段最多几个字符：前面还有消息编号和第几个，整个不超过文件系统的 255 字节。
const NAME: usize = 80;

/// 照 `sources` 的先后拿字节，下载整个最多 `timeout`。都不行的交回每一样为什么，英文，`; ` 隔开；一样都没有的说没有。
///
/// # Errors
///
/// 地址下不下来、路径读不了、base64 解不出：都不行的时候。
pub(crate) async fn bytes(sources: &[Source], timeout: Duration) -> Result<Vec<u8>, String> {
    let mut why = Vec::new();
    for source in sources {
        let got = match source {
            Source::Url(url) => download(url, timeout).await,
            Source::Path(path) => read(path.clone()).await,
            Source::Base64(encoded) => STANDARD
                .decode(encoded)
                .map_err(|_| "base64: not valid".to_string()),
        };
        match got {
            Ok(bytes) => return Ok(bytes),
            Err(error) => why.push(error),
        }
    }
    if why.is_empty() {
        return Err("no url, path or base64 in the answer".to_string());
    }
    Err(why.join("; "))
}

/// 下一个地址：回环的不走代理（开发机上设了代理也连得上本机），别的照环境变量。
async fn download(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let proxy = if miyu_http::is_loopback_url(url) {
        Proxy::Off
    } else {
        Proxy::FromEnvironment
    };
    let client = miyu_http::fetcher(proxy).map_err(|error| format!("url: {error}"))?;
    let got = miyu_http::get(Get {
        client: &client,
        url,
        headers: &[],
        etag: None,
        timeout,
        limit: usize::MAX,
    })
    .await
    .map_err(|error| format!("url: {error}"))?;
    match got {
        Got::Body { bytes, .. } => Ok(bytes),
        // 没带 `ETag` 问，不会回 304。
        Got::NotModified => Err("url: not modified".to_string()),
    }
}

/// 读本机的一个文件（在阻塞线程里）：要是普通文件。
async fn read(path: PathBuf) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        if !path.is_file() {
            return Err("path: not a file here".to_string());
        }
        std::fs::read(&path).map_err(|error| format!("path: {error}"))
    })
    .await
    .map_err(|error| format!("path: {error}"))?
}

/// 存进工作区的文件名：`<消息编号>-<第几个>`，有名字的接 `-<名字>`；名字去掉路径分隔符、Windows 不许的字符和控制字符，去掉
/// 开头的点和空白、结尾的点和空白（Windows 会吃掉），留前 [`NAME`] 个字符；洗完是空的不接。
pub(crate) fn file_name(msg: &str, index: usize, name: Option<&str>) -> String {
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .filter(|char| !char.is_control() && !"/\\:*?\"<>|".contains(*char))
        .collect();
    let cleaned: String = cleaned
        .trim_start_matches(|char: char| char == '.' || char.is_whitespace())
        .trim_end_matches(|char: char| char == '.' || char.is_whitespace())
        .chars()
        .take(NAME)
        .collect();
    let cleaned = cleaned.trim_end_matches(|char: char| char == '.' || char.is_whitespace());
    if cleaned.is_empty() {
        format!("{msg}-{index}")
    } else {
        format!("{msg}-{index}-{cleaned}")
    }
}

/// 把 `bytes` 存成工作区 `cwd`（`~` 换成家目录 `home`）下 `qq-files/` 里的 `name`（[`file_name`] 洗过的），交回落到的真实路径。
/// 在阻塞线程里调。
///
/// # Errors
///
/// 工作区不是绝对路径、没有，`qq-files` 不是目录（链接也算不是）、落到了工作区外面、写不了：英文的一句。
pub(crate) fn save(
    cwd: &str,
    home: Option<&Path>,
    name: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let workspace = expand(cwd, home)?;
    if !workspace.is_absolute() {
        return Err(format!("workspace {cwd} is not an absolute path"));
    }
    let workspace = std::fs::canonicalize(&workspace)
        .map_err(|error| format!("workspace {}: {error}", workspace.display()))?;
    let dir = workspace.join(QQ_FILES);
    match std::fs::symlink_metadata(&dir) {
        Ok(found) if found.is_dir() => {}
        Ok(_) => return Err(format!("{QQ_FILES} is not a folder")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&dir).map_err(|error| format!("{QQ_FILES}: {error}"))?;
        }
        Err(error) => return Err(format!("{QQ_FILES}: {error}")),
    }
    let dir = std::fs::canonicalize(&dir).map_err(|error| format!("{QQ_FILES}: {error}"))?;
    if !dir.starts_with(&workspace) {
        return Err(format!("{QQ_FILES} is outside the workspace"));
    }
    let target = dir.join(name);
    if target.parent() != Some(dir.as_path()) {
        return Err("the file name is not a plain name".to_string());
    }
    if std::fs::symlink_metadata(&target).is_ok_and(|found| found.is_dir()) {
        return Err(format!("{name} is a folder"));
    }
    let temp = dir.join(format!(".{}.part", unique()));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&temp, &target));
    if let Err(error) = written {
        if std::fs::remove_file(&temp).is_err() {
            // 没建起来，或者已经改了名：没有要清的。
        }
        return Err(format!("{name}: {error}"));
    }
    Ok(target)
}

/// 工作区的路径：`~`、`~/…`（Windows 上 `~\…` 也认）换成家目录；别的照原样（`~别人` 也是，之后当相对路径拒掉）。家目录读不出
/// 的说清楚。
fn expand(cwd: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let rest = match cwd.strip_prefix('~') {
        None => return Ok(PathBuf::from(cwd)),
        Some("") => "",
        Some(rest) => match rest.strip_prefix(['/', '\\']) {
            Some(rest) => rest,
            // `~别人`：不认。
            None => return Ok(PathBuf::from(cwd)),
        },
    };
    let home = home.ok_or("no home folder to expand ~")?;
    Ok(home.join(rest))
}

/// 临时文件名里的一段：16 位随机的十六进制；取不到随机数的用进程号和此刻的纳秒（同 `core.rs` 的编号前缀）。
fn unique() -> String {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        Ok(()) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        Err(_) => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            format!("{:x}{nanos:x}", std::process::id())
        }
    }
}

#[cfg(test)]
mod tests;
