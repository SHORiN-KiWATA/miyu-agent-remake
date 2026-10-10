//! 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」，2026-10-11 项目主人定）：场所会话、属主不是
//! 管理员的（群会话、陌生人的私聊、成员的私聊）。文件类工具要碰的每一条都要在这一轮的工作区（或者加进来的目录）里，越过的一律
//! 拒（这些会话没人能确认，A11）；命令要等沙盒把读也关进工作区（5-12 下）才放开，现在一律拒。管理员本人的私聊不限。
//!
//! 拒绝时的两句不进策略快照，这种会话造、载入时读（同 `not-in-venue.txt`）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_fs::within;
use miyu_kernel::event::Said;
use miyu_kernel::template::{Template, TemplateError};
use miyu_kernel::tool::Worded;

/// 拒绝时写给她的两句。
pub(crate) struct Confined {
    /// 要碰的路径在工作区外面（`core/permissions/outside-workspace.txt`），字段 `path`。
    outside: Template,
    /// 这里还不能跑命令（`core/permissions/no-commands.txt`）。
    no_commands: String,
}

impl Confined {
    /// 照出厂的两句造（[`miyu_store::resources::ResourceRoot::confined_texts`]）。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了 `path` 以外的字段。
    pub(crate) fn new(outside: &str, no_commands: String) -> Result<Confined, TemplateError> {
        let outside = Template::parse(outside)?;
        outside.render(&BTreeMap::from([("path", "")]))?;
        Ok(Confined {
            outside,
            no_commands,
        })
    }

    /// `path` 在工作区外面。说法是 `core/permissions/outside-workspace`。
    pub(crate) fn outside(&self, path: &str) -> Worded {
        let text = self
            .outside
            .render(&BTreeMap::from([("path", path)]))
            .unwrap_or_default();
        Worded {
            text,
            said: Some(Said::new("core/permissions/outside-workspace").with("path", path)),
        }
    }

    /// 这里还不能跑命令。说法是 `core/permissions/no-commands`。
    pub(crate) fn no_commands(&self) -> Worded {
        Worded {
            text: self.no_commands.clone(),
            said: Some(Said::new("core/permissions/no-commands")),
        }
    }
}

/// 真实的位置 `real` 在不在工作区 `workspace` 或者加进来的目录 `dirs` 里（都是真实的位置）。
pub(crate) fn inside(real: &Path, workspace: &Path, dirs: &[PathBuf]) -> bool {
    within(real, workspace) || dirs.iter().any(|dir| within(real, dir))
}
