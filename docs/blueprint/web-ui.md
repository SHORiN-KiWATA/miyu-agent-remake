## 网页软件 `miyu-web`

### 是什么

网页界面是一个单独的程序 `miyu-web` 加一套页面文件，装了才有。它是核心的一个头：自己开一个只听本机的 HTTP 端口，给页面，把浏览器的 WebSocket 一帧一条转成核心协议的一行一条，经本机套接字（Windows 上是命名管道）连核心，不读本机令牌。主程序的 `miyu web` 找到它、把参数交给它。

状态：施工 W-9 做好了起停、页面、WebSocket 照转和 `miyu web`；施工 W-10 做好了媒体地址 `/media`；打包随 W-11。这一页从 `web-module.md` 搬出来（第九条、第十一条，W-10 又搬了第十条；「要跟着改的别的页」里定的「网页软件一页」）；核心给网页的通用方法、身份还在那一页。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-web/`（第 5 层，头） | 程序 `miyu-web`：只依赖 `miyu-ipc`、`miyu-store`、`miyu-log`、`miyu-webserve`，不依赖核心 |
| `crates/miyu-web/src/main.rs` | 子命令 `open`、`serve`；找自己旁边的主程序拉起核心 |
| `crates/miyu-web/src/serve.rs` | 单实例、听端口、写 `run/web` 和那一行、空闲退出；每个请求分给谁：Host 不对的记一行、回 403，`/ws`、`/media`、页面文件。网页软件的 `Site` 实现 `miyu_webserve::Site`（数忙照空闲退出） |
| `crates/miyu-webserve/src/lib.rs`（施工 O-16 搬过去，`webserve.md`「搬家表」） | 核对 Host、Origin 的三种写法（`Site::hosts`、`host_allowed`） |
| `crates/miyu-webserve/src/pages.rs`（同上，原来是 `crates/miyu-web/src/pages.rs` 和 `serve.rs` 的一段） | 页面文件：`/` 是 `index.html`，不出页面目录；类型照 `web.json` 的表、四个响应头 |
| `crates/miyu-webserve/src/ws.rs`（同上，原来是 `crates/miyu-web/src/ws.rs`） | 核对 Origin；WebSocket 和核心连接两头照转 |
| `crates/miyu-webserve/src/respond.rs`（同上，原来在 `crates/miyu-web/src/serve.rs`） | 回应的正文、`nosniff`、`no-referrer` |
| `crates/miyu-web/src/media.rs` | `/media`：换票据、照票据一块块给、响应头（施工 W-10） |
| `crates/miyu-web/src/media/tickets.rs` | 票据：造、找、作废、过期、上限 |
| `crates/miyu-web/src/media/link.rs` | 照登录令牌连核心：一个令牌一条，同时问、照编号分回去，60 秒没人用就关 |
| `crates/miyu-web/src/media/range.rs` | `Range` 要哪一段；下载的名字照 RFC 5987 转义 |
| `crates/miyu-web/src/backstage.rs` | 软件后台页：`/page` 换票据、`/p/<票据>/<包>/<路径>` 照 `package.file` 一块块给、响应头（施工 F-6 下）；票据和连着的核心同 `/media` 那一套（`media/tickets.rs` 的 `Tickets` 两边各一份，`media/link.rs` 的 `Cores` 共用） |
| `crates/miyu-web/src/open.rs`、`texts.rs` | `open`：确保 `serve` 在跑；要一次性码；开浏览器；给人看的字。照终端的样子连核心一问一答（`open` 要一次性码、`serve` 起来时问配置）、开浏览器的那两样在 `crates/miyu-webserve/src/open.rs`（施工 9-1 下从 `open.rs` 挪进 `client.rs`，施工 O-16 搬过去） |
| `crates/miyu-web/src/settings.rs`、`resources/web/web.json`、`resources/packages/web.toml` | 端口（出厂 8300）、空闲多久、票据多久不用作废、最多几张是配置项（`web.port`、`web.idle_seconds`、`web.ticket_idle_seconds`、`web.most_tickets`），声明在网页自己的清单里，默认值照清单读，起来时问核心拿最终值（施工 9-1 下）；内容安全策略（页面的、软件后台页的 `backstage_csp`）、页面的媒体类型是常量，在 `web.json` |
| `resources/web/pages/` | 页面文件。M9 的网页搬进主仓库以前是空的，开发时设 `MIYU_WEB_PAGES` 指到网页演示的 `web-demo/` |
| `crates/miyu-cli/src/web.rs`、`help/{zh,en}/web.txt` | 主程序的 `miyu web` 和它的帮助页 |
| `crates/miyu-ipc/src/start.rs` 的 `spawn_detached` | 拉起、跟终端脱开、等那一行：核心和 `serve` 共用 |

### 对外的样子

见 `web-module.md`「网页软件对外的样子」：命令、HTTP、数据根里多的文件（`run/web.lock`、`run/web`）。运行日志 `state/logs/web.log`，目标 `miyu::web`。

### 怎么走

**一、起停、端口、页面、WebSocket**（W-9，原来是 `web-module.md` 第九条）

1. 单实例：`miyu-web serve` 先拿 `run/web.lock`，拿不到写 `running` 走。拿到了听端口，把地址写进 `run/web`（先写临时文件再改名），往标准输出写一行 `ready`，和核心那一行同一个写法（`ipc.md`「那一行」，复用 `miyu-ipc` 的 `Ready`）。端口被占了写 `error port <端口> in use`（`open` 认这个写法，照人的语言说，「施工时定的」第 5 条），别的起不来写 `error <原因>`。
2. 端口：照 `--port`，没写照配置 `web.port`（施工 9-1 下：只能写在系统配置，设置页「软件包」那一页露；改了下次起网页界面时生效）。`serve` 起来时核心在跑的就问它 `config.get` 拿 `web.*` 四项的最终值，不为这个拉起核心：`miyu web` 先经 `open` 连上核心才拉起 `serve`；核心没在跑的照清单的默认值起，记一行 `INFO web config from defaults`，连上了却被拒的记 `WARN web config not read`（固定端口，第 2 题）；`0` 是系统挑一个空的。只听回环地址 `127.0.0.1`，不听别的网卡。端口被占了：`miyu web` 照人的语言说哪个端口被占了、怎么换。出厂端口 8300（「施工时定的」第 1 条）。
3. 空闲退出：没有 WebSocket 连着、没有 `/media`、软件后台页在给，连续 10 分钟就退出（配置 `web.idle_seconds`，出厂 600，设置页不露），先删 `run/web`、再放锁。收到停的信号照样先删再放。
4. 每个请求先核对 Host：只认 `127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>`，别的回 403。别的网站把自己的域名解析到回环地址也进不来（DNS rebinding）。
5. 页面文件：`GET /` 给 `index.html`，别的照路径在页面目录里找。带 `..` 的、换成真实位置以后跑到页面目录外的、不是普通文件的，404。类型照扩展名（`web.json` 的表）。响应头一律带：`X-Content-Type-Options: nosniff`、`Referrer-Policy: no-referrer`、`Cache-Control: no-cache`、`Content-Security-Policy`（照 `web.json`，至少有 `connect-src 'self'`、`frame-ancestors 'none'`）。从来不设 cookie（「起草时定的」第 13 条）。
6. 页面目录：`MIYU_WEB_PAGES` 设了照它，不然是资源目录下的 `web/pages/`。资源目录照 `store/resources.md` 第 1 条找，和核心同一个办法。
7. `GET /ws`：Origin 要正好是 `http://` 加上第 4 款三种之一（带端口），不然 403。接了以后连核心：`connect_or_start_bare`，核心没在跑就拉起来（命令是主程序 `miyu` 加 `core`，主程序在 `miyu-web` 的真实位置旁边）。连不上：往 WebSocket 发一条通知 `{"jsonrpc":"2.0","method":"web.error","params":{"message":<原因>}}`，再关。
8. 一个标签页一条核心连接，不合并（proto/web-demo 分支 `docs/blueprint/web/architecture.md`「多用户、多终端」第 7 条）。两头照转：文字帧加一个 `\n` 是一行，一行去掉 `\n` 是一个文字帧。不读、不改、不加：握手的凭据、命令、推送原样过去。二进制帧：关，1003。一帧超过 1 MiB：关，1009（核心那头一行也就这么长）。
9. 一头断了另一头跟着关。核心那头断了（重启、退出）：WebSocket 关，1012，页面照自己的规矩重连（proto/web-demo 分支 `docs/blueprint/web.md`「连核心」第 1 条）。网页软件这头关的（1003、1009、1012、连不上核心）：发完关闭帧先关写的一半，把浏览器还在发的读掉、扔掉，读到头或者满 2 秒再放套接字（「施工时定的」第 10 条）。
10. 运行日志 `state/logs/web.log`，满了照核心的换法（`log.md`）。只记连上、断开、出错，不记一行的内容、一次性码、密码、登录令牌、票据。

