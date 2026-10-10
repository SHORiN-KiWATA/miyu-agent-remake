use super::*;
use crate::test_support::Scratch;

/// 一个空的临时目录。
fn temp() -> Scratch {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path()).expect("建得了临时目录");
    scratch
}

/// 以前的写法挪成新的：清单进了同名文件夹，原来那个文件夹里的文件照旧在；只有清单、没有文件夹的建一个。
#[test]
fn old_manifests_move_into_their_folders() {
    let home = temp();
    let dir = home.path();
    fs::write(dir.join("relay.toml"), "[package]\n").unwrap();
    fs::create_dir_all(dir.join("relay/page")).unwrap();
    fs::write(dir.join("relay/page/index.html"), "<p>").unwrap();
    fs::write(dir.join("bare.toml"), "[package]\n").unwrap();
    fs::write(dir.join("mermaid.removed"), "").unwrap();
    let moved = old_layout(dir);
    assert_eq!(
        moved
            .iter()
            .map(|m| (m.id.as_str(), m.result.is_ok()))
            .collect::<Vec<_>>(),
        [("bare", true), ("relay", true)]
    );
    assert!(dir.join("relay/package.toml").is_file());
    assert!(
        dir.join("relay/page/index.html").is_file(),
        "原来的文件照旧在"
    );
    assert!(dir.join("bare/package.toml").is_file());
    assert!(!dir.join("relay.toml").exists());
    assert!(dir.join("mermaid.removed").is_file(), "卸掉的那一笔不动");
    assert!(old_layout(dir).is_empty(), "挪过的再来一次什么都不做");
}

/// 文件夹里已经有一份清单的不覆盖，两份都留着；编号不合写法的不动。
#[test]
fn an_existing_manifest_is_not_overwritten() {
    let home = temp();
    let dir = home.path();
    fs::create_dir_all(dir.join("relay")).unwrap();
    fs::write(dir.join("relay/package.toml"), "new").unwrap();
    fs::write(dir.join("relay.toml"), "old").unwrap();
    fs::write(dir.join("Bad.toml"), "x").unwrap();
    let moved = old_layout(dir);
    assert_eq!(moved.len(), 1);
    assert!(matches!(moved[0].result, Err(Unmoved::Both)));
    assert_eq!(
        fs::read_to_string(dir.join("relay/package.toml")).unwrap(),
        "new"
    );
    assert!(dir.join("relay.toml").is_file());
    assert!(dir.join("Bad.toml").is_file());
}
