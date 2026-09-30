//! 会话列表的状态（蓝图 `tui.md`「会话列表 `/sessions`」第 1–3 条）：核心交回的会话、打的字、选中哪一个、按过一次
//! `Ctrl+D` 的是哪一个。排序、筛选在这里，画在 `ui/session_list.rs`。

use crate::core::SessionInfo;

/// 会话列表。
#[derive(Debug, Default)]
pub struct SessionList {
    /// 核心交回的，照它的先后（新的在前）。
    pub all: Vec<SessionInfo>,
    /// 打的字：筛标题、短编号。
    pub query: String,
    /// 选中对得上的第几个。
    pub selected: usize,
    /// 按过一次 `Ctrl+D`：再按一次才删这几个（第 2 条）。
    pub armed: Option<Vec<String>>,
    /// 空格勾上的（第 2 条）。
    pub ticked: Vec<String>,
    /// 核心交回来了：没交回来之前框里写「正在读」。
    pub loaded: bool,
    /// 鼠标钉住的露出的那一段（悬停不挪列表，滚轮挪），按键放开（照输入历史列表，`menu::top`）。
    pub pinned: Option<usize>,
}

impl SessionList {
    /// 对得上的，照先后：置顶的在前，别的照最近动静，新的在前；没有最近动静的照核心交回的先后（第 3 条）。
    pub fn matches(&self) -> Vec<&SessionInfo> {
        let query = self.query.to_lowercase();
        let mut found: Vec<(usize, &SessionInfo)> = self
            .all
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.is_empty()
                    || s.title
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(&query))
                    || short(&s.session).contains(&query)
            })
            .collect();
        found.sort_by(|(i, a), (j, b)| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.last_active.cmp(&a.last_active))
                .then(i.cmp(j))
        });
        found.into_iter().map(|(_, s)| s).collect()
    }

    /// 选中的那一个。
    pub fn picked(&self) -> Option<&SessionInfo> {
        self.matches().get(self.selected).copied()
    }

    /// 核心交回了新的一份：选中的照旧停在同一个会话上（置顶、删了以后重读）。
    pub fn replace(&mut self, all: Vec<SessionInfo>) {
        let was = self.picked().map(|s| s.session.clone());
        self.all = all;
        self.loaded = true;
        let found = self.matches();
        self.selected = was
            .and_then(|w| found.iter().position(|s| s.session == w))
            .unwrap_or(self.selected)
            .min(found.len().saturating_sub(1));
    }

    /// 往上、往下挪一个，到头停。
    pub fn step(&mut self, down: bool) {
        let last = self.matches().len().saturating_sub(1);
        self.selected = if down {
            (self.selected + 1).min(last)
        } else {
            self.selected.saturating_sub(1)
        };
        self.armed = None;
        self.pinned = None;
    }

    /// 空格：勾上选中的那个，勾过的取消；正在用的（`current`）勾不上。
    pub fn tick(&mut self, current: Option<&str>) {
        let Some(id) = self.picked().map(|s| s.session.clone()) else {
            return;
        };
        self.armed = None;
        if current == Some(id.as_str()) {
            return;
        }
        match self.ticked.iter().position(|t| *t == id) {
            Some(at) => {
                self.ticked.remove(at);
            }
            None => self.ticked.push(id),
        }
    }

    /// `Ctrl+A`：勾上对得上的全部（正在用的除外）；全都勾着时全部取消。
    pub fn tick_all(&mut self, current: Option<&str>) {
        let all: Vec<String> = self
            .matches()
            .iter()
            .map(|s| s.session.clone())
            .filter(|s| Some(s.as_str()) != current)
            .collect();
        self.armed = None;
        if all.iter().all(|s| self.ticked.contains(s)) {
            self.ticked.retain(|s| !all.contains(s));
        } else {
            for s in all {
                if !self.ticked.contains(&s) {
                    self.ticked.push(s);
                }
            }
        }
    }

    /// `Ctrl+D` 要删的：勾了的全部（照列表的先后），没勾的是选中的那一个；正在用的不算。
    pub fn doomed(&self, current: Option<&str>) -> Vec<String> {
        let picked = self.picked().map(|s| s.session.clone());
        let ids: Vec<String> = if self.ticked.is_empty() {
            picked.into_iter().collect()
        } else {
            self.matches()
                .iter()
                .map(|s| s.session.clone())
                .filter(|s| self.ticked.contains(s))
                .collect()
        };
        ids.into_iter()
            .filter(|s| Some(s.as_str()) != current)
            .collect()
    }

    /// 删掉了这几个：选中的停在原来的位置，下面那个补上来（2026-10-01 项目主人：原来跳回第一个）。
    pub fn remove(&mut self, gone: &[String]) {
        self.all.retain(|s| !gone.contains(&s.session));
        self.ticked.retain(|s| !gone.contains(s));
        self.armed = None;
        self.selected = self.selected.min(self.matches().len().saturating_sub(1));
    }

    /// 打了字、删了字：回到第一个。
    pub fn typed(&mut self, query: String) {
        self.query = query;
        self.selected = 0;
        self.armed = None;
        self.pinned = None;
    }
}

