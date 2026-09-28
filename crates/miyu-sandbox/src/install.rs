//! 装沙盒要管理员权限的那一次（`docs/blueprint/sandbox/windows.md`，施工 5-8）：Windows 上建一个专用的低权限沙盒
//! 用户、藏起来，密码加密后记进本人的数据根；卸的时候撤干净。别的平台上不用装（[`needed`]）。
//!
//! - [`USER`]：沙盒用户的登录名；[`Owner`]：替谁装；[`InstallError`]：没成的原因；
//! - 只在 Windows 上有的：`setup`、`remove` 照次序干活，要在管理员身份下调；`elevated`、`elevate` 查是不是管理员、
//!   起提升过的自己；`report`、`reported`、`clear_report` 是提升过的自己没成时写给等它的那一个的原因。
//!
//! 调系统接口的几个模块（`elevate`、`user`、`dpapi`、`winsys`）只在 Windows 上编，`unsafe` 只在它们里面放开，每处
//! 写着为什么安全。只在 Windows 上用、却不调系统接口的几个（`password`、`record`、`report`、`files`、`quote`），编
//! 测试时也编进去：三个平台的测试都跑得到它们。

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod dpapi;
#[cfg(windows)]
mod elevate;
#[cfg(any(windows, test))]
mod files;
#[cfg(any(windows, test))]
mod password;
#[cfg(any(windows, test))]
mod quote;
#[cfg(any(windows, test))]
mod record;
#[cfg(any(windows, test))]
mod report;
#[cfg(windows)]
mod user;
#[cfg(windows)]
mod winsys;

#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;

#[cfg(windows)]
pub use elevate::{elevate, elevated};
#[cfg(windows)]
pub use report::{clear_report, report, reported};

/// 沙盒用户的登录名：固定一个。本机用户名最多 20 个字符，不许有空格。
pub const USER: &str = "miyu-sandbox";

/// 这个平台要不要装：只有 Windows 要（`sandbox/windows.md`「怎么走」第 1 条）。
pub const fn needed() -> bool {
    cfg!(windows)
}

/// 替谁装：本人的数据根和本人的 SID。
///
/// 提升过的自己可能是另一个管理员账号（标准用户在 UAC 里输了别人的密码）：它自己的家目录、数据根都不是本人的，所以
/// 记录写到哪、访问控制给谁，都从这里拿，不看自己的环境（2026-09-28 主会话审过时定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// 本人的数据根：绝对路径。
    home: PathBuf,
    /// 本人的 SID：核对过写法，拼进访问控制也不会多出别的规则。
    sid: String,
}

impl Owner {
    /// 核对过写法的主人：数据根要是绝对路径，SID 要是 `S-1-` 开头、二到十六段十进制数字的写法，别的字一个都不收。
    ///
    /// # Errors
    ///
    /// 数据根不是绝对路径、SID 的写法不对：`check owner` 这一步没成。
    pub fn new(home: PathBuf, sid: String) -> Result<Owner, InstallError> {
        if !home.is_absolute() {
            return Err(InstallError::failed(
                "check owner",
                format!("data root is not an absolute path: {}", home.display()),
            ));
        }
        if !valid_sid(&sid) {
            return Err(InstallError::failed(
                "check owner",
                format!("not a SID: {sid}"),
            ));
        }
        Ok(Owner { home, sid })
    }

    /// 本人的数据根。
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// 本人的 SID。
    pub fn sid(&self) -> &str {
        &self.sid
    }
}

/// SID 的写法对不对：`S-1-` 开头，后面二到十六段十进制数字（一段标识机构，一到十五段子机构），用 `-` 隔开。
///
/// 只收这些字：SID 要原样拼进访问控制的写法里（`D:P(A;;FA;;;<SID>)…`），多一个括号、分号就能多给别人一条权限。
fn valid_sid(sid: &str) -> bool {
    let Some(rest) = sid.strip_prefix("S-1-") else {
        return false;
    };
    let groups: Vec<&str> = rest.split('-').collect();
    (2..=16).contains(&groups.len())
        && groups
            .iter()
            .all(|group| !group.is_empty() && group.bytes().all(|b| b.is_ascii_digit()))
}