**二、`miyu web`**（W-9，原来是 `web-module.md` 第十一条，2026-10-04 随 W-8 改过）

1. 主程序的 `miyu web` 照清单找网页软件（施工 9-3：两层清单里子命令是 `web` 的那一份的程序，出厂是 `miyu-web`；清单里找不到的照 `miyu-web`），只找主程序真实位置旁边的（Windows 上加 `.exe`），把 `web` 后面的参数原样交给 `miyu-web open`，等它退出，退出码照它的。没有：印「没装网页界面」和每种装法怎么装，退出码 1。
2. `miyu-web open`：`run/web` 在、锁有人拿着，网页软件就在跑，照 `run/web` 的地址。不然拉起 `miyu-web serve`：和头拉起核心一样（`ipc.md`「连不上就拉起」第 4、5 条），跟终端脱开，工作目录是数据根，等那一行最多 10 秒。
3. 照终端的样子连核心（`connect_or_start`，出示本机令牌）。写了 `--reset` 的，或者还没设过密码的（问一次 `account.setup_code`，`first` 是真的）：网址是 `<地址>/#setup=<一次性码>`，码在 `#` 后面，不发给服务器、不进 Referer，页面拿到以后从地址栏抹掉（设计 21 X6）。别的：网址就是 `<地址>/`，页面用存着的登录令牌，没有、过期了的问用户名和密码；这时不要一次性码（要了不用，5 分钟后自己作废）。
4. 用系统的办法打开网址：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" "<网址>"`。一次性码会出现在进程列表里：它只用一次、5 分钟，用过就作废（第 3 题），不另开跳转页。
5. 交给了浏览器：印网页的地址（不带码）；带了码的再印一句「浏览器没打开的话，用 miyu web --print」（第 3 题说的 snap 装的 Firefox），退出码 0。浏览器开没开、设没设好，`miyu web` 看不到。交不出去（没有 `xdg-open`、没有图形界面）：照 `--print` 办。
6. `--print`：不开浏览器，印整个网址；带了码的，下一行提醒「5 分钟内有效，只能用一次，别发给别人」。
7. `--logout`：照终端的样子连核心，`account.logout`，`all: true`，印作废了几个。不碰网页软件，不改密码。
8. `--package <编号>`（施工 F-6 下，`package-pages.md`「终端」第 2 条）：网址 `#` 后面多一个 `package=<编号>`，和一次性码一起的写成 `#setup=<码>&package=<编号>`；页面起来就打开设置、到「软件后台」里这个软件的页面（页面怎么画见网页的蓝图）。编号只认字母、数字、`-`、`_`、`.`，1 到 64 个，别的照用法不对（退出码 2）。

