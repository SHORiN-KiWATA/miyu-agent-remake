//! 剧本：模型每次请求说什么，每次调工具怎么回。执行前的链、回合开始的挂接点照默认的来：放行，
//! 不注入；要别的，交给 [`super::Stage`] 另排。

use crate::event::{CallError, ErrorClass, Question};

/// 模型的一次回复：想的、说的话、调的工具；或者出错。
///
/// 停住的（[`Line::held`]）：增量都送了，不送说完了，等 [`super::Stage::release_model`] 放行，或者
/// 被打断掐掉。
///
/// 出错的：一般什么都不说就报错（[`Line::fails`]）；说了一半才断的（[`Line::breaks`]），想的、说的、
/// 调的都送出去，但一块都不收全，再报错。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 思考；空的就不想。排在最前面。
    pub reasoning: String,
    /// 正文；空的就不说话。
    pub text: String,
    /// 调用的工具，照先后：名字，和参数的原文。
    pub calls: Vec<(String, String)>,
    /// 出错的分类和原话。
    pub error: Option<CallError>,
    /// 出错时供应商说了要等多久，毫秒。
    pub wait_ms: Option<u64>,
    /// 说到一半停住。
    pub hold: bool,
}

impl Line {
    /// 说一句，不调工具。
    pub fn says(text: &str) -> Line {
        Line::calls(text, &[])
    }

    /// 说一句（可以是空的），调这几件工具：名字，和参数的原文。
    pub fn calls(text: &str, calls: &[(&str, &str)]) -> Line {
        Line {
            reasoning: String::new(),
            text: text.to_string(),
            calls: calls
                .iter()
                .map(|(name, args)| (name.to_string(), args.to_string()))
                .collect(),
            error: None,
            wait_ms: None,
            hold: false,
        }
    }

    /// 出错：请求发出去了，一段增量都没来，驱动报了这个分类和原话。
    pub fn fails(class: ErrorClass, message: &str) -> Line {
        Line {
            error: Some(CallError {
                class,
                message: message.to_string(),
            }),
            ..Line::says("")
        }
    }

    /// 说了一半断了：`text` 送出去，没收全，驱动报了这个分类和原话（施工 3-5 下）。
    pub fn breaks(text: &str, class: ErrorClass, message: &str) -> Line {
        Line {
            text: text.to_string(),
            ..Line::fails(class, message)
        }
    }

    /// 同样的回复，先想一段：思考排在最前面。
    pub fn thinking(self, reasoning: &str) -> Line {
        Line {
            reasoning: reasoning.to_string(),
            ..self
        }
    }

    /// 同样的出错，供应商说了要等 `wait_ms` 毫秒。
    pub fn waits(self, wait_ms: u64) -> Line {
        Line {
            wait_ms: Some(wait_ms),
            ..self
        }
    }

    /// 想了、说了或者调了点什么：出错的也要先送出去。
    pub(super) fn says_something(&self) -> bool {
        !self.reasoning.is_empty() || !self.text.is_empty() || !self.calls.is_empty()
    }

    /// 同样的回复，说到一半停住：增量都送了，等放行才送说完了。
    pub fn held(self) -> Line {
        Line { hold: true, ..self }
    }
}

/// 一次工具调用怎么回。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Play {
    /// 执行完了，结果是这段字。
    Done(String),
    /// 出错了，结果是这段字。
    Fails(String),
    /// 先问人这组题；人回答了，把回答写成结果。
    Asks(Vec<Question>),
    /// 跑到一半停住，等 [`super::Stage::release_tool`] 放行，再照里面那样回。
    Held(Box<Play>),
}

impl Play {
    /// 执行完了，结果是 `text`。
    pub fn done(text: &str) -> Play {
        Play::Done(text.to_string())
    }

    /// 同样的回法，跑到一半停住。
    pub fn held(self) -> Play {
        Play::Held(Box::new(self))
    }
}
