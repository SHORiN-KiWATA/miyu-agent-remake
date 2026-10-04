//! 确认和提问的抽屉（蓝图 `tui.md`「确认和提问的抽屉」，设计 13 第四节）：选到哪、勾了哪些、写了什么，
//! 交出去的回答。纯状态，不碰终端；按键在 `keys.rs`，画在 `ui/drawer/`。

mod event;
mod keys;
mod report;
mod texts;

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub use event::{Answer, Answered, Approval, Asked, Decided, Decision, Fake, Question};
pub use report::{Mark, Report, inline, said};
pub use texts::Texts;

/// 问的是什么。
#[derive(Debug, Clone)]
pub enum Ask {
    /// 她的提问：一道道题。
    Questions(Vec<Question>),
    /// 权限确认。
    Approval(Approval),
}

/// 一页上的一项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// 第几个选项。
    Choice(usize),
    /// 「输入其他答案」：`Enter` 进编辑。
    Other,
    /// 确认的一种回答。
    Decision(Decision),
}

/// 正在写什么（第 3 条）：编辑时按键都进字里，`Enter` 保存、`Esc` 退出编辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// 「输入其他答案」。
    Other,
    /// 按 `n` 写的补充。
    Notes,
    /// 「不允许」的理由。
    Reason,
}

/// 了结的方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 交了（没答的那道是空的回答）。
    Answered(Answered),
    /// 确认定了。
    Decided(Decided),
    /// 按两下 `Esc` 取消：提问算没回答，确认算跳过（第 5 条）。
    Cancelled,
}

/// 按了一下以后怎么样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// 还开着。
    Stay,
    /// 按了 `Esc`（没在编辑）：两下之间够不够快由外面看时刻定，见 `escape`。
    Escape,
    /// 了结了。
    Done(Outcome),
}

/// 一个抽屉。
#[derive(Debug, Clone)]
pub struct Drawer {
    /// 谁在问：子代理、后台命令问的才有。
    pub who: Option<String>,
    /// 调用编号：作答照它交回去。
    pub call_id: String,
    /// 问的是什么。
    pub ask: Ask,
    /// 在第几页：一道题一页，两道题及以上最后多一页「确认」（第 `pages()` 页）。
    pub tab: usize,
    /// 每一页选到第几项。
    pub cursor: Vec<usize>,
    /// 每一页勾上了哪几个选项（能多选的）。
    pub checked: Vec<Vec<bool>>,
    /// 每一页「输入其他答案」写的字；确认是「不允许」的理由。
    pub typed: Vec<String>,
    /// 每一页按 `n` 写的补充。
    pub notes: Vec<String>,
    /// 每一道的回答；还没答的是 `None`。
    pub answers: Vec<Option<Answer>>,
    /// 正在写什么；没在编辑是 `None`。
    pub editing: Option<Edit>,
    /// 按过一下 `Esc` 的时刻：时限里再按一下取消，按键提示那一行换成「再按一次 Esc 取消」。
    pub armed: Option<Instant>,
    /// 放不下时选项从第几行露起：画的时候跟着选中的那一项定。
    pub scroll: usize,
}

impl Drawer {
    /// 她的提问。
    pub fn question(who: Option<String>, asked: Asked) -> Self {
        let pages = asked.questions.len().max(1);
        let mut checked: Vec<Vec<bool>> = asked
            .questions
            .iter()
            .map(|q| vec![false; q.options.len()])
            .collect();
        // 一道题都没有的（照常碰不到）：留一页，只能写「其他」。
        checked.resize(pages, Vec::new());
        Self::new(who, asked.call_id, Ask::Questions(asked.questions), checked)
    }

    /// 权限确认。
    pub fn approval(who: Option<String>, approval: Approval) -> Self {
        Self::new(
            who,
            approval.call_id.clone(),
            Ask::Approval(approval),
            vec![Vec::new()],
        )
    }

    fn new(who: Option<String>, call_id: String, ask: Ask, checked: Vec<Vec<bool>>) -> Self {
        let pages = checked.len();
        Self {
            who,
            call_id,
            ask,
            tab: 0,
            cursor: vec![0; pages],
            checked,
            typed: vec![String::new(); pages],
            notes: vec![String::new(); pages],
            answers: vec![None; pages],
            editing: None,
            armed: None,
            scroll: 0,
        }
    }