**三、媒体地址**（W-10，原来是 `web-module.md` 第十条；2026-09-30 定的「小的经协议，大的由网页给带令牌的地址」，那时说的网页模块现在是网页软件）

1. `POST /media`：`Authorization: Bearer <登录令牌>`；正文是 JSON：`blob`（内容哈希）或者 `path`（绝对路径），正好一个；可以带 `type`（媒体类型）、`name`（存下来叫什么）、`download`（布尔，叫浏览器存下来）。
2. 网页软件照这个登录令牌连核心：同一个令牌的连接留着复用，60 秒不用就关。握手被拒（`bad_login`）回 401。
3. 先问核心有没有、能不能读：`blob.get` 或者 `fs.read`，`length` 写 0。`unknown_blob`、`path_unreadable` 回 404，`path_forbidden` 回 403。
4. 造一张票据：32 个随机字节，64 位小写十六进制。记在内存里：哪个登录令牌、哪个资源、多大、`type`、`name`、`download`。同一个令牌、同一个资源、同样三格的，交回原来那一张。12 小时没用过的作废；最多 4096 张，多了丢最久没用的。网页软件重启，票据全作废，页面照 404 重新换。
5. 回应 `{"url":"/media/<票据>"}`。
6. `GET /media/<票据>`：不认识的 404。带 `Range: bytes=…` 的只认一段，回 206；超出的回 416；不带的回全部。照 `blob.get`、`fs.read` 一块 512 KiB 地读，读一块写一块，不整个读进内存。
7. 类型：`type` 在 `web.json` 的 blob 类型表里的照它；`path` 的照扩展名查表；都没有的 `application/octet-stream`。响应头带 `nosniff`、`Cache-Control: private, no-cache`、`Content-Security-Policy: sandbox; default-src 'none'; img-src data:; media-src data:; style-src 'unsafe-inline'`：有人直接打开这个地址（一个 SVG、一个 HTML），它在一个空的来源里跑，碰不到页面。`download` 的加 `Content-Disposition: attachment`，名字照 `name`（只留最后一段），UTF-8 照 RFC 5987 转义。
8. 链接卡片的图、附件、她写到的本机图片和音视频，都走这一条。网页软件不另开图片代理：抓网上东西的只有核心的 `net` 包，地址闸只有一处。
9. 有 `/media` 在给，网页软件不算空闲。
10. 施工 W-10 定的细处见「施工时定的」第 11 到 16 条：「blob 类型表」就是 `types` 那张表的值；一个令牌一条核心连接、同时问；`Range` 只认一段，好几段、写法不对的照没写；`GET` 时照这时的大小算；令牌作废了票据一起作废；方法只认 `POST`、`GET`。没写名字的下载，本机文件照文件名，blob 只写 `attachment`。

