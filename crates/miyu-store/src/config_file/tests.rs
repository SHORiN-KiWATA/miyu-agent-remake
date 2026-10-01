//! 读配置文件：没有的是空的；BOM 去掉、版本照整份字节；太大的不读；不是 UTF-8 的、读不了的报错；顺着链接读。

use std::fs;
use std::io;
use std::path::Path;

use super::*;
use crate::test_support::Scratch;

#[test]
fn a_missing_file_is_an_empty_layer() {
    let temp = Scratch::new();
    assert!(matches!(read(&temp.path().join("config.toml")), Ok(None)));
}

#[test]
fn the_bom_is_dropped_but_counted_in_the_version() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "\u{FEFF}a = 1\r\n").unwrap();
    let read = read(&path).unwrap().unwrap();
    assert_eq!(read.text, "a = 1\r\n", "\\r\\n 照原样");
    assert_eq!(read.version, version("\u{FEFF}a = 1\r\n".as_bytes()));
    assert_ne!(read.version, version(b"a = 1\r\n"), "带 BOM 的字节算版本");
    assert!(read.version.starts_with("sha256:") && read.version.len() == 7 + 64);
    assert_eq!(
        version(b""),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn a_file_over_one_mebibyte_is_not_read() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, vec![b'#'; usize::try_from(LIMIT).unwrap()]).unwrap();
    assert!(read(&path).unwrap().is_some(), "正好 1 MiB 的读");
    fs::write(&path, vec![b'#'; usize::try_from(LIMIT).unwrap() + 1]).unwrap();
    assert!(matches!(read(&path), Err(ReadError::TooBig)));
}

#[test]
fn bytes_that_are_not_utf8_are_refused() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, b"a = \"\xff\"\n").unwrap();
    assert!(matches!(read(&path), Err(ReadError::NotUtf8)));
}

#[test]
fn a_directory_is_unreadable() {
    let temp = Scratch::new();
    let path = temp.path().join("config.toml");
    fs::create_dir_all(&path).unwrap();
    assert!(matches!(read(&path), Err(ReadError::Unreadable(_))));
}

#[cfg(unix)]
#[test]
fn a_link_is_followed_even_outside() {
    let temp = Scratch::new();
    let elsewhere = temp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("real.toml"), "a = 1\n").unwrap();
    let path = temp.path().join("config.toml");
    std::os::unix::fs::symlink(elsewhere.join("real.toml"), &path).unwrap();
    assert_eq!(read(&path).unwrap().unwrap().text, "a = 1\n");
}

// 写（施工 8-3）：顺着链接写、链接不动、绕圈报错、指向没有的新建；临时文件在本体旁边、崩在改名前原文件不变、这一瞬间有人
// 手改了就放弃；权限位留着；Windows 上开着的文件重试；BOM 照样加回。

/// 目录里除了 `keep` 以外还有没有别的文件：临时文件不该留下。
fn leftovers(dir: &Path, keep: &[&str]) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !keep.contains(&name.as_str()))
        .collect()
}

#[test]
fn a_new_file_is_written_and_its_directory_made() {
    let temp = Scratch::new();
    let path = temp.path().join("home").join("admin").join("settings.toml");
    write(&path, b"a = 1\n", None).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"a = 1\n");
    assert_eq!(
        leftovers(path.parent().unwrap(), &["settings.toml"]),
        Vec::<String>::new()
    );
}

#[test]
fn a_file_changed_since_it_was_read_is_not_written_over() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "a = 1\n").unwrap();
    let read = version(b"a = 1\n");
    write(&path, b"a = 2\n", Some(&read)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"a = 2\n");
    assert!(
        matches!(
            write(&path, b"a = 3\n", Some(&read)),
            Err(WriteError::Changed)
        ),
        "上一次读的已经不是现在的了"
    );
    assert!(
        matches!(write(&path, b"a = 3\n", None), Err(WriteError::Changed)),
        "以为没有"
    );
    assert_eq!(fs::read(&path).unwrap(), b"a = 2\n");
    assert_eq!(
        leftovers(temp.path(), &["config.toml"]),
        Vec::<String>::new()
    );
}

