//! 不读本机令牌地连核心（施工 W-8，`web-module.md`「起草时定的」第 4 条）：网页软件转发浏览器的连接用它。`run/token` 不在了，
//! `connect` 连不上，`connect_bare` 照样连得上、说得上话；三个平台都一样。

mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use miyu_ipc::{connect, connect_bare, open};
use support::{Home, within};

#[tokio::test]
async fn a_bare_connection_never_reads_the_token() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    std::fs::remove_file(home.root.run().join("token")).expect("删得掉");
    assert!(
        within("连上", connect(&home.root)).await.is_err(),
        "要读令牌的连不上"
    );
    // 上面那一次连上了才去读令牌，核心这边先接走它。
    drop(within("接到连接", opened.listener.accept()).await);
    let mut head = within("连上", connect_bare(&home.root))
        .await
        .expect("不读令牌的连得上");
    let mut core = within("接到连接", opened.listener.accept())
        .await
        .expect("接得到");
    head.write_all(b"ping").await.expect("写得进");
    let mut heard = [0u8; 4];
    within("核心读到", core.read_exact(&mut heard))
        .await
        .expect("读得到");
    assert_eq!(&heard, b"ping");
}
