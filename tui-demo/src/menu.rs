//! 斜杠命令列表的状态：开没开、选中哪一条、露出哪一段。
//!
//! 输入框里的字是 `/` 开头的命令名时打开（`13-终端界面.md` 第十节）；`Esc` 关掉以后，字变了才再打开。
//! 滚动让选中的那一条停在正中间，到了开头、结尾才往边上走。

/// 列表的状态。
#[derive(Debug, Default)]
pub struct Menu {
    /// 选中的是筛出来的第几条。
    pub selected: usize,
    /// 上一次照什么字筛的：字变了，选中回到第一条。
    typed: Option<String>,
    /// 按 `Esc` 时输入框里的字；字没变就一直关着。
    dismissed: Option<String>,
}

impl Menu {
    /// 照输入框里的字和筛出来的条数，定开不开。字变了，选中回到第一条。
    pub fn sync(&mut self, text: &str, typed: Option<&str>, matches: usize) -> bool {
        let Some(typed) = typed else {
            // 不是命令了（`/` 删掉了）：Esc 关掉的那一次也算过去了，再打 `/` 照常打开。
            self.typed = None;
            self.dismissed = None;
            return false;
        };
        if self.typed.as_deref() != Some(typed) {
            self.typed = Some(typed.to_string());
            self.selected = 0;
        }
        if self.dismissed.as_deref() != Some(text) {
            self.dismissed = None;
        }
        self.selected = self.selected.min(matches.saturating_sub(1));
        matches > 0 && self.dismissed.is_none() && !text.contains(char::is_whitespace)
    }

    /// 往上、往下挪一条，到头就停。
    pub fn step(&mut self, down: bool, matches: usize) {
        self.selected = if down {
            (self.selected + 1).min(matches.saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
    }

    /// 关掉，直到输入框里的字变了。
    pub fn dismiss(&mut self, text: &str) {
        self.dismissed = Some(text.to_string());
    }
}

/// 露出哪一段：第一条露出的是第几条。选中的停在正中间，两头不留空。
pub fn window(selected: usize, matches: usize, rows: usize) -> usize {
    let top = selected.saturating_sub(rows / 2);
    top.min(matches.saturating_sub(rows))
}

#[cfg(test)]
mod tests {
    use super::{Menu, window};

    #[test]
    fn the_selection_scrolls_from_the_middle() {
        // 13 条露 5 条：选到第 2 条（正中间）以前不滚，之后选中的一直在正中间，最后两条往下走。
        let tops: Vec<_> = (0..13).map(|s| window(s, 13, 5)).collect();
        assert_eq!(tops, vec![0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 8]);
        assert_eq!(window(2, 3, 5), 0, "放得下的不滚");
    }

    #[test]
    fn esc_keeps_it_closed_until_the_text_changes() {
        let mut menu = Menu::default();
        assert!(menu.sync("/u", Some("u"), 2));
        menu.dismiss("/u");
        assert!(!menu.sync("/u", Some("u"), 2));
        assert!(menu.sync("/un", Some("un"), 1));
    }

    #[test]
    fn deleting_the_slash_and_typing_it_again_reopens() {
        let mut menu = Menu::default();
        assert!(menu.sync("/", Some(""), 13));
        menu.dismiss("/");
        // 删掉 `/`：不是命令了。再打一个 `/`：字变过了，照常打开。
        assert!(!menu.sync("", None, 0));
        assert!(menu.sync("/", Some(""), 13));
    }

    #[test]
    fn new_text_goes_back_to_the_first() {
        let mut menu = Menu::default();
        menu.sync("/", Some(""), 13);
        menu.step(true, 13);
        menu.step(true, 13);
        assert_eq!(menu.selected, 2);
        menu.sync("/r", Some("r"), 3);
        assert_eq!(menu.selected, 0);
    }
}
