//! `miyu-web open`（`web-module.md`「怎么走」第十一条）：`miyu web` 把参数交到这里。
//!
//! 1. 照终端的样子连核心（出示本机令牌，没在跑就拉起来），握手拿到人的语言。
//! 2. `--logout`：`account.logout`、`all: true`，说作废了几个，不碰网页软件。
//! 3. 网页软件没在跑（`run/web.lock` 没人拿着、没有 `run/web`）就拉起 `serve`，等那一行最多 10 秒。
//! 4. 问一次 `account.setup_code`：写了 `--reset` 的、还没设过密码的（`first`），网址带 `#setup=<一次性码>`；别的网址就是
//!    地址本身，页面用存着的登录令牌，没有的问用户名和密码（要了没用的码 5 分钟后自己作废）。
//! 5. `--print`、交不给浏览器的：印网址（带了码的另印一句提醒）。别的交给浏览器打开。
//!
//! 网址印在标准输出上（脚本能拿）；别的话印在标准错误上。

use std::io::Write;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::{Connection, Ready};
use miyu_store::root::DataRoot;

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
}

/// 怎么拉起东西：`serve`、核心。
pub struct Launch {
    /// 拉起网页软件：自己加 `serve`，`port` 是 `--port`。
    pub serve: Box<dyn Fn(Option<u16>) -> Command + Send + Sync>,
    /// 拉起核心：主程序加 `core`。
    pub core: CoreCommand,
}

/// 怎么把网址交给浏览器：交出去了的是真。
pub trait Browser {
    /// 打开 `url`。
    fn open(&self, url: &str) -> bool;
}

/// 系统的办法：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" <网址>`。
pub struct SystemBrowser;

impl Browser for SystemBrowser {
    fn open(&self, url: &str) -> bool {
        let mut command = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        } else {
            Command::new("xdg-open")
        };
        command
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command.status().is_ok_and(|status| status.success())
    }
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
    let mut core = match Core::connect(root, launch).await {
        Ok(core) => core,
        Err(reason) => {
            say(err, &Language::En.no_core(&reason));
            return 1;
        }
    };
    let language = core.language;
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
    let url = match &code {
        Some(code) => format!("{site}/#setup={code}"),
        None => format!("{site}/"),
    };
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

/// 照终端的样子连着的核心：一问一答。
struct Core {
    lines: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    language: Language,
}

impl Core {
    /// 连核心（没在跑就拉起来）、出示本机令牌握手。
    async fn connect(root: &DataRoot, launch: &Launch) -> Result<Core, String> {
        let (connection, token) = miyu_ipc::connect_or_start(root, || (launch.core)())
            .await
            .map_err(|error| error.to_string())?;
        let (read, write) = tokio::io::split(connection);
        let mut core = Core {
            lines: BufReader::new(read),
            write,
            language: Language::En,
        };
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": "miyu-web", "version": env!("CARGO_PKG_VERSION")},
            "locale": locale(),
            "token": token,
        });
        let shaken = core.call("hello", "hello", hello).await?;
        core.language = Language::of(shaken["language"].as_str());
        Ok(core)
    }

    /// 发一条请求、等它的回应：交回 `result`，拒绝的交回原话。
    async fn call(&mut self, id: &str, method: &str, params: Value) -> Result<Value, String> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let sent = async {
            self.write
                .write_all(format!("{request}\n").as_bytes())
                .await?;
            self.write.flush().await
        };
        sent.await.map_err(|error| error.to_string())?;
        loop {
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(30), self.lines.read_line(&mut line))
                    .await
                    .map_err(|_| "no answer".to_string())?
                    .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("core disconnected".to_string());
            }
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"] != json!(id) {
                continue;
            }
            return match reply.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(reply["error"]["message"]
                    .as_str()
                    .unwrap_or("refused")
                    .to_string()),
            };
        }
    }
}

/// 这台机器的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 照先后（核心照它算 `auto` 的语言）。
fn locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}
