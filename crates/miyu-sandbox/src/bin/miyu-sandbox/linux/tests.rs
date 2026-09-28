use super::*;
use miyu_sandbox::Network;

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn spec(read: &[&str], write: &[&str], readonly: &[&str], hidden: &[&str]) -> Spec {
    Spec {
        read: paths(read),
        write: paths(write),
        readonly: paths(readonly),
        hidden: paths(hidden),
        network: Network::Off,
    }
}

#[test]
fn inside_goes_by_whole_path_segments() {
    let allowed = paths(&["/a/b"]);
    assert!(inside(Path::new("/a/b"), &allowed), "本身");
    assert!(inside(Path::new("/a/b/c/d"), &allowed), "下面");
    assert!(
        !inside(Path::new("/a/bc"), &allowed),
        "旁边的名字只是开头一样"
    );
    assert!(!inside(Path::new("/a"), &allowed), "上一级");
    assert!(!inside(Path::new("/a/b"), &[]), "什么都没放行");
}

#[test]
fn carve_outs_inside_allowed_paths_are_refused() {
    let readonly = check(&spec(&[], &["/w"], &["/w/.git/hooks"], &[])).expect_err("挖不掉");
    assert_eq!(
        readonly,
        "cannot keep /w/.git/hooks read-only inside a writable path"
    );
    let in_write = check(&spec(&[], &["/w"], &[], &["/w/.miyu"])).expect_err("挖不掉");
    assert_eq!(in_write, "cannot hide /w/.miyu inside an allowed path");
    let in_read = check(&spec(&["/r"], &[], &[], &["/r/secret"])).expect_err("挖不掉");
    assert_eq!(in_read, "cannot hide /r/secret inside an allowed path");
    let itself = check(&spec(&[], &["/w"], &["/w"], &[])).expect_err("本身也算");
    assert_eq!(itself, "cannot keep /w read-only inside a writable path");
}

#[test]
fn carve_outs_outside_or_around_allowed_paths_are_kept_without_digging() {
    // 只读的在只读放行的里面：本来就只能读。
    assert_eq!(check(&spec(&["/r"], &[], &["/r/x"], &[])), Ok(()));
    // 藏起来的在放行的范围外，或者把放行的包在里面（工作区在数据根里）：白名单本来就碰不到别的。
    assert_eq!(
        check(&spec(&["/usr"], &["/w"], &[], &["/home/me/.miyu"])),
        Ok(())
    );
    assert_eq!(
        check(&spec(
            &[],
            &["/home/me/.miyu/home/admin/workspace"],
            &[],
            &["/home/me/.miyu"]
        )),
        Ok(())
    );
    assert_eq!(check(&spec(&[], &["/w"], &[], &[])), Ok(()));
}
