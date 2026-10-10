//! `miyu-web open`（`web-module.md`「怎么走」第十一条）：`miyu web` 把参数交到这里。
//!
//! 1. 照终端的样子连核心（出示本机令牌，没在跑就拉起来），握手拿到人的语言。
//! 2. `--logout`：`account.logout`、`all: true`，说作废了几个，不碰网页软件。
//! 3. 网页软件没在跑（`run/web.lock` 没人拿着、没有 `run/web`）就拉起 `serve`，等那一行最多 10 秒。
//! 4. 问一次 `account.setup_code`：写了 `--reset` 的、还没设过密码的（`first`），网址带 `#setup=<一次性码>`；别的网址就是
//!    地址本身，页面用存着的登录令牌，没有的问用户名和密码（要了没用的码 5 分钟后自己作废）。
//! 5. `--print`、交不给浏览器的：印网址（带了码的另印一句提醒）。别的交给浏览器打开。
//!
//! 网址印在标准输出上（脚本能拿）；别的话印在标准错误上。照终端的样子连核心、开浏览器这两样在 `miyu-webserve`（施工
//! O-16，`webserve.md`「搬家表」）。

use std::io::Write;
use std::process::Command;
use std::time::Duration;

use serde_json::json;

use miyu_ipc::Ready;
use miyu_store::root::DataRoot;
use miyu_webserve::open::Core;
pub use miyu_webserve::open::{Browser, SystemBrowser};

use crate::serve::{CoreCommand, address, running};
use crate::texts::Language;

/// `open` 的参数。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Open {
    /// 网页软件没在跑时，这一次在哪个端口上听。
    pub port: Option<u16>,
    /// 不开浏览器，印网址。
    pub print: bool,
    /// 忘了密码：要一个一次性码重设。
    pub reset: bool,
    /// 作废全部浏览器的登录。
    pub logout: bool,
    /// 打开时直接到「软件后台」里这个软件的页面（`--package <编号>`，施工 F-6 下）：网址 `#` 后面多一个 `package=<编号>`。
    pub package: Option<String>,
}

/// 怎么拉起东西：`serve`、核心。
pub struct Launch {
    /// 拉起网页软件：自己加 `serve`，`port` 是 `--port`。
    pub serve: Box<dyn Fn(Option<u16>) -> Command + Send + Sync>,
    /// 拉起核心：主程序加 `core`。
    pub core: CoreCommand,
}

/// 照 `open` 走一遍，交回退出码。
pub async fn open(
    root: &DataRoot,
    open: &Open,
    launch: &Launch,
    browser: &dyn Browser,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut core = match Core::connect(root, &launch.core, "miyu-web").await {
        Ok(core) => core,
        Err(reason) => {
            say(err, &Language::En.no_core(&reason));
            return 1;
        }
    };
    let language = Language::of(core.language());
    if open.logout {
        return match core
            .call("logout", "account.logout", json!({"all": true}))
            .await
        {
            Ok(result) => {
                let count = result["revoked"].as_u64().unwrap_or(0);
                say(err, &language.logged_out(count));
                0
            }
            Err(reason) => {
                say(err, &language.no_core(&reason));
                1
            }
        };
    }
    let site = match ensure(root, open.port, launch).await {
        Ok(site) => site,
        Err(Started::PortInUse(port)) => {
            say(err, &language.port_in_use(&port));
            return 1;
        }
        Err(Started::Failed(reason)) => {
            say(err, &language.not_started(&reason));
            return 1;
        }
    };
    let issued = match core.call("code", "account.setup_code", json!({})).await {
        Ok(issued) => issued,
        Err(reason) => {
            say(err, &language.no_core(&reason));
            return 1;
        }
    };
    let first = issued["first"].as_bool().unwrap_or(false);
    let code = issued["code"]
        .as_str()
        .filter(|_| open.reset || first)
        .map(str::to_string);
    let url = address_of(&site, code.as_deref(), open.package.as_deref());
    if !open.print && browser.open(&url) {
        if first && !open.reset {
            say(err, &language.first());
        }
        say(err, &language.opened(&site));
        if code.is_some() {
            say(err, &language.print_hint());
        }
        return 0;
    }
    say(err, &language.open_this(open.reset));
    say(out, &url);
    if code.is_some() {
        say(err, &language.code_warning());
    }
    0
}

/// 印一行；印不出来也没有别处可说了。
/// 交给浏览器的网址：`#` 后面是一次性码（`setup=`）、点名的软件后台页（`package=`），都没有的不带 `#`。
fn address_of(site: &str, code: Option<&str>, package: Option<&str>) -> String {
    let parts: Vec<String> = [
        code.map(|code| format!("setup={code}")),
        package.map(|package| format!("package={package}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    match parts.is_empty() {
        true => format!("{site}/"),
        false => format!("{site}/#{}", parts.join("&")),
    }
}

fn say(to: &mut dyn Write, line: &str) {
    if writeln!(to, "{line}").is_err() {
        // 标准输出、标准错误关了：没有别处可说。
    }
}

/// 网页软件为什么没起来。
enum Started {
    /// 端口被占了：哪一个。
    PortInUse(String),
    /// 别的：原话。
    Failed(String),
}

/// 确保网页软件在跑：交回它的地址。
async fn ensure(root: &DataRoot, port: Option<u16>, launch: &Launch) -> Result<String, Started> {
    if running(root)
        && let Some(site) = address(root)
    {
        return Ok(site);
    }
    match miyu_ipc::spawn_detached((launch.serve)(port), root.path()).await {
        Ok(Ready::Ready | Ready::Running) => {}
        Ok(Ready::Failed(reason)) => {
            return Err(
                match reason
                    .strip_prefix("port ")
                    .and_then(|rest| rest.strip_suffix(" in use"))
                {
                    Some(port) => Started::PortInUse(port.to_string()),
                    None => Started::Failed(reason),
                },
            );
        }
        Err(error) => return Err(Started::Failed(error.to_string())),
    }
    // 别人刚拉起的（`running`）可能还没写好地址：等一会儿。
    for _ in 0..200 {
        if let Some(site) = address(root) {
            return Ok(site);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(Started::Failed(format!(
        "run/{} not written",
        crate::serve::ADDRESS
    )))
}
