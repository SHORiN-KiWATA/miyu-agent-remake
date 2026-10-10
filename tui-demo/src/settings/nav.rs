//! 三页、几栏、选中哪一行（蓝图「配置页」第 6 到 9、21、22 条）：切栏到边换页，组织照模型名第一个 `/` 前面那段分，
//! `/` 在当前栏里筛。只看叠好的那一份（`merge.rs`）。

use super::data::{Data, Model, Pool, Provider};

/// 一栏。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    /// 供应商。
    Provider,
    /// 组织（中转站的前缀）。
    Org,
    /// 模型。
    Model,
    /// 默认模型页的两种用途。
    Use,
    /// 模型池。
    Pool,
    /// 池的成员。
    Member,
}

impl Col {
    fn at(self) -> usize {
        self as usize
    }
}

/// 一页。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// 供应商。
    #[default]
    Providers,
    /// 默认模型。
    Defaults,
    /// 模型池。
    Pools,
}

/// 三页的先后。
pub const PAGES: [Page; 3] = [Page::Providers, Page::Defaults, Page::Pools];

/// 默认模型页的两种用途。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// 文本（`models.chat`）。
    Chat,
    /// 视觉（`models.vision`）。
    Vision,
    /// 语义模型（`models.embedding`，4a 的 R-5 补：`model_or`，不收池）。
    Embedding,
}

/// 几种用途的先后。
pub const USES: [Use; 3] = [Use::Chat, Use::Vision, Use::Embedding];

/// 语义模型那一项的键。
pub const EMBEDDING: &str = "models.embedding";

/// 组织栏的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Org {
    /// 全部。
    All,
    /// 一个前缀。
    Named(String),
    /// 不带前缀的。
    Other,
}

/// 在一栏里筛：哪一栏、筛的字、还在打字。
#[derive(Debug, Clone, Default)]
pub struct Search {
    /// 哪一栏。
    pub col: Option<Col>,
    /// 筛的字。
    pub text: String,
    /// 还在打字（字在状态行里，`Enter` 收起）。
    pub typing: bool,
}

/// 在哪一页、哪一栏、每栏选中哪一行、滚到哪。
#[derive(Debug, Default)]
pub struct Nav {
    /// 第几页。
    pub page: Page,
    /// 这一页的第几栏。
    pub col: usize,
    sel: [usize; 6],
    /// 每栏滚到哪（画的时候照选中的那行挪）。
    pub top: [usize; 6],
    /// 在筛。
    pub search: Option<Search>,
}

impl Nav {
    /// 这一页有哪几栏：没有一个模型带前缀的，组织栏不出现。
    pub fn cols(&self, view: &Data) -> Vec<Col> {
        match self.page {
            Page::Providers => {
                let orgs = self.provider(view).is_some_and(|p| !orgs(p).is_empty());
                if orgs {
                    vec![Col::Provider, Col::Org, Col::Model]
                } else {
                    vec![Col::Provider, Col::Model]
                }
            }
            Page::Defaults => vec![Col::Use],
            Page::Pools => vec![Col::Pool, Col::Member],
        }
    }

    /// 焦点在哪一栏。
    pub fn focus(&self, view: &Data) -> Col {
        let cols = self.cols(view);
        cols[self.col.min(cols.len() - 1)]
    }

    /// 一栏选中的第几行。
    pub fn selected(&self, col: Col) -> usize {
        self.sel[col.at()]
    }

    /// 选中一栏的第几行（鼠标点的）：上一级换了，下一级回到第一行。
    pub fn pick(&mut self, col: Col, row: usize) {
        if self.sel[col.at()] != row {
            self.sel[col.at()] = row;
            self.reset_below(col);
        }
    }

    /// 筛这一栏的字；没在筛的是 `None`。
    fn filter(&self, col: Col) -> Option<String> {
        self.search
            .as_ref()
            .filter(|s| s.col == Some(col) && !s.text.is_empty())
            .map(|s| s.text.to_lowercase())
    }

