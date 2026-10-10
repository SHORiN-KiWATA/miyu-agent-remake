//! 往输入框里放一个本机的文件（蓝图 `tui.md`「输入框」第 12 条、「`@` 文件列表」第 5 条）：拖进来的、`@` 列表里选的都走
//! 这里。图片、PDF、音频、视频是附件块（模型收不了的那几种也收成文件块），别的文件、目录是文件块。

use std::path::PathBuf;

use super::{InputBox, dropped};

impl InputBox {
    /// 在光标处放一个文件：认得出种类的收成附件，别的收成文件块（发出去换回路径）。
    pub fn put_file(&mut self, path: PathBuf) {
        let kind = self
            .attach_rule
            .kind_of(&path)
            .filter(|_| path.is_file())
            .map(str::to_string);
        match kind {
            Some(kind) => self.attach(path, &kind),
            None => self.put_path(path),
        }
    }

    /// 收成文件块：块上写文件名，发出去换回路径。
    pub(super) fn put_path(&mut self, path: PathBuf) {
        let label = self.attach_rule.file_label(&path);
        self.editor.insert_file(label, dropped::quoted(&path), path);
    }

    /// 模型收不了这一种：记下要提示（同一种只提示一次），交回 `true`（「照模型收」）。
    pub(super) fn refuses(&mut self, kind: &str) -> bool {
        let refused = self.refused.iter().any(|k| k == kind);
        if refused && !self.demoted.iter().any(|k| k == kind) {
            self.demoted.push(kind.to_string());
        }
        refused
    }

    /// `@` 列表选中了一个：从 `start`（`@` 在哪）到光标的那个词换成一块，后面补一个空格。
    pub fn take_mention(&mut self, start: usize, path: PathBuf) {
        let end = self.editor.cursor();
        self.editor.select(start, end);
        self.put_file(path);
        self.editor.insert(" ");
    }

    /// `@` 列表里按 Tab 进了一个目录：那个词换成 `text`（`@目录/`），接着列这个目录。
    pub fn retype_mention(&mut self, start: usize, text: &str) {
        let end = self.editor.cursor();
        self.editor.select(start, end);
        self.editor.insert(text);
    }
}
