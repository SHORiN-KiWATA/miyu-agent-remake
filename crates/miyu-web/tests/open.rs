//! `miyu-web open`（施工 W-9，`web-module.md`「怎么走」第十一条）：网页软件没在跑先拉起来（真的 `miyu-web serve`）；
//! 没设过密码、`--reset` 的网址带 `#setup=<一次性码>`，别的不带；`--print` 印网址和提醒；交不给浏览器的照 `--print` 办；
//! `--logout` 说作废了几个。核心是替身：照方法名答。

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::support::*;
use miyu_ipc::Listener;
use miyu_web::open::{Browser, Launch, Open, open};

const CODE: &str = "9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c";

/// 核心的替身：一条条连接接着，`hello` 回中文，`account.setup_code` 照 `first` 回，`account.logout` 写了 `all` 的回作废了 3 个。
fn answer(mut core: Listener, first: bool) {
    tokio::spawn(async move {
        while let Ok(connection) = core.accept().await {
            tokio::spawn(async move {
                let (read, mut write) = tokio::io::split(connection);
                let mut lines = BufReader::new(read);
                let mut line = String::new();
                while lines.read_line(&mut line).await.is_ok_and(|read| read > 0) {
                    let request: Value = serde_json::from_str(&line).expect("是 JSON");
                    let result = match request["method"].as_str() {
                        Some("hello") => {
                            json!({"account": "admin", "language": "zh", "protocol": 1})
                        }
                        Some("account.setup_code") => {
                            json!({"code": CODE, "expires": "2026-10-04T00:05:00.000Z", "first": first})
                        }
                        // 只有 `all` 的作废全部；本机令牌的连接不写 `all` 真核心会拒。
                        Some("account.logout") if request["params"]["all"] == json!(true) => {
                            json!({"revoked": 3})
                        }
                        Some("account.logout") => json!({"revoked": 0}),
                        _ => json!({}),
                    };
                    let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
                    if write
                        .write_all(format!("{reply}\n").as_bytes())
                        .await
                        .is_err()
                    {
                        return;
                    }
                    line.clear();
                }
            });
        }
    });
}

/// 记下交给它的网址；`works` 是假的就当交不出去。
struct FakeBrowser {
    works: bool,
    opened: Mutex<Vec<String>>,
}

impl Browser for FakeBrowser {
    fn open(&self, url: &str) -> bool {
        self.opened.lock().expect("没坏").push(url.to_string());
        self.works
    }
}

/// 拉起的是真的 `miyu-web serve`：数据根、资源目录照这个临时目录，空闲 1 秒就走。
fn launch(home: &Home) -> Launch {
    let resources = home.dir.join("resources");
    std::fs::create_dir_all(resources.join("web")).expect("建得了");
    std::fs::create_dir_all(resources.join("packages")).expect("建得了");
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    std::fs::copy(shipped.join("web/web.json"), resources.join("web/web.json")).expect("拷得了");
    // 空闲多久照清单的默认值（施工 9-1 下）：这里改成 1 秒，测完很快就走。核心没在跑，`serve` 照默认起。
    let manifest =
        std::fs::read_to_string(shipped.join("packages/web.toml")).expect("出厂的读得到");
    let quick = manifest.replace("default = 600\n", "default = 1\n");
    assert_ne!(quick, manifest, "清单里有空闲的默认值");
    std::fs::write(resources.join("packages/web.toml"), quick).expect("写得进");
    let data = home.root.path().to_path_buf();
    let pages = home.pages.clone();
    Launch {
        serve: Box::new(move |port| {
            let mut serve = Command::new(env!("CARGO_BIN_EXE_miyu-web"));
            serve
                .arg("serve")
                .env("MIYU_HOME", &data)
                .env("MIYU_RESOURCES", &resources)
                .env("MIYU_WEB_PAGES", &pages);
            if let Some(port) = port {
                serve.args(["--port", &port.to_string()]);
            }
            serve
        }),
        core: no_core(),
    }
}

/// 走一遍 `open`：交回退出码、标准输出、标准错误。
async fn run_open(home: &Home, args: Open, browser: &FakeBrowser) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = open(
        &home.root,
        &args,
        &launch(home),
        browser,
        &mut out,
        &mut err,
    )
    .await;
    (
        code,
        String::from_utf8(out).expect("UTF-8"),
        String::from_utf8(err).expect("UTF-8"),
    )
}

fn browser(works: bool) -> FakeBrowser {
    FakeBrowser {
        works,
        opened: Mutex::new(Vec::new()),
    }
}