    /// 供应商栏的每一行。
    pub fn providers<'a>(&self, view: &'a Data) -> Vec<&'a Provider> {
        let filter = self.filter(Col::Provider);
        view.providers
            .iter()
            .filter(|p| {
                filter.as_ref().is_none_or(|f| {
                    p.id.to_lowercase().contains(f) || p.shown().to_lowercase().contains(f)
                })
            })
            .collect()
    }

    /// 选中的供应商。
    pub fn provider<'a>(&self, view: &'a Data) -> Option<&'a Provider> {
        self.providers(view)
            .get(self.selected(Col::Provider))
            .copied()
    }

    /// 选中的组织；这一家没有组织栏的是 `None`。
    pub fn org(&self, view: &Data) -> Option<Org> {
        let p = self.provider(view)?;
        orgs(p)
            .into_iter()
            .nth(self.selected(Col::Org))
            .map(|(o, _)| o)
    }

    /// 模型栏的每一行。
    pub fn models<'a>(&self, view: &'a Data) -> Vec<&'a Model> {
        let Some(p) = self.provider(view) else {
            return Vec::new();
        };
        let org = self.org(view);
        let filter = self.filter(Col::Model);
        p.models
            .iter()
            .filter(|m| org.as_ref().is_none_or(|o| in_org(m, o)))
            .filter(|m| {
                filter
                    .as_ref()
                    .is_none_or(|f| m.name.to_lowercase().contains(f))
            })
            .collect()
    }

    /// 选中的模型。
    pub fn model<'a>(&self, view: &'a Data) -> Option<&'a Model> {
        self.models(view).get(self.selected(Col::Model)).copied()
    }

    /// 选中的用途。
    pub fn usage(&self) -> Use {
        USES[self.selected(Col::Use).min(USES.len() - 1)]
    }

    /// 池栏的每一行。
    pub fn pools<'a>(&self, view: &'a Data) -> Vec<&'a Pool> {
        let filter = self.filter(Col::Pool);
        view.pools
            .iter()
            .filter(|p| {
                filter
                    .as_ref()
                    .is_none_or(|f| p.name.to_lowercase().contains(f))
            })
            .collect()
    }

    /// 选中的池。
    pub fn pool<'a>(&self, view: &'a Data) -> Option<&'a Pool> {
        self.pools(view).get(self.selected(Col::Pool)).copied()
    }

    /// 一栏有几行。
    pub fn len(&self, col: Col, view: &Data) -> usize {
        match col {
            Col::Provider => self.providers(view).len(),
            Col::Org => self.provider(view).map_or(0, |p| orgs(p).len()),
            Col::Model => self.models(view).len(),
            Col::Use => USES.len(),
            Col::Pool => self.pools(view).len(),
            Col::Member => self.pool(view).map_or(0, |p| p.members.len()),
        }
    }

    /// 左右切栏：在最右一栏再往右进下一页、落在第一栏；最左一栏再往左进上一页、落在最后一栏；到头停住。
    pub fn sideways(&mut self, right: bool, view: &Data) {
        let last = self.cols(view).len() - 1;
        let at = self.col.min(last);
        let page = PAGES.iter().position(|p| *p == self.page).unwrap_or(0);
        if right && at < last {
            self.col = at + 1;
        } else if !right && at > 0 {
            self.col = at - 1;
        } else if right && page + 1 < PAGES.len() {
            self.turn(PAGES[page + 1]);
            self.col = 0;
        } else if !right && page > 0 {
            self.turn(PAGES[page - 1]);
            self.col = self.cols(view).len() - 1;
        }
    }

    /// `,` `.` 换页，落在第一栏；到头停住。
    pub fn page_step(&mut self, next: bool) {
        let page = PAGES.iter().position(|p| *p == self.page).unwrap_or(0);
        let to = if next {
            (page + 1 < PAGES.len()).then(|| page + 1)
        } else {
            page.checked_sub(1)
        };
        if let Some(to) = to {
            self.turn(PAGES[to]);
            self.col = 0;
        }
    }

    /// 换到一页：不在筛了。
    pub fn turn(&mut self, page: Page) {
        self.page = page;
        self.search = None;
    }

    /// 上下移动（`delta` 行，翻页时是一屏）；上一级换了，下一级回到第一行。
    pub fn vertical(&mut self, delta: isize, view: &Data) {
        let col = self.focus(view);
        let len = self.len(col, view);
        if len == 0 {
            return;
        }
        let at = self.sel[col.at()].saturating_add_signed(delta).min(len - 1);
        self.pick(col, at);
    }

    /// 上一级换了，下一级回到第一行。
    fn reset_below(&mut self, col: Col) {
        let below: &[Col] = match col {
            Col::Provider => &[Col::Org, Col::Model],
            Col::Org => &[Col::Model],
            Col::Pool => &[Col::Member],
            _ => &[],
        };
        for c in below {
            self.sel[c.at()] = 0;
        }
    }

    /// 东西少了（删了、筛了、重读了）：选中的、栏都别越界。
    pub fn clamp(&mut self, view: &Data) {
        for col in [
            Col::Provider,
            Col::Org,
            Col::Model,
            Col::Use,
            Col::Pool,
            Col::Member,
        ] {
            let len = self.len(col, view);
            let at = &mut self.sel[col.at()];
            *at = (*at).min(len.saturating_sub(1));
        }
        self.col = self.col.min(self.cols(view).len() - 1);
    }

    /// 选中这一家、这个模型（新加的、刚改的）：组织回到「全部」。
    pub fn show_model(&mut self, view: &Data, provider: &str, model: Option<&str>) {
        self.search = None;
        if let Some(p) = view.providers.iter().position(|p| p.id == provider) {
            self.pick(Col::Provider, p);
            self.sel[Col::Org.at()] = 0;
            let at = model.and_then(|m| view.providers[p].models.iter().position(|x| x.name == m));
            self.sel[Col::Model.at()] = at.unwrap_or(0);
        }
    }

    /// 重拼以后照编号、模型名找回原来选中的（没在筛的才找；找不到的不动）。
    pub fn keep_model(&mut self, view: &Data, provider: &str, model: Option<&str>) {
        if self.search.is_some() {
            return;
        }
        let Some(p) = view.providers.iter().position(|p| p.id == provider) else {
            return;
        };
        self.sel[Col::Provider.at()] = p;
        if let Some(m) = model.and_then(|m| self.models(view).iter().position(|x| x.name == m)) {
            self.sel[Col::Model.at()] = m;
        }
    }

    /// 重拼以后找回原来选中的池。
    pub fn keep_pool(&mut self, view: &Data, name: &str) {
        if self.search.is_some() {
            return;
        }
        if let Some(at) = view.pools.iter().position(|p| p.name == name) {
            self.sel[Col::Pool.at()] = at;
        }
    }

    /// 选中这个池。
    pub fn show_pool(&mut self, view: &Data, name: &str) {
        self.search = None;
        if let Some(at) = view.pools.iter().position(|p| p.name == name) {
            self.pick(Col::Pool, at);
        }
    }
}