    /// 几道题（确认算一页）。
    pub fn pages(&self) -> usize {
        self.cursor.len()
    }

    /// 有「确认」页：两道题及以上（第 3 条）。
    pub fn has_review(&self) -> bool {
        self.pages() > 1
    }

    /// 在「确认」页上。
    pub fn on_review(&self) -> bool {
        self.tab >= self.pages()
    }

    /// 是提问（不是确认）。
    pub fn is_question(&self) -> bool {
        matches!(self.ask, Ask::Questions(_))
    }

    /// 这一道题；确认是 `None`。
    pub fn question_at(&self, page: usize) -> Option<&Question> {
        match &self.ask {
            Ask::Questions(qs) => qs.get(page),
            Ask::Approval(_) => None,
        }
    }

    /// 这一道在标签、「确认」页、留下的引用块里叫什么：短名，没有的写问题。
    pub fn label(&self, page: usize) -> String {
        self.question_at(page).map_or_else(String::new, |q| {
            q.header
                .clone()
                .unwrap_or_else(|| q.question.trim().to_string())
        })
    }

    /// 这一页能不能多选。
    pub fn multiple(&self, page: usize) -> bool {
        self.question_at(page).is_some_and(|q| q.multiple)
    }

    /// 这一页的各项，照画的先后；「确认」页没有项。
    pub fn items(&self, page: usize) -> Vec<Item> {
        match &self.ask {
            Ask::Questions(qs) => qs.get(page).map_or_else(Vec::new, |q| {
                (0..q.options.len())
                    .map(Item::Choice)
                    .chain([Item::Other])
                    .collect()
            }),
            Ask::Approval(_) if page == 0 => {
                Decision::ALL.into_iter().map(Item::Decision).collect()
            }
            Ask::Approval(_) => Vec::new(),
        }
    }

    /// 这一页的选项带文字画：抽屉可以高过半屏（第 2 条）。
    pub fn has_preview(&self) -> bool {
        self.question_at(self.tab)
            .is_some_and(|q| q.options.iter().any(|o| o.preview.is_some()))
    }

    /// 选中的那一项。
    pub fn current(&self) -> Option<Item> {
        let at = *self.cursor.get(self.tab)?;
        self.items(self.tab).get(at).copied()
    }

    /// 按了一下 `Esc`：上一下还在时限里就取消，不然记下这一下（第 5 条）。
    pub fn escape(&mut self, now: Instant, window: Duration) -> Step {
        if self.armed(now, window) {
            self.armed = None;
            return Step::Done(Outcome::Cancelled);
        }
        self.armed = Some(now);
        Step::Stay
    }

    /// 按过一下 `Esc`，还在时限里。
    pub fn armed(&self, now: Instant, window: Duration) -> bool {
        self.armed
            .is_some_and(|at| now.saturating_duration_since(at) <= window)
    }

    /// 交回去的整份：没答的那道是空的回答，补充的话接进各道。
    fn answered(&self) -> Answered {
        let answers = (0..self.pages())
            .map(|p| {
                let mut a = self.answers[p].clone().unwrap_or_default();
                let notes = self.notes[p].trim();
                a.notes = (!notes.is_empty()).then(|| notes.to_string());
                a
            })
            .collect();
        Answered {
            call_id: self.call_id.clone(),
            answers,
        }
    }
}

/// 排着的抽屉：一次开一个，了结一个开下一个（第 1 条）。
#[derive(Debug, Default)]
pub struct Drawers {
    /// 现在开着的这一个。
    pub current: Option<Drawer>,
    queue: VecDeque<Drawer>,
}

impl Drawers {
    /// 来了一个：没有开着的就开它，有就排在后面。
    pub fn push(&mut self, drawer: Drawer) {
        if self.current.is_none() {
            self.current = Some(drawer);
        } else {
            self.queue.push_back(drawer);
        }
    }

    /// 开着一个。
    pub fn open(&self) -> bool {
        self.current.is_some()
    }

    /// 开着的那一个正在编辑。
    pub fn editing(&self) -> bool {
        self.current.as_ref().is_some_and(|d| d.editing.is_some())
    }

    /// 了结现在这一个，交回它；排着的下一个打开。
    pub fn finish(&mut self) -> Option<Drawer> {
        let done = self.current.take();
        self.current = self.queue.pop_front();
        done
    }
}
