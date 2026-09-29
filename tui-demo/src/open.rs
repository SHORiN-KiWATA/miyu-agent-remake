//! 用系统的打开方式开一个地址（蓝图 `tui.md`「她的回答：Markdown」第 10 条）：Linux `xdg-open`，
//! macOS `open`，Windows `start`。不等它、不管它的输出；只开 `http`、`https`、`file` 和本机存在的文件，别的协议不碰。

use std::io;
use std::process::{Command, Stdio};

/// 开 `url`。
///
/// # Errors
///
/// 协议不认、起不来打开它的程序。
pub fn open(url: &str) -> io::Result<()> {
    let Some(target) = target(url) else {
        return Err(io::Error::other(format!("不开这种地址：{url}")));
    };
    let mut command = opener(&target);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // 另起一个线程等它退出，免得留下僵尸进程；等的结果没人要。
    std::thread::spawn(move || child.wait().is_ok());
    Ok(())
}

/// 交给打开程序的是什么：认得的协议照原样；本机的路径（和图片同一套认法，`local.rs`）换成文件的完整路径，
/// 文件得在；别的协议不开。
fn target(url: &str) -> Option<String> {
    if ["http://", "https://", "file://"]
        .iter()
        .any(|s| url.starts_with(s))
    {
        return Some(url.to_string());
    }
    if has_scheme(url) {
        return None;
    }
    let path = crate::local::resolve(url)?;
    path.exists().then(|| path.display().to_string())
}

/// 开头是「协议:」：字母打头、两个字以上（`C:` 这种盘符不算），只有字母、数字、`+`、`-`、`.`。
fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(head, _)| {
        head.len() > 1
            && head.starts_with(|c: char| c.is_ascii_alphabetic())
            && head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut c = Command::new("open");
    c.arg(url);
    c
}

#[cfg(windows)]
fn opener(url: &str) -> Command {
    let mut c = Command::new("cmd");
    c.args(["/C", "start", "", url]);
    c
}

#[cfg(all(unix, not(target_os = "macos")))]
fn opener(url: &str) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(url);
    c
}

#[cfg(test)]
mod tests {
    use super::open;

    #[test]
    fn other_schemes_are_refused() {
        assert!(open("javascript:alert(1)").is_err());
        assert!(open("ssh://host").is_err());
    }

    #[test]
    fn local_paths_that_exist_are_opened_as_files() {
        use super::target;
        let here = std::env::current_dir().unwrap();
        let file = here.join("Cargo.toml");
        assert_eq!(
            target(file.to_str().unwrap()),
            Some(file.display().to_string())
        );
        assert_eq!(target("Cargo.toml"), Some(file.display().to_string()));
        assert_eq!(target("https://a.com"), Some("https://a.com".to_string()));
        assert_eq!(target("/没有/这个/文件.png"), None);
        assert_eq!(target("mailto:a@b.c"), None);
    }
}