/// 短编号：整个编号的最后 8 个字符（核心 C-1 定的，侧边栏、列表、她看到的都是这个）。
pub fn short(session: &str) -> String {
    let skip = session.chars().count().saturating_sub(8);
    session.chars().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::{SessionList, short};
    use crate::core::SessionInfo;

    fn info(session: &str, title: Option<&str>, pinned: bool, active: Option<&str>) -> SessionInfo {
        SessionInfo {
            session: session.into(),
            title: title.map(str::to_string),
            pinned,
            cwd: None,
            busy: false,
            last_active: active.map(|a| a.parse().unwrap()),
        }
    }

    fn order(list: &SessionList) -> Vec<&str> {
        list.matches().iter().map(|s| s.session.as_str()).collect()
    }

    #[test]
    fn pinned_come_first_then_the_most_recently_active_then_the_core_order() {
        let mut list = SessionList::default();
        list.replace(vec![
            info("0000-aaaaaaaa", Some("新的"), false, None),
            info("0000-bbbbbbbb", Some("旧的"), false, None),
            info("0000-cccccccc", Some("置顶"), true, None),
        ]);
        assert_eq!(
            order(&list),
            ["0000-cccccccc", "0000-aaaaaaaa", "0000-bbbbbbbb"]
        );
        list.replace(vec![
            info("0000-aaaaaaaa", None, false, Some("2026-10-01T08:00:00Z")),
            info("0000-bbbbbbbb", None, false, Some("2026-10-01T09:00:00Z")),
        ]);
        assert_eq!(
            order(&list),
            ["0000-bbbbbbbb", "0000-aaaaaaaa"],
            "C-3 以后照最近动静"
        );
    }

    #[test]
    fn ticked_ones_are_deleted_together_and_the_pick_stays_where_it_was() {
        // 2026-10-01 项目主人：删完光标跳回第一个、不能批量删。
        let mut list = SessionList::default();
        list.replace(vec![
            info("0000-aaaaaaaa", Some("一"), false, None),
            info("0000-bbbbbbbb", Some("二"), false, None),
            info("0000-cccccccc", Some("三"), false, None),
            info("0000-dddddddd", Some("四"), false, None),
        ]);
        let here = Some("0000-aaaaaaaa");
        list.step(true);
        assert_eq!(list.doomed(here), ["0000-bbbbbbbb"], "没勾的删选中的");
        list.tick(here);
        list.step(true);
        list.tick(here);
        assert_eq!(
            list.doomed(here),
            ["0000-bbbbbbbb", "0000-cccccccc"],
            "勾了的全删"
        );
        list.tick(here);
        assert_eq!(list.doomed(here), ["0000-bbbbbbbb"], "再按取消");
        list.step(false);
        list.step(false);
        list.tick(here);
        assert_eq!(list.doomed(here), ["0000-bbbbbbbb"], "正在用的勾不上");
        list.step(true);
        list.remove(&["0000-bbbbbbbb".to_string()]);
        assert_eq!(
            list.picked().unwrap().session,
            "0000-cccccccc",
            "停在原来的位置，下面那个补上来"
        );
        assert!(list.ticked.is_empty());
        list.step(true);
        list.step(true);
        list.remove(&["0000-dddddddd".to_string()]);
        assert_eq!(
            list.picked().unwrap().session,
            "0000-cccccccc",
            "删的是最后一个：停在新的最后一个"
        );
    }

    #[test]
    fn ctrl_a_ticks_every_match_but_the_current_one_and_again_clears() {
        // 2026-10-01 项目主人：做 Ctrl+A 全选（有 222 个未命名会话要删）。
        let mut list = SessionList::default();
        list.replace(vec![
            info("0000-aaaaaaaa", Some("一"), false, None),
            info("0000-bbbbbbbb", None, false, None),
            info("0000-cccccccc", None, false, None),
        ]);
        let here = Some("0000-aaaaaaaa");
        list.typed("cccc".into());
        list.tick_all(here);
        assert_eq!(list.ticked, ["0000-cccccccc"], "只勾对得上的");
        list.typed(String::new());
        list.tick_all(here);
        assert_eq!(
            list.doomed(here),
            ["0000-bbbbbbbb", "0000-cccccccc"],
            "正在用的不勾"
        );
        list.tick_all(here);
        assert!(list.ticked.is_empty(), "全都勾着：全部取消");
    }

    #[test]
    fn typing_filters_titles_and_short_ids_and_the_pick_follows_its_session() {
        let mut list = SessionList::default();
        list.replace(vec![
            info("0000-aaaaaaaa", Some("回文函数"), false, None),
            info("0000-bbbbbbbb", Some("Rust 宏"), false, None),
        ]);
        list.typed("rust".into());
        assert_eq!(order(&list), ["0000-bbbbbbbb"], "不分大小写");
        list.typed("aaaa".into());
        assert_eq!(order(&list), ["0000-aaaaaaaa"], "短编号也算");
        list.typed(String::new());
        list.step(true);
        assert_eq!(list.picked().unwrap().session, "0000-bbbbbbbb");
        // 重读以后（置顶了、删了一个）选中的还是它。
        list.replace(vec![
            info("0000-bbbbbbbb", Some("Rust 宏"), true, None),
            info("0000-aaaaaaaa", Some("回文函数"), false, None),
        ]);
        assert_eq!(list.picked().unwrap().session, "0000-bbbbbbbb");
        assert_eq!(short("0192f3a0-1111-7abc-8def-001122334455"), "22334455");
    }
}
