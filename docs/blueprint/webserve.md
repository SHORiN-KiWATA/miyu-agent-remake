## 网页端口共用的底子 `miyu-webserve`

### 是什么

一个只听本机的网页端口要的几样：给页面文件、核对 Host 和 Origin、`/ws` 原样转给核心、照终端的样子连核心要一次性码再开浏览器。网页软件 `miyu-web`（`web-ui.md`）用它；QQ 桥原来的 WebUI 也用（`docs/designs/18-通讯平台.md` 第三节「抽成两边共用的库，不抄一份」），随施工 O-28 下去掉，桥只剩 `start`、`stop`、`restart`、`status` 照终端的样子连核心用 `open::Core`（`onebot.md` 第一条「施工时定的」第 166 条）。

状态：施工 O-16 从 `miyu-web` 抽出来（2026-10-07）。函数原样搬，不改名、不改行为，`miyu-web` 的行为一个字节不变；每个函数从哪搬到哪见「搬家表」（W-11 打包的分支变基时照它挪）。

规矩本身（路径、类型、响应头、Origin、一帧一行、1003、1009、1012、`web.error`、关了以后读掉再放）写在 `web-ui.md`「怎么走」第一条第 4、5、7、8、9 款和第二条第 3、4 款，这一页不重抄，只写这个 crate 交出什么、谁用、从哪搬来。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-webserve/`（第 3 层，执行器） | 只依赖 `miyu-ipc`、`miyu-store`；不依赖 `miyu-web`、`miyu-onebot` 或别的头 |
| `crates/miyu-webserve/src/lib.rs` | `Site`：Host、Origin 认的三种写法（`hosts`、`host_allowed`），`/ws` 要的数据根、怎么拉起核心、连着的怎么数；`CoreCommand` |
| `crates/miyu-webserve/src/respond.rs` | 回应的正文 `Body`、`full`、`empty`，一律带的 `nosniff`、`no-referrer`（`secure`），字写成头的值（`value`） |
| `crates/miyu-webserve/src/pages.rs` | 给一个页面文件（`serve`）：405、404、类型照表（`type_of`）、四个头；路径不出页面目录（`find`、`decode`） |
| `crates/miyu-webserve/src/ws.rs` | `/ws`：核对 Origin、升级、连核心不读本机令牌、两头照转、关了以后读掉再放 |
| `crates/miyu-webserve/src/open.rs` | 照终端的样子连核心（拉起或者只连在跑的）、一问一答（`Core`）；系统的办法开浏览器（`Browser`、`SystemBrowser`）；这台机器的语言（`locale`） |

谁用：

| 用的一方 | 用哪几样 |
|---|---|
| `crates/miyu-web/src/serve.rs` | `Site`（网页软件的 `Site` 实现它：数忙照空闲退出那一套）、`respond`、`pages::serve`、`ws::accept`；`CoreCommand`、`Body`、`full`、`empty`、`secure` 照原来的路径转出去（`miyu_web::serve::CoreCommand`，`media.rs` 的 `crate::serve::…` 不用改） |
| `crates/miyu-web/src/settings.rs` | `Settings::type_of` 留着，转给 `pages::type_of`；`serve` 起来时问配置用 `open::Core::connect_running`（报 `miyu-web`） |
| `crates/miyu-web/src/open.rs` | `open::Core`（报 `miyu-web`）；`Browser`、`SystemBrowser` 照原来的路径转出去（`miyu_web::open::Browser`） |
| `crates/miyu-onebot/src/control.rs` | `open::Core`（报 `onebot`）、`CoreCommand`（O-28 下起桥只用这两样：原来的 WebUI 用的 `Site`、`respond`、`pages::serve`、`ws::accept`、`Browser` 随它去掉） |

### 对外的样子

| 名字 | 是什么 |
|---|---|
| `trait Site` | `type Busy`；`root()`、`port()`（实际听的端口）、`core()`、`busy(self: &Arc<Self>)`；默认方法 `hosts()`（`127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>`）、`host_allowed(host)`（只认这三种，`localhost` 不分大小写） |
| `CoreCommand` | `Arc<dyn Fn() -> Command + Send + Sync>`：拉起核心的命令 |
| `respond::{Body, full, empty, secure, value}` | 见「在哪」 |
| `pages::serve(request, pages, csp, types)` | `GET`、`HEAD` 以外 405；`find` 不到、读不了 404；类型照 `type_of`，带 `Cache-Control: no-cache`、`Content-Security-Policy: <csp>`、`nosniff`、`no-referrer`；`HEAD` 只给头 |
| `pages::type_of(types, path)` | 扩展名（小写）在表里的媒体类型；没有的 `application/octet-stream` |
| `pages::find(pages, path)` | 请求的路径对应页面目录里哪份文件；带 `..`、`.`、空段、反斜杠的，`%xx` 解不开、不是 UTF-8、有 NUL 的，换成真实位置跑出去的，不是普通文件的，都是空的 |
| `ws::accept(request, site)` | Origin 不是 `http://` 加上 `hosts()` 之一的 403；不是 WebSocket 升级的 400；对的回 101，升级好以后在别的任务里转，连着时拿着 `site.busy()` |
| `open::Core::connect(root, core, kind)` | 照终端的样子连核心（没在跑就拉起），出示本机令牌握手，`head.kind` 报 `kind`；`language()` 是握手回的语言 |
| `open::Core::connect_running(root, kind)` | 同上，只连已经在跑的核心，不拉起（施工 9-1 下，网页软件的 `serve` 起来时问配置用） |
| `open::Core::call(id, method, params)` | 一问一答，30 秒没回算没回；拒绝的交回核心的原话 |
| `open::{Browser, SystemBrowser}`、`open::locale()` | 见 `web-ui.md`「怎么走」第二条第 4 款；`LC_ALL`、`LC_MESSAGES`、`LANG` 照先后 |

