//! 斜杠命令列表的状态：开没开、选中哪一条、露出哪一段。
//!
//! 输入框里的字是 `/` 开头的命令名时打开（`13-终端界面.md` 第十节）；`Esc` 关掉以后，字变了才再打开。
//! 按键选的时候滚动让选中的那一条停在正中间，到了开头、结尾才往边上走；鼠标来了钉住露的那一段（[`pin`]），悬停
//! 不挪列表、滚轮一格滚一条。输入历史列表、`@` 文件列表也照这一套（蓝图 `tui.md`「斜杠命令列表」第 3 条）。

/// 列表的状态。
#[derive(Debug, Default)]
pub struct Menu {
    /// 选中的是筛出来的第几条。
    pub selected: usize,
    /// 鼠标钉住的露出来的第一条（[`pin`]）；按键选、字变了放开。
    pub pinned: Option<usize>,
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
            self.pinned = None;
        }
        if self.dismissed.as_deref() != Some(text) {
            self.dismissed = None;
        }
        self.selected = self.selected.min(matches.saturating_sub(1));
        matches > 0 && self.dismissed.is_none() && !text.contains(char::is_whitespace)
    }

    /// 往上、往下挪一条，到头就停；鼠标钉住的放开。
    pub fn step(&mut self, down: bool, matches: usize) {
        self.pinned = None;
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

/// 露出哪一段：鼠标钉住的照它，不然让选中的停在正中间（[`window`]）。
pub fn top(selected: usize, pinned: Option<usize>, matches: usize, rows: usize) -> usize {
    match pinned {
        Some(top) => top.min(matches.saturating_sub(rows)),
        None => window(selected, matches, rows),
    }
}

/// 鼠标在列表上：把现在露的这一段钉住（悬停不挪列表），滚轮再挪 `by` 条，到头就停。
pub fn pin(selected: usize, pinned: &mut Option<usize>, matches: usize, rows: usize, by: isize) {
    let now = top(selected, *pinned, matches, rows);
    let moved = now
        .saturating_add_signed(by)
        .min(matches.saturating_sub(rows));
    *pinned = Some(moved);
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
    fn the_mouse_pins_the_list_and_the_wheel_scrolls_it() {
        // 2026-09-30 项目主人：一悬停就选中、列表跟着挪到正中间，鼠标一靠近就跑；滚轮不滚列表。
        use super::{pin, top};
        let (count, rows) = (13, 5);
        let mut pinned = None;
        assert_eq!(top(0, pinned, count, rows), 0);
        pin(0, &mut pinned, count, rows, 0);
        assert_eq!(top(4, pinned, count, rows), 0, "悬停选中下面那条，列表不动");
        pin(4, &mut pinned, count, rows, 1);
        assert_eq!(top(4, pinned, count, rows), 1, "滚轮一格滚一条");
        pin(4, &mut pinned, count, rows, 100);
        assert_eq!(top(4, pinned, count, rows), 8, "到头就停");
        pin(4, &mut pinned, count, rows, -100);
        assert_eq!(top(4, pinned, count, rows), 0);
        let mut menu = Menu {
            pinned,
            ..Menu::default()
        };
        menu.step(true, count);
        assert_eq!(menu.pinned, None, "按键选的放开，回到选中的停在正中间");
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
