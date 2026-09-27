//! 别的系统：没有做回收站，一律当收不了，不删（施工 4-6 下只做 Linux、macOS、Windows）。

use std::path::Path;

use super::Refused;

/// 收不了。
pub(super) fn put(_real: &Path, _home: Option<&Path>) -> Result<String, Refused> {
    Err(Refused::Unavailable)
}
