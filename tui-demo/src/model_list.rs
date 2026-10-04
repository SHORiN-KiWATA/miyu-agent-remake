//! `/model` 框的状态（蓝图 `tui.md`「配置与模型」第 1 条）：核心交回的一行行、打的字、选中哪一个。筛选在这里，画在
//! `ui/model_list.rs`。

use crate::core::Choice;

/// `/model` 框。
#[derive(Debug, Default)]
pub struct ModelList {
    /// 核心交回的，照它排好的先后（挡位、池、模型）。
    pub all: Vec<Choice>,
    /// 打的字：筛名字和引用。
    pub query: String,
    /// 选中对得上的第几个。
    pub selected: usize,
    /// 核心交回来了：没交回来之前框里写「正在读」。
    pub loaded: bool,
    /// 鼠标钉住的露出的那一段（照会话列表）。
    pub pinned: Option<usize>,
}

impl ModelList {
    /// 对得上的，照原来的先后。
    pub fn matches(&self) -> Vec<&Choice> {
        let query = self.query.to_lowercase();
        self.all
            .iter()
            .filter(|c| {
                query.is_empty()
                    || c.name.to_lowercase().contains(&query)
                    || c.reference.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// 选中的那一个。
    pub fn picked(&self) -> Option<&Choice> {
        self.matches().get(self.selected).copied()
    }

    /// 核心交回了：选中正在用的那个（`current` 是会话现在用的引用），没有的停在第一个。
    pub fn replace(&mut self, all: Vec<Choice>, current: Option<&str>) {
        self.all = all;
        self.loaded = true;
        self.selected = self
            .matches()
            .iter()
            .position(|c| Some(c.reference.as_str()) == current)
            .unwrap_or(0);
    }

    /// 往上、往下挪一个，到头停。
    pub fn step(&mut self, down: bool) {
        let last = self.matches().len().saturating_sub(1);
        self.selected = if down {
            (self.selected + 1).min(last)
        } else {
            self.selected.saturating_sub(1)
        };
        self.pinned = None;
    }

    /// 打了字、删了字：回到第一个。
    pub fn typed(&mut self, query: String) {
        self.query = query;
        self.selected = 0;
        self.pinned = None;
    }
}

#[cfg(test)]
mod tests {
    use super::ModelList;
    use crate::core::{Choice, ChoiceState};

    fn choice(reference: &str, name: &str) -> Choice {
        Choice {
            reference: reference.into(),
            name: name.into(),
            detail: String::new(),
            state: ChoiceState::Ok,
        }
    }

    #[test]
    fn it_opens_on_the_model_in_use_and_filters_by_name_or_reference() {
        let mut list = ModelList::default();
        list.replace(
            vec![
                choice("standard", "standard"),
                choice("@duo", "@duo"),
                choice("dev/m1", "Flash"),
            ],
            Some("@duo"),
        );
        assert_eq!(list.picked().unwrap().reference, "@duo", "选中正在用的");
        list.typed("flash".into());
        assert_eq!(list.matches().len(), 1);
        list.typed("dev/".into());
        assert_eq!(list.picked().unwrap().reference, "dev/m1", "引用也算");
    }
}
