//! 造不成、载入不了的几种（施工 3-8 七补从 `open.rs` 挪出来，那边放不下了）。

use std::fmt;
use std::io;

use miyu_kernel::session::LoadError as Broken;
use miyu_policy::{BuildError, SnapshotError};
use miyu_store::blob::BlobError;
use miyu_store::log::OpenError;
use miyu_store::resources::SourceError;

/// 造不成。
#[derive(Debug)]
pub enum CreateError {
    /// 人格读不出来：编号不合写法，或者哪一份文件读不了。
    Persona(SourceError),
    /// 随核心附带的字造不出策略：安装坏了。
    Policy(BuildError),
    /// 存不下快照、建不了会话目录和日志。
    Disk(io::Error),
    /// 造会话那一条没落盘，会话就停了。
    Stopped,
}

/// 载入不了。
#[derive(Debug)]
pub enum LoadError {
    /// 日志打不开：没有这个会话，或者日志坏了。
    Log(OpenError),
    /// 日志里没有造会话那一条。
    NotCreated,
    /// 策略快照取不出来。
    Blob(BlobError),
    /// 策略快照读不懂。
    Snapshot(SnapshotError),
    /// 快照造不出策略。
    Policy(BuildError),
    /// 内核载入不了：日志过不了账本。
    Kernel(Broken),
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateError::Persona(error) => write!(f, "persona not readable: {error}"),
            CreateError::Policy(error) => write!(f, "policy not built: {error}"),
            CreateError::Disk(error) => write!(f, "session not created on disk: {error}"),
            CreateError::Stopped => write!(f, "session.created not stored; the session stopped"),
        }
    }
}

impl std::error::Error for CreateError {}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Log(error) => write!(f, "session log not opened: {error}"),
            LoadError::NotCreated => write!(f, "the session log has no session.created"),
            LoadError::Blob(error) => write!(f, "policy snapshot not fetched: {error}"),
            LoadError::Snapshot(error) => write!(f, "policy snapshot not understood: {error}"),
            LoadError::Policy(error) => write!(f, "policy not built from the snapshot: {error}"),
            LoadError::Kernel(error) => write!(f, "not loaded: {error}"),
        }
    }
}

impl std::error::Error for LoadError {}
