//! 输入历史列表的状态（蓝图 `tui.md`「输入历史列表」）：开没开、搜索的字、选中哪一条。
//!
//! 列的是输入框记着的发过的话和命令（和 `↑`、`↓` 翻的是同一份）。对得上的照「最新的在前」排，
//! 选中的是其中第几条：0 是最新的，画的时候最新的贴着输入框。

/// 列表的状态。
#[derive(Debug, Default)]
pub struct History {
    /// 开着。
    pub open: bool,
    /// 「历史：」后面打的字。
    pub query: String,
    /// 选中的是对得上的第几条，0 是最新的。
    pub selected: usize,
    /// `Tab` 展开着：选中的那一条写全文，换一条也展开着。
    pub expanded: bool,
}

impl History {
    /// 打开：搜索的字清空，选中最新的一条。
    pub fn open(&mut self) {
        *self = Self {
            open: true,
            ..Self::default()
        };
    }

    /// `Tab`：选中的那一条展开成全文，再按收回一行。
    pub fn toggle_full(&mut self) {
        self.expanded = !self.expanded;
    }

    /// 关掉。
    pub fn close(&mut self) {
        self.open = false;
    }

    /// 对得上的几条，最新的在前：包含搜索的字（不分大小写）；一样的只留最新的那一次。
    pub fn matches<'a>(&self, sent: &'a [String]) -> Vec<&'a str> {
        let query = self.query.to_lowercase();
        let mut seen = Vec::new();
        for text in sent.iter().rev() {
            if text.to_lowercase().contains(&query) && !seen.contains(&text.as_str()) {
                seen.push(text.as_str());
            }
        }
        seen
    }

    /// 往更早的走（`↑`、再按 `Ctrl+R`），到头就停。`count` 是对得上的条数。
    pub fn older(&mut self, count: usize) {
        self.selected = (self.selected + 1).min(count.saturating_sub(1));
    }

    /// 往更新的走（`↓`），到头就停。
    pub fn newer(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// 搜索的字后面接上 `text`，选中回到最新的一条。
    pub fn type_text(&mut self, text: &str) {
        self.query.push_str(text);
        self.selected = 0;
    }

    /// 删掉搜索的最后一个字，选中回到最新的一条。
    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    fn sent() -> Vec<String> {
        [
            "跑一下全部测试",
            "看看 README",
            "把失败的测试修掉",
            "看看 README",
        ]
        .map(String::from)
        .to_vec()
    }

    #[test]
    fn newest_first_and_each_text_once() {
        let mut h = History::default();
        h.open();
        assert_eq!(
            h.matches(&sent()),
            vec!["看看 README", "把失败的测试修掉", "跑一下全部测试"]
        );
    }

    #[test]
    fn typing_filters_ignoring_case_and_goes_back_to_the_newest() {
        let mut h = History::default();
        h.open();
        h.older(3);
        h.type_text("readme");
        assert_eq!((h.matches(&sent()), h.selected), (vec!["看看 README"], 0));
        h.backspace();
        h.backspace();
        h.backspace();
        h.backspace();
        h.backspace();
        h.backspace();
        h.type_text("测试");
        assert_eq!(
            h.matches(&sent()),
            vec!["把失败的测试修掉", "跑一下全部测试"]
        );
    }

    #[test]
    fn moving_stops_at_both_ends() {
        let mut h = History::default();
        h.open();
        h.newer();
        assert_eq!(h.selected, 0);
        for _ in 0..5 {
            h.older(3);
        }
        assert_eq!(h.selected, 2);
    }
}