运行日志的目标照搬来时的 `miyu::web`：网页软件的 `web.log` 一个字不变（「施工时定的」第 3 条）。

### 搬家表

施工 O-16 从 `miyu-web` 原样搬过来（`miyu-web` 改成用它）。W-11 打包的分支改过 `serve.rs`、`ws.rs`，变基时照这张表把改动挪到右边那一格。

| 原来在 `miyu-web` | 现在在 `miyu-webserve` | 搬的时候动了什么 |
|---|---|---|
| `src/serve.rs` 的 `type Body` | `src/respond.rs` 的 `Body` | `pub(crate)` 改 `pub`；`miyu-web` 的 `serve.rs` 照原名转出去 |
| `src/serve.rs` 的 `full` | `src/respond.rs` 的 `full` | 同上 |
| `src/serve.rs` 的 `empty` | `src/respond.rs` 的 `empty` | 同上 |
| `src/serve.rs` 的 `secure` | `src/respond.rs` 的 `secure` | 同上 |
| `src/serve.rs` 的 `value` | `src/respond.rs` 的 `value` | 私有改 `pub` |
| `src/serve.rs` 的 `type CoreCommand` | `src/lib.rs` 的 `CoreCommand` | `miyu-web` 的 `serve.rs` 照原名转出去（`pub use`） |
| `src/serve.rs` 的 `Site::hosts` | `src/lib.rs` 的 `trait Site` 的默认方法 `hosts` | 端口照 `port()` 取 |
| `src/serve.rs` 的 `Site::host_allowed` | `src/lib.rs` 的 `trait Site` 的默认方法 `host_allowed` | 私有改 `pub` |
| `src/serve.rs` 的 `handle` 里给页面文件的那一段（405、`pages::find`、读、`Content-Type`、`Cache-Control`、`Content-Security-Policy`、`secure`） | `src/pages.rs` 的 `serve` | 抽成一个函数，`csp`、`types` 由调的一方给；`handle` 里换成一行 `miyu_webserve::pages::serve(…)` |
| `src/settings.rs` 的 `Settings::type_of` 的身子 | `src/pages.rs` 的 `type_of` | 表由调的一方给；`Settings::type_of` 留着，转给它 |
| `src/pages.rs` 的 `find` | `src/pages.rs` 的 `find` | `pub(crate)` 改 `pub` |
| `src/pages.rs` 的 `decode` | `src/pages.rs` 的 `decode` | 不动 |
| `src/ws.rs` 的 `LIMIT`、`LINGER` | `src/ws.rs` 的 `LIMIT`、`LINGER` | 不动 |
| `src/ws.rs` 的 `accept` | `src/ws.rs` 的 `accept` | `site: Arc<Site>` 改成 `site: Arc<S>`（`S: Site`）；`pub(crate)` 改 `pub` |
| `src/ws.rs` 的 `bridge` | `src/ws.rs` 的 `bridge` | 同上；`site.root`、`site.core` 改成 `site.root()`、`site.core()` |
| `src/ws.rs` 的 `close`、`send`、`finish`、`linger` | `src/ws.rs` 的 `close`、`send`、`finish`、`linger` | 不动 |
| `src/open.rs` 的 `Browser`、`SystemBrowser` | `src/open.rs` 的 `Browser`、`SystemBrowser` | 不动；`miyu-web` 的 `open.rs` 照原名转出去 |
| `src/client.rs`（施工 9-1 下从 `open.rs` 挪出来）的 `Core`、`Core::call` | `src/open.rs` 的 `Core`、`Core::call` | `pub`；`miyu-web` 的 `client.rs` 删掉，`open.rs`、`settings.rs` 改用 `miyu_webserve::open::Core` |
| `src/client.rs` 的 `Core::connect` | `src/open.rs` 的 `Core::connect` | 参数 `core: &CoreCommand` 不动，多一个 `kind`（握手报的头，网页软件报 `miyu-web`，和原来一样）；握手回的语言存成原样的字，`miyu-web` 照它算 `Language` |
| `src/client.rs` 的 `Core::connect_running`、`Core::shake` | `src/open.rs` 的 `Core::connect_running`、`Core::shake` | 都多一个 `kind`，同 `connect` |
| `src/client.rs` 的 `locale` | `src/open.rs` 的 `locale` | `pub` |
| `tests/ws.rs` 的 `the_forwarding_code_never_reads_the_local_token` | 不搬，照搬过去的位置查 `miyu-webserve` 的 `ws.rs`、`pages.rs`、`respond.rs`、`lib.rs` | |

