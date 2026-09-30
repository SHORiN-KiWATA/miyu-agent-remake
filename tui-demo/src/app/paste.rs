//! 粘贴和复制（蓝图 `tui.md`「按键」`Ctrl+V`、「输入框」第 11、12 条、「斜杠命令」`/copy`）：粘进来的字去哪、剪贴板里
//! 的图收成附件，附件怎么编号、发出去在正文里记成什么，复制以后提示一句。

use super::App;
use crate::clipboard;
use crate::config::Config;
use crate::input::{AttachKind, AttachRule, Draft};
use crate::transcript::Chip;

/// 剪贴板里的截图算哪一种附件（`attachments.json` 里的名字）。
const IMAGE: &str = "image";

/// 照配置定附件的规矩：每一种认哪些扩展名（`attachments.json`）、块上写什么（`attachment_labels`）。
pub(super) fn attach_rule(config: &Config) -> AttachRule {
    let kinds = config.attachments.kinds.iter().map(|k| AttachKind {
        name: k.name.clone(),
        extensions: k.extensions.clone(),
        label: config
            .text
            .attachment_labels
            .get(&k.name)
            .cloned()
            .unwrap_or_default(),
    });
    AttachRule {
        kinds: kinds.collect(),
    }
}

/// 发出去的一句在正文里记成的块：块上的字、原文，附件另记种类（「正文」第 2 条、「输入框」第 12 条）。
pub(super) fn chips(draft: &Draft) -> Vec<Chip> {
    draft
        .blocks
        .iter()
        .map(|b| Chip {
            label: draft.text[b.start..b.end].to_string(),
            full: b.text.clone(),
            kind: b.attachment.as_ref().map(|a| a.kind.clone()),
        })
        .collect()
}

/// 被退回的一句拼回输入框用的粘贴块：附件不跟着回来，块上的字照字留着（「输入框」第 12 条）。
pub(super) fn pasted(chips: Vec<Chip>) -> Vec<(String, String)> {
    chips
        .into_iter()
        .filter(|c| !c.attachment())
        .map(|c| (c.label, c.full))
        .collect()
}

impl App {
    /// 粘贴进来的字（终端送来的、`Ctrl+V` 读剪贴板的）：历史列表开着的进「历史：」，抽屉开着的进抽屉里写的字，
    /// 别的进输入框（大段的收成一块，`tui.md`「输入框」第 11 条）。
    pub(super) fn paste(&mut self, text: &str) {
        if self.history.open {
            self.history.type_text(&text.replace(['\r', '\n'], " "));
        } else if self.drawers.open() {
            self.drawer_paste(text);
        } else {
            self.input.paste(text);
        }
    }

    /// `Ctrl+V`：剪贴板里是图的收成一块附件（输入框、没开列表和抽屉时；`tui.md`「输入框」第 12 条）；不然读字，照粘贴
    /// 处理；读不到、是空的，提示一句（「按键」）。
    pub(super) fn paste_clipboard(&mut self) {
        if !self.history.open && !self.drawers.open() {
            let dir = self.staging.as_ref().map(clipboard::Staging::dir);
            if let Some(file) = dir.and_then(clipboard::read_image) {
                self.input.attach(file, IMAGE);
                return;
            }
        }
        match clipboard::read() {
            Ok(text) if text.is_empty() => {
                self.hint(self.config.text.clipboard_empty.clone(), false);
            }
            Ok(text) => self.paste(&text),
            Err(_) => self.hint(self.config.text.clipboard_unreadable.clone(), false),
        }
    }

    /// 输入框里的附件照正文里这一种已有几个接着编号（每一帧画之前，「输入框」第 12 条）。
    pub(super) fn renumber_attachments(&mut self) {
        self.input.renumber(self.transcript.attachment_counts());
    }

    pub(super) fn copy(&mut self, text: &str) {
        let words = &self.config.text;
        let (message, good) = match clipboard::copy(text) {
            Ok(()) => (
                words
                    .copied
                    .replace("{count}", &text.chars().count().to_string()),
                true,
            ),
            Err(e) => (words.copy_failed.replace("{reason}", &e.to_string()), false),
        };
        self.hint(message, good);
    }
}
