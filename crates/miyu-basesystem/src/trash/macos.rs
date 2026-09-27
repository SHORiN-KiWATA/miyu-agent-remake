//! macOS 的回收站（施工 4-6 下）：系统的 `NSFileManager` 的 `trashItemAtURL:resultingItemURL:error:`，它交回东西在
//! 回收站里的新位置（名字可能被系统改过，所以要记它交回的）。`objc2-foundation` 里这几个接口都是安全的。收不了的盘
//! 它自己报错（`NSFeatureUnsupportedError`），不会永久删掉：这一种当收不了，别的错照系统的原话说。

use std::io;
use std::path::Path;

use objc2::rc::autoreleasepool;
use objc2_foundation::{NSFeatureUnsupportedError, NSFileManager, NSString, NSURL};

use super::Refused;

/// 把 `real` 放进回收站，交回它在回收站里的位置。
pub(super) fn put(real: &Path, _home: Option<&Path>) -> Result<String, Refused> {
    let Some(text) = real.to_str() else {
        return Err(Refused::Failed(io::Error::from(
            io::ErrorKind::InvalidInput,
        )));
    };
    autoreleasepool(|_| {
        let manager = NSFileManager::defaultManager();
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        let mut resulting = None;
        match manager.trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting)) {
            Ok(()) => resulting
                .and_then(|url| url.path())
                .map(|path| path.to_string())
                .ok_or(Refused::Lost),
            Err(error) if error.code() == NSFeatureUnsupportedError => Err(Refused::Unavailable),
            Err(error) => Err(Refused::Failed(io::Error::other(
                error.localizedDescription().to_string(),
            ))),
        }
    })
}
