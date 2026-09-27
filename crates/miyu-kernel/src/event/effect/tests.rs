//! 效果的测试（施工 4-6 上）：三种认识的读写一字不差；一行都没显示的不写 `lines`，新建的 `before` 写成
//! `null`；不认识的种类整块原样留着；缺了 `kind`、认识的种类缺了字段，报错。

use super::*;

/// 一个字母 `a` 的内容哈希。
const A: &str = "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";
/// 空内容的哈希。
const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn hash(text: &str) -> ContentHash {
    serde_json::from_str(&format!("\"{text}\"")).unwrap()
}

/// 读进来，再写出去：一字不差。交回读到的。
fn round_trip(json: &str) -> Effect {
    let effect: Effect = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&effect).unwrap(), json);
    effect
}

#[test]
fn the_three_kinds_round_trip() {
    let read = format!(
        r#"{{"kind":"file.read","path":"/home/me/src/lib.rs","lines":[1,37],"hash":"{A}"}}"#
    );
    assert_eq!(
        round_trip(&read),
        Effect::FileRead(FileRead {
            path: "/home/me/src/lib.rs".to_string(),
            lines: Some([1, 37]),
            hash: hash(A),
        })
    );
    let changed = format!(
        r#"{{"kind":"file.changed","path":"/home/me/src/lib.rs","before":"{A}","after":"{EMPTY}"}}"#
    );
    assert_eq!(
        round_trip(&changed),
        Effect::FileChanged(FileChanged {
            path: "/home/me/src/lib.rs".to_string(),
            before: Some(hash(A)),
            after: hash(EMPTY),
        })
    );
    let trashed = r#"{"kind":"file.trashed","path":"/home/me/notes.md","trash":"/home/me/.local/share/Trash/files/notes.md"}"#;
    assert_eq!(
        round_trip(trashed),
        Effect::FileTrashed(FileTrashed {
            path: "/home/me/notes.md".to_string(),
            trash: "/home/me/.local/share/Trash/files/notes.md".to_string(),
        })
    );
}

#[test]
fn nothing_shown_has_no_lines_and_a_new_file_has_a_null_before() {
    let empty = Effect::FileRead(FileRead {
        path: "/e".to_string(),
        lines: None,
        hash: hash(EMPTY),
    });
    assert_eq!(
        serde_json::to_string(&empty).unwrap(),
        format!(r#"{{"kind":"file.read","path":"/e","hash":"{EMPTY}"}}"#)
    );
    let created = Effect::FileChanged(FileChanged {
        path: "/n".to_string(),
        before: None,
        after: hash(A),
    });
    assert_eq!(
        serde_json::to_string(&created).unwrap(),
        format!(r#"{{"kind":"file.changed","path":"/n","before":null,"after":"{A}"}}"#)
    );
    // 没写 `before` 的，当新建读。
    let without: Effect = serde_json::from_str(&format!(
        r#"{{"kind":"file.changed","path":"/n","after":"{A}"}}"#
    ))
    .unwrap();
    assert_eq!(without, created);
}

#[test]
fn an_unknown_kind_is_kept_as_it_is() {
    let json = r#"{"kind":"diagram.drawn", "file" : "a.svg","scale":1.50}"#;
    let effect = round_trip(json);
    assert!(matches!(effect, Effect::Unknown(_)), "{effect:?}");
}

#[test]
fn broken_effects_are_errors() {
    for json in [
        r#"{"path":"/a"}"#,
        r#"{"kind":"file.read","path":"/a"}"#,
        r#"{"kind":"file.changed","path":"/a","before":null}"#,
        r#"{"kind":"file.read","path":"/a","hash":"md5:00"}"#,
        r#"{"kind":"file.trashed","path":"/a"}"#,
    ] {
        assert!(serde_json::from_str::<Effect>(json).is_err(), "{json}");
    }
}