/// 一家的组织：没有一个模型带前缀的是空的；有的，第一项「全部」，下面照名字排，带前缀、不带的都有时末尾「其他」。
/// 每项带几个模型。
pub fn orgs(p: &Provider) -> Vec<(Org, usize)> {
    let mut names: Vec<&str> = p.models.iter().filter_map(|m| prefix(&m.name)).collect();
    if names.is_empty() {
        return Vec::new();
    }
    names.sort_unstable();
    names.dedup();
    let mut list = vec![(Org::All, p.models.len())];
    for name in names {
        let n = p
            .models
            .iter()
            .filter(|m| prefix(&m.name) == Some(name))
            .count();
        list.push((Org::Named(name.to_string()), n));
    }
    let others = p
        .models
        .iter()
        .filter(|m| prefix(&m.name).is_none())
        .count();
    if others > 0 {
        list.push((Org::Other, others));
    }
    list
}

/// 模型名第一个 `/` 前面那段。
fn prefix(name: &str) -> Option<&str> {
    name.split_once('/')
        .map(|(org, _)| org)
        .filter(|o| !o.is_empty())
}

fn in_org(m: &Model, org: &Org) -> bool {
    match org {
        Org::All => true,
        Org::Named(name) => prefix(&m.name) == Some(name),
        Org::Other => prefix(&m.name).is_none(),
    }
}

/// 模型栏里写的名字：选了某个组织的去掉前缀。
pub fn shown_name<'a>(m: &'a Model, org: Option<&Org>) -> &'a str {
    match org {
        Some(Org::Named(name)) => m
            .name
            .strip_prefix(name.as_str())
            .and_then(|r| r.strip_prefix('/'))
            .unwrap_or(&m.name),
        _ => &m.name,
    }
}

#[cfg(test)]
mod tests;
