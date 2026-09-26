//! 剧本：模型每次请求说什么，每次调工具怎么回。执行前的链、回合开始的挂接点照默认的来：放行，
//! 不注入；要别的，交给 [`super::Stage`] 另排。

use crate::event::{CallError, ErrorClass, Question};

/// 模型的一次回复：说的话、调的工具；或者出错。
///
/// 停住的（[`Line::held`]）：增量都送了，不送说完了，等 [`super::Stage::release_model`] 放行，或者
/// 被打断掐掉。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 正文；空的就不说话。
    pub text: String,
    /// 调用的工具，照先后：名字，和参数的原文。
    pub calls: Vec<(String, String)>,
    /// 出错的分类和原话。出错的不说话、不调工具。
    pub error: Option<CallError>,
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
            text: text.to_string(),
            calls: calls
                .iter()
                .map(|(name, args)| (name.to_string(), args.to_string()))
                .collect(),
            error: None,
            hold: false,
        }
    }

    /// 出错：请求发出去了，一段增量都没来，驱动报了这个分类和原话。
    pub fn fails(class: ErrorClass, message: &str) -> Line {
        Line {
            text: String::new(),
            calls: Vec::new(),
            error: Some(CallError {
                class,
                message: message.to_string(),
            }),
            hold: false,
        }
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
