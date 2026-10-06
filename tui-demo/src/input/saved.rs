//! 输入历史记在文件里（蓝图「输入历史列表」第 8 条，2026-10-07 项目主人定：所有会话一起、重启以后还在）：每发一句追加一行
//! JSON，起来时读回；粘贴块、附件、文件块原样记。写不进、读不懂的不管，照常用。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{Attachment, Block, Draft, Sent};

/// 记输入历史的那个文件；没有的（单元测试、找不到数据根）什么都不记。
#[derive(Debug, Default)]
pub struct Saved {
    path: Option<PathBuf>,
}

/// 文件里的一行。
#[derive(Debug, Serialize, Deserialize)]
struct Line {
    /// 什么时候发的：毫秒。
    at: u64,
    /// 输入框里的样子。
    text: String,
    /// 粘贴块、附件、文件块。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blocks: Vec<SavedBlock>,
}

/// 一块。
#[derive(Debug, Serialize, Deserialize)]
struct SavedBlock {
    start: usize,
    end: usize,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
}

impl Saved {
    /// 记在 `path`。
    pub fn at(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    /// 读回最近的 `keep` 条，旧的在前；连着一样的只留一条。行数多过两倍的重写一遍，只留这几条。
    pub fn load(&self, keep: usize) -> Vec<Sent> {
        let Some(path) = &self.path else {
            return Vec::new();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let mut sent: Vec<Sent> = Vec::new();
        for line in &lines {
            let Some(one) = serde_json::from_str::<Line>(line).ok().map(Line::into_sent) else {
                continue;
            };
            if sent.last().is_none_or(|last| last.draft != one.draft) {
                sent.push(one);
            }
        }
        let cut = sent.len().saturating_sub(keep);
        sent.drain(..cut);
        if lines.len() > keep.saturating_mul(2) {
            self.rewrite(path, &sent);
        }
        sent
    }

    /// 追加一句。
    pub fn append(&self, sent: &Sent) {
        if let Some(path) = &self.path {
            let line = Line::of(sent);
            if let Ok(json) = serde_json::to_string(&line)
                && let Ok(mut file) = open(path, true)
                && writeln!(file, "{json}").is_err()
            {
                // 写不进（满了、没权限）：这一句不记，照常用。
            }
        }
    }

    fn rewrite(&self, path: &Path, sent: &[Sent]) {
        let body: String = sent
            .iter()
            .filter_map(|s| serde_json::to_string(&Line::of(s)).ok())
            .map(|l| l + "\n")
            .collect();
        let temp = path.with_extension("jsonl.tmp");
        let written = open(&temp, false).and_then(|mut f| f.write_all(body.as_bytes()));
        if written.is_ok() && std::fs::rename(&temp, path).is_err() {
            // 换不上：下次起来再试。
        }
    }
}

/// 打开（没有的建上，连目录）：只给自己读写。`append` 是接在后面，不然从头写。
fn open(path: &Path, append: bool) -> std::io::Result<std::fs::File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl Line {
    fn of(sent: &Sent) -> Self {
        let at = sent
            .at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let blocks = sent
            .draft
            .blocks
            .iter()
            .map(|b| SavedBlock {
                start: b.start,
                end: b.end,
                text: b.text.clone(),
                path: b.path.clone(),
                file: b.attachment.as_ref().map(|a| a.file.clone()),
                kind: b.attachment.as_ref().map(|a| a.kind.clone()),
            })
            .collect();
        Self {
            at,
            text: sent.draft.text.clone(),
            blocks,
        }
    }

    /// 读成发过的一句；块的位置对不上字的（手改坏了）不要块，字照留。
    fn into_sent(self) -> Sent {
        let text = self.text;
        let fits = |b: &SavedBlock| {
            b.start <= b.end
                && b.end <= text.len()
                && text.is_char_boundary(b.start)
                && text.is_char_boundary(b.end)
        };
        let blocks = if self.blocks.iter().all(fits) {
            self.blocks
                .into_iter()
                .map(|b| Block {
                    start: b.start,
                    end: b.end,
                    text: b.text,
                    attachment: b.file.map(|file| Attachment {
                        file,
                        kind: b.kind.unwrap_or_default(),
                    }),
                    path: b.path,
                })
                .collect()
        } else {
            Vec::new()
        };
        Sent {
            draft: Draft { text, blocks },
            at: UNIX_EPOCH + Duration::from_millis(self.at),
        }
    }
}

/// 现在的时刻（发一句时记）。
pub fn now() -> SystemTime {
    SystemTime::now()
}

#[cfg(test)]
mod tests {
    use super::{Saved, now};
    use crate::input::{Block, Draft, Sent};

    fn sent(text: &str) -> Sent {
        Sent {
            draft: Draft::plain(text),
            at: now(),
        }
    }

    #[test]
    fn sent_lines_come_back_after_a_restart_with_their_blocks() {
        let dir = std::env::temp_dir().join(format!("miyu-tui-saved-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        let saved = Saved::at(Some(path.clone()));
        saved.append(&sent("第一句"));
        let mut pasted = sent("看看 [粘贴 3 行]");
        pasted.draft.blocks.push(Block {
            start: 7,
            end: 21,
            text: "a\nb\nc".into(),
            attachment: None,
            path: None,
        });
        saved.append(&pasted);
        saved.append(&sent("看看 [粘贴 3 行]"));
        std::fs::write(
            &path,
            std::fs::read_to_string(&path).unwrap() + "坏的一行\n",
        )
        .unwrap();
        let back = Saved::at(Some(path.clone())).load(10);
        let texts: Vec<&str> = back.iter().map(|s| s.draft.text.as_str()).collect();
        assert_eq!(
            texts,
            ["第一句", "看看 [粘贴 3 行]", "看看 [粘贴 3 行]"],
            "读不懂的行不管"
        );
        assert_eq!(back[1].draft.blocks.len(), 1, "粘贴块原样回来");
        assert_eq!(back[1].draft.blocks[0].text, "a\nb\nc");
        assert!(back[2].draft.blocks.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "只给自己读写");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_latest_are_kept_and_a_long_file_is_rewritten() {
        let dir = std::env::temp_dir().join(format!("miyu-tui-keep-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        let saved = Saved::at(Some(path.clone()));
        for i in 0..7 {
            saved.append(&sent(&format!("第 {i} 句")));
        }
        let back = saved.load(3);
        let texts: Vec<&str> = back.iter().map(|s| s.draft.text.as_str()).collect();
        assert_eq!(texts, ["第 4 句", "第 5 句", "第 6 句"]);
        let lines = std::fs::read_to_string(&path).unwrap().lines().count();
        assert_eq!(lines, 3, "多过两倍：重写成只留这几条");
        assert!(Saved::default().load(3).is_empty(), "没有文件的什么都不记");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
