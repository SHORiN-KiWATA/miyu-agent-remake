//! 提问的事件（`docs/designs/03-事件模型.md` 第三节「提问的事件怎么写」）：一个在跑的调用请人
//! 回答一组题；人的回答。

use serde::{Deserialize, Serialize};

use crate::id::CallId;

/// `question.asked`：一个在跑的调用请人回答一组题（`02-内核.md` 第六节「提问怎么走」）。
/// `by` 是那次调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionAsked {
    /// 问的是哪一次调用。
    pub call_id: CallId,
    /// 一组题，照先后。几道都行，内核不设上限。
    pub questions: Vec<Question>,
}

/// 一道题。人总能自己写，不用列成选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// 顶上那一排标签里的短名字；可以不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// 问的话。
    pub question: String,
    /// 几个选项；也可以一个都没有。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Choice>,
    /// 能不能多选；单选的不写。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multiple: bool,
}

/// 一个选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// 一行标题。同一道题里不重复，回答里写的就是它。
    pub label: String,
    /// 一行说明；可以不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `question.answered`：人对一组题的回答。`by` 是回答的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionAnswered {
    /// 回答的是哪一次调用的题。
    pub call_id: CallId,
    /// 照题目的先后，一道一条。
    pub answers: Vec<Response>,
}

/// 一道题的回答。两样都没有，就是这道没答。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// 选了哪几项，写选项的标题。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub picked: Vec<String>,
    /// 自己写的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// 一组回答对不对得上这组题：几道题几条；选的都是那道题的选项，不重复；单选的至多选一项。
pub fn fits(questions: &[Question], answers: &[Response]) -> bool {
    questions.len() == answers.len()
        && questions
            .iter()
            .zip(answers)
            .all(|(question, answer)| question.accepts(answer))
}

impl Question {
    /// 这一条回答对不对得上这道题。
    fn accepts(&self, answer: &Response) -> bool {
        let known = answer
            .picked
            .iter()
            .all(|label| self.options.iter().any(|choice| &choice.label == label));
        let distinct = answer
            .picked
            .iter()
            .enumerate()
            .all(|(k, label)| !answer.picked[..k].contains(label));
        known && distinct && (self.multiple || answer.picked.len() <= 1)
    }
}

#[cfg(test)]
mod tests;