**四、软件后台页**（施工 F-6 下；`package-pages.md`「网页软件怎么给后台页」，设计 30 第十三节）

1. `POST /page`：`Authorization: Bearer <登录令牌>`；正文是 JSON `{"package"}`，包的编号的写法同第二条第 8 款，不对的 400。
2. 照这个登录令牌连核心（和 `/media` 共用连着的那一条），问 `package.list`：这个包那一项 `page` 是真的才给，不然 404。握手被拒 401。
3. 造一张票据，写法、多久作废、最多几张同第三条第 4 款（配置 `web.ticket_idle_seconds`、`web.most_tickets`），和 `/media` 的各是各的一份：同一个令牌、同一个包的交回原来那一张。回应 `{"url":"/p/<票据>/<包的编号>/"}`。框的地址里只有票据、不带登录令牌：框里的页面读得到自己的地址。
4. `GET /p/<票据>/<包>/<路径>`：票据不认识、包和票据对不上的 404。路径解开 `%xx`（解出来不是 UTF-8、有 NUL 的 404），以 `/` 结尾的补上 `index.html`，空的照核心认成 `index.html`；带 `..`、绝对路径的照原样交给核心，核心拒 `bad_params`（400）。
5. 照 `package.file` 一块 512 KiB 地读：先读第一块拿到整份多大、写响应头，再一块块读、一块块写。核心拒的：`not_found`、`no_page`、`unknown_package` 404，`bad_params` 400，别的 502；握手被拒 401，这个令牌的后台页票据一起作废。给到一半核心那头断了、给得比说的少：连接照 HTTP 的规矩断掉。不认 `Range`：页面文件都小，一次给完。
6. 类型照扩展名查 `web.json` 的 `types`（空路径照 `index.html`），没有的 `application/octet-stream`。响应头：`Content-Security-Policy` 照 `web.json` 的 `backstage_csp`（`sandbox allow-scripts allow-forms; default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'none'; frame-ancestors 'self'`：沙箱、只取它自己的文件、不许联网、只许嵌在网页里），`X-Content-Type-Options: nosniff`、`Referrer-Policy: no-referrer`、`Cache-Control: no-cache`，`Access-Control-Allow-Origin: *`。
7. 页面把它放在 `<iframe sandbox="allow-scripts allow-forms">` 里，不给 `allow-same-origin`：页面在一个空的来源里跑，读不到网页的存储、登录令牌。空来源的框取 ES 模块脚本、字体是按跨源取的，所以要第 6 款的 `Access-Control-Allow-Origin`；票据就是凭据，不多开口子。`'self'` 在空来源的框里照文件的地址算，对得上网页软件自己（网页演示的桥 2026-10-10 在 Chromium 里实测）。框和网页之间的通道由页面做（网页的蓝图「软件后台」）。
8. 有后台页在给，网页软件不算空闲。方法只认 `POST /page`、`GET /p/…`，别的 405。

