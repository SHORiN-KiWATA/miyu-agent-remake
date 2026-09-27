//! 核心在套接字上等连接，头连过去（施工 3-8 下）。

#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixListener as StdListener;

use miyu_ipc::{ConnectError, Dirs, OpenError, connect, fingerprint, open};
use miyu_store::env::Platform;
use support::{Home, mode, talk};

#[tokio::test]
async fn a_head_connects_through_run_socket_and_gets_the_token() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    let socket = home.root.run().join("core.sock");
    assert_eq!(opened.listener.path(), socket);
    assert_eq!(home.location(), socket);
    assert_eq!(home.token(), opened.token);
    assert_eq!(opened.token.len(), 64);
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn on_linux_it_listens_under_the_runtime_dir() {
    let home = Home::new();
    let runtime = home.private_dir("runtime");
    let dirs = Dirs {
        platform: Platform::Linux,
        runtime_dir: Some(runtime.clone()),
        ..home.dirs()
    };
    let mut opened = open(&home.root, &dirs).expect("起得来");
    let dir = runtime.join(format!("miyu-{}", fingerprint(&home.root)));
    assert_eq!(opened.listener.path(), dir.join("core.sock"));
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(home.location(), dir.join("core.sock"));
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn a_long_root_listens_under_the_temp_dir() {
    let home = Home::deep();
    let dirs = home.dirs();
    let mut opened = open(&home.root, &dirs).expect("起得来");
    let uid = dirs.uid.expect("Unix 上有用户编号");
    assert_eq!(
        uid,
        fs::metadata(&home.dir).expect("在").uid(),
        "是自己的用户编号"
    );
    let dir = dirs.temp_dir.join(format!("miyu-{uid}"));
    let socket = dir.join(format!("{}.sock", fingerprint(&home.root)));
    assert_eq!(opened.listener.path(), socket);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(home.location(), socket);
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn one_core_per_root_until_it_goes() {
    let home = Home::new();
    let first = open(&home.root, &home.dirs()).expect("起得来");
    let error = open(&home.root, &home.dirs()).expect_err("第二个起不来");
    assert!(matches!(error, OpenError::Running), "{error:?}");
    assert!(first.listener.path().exists(), "第二个没动第一个的套接字");
    let old = first.token.clone();
    drop(first);
    assert!(
        !home.root.run().join("core.sock").exists(),
        "走的时候删了套接字文件"
    );
    let mut second = open(&home.root, &home.dirs()).expect("第一个走了以后起得来");
    assert_ne!(second.token, old, "每次起来换一个令牌");
    talk(&home, &mut second).await;
}

#[tokio::test]
async fn a_stale_socket_is_cleared() {
    let home = Home::new();
    let socket = home.root.run().join("core.sock");
    drop(StdListener::bind(&socket).expect("绑得上"));
    assert!(
        fs::symlink_metadata(&socket)
            .expect("旧套接字还在")
            .file_type()
            .is_socket()
    );
    let mut opened = open(&home.root, &home.dirs()).expect("照样起得来");
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn something_else_in_the_way_is_left_alone() {
    let home = Home::new();
    let socket = home.root.run().join("core.sock");
    fs::write(&socket, "not a socket").expect("写得进");
    let error = open(&home.root, &home.dirs()).expect_err("位置上有文件");
    assert!(
        matches!(&error, OpenError::Occupied(path) if *path == socket),
        "{error:?}"
    );
    assert_eq!(fs::read_to_string(&socket).expect("还在"), "not a socket");
    fs::remove_file(&socket).expect("删得了");

    let listening = StdListener::bind(&socket).expect("绑得上");
    let error = open(&home.root, &home.dirs()).expect_err("位置上有人在听");
    assert!(matches!(error, OpenError::Occupied(_)), "{error:?}");
    listening.set_nonblocking(true).expect("设得了");
    let _probe = listening.accept().expect("试着连的那一下排在它这里");
    assert!(
        fs::symlink_metadata(&socket)
            .expect("还在")
            .file_type()
            .is_socket()
    );
}

#[tokio::test]
async fn a_directory_others_can_enter_is_not_used() {
    let home = Home::new();
    let runtime = home.private_dir("runtime");
    let print = fingerprint(&home.root);
    let shared = runtime.join(format!("miyu-{print}"));
    let dirs = Dirs {
        platform: Platform::Linux,
        runtime_dir: Some(runtime.clone()),
        ..home.dirs()
    };
    home.dir_with_mode(&format!("runtime/miyu-{print}"), 0o755);
    let error = open(&home.root, &dirs).expect_err("别人进得来的目录");
    assert!(
        matches!(&error, OpenError::NotPrivate(dir) if *dir == shared),
        "{error:?}"
    );

    fs::remove_dir(&shared).expect("删得了");
    let elsewhere = home.private_dir("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, &shared).expect("建得了链接");
    let error = open(&home.root, &dirs).expect_err("链接过去的目录");
    assert!(matches!(error, OpenError::NotPrivate(_)), "{error:?}");
}

#[tokio::test]
async fn the_temp_dir_fallback_must_be_private_too() {
    let home = Home::deep();
    let dirs = home.dirs();
    let uid = dirs.uid.expect("Unix 上有用户编号");
    home.dir_with_mode(&format!("t/miyu-{uid}"), 0o777);
    let error = open(&home.root, &dirs).expect_err("别人进得来的目录");
    assert!(matches!(error, OpenError::NotPrivate(_)), "{error:?}");
}

#[tokio::test]
async fn a_head_does_not_connect_through_a_directory_others_can_enter() {
    let home = Home::new();
    let runtime = home.private_dir("runtime");
    let dirs = Dirs {
        platform: Platform::Linux,
        runtime_dir: Some(runtime),
        ..home.dirs()
    };
    let opened = open(&home.root, &dirs).expect("起得来");
    let dir = opened
        .listener
        .path()
        .parent()
        .expect("有上一层")
        .to_path_buf();
    let relative = dir.strip_prefix(&home.dir).expect("在临时目录下");
    home.dir_with_mode(&relative.to_string_lossy(), 0o755);
    let error = connect(&home.root).await.expect_err("不连");
    assert!(
        matches!(&error, ConnectError::NotPrivate(shared) if *shared == dir),
        "{error:?}"
    );
}

#[tokio::test]
async fn without_a_core_there_is_nothing_to_connect_to() {
    let home = Home::new();
    let error = connect(&home.root).await.expect_err("没起过");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
    drop(open(&home.root, &home.dirs()).expect("起得来"));
    let error = connect(&home.root).await.expect_err("走了");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
    let socket = home.root.run().join("core.sock");
    drop(StdListener::bind(&socket).expect("绑得上"));
    let error = connect(&home.root).await.expect_err("崩了留下的旧套接字");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
    // 记下的位置所在的目录没了：例如重启以后 `$XDG_RUNTIME_DIR` 清空了。
    fs::write(
        home.root.run().join("socket"),
        format!("{}/gone/core.sock\n", home.dir.display()),
    )
    .expect("写得进");
    let error = connect(&home.root).await.expect_err("目录没了");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
}
