//! 拒绝（`docs/designs/04-核心协议.md` 第六节第 4 条、第九节「先做的几样怎么写」）：给程序看的原因码是
//! 稳定的英文，给人看的话照头的语言写。JSON-RPC 自己的几种照它的标准码；Miyu 的一律 `-32010`。

use miyu_kernel::session::Reason;

/// 一次拒绝：JSON-RPC 的错误码，和原因码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// JSON-RPC 的错误码。
    pub(crate) code: i64,
    /// 原因码：稳定的英文，给程序看。
    pub(crate) reason: &'static str,
}

/// Miyu 自己的拒绝，一律这个码，原因写在 `data.reason` 里。
const REFUSED: i64 = -32010;

impl Refusal {
    /// 读不懂：不是 JSON，或者一行太长。
    pub(crate) const PARSE: Refusal = Refusal {
        code: -32700,
        reason: "parse_error",
    };
    /// 是 JSON，但不是请求。
    pub(crate) const INVALID: Refusal = Refusal {
        code: -32600,
        reason: "invalid_request",
    };
    /// 没有这个方法。
    pub(crate) const UNKNOWN_METHOD: Refusal = Refusal {
        code: -32601,
        reason: "unknown_method",
    };
    /// 参数不对。
    pub(crate) const BAD_PARAMS: Refusal = Refusal {
        code: -32602,
        reason: "bad_params",
    };
    /// 核心自己出了问题：装坏了、磁盘上建不成。
    pub(crate) const INTERNAL: Refusal = Refusal {
        code: -32603,
        reason: "internal_error",
    };
    /// 连上以后第一条不是 `hello`。
    pub(crate) const HELLO_FIRST: Refusal = Refusal {
        code: REFUSED,
        reason: "hello_first",
    };
    /// 头支持的主版本和核心的没有交集。
    pub(crate) const PROTOCOL: Refusal = Refusal {
        code: REFUSED,
        reason: "protocol_mismatch",
    };
    /// 本机令牌不对。
    pub(crate) const BAD_TOKEN: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_token",
    };
    /// 没有这个人格。
    pub(crate) const UNKNOWN_PERSONA: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_persona",
    };
    /// 没有这个会话。
    pub(crate) const NOT_FOUND: Refusal = Refusal {
        code: REFUSED,
        reason: "session_not_found",
    };
    /// 会话停了：写不进去、出了 bug。
    pub(crate) const STOPPED: Refusal = Refusal {
        code: REFUSED,
        reason: "session_stopped",
    };
    /// 会话载入不了：日志或者策略快照坏了、读不了。
    pub(crate) const BROKEN: Refusal = Refusal {
        code: REFUSED,
        reason: "session_broken",
    };
    /// 加进来的目录太宽（施工 5-10 上）：家目录、根目录、包含数据根的、落在数据根里的。
    pub(crate) const DIR_TOO_WIDE: Refusal = Refusal {
        code: REFUSED,
        reason: "dir_too_wide",
    };

    /// 内核拒了这个命令。
    pub(crate) fn kernel(reason: Reason) -> Refusal {
        Refusal {
            code: REFUSED,
            reason: reason.code(),
        }
    }

    /// 给人看的话，照头的语言。
    pub(crate) fn message(self, locale: Locale) -> &'static str {
        let (zh, en) = match self.reason {
            "parse_error" => ("读不懂这条消息。", "The message could not be read."),
            "invalid_request" => ("这不是一条请求。", "This is not a request."),
            "unknown_method" => ("没有这个方法。", "There is no such method."),
            "bad_params" => ("参数不对。", "The parameters are not right."),
            "internal_error" => (
                "核心出了问题，详情在运行日志里。",
                "The core ran into a problem; the runtime log has the details.",
            ),
            "hello_first" => (
                "连上以后要先打招呼（hello）。",
                "Say hello first after connecting.",
            ),
            "protocol_mismatch" => (
                "头和核心的协议版本对不上，请把它们升级到同一个版本。",
                "The head and the core speak different protocol versions; upgrade them to the same release.",
            ),
            "bad_token" => ("本机令牌不对。", "The local token is wrong."),
            "unknown_persona" => ("没有这个人格。", "There is no such persona."),
            "session_not_found" => ("没有这个会话。", "There is no such session."),
            "session_stopped" => (
                "这个会话停了，详情在运行日志里；再发一次会重新载入。",
                "This session has stopped; the runtime log has the details. Sending again reloads it.",
            ),
            "session_broken" => (
                "这个会话载入不了：它的日志或者策略快照坏了。",
                "This session cannot be loaded: its log or policy snapshot is broken.",
            ),
            "empty_message" => ("消息是空的。", "The message is empty."),
            "dir_too_wide" => (
                "加进来的目录太宽：家目录、根目录、Miyu 的数据根不能整个放行。",
                "An added directory is too wide: the home directory, the root and Miyu's data root cannot be opened up whole.",
            ),
            "not_running" => (
                "没有正在进行的回合，打断不了。",
                "No turn is running, so there is nothing to interrupt.",
            ),
            "turn_running" => (
                "有回合在进行，撤销不了：先打断再撤。",
                "A turn is running; interrupt it before undoing.",
            ),
            "unknown_turn" => (
                "没有这一轮，或者它已经撤掉了。",
                "There is no such turn, or it has already been undone.",
            ),
            "nothing_to_unrevert" => (
                "没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。",
                "There is nothing to restore: nothing was undone, or a turn or compaction came since.",
            ),
            "restoring" => (
                "正在撤销、恢复，等它做完再来。",
                "An undo or restore is still in progress; try again when it is done.",
            ),
            "nothing_to_revert" => ("没有能撤销的回合。", "There is no turn to undo."),
            _ => ("被拒绝了。", "Refused."),
        };
        match locale {
            Locale::Zh => zh,
            Locale::En => en,
        }
    }
}

/// 给人看的话用哪种语言：头握手时报的 `locale`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Locale {
    /// `zh` 开头的。
    Zh,
    /// 别的，和还没握手的时候。
    #[default]
    En,
}

impl Locale {
    /// 照 `locale` 选：`zh` 开头的是中文。
    pub(crate) fn of(locale: Option<&str>) -> Locale {
        match locale {
            Some(locale) if locale.starts_with("zh") => Locale::Zh,
            _ => Locale::En,
        }
    }
}
