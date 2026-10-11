use super::*;
use crate::test_support::Scratch;

fn temp() -> Scratch {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path()).expect("建得了");
    scratch
}

#[test]
fn a_package_is_scanned_written_and_read_back() {
    let scratch = temp();
    let folder = scratch.path().join("pkg");
    fs::create_dir_all(folder.join("page/a b")).unwrap();
    fs::write(folder.join("package.toml"), "[package]\n").unwrap();
    fs::write(folder.join("page/a b/index.html"), "<p>").unwrap();
    let files = scan(&folder).expect("扫得了");
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["package.toml", "page/a b/index.html"],
        "照路径排、用 / 分开"
    );
    assert_eq!(files[1].size, 3);
    let root = scratch.path().join(".local");
    let desc = Desc {
        id: "pkg".to_string(),
        version: Some("1.0".to_string()),
        installed: "2026-10-11T00:00:00.000Z".to_string(),
        source: "/home/me/pkg".to_string(),
        size: size(&files),
    };
    write(&root, &desc, &files).expect("写得进");
    assert_eq!(read(&root, "pkg"), Some((desc.clone(), files.clone())));
    assert_eq!(ids(&root), ["pkg"]);
    assert!(!root.join(".pkg.new").exists(), "暂存换上了");
    write(&root, &desc, &files[..1]).expect("再写一次换掉");
    assert_eq!(read(&root, "pkg").map(|(_, files)| files.len()), Some(1));
    remove(&root, "pkg").expect("删得掉");
    remove(&root, "pkg").expect("没有的不算错");
    assert_eq!(read(&root, "pkg"), None);
}

#[test]
fn the_hash_is_the_sha256_of_the_content() {
    let scratch = temp();
    let folder = scratch.path().join("pkg");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("x"), "abc").unwrap();
    let files = scan(&folder).expect("扫得了");
    assert_eq!(
        files[0].sha256,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn a_broken_files_list_reads_as_nothing() {
    let scratch = temp();
    let root = scratch.path().join(".local");
    fs::create_dir_all(root.join("pkg")).unwrap();
    fs::write(
        root.join("pkg/desc"),
        r#"{"id":"pkg","installed":"x","size":0}"#,
    )
    .unwrap();
    fs::write(root.join("pkg/files"), "not a line\n").unwrap();
    assert_eq!(read(&root, "pkg"), None);
}
