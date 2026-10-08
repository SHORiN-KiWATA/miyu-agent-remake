//! 造会话、载入会话的报错说的是英文（施工 4-9 再补四中）：协议端点把它们原样写进运行日志（`create failed`、
//! `load failed`），运行日志一律英文（`28-运行日志.md` LG1）。

use std::io;
use std::path::PathBuf;

use miyu_kernel::id::ContentHash;
use miyu_kernel::session::LoadError as Broken;
use miyu_policy::{BuildError, Snapshot};
use miyu_store::blob::BlobError;
use miyu_store::log::OpenError;
use miyu_store::resources::SourceError;

use super::{CreateError, LoadError};
use crate::handle::Stopped;

fn duplicate() -> BuildError {
    BuildError::DuplicateTool("read".to_string())
}

#[test]
fn creating_says_it_in_english() {
    assert_eq!(
        CreateError::Persona(SourceError::Read {
            path: std::path::PathBuf::from("core/facts/session.txt"),
            error: io::Error::other("gone"),
        })
        .to_string(),
        "persona not readable: cannot read core/facts/session.txt: gone"
    );
    assert_eq!(
        CreateError::Policy(duplicate()).to_string(),
        "policy not built: two tools named \"read\""
    );
    assert_eq!(
        CreateError::Disk(io::Error::other("no space left")).to_string(),
        "session not created on disk: no space left"
    );
    assert_eq!(
        CreateError::Stopped.to_string(),
        "session.created not stored; the session stopped"
    );
}

#[test]
fn loading_says_it_in_english() {
    let missing = PathBuf::from("/data/sessions/x");
    assert_eq!(
        LoadError::Log(OpenError::Missing(missing.clone())).to_string(),
        format!(
            "session log not opened: no session log in {}",
            missing.display()
        )
    );
    assert_eq!(
        LoadError::NotCreated.to_string(),
        "the session log has no session.created"
    );
    let hash = ContentHash::of(b"");
    assert_eq!(
        LoadError::Blob(BlobError::Missing(hash.clone())).to_string(),
        format!("policy snapshot not fetched: no blob {hash}")
    );
    let Err(unreadable) = Snapshot::from_bytes(b"x") else {
        panic!("该读不回来");
    };
    let said = LoadError::Snapshot(unreadable).to_string();
    assert!(
        said.starts_with("policy snapshot not understood: policy snapshot not readable: "),
        "{said}"
    );
    assert!(said.is_ascii(), "{said}");
    assert_eq!(
        LoadError::Policy(duplicate()).to_string(),
        "policy not built from the snapshot: two tools named \"read\""
    );
    assert_eq!(
        LoadError::Kernel(Broken::Empty).to_string(),
        "not loaded: the log has no events"
    );
    assert_eq!(Stopped.to_string(), "the session stopped");
}
