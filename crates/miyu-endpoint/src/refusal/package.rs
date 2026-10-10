//! 装、卸软件包时的三种拒绝（施工 F-5 上，`packages.md`「装卸」）：`refusal.rs` 到了 500 行，挪到这里。

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
