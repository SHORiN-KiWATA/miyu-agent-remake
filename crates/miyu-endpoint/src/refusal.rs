//! 拒绝（`docs/designs/04-核心协议.md` 第六节第 4 条、第九节「先做的几样怎么写」）：给程序看的原因码是
//! 稳定的英文，给人看的话照头的语言写。JSON-RPC 自己的几种照它的标准码；Miyu 的一律 `-32010`。

use miyu_kernel::session::Reason;

mod message;

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
    /// 没有这个预设（施工 P-2 上）。
    pub(crate) const UNKNOWN_PRESET: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_preset",
        data: None,
    };
    /// 换工作区换到的是一个文件，不是目录（施工 9-7 上）。
    pub(crate) const NOT_A_DIRECTORY: Refusal = Refusal {
        code: REFUSED,
        reason: "not_a_directory",
        data: None,
    };
    /// 没有这个编号的软件包，或者它的清单读不成（施工 9-4 上）。
    pub(crate) const UNKNOWN_PACKAGE: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_package",
        data: None,
    };
    /// 这个包是界面，不是核心拉起的扩展（施工 9-4 上）。
    pub(crate) const NOT_AN_EXTENSION: Refusal = Refusal {
        code: REFUSED,
        reason: "not_an_extension",
        data: None,
    };
    /// 重启一个关着的扩展（施工 9-4 上）。
    pub(crate) const EXTENSION_OFF: Refusal = Refusal {
        code: REFUSED,
        reason: "extension_off",
        data: None,
    };
    /// 没有这个会话。
    pub(crate) const NOT_FOUND: Refusal = Refusal {
        code: REFUSED,
        reason: "session_not_found",
        data: None,
    };
    /// 场所会话的属主该是系统账号，还没有（施工 O-3；系统账号随 O-4）。
    pub(crate) const NO_SYSTEM_ACCOUNT: Refusal = Refusal {
        code: REFUSED,
        reason: "no_system_account",
        data: None,
    };
    /// 场所会话只收代表外部的人说的话（施工 O-3，`venues.md`）：本机的头不能直接说话。
    pub(crate) const VENUE_SESSION: Refusal = Refusal {
        code: REFUSED,
        reason: "venue_session",
        data: None,
    };
    /// `check` 写的文件不是 Miyu 读的那几种（施工 8-30）。
    pub(crate) const UNKNOWN_FILE: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_file",
        data: None,
    };
    /// 斜杠命令认不出（施工 O-6，`command.run`）。
    pub(crate) const UNKNOWN_COMMAND: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_command",
        data: None,
    };
    /// 这个人不能用斜杠命令（施工 O-6）：群里既不是主人、也不是管理的人。
    pub(crate) const COMMAND_NOT_ALLOWED: Refusal = Refusal {
        code: REFUSED,
        reason: "command_not_allowed",
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
    /// 附件太大（施工 3-9 三补）：超过 20 MiB；图片超过 5 MiB，或者哪一边超过 8000 像素。分块上传
    /// `blob.open`、`blob.close` 共用这一种（施工 W-5）。
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
    /// `fs.list`、`fs.find` 读不了这个路径（施工 W-2）：换不成真实的位置、不在、该是目录的不是目录、没有权限。
    /// `fs.realpath` 也用它（施工 W-3）：换不成真实的位置——一层都不在、路上的链接指向不存在的地方、没有家目录。
    /// `fs.read` 也用它（施工 W-6）：换不成真实的位置、没有、不是普通文件、没有权限。
    pub(crate) const PATH_UNREADABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "path_unreadable",
        data: None,
    };
    /// `fs.list`、`fs.find` 的目录落在数据根里、又不在这个账号的工作区里（施工 W-2）。`fs.read` 的路径也一样
    /// （施工 W-6）。
    pub(crate) const PATH_FORBIDDEN: Refusal = Refusal {
        code: REFUSED,
        reason: "path_forbidden",
        data: None,
    };
    /// `blob.get` 的 blob 这个账号没有（施工 W-6）。
    pub(crate) const UNKNOWN_BLOB: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_blob",
        data: None,
    };
    // `mermaid_too_long`、`mermaid_failed`（施工 W-4）：查询方法（`queries.rs`）只拿得到
    // `queries::QueryError`，这两种拒绝经 `From<QueryError>` 现造，不在这里登记成常量。

    /// 这个连接上同时开着 4 个分块上传了（施工 W-5，`blob.open`）。
    pub(crate) const TOO_MANY_UPLOADS: Refusal = Refusal {
        code: REFUSED,
        reason: "too_many_uploads",
        data: None,
    };
    /// 没有这个上传：编号不对、作废了、不是这个连接开的（施工 W-5，`blob.write`、`blob.close`）。
    pub(crate) const UPLOAD_UNKNOWN: Refusal = Refusal {
        code: REFUSED,
        reason: "upload_unknown",
        data: None,
    };
    /// `blob.write` 的 `offset` 和已经收到的字节数对不上（施工 W-5）：`data.received` 是实际收到的几个。
    pub(crate) fn upload_offset(received: u64) -> Refusal {
        Refusal::with("upload_offset", "received", serde_json::json!(received))
    }
    /// `blob.close` 时还没收齐（施工 W-5）：`data.received` 是实际收到的几个。
    pub(crate) fn upload_incomplete(received: u64) -> Refusal {
        Refusal::with("upload_incomplete", "received", serde_json::json!(received))
    }

    /// `config.trust` 时这个目录找不到项目配置（施工 8-3）。
    /// 握手的一次性码对不上、过期了、用过了（施工 W-8）。
    pub(crate) const BAD_CODE: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_code",
        data: None,
    };
    /// 握手的登录令牌对不上、过期了、作废了（施工 W-8）。
    pub(crate) const BAD_LOGIN: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_login",
        data: None,
    };
    /// 握手的用户名、密码对不上（施工 W-8）：用户名不对和密码不对一样说。
    pub(crate) const BAD_PASSWORD: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_password",
        data: None,
    };
    /// 用一次性码连上的，先设密码（施工 W-8）。
    pub(crate) const SETUP_FIRST: Refusal = Refusal {
        code: REFUSED,
        reason: "setup_first",
        data: None,
    };
    /// 不是出示本机令牌连上的，要不了一次性码（施工 W-8）。
    pub(crate) const LOCAL_ONLY: Refusal = Refusal {
        code: REFUSED,
        reason: "local_only",
        data: None,
    };
    /// 这个用户名 60 秒内错了 5 次（施工 W-8）：`data.retry_after_ms` 还要等多久。
    pub(crate) fn login_throttled(retry_after_ms: u64) -> Refusal {
        Refusal::with(
            "login_throttled",
            "retry_after_ms",
            serde_json::json!(retry_after_ms),
        )
    }

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
    /// `model.list` 的 `provider` 不是配好了的（施工 8-7）。
    pub(crate) const UNKNOWN_PROVIDER: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_provider",
        data: None,
    };
    /// `session.create`、`session.configure` 的 `model` 解析不出（施工 8-8）：写法不对（连同以前的挡位名，施工 8-8 补）、没有
    /// 这家供应商、没有这个池、池是空的。
    pub(crate) const UNKNOWN_MODEL: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_model",
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

    /// 人格的文件写错了（施工 P-1 上，`personas.md`）：`data.problem` 写明哪一层、哪个文件第几行、错在哪。
    pub(crate) fn persona_invalid(problem: String) -> Refusal {
        Refusal::with(
            "persona_invalid",
            "problem",
            serde_json::Value::String(problem),
        )
    }

    /// 预设的文件写错了（施工 P-2 上，`presets.md`）：`data.problem` 写明哪一层、哪个文件第几行、错在哪。
    pub(crate) fn preset_invalid(problem: String) -> Refusal {
        Refusal::with(
            "preset_invalid",
            "problem",
            serde_json::Value::String(problem),
        )
    }

    /// `model.call` 没有能用的模型（施工 8-20）：`data.message` 是原话。
    pub(crate) fn no_model(message: String) -> Refusal {
        Refusal::with("no_model", "message", serde_json::Value::String(message))
    }

    /// `model.call` 的候选全在冷却，没发（施工 8-20）：`data.message` 是原话，`data.wait_ms` 是最早恢复的还要多久。
    pub(crate) fn cooling(message: String, wait_ms: u64) -> Refusal {
        let mut refusal = Refusal::with("cooling", "message", serde_json::Value::String(message));
        if let Some(data) = &mut refusal.data {
            data.insert("wait_ms".to_string(), serde_json::json!(wait_ms));
        }
        refusal
    }

    /// `model.call` 发了、出错了（施工 8-20）：`data` 是 `class`、`status`（有状态码的才写）、`message`，和
    /// `model.called` 的 `error` 一样。
    pub(crate) fn model_failed(error: &miyu_kernel::event::CallError) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert("class".to_string(), serde_json::json!(error.class.as_str()));
        if let Some(status) = error.status {
            data.insert("status".to_string(), serde_json::json!(status));
        }
        data.insert("message".to_string(), serde_json::json!(error.message));
        Refusal {
            code: REFUSED,
            reason: "model_failed",
            data: Some(data),
        }
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
        let (zh, en) = message::of(self.reason);
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

/// 可选软件包登记的查询拒绝时（施工 W-4，`queries.rs`），翻成协议上真正的拒绝：软件包的代码（`miyu-core`
/// 之类）不认得 JSON-RPC 的错误码，只拿得到 [`crate::queries::QueryError`] 这几种。
impl From<crate::queries::QueryError> for Refusal {
    fn from(error: crate::queries::QueryError) -> Refusal {
        use crate::queries::QueryError;
        match error {
            QueryError::BadParams => Refusal::BAD_PARAMS,
            QueryError::Internal => Refusal::INTERNAL,
            QueryError::Reason(reason) => Refusal {
                code: REFUSED,
                reason,
                data: None,
            },
            QueryError::ReasonWithDetail(reason, field, value) => {
                Refusal::with(reason, field, value)
            }
        }
    }
}
