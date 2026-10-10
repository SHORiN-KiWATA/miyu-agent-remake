//! 网页演示的桥：一个端口上给页面文件，`/ws` 上把浏览器的 WebSocket 转到核心的本机套接字。
//!
//! 它顶替核心里还没有的网页模块（设计 21 X5「页面由核心的 HTTP 模块提供」、04 P5），所以只做模块会做的事：
//!
//! - 只听 127.0.0.1（04 P6：远程访问默认关闭）；
//! - 起来时换一个访问口令，打出带着它的网址（`#k=…` 在 `#` 后面，不发给服务器、不进 Referer）；页面拿到以后
//!   从地址栏抹掉，连 `/ws` 时带上。没有口令的连不上：同一台机器上的别的程序、沙盒里的命令不能冒充你
//!   （设计 21 X6 的一次性登录链接，演示里简化成这一次启动有效）；
//! - 核对 Origin，别的网站连不进来（04 第二节「各平台的坑」）；
//! - 握手时替页面出示本机令牌：浏览器读不到本机令牌，也不该读到（11 第五节）；
//! - 读历史用核心的订阅补发（`subscribe` 的 `after`）；桥不读会话日志。
//! - 给人看的字不经桥：页面握手以后直接问核心的 `human.get`（核心施工 W-1）；
//! - mermaid 图不经桥：页面直接问核心的 `mermaid.render`（核心施工 W-4）；
//! - 本机文件、blob 经 `/file`、`/blob` 给（`media.rs`），带口令；读的那一半经核心的 `fs.read`、`blob.get`（核心施工 W-6，`core.rs`）。
//! - 页面连不上时经 `/key?k=口令` 问口令对不对（`204`、`403`，别的不说），好写清楚是桥重启过换了口令还是别的。
//! - 附件不经桥：页面经 WebSocket 分块直接传给核心（`blob.open`、`blob.write`、`blob.close`，核心施工 W-5）。
//! - 链接卡片不经桥：页面直接问核心的 `link.preview`，卡片的图是 blob，照 `/blob` 取（核心施工 W-7）。
//! - `@` 选文件不经桥：页面直接问核心的 `fs.list`、`fs.find`（核心施工 W-2）。
//! - 选工作区的「选择文件夹…」经 `/pick-dir?k=口令&title=…&start=…` 开系统的选目录对话框（`dialog.rs`），回选的绝对路径。
//! - 软件后台页经 `POST /page` 换票据、`/p/<票据>/<包>/<路径>` 给文件（`pages.rs`，核心 F-6 中）：文件经核心的 `package.file` 读。
//!
//! 用法：`cargo run -- [端口] [--package <编号>]`，默认 8765；页面文件是这个 crate 上一层的 `web-demo/`。带 `--package` 的打出的网址
//! 打开时直接到「软件后台」里这个软件的页面（照 `miyu web --package`）。
//! 核心没在跑、给了 `MIYU_CORE_BIN` 的，拉起来（`<它> core`）；别的环境变量（`MIYU_HOME`、数据根的配置里
//! key 引用的那个，开发用的是 `DEEPSEEK_API_KEY`）由拉起的核心照常读。

mod core;
mod dialog;
mod files;
mod link;
mod media;
mod pages;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

/// 这一次启动的几样：页面文件在哪、访问口令、端口。
pub struct Site {
    /// 页面文件的目录（`web-demo/`）。
    pub dir: PathBuf,
    /// 访问口令：十六进制。
    pub key: String,
    /// 在哪个端口上听。
    pub port: u16,
    /// 给本机文件、blob 时的媒体类型（`/file`、`/blob`）。
    pub types: media::Types,
    /// 软件后台页的票据（`/page`、`/p/`）。
    pub pages: pages::Tickets,
    /// 软件后台页的文件的媒体类型（资源目录的 `web/web.json`）。
    pub page_types: pages::PageTypes,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port = args.iter().find_map(|a| a.parse().ok()).unwrap_or(8765);
    // 打开时直接到哪个软件的后台页：只认包编号的写法（字母、数字、`-`、`_`、`.`），别的不带
    let launch = args.iter().position(|a| a == "--package").and_then(|i| args.get(i + 1))
        .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)))
        .map(|id| format!("&package={id}"))
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let dir = match dir.canonicalize() {
        Ok(dir) => dir,
        Err(e) => return eprintln!("找不到页面文件的目录 {}：{e}", dir.display()),
    };
    let mut bytes = [0u8; 24];
    if let Err(e) = getrandom::fill(&mut bytes) {
        return eprintln!("取不到随机数：{e}");
    }
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => return eprintln!("听不了 127.0.0.1:{port}：{e}"),
    };
    let types = match media::Types::load(&dir) {
        Ok(t) => t,
        Err(e) => return eprintln!("{e}"),
    };
    let page_types = match pages::PageTypes::load() {
        Ok(t) => t,
        Err(e) => return eprintln!("{e}"),
    };
    let site = Arc::new(Site { dir, key, port, types, pages: pages::Tickets::default(), page_types });
    println!("网页演示（真核心）：http://127.0.0.1:{port}/#k={}{launch}", site.key);
    println!("这个链接这一次启动有效；只在本机能打开。Ctrl+C 停。");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let site = site.clone();
                tokio::spawn(async move { serve(stream, site).await });
            }
            Err(e) => eprintln!("接不了连接：{e}"),
        }
    }
}

/// 一条连接：看一眼请求行，`/ws` 的交给 WebSocket，别的当页面文件。
async fn serve(stream: TcpStream, site: Arc<Site>) {
    let mut head = [0u8; 1024];
    let mut seen = 0;
    for _ in 0..50 {
        seen = match stream.peek(&mut head).await {
            Ok(n) => n,
            Err(_) => return,
        };
        if seen == 0 || head[..seen].contains(&b'\n') {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    if head[..seen].starts_with(b"GET /ws") {
        link::run(stream, site).await;
    } else if let Err(e) = files::serve(stream, &site).await {
        eprintln!("给页面文件时出错：{e}");
    }
}
