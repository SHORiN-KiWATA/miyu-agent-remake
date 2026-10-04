## 网页软件 `miyu-web`

### 是什么

网页界面是一个单独的程序 `miyu-web` 加一套页面文件，装了才有。它是核心的一个头：自己开一个只听本机的 HTTP 端口，给页面，把浏览器的 WebSocket 一帧一条转成核心协议的一行一条，经本机套接字（Windows 上是命名管道）连核心，不读本机令牌。主程序的 `miyu web` 找到它、把参数交给它。

状态：施工 W-9 做好了起停、页面、WebSocket 照转和 `miyu web`；媒体地址 `/media` 随 W-10（`web-module.md` 第十条），打包随 W-11。这一页从 `web-module.md` 搬出来（第九条、第十一条，「要跟着改的别的页」里定的「网页软件一页」）；核心给网页的通用方法、身份还在那一页。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-web/`（第 5 层，头） | 程序 `miyu-web`：只依赖 `miyu-ipc`、`miyu-store`、`miyu-log`，不依赖核心 |
| `crates/miyu-web/src/main.rs` | 子命令 `open`、`serve`；找自己旁边的主程序拉起核心 |
| `crates/miyu-web/src/serve.rs` | 单实例、听端口、写 `run/web` 和那一行、空闲退出；核对 Host、给页面 |
| `crates/miyu-web/src/pages.rs` | 页面文件：`/` 是 `index.html`，不出页面目录 |
| `crates/miyu-web/src/ws.rs` | 核对 Origin；WebSocket 和核心连接两头照转 |
| `crates/miyu-web/src/open.rs`、`texts.rs` | `open`：确保 `serve` 在跑；要一次性码；开浏览器；给人看的字 |
| `crates/miyu-web/src/settings.rs`、`resources/web/web.json` | 出厂的端口（8300）、空闲多久、内容安全策略、页面的媒体类型 |
| `resources/web/pages/` | 页面文件。M9 的网页搬进主仓库以前是空的，开发时设 `MIYU_WEB_PAGES` 指到网页演示的 `web-demo/` |
| `crates/miyu-cli/src/web.rs`、`help/{zh,en}/web.txt` | 主程序的 `miyu web` 和它的帮助页 |
| `crates/miyu-ipc/src/start.rs` 的 `spawn_detached` | 拉起、跟终端脱开、等那一行：核心和 `serve` 共用 |

### 对外的样子

见 `web-module.md`「网页软件对外的样子」：命令、HTTP、数据根里多的文件（`run/web.lock`、`run/web`）。运行日志 `state/logs/web.log`，目标 `miyu::web`。

### 怎么走

**一、起停、端口、页面、WebSocket**（W-9，原来是 `web-module.md` 第九条）

1. 单实例：`miyu-web serve` 先拿 `run/web.lock`，拿不到写 `running` 走。拿到了听端口，把地址写进 `run/web`（先写临时文件再改名），往标准输出写一行 `ready`，和核心那一行同一个写法（`ipc.md`「那一行」，复用 `miyu-ipc` 的 `Ready`）。端口被占了写 `error port <端口> in use`（`open` 认这个写法，照人的语言说，「施工时定的」第 5 条），别的起不来写 `error <原因>`。
2. 端口：照 `--port`，没写照 `resources/web/web.json` 的出厂值（固定端口，第 2 题）；`0` 是系统挑一个空的。只听回环地址 `127.0.0.1`，不听别的网卡。端口被占了：`miyu web` 照人的语言说哪个端口被占了、怎么换。出厂端口 8300（「施工时定的」第 1 条）。
3. 空闲退出：没有 WebSocket 连着、没有 `/media` 在给，连续 10 分钟就退出（`web.json` 的出厂值），先删 `run/web`、再放锁。收到停的信号照样先删再放。
4. 每个请求先核对 Host：只认 `127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>`，别的回 403。别的网站把自己的域名解析到回环地址也进不来（DNS rebinding）。
5. 页面文件：`GET /` 给 `index.html`，别的照路径在页面目录里找。带 `..` 的、换成真实位置以后跑到页面目录外的、不是普通文件的，404。类型照扩展名（`web.json` 的表）。响应头一律带：`X-Content-Type-Options: nosniff`、`Referrer-Policy: no-referrer`、`Cache-Control: no-cache`、`Content-Security-Policy`（照 `web.json`，至少有 `connect-src 'self'`、`frame-ancestors 'none'`）。从来不设 cookie（「起草时定的」第 13 条）。
6. 页面目录：`MIYU_WEB_PAGES` 设了照它，不然是资源目录下的 `web/pages/`。资源目录照 `store/resources.md` 第 1 条找，和核心同一个办法。
7. `GET /ws`：Origin 要正好是 `http://` 加上第 4 款三种之一（带端口），不然 403。接了以后连核心：`connect_or_start_bare`，核心没在跑就拉起来（命令是主程序 `miyu` 加 `core`，主程序在 `miyu-web` 的真实位置旁边）。连不上：往 WebSocket 发一条通知 `{"jsonrpc":"2.0","method":"web.error","params":{"message":<原因>}}`，再关。
8. 一个标签页一条核心连接，不合并（proto/web-demo 分支 `docs/blueprint/web/architecture.md`「多用户、多终端」第 7 条）。两头照转：文字帧加一个 `\n` 是一行，一行去掉 `\n` 是一个文字帧。不读、不改、不加：握手的凭据、命令、推送原样过去。二进制帧：关，1003。一帧超过 1 MiB：关，1009（核心那头一行也就这么长）。
9. 一头断了另一头跟着关。核心那头断了（重启、退出）：WebSocket 关，1012，页面照自己的规矩重连（proto/web-demo 分支 `docs/blueprint/web.md`「连核心」第 1 条）。网页软件这头关的（1003、1009、1012、连不上核心）：发完关闭帧先关写的一半，把浏览器还在发的读掉、扔掉，读到头或者满 2 秒再放套接字（「施工时定的」第 10 条）。
10. 运行日志 `state/logs/web.log`，满了照核心的换法（`log.md`）。只记连上、断开、出错，不记一行的内容、一次性码、密码、登录令牌、票据。

