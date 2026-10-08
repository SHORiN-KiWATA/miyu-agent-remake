//! `miyu-onebot web`（施工 O-16，`onebot.md` 第二条「对外的样子」、「施工时定的」第 5 条）：桥要已经在跑，不在跑的说先
//! `miyu onebot start`（施工 O-18 改）；还没设过密码的网址带 `#setup=<一次性码>`，别的不带；`--print`、交不给浏览器的印网址（带码的另
//! 提醒）。核心是替身，照握手回的中文说。网页的端口照状态文件里桥实际听的，没有的照清单的默认值（施工 O-20）。

use std::sync::Mutex;
use std::sync::atomic::Ordering;

use miyu_onebot::open::{Browser, Open, Opening, open, port};
use miyu_onebot::serve::CoreCommand;
use miyu_onebot::status_file;
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{CODE, fake_core};
use crate::support::*;

/// 记下交给它的网址；`works` 是假的就当交不出去。
struct FakeBrowser {
    works: bool,
    opened: Mutex<Vec<String>>,
}

impl Browser for FakeBrowser {
    fn open(&self, url: &str) -> bool {
        self.opened.lock().expect("没 panic").push(url.to_string());
        self.works
    }
}

/// 跑一次：交回退出码、标准输出、标准错误、交给浏览器的网址。
async fn run(
    root: &DataRoot,
    port: u16,
    print: bool,
    works: bool,
) -> (u8, String, String, Vec<String>) {
    let browser = FakeBrowser {
        works,
        opened: Mutex::new(Vec::new()),
    };
    let mut texts = Texts::load(ResourceRoot::at(resources()), "en").expect("读得出来");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let core: CoreCommand = std::sync::Arc::new(no_core);
    let code = open(
        root,
        port,
        &Open { print },
        &core,
        &browser,
        &mut texts,
        &mut out,
        &mut err,
    )
    .await;
    let opened = browser.opened.lock().expect("没 panic").clone();
    (
        code,
        String::from_utf8(out).expect("UTF-8"),
        String::from_utf8(err).expect("UTF-8"),
        opened,
    )
}

/// 照中文说的那一句。
fn zh(opening: &Opening) -> String {
    Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .opening(opening)
}

#[tokio::test]
async fn without_a_running_bridge_it_says_to_start_one() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    // 没人听的端口：系统挑一个、绑着不听，用完才放。挑来马上放掉的号，负载高时可能被别的测试拿去听（`support/ports.rs`）。
    let unheard = tokio::net::TcpSocket::new_v4().expect("开得了");
    unheard.bind(([127, 0, 0, 1], 0).into()).expect("挑得到");
    let port = unheard.local_addr().expect("有地址").port();
    let (code, out, err, opened) = run(&root, port, false, true).await;
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(opened.is_empty(), "没开浏览器");
    // 这时还没连核心，照起来时的语言说（测试给的是英文）。
    let english = Texts::load(ResourceRoot::at(resources()), "en")
        .expect("读得出来")
        .opening(&Opening::NotRunning(port));
    assert_eq!(err, format!("{english}\n"));
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn the_first_time_the_address_carries_a_one_time_code() {
    let (dir, root) = temp_root();
    let core = fake_core(&root);
    let bridge = start(serve(root.clone(), settings())).await;
    let site = format!("http://127.0.0.1:{}", bridge.web);
    let (code, out, err, opened) = run(&root, bridge.web, false, true).await;
    assert_eq!(code, 0);
    assert_eq!(out, "");
    assert_eq!(opened, [format!("{site}/#setup={CODE}")]);
    assert_eq!(
        err,
        format!(
            "{}\n{}\n{}\n",
            zh(&Opening::First),
            zh(&Opening::Opened(site.clone())),
            zh(&Opening::PrintHint)
        )
    );
    core.first.store(false, Ordering::SeqCst);
    let (code, _, err, opened) = run(&root, bridge.web, false, true).await;
    assert_eq!(code, 0);
    assert_eq!(opened, [format!("{site}/")], "设过密码的不带码");
    assert_eq!(err, format!("{}\n", zh(&Opening::Opened(site.clone()))));
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn print_or_no_browser_prints_the_address() {
    let (dir, root) = temp_root();
    let core = fake_core(&root);
    let bridge = start(serve(root.clone(), settings())).await;
    let site = format!("http://127.0.0.1:{}", bridge.web);
    let (code, out, err, opened) = run(&root, bridge.web, true, true).await;
    assert_eq!(code, 0);
    assert!(opened.is_empty(), "--print 不开浏览器");
    assert_eq!(out, format!("{site}/#setup={CODE}\n"));
    assert_eq!(
        err,
        format!(
            "{}\n{}\n",
            zh(&Opening::OpenThis),
            zh(&Opening::CodeWarning)
        )
    );
    core.first.store(false, Ordering::SeqCst);
    let (code, out, err, opened) = run(&root, bridge.web, false, false).await;
    assert_eq!(code, 0);
    assert_eq!(opened, [format!("{site}/")], "试过交给浏览器");
    assert_eq!(out, format!("{site}/\n"), "交不出去照 --print 办");
    assert_eq!(err, format!("{}\n", zh(&Opening::OpenThis)));
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[test]
fn the_port_comes_from_the_status_file_or_the_manifest() {
    let (dir, root) = temp_root();
    assert_eq!(port(&root, 8302), 8302, "桥还没在这里跑过：照清单的默认值");
    let file = status_file::path(&root);
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(
        &file,
        r#"{"pid": 1, "listen": 18301, "web": 18402, "napcat": {"connected": false}}"#,
    )
    .expect("写得进");
    assert_eq!(port(&root, 8302), 18402, "照桥实际听的");
    for broken in [
        "not json",
        r#"{"pid": 1, "web": 70000}"#,
        r#"{"pid": 1, "web": "18402"}"#,
    ] {
        std::fs::write(&file, broken).expect("写得进");
        assert_eq!(port(&root, 8302), 8302, "{broken}");
    }
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}