### 出错、运行日志

| 级别 | 这件事 | 什么时候 |
|---|---|---|
| `INFO` | `listening url=…`、`stopped reason=…` | 起来、退出（`idle`、`signal`） |
| `WARN` | `rejected host=… origin=…` | Host、Origin 不对 |
| `WARN` | `core unreachable error=…` | 连不上核心 |
| `DEBUG` | `websocket connected`、`websocket closed` | 一个标签页连上、断开 |
| `WARN` | `media cut short offset=… error=…` | `/media` 给到一半核心那头断了、给得比说的少：连接照 HTTP 的规矩断掉，浏览器知道没收全 |
| `WARN` | `no random bytes for a ticket` | 票据造不成，回 500 |
| `WARN` | `page cut short offset=… error=…` | 软件后台页给到一半核心那头断了、给得比说的少：连接照 HTTP 的规矩断掉 |

`/media` 的状态码见 `web-module.md`「出错」网页软件的 HTTP 那张表。

### 给人看的字

`miyu web` 印的，照核心握手回的 `language`（中文、英文）：见 `web-module.md`「给人看的字」第二张表（`crates/miyu-web/src/texts.rs`）。网址印在标准输出上，别的话印在标准错误上。

### 守着它的

| 测试 | 守着什么 |
|---|---|
| `crates/miyu-web/tests/serve.rs` | 单实例、`run/web`、那一行；端口被占说清楚；Host 只认三种写法；页面文件不出页面目录（`..`、`%2e%2e`、链接、目录）；响应头一个不少、从不设 cookie；`HEAD`、别的方法 405；空闲到点退出、删 `run/web`、放锁 |
| `crates/miyu-web/tests/ws.rs` | 一帧一行两头照转，一个字节不改（凭据、中文、空白、很长的一行）；Origin 不对 403；二进制 1003、超过 1 MiB 1009，照原始字节发的超长帧读得到 1009、读到头不是被重置；核心断了 1012；连不上核心发 `web.error` 再关；转发的代码里不读本机令牌（照源码查，施工 O-16 起连同搬进 `miyu-webserve` 的那几份） |
| `crates/miyu-web/tests/media.rs` | 核心用替身（照登录令牌握手、答 `blob.get`、`fs.read`，同一条连接上乱序答）：换票据要登录令牌（没带、带错 401）；正文 `blob`、`path` 正好一个，不对 400；没有的 404、数据根里的 403、连不上核心 502；同一个令牌、资源、三格交回同一张，连接复用；全部、`Range` 206 和 `Content-Range`、超出 416、好几段和写法不对的照没写；一块不超过 512 KiB、拼起来一个字节不差；同一条连接上同时几问各拿各的；类型照表、表里没有的不认、`sandbox`、`nosniff`、`private, no-cache`；下载的名字；令牌作废了 401、票据一起作废；在给不算空闲；票据过期、满了丢最久没用的 |
| `crates/miyu-web/tests/backstage.rs` | 核心用替身（多答 `package.list`、`package.file`）：换票据要登录令牌（没带、带错、不是 Bearer 401）；正文不对 400；没有后台页、没有这个包 404；同一个令牌、同一个包交回同一张；页面文件和响应头（类型、`backstage_csp`、`nosniff`、`no-referrer`、`no-cache`、`Access-Control-Allow-Origin: *`、不设 cookie）；以 `/` 结尾补 `index.html`；没有的 404、核心拒 `bad_params` 400；票据只开它的包、不认识的 404；大文件一块块问、拼起来一个字节不差；令牌作废了 401、票据一起作废；方法 405 |
| `crates/miyu-web/src/backstage/tests.rs` | 地址拆成票据、包、文件（`%xx`、补 `index.html`、NUL、不是 UTF-8、包编号写法）；包编号的写法；核心拒的换成状态码；类型照扩展名 |
| `crates/miyu-web/src/media/tests.rs` | `Range` 每种写法（大小写、超出、好几段、写法不对、空的资源）；下载的名字只留最后一段、`attr-char` 以外都转义 |
| `crates/miyu-web/tests/open.rs` | 拉起真的 `miyu-web serve`；没设过密码、`--reset` 的带 `#setup=`，别的不带；`--package` 的带 `package=`，和一次性码一起是 `#setup=…&package=…`；`--print`、交不给浏览器的印网址和提醒；`--logout`；端口被占照人的语言说 |
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

