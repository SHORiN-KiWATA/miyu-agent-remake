//! 往本人的数据根里写只有本人和 SYSTEM 读得到的文件（`docs/blueprint/sandbox/windows.md`「怎么走」第 4 条第 7 步）：
//! 认标记、不经链接、只许新建。

use super::*;
use crate::install::testkit::Dir;

#[test]
fn a_data_root_is_known_by_its_marker() {
    let root = Dir::root();
    check_root(root.path()).expect("有标记");
    let plain = Dir::new();
    assert_eq!(
        check_root(plain.path()),
        Err(InstallError::NotDataRoot {
            path: plain.path().display().to_string()
        })
    );
    // 标记只看在不在，和建骨架那边一样（`store.md`「怎么走」第 2 条）。
    let odd = Dir::new();
    std::fs::create_dir(odd.path().join(miyu_store::root::MARKER)).expect("建得了");
    check_root(odd.path()).expect("是目录也算");
}

#[test]
fn the_sandbox_directory_is_made_under_state() {
    let root = Dir::root();
    let dir = sandbox_dir(root.path()).expect("建得了");
    assert_eq!(dir, root.path().join("state").join("sandbox"));
    assert!(dir.is_dir());
    // 再来一次也成。
    assert_eq!(sandbox_dir(root.path()).expect("还在"), dir);
}

#[test]
fn a_private_file_is_only_ever_new() {
    let root = Dir::root();
    let dir = sandbox_dir(root.path()).expect("建得了");
    let path = dir.join("one.json");
    write_private(&path, b"first\n", &crate::install::testkit::sid()).expect("写得进");
    assert_eq!(std::fs::read(&path).expect("读得回来"), b"first\n");
    let again = write_private(&path, b"second\n", &crate::install::testkit::sid());
    assert!(again.is_err(), "已经有了的不覆盖");
    assert_eq!(std::fs::read(&path).expect("读得回来"), b"first\n");
}

#[cfg(unix)]
#[test]
fn links_on_the_way_are_not_written_through() {
    use std::os::unix::fs::symlink;
    for link_at in ["state", "state/sandbox"] {
        let root = Dir::root();
        let elsewhere = Dir::new();
        let at = root.path().join(link_at);
        std::fs::create_dir_all(at.parent().expect("有上一级")).expect("建得了");
        symlink(elsewhere.path(), &at).expect("建得了链接");
        let error = sandbox_dir(root.path()).expect_err("不经链接写");
        assert!(error.to_string().contains("link"), "{link_at}: {error}");
        let untouched: Vec<_> = std::fs::read_dir(elsewhere.path())
            .expect("读得了")
            .collect();
        assert!(untouched.is_empty(), "{link_at}：链接那头什么都没多");
    }
}

#[cfg(unix)]
#[test]
fn a_link_where_the_file_goes_is_not_followed() {
    use std::os::unix::fs::symlink;
    let root = Dir::root();
    let dir = sandbox_dir(root.path()).expect("建得了");
    let elsewhere = Dir::new();
    let target = elsewhere.path().join("victim");
    let path = dir.join("one.json");
    symlink(&target, &path).expect("建得了链接");
    assert!(write_private(&path, b"x\n", &crate::install::testkit::sid()).is_err());
    assert!(!target.exists(), "没顺着链接写到别处");
}

#[cfg(unix)]
#[test]
fn on_unix_the_file_is_for_the_owner_alone() {
    use std::os::unix::fs::PermissionsExt;
    let root = Dir::root();
    let dir = sandbox_dir(root.path()).expect("建得了");
    let path = dir.join("one.json");
    write_private(&path, b"x\n", &crate::install::testkit::sid()).expect("写得进");
    let mode = std::fs::metadata(&path).expect("在").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(windows)]
#[test]
fn on_windows_the_file_is_born_for_the_owner_and_system_only() {
    let root = Dir::root();
    let dir = sandbox_dir(root.path()).expect("建得了");
    let path = dir.join("one.json");
    let sid = crate::install::testkit::sid();
    write_private(&path, b"x\n", &sid).expect("写得进");
    let sddl = crate::install::winsys::dacl_of(&path).expect("读得出访问控制");
    // 比的是访问控制本身：跑测试的是内置管理员时，系统把它的 SID 写成 `LA`（CI 上就是）。
    let expected =
        crate::install::winsys::canonical_dacl(&format!("D:P(A;;FA;;;{sid})(A;;FA;;;SY)"))
            .expect("写法对");
    assert_eq!(sddl, expected);
    assert!(sddl.starts_with("D:P("), "受保护、不继承上一层：{sddl}");
}

#[cfg(windows)]
#[test]
fn on_windows_a_linked_directory_is_not_written_through() {
    // 建目录链接要开发者模式或管理员：CI 上是管理员；建不了的机器上这条跳过。
    let root = Dir::root();
    let elsewhere = Dir::new();
    let state = root.path().join("state");
    if let Err(error) = std::os::windows::fs::symlink_dir(elsewhere.path(), &state) {
        eprintln!("建不了目录链接，跳过：{error}");
        return;
    }
    let error = sandbox_dir(root.path()).expect_err("不经链接写");
    assert!(error.to_string().contains("link"), "{error}");
    let untouched: Vec<_> = std::fs::read_dir(elsewhere.path())
        .expect("读得了")
        .collect();
    assert!(untouched.is_empty(), "链接那头什么都没多");
}

#[cfg(windows)]
#[test]
fn on_windows_a_junction_on_the_way_is_not_written_through() {
    // 目录联接不要任何特权，谁都建得了：真要挡的就是它。标准库把它当链接（名字代理类的重解析点）。
    let root = Dir::root();
    let elsewhere = Dir::new();
    let state = root.path().join("state");
    let made = std::process::Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(&state)
        .arg(elsewhere.path())
        .output()
        .expect("起得了 cmd");
    assert!(made.status.success(), "建得了联接：{made:?}");
    let error = sandbox_dir(root.path()).expect_err("不经联接写");
    assert!(error.to_string().contains("link"), "{error}");
    let untouched: Vec<_> = std::fs::read_dir(elsewhere.path())
        .expect("读得了")
        .collect();
    assert!(untouched.is_empty(), "联接那头什么都没多");
}
