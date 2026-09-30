//! `/language`：在中英文之间切换界面语言（蓝图 `tui.md`「界面语言」）。界面上的字、命令的说明、运行状态行的词、工具的
//! 显示名换成另一种；已经画在正文里的不改，新画的照新语言。只管这一次启动。

use super::App;
use crate::config::Config;

impl App {
    /// 换到另一种语言，提示一句。
    pub(super) fn switch_language(&mut self) {
        let next = self.language.other();
        let Ok(fresh) = Config::load(next) else {
            return;
        };
        self.config.text = fresh.text;
        self.config.commands = fresh.commands;
        self.config.pulse = fresh.pulse;
        self.human = next.human();
        self.language = next;
        // 排好的行里有旧语言的字（时间线的标题、工具的显示名）：重排。
        *self.row_cache.borrow_mut() = Default::default();
        self.hint(self.config.text.language_switched.clone(), false);
    }
}