### 施工时定的（施工 W-10，2026-10-04）

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 11 | 第三条第 7 款说的「blob 类型表」就是 `web.json` 里 `types` 那张表的值：`type` 是表里出现过的才照它，别的不认、照没写 | 一张表两头用，不会一边加了一边忘 | `web.json` 另开一张允许的媒体类型表 |
| 12 | 连核心：一个登录令牌一条连接，几个请求在同一条上同时问，照编号把回应分回去；60 秒没人用就关；用着的断了（核心重启、令牌作废了核心断开）重连再问一次 | 一个视频拖进度会同时来好几个分段请求，一问一答会排队 | 每个请求单开一条连接（每次都要握手） |
| 13 | `Range` 只认一段（`bytes=a-b`、`bytes=a-`、`bytes=-n`）；好几段的、写法不对的照没写，回全部（RFC 9110 允许不理）；超出的 416 带 `Content-Range: bytes */<大小>`；回应都带 `Accept-Ranges: bytes` | 浏览器放音视频只发一段；好几段要拼 multipart，没人用 | 好几段的回 416 |
| 14 | `GET` 时再问一次大小，照这时的大小算 `Range`、`Content-Length`；给得比说的少就断开连接 | 票据活 12 小时，文件可能变了 | 照换票据时记下的大小 |
| 15 | 握手被拒（令牌作废了、过期了）：`POST`、`GET` 都回 401，这个令牌的票据一起作废 | 退出登录、`miyu web --logout` 以后，旧票据不该还能拿到东西 | 票据活到 12 小时 |
| 16 | 票据多久不用、最多几张写进 `web.json`（`ticket_idle_seconds`、`most_tickets`），连核心的 60 秒写在代码里；施工 9-1 下起这两项挪成配置项 `web.ticket_idle_seconds`、`web.most_tickets`（设置页不露） | 「起草时定的」第 25 条：网页软件的数放在 `web.json`；60 秒只是省一条连接，不是给人调的 | 都写进 `web.json` |

### 施工时定的（施工 F-6 下，2026-10-10）

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 17 | 票据的类型 `Tickets` 照要的东西做成泛型，`/media`、软件后台页各一份；连着的核心（`Cores`）共用 | 造、找、作废、过期、上限一份代码；两边的票据互相开不了对方的东西 | 一份票据表加一种来源（`/media/<票据>` 要另外挡掉后台页的票据） |
| 18 | 换票据时问一次 `package.list` 查有没有后台页，没有的 404 | 不给没后台页的包造票据，页面马上知道打不开 | 照给文件时 `package.file` 的 `no_page` 才知道 |
| 19 | `/p/` 不认 `Range`，第一块读到了才写响应头（`Content-Length` 照整份） | 页面文件都小；先读一块才知道有没有这份、多大 | 先问一次大小再读（`package.file` 没有只问大小的写法，要多读一块） |
| 20 | `Access-Control-Allow-Origin: *` | 不给 `allow-same-origin` 的框来源是空的，ES 模块脚本、字体照跨源取，不给取不到；票据就是凭据 | 写 `null`（空来源都是 `null`，口子一样大） |
| 21 | `miyu web --package <编号>` 照包编号的写法查（`backstage::valid_package`），网址里 `package=` 跟在一次性码后面 | 编号放进网址不用转义；页面照 `#` 后面的几项读 | 不查原样带（写错的到了页面才发现） |
| 22 | `decode`（解开 `%xx`）从 `miyu-webserve` 的页面文件那里开放出来用 | 同一套规矩（不是 UTF-8、有 NUL 的不要）只写一处 | 网页软件里再写一份 |

