//! 人格头像的账（蓝图 `tui.md`「空会话的首页」第 10 条）：头像照版本（核心给的图的字节的哈希前 16 位）存成缓存目录里的
//! 文件，重启以后照样在、不再要；没存过的照 `persona.avatar` 要，一次只要一张，要不到、存不成的这次不再要。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

/// 账。
#[derive(Debug, Default)]
pub struct Avatars {
    /// 存头像的目录（`<缓存目录>/tui/avatars`）；找不到缓存目录的没有，那就一张都不画。
    dir: Option<PathBuf>,
    /// 要过的版本：存成的文件，要不到、存不成的是 `None`。
    files: HashMap<String, Option<PathBuf>>,
    /// 在要的那一张：人格编号、版本。
    asking: Option<(String, String)>,
}

impl Avatars {
    /// 照机器共用的缓存目录。
    pub fn cached() -> Self {
        miyu_store::root::cache_root(&miyu_store::env::Env::current())
            .map(|root| Self::at(root.join("tui").join("avatars")))
            .unwrap_or_default()
    }

    /// 存在 `dir` 里。
    pub fn at(dir: PathBuf) -> Self {
        Self {
            dir: Some(dir),
            ..Self::default()
        }
    }

    /// 这个版本的头像文件：存过的（这次要到的、以前存下的）才有。
    pub fn file(&self, version: &str) -> Option<PathBuf> {
        if let Some(known) = self.files.get(version) {
            return known.clone();
        }
        self.path(version).filter(|p| p.is_file())
    }

    /// 这次要不到、存不成的。
    pub fn refused(&self, version: &str) -> bool {
        matches!(self.files.get(version), Some(None))
    }

    /// 断开了：在要的那张不等了（回应不会来了），连上以后照列表再要。
    pub fn dropped(&mut self) {
        self.asking = None;
    }

    /// 还要不要它：没存过、没要不到过，也不是正在要的。
    fn wants(&self, version: &str) -> bool {
        !self.files.contains_key(version)
            && self.file(version).is_none()
            && self.asking.as_ref().is_none_or(|(_, v)| v != version)
    }

    /// 照要画的几张（人格编号、版本）挑一张去要：没在要别的、有还没存过的才交回它的人格编号。
    pub fn next(&mut self, wanted: &[(&str, &str)]) -> Option<String> {
        if self.asking.is_some() || self.dir.is_none() {
            return None;
        }
        let (persona, version) = wanted.iter().find(|(_, v)| self.wants(v))?;
        self.asking = Some((persona.to_string(), version.to_string()));
        Some(persona.to_string())
    }

    /// `persona.avatar` 回来了（`{"avatar", "media_type", "data"}`；没有头像的 `null`，拒了的 `None`）：照回来的版本
    /// 存成文件。回来的版本和要的时候不一样（中途换过）的照回来的存，要的那个版本记成要不到，下一帧照新的列表再要。
    pub fn answered(&mut self, got: Option<&Value>) {
        let Some((_, asked)) = self.asking.take() else {
            return;
        };
        let saved = got.and_then(|got| {
            let version = got["avatar"].as_str()?;
            let bytes = STANDARD.decode(got["data"].as_str()?).ok()?;
            let path = self.path(version)?;
            std::fs::create_dir_all(path.parent()?).ok()?;
            std::fs::write(&path, bytes).ok()?;
            Some((version.to_string(), path))
        });
        if let Some((version, path)) = &saved {
            self.files.insert(version.clone(), Some(path.clone()));
        }
        if saved.as_ref().is_none_or(|(v, _)| *v != asked) {
            self.files.insert(asked, None);
        }
    }

    /// 一个版本存在哪：名字只留字母数字（版本是十六进制，别的字不会有）。
    fn path(&self, version: &str) -> Option<PathBuf> {
        let name: String = version
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        (!name.is_empty())
            .then(|| self.dir.as_deref().map(|d: &Path| d.join(name)))
            .flatten()
    }
}

pub mod shrink;

#[cfg(test)]
mod tests;
