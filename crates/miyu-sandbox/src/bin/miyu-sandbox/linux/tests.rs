use super::*;

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn spec(write: &[&str], hidden: &[&str]) -> Spec {
    Spec {
        write: paths(write),
        hidden: paths(hidden),
        read: None,
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
fn hidden_inside_a_writable_path_is_refused() {
    let error = check(&spec(&["/w"], &["/w/.miyu"])).expect_err("挖不掉");
    assert_eq!(error, "cannot hide /w/.miyu inside a writable path");
    let itself = check(&spec(&["/w"], &["/w"])).expect_err("本身也算");
    assert_eq!(itself, "cannot hide /w inside a writable path");
}

#[test]
fn writable_inside_hidden_and_hidden_beside_writable_are_fine() {
    // 工作区在数据根里：能写的有自己的一条规则。
    assert_eq!(
        check(&spec(
            &["/home/me/.miyu/home/admin/workspace"],
            &["/home/me/.miyu"]
        )),
        Ok(())
    );
    assert_eq!(check(&spec(&["/w"], &["/home/me/.miyu"])), Ok(()));
    assert_eq!(check(&spec(&[], &["/home/me/.miyu"])), Ok(()));
}
