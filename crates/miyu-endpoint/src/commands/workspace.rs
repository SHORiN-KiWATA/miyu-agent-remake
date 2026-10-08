//! `/workspace <路径>`（施工 9-7 下，`docs/blueprint/protocol.md` 的 `command.run` 第 5 条）：同 `session.set_workspace` 只换
//! 工作目录，加进来的目录照旧。相对的照头带来的 `cwd`（头所在的目录）接，没带的照会话现在的工作区接；不带路径的只说现在
//! 在哪。写错的照 `set_workspace` 的那几种原因拒绝；太宽的退回账号的工作区，回执说一声。

use std::path::Path;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;

use super::{Said, accepted, command};
use crate::Core;
use crate::refusal::Refusal;
use crate::sessions::Found;
use crate::workspace::{checked, home};

/// 头带来的 `cwd` 合不合写法：绝对的，或者 `~` 开头的。
pub(super) fn head_cwd_ok(cwd: &str) -> bool {
    Path::new(cwd).is_absolute() || miyu_fs::tilde(cwd).is_some()
}

/// 照 `asked`（命令后面跟的字，去掉了前后空白）换会话 `found` 的工作目录，交回追加的序号和回执。`head` 是头所在的目录。
pub(super) async fn run(
    core: &Core,
    session: &SessionId,
    found: &Found,
    id: &CommandId,
    by: &By,
    asked: &str,
    head: Option<&str>,
) -> Result<(Vec<u64>, Said), Refusal> {
    if asked.is_empty() {
        let said = Said {
            key: "commands/workspace-is",
            fields: vec![("cwd", found.cwd.clone())],
        };
        return Ok((Vec::new(), said));
    }
    let wanted = absolute(core, asked, head.unwrap_or(&found.cwd))?;
    let cwd = checked(core, &wanted)?;
    let change = Command::SetWorkspace {
        cwd: cwd.clone(),
        dirs: None,
    };
    let events = accepted(command(core, session, &found.handle, id, by, change).await?)?;
    core.sessions.moved(session, cwd.clone(), None).await;
    let said = if cwd == wanted {
        Said {
            key: "commands/workspace",
            fields: vec![("cwd", cwd)],
        }
    } else {
        Said {
            key: "commands/workspace-too-wide",
            fields: vec![("asked", asked.to_string()), ("cwd", cwd)],
        }
    };
    Ok((events, said))
}

/// 人写的路径：绝对的、`~` 开头的照原样，相对的照 `base` 接成真实的位置。
fn absolute(core: &Core, asked: &str, base: &str) -> Result<String, Refusal> {
    if Path::new(asked).is_absolute() || miyu_fs::tilde(asked).is_some() {
        return Ok(asked.to_string());
    }
    let home = home(core);
    let base = miyu_fs::resolve(Path::new("/"), home.as_deref(), base)
        .map_err(|_| Refusal::PATH_UNREADABLE)?;
    let real =
        miyu_fs::resolve(&base, home.as_deref(), asked).map_err(|_| Refusal::PATH_UNREADABLE)?;
    Ok(real.to_string_lossy().into_owned())
}
