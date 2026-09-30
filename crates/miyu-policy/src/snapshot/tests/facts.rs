//! 会话编号的模板进快照（施工 1-13 再补，`docs/blueprint/policy.md`「`CoreTexts`」）：出厂的快照带着它，以前造的没有。
//! 切了级别以后的权限那一份也一样（施工 2-7 补）。

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::SessionId;

use super::*;

/// 会话编号的模板（施工 1-13 再补）：出厂的快照带着它，造出的策略写得出那一块；坏了照名字报；以前造的快照没有这一格，
/// 读进来再写出去一字不差，造出的策略没有这一块。
#[test]
fn the_session_template_goes_in_and_older_snapshots_lack_it() {
    let id = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ec8").unwrap();
    let snapshot = engineer();
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let field = r#","session":"<session id=\"{id}\"/>\n""#;
    assert!(text.contains(field), "{text}");
    let fact = snapshot.policy().unwrap().facts.session(&id).unwrap();
    assert_eq!(
        fact.text,
        "<session id=\"01a0f233-cfec-7023-8ed5-2a037a1d5ec8\"/>\n"
    );
    let mut broken = snapshot.clone();
    broken.core.facts.session = Some("<session id=\"{who}\"/>\n".to_string());
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "fact templates",
                ..
            }
        ),
        "{error:?}"
    );
    let older = text.replacen(field, "", 1);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.core.facts.session, None);
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(read.policy().unwrap().facts.session(&id), None);
}

/// 切了级别以后的权限那一份（施工 2-7 补）：出厂的快照带着它，造出的策略写得出那一块；坏了照名字报；以前造的快照没有这
/// 一格，读进来再写出去一字不差，造出的策略没有这一块（切了照旧写平常那一份）。
#[test]
fn the_permission_changed_template_goes_in_and_older_snapshots_lack_it() {
    let full = Permission {
        level: Level::Full,
        read_only: false,
    };
    let snapshot = engineer();
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let field = r#","permission_changed":"<permission level=\"{level}\" previous=\"{previous}\">The user changed the permission level.</permission>\n""#;
    assert!(text.contains(field), "{text}");
    let fact = snapshot
        .policy()
        .unwrap()
        .facts
        .permission_changed(&full, "workspace")
        .unwrap();
    assert_eq!(
        fact.text,
        "<permission level=\"full\" previous=\"workspace\">The user changed the permission level.</permission>\n"
    );
    let mut broken = snapshot.clone();
    broken.core.facts.permission_changed = Some("<permission was=\"{who}\"/>\n".to_string());
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "fact templates",
                ..
            }
        ),
        "{error:?}"
    );
    let older = text.replacen(field, "", 1);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.core.facts.permission_changed, None);
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(
        read.policy()
            .unwrap()
            .facts
            .permission_changed(&full, "workspace"),
        None
    );
}
