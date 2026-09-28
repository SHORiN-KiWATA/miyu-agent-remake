//! 装到哪了那份记录（`docs/blueprint/sandbox/windows.md`「装到哪了那份记录」）：本人数据根里的
//! `state/sandbox/windows.json`，一行 JSON，只给本人和 SYSTEM 读。卸载、`miyu doctor` 照它找，5-9 照它以沙盒用户的
//! 身份登录。

use miyu_kernel::time::Timestamp;
use serde::{Deserialize, Serialize};

use super::{InstallError, Owner, files};

/// 记录的文件名，在数据根的 `state/sandbox/` 里。
pub(crate) const FILE: &str = "windows.json";

/// 记录的版本：改了写法就加一。读的时候只认它。
pub(crate) const VERSION: u32 = 1;

/// 装到哪了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    /// 这份记录的版本：[`VERSION`]。
    pub(crate) version: u32,
    /// 沙盒用户的登录名。
    pub(crate) username: String,
    /// 沙盒用户的 SID。
    pub(crate) user_sid: String,
    /// 机器范围的 DPAPI 加密过的密码，base64。
    pub(crate) password: String,
    /// 装好的时刻。
    pub(crate) created_at: Timestamp,
}

impl Record {
    /// 这一版的记录。
    pub(crate) fn new(
        username: String,
        user_sid: String,
        password: String,
        created_at: Timestamp,
    ) -> Record {
        Record {
            version: VERSION,
            username,
            user_sid,
            password,
            created_at,
        }
    }

    /// 写成一行 JSON，后面一个换行。
    ///
    /// # Errors
    ///
    /// 写不成 JSON：几格都是字符串、数字和时刻，实际不会出错。
    pub(crate) fn to_line(&self) -> Result<String, serde_json::Error> {
        Ok(format!("{}\n", serde_json::to_string(self)?))
    }

    /// 读回来：认得的这一版才算。
    ///
    /// # Errors
    ///
    /// 不是 JSON、少了格、多了认不得的格、类型不对、版本不是 [`VERSION`]：交回原因。
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "5-9 以沙盒用户的身份登录时读它；这一步只有测试读")
    )]
    pub(crate) fn from_json(text: &str) -> Result<Record, String> {
        let record: Record = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if record.version != VERSION {
            return Err(format!("version {}", record.version));
        }
        Ok(record)
    }
}

/// 把记录写进本人的数据根：认标记，不经链接，只给本人和 SYSTEM，写好了再改名盖掉旧的（`files`）。
///
/// # Errors
///
/// 数据根没有标记；写不了（`write record`）。
pub(crate) fn write(owner: &Owner, record: &Record) -> Result<(), InstallError> {
    files::check_root(owner.home())?;
    let failed = |error: &dyn std::fmt::Display| InstallError::failed("write record", error);
    let line = record.to_line().map_err(|error| failed(&error))?;
    files::replace_private(owner.home(), FILE, line.as_bytes(), owner.sid())
        .map_err(|error| failed(&error))
}

/// 删掉本人数据根里的记录，本来没有不算错。
///
/// # Errors
///
/// 数据根没有标记；删不了（`delete record`）。
pub(crate) fn delete(owner: &Owner) -> Result<(), InstallError> {
    files::check_root(owner.home())?;
    let path = owner.home().join("state").join("sandbox").join(FILE);
    files::remove_if_there(&path).map_err(|error| InstallError::failed("delete record", error))
}

#[cfg(test)]
mod tests;
