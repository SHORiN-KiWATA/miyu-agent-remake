//! 说不成的几种（`docs/blueprint/mcp.md`「出错」）。

use std::fmt;

use serde_json::Value;

/// 一次请求、一次握手没成。
#[derive(Debug, Clone, PartialEq)]
pub enum Failed {
    /// 管道断了：服务退出了、关了标准输出，或者读到一行写法不对、太长，这条连接不能再用。
    Closed,
    /// 等了限定的时间没回。
    TimedOut,
    /// 服务回了 JSON-RPC 的错：错误码、原话、附带的。
    Rpc {
        /// 错误码。
        code: i64,
        /// 原话。
        message: String,
        /// 附带的，没有的是空的。
        data: Option<Value>,
    },
    /// 回了，但写法不对：说哪里不对。
    Malformed(String),
    /// 两边没有共同的版本：服务列的那几个（旧时代是它握手时答的那一个）。
    NoCommonVersion(Vec<String>),
    /// 新时代的服务要客户端补信息再问一次（`input_required`）：不支持。
    InputRequired,
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failed::Closed => write!(f, "the MCP server closed the connection"),
            Failed::TimedOut => write!(f, "the MCP server did not answer in time"),
            Failed::Rpc { code, message, .. } => write!(f, "MCP error {code}: {message}"),
            Failed::Malformed(problem) => write!(f, "malformed MCP answer: {problem}"),
            Failed::NoCommonVersion(offered) => write!(
                f,
                "no protocol version in common (the server offers {})",
                offered.join(", ")
            ),
            Failed::InputRequired => write!(f, "the MCP server asked for more input"),
        }
    }
}

impl std::error::Error for Failed {}
