//! 附件先传到桥（蓝图 `web.md`「附件」第 2 条）：浏览器拿不到文件在本机的哪儿，核心的 `blob.put` 要路径（`data` 放在一行 JSON
//! 里，一行最长 1 MiB，大的传不了，`protocol.md`）。页面把文件 `POST` 到 `/upload?k=口令&name=文件名`，桥存进这台机器的临时目录
//! （`<临时目录>/miyu-web-uploads/<随机>/<文件名>`），交回路径；页面拿它 `blob.put`，核心存好了发 `web.upload_done` 让桥删掉。
//! 桥起来时清掉放了一天以上的（页面没来得及说的）。临时目录不在数据根里，核心读得到（`fs.md` 第一节）。

use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::Site;
use crate::media::plain;

/// 一个最多多大：和核心的 `blob.put` 一样（20 MiB，`protocol.md`），多的核心也不收，桥先挡掉、不整份收下来。
pub const LIMIT: u64 = 20 * 1024 * 1024;
/// 放了多久还没被说删的，桥起来时清掉。
const STALE: Duration = Duration::from_secs(24 * 60 * 60);
/// 每个附件一层随机目录，名字是这么多字节的十六进制。
const ID_LEN: usize = 12;
/// 文件名最长多少字节（常见文件系统的上限）。
const NAME_MAX: usize = 255;

/// 放附件的地方：这台机器的临时目录下一层。
pub fn root() -> PathBuf {
    std::env::temp_dir().join("miyu-web-uploads")
}

/// 文件名只留最后一段：不带路径、不是 `.`、`..`、不含控制字符、不超长；不合的是 `None`。
pub fn safe_name(name: &str) -> Option<String> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if last.is_empty() || last == "." || last == ".." || last.len() > NAME_MAX || last.chars().any(char::is_control) {
        return None;
    }
    Some(last.to_string())
}

/// 收一个附件：`head` 是读到的请求头，`already` 是和请求头一起读进来的那一截正文。
///
/// # Errors
///
/// 读写连接出错。
pub async fn serve(stream: &mut TcpStream, site: &Site, query: &str, head: &str, already: &[u8]) -> io::Result<()> {
    let q = crate::media::params(query);
    if q.get("k") != Some(&site.key) {
        return plain(stream, "403 Forbidden", "口令不对").await;
    }
    let Some(name) = q.get("name").and_then(|n| safe_name(n)) else {
        return plain(stream, "400 Bad Request", "文件名不对").await;
    };
    let Some(len) = content_length(head) else {
        return plain(stream, "411 Length Required", "要 Content-Length").await;
    };
    if len > LIMIT {
        return plain(stream, "413 Content Too Large", "附件太大（最多 20 MiB）").await;
    }
    let len = usize::try_from(len).unwrap_or(usize::MAX);
    let mut body = already.to_vec();
    body.truncate(len);
    let mut chunk = vec![0u8; 64 * 1024];
    while body.len() < len {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return plain(stream, "400 Bad Request", "正文没收齐").await;
        }
        body.extend_from_slice(&chunk[..n.min(len - body.len())]);
    }
    let mut id = [0u8; ID_LEN];
    if getrandom::fill(&mut id).is_err() {
        return plain(stream, "500 Internal Server Error", "取不到随机数").await;
    }
    let dir = root().join(id.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let file = dir.join(&name);
    let saved = async {
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(&file, &body).await
    }
    .await;
    if let Err(e) = saved {
        return plain(stream, "500 Internal Server Error", &format!("存不下：{e}")).await;
    }
    let body = json!({"path": file.display().to_string()}).to_string();
    crate::files::reply(stream, "200 OK", "application/json; charset=utf-8", body.as_bytes()).await
}

/// 请求头里的 `Content-Length`。
fn content_length(head: &str) -> Option<u64> {
    head.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok()).flatten()
    })
}

/// 核心存好了（`web.upload_done`）：删掉那个文件和它那一层随机目录。只删放附件的地方下面的。
///
/// # Errors
///
/// 不在放附件的地方下面的，交回一句说清楚的话。
pub fn done(path: &str) -> Result<Value, String> {
    let file = Path::new(path);
    // 只认 `<放附件的地方>/<随机>/<文件名>`：不带 `..`、`.`，中间那层是桥起的十六进制名字
    let plain = file.components().all(|c| matches!(c, Component::Normal(_) | Component::RootDir | Component::Prefix(_)));
    let dir = file.parent().filter(|d| plain && d.parent() == Some(root().as_path()));
    let ours = dir.and_then(Path::file_name).and_then(|n| n.to_str()).is_some_and(|n| n.len() == ID_LEN * 2 && n.bytes().all(|b| b.is_ascii_hexdigit()));
    let Some(dir) = dir.filter(|_| ours) else {
        return Err("不是附件的路径".to_string());
    };
    std::fs::remove_dir_all(dir).map_err(|e| format!("删不掉：{e}"))?;
    Ok(json!({}))
}

/// 桥起来时：放了一天以上的清掉（页面传了没说删的）。清不掉的不管。
pub fn sweep() {
    let Ok(entries) = std::fs::read_dir(root()) else { return };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let old = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age > STALE);
        if old {
            drop(std::fs::remove_dir_all(entry.path()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_only_the_last_part() {
        assert_eq!(safe_name("报告.pdf").as_deref(), Some("报告.pdf"));
        assert_eq!(safe_name("a/b/c.png").as_deref(), Some("c.png"));
        assert_eq!(safe_name("..\\x.txt").as_deref(), Some("x.txt"));
        assert_eq!(safe_name(".."), None);
        assert_eq!(safe_name(""), None);
        assert_eq!(safe_name("a\nb"), None);
        assert_eq!(safe_name(&"x".repeat(300)), None);
    }

    #[test]
    fn content_length_is_read_from_the_head() {
        assert_eq!(content_length("POST /upload HTTP/1.1\r\ncontent-length: 12\r\n\r\n"), Some(12));
        assert_eq!(content_length("POST /upload HTTP/1.1\r\n\r\n"), None);
    }

    #[test]
    fn done_only_removes_uploads() {
        assert!(done("/etc/passwd").is_err());
        assert!(done(&root().join("..").join("x").display().to_string()).is_err());
        assert!(done(&root().join("not-ours").join("x").display().to_string()).is_err());
        let dir = root().join("0123456789abcdef01234567");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        assert!(done(&dir.join("a.txt").display().to_string()).is_ok());
        assert!(!dir.exists());
    }
}