/// 装、卸没成的原因。提升过的自己没成时照原样写给等它的那一个（`report`），所以能写成一行 JSON。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "kebab-case")]
pub enum InstallError {
    /// 已经有一个叫 [`USER`] 的账号，而且是管理员：不动它。沙盒用户是管理员，沙盒就形同虚设。
    Administrator,
    /// 给的数据根里没有标记，不是 Miyu 的数据根：什么都不写。
    NotDataRoot {
        /// 给的数据根，写成给人看的样子。
        path: String,
    },
    /// 哪一步没成。
    Failed {
        /// 这一步的英文名字，例如 `create user`（`sandbox/windows.md`「怎么走」第 7 条）。
        step: String,
        /// 系统的原话。
        detail: String,
    },
}

impl InstallError {
    /// `step` 这一步没成，原话是 `detail`。
    pub fn failed(step: &str, detail: impl fmt::Display) -> InstallError {
        InstallError::Failed {
            step: step.to_string(),
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::Administrator => {
                write!(f, "an administrator account named {USER} already exists")
            }
            InstallError::NotDataRoot { path } => write!(f, "{path} is not a Miyu data root"),
            InstallError::Failed { step, detail } => write!(f, "{step}: {detail}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// 起提升过的自己没成（Windows 上的 `elevate`；定义在这里，三个平台上怎么走的测试都用得到它）。
#[derive(Debug)]
pub enum Elevation {
    /// 人在 UAC 里点了取消：什么都没改。
    Cancelled,
    /// 别的原因起不来、等不了：系统的原话。
    Failed(std::io::Error),
}

/// 装好（`sandbox/windows.md`「怎么走」第 4 条）：先认数据根，再照次序建用户、加进 `Users`、藏起来、查出 SID、
/// 密码加密，最后写记录。每一步幂等，装了一半的再跑一次接着装。要在管理员身份下调。
///
/// 认数据根放在最前面：给的地方不对，用户和密码一样都不碰，免得密码换了却没处记。
///
/// # Errors
///
/// 数据根没有标记；已经有这个名字的管理员账号；哪一步没成（[`InstallError`]）。
#[cfg(windows)]
pub fn setup(owner: &Owner) -> Result<(), InstallError> {
    files::check_root(owner.home())?;
    user::check(USER)?;
    let mut random = [0u8; password::LENGTH];
    getrandom::fill(&mut random).map_err(|error| InstallError::failed("create user", error))?;
    let password = password::generate(&random);
    user::ensure(USER, &password)?;
    user::join_users(USER)?;
    user::hide(USER)?;
    let sid = user::sid(USER)?
        .ok_or_else(|| InstallError::failed("look up user", format!("{USER} is not there")))?;
    let protected = dpapi::protect(password.as_bytes())
        .map_err(|error| InstallError::failed("protect password", error))?;
    let record = record::Record::new(USER.to_string(), sid, encode(&protected), now()?);
    record::write(owner, &record)
}

/// 撤干净（`sandbox/windows.md`「怎么走」第 5 条）：删 profile、删用户、取消隐藏、删记录。哪一样本来就没有，跳过
/// 不算错。要在管理员身份下调。
///
/// 先删用户、后取消隐藏：删到一半没成的，账号照样藏着，不会出现在登录界面上。
///
/// # Errors
///
/// 数据根没有标记；这个名字的账号是管理员；哪一步没成（[`InstallError`]）。
#[cfg(windows)]
pub fn remove(owner: &Owner) -> Result<(), InstallError> {
    files::check_root(owner.home())?;
    user::check(USER)?;
    if let Some(sid) = user::sid(USER)? {
        user::delete_profile(&sid)?;
        user::delete(USER)?;
    }
    user::unhide(USER)?;
    record::delete(owner)
}

/// 加密过的密码写成 base64。
#[cfg(windows)]
fn encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 现在，UTC，精确到毫秒。
#[cfg(windows)]
fn now() -> Result<miyu_kernel::time::Timestamp, InstallError> {
    let failed = |detail: &dyn fmt::Display| InstallError::failed("write record", detail);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| failed(&error))?;
    i64::try_from(since.as_millis())
        .ok()
        .and_then(miyu_kernel::time::Timestamp::from_unix_millis)
        .ok_or_else(|| failed(&"the clock is out of range"))
}
