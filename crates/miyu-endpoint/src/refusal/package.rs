//! 装、卸软件包时的三种拒绝（施工 F-5 上，`packages.md`「装卸」）：`refusal.rs` 到了 500 行，挪到这里。施工 F-6 上多开关的两种
//! （`package-pages.md`「开关」「程序不在就当没装」）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 装的包和出厂的同一个编号（施工 F-5 上）。
    pub(crate) const PACKAGE_EXISTS: Refusal = Refusal {
        code: REFUSED,
        reason: "package_exists",
        data: None,
    };
    /// 卸必需的包（施工 F-5 上）：基础系统。
    pub(crate) const PACKAGE_REQUIRED: Refusal = Refusal {
        code: REFUSED,
        reason: "package_required",
        data: None,
    };
    /// 包的程序不在 `miyu` 旁边（施工 F-6 上）：当没装，开不了。
    pub(crate) const PROGRAM_MISSING: Refusal = Refusal {
        code: REFUSED,
        reason: "program_missing",
        data: None,
    };
    /// 这个包没有开关（施工 F-6 上）：必需的、界面、小程序。
    pub(crate) const NOT_SWITCHABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "not_switchable",
        data: None,
    };
    /// 这个包没有后台页（施工 F-6 中，`package-pages.md`「`package.file`」）。
    pub(crate) const NO_PAGE: Refusal = Refusal {
        code: REFUSED,
        reason: "no_page",
        data: None,
    };
    /// 后台页里没有这份文件、跑出了后台页的目录、不是普通文件（施工 F-6 中）。
    pub(crate) const FILE_NOT_FOUND: Refusal = Refusal {
        code: REFUSED,
        reason: "not_found",
        data: None,
    };
    /// 包的程序没连着：没开、停了、还没登记方法（施工 F-6 中，`package.call`）。
    pub(crate) const NOT_RUNNING: Refusal = Refusal {
        code: REFUSED,
        reason: "program_not_running",
        data: None,
    };
    /// 程序到时限没回（施工 F-6 中）。
    pub(crate) const METHOD_TIMEOUT: Refusal = Refusal {
        code: REFUSED,
        reason: "method_timeout",
        data: None,
    };
    /// 程序没登记这个方法（施工 F-6 中）：`data.method` 是哪一个。
    pub(crate) fn unregistered(method: &str) -> Refusal {
        Refusal::with(
            "unregistered",
            "method",
            serde_json::Value::String(method.to_string()),
        )
    }
    /// 程序回了错（施工 F-6 中）：`data.message` 是它说的那一句，`data.code` 是它的错误码，没有的不写。
    pub(crate) fn method_failed(message: String, code: Option<serde_json::Value>) -> Refusal {
        let mut refusal = Refusal::with(
            "method_failed",
            "message",
            serde_json::Value::String(message),
        );
        if let (Some(data), Some(code)) = (&mut refusal.data, code) {
            data.insert("code".to_string(), code);
        }
        refusal
    }
    /// 装的清单写错了、和别的包撞了（施工 F-5 上）：`data.problem` 照连接的语言说一句，知道第几行的带 `data.line`。
    pub(crate) fn package_invalid(problem: String, line: Option<usize>) -> Refusal {
        let mut refusal = Refusal::with(
            "package_invalid",
            "problem",
            serde_json::Value::String(problem),
        );
        if let (Some(data), Some(line)) = (&mut refusal.data, line) {
            data.insert("line".to_string(), serde_json::json!(line));
        }
        refusal
    }
}
