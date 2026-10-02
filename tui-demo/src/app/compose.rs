//! `Ctrl+G`：用编辑器写输入框里的话（蓝图 `tui.md`「按键」）。把输入框的字写进一份临时文件，主循环让出终端给
//! 编辑器（和点开本机的文本文件同一条路，`main.rs` 的 `edit`），退出以后读回来放进输入框，块照样子接回去。

use std::path::PathBuf;

use super::App;
use crate::input::Draft;

/// 正在编辑器里写的那句：临时文件、写之前输入框里的样子（块照它接回去）。
#[derive(Debug)]
pub struct Composing {
    file: PathBuf,
    before: Draft,
}

impl App {
    /// 按了 `Ctrl+G`：写临时文件，交给主循环开编辑器。编辑器照 `$VISUAL`、`$EDITOR`，都没有的用 `vi`；Windows 上
    /// 没有（`editor.rs`）。
    pub(super) fn compose(&mut self) {
        let Some(editor) = self
            .editor
            .clone()
            .or_else(|| cfg!(unix).then(|| "vi".to_string()))
        else {
            return;
        };
        let before = self.input.draft();
        let file = std::env::temp_dir().join(format!("miyu-prompt-{}.md", std::process::id()));
        if let Err(e) = std::fs::write(&file, &before.text) {
            let note = self
                .config
                .text
                .open
                .failed
                .replace("{reason}", &e.to_string());
            self.hint(note, false);
            return;
        }
        self.composing = Some(Composing {
            file: file.clone(),
            before,
        });
        self.edit = Some((editor, file));
    }

    /// 编辑器退出了：是 `Ctrl+G` 开的就读回来放进输入框（末尾多出来的那个换行去掉），临时文件删掉。交回是不是它。
    pub(super) fn composed(&mut self) -> bool {
        let Some(Composing { file, before }) = self.composing.take() else {
            return false;
        };
        if let Ok(text) = std::fs::read_to_string(&file) {
            let text = text.strip_suffix('\n').unwrap_or(&text);
            self.input.editor.set_draft(Draft::rebind(text, &before));
        }
        drop(std::fs::remove_file(&file));
        true
    }
}
