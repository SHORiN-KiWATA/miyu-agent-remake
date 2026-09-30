//! 回收处的测试：挪进去的整个目录一个字节不变、多一个 `deleted_at`，原处没了；会话不在的报错；清的时候满了时限的删、
//! 正好满的也删、没满的留、时钟往回拨的留、读不出删的时刻的留并报出来、别的名字不看、没有回收处什么都不做。时钟用测试的。

use std::time::Duration;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

/// 留 7 天。
const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

fn alice() -> AccountId {
    AccountId::parse("alice").unwrap()
}

/// 临时目录下的 `data`，当数据根，建好了骨架。
fn root_in(scratch: &Scratch) -> DataRoot {
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .unwrap();
    root.prepare().unwrap();
    root
}

/// 第 `n` 个会话编号。
fn session(n: u32) -> SessionId {
    SessionId::parse(&format!("01900000-0000-7000-8000-{n:012}")).unwrap()
}

/// 这一刻：2026-09-30 12:00 加 `millis` 毫秒。
fn at(millis: i64) -> Timestamp {
    Timestamp::from_unix_millis(
        Timestamp::parse("2026-09-30T12:00:00.000Z")
            .unwrap()
            .unix_millis()
            + millis,
    )
    .unwrap()
}

/// 在原处造一个会话目录：一段日志、一份后台命令的输出。
fn made(root: &DataRoot, session: &SessionId) -> std::path::PathBuf {
    let dir = root.session_dir(&alice(), session);
    fs::create_dir_all(dir.join("jobs")).unwrap();
    fs::write(dir.join("000000000001.jsonl"), b"{\"seq\":1}\n").unwrap();
    fs::write(dir.join("jobs").join("j1.out"), b"ok\n").unwrap();
    dir
}

/// 回收处里放一个会话，删的时刻写成 `text`。
fn trashed(root: &DataRoot, session: &SessionId, text: &str) -> std::path::PathBuf {
    let dir = root.trashed_sessions(&alice()).join(session.as_str());
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(DELETED_AT), text).unwrap();
    dir
}

#[test]
fn a_discarded_session_moves_whole_and_says_when() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let (gone, kept) = (session(1), session(2));
    let from = made(&root, &gone);
    made(&root, &kept);
    discard(&root, &alice(), &gone, at(0)).unwrap();
    assert!(!from.exists(), "原处没了");
    let to = root.trashed_sessions(&alice()).join(gone.as_str());
    assert_eq!(
        fs::read(to.join("000000000001.jsonl")).unwrap(),
        b"{\"seq\":1}\n"
    );
    assert_eq!(fs::read(to.join("jobs").join("j1.out")).unwrap(), b"ok\n");
    assert_eq!(
        fs::read_to_string(to.join(DELETED_AT)).unwrap(),
        "2026-09-30T12:00:00.000Z\n"
    );
    assert_eq!(root.sessions(&alice()).unwrap(), [kept], "列会话只剩另一个");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for dir in [
            root.trashed_sessions(&alice()),
            root.account_dir(&alice()).join("trash"),
        ] {
            let mode = fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", dir.display());
        }
    }
}

#[test]
fn a_missing_session_is_an_error_and_nothing_is_written() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let error = discard(&root, &alice(), &session(1), at(0)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(!root.trashed_sessions(&alice()).exists());
}

#[test]
fn a_purge_removes_what_has_been_there_long_enough() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let week = i64::try_from(WEEK.as_millis()).unwrap();
    let old = trashed(&root, &session(1), "2026-09-22T12:00:00.000Z\n");
    let exact = trashed(&root, &session(2), &format!("{}\n", at(-week)));
    let young = trashed(&root, &session(3), &format!("{}\n", at(1 - week)));
    let future = trashed(&root, &session(4), "2026-10-30T12:00:00.000Z\n");
    let unreadable = trashed(&root, &session(5), "last tuesday\n");
    let missing = root.trashed_sessions(&alice()).join(session(6).as_str());
    fs::create_dir_all(&missing).unwrap();
    let stranger = root.trashed_sessions(&alice()).join("notes");
    fs::create_dir_all(&stranger).unwrap();
    fs::write(stranger.join(DELETED_AT), "2020-01-01T00:00:00.000Z\n").unwrap();

    let purged = purge(&root, &alice(), at(0), WEEK).unwrap();
    assert_eq!(purged.removed, 2);
    assert!(!old.exists(), "满了七天");
    assert!(!exact.exists(), "正好满七天");
    assert!(young.exists(), "差一毫秒");
    assert!(future.exists(), "删的时刻比现在还晚：时钟往回拨过");
    assert!(
        unreadable.exists() && missing.exists(),
        "说不清删了多久的不猜"
    );
    assert!(stranger.exists(), "不是这里放的不看");
    let mut failed: Vec<SessionId> = purged
        .failed
        .into_iter()
        .map(|(session, _)| session)
        .collect();
    failed.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(failed, [session(5), session(6)]);
}

#[test]
fn nothing_to_purge_without_a_trash() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let purged = purge(&root, &alice(), at(0), WEEK).unwrap();
    assert_eq!((purged.removed, purged.failed.len()), (0, 0));
}
