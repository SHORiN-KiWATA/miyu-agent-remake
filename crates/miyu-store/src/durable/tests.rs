//! 建目录的测试：一层一层建出来，Unix 上都是 0700；建两次不出错；该是目录的地方是个文件，报错。
//! 同步测不出来：测试里没法断电。

use super::*;
use crate::test_support::Scratch;

#[test]
fn missing_levels_are_made_one_by_one() {
    let scratch = Scratch::new();
    let deep = scratch.path().join("a").join("b").join("c");
    create_dir(&deep).unwrap();
    assert!(deep.is_dir());
    // 建两次也不出错。
    create_dir(&deep).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for level in [
            scratch.path().to_path_buf(),
            scratch.path().join("a"),
            scratch.path().join("a").join("b"),
            deep,
        ] {
            let mode = fs::metadata(&level).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", level.display());
        }
    }
}

#[test]
fn a_file_in_the_way_is_an_error() {
    let scratch = Scratch::new();
    create_dir(scratch.path()).unwrap();
    let file = scratch.path().join("a");
    fs::write(&file, "一个文件").unwrap();
    assert!(create_dir(&file).is_err());
    // 挡在中间一层的也一样。
    assert!(create_dir(&file.join("b")).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "一个文件");
}
