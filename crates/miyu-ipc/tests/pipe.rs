//! Windows 上核心在命名管道上等连接，头连过去（施工 3-8 补）。

#![cfg(windows)]

mod support;

use std::path::PathBuf;

use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

use miyu_ipc::{ConnectError, OpenError, connect, fingerprint, open};
use support::{Home, talk, within};

/// 这个数据根的管道名。
fn pipe(home: &Home) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\miyu-{}", fingerprint(&home.root)))
}

#[tokio::test]
async fn a_head_connects_through_the_pipe_and_gets_the_token() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    assert_eq!(opened.listener.path(), pipe(&home));
    assert_eq!(home.location(), pipe(&home));
    assert_eq!(home.token(), opened.token);
    talk(&home, &mut opened).await;
    // 接走一个以后，下一个实例已经等着了。
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn a_head_that_leaves_at_once_does_not_jam_the_pipe() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    // 连上就走，核心还没去接。
    drop(ClientOptions::new().open(pipe(&home)).expect("连得上"));
    // 核心去接：接到的是一个已经走了的连接（Windows 上是这样），报错也行；之后要照样接得了新的。
    match within("接", opened.listener.accept()).await {
        Ok(gone) => drop(gone),
        Err(error) => eprintln!("连上就走的那一个：{error}"),
    }
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn one_core_per_root_until_it_goes() {
    let home = Home::new();
    let first = open(&home.root, &home.dirs()).expect("起得来");
    let error = open(&home.root, &home.dirs()).expect_err("第二个起不来");
    assert!(matches!(error, OpenError::Running), "{error:?}");
    let old = first.token.clone();
    drop(first);
    let mut second = open(&home.root, &home.dirs()).expect("第一个走了以后起得来");
    assert_ne!(second.token, old, "每次起来换一个令牌");
    talk(&home, &mut second).await;
}

#[tokio::test]
async fn a_name_taken_by_another_program_is_left_alone() {
    let home = Home::new();
    let taken = ServerOptions::new()
        .first_pipe_instance(true)
        .create(pipe(&home))
        .expect("别的程序先占了这个名字");
    let error = open(&home.root, &home.dirs()).expect_err("名字被占了");
    assert!(
        matches!(&error, OpenError::Occupied(path) if *path == pipe(&home)),
        "{error:?}"
    );
    drop(taken);
    open(&home.root, &home.dirs()).expect("放开以后起得来");
}

#[tokio::test]
async fn without_a_core_there_is_nothing_to_connect_to() {
    let home = Home::new();
    let error = connect(&home.root).await.expect_err("没起过");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
    drop(open(&home.root, &home.dirs()).expect("起得来"));
    let error = connect(&home.root).await.expect_err("走了");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
}
