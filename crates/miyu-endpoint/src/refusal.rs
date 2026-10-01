//! 拒绝（`docs/designs/04-核心协议.md` 第六节第 4 条、第九节「先做的几样怎么写」）：给程序看的原因码是
//! 稳定的英文，给人看的话照头的语言写。JSON-RPC 自己的几种照它的标准码；Miyu 的一律 `-32010`。

use miyu_kernel::session::Reason;

/// 一次拒绝：JSON-RPC 的错误码，和原因码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// JSON-RPC 的错误码。
    pub(crate) code: i64,
    /// 原因码：稳定的英文，给程序看。
    pub(crate) reason: &'static str,
    /// `data` 里除了 `reason` 多的几格（施工 8-2：`unknown_config_key` 的 `problems`）；没有的是空的。
    pub(crate) data: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Miyu 自己的拒绝，一律这个码，原因写在 `data.reason` 里。
const REFUSED: i64 = -32010;

impl Refusal {
    /// 读不懂：不是 JSON，或者一行太长。
    pub(crate) const PARSE: Refusal = Refusal {
        code: -32700,
        reason: "parse_error",
        data: None,
    };
    /// 是 JSON，但不是请求。
    pub(crate) const INVALID: Refusal = Refusal {
        code: -32600,
        reason: "invalid_request",
        data: None,
    };
    /// 没有这个方法。
    pub(crate) const UNKNOWN_METHOD: Refusal = Refusal {
        code: -32601,
        reason: "unknown_method",
        data: None,
    };
    /// 参数不对。
    pub(crate) const BAD_PARAMS: Refusal = Refusal {
        code: -32602,
        reason: "bad_params",
        data: None,
    };
    /// 核心自己出了问题：装坏了、磁盘上建不成。
    pub(crate) const INTERNAL: Refusal = Refusal {
        code: -32603,
        reason: "internal_error",
        data: None,
    };
    /// 连上以后第一条不是 `hello`。
    pub(crate) const HELLO_FIRST: Refusal = Refusal {
        code: REFUSED,
        reason: "hello_first",
        data: None,
    };
    /// 头支持的主版本和核心的没有交集。
    pub(crate) const PROTOCOL: Refusal = Refusal {
        code: REFUSED,
        reason: "protocol_mismatch",
        data: None,
    };
    /// 本机令牌不对。
    pub(crate) const BAD_TOKEN: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_token",
        data: None,
    };
    /// 没有这个人格。
    pub(crate) const UNKNOWN_PERSONA: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_persona",
        data: None,
    };
    /// 没有这个会话。
    pub(crate) const NOT_FOUND: Refusal = Refusal {
        code: REFUSED,
        reason: "session_not_found",
        data: None,
    };
    /// 会话停了：写不进去、出了 bug。
    pub(crate) const STOPPED: Refusal = Refusal {
        code: REFUSED,
        reason: "session_stopped",
        data: None,
    };
    /// 会话载入不了：日志或者策略快照坏了、读不了。
    pub(crate) const BROKEN: Refusal = Refusal {
        code: REFUSED,
        reason: "session_broken",
        data: None,
    };
    /// 没有这个任务，或者它已经结束了（施工 7-4，`job.stop`）。
    pub(crate) const UNKNOWN_JOB: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_job",
        data: None,
    };
    /// 读输出的是子代理，不是后台命令（施工 7-4 补，`job.output`）：头订阅它的子会话看。
    pub(crate) const NOT_A_COMMAND: Refusal = Refusal {
        code: REFUSED,
        reason: "not_a_command",
        data: None,
    };
    /// 加进来的目录太宽（施工 5-10 上）：家目录、根目录、包含数据根的、落在数据根里的。
    pub(crate) const DIR_TOO_WIDE: Refusal = Refusal {
        code: REFUSED,
        reason: "dir_too_wide",
        data: None,
    };
    /// `blob.put` 读不了这个文件（施工 3-9 三补）：换不成真实的位置、没有、不是普通文件、没有权限。
    pub(crate) const ATTACHMENT_UNREADABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "attachment_unreadable",
        data: None,
    };
    /// 附件太大（施工 3-9 三补）：超过 20 MiB；图片超过 5 MiB，或者哪一边超过 8000 像素。
    pub(crate) const ATTACHMENT_TOO_BIG: Refusal = Refusal {
        code: REFUSED,
        reason: "attachment_too_big",
        data: None,
    };
    /// `blob.put` 的文件在数据根里、管理员的工作区以外（施工 3-9 三补）。
    pub(crate) const ATTACHMENT_IN_DATA_ROOT: Refusal = Refusal {
        code: REFUSED,
        reason: "attachment_in_data_root",
        data: None,
    };
    /// `session.send` 附的 blob 这个核心里没有（施工 3-9 三补）。
    pub(crate) const UNKNOWN_ATTACHMENT: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_attachment",
        data: None,
    };

    /// `config.trust` 时这个目录找不到项目配置（施工 8-3）。
    pub(crate) const NO_PROJECT_CONFIG: Refusal = Refusal {
        code: REFUSED,
        reason: "no_project_config",
        data: None,
    };

    /// `secret.delete` 删的密钥没有（施工 8-5）。
    pub(crate) const UNKNOWN_SECRET: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_secret",
        data: None,
    };
    /// 请求里写了清单里没有的配置项（施工 8-2，`config.schema`、`config.get`、`config.set`）：`data.problems` 里每个不认识的
    /// 一条。
    pub(crate) fn unknown_config_key(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "unknown_config_key",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// `config.set` 的值不对、不能写在这一层，整份换的字里有错误（施工 8-3）：`data.problems` 里是每一处。
    pub(crate) fn config_invalid(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "config_invalid",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// 文件现在读不进来，没法只改几项（施工 8-3）：`data.problems` 里是那几处。
    pub(crate) fn config_file_broken(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "config_file_broken",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// `config.set` 的 `expect` 对不上（施工 8-3）：`data.current` 是这一层里这一项现在的样子，`{"value": …}` 或 `{}`。
    pub(crate) fn config_conflict_current(current: serde_json::Value) -> Refusal {
        Refusal::with("config_conflict", "current", current)
    }

    /// 版本对不上（施工 8-3）：整份换的、信任的那一份人看过以后又变了，写的那一瞬间有人手改了。`data.version` 是现在的
    /// 版本，文件没有的是 `null`。
    pub(crate) fn config_conflict_version(version: Option<String>) -> Refusal {
        Refusal::with("config_conflict", "version", serde_json::json!(version))
    }

    /// Miyu 的拒绝，`data` 里除了 `reason` 多一格 `field`。
    fn with(reason: &'static str, field: &str, value: serde_json::Value) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert(field.to_string(), value);
        Refusal {
            code: REFUSED,
            reason,
            data: Some(data),
        }
    }

    /// 内核拒了这个命令。
    pub(crate) fn kernel(reason: Reason) -> Refusal {
        Refusal {
            code: REFUSED,
            reason: reason.code(),
            data: None,
        }
    }

    /// 给人看的话，照头的语言。
    pub(crate) fn message(&self, locale: Locale) -> &'static str {
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
            "attachment_unreadable" => (
                "读不了这个文件：没有、不是普通文件，或者没有权限。",
                "This file cannot be read: it is missing, not a regular file, or not permitted.",
            ),
            "attachment_too_big" => (
                "附件太大：一个最多 20 MiB，图片最多 5 MiB、每边最多 8000 像素。",
                "The attachment is too big: at most 20 MiB, and an image at most 5 MiB and 8000 pixels a side.",
            ),
            "attachment_in_data_root" => (
                "Miyu 的数据根里的文件不能当附件。",
                "Files in Miyu's data root cannot be attached.",
            ),
            "unknown_attachment" => (
                "附件不在核心里：先用 blob.put 传上来。",
                "The attachment is not in the core; upload it with blob.put first.",
            ),
            "not_running" => (
                "没有正在进行的回合，打断不了。",
                "No turn is running, so there is nothing to interrupt.",
            ),
            "turn_running" => (
                "有回合在进行：先打断，或者等它做完。",
                "A turn is running; interrupt it or wait for it to finish.",
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
            "unknown_job" => (
                "没有这个任务，或者它已经结束了。",
                "There is no such job, or it has already ended.",
            ),
            "not_a_command" => (
                "这是子代理，不是后台命令：去看它的会话。",
                "This is a subagent, not a background command; open its session instead.",
            ),
            "nothing_to_compact" => (
                "没有能压的：还没压过的内容都在原样留着的最近一段里。",
                "Not enough to compact: everything not yet compacted is in the recent part that stays as it is.",
            ),
            // 2026-09-30 项目主人定（施工 6-8 补）：头把它当一条提示通知显示。
            "nothing_to_clear" => ("上下文为空", "The context is empty."),
            // 2026-09-30 项目主人定（施工 4-7 再补）：两种情况一句话，头把它当一条提示通知显示。
            "not_redoable" => ("无法重做", "Cannot redo."),
            // 2026-10-01 主会话定（施工 3-8 四补）。
            "nothing_to_recap" => ("还没有可回顾的内容", "There is nothing to recap yet."),
            // 施工 8-2（`config.md`「协议拒绝时的话」）。
            "unknown_config_key" => ("没有这一项配置。", "There is no such setting."),
            // 施工 8-3（`config.md`「协议拒绝时的话」）。
            "config_invalid" => (
                "配置有几处不对，没有改。",
                "Some settings are not right. Nothing was changed.",
            ),
            "config_conflict" => (
                "这一项刚被别处改过，没有改：先看看现在的值。",
                "This was just changed elsewhere. Nothing was changed. Look at the current value first.",
            ),
            "config_file_broken" => (
                "配置文件现在读不进来，没法只改一项：先把它改好，比如用 miyu config edit。",
                "The config file cannot be read right now, so a single setting cannot be changed. Fix the file first, e.g. with miyu config edit.",
            ),
            "no_project_config" => (
                "这个目录找不到项目配置。",
                "There is no project config for this directory.",
            ),
            "unknown_secret" => ("没有这个密钥。", "There is no such secret."),
            "recap_failed" => (
                "回顾没写成：请求模型出错了。",
                "The recap could not be written: the model request failed.",
            ),
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
