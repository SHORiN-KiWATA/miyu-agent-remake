//! 回答里写的本机地址指到哪个文件（蓝图 `tui.md`「她的回答：Markdown」第 10、12 条）：
//! `file://` 去掉头，`~/` 换成家目录，别的相对路径照工作目录找。点链接和画图片用同一套认法。

use std::path::{Path, PathBuf};

/// 家目录里的路径写成 `~/…`（照 `miyu ask`）：时间线的对象、侧边栏的工作目录都照它写。
pub fn home_short(value: &str) -> String {
    match std::env::home_dir().and_then(|home| {
        Path::new(value)
            .strip_prefix(home)
            .ok()
            .map(|r| r.display().to_string())
    }) {
        Some(rest) if rest.is_empty() => "~".into(),
        Some(rest) => format!("~/{rest}"),
        None => value.to_string(),
    }
}

/// 照给定的家目录、工作目录认。
pub fn path(url: &str, home: Option<&Path>, cwd: &Path) -> PathBuf {
    let url = url.strip_prefix("file://").unwrap_or(url);
    if let (Some(rest), Some(home)) = (url.strip_prefix("~/"), home) {
        return home.join(rest);
    }
    let path = Path::new(url);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// 照这个进程的家目录（`HOME`）、工作目录认；工作目录读不出来时交回 `None`。
pub fn resolve(url: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cwd = std::env::current_dir().ok()?;
    Some(path(url, home.as_deref(), &cwd))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::path;

    #[test]
    fn addresses_resolve_to_files() {
        let home = Path::new("/home/me");
        let cwd = Path::new("/work");
        assert_eq!(
            path("~/a.png", Some(home), cwd),
            PathBuf::from("/home/me/a.png")
        );
        assert_eq!(
            path("file:///tmp/b.png", Some(home), cwd),
            PathBuf::from("/tmp/b.png")
        );
        assert_eq!(path("/x/c.png", Some(home), cwd), PathBuf::from("/x/c.png"));
        assert_eq!(
            path("img/d.png", Some(home), cwd),
            PathBuf::from("/work/img/d.png")
        );
    }
}
