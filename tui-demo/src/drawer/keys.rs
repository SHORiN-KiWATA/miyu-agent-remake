//! 抽屉的按键（蓝图「确认和提问的抽屉」第 3、4 条）：选、勾、换题、`Enter`、数字选第几项；编辑「其他」、
//! 补充、理由时按键进字里。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{Answer, Decided, Decision, Drawer, Edit, Item, Outcome, Step};

impl Drawer {
    /// 按一下（抽屉开着时按键都先归它）。
    pub fn key(&mut self, key: KeyEvent) -> Step {
        // 按了别的键：上一下 `Esc` 不算了。
        if key.code != KeyCode::Esc || self.editing.is_some() {
            self.armed = None;
        }
        if let Some(edit) = self.editing {
            return self.edit_key(edit, key);
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let count = self.items(self.tab).len();
        match key.code {
            KeyCode::Esc => return Step::Escape,
            KeyCode::Up | KeyCode::Down if count > 0 => {
                let cursor = &mut self.cursor[self.tab];
                *cursor = if key.code == KeyCode::Up {
                    cursor.saturating_sub(1)
                } else {
                    (*cursor + 1).min(count - 1)
                };
            }
            KeyCode::Left | KeyCode::BackTab => self.turn(false),
            KeyCode::Right | KeyCode::Tab => self.turn(true),
            KeyCode::Enter => return self.enter(),
            KeyCode::Char(c) if plain => return self.char(c),
            _ => {}
        }
        Step::Stay
    }

    /// 点中第几项：等于移过去按 `Enter`（能多选的是勾上、取消）。
    pub fn click(&mut self, index: usize) -> Step {
        self.editing = None;
        self.armed = None;
        self.pick(index)
    }

    /// 编辑时的按键：`Enter` 保存，`Shift+Enter`、`Alt+Enter`、`Ctrl+J` 换行，`Esc` 退出编辑（字留着）。
    fn edit_key(&mut self, edit: Edit, key: KeyEvent) -> Step {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let newline = key
            .modifiers
            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT);
        let text = match edit {
            Edit::Notes => &mut self.notes[self.tab],
            Edit::Other | Edit::Reason => &mut self.typed[self.tab],
        };
        match key.code {
            KeyCode::Esc => self.editing = None,
            KeyCode::Enter if newline => text.push('\n'),
            KeyCode::Char('j') if ctrl => text.push('\n'),
            KeyCode::Enter => return self.save(edit),
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Char(c) if !ctrl => text.push(c),
            _ => {}
        }
        Step::Stay
    }

    /// 编辑时按 `Enter`：「其他」单选的就答了这一道，多选的算勾上；补充只是存下；理由交「不允许」。
    fn save(&mut self, edit: Edit) -> Step {
        self.editing = None;
        let typed = self.typed[self.tab].trim().to_string();
        match edit {
            Edit::Notes => Step::Stay,
            Edit::Reason => Step::Done(Outcome::Decided(Decided {
                call_id: self.call_id.clone(),
                decision: Decision::Deny,
                reason: (!typed.is_empty()).then_some(typed),
            })),
            Edit::Other if typed.is_empty() || self.multiple(self.tab) => Step::Stay,
            Edit::Other => self.settle(Answer {
                text: Some(typed),
                ..Answer::default()
            }),
        }
    }

    /// 没在编辑时打了一个字：能多选的空格勾上；`n` 写补充；数字选第几项。
    fn char(&mut self, c: char) -> Step {
        match c {
            ' ' => self.toggle(),
            'n' if self.is_question() && !self.on_review() => self.editing = Some(Edit::Notes),
            _ => {
                if let Some(d @ 1..=9) = c.to_digit(10) {
                    return self.pick(d as usize - 1);
                }
            }
        }
        Step::Stay
    }

    /// 移到第几项、按 `Enter`；能多选的选项是勾上、取消。没有这一项的不动。
    fn pick(&mut self, index: usize) -> Step {
        let items = self.items(self.tab);
        let Some(item) = items.get(index) else {
            return Step::Stay;
        };
        self.cursor[self.tab] = index;
        if self.multiple(self.tab) && matches!(item, Item::Choice(_)) {
            self.toggle();
            return Step::Stay;
        }
        self.enter()
    }

    /// 能多选的：勾上、取消选中的那个选项。
    fn toggle(&mut self) {
        if !self.multiple(self.tab) {
            return;
        }
        if let Some(Item::Choice(i)) = self.current()
            && let Some(mark) = self.checked[self.tab].get_mut(i)
        {
            *mark = !*mark;
        }
    }

    /// 换到下一页、上一页（连「确认」页），到头绕回来。
    fn turn(&mut self, forward: bool) {
        let n = self.pages() + usize::from(self.has_review());
        self.tab = if forward {
            (self.tab + 1) % n
        } else {
            (self.tab + n - 1) % n
        };
    }

    /// 按 `Enter`。
    fn enter(&mut self) -> Step {
        if self.on_review() {
            return Step::Done(Outcome::Answered(self.answered()));
        }
        match self.current() {
            Some(Item::Other) => {
                self.editing = Some(Edit::Other);
                Step::Stay
            }
            Some(Item::Decision(Decision::Deny)) => {
                self.editing = Some(Edit::Reason);
                Step::Stay
            }
            Some(Item::Decision(decision)) => Step::Done(Outcome::Decided(Decided {
                call_id: self.call_id.clone(),
                decision,
                reason: None,
            })),
            Some(Item::Choice(i)) => {
                let answer = self.choose(i);
                self.settle(answer)
            }
            None => Step::Stay,
        }
    }

    /// 在第 `i` 个选项上按 `Enter` 的回答：单选的就是它；能多选的交勾上的（一个没勾交它），接着「其他」写的字。
    fn choose(&self, i: usize) -> Answer {
        let Some(question) = self.question_at(self.tab) else {
            return Answer::default();
        };
        let label = |i: usize| question.options[i].label.clone();
        if !question.multiple {
            return Answer {
                picked: vec![label(i)],
                ..Answer::default()
            };
        }
        let typed = self.typed[self.tab].trim();
        let text = (!typed.is_empty()).then(|| typed.to_string());
        let mut picked: Vec<String> = self.checked[self.tab]
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(i, _)| label(i))
            .collect();
        if picked.is_empty() && text.is_none() {
            picked.push(label(i));
        }
        Answer {
            picked,
            text,
            ..Answer::default()
        }
    }

    /// 记下这一道的回答，跳到下一道没答的；都答了到「确认」页，只有一道题的直接交。
    fn settle(&mut self, answer: Answer) -> Step {
        self.answers[self.tab] = Some(answer);
        if !self.has_review() {
            return Step::Done(Outcome::Answered(self.answered()));
        }
        let n = self.pages();
        self.tab = (1..n)
            .map(|k| (self.tab + k) % n)
            .find(|&p| self.answers[p].is_none())
            .unwrap_or(n);
        Step::Stay
    }
}
