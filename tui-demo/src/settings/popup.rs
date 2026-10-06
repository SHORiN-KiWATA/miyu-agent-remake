//! 悬浮窗里除了编辑窗以外的两种（蓝图「配置页」第 13、16、20 条）：选默认模型的窗、问一句的窗。

use super::data::Data;
use super::forms::Form;
use super::nav::Use;

/// 开着的悬浮窗。
#[derive(Debug)]
pub enum Popup {
    /// 编辑供应商、模型、池。
    Form(Form),
    /// 选默认模型。
    Pick(Pick),
    /// 问一句。
    Confirm(Confirm),
}

/// 选默认模型的窗。
#[derive(Debug, Clone)]
pub struct Pick {
    /// 选哪种用途的。
    pub usage: Use,
    /// 筛的字。
    pub search: String,
    /// 在打筛的字。
    pub typing: bool,
    /// 选中第几行（段名不算能选的，但占一行）。
    pub sel: usize,
    /// 滚到哪。
    pub top: usize,
}

/// 选模型的窗里的一行。
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// 段名：池那一段是 `None`，供应商的是它的名字。
    Head(Option<String>),
    /// 能选的一行（`off` 是选不了的原因的名字：`off_vision`、`off_empty`）。
    Choice {
        /// 引用（池写 `@名字`）。
        reference: String,
        /// 写什么。
        label: String,
        /// 右边暗着写的。
        tag: Choice,
        /// 选不了的原因。
        off: Option<&'static str>,
    },
}

/// 右边写的：池的分法和几个成员，模型的窗口和能不能看图。
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// 池。
    Pool(Option<String>, usize),
    /// 模型。
    Model(Option<u64>, bool),
}

impl Pick {
    /// 开窗：选中现在用的那个。
    pub fn open(usage: Use, view: &Data) -> Self {
        let mut pick = Self {
            usage,
            search: String::new(),
            typing: false,
            sel: 0,
            top: 0,
        };
        let current = match usage {
            Use::Chat => view.chat.clone(),
            Use::Vision => view.vision.clone(),
        };
        let items = pick.items(view);
        pick.sel = items
            .iter()
            .position(|i| matches!(i, Item::Choice { reference, off: None, .. } if Some(reference) == current.as_ref()))
            .unwrap_or(0);
        pick.settle(&items);
        pick
    }

    /// 一行行：池在上，再按供应商分段列模型；视觉的只列能看图的，池要每个成员都能看图。
    pub fn items(&self, view: &Data) -> Vec<Item> {
        let needle = self.search.to_lowercase();
        let hit = |s: &str| needle.is_empty() || s.to_lowercase().contains(&needle);
        let vision = self.usage == Use::Vision;
        let mut items = Vec::new();
        let pools: Vec<_> = view.pools.iter().filter(|p| hit(&p.name)).collect();
        if !pools.is_empty() {
            items.push(Item::Head(None));
        }
        for p in pools {
            let sees = p
                .members
                .iter()
                .all(|m| view.model(m).is_some_and(|(_, m)| m.sees()));
            let off = if p.members.is_empty() {
                Some("off_empty")
            } else if vision && !sees {
                Some("off_vision")
            } else {
                None
            };
            items.push(Item::Choice {
                reference: format!("@{}", p.name),
                label: p.name.clone(),
                tag: Choice::Pool(p.strategy.clone(), p.members.len()),
                off,
            });
        }
        for provider in &view.providers {
            let models: Vec<_> = provider
                .models
                .iter()
                .filter(|m| (!vision || m.sees()) && hit(&m.reference))
                .collect();
            if models.is_empty() {
                continue;
            }
            items.push(Item::Head(Some(provider.shown().to_string())));
            for m in models {
                items.push(Item::Choice {
                    reference: m.reference.clone(),
                    label: m.reference.clone(),
                    tag: Choice::Model(m.window(), m.sees()),
                    off: None,
                });
            }
        }
        items
    }

    /// 往 `dir` 挪一行能选的；没有能选的停在原地。
    pub fn step(&mut self, dir: isize, view: &Data) {
        let items = self.items(view);
        let mut at = self.sel;
        loop {
            match at.checked_add_signed(dir) {
                Some(next) if next < items.len() => {
                    at = next;
                    if selectable(&items[at]) {
                        self.sel = at;
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    /// 筛的字变了：停到第一行能选的。
    pub fn refilter(&mut self, view: &Data) {
        self.sel = 0;
        self.top = 0;
        let items = self.items(view);
        self.settle(&items);
    }

    /// 停在段名、选不了的行上时：往下找一行能选的，下面没有的找第一行能选的。
    fn settle(&mut self, items: &[Item]) {
        if items.get(self.sel).is_some_and(selectable) {
            return;
        }
        let below = items
            .iter()
            .skip(self.sel)
            .position(selectable)
            .map(|i| i + self.sel);
        self.sel = below
            .or_else(|| items.iter().position(selectable))
            .unwrap_or(0);
    }

    /// 选中的那一行的引用。
    pub fn chosen(&self, view: &Data) -> Option<String> {
        match self.items(view).into_iter().nth(self.sel)? {
            Item::Choice {
                reference,
                off: None,
                ..
            } => Some(reference),
            _ => None,
        }
    }
}

fn selectable(item: &Item) -> bool {
    matches!(item, Item::Choice { off: None, .. })
}

/// 问一句的窗：标题、几行话、几个按钮，选了做什么。
#[derive(Debug, Clone)]
pub struct Confirm {
    /// 标题。
    pub title: String,
    /// 几行话（`true` 的暗着写）。
    pub lines: Vec<(String, bool)>,
    /// 按钮（`true` 的是删除那种，选中时红底）。
    pub buttons: Vec<(String, bool)>,
    /// 选中第几个按钮。
    pub sel: usize,
    /// 删哪一样。
    pub ask: Delete,
}

/// 问的是删哪一样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delete {
    /// 供应商：编号。
    Provider(String),
    /// 模型：供应商编号、模型名。
    Model(String, String),
    /// 池：名字。
    Pool(String),
}

#[cfg(test)]
mod tests {
    use super::{Choice, Item, Pick};
    use crate::settings::nav::Use;
    use crate::settings::test_support::sample;

    #[test]
    fn the_vision_picker_lists_seeing_models_and_only_all_seeing_pools() {
        let view = sample();
        let pick = Pick::open(Use::Vision, &view);
        let items = pick.items(&view);
        assert_eq!(items[0], Item::Head(None));
        assert!(
            matches!(&items[1], Item::Choice { label, off: Some("off_vision"), .. } if label == "daily")
        );
        assert!(matches!(
            &items[2],
            Item::Choice {
                off: Some("off_empty"),
                ..
            }
        ));
        let models: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                Item::Choice {
                    tag: Choice::Model(..),
                    label,
                    ..
                } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(models, ["relay/zhipu/glm-5v"]);
        assert_eq!(
            pick.chosen(&view).as_deref(),
            Some("relay/zhipu/glm-5v"),
            "开窗选中现在用的"
        );
    }

    #[test]
    fn moving_skips_heads_and_unusable_rows() {
        let view = sample();
        let mut pick = Pick::open(Use::Chat, &view);
        assert_eq!(pick.chosen(&view).as_deref(), Some("@daily"));
        pick.step(1, &view);
        assert_eq!(
            pick.chosen(&view).as_deref(),
            Some("dev/flash"),
            "空池、段名跳过"
        );
        pick.search = "glm".into();
        pick.refilter(&view);
        assert_eq!(pick.chosen(&view).as_deref(), Some("relay/zhipu/glm-5v"));
    }
}
