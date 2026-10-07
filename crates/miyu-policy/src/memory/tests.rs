//! 记忆的范围记进快照（施工 R-3 下）：写进去、读回来一字不差；以前造的没有，照 `persona`；不认识的照 `off`。

use super::*;
use crate::Snapshot;
use crate::test_support::engineer as snapshot;

#[test]
fn the_scope_goes_into_the_snapshot_and_comes_back() {
    for scope in [MemoryScope::Persona, MemoryScope::Session, MemoryScope::Off] {
        let made = snapshot().with_memory(scope);
        let back = Snapshot::from_bytes(&made.to_bytes()).unwrap();
        assert_eq!(back.memory_scope(), scope);
        assert_eq!(back.to_bytes(), made.to_bytes());
        assert!(
            String::from_utf8(made.to_bytes())
                .unwrap()
                .ends_with(&format!(",\"memory\":\"{scope}\"}}"))
        );
    }
}

#[test]
fn an_old_snapshot_is_persona_and_an_unknown_scope_is_off() {
    let old = snapshot();
    assert!(
        !String::from_utf8(old.to_bytes())
            .unwrap()
            .contains("\"memory\""),
        "没有的不写，旧快照的字节不变"
    );
    assert_eq!(
        Snapshot::from_bytes(&old.to_bytes())
            .unwrap()
            .memory_scope(),
        MemoryScope::Persona
    );
    let mut odd = old;
    odd.memory = Some("everywhere".to_string());
    assert_eq!(odd.memory_scope(), MemoryScope::Off, "认不出的宁可不记");
}

#[test]
fn the_three_scopes_are_written_as_words() {
    for (scope, text) in [
        (MemoryScope::Persona, "persona"),
        (MemoryScope::Session, "session"),
        (MemoryScope::Off, "off"),
    ] {
        assert_eq!(scope.as_str(), text);
        assert_eq!(MemoryScope::parse(text), Some(scope));
    }
    assert_eq!(MemoryScope::parse("Persona"), None);
}
