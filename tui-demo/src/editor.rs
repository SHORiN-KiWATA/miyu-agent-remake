//! 本机的文本文件在这个终端里用编辑器打开（蓝图 `tui.md`「她的回答：Markdown」第 10 条）：用哪个编辑器、是不是文本
//! 文件、怎么跑。界面让出、收回屏幕在 `main.rs`。

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

/// 看开头多少字节认是不是文本。
const SNIFF: u64 = 8 * 1024;

/// 用哪个编辑器：`$VISUAL`，没有的用 `$EDITOR`；空的不算。Windows 上不用（照旧交给系统）。
pub fn command(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    if !cfg!(unix) {
        return None;
    }
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(var)
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty())
}

/// 是本机的文本文件：普通文件，开头 8 KiB 里没有空字节、是 UTF-8（最后一个字被截在半中间的也算）。读不了的不算。
pub fn is_text(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let mut head = Vec::new();
    let read = File::open(path).and_then(|f| f.take(SNIFF).read_to_end(&mut head));
    if read.is_err() || head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(&head) {
        Ok(_) => true,
        // 截在一个字的半中间：`error_len` 是 `None`。
        Err(e) => e.error_len().is_none(),
    }
}

/// 用 `editor` 打开 `file`，等它退出。`editor` 可以带参数（`code --wait`），照 shell 的样子拆。
///
/// # Errors
///
/// 起不来 `sh`。
pub fn run(editor: &str, file: &Path) -> io::Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("miyu-edit")
        .arg(file)
        .status()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{command, is_text};

    #[test]
    fn visual_comes_before_editor_and_blanks_do_not_count() {
        let env = |pairs: &[(&str, &str)]| {
            let map: HashMap<String, String> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            move |name: &str| map.get(name).cloned()
        };
        if !cfg!(unix) {
            return;
        }
        assert_eq!(command(env(&[("EDITOR", "vim")])).as_deref(), Some("vim"));
        let both = env(&[("VISUAL", "nvim"), ("EDITOR", "vim")]);
        assert_eq!(command(both).as_deref(), Some("nvim"));
        assert_eq!(
            command(env(&[("VISUAL", "  "), ("EDITOR", "vim")])).as_deref(),
            Some("vim")
        );
        assert_eq!(command(env(&[])), None);
    }

    #[test]
    fn text_is_utf8_without_nul_bytes() {
        let dir = std::env::temp_dir().join(format!("miyu-editor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        assert!(is_text(&file("a.md", "# 标题\n正文".as_bytes())));
        assert!(is_text(&file("empty.txt", b"")));
        assert!(
            !is_text(&file("a.png", b"\x89PNG\r\n\x1a\n\0\0\0")),
            "有空字节"
        );
        assert!(!is_text(&file("a.bin", b"\xff\xfe\xfd abc")), "不是 UTF-8");
        // 开头 8 KiB 正好截在一个汉字的半中间，照样是文本。
        let mut long = "a".repeat(8 * 1024 - 1).into_bytes();
        long.extend("字".as_bytes());
        assert!(is_text(&file("long.txt", &long)));
        assert!(!is_text(&dir), "目录不算");
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }
}
