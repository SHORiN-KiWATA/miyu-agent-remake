//! 安全地打开（施工 4-3 上）：普通文件打得开；目录、FIFO、设备、套接字、最后一层的链接都不开，报是什么；
//! FIFO 不卡住。

mod support;

use std::io::Read;

use miyu_fs::{Kind, OpenError, open_file};
use support::Site;

#[test]
fn a_regular_file_opens_and_reads() {
    let site = Site::new();
    let mut file = open_file(&site.real("work/src/a.rs")).expect("打得开");
    let mut text = String::new();
    file.read_to_string(&mut text).expect("读得出");
    assert_eq!(text, "fn a() {}");
}

#[test]
fn a_directory_or_a_missing_file_is_refused() {
    let site = Site::new();
    assert!(matches!(
        open_file(&site.real("work/src")),
        Err(OpenError::NotAFile(Kind::Directory))
    ));
    assert!(matches!(
        open_file(&site.real("work").join("missing.rs")),
        Err(OpenError::NotFound)
    ));
}

#[cfg(unix)]
#[test]
fn a_link_as_the_last_part_is_not_followed() {
    use std::os::unix::fs::symlink;

    // 检查完以后，文件被换成了指向别处的链接：打开时不跟着走。
    let site = Site::new();
    symlink(site.at("home/.ssh/id"), site.at("work/swapped")).expect("造得了链接");
    let result = open_file(&site.real("work").join("swapped"));
    assert!(
        matches!(result, Err(OpenError::NotAFile(Kind::Link))),
        "{result:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_fifo_is_refused_without_waiting_for_a_writer() {
    let site = Site::new();
    let fifo = site.real("work").join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("有 mkfifo");
    assert!(made.success());
    // 在另一个线程里开：卡住的话，五秒后这里就报红，不陪它一直等。
    let (tell, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = open_file(&fifo).map(|_| ());
        tell.send(format!("{result:?}")).expect("测试还在等");
    });
    let result = heard
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("打开 FIFO 卡住了");
    assert_eq!(result, "Err(NotAFile(Fifo))");
}

#[cfg(unix)]
#[test]
fn a_device_and_a_socket_are_refused() {
    assert!(matches!(
        open_file(std::path::Path::new("/dev/null")),
        Err(OpenError::NotAFile(Kind::Device))
    ));
    let site = Site::new();
    let path = site.real("work").join("sock");
    let _listening = std::os::unix::net::UnixListener::bind(&path).expect("绑得上");
    let result = open_file(&path);
    assert!(
        matches!(result, Err(OpenError::NotAFile(Kind::Socket))),
        "{result:?}"
    );
}

#[cfg(windows)]
#[test]
fn a_link_as_the_last_part_is_not_followed_on_windows() {
    let site = Site::new();
    let link = site.real("work").join("swapped");
    if let Err(error) = std::os::windows::fs::symlink_file(site.at("home/.ssh/id"), &link) {
        assert!(
            std::env::var_os("CI").is_none(),
            "CI 上应该造得了符号链接：{error}"
        );
        eprintln!("造不了符号链接（{error}），跳过：Windows 上要管理员权限或开发者模式");
        return;
    }
    let result = open_file(&link);
    assert!(
        matches!(result, Err(OpenError::NotAFile(Kind::Link))),
        "{result:?}"
    );
}
