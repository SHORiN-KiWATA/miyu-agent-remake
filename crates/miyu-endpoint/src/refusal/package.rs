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
