//! 提问的端口（施工 D-2，`docs/blueprint/tools/ask_user.md`，`tools/interface.md`）：`ask_user` 这件工具只拿它，不认识内核。
//! 执行器照每一次调用造一个：工具交一组题，执行器交给内核记 `question.asked`、等人回答，回答落了盘再送回来。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）：这里只定义形状，执行器（`miyu-session`）照
//! 每一次调用造一个，交给 [`crate::Call::questions`]。没人能回答的会话、测试里的假调用没有，`ask_user` 照「这里没人能
//! 回答」出错。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::event::{Question, Response};

/// 这件工具的名字（`10-自带软件.md` 第三节）：只在有人能回答的本机主会话的工具面上。
pub const ASK_USER: &str = "ask_user";

/// 问人一组题，等回答。
pub trait QuestionPort: Send + Sync {
    /// 把 `questions` 交上去，等人回答：交回照题目先后、一道一条的回答。没答到就了结的（打断、等的时候来了一句话、
    /// 收紧成只读、核心停了）交回 `None`：这次调用的结果由内核照原来的规矩补，工具交回的不再记。
    fn ask(&self, questions: Vec<Question>) -> Answering<'_>;
}

/// 等回答的 future。
pub type Answering<'a> = Pin<Box<dyn Future<Output = Option<Vec<Response>>> + Send + 'a>>;

impl fmt::Debug for dyn QuestionPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QuestionPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn QuestionPort {
    fn eq(&self, other: &dyn QuestionPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn QuestionPort {}
