//! 附件照模型收（蓝图 `tui.md`「输入框」第 12 条「照模型收」）：这个会话的模型（还没开会话的照默认的聊天模型）收不了的那几种
//! 交给输入框，拖进来、贴进来时收成文件块，提示一句；发出去时照那一刻再认一遍。模型资料照 `model.list`（`effort.rs` 要的
//! 那一份），读不到的都收。

use super::App;

impl App {
    /// 现在的模型收不了的那几种附件。
    pub(super) fn refused_kinds(&self) -> Vec<String> {
        let Some(reference) = self.transcript.model_ref().or(self.default_chat.as_deref()) else {
            return Vec::new();
        };
        self.config
            .attachments
            .kinds
            .iter()
            .map(|k| k.name.clone())
            .filter(|kind| !self.abilities.takes(reference, kind))
            .collect()
    }

    /// 每一帧：把收不了的那几种交给输入框；刚改成文件块的提示一句。
    pub(super) fn sync_refused(&mut self) {
        let refused = self.refused_kinds();
        self.input.set_refused(refused);
        let demoted = self.input.take_demoted();
        self.tell_demoted(&demoted);
    }

    /// 提示「当前模型不收图片，改成了文件路径」；一种都没有的不说。
    pub(super) fn tell_demoted(&mut self, kinds: &[String]) {
        if kinds.is_empty() {
            return;
        }
        let texts = &self.config.text.attach;
        let names: Vec<&str> = kinds
            .iter()
            .map(|k| texts.names.get(k).map_or(k.as_str(), String::as_str))
            .collect();
        let note = texts.refused.replace("{kinds}", &names.join("、"));
        self.hint(note, false);
    }

    /// 发出去以前：附了以后才换的模型，收不了的附件改成文件块写路径。
    pub(super) fn demote_for_send(&mut self, draft: &mut crate::input::Draft) {
        let kinds = draft.demote(&self.refused_kinds());
        self.tell_demoted(&kinds);
    }
}
