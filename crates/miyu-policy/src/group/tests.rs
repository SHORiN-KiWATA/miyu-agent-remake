//! 群会话（施工 O-13 中）：格式说明接在人设后面、记下 `group`，字节读得回来；没有的不写这一格；交给组装器的时区和空的那一句；
//! 时区坏了的快照造不出策略。

use miyu_kernel::time::UtcOffset;

use super::*;
use crate::test_support::engineer;

const NOTE: &str = "<group-chat-format>\nOne record per line.\n</group-chat-format>\n";

fn tokyo() -> GroupChat {
    GroupChat {
        offset: 540,
        no_text: "[no text content]\n".to_string(),
    }
}

#[test]
fn the_note_follows_the_persona_and_the_group_is_kept() {
    let plain = engineer();
    let group = engineer().with_group(NOTE, tokyo());
    assert_eq!(
        group.system,
        format!("{}\n\n{}", plain.system, NOTE.trim_end())
    );
    assert_eq!(group.group, Some(tokyo()));
    assert_eq!(Snapshot::from_bytes(&group.to_bytes()).unwrap(), group);
}

#[test]
fn a_snapshot_without_a_group_writes_no_group() {
    let bytes = String::from_utf8(engineer().to_bytes()).unwrap();
    assert!(!bytes.contains("\"group\":"), "{bytes}");
}

#[test]
fn the_assembler_gets_the_pinned_offset_and_the_bare_line() {
    assert_eq!(
        tokyo().texts().unwrap(),
        miyu_assemble::GroupChat {
            offset: UtcOffset::from_minutes(540).unwrap(),
            no_text: "[no text content]".to_string(),
        }
    );
}

#[test]
fn a_time_zone_out_of_range_builds_no_policy() {
    let broken = GroupChat {
        offset: 15 * 60,
        ..tokyo()
    };
    let snapshot = engineer().with_group(NOTE, broken);
    let Err(error) = snapshot.policy() else {
        panic!("时区坏了的快照造不出策略");
    };
    assert_eq!(error, BuildError::Offset(900));
    assert_eq!(
        error.to_string(),
        "group chat time zone out of range: 900 minutes"
    );
    assert!(engineer().with_group(NOTE, tokyo()).policy().is_ok());
}
