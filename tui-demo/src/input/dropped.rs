//! 拖进终端的文件（蓝图 `tui.md`「输入框」第 12 条）：终端把文件的路径当一次粘贴送进来。kitty 一行一个、不加引号；
//! 别的终端多是加引号、反斜杠、空格隔开，也有 `file://`（`%20` 这样的转义）的。粘进来的全是本机现成的文件、目录的，
//! 交回一项一项，由输入框放：认得出种类（图片、PDF、音频、视频，`attachments.json`）的收成附件，别的收成文件块
//! （发出去换回路径，她要看自己用工具读）。有一个不是现成的交回 `None`，照字原样粘。

use std::path::{Path, PathBuf};

use super::attach::AttachRule;

/// 拖进来的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dropped {
    /// 认得出种类的文件：收成附件。
    File(PathBuf, String),
    /// 别的文件、目录：收成文件块，发出去换回路径。
    Path(PathBuf),
}

/// 粘进来的一段字是不是拖进来的一批文件：是的交回一项一项，不是的交回 `None`。`home` 是家目录（`~` 换成它）。
pub fn dropped(text: &str, home: Option<&str>, rule: &AttachRule) -> Option<Vec<Dropped>> {
    let items =
        paths(text, home)?
            .into_iter()
            .map(|p| match rule.kind_of(&p).filter(|_| p.is_file()) {
                Some(kind) => Dropped::File(p.clone(), kind.to_string()),
                None => Dropped::Path(p),
            });
    Some(items.collect())
}

/// 照路径写进输入框的样子：带空白、引号的加单引号（里面的单引号照 shell 的样子写成 `'\''`）。
pub fn quoted(path: &Path) -> String {
    let text = path.display().to_string();
    if text.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') {
        format!("'{}'", text.replace('\'', r"'\''"))
    } else {
        text
    }
}

/// 粘进来的一段字里的路径：一行一行认，整行就是一个现成的文件、目录的照一个算（kitty），不是的照 shell 的样子切。
/// 有一段不是本机现成的，交回 `None`。
fn paths(text: &str, home: Option<&str>) -> Option<Vec<PathBuf>> {
    let found = |word: &str| path(word, home).filter(|p| p.exists());
    let mut out = Vec::new();
    for line in text
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        match found(line) {
            Some(p) => out.push(p),
            None => {
                for word in words(line)? {
                    out.push(found(&word)?);
                }
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// 照 shell 的样子切成几段：空白隔开，单引号、双引号里的原样，反斜杠转义下一个字。引号没收尾的不算。
fn words(text: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = text.chars();
    let mut open = false;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => word.extend(chars.next()),
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                open = true;
            }
            (None, '\\') => {
                word.extend(chars.next());
                open = true;
            }
            (None, c) if c.is_whitespace() => {
                if open || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                open = false;
            }
            (None, c) => {
                word.push(c);
                open = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if open || !word.is_empty() {
        out.push(word);
    }
    Some(out)
}

/// 一段换成路径：`file://` 的去掉头、解开 `%XX`；`~` 换成家目录；只认绝对路径。
fn path(word: &str, home: Option<&str>) -> Option<PathBuf> {
    let text = match word.strip_prefix("file://") {
        Some(rest) => unescape(rest.strip_prefix("localhost").unwrap_or(rest))?,
        None => word.to_string(),
    };
    let text = match (text.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            format!("{home}{rest}")
        }
        _ => text,
    };
    let path = PathBuf::from(text);
    path.is_absolute().then_some(path)
}

/// 解开 `%XX`：不成对的、解出来不是 UTF-8 的不算。
fn unescape(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Dropped, dropped, quoted, words};
    use crate::input::tests::attach_rule;

    #[test]
    fn words_follow_the_shell() {
        assert_eq!(words("/a/b.png").unwrap(), ["/a/b.png"]);
        assert_eq!(
            words("'/a/b c.png' /d.png").unwrap(),
            ["/a/b c.png", "/d.png"]
        );
        assert_eq!(words(r"/a/b\ c.png").unwrap(), ["/a/b c.png"]);
        assert_eq!(words(r#""/a/说明 1.png""#).unwrap(), ["/a/说明 1.png"]);
        assert!(words("'/a/b.png").is_none(), "引号没收尾");
    }

    #[test]
    fn paths_with_spaces_or_quotes_are_single_quoted() {
        assert_eq!(quoted(Path::new("/a/b.txt")), "/a/b.txt");
        assert_eq!(quoted(Path::new("/a/b c.txt")), "'/a/b c.txt'");
        assert_eq!(quoted(Path::new("/a/it's.txt")), r"'/a/it'\''s.txt'");
    }

    #[test]
    fn local_files_come_in_one_by_one_with_the_known_kinds_as_attachments() {
        let dir = std::env::temp_dir().join(format!("miyu-drop-{}", std::process::id()));
        let sub = dir.join("子目录");
        std::fs::create_dir_all(&sub).unwrap();
        let shot = dir.join("截图 1.png");
        let doc = dir.join("报告 1.pdf");
        let note = dir.join("说明.txt");
        for f in [&shot, &doc, &note] {
            std::fs::write(f, b"x").unwrap();
        }
        let rule = attach_rule();
        let found = |text: &str| dropped(text, None, &rule);
        let file = |p: &PathBuf, kind: &str| Dropped::File(p.clone(), kind.to_string());
        let shot_text = shot.display().to_string();
        // 别的终端：带引号的、带反斜杠的、file:// 的都认。
        assert_eq!(
            found(&format!("'{shot_text}'")),
            Some(vec![file(&shot, "image")])
        );
        assert_eq!(
            found(&shot_text.replace(' ', r"\ ")),
            Some(vec![file(&shot, "image")])
        );
        let uri = format!("file://{}", shot_text.replace(' ', "%20"));
        let encoded = uri.replace("截图", "%E6%88%AA%E5%9B%BE");
        assert_eq!(found(&encoded), Some(vec![file(&shot, "image")]));
        // kitty：一行一个、不加引号，名字带空格的整行是一个。
        let kitty = format!("{}\n{}\n{}\n", doc.display(), note.display(), sub.display());
        assert_eq!(
            found(&kitty),
            Some(vec![
                file(&doc, "pdf"),
                Dropped::Path(note.clone()),
                Dropped::Path(sub.clone())
            ]),
            "别的文件、目录照路径"
        );
        // 别的文件单拖进来也收成文件块（2026-09-30 项目主人定：所有文件都做成能点开的块）。
        assert_eq!(
            found(&note.display().to_string()),
            Some(vec![Dropped::Path(note.clone())])
        );
        // 有一个不是现成的、夹着别的字：照字原样粘。
        assert_eq!(
            found(&format!("{}\n/nonexistent/a.png", doc.display())),
            None
        );
        assert_eq!(found("/nonexistent/a.png"), None);
        assert_eq!(found(&format!("看看 '{shot_text}'")), None);
        assert_eq!(found(""), None);
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }
}
