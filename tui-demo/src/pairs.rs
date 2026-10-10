//! 一轮轮的示范对话怎么编（蓝图「配置页」第 41 条、「第一次打开的引导」第 21 条）：配置页的示范对话窗和引导里建人格
//! 那一步共用。只管数据和按键；存不存、画成什么样各自管。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::Editor;

/// 一轮示范对话。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pair {
    /// 人说的。
    pub user: String,
    /// 她答的。
    pub assistant: String,
}

/// 列表按了一个键以后要做的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKey {
    /// 接着开着，没改。
    Stay,
    /// 挪了、删了：要存。
    Changed,
    /// 开两格窗：改第几轮，`None` 是加一轮。
    Edit(Option<usize>),
    /// 关掉列表。
    Close,
}

/// 列表的按键：上下挪，`Enter` 改这一轮，`a` 加一轮，`d` 删，`J` `K`（`Shift+↓` `Shift+↑`）往下往上挪，`Esc` 关。
pub fn list_key(pairs: &mut Vec<Pair>, sel: &mut usize, key: KeyEvent) -> ListKey {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let last = pairs.len().saturating_sub(1);
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => ListKey::Close,
        KeyCode::Char('J') | KeyCode::Down if shift || key.code == KeyCode::Char('J') => {
            if *sel < last {
                pairs.swap(*sel, *sel + 1);
                *sel += 1;
                return ListKey::Changed;
            }
            ListKey::Stay
        }
        KeyCode::Char('K') | KeyCode::Up if shift || key.code == KeyCode::Char('K') => {
            if *sel > 0 && !pairs.is_empty() {
                pairs.swap(*sel, *sel - 1);
                *sel -= 1;
                return ListKey::Changed;
            }
            ListKey::Stay
        }
        KeyCode::Char('j') | KeyCode::Down => {
            *sel = (*sel + 1).min(last);
            ListKey::Stay
        }
        KeyCode::Char('k') | KeyCode::Up => {
            *sel = sel.saturating_sub(1);
            ListKey::Stay
        }
        KeyCode::Char('a') => ListKey::Edit(None),
        KeyCode::Enter if !pairs.is_empty() => ListKey::Edit(Some(*sel)),
        KeyCode::Char('d') if !pairs.is_empty() => {
            pairs.remove(*sel);
            *sel = (*sel).min(pairs.len().saturating_sub(1));
            ListKey::Changed
        }
        _ => ListKey::Stay,
    }
}

/// 两格窗按了一个键以后要做的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormKey {
    /// 接着开着。
    Stay,
    /// 不存，回列表。
    Cancel,
    /// 两格都写了：这一轮。
    Done(Pair),
    /// 有一格空着：光标已经跳过去。
    Missing,
}

/// 两格窗的两格，照这一轮的字填好。
pub fn opened(pair: &Pair) -> [Editor; 2] {
    let side = |text: &str| {
        let mut editor = Editor::default();
        editor.set(text);
        editor
    };
    [side(&pair.user), side(&pair.assistant)]
}

/// 两格窗的按键：`↑` `↓`（`Tab` 也行）换格；在「你问的」按 `Enter` 跳到「AI回的」，在「AI回的」按 `Enter` 交回这一轮
/// （有一格空着的光标跳过去）；`Esc` 不存；别的照常打字（2026-10-08 项目主人）。
pub fn form_key(sides: &mut [Editor; 2], focus: &mut usize, key: KeyEvent) -> FormKey {
    match key.code {
        KeyCode::Esc => return FormKey::Cancel,
        KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab => *focus = 1 - *focus,
        KeyCode::Enter if *focus == 0 => *focus = 1,
        KeyCode::Enter => {
            let [user, assistant] = sides.each_ref().map(|e| e.text().trim().to_string());
            if user.is_empty() || assistant.is_empty() {
                *focus = usize::from(!user.is_empty());
                return FormKey::Missing;
            }
            return FormKey::Done(Pair { user, assistant });
        }
        _ => {
            let editor = &mut sides[*focus];
            match key.code {
                KeyCode::Backspace => editor.backspace(),
                KeyCode::Delete => editor.delete(),
                KeyCode::Left => editor.left(false),
                KeyCode::Right => editor.right(false),
                KeyCode::Home => editor.move_to(0, false),
                KeyCode::End => editor.move_to(editor.text().len(), false),
                KeyCode::Char(c) => editor.insert(&c.to_string()),
                _ => {}
            }
        }
    }
    FormKey::Stay
}

/// 改好的一轮放回去：改的照原位，加的放末尾、选中它。
pub fn put(pairs: &mut Vec<Pair>, sel: &mut usize, index: Option<usize>, pair: Pair) {
    match index {
        Some(i) if i < pairs.len() => pairs[i] = pair,
        _ => {
            pairs.push(pair);
            *sel = pairs.len() - 1;
        }
    }
}