#[tokio::test]
async fn the_first_time_opens_with_a_code_after_starting_the_web_ui() {
    let home = Home::new();
    answer(fake_core(&home), true);
    let browser = browser(true);
    let (code, out, err) = run_open(
        &home,
        Open {
            port: Some(0),
            ..Open::default()
        },
        &browser,
    )
    .await;
    assert_eq!(code, 0, "{err}");
    let site = miyu_web::serve::address(&home.root).expect("拉起来了、写了地址");
    assert!(miyu_web::serve::running(&home.root));
    assert_eq!(
        *browser.opened.lock().expect("没坏"),
        [format!("{site}/#setup={CODE}")]
    );
    assert_eq!(out, "", "网址交给了浏览器，不印");
    assert!(err.contains("还没设过网页的登录密码"), "{err}");
    assert!(
        err.contains(&format!("网页开在 {site}，已经交给浏览器打开。")),
        "{err}"
    );
    assert!(err.contains("miyu web --print"), "{err}");
    assert!(!err.contains(CODE), "码只在网址里：{err}");
    // 已经在跑的照它的地址，不再拉起。
    let again = browser_again(&home, Open::default(), true).await;
    assert_eq!(again, [format!("{site}/#setup={CODE}")]);
}

async fn browser_again(home: &Home, args: Open, works: bool) -> Vec<String> {
    let browser = browser(works);
    let (code, _, _) = run_open(home, args, &browser).await;
    assert_eq!(code, 0);
    browser.opened.into_inner().expect("没坏")
}

#[tokio::test]
async fn later_opens_carry_no_code_unless_reset() {
    let home = Home::new();
    answer(fake_core(&home), false);
    let browser = browser(true);
    let (code, _, err) = run_open(
        &home,
        Open {
            port: Some(0),
            ..Open::default()
        },
        &browser,
    )
    .await;
    assert_eq!(code, 0, "{err}");
    let site = miyu_web::serve::address(&home.root).expect("有地址");
    assert_eq!(*browser.opened.lock().expect("没坏"), [format!("{site}/")]);
    assert!(
        !err.contains("还没设过") && !err.contains("--print"),
        "{err}"
    );
    let reset = browser_again(
        &home,
        Open {
            reset: true,
            ..Open::default()
        },
        true,
    )
    .await;
    assert_eq!(reset, [format!("{site}/#setup={CODE}")]);
}

#[tokio::test]
async fn print_and_a_missing_browser_print_the_address() {
    let home = Home::new();
    answer(fake_core(&home), true);
    let browser = browser(true);
    let (code, out, err) = run_open(
        &home,
        Open {
            port: Some(0),
            print: true,
            ..Open::default()
        },
        &browser,
    )
    .await;
    assert_eq!(code, 0, "{err}");
    let site = miyu_web::serve::address(&home.root).expect("有地址");
    assert!(
        browser.opened.lock().expect("没坏").is_empty(),
        "--print 不开浏览器"
    );
    assert_eq!(out, format!("{site}/#setup={CODE}\n"));
    assert!(err.contains("在浏览器里打开："), "{err}");
    assert!(err.contains("5 分钟内有效，只能用一次"), "{err}");
    let nothing = FakeBrowser {
        works: false,
        opened: Mutex::new(Vec::new()),
    };
    let (code, out, _) = run_open(&home, Open::default(), &nothing).await;
    assert_eq!(code, 0);
    assert_eq!(
        out,
        format!("{site}/#setup={CODE}\n"),
        "交不出去照 --print 办"
    );
}

#[tokio::test]
async fn logout_says_how_many_and_leaves_the_web_ui_alone() {
    let home = Home::new();
    answer(fake_core(&home), false);
    let browser = browser(true);
    let (code, out, err) = run_open(
        &home,
        Open {
            logout: true,
            ..Open::default()
        },
        &browser,
    )
    .await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "");
    assert!(err.contains("作废了 3 个登录"), "{err}");
    assert!(browser.opened.lock().expect("没坏").is_empty());
    assert!(!miyu_web::serve::running(&home.root), "没拉起网页软件");
}

#[tokio::test]
async fn a_taken_port_is_said_in_the_persons_language() {
    let home = Home::new();
    answer(fake_core(&home), true);
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("占得了");
    let port = taken.local_addr().expect("有地址").port();
    let (code, _, err) = run_open(
        &home,
        Open {
            port: Some(port),
            ..Open::default()
        },
        &browser(true),
    )
    .await;
    assert_eq!(code, 1);
    assert!(
        err.contains(&format!(
            "端口 {port} 被占了。换一个：miyu web --port <端口>"
        )),
        "{err}"
    );
    let _ = Arc::new(());
}