#[test]
fn a_hand_edit_just_before_the_rename_wins_and_nothing_is_left_behind() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "a = 1\n").unwrap();
    let mut beside = None;
    let written = write_with(
        &path,
        b"a = 2\n",
        Some(&version(b"a = 1\n")),
        Mode::Keep,
        &mut |temp| {
            beside = Some(temp.to_path_buf());
            fs::write(&path, "a = 9\n")
        },
    );
    assert!(matches!(written, Err(WriteError::Changed)));
    assert_eq!(fs::read(&path).unwrap(), b"a = 9\n", "手改的留着");
    let beside = beside.unwrap();
    assert_eq!(beside.parent(), path.parent(), "临时文件在本体旁边");
    let name = beside.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with(".config.toml.") && name.ends_with(".tmp"),
        "{name}"
    );
    assert_eq!(
        leftovers(temp.path(), &["config.toml"]),
        Vec::<String>::new()
    );
}

#[test]
fn a_crash_before_the_rename_leaves_the_file_as_it_was() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "a = 1\n").unwrap();
    let written = write_with(
        &path,
        b"a = 2\n",
        Some(&version(b"a = 1\n")),
        Mode::Keep,
        &mut |_| Err(io::Error::other("崩了")),
    );
    assert!(matches!(written, Err(WriteError::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), b"a = 1\n");
}

#[test]
fn the_bom_goes_back_on_and_is_remembered() {
    assert_eq!(bytes("a = 1\n", true), "\u{FEFF}a = 1\n".as_bytes());
    assert_eq!(bytes("a = 1\n", false), b"a = 1\n");
    assert!(text("\u{FEFF}a".as_bytes()).unwrap().bom);
    assert!(!text(b"a").unwrap().bom);
}

#[cfg(unix)]
#[test]
fn the_permission_bits_stay() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "a = 1\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    write(&path, b"a = 2\n", Some(&version(b"a = 1\n"))).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[cfg(unix)]
#[test]
fn links_are_followed_and_left_alone() {
    let temp = Scratch::new();
    let dotfiles = temp.path().join("dotfiles");
    let system = temp.path().join("system");
    fs::create_dir_all(&dotfiles).unwrap();
    fs::create_dir_all(&system).unwrap();
    fs::write(dotfiles.join("miyu.toml"), "a = 1\n").unwrap();
    let link = system.join("config.toml");
    std::os::unix::fs::symlink("../dotfiles/miyu.toml", &link).unwrap();
    write(&link, b"a = 2\n", Some(&version(b"a = 1\n"))).unwrap();
    assert_eq!(
        fs::read(dotfiles.join("miyu.toml")).unwrap(),
        b"a = 2\n",
        "写的是本体"
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "链接还是链接"
    );
    assert_eq!(
        fs::read_link(&link).unwrap(),
        Path::new("../dotfiles/miyu.toml")
    );
    assert_eq!(leftovers(&system, &["config.toml"]), Vec::<String>::new());
    assert_eq!(leftovers(&dotfiles, &["miyu.toml"]), Vec::<String>::new());

    let dangling = system.join("settings.toml");
    std::os::unix::fs::symlink(dotfiles.join("new.toml"), &dangling).unwrap();
    write(&dangling, b"b = 1\n", None).unwrap();
    assert_eq!(
        fs::read(dotfiles.join("new.toml")).unwrap(),
        b"b = 1\n",
        "指向没有的在那里新建"
    );

    let one = system.join("one.toml");
    let two = system.join("two.toml");
    std::os::unix::fs::symlink(&two, &one).unwrap();
    std::os::unix::fs::symlink(&one, &two).unwrap();
    assert!(
        matches!(write(&one, b"c = 1\n", None), Err(WriteError::Io(_))),
        "绕圈的报错"
    );
}

#[cfg(windows)]
#[test]
fn a_file_held_open_by_another_program_is_retried() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "a = 1\n").unwrap();
    // 只许别人读、不许删改名：像一个开着文件的编辑器。
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        drop(held);
    });
    write(&path, b"a = 2\n", Some(&version(b"a = 1\n"))).unwrap();
    release.join().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"a = 2\n");
}
