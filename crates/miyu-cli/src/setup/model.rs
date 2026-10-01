//! 选主对话的模型（`docs/blueprint/cli/setup.md`「怎么走」第 9 条，施工 8-11）：试的那一个（推荐的）排第一，别的照列出的
//! 先后；最多列 [`SHOWN`] 个，多的敲名字。不在终端里的不问，用推荐的。

use super::flow::{Flow, Tried};
use super::pick::{Row, Typed, numbered, typed};
use crate::exit;
use crate::shown::{say, write};

/// 最多列几个模型。
const SHOWN: usize = 20;

impl Flow<'_> {
    /// 照 `tried` 选一个，交回模型名。
    pub(super) fn pick_model(&mut self, tried: &Tried) -> Result<String, u8> {
        if !self.console.terminal() {
            return Ok(tried.model.clone());
        }
        let language = self.plan.language;
        let ordered: Vec<&str> = std::iter::once(tried.model.as_str())
            .chain(
                tried
                    .models
                    .iter()
                    .map(String::as_str)
                    .filter(|model| *model != tried.model),
            )
            .collect();
        let shown = &ordered[..ordered.len().min(SHOWN)];
        let rows: Vec<Row> = shown
            .iter()
            .enumerate()
            .map(|(at, model)| {
                let suffix = if at == 0 { language.recommended() } else { "" };
                Row::usable(vec![format!("{model}{suffix}")])
            })
            .collect();
        say(self.err, language.models_heading());
        for line in numbered(&rows, None) {
            write(self.err, &line.paint(self.plan.gray));
        }
        if ordered.len() > shown.len() {
            say(self.err, &language.more_models(ordered.len() - shown.len()));
        }
        loop {
            match typed(self.ask(language.pick_model()), shown.len()) {
                Typed::Picked(at) => return Ok(shown[at].to_string()),
                Typed::Empty => return Ok(tried.model.clone()),
                Typed::End => {
                    say(self.err, language.not_picked());
                    return Err(exit::ERROR);
                }
                Typed::Zero => say(self.err, &language.not_listed("0")),
                Typed::Other(text) if ordered.contains(&text.as_str()) => return Ok(text),
                Typed::Other(text) => say(self.err, &language.not_listed(&text)),
            }
        }
    }
}
