//! `miyu-onebot web`（`onebot.md` 第二条「对外的样子」，施工 O-16）：打开桥的 WebUI。
//!
//! 1. 桥要已经在跑（「施工时定的」第 5 条：桥由核心照开关拉起，开不开是人的决定，这里不替人开）：连不上
//!    `127.0.0.1:<端口>` 的，说先 `miyu onebot start`，退出码 1（施工 O-18 改）。端口照状态文件里桥实际听的（[`port`]，施工
//!    O-20：桥不再自己读配置；端口改了当场换，用不着提醒 `restart`）。
//! 2. 照终端的样子连核心（出示本机令牌），握手以后照核心回的语言说；问一次 `account.setup_code`：还没设过密码的（`first`），
//!    网址带 `#setup=<一次性码>`（照 `miyu web`，`web-ui.md`「怎么走」第二条第 3 款）；别的网址就是地址本身，页面用存着的
//!    登录令牌，没有的问用户名和密码。
//! 3. `--print`、交不给浏览器的：印网址（带了码的另印一句提醒）。别的交给浏览器打开。
//!
//! 网址印在标准输出上；别的话印在标准错误上。连核心、开浏览器是和网页软件共用的 `miyu_webserve::open`。

use std::io::Write;

use serde_json::json;
use tokio::net::TcpStream;

use miyu_store::root::DataRoot;
use miyu_webserve::CoreCommand;
use miyu_webserve::open::Core;
pub use miyu_webserve::open::{Browser, SystemBrowser};

use crate::serve::Failure;
use crate::status_file;
use crate::texts::Texts;

/// 没开成的退出码。
const FAILED: u8 = 1;

/// `miyu-onebot web` 的参数。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Open {
    /// 不开浏览器，印网址。
    pub print: bool,
}

/// `miyu-onebot web` 说给人听的（「给人看的字」）：怎么说照 `texts`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// 这个端口上没有桥的 WebUI。
    NotRunning(u16),
    /// 还没设过登录密码：带着一次性码开。
    First,
    /// 交给了浏览器：网页的地址（不带码）。
    Opened(String),
    /// 带了码的：浏览器没打开怎么办。
    PrintHint,
    /// `--print`、交不给浏览器：下一行是网址。
    OpenThis,
    /// 印出来的网址带了码：5 分钟、一次、别给别人。
    CodeWarning,
}

/// 桥的网页在哪个端口（第一条「施工时定的」第 43 条）：状态文件（`crate::status_file`）里桥实际听的
/// `web`；没有状态文件、读不懂的（桥还没在这个数据根上跑过）照 `fallback`（清单的默认值）。
pub fn port(root: &DataRoot, fallback: u16) -> u16 {
    status_file::read(root)
        .and_then(|file| file["web"].as_u64())
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or(fallback)
}

/// 照 `open` 走一遍，交回退出码。`port` 是桥的网页的端口（[`port`]）；核心没在跑时照 `core` 拉起；说的话照 `texts`，握手以后换成核心回
/// 的语言。
#[expect(
    clippy::too_many_arguments,
    reason = "照 miyu-web 的 open：几样都是调的一方给的，测试换得掉"
)]
pub async fn open(
    root: &DataRoot,
    port: u16,
    open: &Open,
    core: &CoreCommand,
    browser: &dyn Browser,
    texts: &mut Texts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        say(err, &texts.opening(&Opening::NotRunning(port)));
        return FAILED;
    }
    let mut core = match Core::connect(root, core, "onebot").await {
        Ok(core) => core,
        Err(reason) => {
            say(err, &texts.failure(&Failure::Core(reason)));
            return FAILED;
        }
    };
    if let Some(language) = core.language()
        && let Err(error) = texts.speak(language)
    {
        // 那种语言的字读不懂：照实说一行，接着照原来的说。
        say(err, &format!("{}: {error}", crate::PROGRAM));
    }
    let issued = match core.call("code", "account.setup_code", json!({})).await {
        Ok(issued) => issued,
        Err(reason) => {
            say(err, &texts.failure(&Failure::Core(reason)));
            return FAILED;
        }
    };
    let first = issued["first"].as_bool().unwrap_or(false);
    let code = issued["code"].as_str().filter(|_| first);
    let site = format!("http://127.0.0.1:{port}");
    let url = match code {
        Some(code) => format!("{site}/#setup={code}"),
        None => format!("{site}/"),
    };
    if !open.print && browser.open(&url) {
        if first {
            say(err, &texts.opening(&Opening::First));
        }
        say(err, &texts.opening(&Opening::Opened(site)));
        if code.is_some() {
            say(err, &texts.opening(&Opening::PrintHint));
        }
        return 0;
    }
    say(err, &texts.opening(&Opening::OpenThis));
    say(out, &url);
    if code.is_some() {
        say(err, &texts.opening(&Opening::CodeWarning));
    }
    0
}

/// 印一行；印不出来也没有别处可说了。
fn say(to: &mut dyn Write, line: &str) {
    if writeln!(to, "{line}").is_err() {
        // 标准输出、标准错误关了：没有别处可说。
    }
}