留在 `miyu-web` 的：单实例、听端口、`run/web`、那一行、空闲退出（`Site` 的数忙、`idle_for`、`Busy`）、`handle` 的分派和 Host 不对时的那一行运行日志、`/media`、`web.json`、`open` 的分派和给人看的字、`stopped`。

### 守着它的

这个 crate 的规矩经两个用它的程序整套测：

| 测试 | 守着什么 |
|---|---|
| `crates/miyu-web/tests/serve.rs`、`ws.rs`、`open.rs`、`media.rs` | 原有的测试一个不改照过（`ws.rs` 查源码的那一条照搬过去的位置查）：网页软件的行为一个字节不变 |
| `crates/miyu-webserve/src/tests.rs` | `hosts`、`host_allowed` 的写法；`type_of` 照扩展名、不分大小写、没有的不猜；`find` 带 `..`、`%2e%2e`、`%zz`、NUL、反斜杠的不要 |

### 施工时定的（施工 O-16，2026-10-07）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | `/ws` 要的几样做成 `trait Site`：网页软件、桥各实现一份；`hosts`、`host_allowed` 是默认方法 | `ws.rs` 原样搬，只把 `Site` 换成泛型；网页软件的 `Site` 还管 `/media`、空闲退出，桥的管 `/status`，各留各的 | 搬一个共用的结构体，两家把自己的东西挂在旁边（`media.rs` 里 `site.root` 这些全要改，W-11 变基更难）；数忙的那一套也搬过来（桥用不上空闲退出） |
| 2 | 给页面文件那一段抽成 `pages::serve`，类型表、内容安全策略由调的一方给 | 两家的表、策略放在各自的资源里（`web.json`、`bridge.json`） | 网页软件的 `Settings` 整个搬过来（端口、空闲、票据是网页软件自己的） |
| 3 | 运行日志的目标照搬来时的 `miyu::web` | 网页软件的 `web.log` 一个字不变；`tracing` 的目标只能是常量，不能随用的一方变 | 改成 `miyu::webserve`（网页软件的日志变了） |
| 4 | `CoreCommand`、`Body`、`full`、`empty`、`secure`、`Browser`、`SystemBrowser` 在 `miyu-web` 里照原来的路径转出去 | `media.rs`、`main.rs` 和测试一行不改，W-11 变基少撞 | 改掉每一处用的地方 |