**二、`miyu web`**（W-9，原来是 `web-module.md` 第十一条，2026-10-04 随 W-8 改过）

1. 主程序的 `miyu web` 找主程序真实位置旁边的 `miyu-web`（有了软件包的清单以后照清单找，M9）（Windows 上是 `miyu-web.exe`），把 `web` 后面的参数原样交给 `miyu-web open`，等它退出，退出码照它的。没有：印「没装网页界面」和每种装法怎么装，退出码 1。
2. `miyu-web open`：`run/web` 在、锁有人拿着，网页软件就在跑，照 `run/web` 的地址。不然拉起 `miyu-web serve`：和头拉起核心一样（`ipc.md`「连不上就拉起」第 4、5 条），跟终端脱开，工作目录是数据根，等那一行最多 10 秒。
3. 照终端的样子连核心（`connect_or_start`，出示本机令牌）。写了 `--reset` 的，或者还没设过密码的（问一次 `account.setup_code`，`first` 是真的）：网址是 `<地址>/#setup=<一次性码>`，码在 `#` 后面，不发给服务器、不进 Referer，页面拿到以后从地址栏抹掉（设计 21 X6）。别的：网址就是 `<地址>/`，页面用存着的登录令牌，没有、过期了的问用户名和密码；这时不要一次性码（要了不用，5 分钟后自己作废）。
4. 用系统的办法打开网址：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" "<网址>"`。一次性码会出现在进程列表里：它只用一次、5 分钟，用过就作废（第 3 题），不另开跳转页。
5. 交给了浏览器：印网页的地址（不带码）；带了码的再印一句「浏览器没打开的话，用 miyu web --print」（第 3 题说的 snap 装的 Firefox），退出码 0。浏览器开没开、设没设好，`miyu web` 看不到。交不出去（没有 `xdg-open`、没有图形界面）：照 `--print` 办。
6. `--print`：不开浏览器，印整个网址；带了码的，下一行提醒「5 分钟内有效，只能用一次，别发给别人」。
7. `--logout`：照终端的样子连核心，`account.logout`，`all: true`，印作废了几个。不碰网页软件，不改密码。

### 出错、运行日志

| 级别 | 这件事 | 什么时候 |
|---|---|---|
| `INFO` | `listening url=…`、`stopped reason=…` | 起来、退出（`idle`、`signal`） |
| `WARN` | `rejected host=… origin=…` | Host、Origin 不对 |
| `WARN` | `core unreachable error=…` | 连不上核心 |
| `DEBUG` | `websocket connected`、`websocket closed` | 一个标签页连上、断开 |

