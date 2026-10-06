//! 剧本：模型每次请求说什么，每次调工具怎么回。执行前的链、回合开始的挂接点照默认的来：放行，
//! 不注入；要别的，交给 [`super::Stage`] 另排。

use crate::event::{CallError, ErrorClass, JobKind, JobStarted, Question, Todo, Usage};
use crate::id::{JobId, SessionId};

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
    /// 超长时驱动解析出超了多少 token（施工 6-6 中）。
    pub excess: Option<u64>,
    /// 说到一半停住。
    pub hold: bool,
    /// 说完了报的用量；没有的照默认：没命中缓存的 100，输出 10（施工 6-2 上）。
    pub usage: Option<Usage>,
    /// 出错时端口说换了端点（施工 8-9）。
    pub failover: bool,
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
            excess: None,
            hold: false,
            usage: None,
            failover: false,
        }
    }

    /// 出错：请求发出去了，一段增量都没来，驱动报了这个分类和原话。
    pub fn fails(class: ErrorClass, message: &str) -> Line {
        Line {
            error: Some(CallError {
                class,
                message: message.to_string(),
                status: None,
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

    /// 同样的出错，端口说换了端点（施工 8-9）：内核不管分类当场再来。
    pub fn fails_over(self) -> Line {
        Line {
            failover: true,
            ..self
        }
    }

    /// 同样的超长，驱动解析出超了 `tokens`（施工 6-6 中）。
    pub fn exceeds(self, tokens: u64) -> Line {
        Line {
            excess: Some(tokens),
            ..self
        }
    }

    /// 想了、说了或者调了点什么：出错的也要先送出去。
    pub(super) fn says_something(&self) -> bool {
        !self.reasoning.is_empty() || !self.text.is_empty() || !self.calls.is_empty()
    }

    /// 同样的回复，说完了报的用量一共是 `total`：都算没命中缓存的输入（施工 6-2 上）。
    pub fn reports(self, total: u64) -> Line {
        Line {
            usage: Some(Usage {
                uncached: total,
                cache_read: 0,
                cache_write: 0,
                output: 0,
            }),
            ..self
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
    /// 读了一个文件（施工 6-5）：结果是它的内容，报 `file.read`，真实的位置是 `path`。
    Read {
        /// 真实的位置。
        path: String,
        /// 内容。
        text: String,
    },
    /// 派出去一个任务（施工 7-2）：结果是这段字，报 `job.started`。
    Starts {
        /// 结果。
        text: String,
        /// 派出去的任务。
        started: JobStarted,
    },
    /// 给子代理 `job` 留了言（施工 7-7）：报 `job.messaged`。
    Messages(JobId),
    /// 订了会话的「空了告诉我」（施工 C-6）：报 `peer.watch`。
    Watches(SessionId),
    /// 换上一份待办（施工 D-3）：报 `todo.written`。
    Writes(Vec<Todo>),
}

impl Play {
    /// 执行完了，结果是 `text`。
    pub fn done(text: &str) -> Play {
        Play::Done(text.to_string())
    }

    /// 读了真实的位置 `path` 上的文件，内容是 `text`（施工 6-5）。
    pub fn read(path: &str, text: &str) -> Play {
        Play::Read {
            path: path.to_string(),
            text: text.to_string(),
        }
    }

    /// 派出去一个后台命令，编号是 `j<job>`，标题是 `title`（施工 7-2）。
    ///
    /// # Panics
    ///
    /// `job` 是 0：任务编号从 1 数起。
    pub fn starts_command(job: u64, title: &str) -> Play {
        Play::starts(job, title, JobKind::Command, None)
    }

    /// 派出去一个子代理，编号是 `j<job>`，标题是 `title`，子会话是 `session`（施工 7-2）。
    ///
    /// # Panics
    ///
    /// `job` 是 0，或者 `session` 不是会话编号的写法。
    pub fn starts_agent(job: u64, title: &str, session: &str) -> Play {
        let session =
            SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        Play::starts(job, title, JobKind::Agent, Some(session))
    }

    fn starts(job: u64, title: &str, what: JobKind, session: Option<SessionId>) -> Play {
        let job = JobId::new(job).unwrap_or_else(|| panic!("任务编号从 1 数起"));
        Play::Starts {
            text: format!("Started {job}."),
            started: JobStarted {
                job,
                what,
                title: title.to_string(),
                session,
            },
        }
    }

    /// 给子代理 `j<job>` 留了言（施工 7-7）：结果是一句送到了，报 `job.messaged`。
    ///
    /// # Panics
    ///
    /// `job` 是 0。
    pub fn messages(job: u64) -> Play {
        Play::Messages(JobId::new(job).unwrap_or_else(|| panic!("任务编号从 1 数起")))
    }

    /// 订了会话 `session` 的「空了告诉我」（施工 C-6）：结果是一句订了，报 `peer.watch`。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法。
    pub fn watches(session: &str) -> Play {
        Play::Watches(
            SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}")),
        )
    }

    /// 同样的回法，跑到一半停住。
    pub fn held(self) -> Play {
        Play::Held(Box::new(self))
    }
}
