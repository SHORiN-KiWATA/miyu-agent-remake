use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::test_support::Scratch;

fn dir_with_mode(scratch: &Scratch, name: &str, mode: u32) -> PathBuf {
    let dir = scratch.path().join(name);
    fs::create_dir(&dir).expect("建得了");
    fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).expect("改得了");
    dir
}

#[test]
fn the_runtime_dir_must_be_absolute_and_private() {
    let scratch = Scratch::new();
    let private = dir_with_mode(&scratch, "private", 0o700);
    let shared = dir_with_mode(&scratch, "shared", 0o755);
    assert_eq!(
        runtime_dir(Some(private.clone().into())),
        Some(private.clone())
    );
    assert_eq!(runtime_dir(Some(shared.into())), None);
    assert_eq!(
        runtime_dir(Some(scratch.path().join("missing").into())),
        None
    );
    // 相对路径：哪怕照当前目录找得到一个只有自己能进的目录，也不算。
    let relative =
        PathBuf::from("../".repeat(64)).join(private.strip_prefix("/").expect("绝对路径"));
    assert!(
        fs::metadata(&relative).is_ok_and(|meta| meta.is_dir()),
        "照当前目录找得到"
    );
    assert_eq!(runtime_dir(Some(relative.into())), None);
    assert_eq!(runtime_dir(Some("".into())), None);
    assert_eq!(runtime_dir(None), None);
}

#[test]
fn only_a_real_directory_of_ones_own_is_private() {
    let scratch = Scratch::new();
    let private_dir = dir_with_mode(&scratch, "private", 0o700);
    assert!(private(&private_dir).expect("看得了"));
    for mode in [0o750, 0o705, 0o710, 0o701] {
        let dir = dir_with_mode(&scratch, &format!("m{mode:o}"), mode);
        assert!(!private(&dir).expect("看得了"), "{mode:o}");
    }
    let link = scratch.path().join("link");
    std::os::unix::fs::symlink(&private_dir, &link).expect("建得了链接");
    assert!(!private(&link).expect("看得了"), "链接到自己的目录也不算");
    let file = scratch.path().join("file");
    fs::write(&file, "").expect("写得进");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("改得了");
    assert!(!private(&file).expect("看得了"), "文件不是目录");
}

#[test]
fn someone_elses_directory_is_not_private() {
    assert!(alone(1000, 0o40700, 1000));
    assert!(!alone(0, 0o40700, 1000), "别人的");
    assert!(!alone(1000, 0o40750, 1000), "组能进");
    assert!(!alone(1000, 0o40701, 1000), "别人能进");
}

#[test]
fn a_new_private_dir_is_0700_and_an_old_one_is_left_as_is() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("new");
    make_private(&dir).expect("建得了");
    let mode = fs::metadata(&dir).expect("在").permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    let shared = dir_with_mode(&scratch, "shared", 0o755);
    make_private(&shared).expect("已经有的不算错");
    let mode = fs::metadata(&shared).expect("在").permissions().mode();
    assert_eq!(mode & 0o777, 0o755, "已经有的不改，交给核对");
    let error =
        make_private(&scratch.path().join("no").join("parent")).expect_err("上一层要已经在");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