### 给人看的字

`miyu web` 印的，照核心握手回的 `language`（中文、英文）：见 `web-module.md`「给人看的字」第二张表（`crates/miyu-web/src/texts.rs`）。网址印在标准输出上，别的话印在标准错误上。

### 守着它的

| 测试 | 守着什么 |
|---|---|
| `crates/miyu-web/tests/serve.rs` | 单实例、`run/web`、那一行；端口被占说清楚；Host 只认三种写法；页面文件不出页面目录（`..`、`%2e%2e`、链接、目录）；响应头一个不少、从不设 cookie；`HEAD`、别的方法 405；空闲到点退出、删 `run/web`、放锁 |
| `crates/miyu-web/tests/ws.rs` | 一帧一行两头照转，一个字节不改（凭据、中文、空白、很长的一行）；Origin 不对 403；二进制 1003、超过 1 MiB 1009，照原始字节发的超长帧读得到 1009、读到头不是被重置；核心断了 1012；连不上核心发 `web.error` 再关；转发的代码里不读本机令牌（照源码查） |
| `crates/miyu-web/tests/open.rs` | 拉起真的 `miyu-web serve`；没设过密码、`--reset` 的带 `#setup=`，别的不带；`--print`、交不给浏览器的印网址和提醒；`--logout`；端口被占照人的语言说 |
| `crates/miyu-cli/src/web/tests.rs` | 参数照原样交给 `miyu-web open`；没装时说怎么装、退出码 1；装了的照它的退出码 |

### 施工时定的（施工 W-9，2026-10-04）

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 出厂端口 8300（项目主人定：Miyu 的生日是 8 月 30 日） | 好记，和角色对得上 | 8765（网页演示的桥一直用的） |
| 2 | `open` 拉起 `serve` 照拉起核心的办法：`miyu-ipc` 把「拉起、跟终端脱开、等那一行」开放出来（`spawn_detached`） | 同一套脱开、收尸、读一行的代码，三个平台的坑只踩一次 | `miyu-web` 自己再写一份 |
| 3 | `miyu-web` 拉起核心用自己真实位置旁边的 `miyu`，和 `miyu web` 找它是一个办法 | 两个程序装在一处（同一个发行包的规矩，W-11 打包时检查） | 照 `PATH` 找（装了两份时找错） |
| 4 | 测试里核心用 `miyu-ipc` 的监听当替身 | 网页软件只认本机传输，测试不该为了替身反过来依赖核心；真核心在真机实测时走 | 测试依赖 `miyu-core`（层序反过来） |
| 5 | 端口被占时 `serve` 写 `error port <端口> in use`，`open` 认这个写法、照人的语言说 | `serve` 起来时还没连核心，不知道人的语言；那一行是两个程序之间的话 | `serve` 照系统语言写中文（`open` 原样印，英文的人看到中文） |
| 6 | 往浏览器写的都经一个写的任务（通道），读核心、读浏览器各在一处 | 两头都可能要关 WebSocket（1003、1009、1012）；读一行不能在 `select!` 里被打断（读到一半丢字） | 两头抢着写一个 sink |
| 7 | 只数 WebSocket 算忙，要页面不算 | 页面一次就拿完；开着的标签页总有一条 WebSocket | 每个 HTTP 请求都续一次（一个探活的脚本就能让它不退） |
| 8 | `cli/web.md` 不另开，`miyu web` 写在这一页 | `miyu web` 只是找程序、交参数，怎么走都在 `miyu-web open` | 另开一页（两页说同一件事） |
| 9 | 资源目录最上一层的 `web/`（`web.json`，以后的页面）不进提示词登记簿（`xtask/src/ledger.rs` 豁免，和 `models/` 一样） | 给浏览器的，不发给模型 | 登记进 26 第十节（登记簿里混进不发给模型的东西） |
| 10 | 关了以后先关写的一半、把浏览器还在发的读掉再放套接字，最多等 2 秒（W-9 验收时补，2026-10-04） | 带着没读的数据关，系统回 RST，刚写出去的关闭帧可能被对面丢掉：超过 1 MiB 的帧正文没读，Windows 上浏览器收不到 1009、只看到连接被重置（CI 上稳定复现）；读掉再关就是正常的 FIN | 只改测试让客户端边发边读（真浏览器也是边发边读，但 RST 和关闭帧照样赛跑）；把那一帧读完再关（帧可能很大，要另加上限） |
