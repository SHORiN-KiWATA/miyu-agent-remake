//! 事实的模板装进快照（`docs/blueprint/policy.md`「`CoreTexts`」）：`resources/core/facts/` 下的几份原文。会话编号那一份
//! （施工 1-13 再补）、切了级别以后的权限那一份（施工 2-7 补）以前造的快照里没有，读成没有、不写。施工 2-7 补从
//! `snapshot.rs` 挪出来（行数门禁）。

use miyu_kernel::facts::FactTemplates;
use serde::{Deserialize, Serialize};

use crate::snapshot::BuildError;

/// 事实的模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactTexts {
    /// 环境（`env.txt`）。
    pub env: String,
    /// 权限级别（`permission.txt`）。
    pub permission: String,
    /// 回复没说完就断了（`reply-cut.txt`）。
    pub reply_cut: String,
    /// 会话编号（`session.txt`，施工 1-13 再补）。以前造的快照里没有，读成没有：那些会话不注入这一块；没有的不写，
    /// 旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// 切了级别以后的权限（`permission-changed.txt`，施工 2-7 补）。以前造的快照里没有，读成没有：那些会话切了照旧用
    /// `permission`；没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_changed: Option<String>,
}

impl FactTexts {
    /// 交给内核的样子：造的时候拿全部字段试换一次（`kernel/request.md`「事实」第 7 条）。
    ///
    /// # Errors
    ///
    /// 哪一份的写法坏了、要了没有的字段，说是 `fact templates`。
    pub(crate) fn templates(&self) -> Result<FactTemplates, BuildError> {
        FactTemplates::new(
            &self.env,
            &self.permission,
            &self.reply_cut,
            self.session.as_deref(),
            self.permission_changed.as_deref(),
        )
        .map_err(|error| BuildError::Texts {
            which: "fact templates",
            error,
        })
    }
}
