## 网页界面和核心给它的通用方法

### 是什么

网页界面是一个单独的软件：程序 `miyu-web` 加一套页面文件，装了才有，不装就没有。它和终端界面一样是核心的一个头：自己开一个只听本机的 HTTP 端口，给页面，把浏览器的 WebSocket 一帧一条转成核心协议的一行一条（`protocol.md`），经本机套接字（Windows 上是命名管道）连核心。

核心里没有为网页写的代码。网页要的、终端也用得上的几样，核心做成通用的协议方法，哪个头都能调：给人看的字、列文件和找文件、路径、mermaid 画成 SVG、分块传和分块读、链接卡片。身份照旧由核心验：每条连接在握手时出示凭据，核心自己认，网页软件只转发。

网页演示（proto/web-demo 分支的 `web-demo/`）现在靠一个桥顶着这些活（proto/web-demo 分支的 `docs/blueprint/web.md`「和设计 21 的出入」）。这条线做完，桥整个删掉：

| 桥现在干的 | 以后谁干 | 协议上叫什么 | 步 |
|---|---|---|---|
| 1. 页面文件、WebSocket、访问口令、核对 Origin | 网页软件；身份由核心验：第一次用一次性码建管理员账号，以后用户名和密码登录 | 随用户系统定 | W-8、W-9 |
| 2. `web.human` 给人看的字 | 核心 | `human.get` | W-1 |
| 3. `web.info`、`web.realpath` | 核心 | 握手回应的 `host`，`fs.realpath` | W-3 |
| 4. `/file`、`/blob` | 内容由核心给，地址由网页软件给 | `blob.get`、`fs.read`；网页软件的 `/media` | W-6、W-10 |
| 5. `/upload`、`web.upload_done` | 页面经核心分块传，网页软件不经手 | `blob.open`、`blob.write`、`blob.close` | W-5 |
| 6. `web.link_preview`、`/link-image` | 核心的 `net` 包；卡片的图存成 blob，经 `/media` 给 | `link.preview` | W-7、W-10 |
| 7. `web.mermaid` | 核心的 `mermaid` 包 | `mermaid.render` | W-4 |
| 8. `web.files` | 核心 | `fs.list`、`fs.find` | W-2 |
| 9. 核心没在跑时拉起它 | 网页软件，照别的头 | 无 | W-9 |

状态：图纸，2026-10-01 项目主人批准。「网页界面是一个软件，网页的东西不放进核心」是项目主人 2026-10-01 定的，重开了设计 04 的 P5（末尾「要改的设计」）；「画 mermaid 在核心里，做成可选的软件包」也是同一天项目主人定的。技术细节照推荐定了，写在末尾「起草时定的」；项目主人拍板的六题单列一节。施工步子 W-1 到 W-11，W 是和 M8 并行的一条线，不占里程碑的号。W-1 做好了：`human.get`（`crates/miyu-endpoint/src/human.rs`、`crates/miyu-store/src/human.rs` 交出模板原文，`crates/miyu-endpoint/tests/human.rs`）。

- W-1 到 W-7（核心的通用方法）现在就做，和 M8 并行。
- W-8 到 W-11（身份、网页软件、媒体地址、打包）等用户系统：项目主人要的是第一次用一次性码进网页、建管理员账号，码当场作废，以后用用户名和密码登录（第 1、3 题）。用户系统照约定 M8 做完以后专门过一遍（「多用户、多终端」那次讨论），这几步的细节那时重画。这一页第一条、第十一条和 W-8 那几行写的是起草时的样子，只当参考；第九条、第十条的大部分不受影响。
- 在那之前网页照旧用桥，桥的活随 W-1 到 W-7 一样样挪进核心。

### 在哪

施工时照这个放。

**核心和主程序这边**：

| 代码 | 管什么 | 步 |
|---|---|---|
| `crates/miyu-endpoint/src/hello.rs` | 握手多认 `code`、`login`；回应多 `host`、`login` | W-3、W-8 |
| `crates/miyu-endpoint/src/human.rs` | `human.get` | W-1 |
| `crates/miyu-endpoint/src/files.rs`、`files/` | `fs.list`、`fs.find`、`fs.realpath`、`fs.read` 的参数和回应；找文件的清单记几份 | W-2、W-3、W-6 |
| `crates/miyu-endpoint/src/uploads.rs` | `blob.open`、`blob.write`、`blob.close`：跟着连接走的上传表、60 秒不写作废 | W-5 |
| `crates/miyu-endpoint/src/attach.rs` | `blob.put` 认是什么、上限，挪成和分块上传共用的一份（W-5）；`blob.get`（W-6） | W-5、W-6 |
| `crates/miyu-endpoint/src/queries.rs` | 可选软件包登记的查询：方法名到怎么答的一张表 | W-4 |
| `crates/miyu-endpoint/src/login.rs`、`login/` | 登录码、登录令牌、`account.login_code`、`account.logout`，用登录令牌连着的连接 | W-8 |
| `crates/miyu-store/src/human.rs` | 交出模板的原文，不只是换好的字 | W-1 |
| `crates/miyu-store/src/blob.rs` | 分块暂存、改名进位置；读一段 | W-5、W-6 |
| `crates/miyu-store/src/logins.rs` | `home/<账号>/logins.json`：读、写、删过期的 | W-8 |
| `crates/miyu-fs/src/list.rs`、`find.rs` | 列一层；建清单、打分 | W-2 |
| `crates/miyu-fs/src/real.rs` | 从最近一层在的目录换成真实的位置 | W-3 |
| `crates/miyu-fs/src/range.rs` | 安全地打开以后读一段 | W-6 |
| `crates/miyu-mermaid/`（新，第 3 层） | 画 SVG、三种记号色、缓存、第一次用才读字体。可选软件包 `mermaid` | W-4 |
| `crates/miyu-net/`（新，第 3 层） | 地址闸、钉住解析好的地址、自己跟重定向、读到 `</head>`、挖元数据、认图。可选软件包 `net`，以后 `web_fetch` 用同一份 | W-7 |
| `crates/miyu-core/src/packages.rs` | 照编进来的可选软件包往查询表里登记（cargo 开关 `mermaid`、`net`）；起来时清掉分块上传留下的暂存 | W-4、W-5、W-7 |
| `crates/miyu-ipc/src/lib.rs`、`start.rs` | 不读本机令牌的连法：`connect_bare`、`connect_or_start_bare` | W-8 |
| `crates/miyu-cli/src/web.rs` | `miyu web`：找主程序旁边的 `miyu-web`，把参数交给它 | W-9 |
| `resources/software/mermaid/style.json` | 字体、三种记号色 | W-4 |
| `resources/software/net/link_preview.json` | 抓链接卡片的规矩：时限、上限、请求头、记多久（照桥的那份） | W-7 |
| `resources/core/human/{zh,en,ja}.json` | 新原因码的话 | 各步 |

**网页软件**：

| 代码 | 管什么 | 步 |
|---|---|---|
| `crates/miyu-web/`（新，第 5 层，头） | 程序 `miyu-web` | W-9 |
| `crates/miyu-web/src/main.rs` | 子命令 `open`、`serve` | W-9 |
| `crates/miyu-web/src/open.rs` | 确保 `serve` 在跑；第一次没有管理员账号的，照终端的样子出示本机令牌要一次性码；开浏览器 | W-9 |
| `crates/miyu-web/src/serve.rs` | 单实例、听端口、写那一行、空闲退出、清过期的跳转页 | W-9 |
| `crates/miyu-web/src/pages.rs` | 页面文件、响应头 | W-9 |
| `crates/miyu-web/src/ws.rs` | 核对 Host、Origin；WebSocket 和核心连接两头照转 | W-9 |
| `crates/miyu-web/src/media.rs`、`media/tickets.rs` | `POST /media` 换票据，`GET /media/<票据>` 分段给 | W-10 |
| `resources/web/web.json` | 出厂的端口、空闲多久、票据记多久、媒体类型的表、页面的内容安全策略 | W-9、W-10 |
| `resources/web/pages/` | 页面文件。M9 的网页搬进主仓库以前，开发时设 `MIYU_WEB_PAGES` 指到网页演示的 `web-demo/` | W-9 |

- 分层照 `01-架构.md` 第九节：`miyu-mermaid`、`miyu-net` 在第 3 层（执行器），`miyu-web` 在第 5 层（头）。门禁读那张表，三行要先登记（「要跟着改的别的页」）。
- `miyu-web` 只依赖 `miyu-ipc`、`miyu-store`、`miyu-kernel`（编号的写法），不依赖 `miyu-endpoint`、`miyu-core`：它不是核心。
- 主程序 `miyu` 不依赖 `miyu-web`。`miyu web` 只是一个找程序、交参数的入口（「怎么走」第十一条）。
- 标 W-8 到 W-11 的几行随用户系统重画（「是什么」末尾）。

### 对外的样子

#### 核心多的方法

都在握手以后用。现在连上来的都是管理员，照 `protocol.md`「握手」第 4 条。

| 方法 | 做什么 | 谁能调 | 步 |
|---|---|---|---|
| `human.get` | 给人看的字：工具的样子、说法的模板 | 都能 | W-1 |
| `fs.list` | 列一层目录 | 都能 | W-2 |
| `fs.find` | 在一个目录里模糊找文件 | 都能 | W-2 |
| `fs.realpath` | 一个路径换成真实的位置 | 都能 | W-3 |
| `mermaid.render` | mermaid 源码画成 SVG。编进了 `mermaid` 包才有 | 都能 | W-4 |
| `blob.open`、`blob.write`、`blob.close` | 分块传一个附件，最后存成 blob | 都能 | W-5 |
| `blob.get` | 分块读这个账号的一个 blob | 都能 | W-6 |
| `fs.read` | 分块读本机的一份文件 | 都能 | W-6 |
| `link.preview` | 一个链接的卡片。编进了 `net` 包才有 | 都能 | W-7 |
| `account.login_code` | 要一个一次性登录码 | 只有出示本机令牌连上的 | W-8 |
| `account.logout` | 作废登录令牌 | 都能 | W-8 |

#### 握手多的

参数（W-8）：`token`、`code`、`login` 正好写一个。

| 参数 | 类型 | 说明 |
|---|---|---|
| `token` | 字符串 | 本机令牌，照旧（`ipc.md`） |
| `code` | 字符串 | 一次性登录码：64 位小写十六进制 |
| `login` | 字符串 | 登录令牌：64 位小写十六进制 |

回应多两格：

| 格 | 值 | 步 |
|---|---|---|
| `host` | `{"home": <系统的家目录>, "platform": "linux" 或 "macos" 或 "windows", "workspace": <这个账号的工作区>}`，总有 | W-3 |
| `login` | `{"expires": <时刻>, "token": <登录令牌>}`：只在用登录码握手时有 | W-8 |

```json
{"id":"h1","jsonrpc":"2.0","result":{"account":"admin","core":{"version":"0.1.0"},"host":{"home":"<家目录>","platform":"macos","workspace":"<家目录>/.miyu/home/admin/workspace"},"language":"zh","login":{"expires":"2026-10-31T06:00:00.000Z","token":"9f…"},"protocol":1,"sandbox":{"usable":true}}}
```

#### 每个方法的参数和回应

**`human.get`**（W-1）

| 参数 | 类型 | 说明 |
|---|---|---|
| `language` | 字符串，可以不写 | 2 到 8 个小写字母，例如 `zh`。不写照这个连接的语言（握手回应的 `language`） |

回应 `{"language": <语言>, "said": {<说法的编号>: <模板>}, "tools": {<工具名>: <样子>}}`。样子照 `store/resources.md` 的 `Face`：`name`，可以有 `subject`、`icon`、`block`。

**`fs.list`**（W-2）

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 相对的路径照它接：绝对路径，或者 `~`、`~/…`，头报的那种写法 |
| `dir` | 字符串，不写是 `""` | 打的那一截目录：`~` 打头的照家目录，绝对的照原样，别的照 `cwd` |
| `prefix` | 字符串，不写是 `""` | 名字的开头 |

**`fs.find`**（W-2）

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 在哪个目录里找，写法同上 |
| `query` | 字符串，不写是 `""` | 打的字 |
| `fresh` | 布尔，不写是 `false` | 头开列表时写 `true`：清单建好 10 秒以上的重建 |

两个的回应一个样子：`items` 每一条 `{"dir": <布尔>, "full": <绝对路径>, "marks": [<第几个字>…], "path": <列表上写的>, "size": <字节数>}`，`size` 只有文件才有；`partial` 列没列全；`fs.find` 另有 `building`：清单还在建。

```json
{"building":false,"items":[{"dir":false,"full":"<家目录>/src/miyu/src/main.rs","marks":[4,5,6,7],"path":"src/main.rs","size":2048}],"partial":false}
```

**`fs.realpath`**（W-3）：`path` 必写；`cwd` 可以不写，`path` 是相对的才要。回应 `{"path": <真实的位置>}`。

**`mermaid.render`**（W-4）：`source` 必写。回应 `{"marks": {"label": <色>, "line": <色>, "text": <色>}, "svg": <SVG 的字>}`：SVG 里字、线、连线标签垫底用的三种记号色，头照它换成自己的颜色（第五条第 7 款）。

**`blob.open`、`blob.write`、`blob.close`**（W-5）

| 方法 | 参数 | 回应 |
|---|---|---|
| `blob.open` | `name`（必写，照 `blob.put`）、`media_type`（可以不写，照 `blob.put`）、`size`（必写，非负整数：一共几个字节） | `{"upload": <上传编号>}` |
| `blob.write` | `upload`、`offset`（非负整数，从 0 数）、`data`（base64，一块最多 512 KiB） | `{"received": <一共收到几个字节>}` |
| `blob.close` | `upload` | 照 `blob.put` 的回应：`blob`、`name`、`media_type`、`kind`，图片另有 `width`、`height` |

**`blob.get`**（W-6）：`blob` 必写（内容哈希）；`offset` 不写是 0；`length` 不写是 512 KiB，最多 512 KiB，写 0 只问大小。回应 `{"data": <这一段的 base64>, "size": <一共几个字节>}`。

**`fs.read`**（W-6）：`path` 必写（绝对路径，或者 `~`、`~/…`）；`offset`、`length` 同 `blob.get`。回应同 `blob.get`。

**`link.preview`**（W-7）：`url` 必写。回应二选一：

```json
{"card":{"description":"…","icon":{"blob":"sha256:…","media_type":"image/x-icon"},"image":{"blob":"sha256:…","media_type":"image/png"},"site":"GitHub","title":"…","url":"https://github.com/…"}}
{"card":null,"why":"no_preview"}
```

- `image`、`icon` 可以是 `null`。图是这个账号的 blob，头照 `blob.get` 读，网页照 `/media` 给。
- `why`：`not_a_url`（读不成地址）、`unsupported_scheme`（不是 http、https）、`no_preview`（不是网页、没有标题、地址过不了闸、跳转太多，下次也一样）、`unreachable`（超时、连不上、对方回 4xx、5xx，过会儿可能就好了）。做不出卡片是正常的结果之一，不是出错。

**`account.login_code`**（W-8）：没有参数。回应 `{"code": <登录码>, "expires": <时刻>}`。

**`account.logout`**（W-8）：`all` 布尔，不写是 `false`。回应 `{"revoked": <作废了几个>}`。

#### 网页软件对外的样子

命令（W-9）：

| 命令 | 做什么 |
|---|---|
| `miyu web` | 打开网页：确保网页软件在跑，要一个登录码，开浏览器 |
| `miyu web --print` | 不开浏览器，印出带登录码的网址，人自己开 |
| `miyu web --port <端口>` | 网页软件这一次在哪个端口上听；`0` 是随便挑一个空的 |
| `miyu web --logout` | 作废这个账号全部的登录令牌：所有浏览器都要再 `miyu web` |
| `miyu-web open …`、`miyu-web serve` | `miyu web` 交给的程序本身；`serve` 是 `open` 拉起来的，不写进帮助 |

HTTP（W-9、W-10）：

| 请求 | 做什么 | 要什么 |
|---|---|---|
| `GET /`、`GET /<页面文件>` | 页面文件 | Host 对得上。不要登录：里面没有秘密 |
| `GET /ws` | WebSocket：转到核心 | Host、Origin 对得上；身份在握手里，核心验 |
| `POST /media` | 换一张票据 | `Authorization: Bearer <登录令牌>` |
| `GET /media/<票据>` | 一个 blob 或者一份本机文件，可以带 `Range` | 票据 |

数据根里多的文件（W-9）：

| 文件 | 内容 | 谁写 |
|---|---|---|
| `run/web.lock` | 空文件，网页软件的单实例锁在它上面 | 网页软件 |
| `run/web` | 一行：网页的地址，`http://127.0.0.1:<端口>` | 网页软件，每次起来 |
| `run/web-open-<16 位十六进制>.html` | 跳转页：带着登录码跳到网页；Unix 上 0600；5 分钟后删 | `miyu-web open` |
| `home/<账号>/logins.json` | 登录令牌的哈希、什么时候造的、什么时候过期（W-8） | 核心 |

### 怎么走

**一、身份：本机的浏览器怎么进来**（W-8，随用户系统重画）

项目主人 2026-10-01 定的方向：

- 身份由核心验，网页软件只转发、不读本机令牌（第 1 题）。
- 第一次：`miyu web` 给一个一次性码（命令行里出现就出现，它只用一次），进网页是一个引导，建管理员账号（用户名、密码）；建好了码当场作废（第 1、3 题）。
- 以后：用用户名和密码登录，浏览器记住 30 天（第 2 题）。
- 本机的头（终端界面、`miyu ask`）照旧用本机令牌。

带进用户系统那次讨论的：管理员账号和现在的 `admin` 账号是不是同一个；密码照设计 06 U4（argon2id、失败限流）；忘了密码怎么从本机终端重设；`miyu web --logout` 还要不要。

下面是起草时的样子（登录码换登录令牌、跳转页），只当参考：

```mermaid
sequenceDiagram
    participant T as miyu web
    participant C as 核心
    participant W as 网页软件
    participant B as 浏览器
    T->>C: hello（本机令牌）
    T->>C: account.login_code
    C-->>T: 登录码，5 分钟、一次
    T->>B: 打开数据根里的跳转页（命令行里只有它的路径）
    B->>W: GET /（跳转到 #code=登录码）
    B->>W: WebSocket /ws（核对 Host、Origin）
    W->>C: 连核心，不带本机令牌
    B->>C: hello（登录码），网页软件照转
    C-->>B: 回应带登录令牌，30 天
    Note over B: 以后握手出示登录令牌
```

1. 本机令牌只给本机的头：终端界面、`miyu ask`，还有 `miyu-web open` 要登录码的那一下，都照终端的样子出示它（`ipc.md`）。网页软件转发浏览器的那些连接不读、不出示本机令牌：它用 `connect_bare` 连核心，那条路不读 `run/token`。
2. `account.login_code`：只给出示本机令牌连上的连接，别的回 `local_only`。登录码是 32 个系统给的随机字节，写成 64 位小写十六进制；5 分钟内有效、只能用一次（设计 21 X6）；只在核心的内存里，核心重启全部作废；同时最多 16 个，多了丢掉最早的。
3. 握手带 `code`：在内存里、没过期，这个连接就是要它的那个账号；登录码当场作废。接着造一个登录令牌（32 个随机字节，64 位小写十六进制），把它的 SHA-256 和造的时刻、过期的时刻（30 天后，设计 06 U4）写进 `home/<账号>/logins.json`，落了盘才回应，回应带 `login`。对不上、过期了、用过了：`bad_code`，回完断开。
4. 握手带 `login`：它的 SHA-256 在这个账号的 `logins.json` 里、没过期，这个连接就是这个账号。对不上、过期了、作废了：`bad_login`，回完断开。查的时候照哈希找，不逐字节比原文：哈希是 32 个随机字节算出来的，比的快慢透露不了什么。
5. 三个都没写：`bad_token`，照旧。写了不止一个：`bad_params`，回完断开。
6. 用登录码、登录令牌连上的，和出示本机令牌的一样是这个账号，命令照账号判（设计 06）。只差一样：不能要登录码。浏览器拿到登录令牌也换不出新的登录码，登录码只能从本机的终端来。
7. `account.logout`：用登录令牌连上的，`all` 不写，作废这一个，回完断开这个连接；`all` 写 `true`，作废这个账号全部的登录令牌，断开所有用登录令牌连着的连接（这个连接回完再断）。出示本机令牌的只能写 `all: true`，不然 `bad_params`。作废就是从 `logins.json` 里删掉那一行。
8. `logins.json` 的写法：`{"version":1,"tokens":[{"created":<时刻>,"expires":<时刻>,"hash":"sha256:<64 位>"}]}`，照写配置文件的办法（`crates/miyu-store/src/config_file.rs`）先写临时文件再改名，Unix 上 0600。每次写都把过期的删掉；最多 64 行，多了删最早造的。读不了、坏了：当是空的，记一行 `WARN logins not read`，用登录令牌的都进不来，再 `miyu web` 一次就是。
9. 登录码、登录令牌一个字都不进运行日志。握手过了记 `INFO connected head=… version=… protocol=1 via=token`（`code`、`login`）。
10. 别的进程（包括沙盒里的命令）连上核心的套接字、网页软件的端口，都没有凭据：本机令牌、`logins.json` 在数据根里，沙盒读不到（设计 11 第五节）；登录码不进命令行（第十一条第 4 款）；登录令牌只在浏览器里，和网页软件转发、换票据时的内存里（第十条第 2 款），不落盘。

**二、给人看的字**（`human.get`，W-1）

1. 每次现读资源目录：照 `Human::load` 的读法，先读内核的 `core/human/<语言>.json`，再照名字的先后读 `software/` 下每个软件包的 `human/<语言>.json`，哪一份没有这种语言照英文（`store/resources.md`「怎么走」第 3 条）。开发时改了资源，不用重启核心。在阻塞线程里读。
2. `tools` 合成一张，后读的盖掉先读的同名工具；`said` 的编号前面加上它在资源目录里的位置（`core/…`、`software/<包>/…`），模板照原样给。换字段是头的事，照 `store/resources.md`「怎么走」第 4 条：控制字符换成 `�`。
3. 不给 `config` 那一格：配置的名字、说明在 `config.schema` 里。
4. 读得到却读不懂的：`internal_error`，记一行 `WARN human not read error=…`，写明是哪一份。
5. 回应的 `language` 是要的那一种；哪一份退回了英文，回应里不分，和 `miyu ask` 读到的一样。

**三、列文件、找文件**（`fs.list`、`fs.find`，W-2；照 proto/tui-demo 分支 `docs/blueprint/tui.md`「`@` 文件列表」第 2、3 条，设计 13 H14）

1. `cwd` 照 `fs.md` 换成真实的位置（`~` 照家目录接），要是一个目录。换不成、不在、不是目录：`path_unreadable`。
2. 数据根：落在数据根里、又不在这个账号的工作区里的，不列、不找，回 `path_forbidden`（照 `fs.md` 边界表的「谁都不能碰」那一片）。列的时候每一条照真实的位置判，落进那一片的不列。工作区在数据根里（默认的会话就在那里干活），照样能列。
3. `fs.list`：`dir` 照参数表接好、换成真实的位置，只读那一层。名字照开头对 `prefix`，大小写不论。点开头的藏起来，`prefix` 以 `.` 开头才列。目录在前、文件在后，各照名字排（大小写不论）。目录的 `path` 后面带 `/`。最多 50 条，多了截掉、`partial` 是 `true`。`marks` 是 `path` 的前几个字，`prefix` 有几个字就几个。
4. `fs.find`：在 `cwd` 里建一份清单：`ignore` 库，和核心的 `glob`、`grep` 同一套，认 `.gitignore`（不要求是 git 仓库），跳过隐藏目录和出厂名单里的 `node_modules`、`target`，跳过数据根（工作区除外），不跟链接；最深 8 层，最多 20000 个，收满就停、`partial` 是 `true`。`path` 是相对 `cwd` 的，用 `/` 连，目录后面带 `/`。
5. 清单在阻塞线程里建，不挡别的请求。还没建完，照已经建好的那一部分答，`building` 是 `true`；头隔 200 毫秒再问，直到 `false`。
6. 什么时候重建：这个目录还没有清单；`fresh` 是 `true`、清单建好 10 秒以上。核心最多记 4 个目录的清单，多了丢最久没用的。同一个目录同时来两次，第二次等第一次建的那一份。
7. 怎么排：打的字照先后都在 `path` 里（大小写不论）才列。先试整个落在文件名里，落不下再从路径开头找；每个字对上 1 分，落在文件名里多 3 分，在一段的开头（路径的头一个字，或者前面是 `/`、`-`、`_`、`.`、空格）多 8 分，和上一个字连着多 5 分；文件名去掉扩展名正好是打的字多 100 分。分高的在前，一样的路径短的在前，再一样的照字排。最多 50 条。`query` 是空的都对得上、0 分。打分照桥的 `mention.rs` 的 `score`，测试一起搬过来。
8. `full` 照平台的写法（Windows 上是 `C:\…`）；`size` 照文件现在的大小，读不出的不写。
9. 出厂的数（50、20000、8 层、10 秒、200 毫秒、4 份、跳过的名单）写在 `crates/miyu-fs/src/find.rs`，照 `jobs.output_chars` 的放法，配置那一步能改。

**四、路径**（W-3）

1. 握手回应的 `host`：`home` 是核心起来时拿到的系统的家目录，照原样；`platform` 是核心所在的平台；`workspace` 是这个账号的工作区，换成真实的位置。头拿 `home` 把路径写成 `~/…`，拿 `platform` 认路径分隔、命令的引号怎么写，拿 `workspace` 当新会话默认的工作目录（设计 11 第四节：网页上开的会话默认在账号的工作区）。
2. `fs.realpath`：`~` 照家目录接；相对的接在 `cwd` 上，没给 `cwd` 的 `bad_params`。从它自己往上找第一层在的，那一层换成真实的位置，后面几段原样接上：还没建出来的目录，建出来以后就是这个位置。一层都不在（Windows 上盘符都没有）：`path_unreadable`。
3. 只说位置，不读内容：落在数据根里的照样换。

**五、mermaid**（`mermaid.render`，W-4；设计 04 第五节、13 第九节，2026-10-01 项目主人定）

1. 画 mermaid 在核心里：代码只有一份，同一张图终端和网页看只画一次，画图的内存只在核心里。做成可选的软件包 `mermaid`：crate `miyu-mermaid`，经 `miyu-core` 的 cargo 开关 `mermaid` 编进来，发行版默认打开。没编进来的核心里没有这块代码，`mermaid.render` 回 `unknown_method`，头照代码块显示源码。
2. 第一次调才初始化：读 `resources/software/mermaid/style.json`、读系统的字体库（画图的库要量字的宽）。之后一直留着，直到核心退出。
3. 源码去掉前后空白。空的：`bad_params`。超过 64 KiB：`mermaid_too_long`。
4. 缓存：照源码的 SHA-256，最多 64 张，满了丢最久没用的。设计说「源码加尺寸」：SVG 不分尺寸，缓存只照源码；尺寸只在终端栅格化时用，那一步在头里。
5. 一次画一张（一把锁），在阻塞线程里画。画图的库崩了（panic），当画不出。
6. 画不出：`mermaid_failed`，`data.detail` 是画图的库的原话（英文）。
7. 样子照终端演示和桥：从画图的库的暗色主题改起，底和框都不填色；字、线、连线标签的垫底先填三种图里不会自己出现的记号色，回应的 `marks` 写明是哪三种。网页把它们换成页面的 CSS 变量（换主题不用重画），终端换成主题色再栅格化。字体照 `style.json` 的一串（正文常用的几种，最后是 `sans-serif`）。
8. 用的库照终端演示和桥：`mermaid-rs-renderer` 0.3.1，同一个版本。核心出了以后，终端演示和桥各带的那一份都去掉。
9. 视图投影（M9）做出来以后，`view.detail` 取 mermaid 那一项，照同一个画法、同一份缓存给。

**六、分块上传**（`blob.open`、`blob.write`、`blob.close`，W-5；设计 04 第十节「后续再定」的分块上传）

1. `blob.open`：`name`、`media_type` 照 `blob.put` 第 1 条的写法查；`size` 超过 20 MiB（20,971,520 字节）当场 `attachment_too_big`。这个连接同时开着 4 个的：`too_many_uploads`。成了交回上传编号（16 位小写十六进制），在这个账号的 `blobs/tmp/` 里建暂存文件 `upload-<编号>`。
2. `blob.write`：`offset` 要正好等于已经收到的字节数，不对回 `upload_offset`，`data.received` 写已经收到几个，头从那里接着传。`data` 不是 base64、解出来超过 512 KiB、加上它超过 `size`：`bad_params`。写进暂存文件，边写边算 SHA-256。
3. `blob.close`：收到的不够 `size`：`upload_incomplete`（`data.received`）。够了：照 `blob.put` 第 4 条认是什么、查图片的上限，照第 5 条存：暂存文件改名进位置，同一份内容已经有了的，删掉暂存的，还是那一个 blob。回应和 `blob.put` 一样。
4. 上传跟着连接走：编号只认开它的那个连接，别的连接拿来用回 `upload_unknown`。连接断了，它开的上传全部作废、删掉暂存文件。60 秒没有 `blob.write` 的，也作废。`close` 以后编号作废。
5. 同一个连接上的请求本来就一条条办（`protocol.md`「一个连接」第 1 条），一个上传不会同时写两块。一个附件拆成 40 块左右，一块一个来回。
6. 核心起来时清掉 `blobs/tmp/` 里的 `upload-*`：崩了、被杀留下的。
7. 不碰会话，不进会话的日志，和 `blob.put` 第 6 条一样。附件的大小上限和 `blob.put` 同一个数，一处定义。

**七、分块读**（`blob.get`、`fs.read`，W-6）

1. `blob.get`：这个账号的 blob，照属主给，不照会话（「起草时定的」第 12 条）。没有：`unknown_blob`。
2. `fs.read`：照 `fs.md` 换成真实的位置，`~` 照家目录接，相对的 `bad_params`。照边界表落在「谁都不能碰」那一片（数据根里、工作区以外）：`path_forbidden`。照 `fs.md` 第四节安全地打开，路上一层链接都不跟；没有、不是普通文件、没有权限：`path_unreadable`。能读哪些是项目主人 2026-10-01 定的（第 4 题）。
3. 读法：从 `offset` 起读 `length` 个字节，读到结尾就停；`offset` 过了结尾的，`data` 是空的。`size` 是打开那一刻的大小。`length` 写 0 只回 `size`，`data` 是空的。超过 512 KiB：`bad_params`。
4. 一段一段读不重新核对哈希：核对整个 blob 的哈希在核心自己用它的时候（`store.md` 第 10 条）。
5. 在阻塞线程里读。

**八、链接预览**（`link.preview`，W-7；规矩照桥的 `link_preview/`，桥照的是旧版）

1. 可选软件包 `net`：抓取和地址闸在 crate `miyu-net`，经 `miyu-core` 的 cargo 开关 `net` 编进来，发行版默认打开；没编进来的 `link.preview` 回 `unknown_method`，头照原样留着链接。以后 `web_fetch` 用同一份抓取和地址闸（设计 10 第四节）。
2. 地址：去掉前后空白读成网址，读不成 `not_a_url`；不是 `http`、`https` 的 `unsupported_scheme`。
3. 地址闸，每一跳都过：
   1. 样子：只认 `http`、`https`；不带用户名、密码；`localhost`、`*.localhost`、`*.local` 不去；主机写的就是 IP 的照第 3 层判。
   2. 解析：每一个解析出来的地址都要是公网的，有一个不是就整个不去。解析好的地址钉进这一跳的连接：查过的就是连上的，中间没有第二次解析（防 DNS rebinding）。
   3. IP 段：回环、私网、链路本地、运营商级 NAT、唯一本地、组播、未指定、广播、保留、文档的段都不是公网（测性能的段 `198.18.0.0/15` 算公网，见第 5 条）；里面嵌着 IPv4 的 IPv6（映射、NAT64、6to4）照那个 IPv4 判，IPv4 兼容的写法不去。表照桥的 `guard.rs`，测试一起搬过来。
4. 跳转自己跟，最多 5 跳，每一跳重新过闸。
5. 代理（2026-10-01 项目主人定，第 5 题）：和请求模型一样照环境变量走（`HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`，小写的也认，`http.md`「客户端」第 5 条）。
   - 这一跳要走代理的：先在本机解析一遍，过第 2 层（有一个不是公网的就不去），再交给代理连。代理那头怎么解析我们管不着，这个口子项目主人认了。
   - 不走代理的（没设，或者在 `NO_PROXY` 里）：照第 2 层钉住解析好的地址。
   - 本机解析不出来、代理那头解析得出来的（被污染的域名常这样）：照样交给代理，第 1 层、第 3 层照常过。
   - 测性能的段 `198.18.0.0/15` 当公网：Clash、mihomo、sing-box、Surge 的假地址（fake-ip）默认就在这一段，开着这类代理的机器上每个域名都解析到这里，连上的其实是代理软件，它照域名去连。不放过这一段，这些机器上一张卡片都出不来。
6. 页面：一跳 12 秒；回的不是 HTML 的 `no_preview`；读到 `</head>` 或者 `<body` 就停，最多 2 MiB。
7. 挖元数据：标题、简介、图照 `og:*`、`twitter:*`、`<title>` 和 `<meta name=description>` 的先后；站名照 `og:site_name`，没有用主机名去掉 `www.`；图标照 `rel=icon`、`apple-touch-icon`，都没有试 `/favicon.ico`；相对地址照最后落到的那一页算；空白收拢，标题最多 120 个字、简介 300、站名 60，超出的截断加 `…`。没有标题的 `no_preview`。
8. 图：一张 8 秒、最多 3 MiB，只收照开头的魔数认得出的五种（PNG、JPEG、GIF、WebP、ICO），不收 SVG（它能带脚本）。存成这个账号的 blob，回应里写哈希和认出来的类型。抓不到的那一格是 `null`，卡片照样成立。
9. 抓过的记在核心的内存里：抓到了的记 6 小时，`no_preview` 记 15 分钟，`unreachable` 记 45 秒；最多 512 条，满了整个清空。记着的卡片指的 blob 没了的，那一格交 `null`。
10. 请求头、时限、上限都在 `resources/software/net/link_preview.json`，照桥的那份搬。
11. 不碰会话。在后台的任务里抓，不挡这个连接上后面的请求的读：回应照 `id` 对上（`protocol.md`「一个连接」第 1 条要改成这一种例外，「要跟着改的别的页」）。

**九、网页软件：起停、端口、页面、WebSocket**（W-9）

1. 单实例：`miyu-web serve` 先拿 `run/web.lock`，拿不到写 `running` 走。拿到了听端口，把地址写进 `run/web`（先写临时文件再改名），往标准输出写一行 `ready`，和核心那一行同一个写法（`ipc.md`「那一行」，复用 `miyu-ipc` 的 `Ready`）。起不来写 `error <原因>`。
2. 端口：照 `--port`，没写照 `resources/web/web.json` 的出厂值（固定端口，第 2 题）；`0` 是系统挑一个空的。只听回环地址 `127.0.0.1`，不听别的网卡。端口被占了：`error 端口 <端口> 被占了`，`miyu web` 照原样印出来，后面接一句怎么换。
3. 空闲退出：没有 WebSocket 连着、没有 `/media` 在给，连续 10 分钟就退出（`web.json` 的出厂值），先删 `run/web`、再放锁。收到停的信号照样先删再放。
4. 每个请求先核对 Host：只认 `127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>`，别的回 403。别的网站把自己的域名解析到回环地址也进不来（DNS rebinding）。
5. 页面文件：`GET /` 给 `index.html`，别的照路径在页面目录里找。带 `..` 的、换成真实位置以后跑到页面目录外的、不是普通文件的，404。类型照扩展名（`web.json` 的表）。响应头一律带：`X-Content-Type-Options: nosniff`、`Referrer-Policy: no-referrer`、`Cache-Control: no-cache`、`Content-Security-Policy`（照 `web.json`，至少有 `connect-src 'self'`、`frame-ancestors 'none'`）。从来不设 cookie（「起草时定的」第 13 条）。
6. 页面目录：`MIYU_WEB_PAGES` 设了照它，不然是资源目录下的 `web/pages/`。资源目录照 `store/resources.md` 第 1 条找，和核心同一个办法。
7. `GET /ws`：Origin 要正好是 `http://` 加上第 4 款三种之一（带端口），不然 403。接了以后连核心：`connect_or_start_bare`，核心没在跑就拉起来（命令是主程序 `miyu` 加 `core`，主程序在 `miyu-web` 的真实位置旁边）。连不上：往 WebSocket 发一条通知 `{"jsonrpc":"2.0","method":"web.error","params":{"message":<原因>}}`，再关。
8. 一个标签页一条核心连接，不合并（proto/web-demo 分支 `docs/blueprint/web/architecture.md`「多用户、多终端」第 7 条）。两头照转：文字帧加一个 `\n` 是一行，一行去掉 `\n` 是一个文字帧。不读、不改、不加：握手的凭据、命令、推送原样过去。二进制帧：关，1003。一帧超过 1 MiB：关，1009（核心那头一行也就这么长）。
9. 一头断了另一头跟着关。核心那头断了（重启、退出）：WebSocket 关，1012，页面照自己的规矩重连（proto/web-demo 分支 `docs/blueprint/web.md`「连核心」第 1 条）。
10. 运行日志 `state/logs/web.log`，满了照核心的换法（`log.md`）。只记连上、断开、出错，不记一行的内容、登录码、登录令牌、票据。

**十、网页软件：媒体地址**（`/media`，W-10；2026-09-30 定的「小的经协议，大的由网页给带令牌的地址」，那时说的网页模块现在是网页软件）

1. `POST /media`：`Authorization: Bearer <登录令牌>`；正文是 JSON：`blob`（内容哈希）或者 `path`（绝对路径），正好一个；可以带 `type`（媒体类型）、`name`（存下来叫什么）、`download`（布尔，叫浏览器存下来）。
2. 网页软件照这个登录令牌连核心：同一个令牌的连接留着复用，60 秒不用就关。握手被拒（`bad_login`）回 401。
3. 先问核心有没有、能不能读：`blob.get` 或者 `fs.read`，`length` 写 0。`unknown_blob`、`path_unreadable` 回 404，`path_forbidden` 回 403。
4. 造一张票据：32 个随机字节，64 位小写十六进制。记在内存里：哪个登录令牌、哪个资源、多大、`type`、`name`、`download`。同一个令牌、同一个资源、同样三格的，交回原来那一张。12 小时没用过的作废；最多 4096 张，多了丢最久没用的。网页软件重启，票据全作废，页面照 404 重新换。
5. 回应 `{"url":"/media/<票据>"}`。
6. `GET /media/<票据>`：不认识的 404。带 `Range: bytes=…` 的只认一段，回 206；超出的回 416；不带的回全部。照 `blob.get`、`fs.read` 一块 512 KiB 地读，读一块写一块，不整个读进内存。
7. 类型：`type` 在 `web.json` 的 blob 类型表里的照它；`path` 的照扩展名查表；都没有的 `application/octet-stream`。响应头带 `nosniff`、`Cache-Control: private, no-cache`、`Content-Security-Policy: sandbox; default-src 'none'; img-src data:; media-src data:; style-src 'unsafe-inline'`：有人直接打开这个地址（一个 SVG、一个 HTML），它在一个空的来源里跑，碰不到页面。`download` 的加 `Content-Disposition: attachment`，名字照 `name`（只留最后一段），UTF-8 照 RFC 5987 转义。
8. 链接卡片的图、附件、她写到的本机图片和音视频，都走这一条。网页软件不另开图片代理：抓网上东西的只有核心的 `net` 包，地址闸只有一处。
9. 有 `/media` 在给，网页软件不算空闲。

**十一、`miyu web`**（W-9。登录码、跳转页那几款随用户系统照第一条的方向重画：第一次印一次性码、开浏览器进引导，以后开浏览器登录）

1. 主程序的 `miyu web` 找主程序真实位置旁边的 `miyu-web`（Windows 上是 `miyu-web.exe`），把 `web` 后面的参数原样交给 `miyu-web open`，等它退出，退出码照它的。没有：印「没装网页界面」和每种装法怎么装，退出码 1。
2. `miyu-web open`：`run/web` 在、锁有人拿着，网页软件就在跑，照 `run/web` 的地址。不然拉起 `miyu-web serve`：和头拉起核心一样（`ipc.md`「连不上就拉起」第 4、5 条），跟终端脱开，工作目录是数据根，等那一行最多 10 秒。
3. 照终端的样子连核心（`connect_or_start`，出示本机令牌），`account.login_code` 要一个登录码。网址是 `<地址>/#code=<登录码>`：登录码在 `#` 后面，不发给服务器、不进 Referer，页面拿到以后从地址栏抹掉（设计 21 X6）。
4. 把网址写进数据根里的跳转页 `run/web-open-<16 位十六进制>.html`（Unix 上建的时候就是 0600），用系统的办法打开这个文件：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" "<路径>"`。命令行里只有跳转页的路径，登录码不进进程列表：同一个用户的程序（包括沙盒里的命令）读得到别的进程的命令行，读不到数据根（起草时的样子，第 3 题没用它）。
5. 跳转页：一行 `<meta http-equiv="refresh" content="0;url=…">`，加一个同样地址的链接，浏览器不自动跳时点它。`miyu-web serve` 起来时和之后每分钟删掉 5 分钟以前的跳转页（登录码也就 5 分钟）。
6. 交给了浏览器：印网页的地址（不带登录码），再印一句「浏览器没打开登录页的话，用 miyu web --print」（第 3 题说的 snap 装的 Firefox），退出码 0。浏览器开没开、登没登上，`miyu web` 看不到。交不出去（没有 `xdg-open`、没有图形界面）：照 `--print` 办。
7. `--print`：不开浏览器，印带登录码的整个网址，下一行提醒「5 分钟内有效，只能用一次，别发给别人」。终端里印的字别的进程读不到。
8. `--logout`：照终端的样子连核心，`account.logout`，`all: true`，印作废了几个。不碰网页软件。
9. 已经登录着的浏览器（存着登录令牌）打开这个网址：页面先用存着的登录令牌，被拒了才用登录码；没用上的登录码 5 分钟后自己作废。

**十二、三个平台**

1. 网页软件连核心走 `miyu-ipc`：Linux、macOS 是 Unix 域套接字，Windows 是命名管道，照 `ipc.md`。`connect_bare` 在三个平台上都只是不读 `run/token`，核对目录、核对管道另一头照旧。
2. 拉起 `serve` 照拉起核心的办法跟终端脱开：Unix 上自成一个进程组，Windows 上 `DETACHED_PROCESS`、`CREATE_NEW_PROCESS_GROUP`。
3. 跳转页的路径里可能有空格（Windows 的用户名）：`xdg-open`、`open` 收一个参数，不经 shell；Windows 照 `cmd` 的规矩加引号。
4. `fs.list`、`fs.find` 的 `path` 一律用 `/`，`full` 照平台；`dir` 在 Windows 上 `\` 和 `/` 都认。点开头的算藏起来，三个平台一样，不看 Windows 的隐藏属性。
5. 测试里的临时目录先换成真实的位置再比（macOS 的 `/var` 是链接）。
6. 端口只听 `127.0.0.1`。浏览器把 `localhost` 解析成 `::1` 的连不上，所以网址一律写 `127.0.0.1`；Host 认 `localhost`、`[::1]` 只为人自己敲地址的时候。

### 样子

这条线不加给模型看的字：`link.preview` 只给头，`mermaid.render` 的 SVG 只给头，都不进请求。

**`miyu web`**，例子：

```text
$ miyu web
网页开在 http://127.0.0.1:<端口>，已经交给浏览器打开。
浏览器没打开登录页的话，用 miyu web --print。

$ miyu web --print
在浏览器里打开：
http://127.0.0.1:<端口>/#code=9f03b21c…
这个链接 5 分钟内有效，只能用一次，别发给别人。
```

**跳转页**，例子：

```html
<!doctype html>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0;url=http://127.0.0.1:<端口>/#code=9f03b21c…">
<a href="http://127.0.0.1:<端口>/#code=9f03b21c…">打开 Miyu</a>
```

### 出错

协议的拒绝照 `protocol.md`「出错」：`code` 是 `-32010`，原因码在 `data.reason`。新的原因码：

| 原因码 | 什么时候 | 步 |
|---|---|---|
| `bad_code` | 握手的登录码对不上、过期了、用过了（之后断开） | W-8 |
| `bad_login` | 握手的登录令牌对不上、过期了、作废了（之后断开） | W-8 |
| `local_only` | 不是出示本机令牌连上的，要登录码 | W-8 |
| `path_unreadable` | 换不成真实的位置、不在、该是目录的不是目录、该是普通文件的不是、没有权限 | W-2、W-3、W-6 |
| `path_forbidden` | 落在数据根里、又不在这个账号的工作区里 | W-2、W-6 |
| `mermaid_too_long` | 源码超过 64 KiB | W-4 |
| `mermaid_failed` | 画不出；`data.detail` 是画图的库的原话 | W-4 |
| `too_many_uploads` | 这个连接同时开着 4 个上传 | W-5 |
| `upload_unknown` | 没有这个上传：编号不对、作废了、不是这个连接开的 | W-5 |
| `upload_offset` | `offset` 和收到的对不上；`data.received` | W-5 |
| `upload_incomplete` | `close` 时没收齐；`data.received` | W-5 |
| `unknown_blob` | 这个账号没有这个 blob | W-6 |

- `bad_params` 多几种：握手写了不止一种凭据；`language` 不合写法；`fs.realpath` 相对的没给 `cwd`；`fs.read` 是相对的；`length` 超过 512 KiB；`blob.write` 的 `data` 不是 base64、太大、超过 `size`；`account.logout` 出示本机令牌的没写 `all: true`；mermaid 源码是空的。
- `attachment_too_big` 多一种：`blob.open` 的 `size` 超过 20 MiB；`blob.close` 认出是图、超了图的上限。
- `internal_error` 多几种：给人看的字读不懂；`logins.json` 写不下；暂存文件建不了、写不进；画图的库初始化不了（读不到字体）。

网页软件的 HTTP：

| 状态 | 什么时候 |
|---|---|
| 403 | Host 不对；`/ws` 的 Origin 不对；`/media` 的路径在数据根里 |
| 401 | `/media` 没带、带错了登录令牌 |
| 404 | 页面文件没有；票据不认识；blob、文件没有 |
| 405 | 页面文件、`/media/<票据>` 不是 `GET`；`/media` 不是 `POST` |
| 416 | `Range` 超出 |
| 502 | 连不上核心（`/media`） |

运行日志（核心的目标 `miyu::endpoint`，网页软件的 `miyu::web`，一律英文）：

| 级别 | 行 | 什么时候 |
|---|---|---|
| `WARN` | `human not read error=…` | 给人看的字读不懂（W-1） |
| `WARN` | `files index failed dir=… error=…` | 建清单时读不了一层目录，跳过它接着建（W-2） |
| `WARN` | `mermaid not ready error=…` | 画图的库初始化不了（W-4） |
| `WARN` | `upload not stored error=…` | 暂存、改名进位置没成（W-5） |
| `INFO` | `login issued` | 用登录码换了一个登录令牌（W-8） |
| `INFO` | `logins revoked count=…` | `account.logout`（W-8） |
| `WARN` | `logins not read error=…`、`logins not written error=…` | `logins.json` 读不了、写不下（W-8） |
| `WARN` | `bad code`、`bad login` | 握手被拒（W-8） |
| `INFO` | `listening url=…`、`stopped reason=…` | 网页软件起来、退出（W-9） |
| `WARN` | `rejected host=… origin=…` | Host、Origin 不对（W-9） |
| `WARN` | `core unreachable error=…` | 连不上核心（W-9） |
| `WARN` | `link preview failed host=… why=…` | 抓卡片没成，只写主机名（W-7，目标 `miyu::net`） |

### 给人看的字

协议的拒绝照握手时的语言，中文、英文各一句：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `bad_code` | 登录链接用不了了：过期了，或者已经用过。再运行一次 miyu web。 | The login link no longer works: it expired or was already used. Run miyu web again. |
| `bad_login` | 登录过期了，或者被退出了。再运行一次 miyu web。 | The login expired or was signed out. Run miyu web again. |
| `local_only` | 只有本机的终端能要登录码。 | Only a terminal on this machine can ask for a login code. |
| `path_unreadable` | 读不了这个路径。 | This path cannot be read. |
| `path_forbidden` | 这是 Miyu 自己的数据，不给看。 | This is Miyu's own data and is not shown. |
| `mermaid_too_long` | 这张图的源码太长了。 | The diagram source is too long. |
| `mermaid_failed` | 这张图画不出来。 | The diagram could not be drawn. |
| `too_many_uploads` | 同时传的文件太多了，等前面的传完。 | Too many uploads at once; wait for the others to finish. |
| `upload_unknown` | 没有这个上传，可能等太久作废了，重新传一次。 | No such upload; it may have expired. Upload the file again. |
| `upload_offset` | 上传接不上，从核心说的地方接着传。 | The upload is out of step; continue from where the core says. |
| `upload_incomplete` | 文件还没传完。 | The file is not fully uploaded yet. |
| `unknown_blob` | 找不到这份内容。 | This content cannot be found. |

`miyu web` 和网页软件印的（照 `ui.language`，和 `miyu ask` 一样）：

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 交给了浏览器 | 网页开在 {url}，已经交给浏览器打开。 | The web UI is at {url} and has been opened in your browser. |
| 提示 | 浏览器没打开登录页的话，用 miyu web --print。 | If the browser did not open the login page, use miyu web --print. |
| `--print` | 在浏览器里打开： | Open this in a browser: |
| `--print` 的提醒 | 这个链接 5 分钟内有效，只能用一次，别发给别人。 | This link works once within 5 minutes. Do not share it. |
| 没装 | 没装网页界面。装法：{怎么装} | The web UI is not installed. To install it: {how} |
| 端口被占 | 端口 {port} 被占了。换一个：miyu web --port <端口> | Port {port} is in use. Pick another: miyu web --port <port> |
| `--logout` | 作废了 {count} 个登录，浏览器要再运行 miyu web 才能进来。 | Signed out {count} logins; browsers need miyu web again. |

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 | 步 |
|---|---|---|
| `crates/miyu-endpoint/tests/human.rs` | 和 `Human::load` 读到的一样；编号带位置；软件包盖掉内核的同名工具；没有这种语言照英文；不带 `config`；读不懂的说是哪一份；改了资源下一次就是新的 | W-1 |
| `crates/miyu-fs/src/find/tests.rs`、`list/tests.rs` | 打分（照桥的 `score` 测试）；按目录找：开头对、点开头打了点才列、目录在前、50 条截断；模糊找：认 `.gitignore`、跳过隐藏目录和名单、最深几层、收满就停 | W-2 |
| `crates/miyu-endpoint/tests/files.rs` | 真核心：数据根不列不找、工作区照样列；清单没建完先给一部分、`building`；`fresh` 隔 10 秒才重建；最多记 4 份；`path` 用 `/` | W-2 |
| `crates/miyu-endpoint/tests/hello.rs` | `host` 三格，`workspace` 是真实的位置；`fs.realpath` 往上找最近在的一层、相对的要 `cwd` | W-3 |
| `crates/miyu-mermaid/src/tests.rs`、`crates/miyu-core/tests/packages.rs` | 记号色都换得掉、底和框不填色；同一份源码第二次不重画；空的、太长、画不出各说一句；没编进来回 `unknown_method`；第一次调之前不读字体 | W-4 |
| `crates/miyu-endpoint/tests/uploads.rs` | 分块传完和 `blob.put` 同一个回应、同一个 blob；接不上回 `received`；没收齐不收；别的连接用不了；断开、60 秒不写作废并删暂存；超过 20 MiB 当场拒；同时 4 个；起来时清暂存 | W-5 |
| `crates/miyu-endpoint/tests/reads.rs` | 读一段、读到结尾、过了结尾是空的、只问大小；没有这个 blob；`fs.read` 数据根拒、工作区能读、链接不跟、不是普通文件 | W-6 |
| `crates/miyu-net/src/guard/tests.rs`、`html/tests.rs`、`tests/preview.rs` | 地址闸的表（照桥的 `guard.rs` 测试）；元数据的先后、截断；跳转每一跳过闸；不是 HTML、没有标题；图只收五种、不收 SVG；记多久、满了清空；本机的假服务器上整条走通 | W-7 |
| `crates/miyu-endpoint/tests/login.rs` | 登录码只给本机令牌的连接、一次、5 分钟；换出登录令牌、落了盘才回；登录令牌认得、过期不认、作废不认；凭据写两种拒；`logout` 一个、全部、断开连接；`logins.json` 坏了当空的；日志里没有码和令牌 | W-8 |
| `crates/miyu-ipc/tests/socket.rs`、`pipe.rs` | `connect_bare` 连得上、不读 `run/token` | W-8 |
| `crates/miyu-web/tests/serve.rs` | 单实例、`run/web`、那一行；Host、Origin 不对 403；页面文件不出页面目录；响应头；不设 cookie；空闲退出 | W-9 |
| `crates/miyu-web/tests/ws.rs` | 真核心：一帧一行两头照转、一个字节都不改；握手的凭据照原样到核心；核心断了 WebSocket 关 1012；网页软件的代码里不读本机令牌（照源码查） | W-9 |
| `crates/miyu-web/tests/open.rs`、`crates/miyu/tests/web.rs` | 跳转页在数据根里、0600、5 分钟删；命令行里没有登录码；`--print`；没装时说怎么装 | W-9 |
| `crates/miyu-web/tests/media.rs` | 换票据要登录令牌；同一个资源交回同一张；`Range` 206、416；类型照表、`nosniff`、`sandbox`；下载的名字转义；票据作废 404 | W-10 |
| 真机实测 | 三个平台各开一次网页、登录、发一句带附件的话、看一张图和一段视频拖进度、一张链接卡片、一张 mermaid 图；终端演示经核心出 mermaid 图、`@` 选文件 | W-9、W-10 |

### 出处

- 项目主人 2026-10-01：网页界面是一个软件，装了才有，网页的东西不进核心（重开设计 04 P5）；核心只给跟界面无关的通用方法；以后远程访问每条连接的身份由核心自己验，网页软件只转发，不拿本机令牌替远程的人登录。
- 项目主人 2026-10-01：画 mermaid 在核心里（设计 04 第五节、13 第九节，2026-09-26 定的），做成可选的软件包，第一次用到才初始化。
- `docs/designs/21-网页.md` X5、X6、第七节；`04-核心协议.md` 第二节、第四节、第五节、P5、P6、第十节「后续再定」（分块上传）；`06-多用户与身份.md` 第二节、U4（登录令牌只存哈希、30 天）；`11-权限与沙盒.md` 第四节（网页上开的会话默认在账号的工作区）、第五节（本机令牌、沙盒读不到数据根）；`10-自带软件.md` 第四节（`net` 包）；`12-进程形态与分发.md` 第三节、第四节（单独分发的程序、软件包）；`13-终端界面.md` 第九节、H14。
- 网页演示的桥：proto/web-demo 分支的 `web-demo/bridge/src/`（规矩、数、测试照它搬）；它的蓝图 `docs/blueprint/web.md`、`docs/blueprint/web/architecture.md`。
- 2026-09-30 和网页演示的会话定的：本机文件、blob 小的经协议，大的由网页给带令牌的地址。
- RFC 6265 第 8.5 节：cookie 不分端口（不用 cookie 的理由）。Jupyter 打开浏览器时写跳转文件，同一个理由：登录码不进进程列表。

### 还没有的

- 远程访问：网页软件听别的网卡、证书（设计 21「后续再定」）、账号密码登录（设计 06 U4 的密码那一半、失败限流）。到时候网页软件照样只转发，凭据照样由核心验。
- 多用户：成员的 `fs.*` 只给自己的工作区（设计 11 第三节）；`blob.get` 分享来的会话（U12）要加 `session` 一格，只给那个会话用过的 blob。
- 登录过的设备列出来、一个个撤（设计 06 第二节）：现在只能全部作废。
- 装软件的管理（`miyu pkg`）：`mermaid`、`net` 现在是编译时的开关，网页软件现在是单独的发行包；有了软件包管理以后照它装、停用。网页软件的数（端口这些）那时搬进它清单里声明的配置项。
- `view.detail`（M9 的视图投影）：到时候 mermaid 照同一份缓存给。
- `web_fetch`、`web_search`（`net` 包的工具）：用 `miyu-net` 的抓取和地址闸。
- 页面进主仓库：随 M9 的网页。在那之前网页软件照 `MIYU_WEB_PAGES` 给网页演示的页面。
- 桌面端（Tauri）：外壳照终端的样子连核心，用的是同一套通用方法；`/media` 换成它注册的自定义协议。
- 链接卡片的图随存储的回收清掉：它们是没人引用的 blob（`store.md`「还没有的」）。

### 施工步子

编号 W-1 到 W-11，2026-10-01 项目主人批准。每一步一个工作树、一张施工单。先做核心的通用方法（终端也能用，经桥的 WebSocket 照转，网页当场就能用）；身份、网页软件、媒体地址、打包等用户系统，那时照第一条的方向重画再施工。每一步合进 main，照规矩把 sha 和形状告诉两个头（终端界面、网页）。

| 步 | 做什么 | 合了网页能去掉桥的哪一块 | 先后 |
|---|---|---|---|
| W-1 | 给人看的字 `human.get` | `human.rs` 和 `web.human`。终端演示不用再自己照 `MIYU_RESOURCES` 读 | 第一步 |
| W-2 | 列文件、找文件 `fs.list`、`fs.find`，打分和清单挪进 `miyu-fs` | `mention.rs`、`resources/mention.json`、`web.files`。终端演示 `src/mention/` 里列目录、建清单、打分的那几份（`listing.rs`、`index.rs`、`fuzzy.rs`） | W-1 以后，能和 W-3 同时做 |
| W-3 | 路径：握手回应的 `host`，`fs.realpath` | `web.info`、`web.realpath`、`cwd()` | W-1 以后 |
| W-4 | mermaid：可选软件包 `mermaid`、crate `miyu-mermaid`、查询表、`mermaid.render` | `mermaid.rs`、`resources/mermaid.json`（颜色搬进页面）、`web.mermaid`、依赖 `mermaid-rs-renderer`。终端演示 `figures/mermaid.rs` 里画 SVG 的那一半和这个依赖（栅格化留着） | W-1 以后 |
| W-5 | 分块上传 `blob.open`、`blob.write`、`blob.close` | `upload.rs`、`/upload`、`web.upload_done`：页面经桥的 WebSocket 直接分块传 | W-1 以后 |
| W-6 | 分块读 `blob.get`、`fs.read` | 桥的 `media.rs` 改用它们，`history.rs` 删掉（不再读会话日志） | W-5 以后（同改 `blob.rs`） |
| W-7 | 链接卡片：可选软件包 `net`、crate `miyu-net`、`link.preview`，图存成 blob | `link_preview/` 四个文件、`resources/link_preview.json`、`web.link_preview`、`/link-image`：卡片的图照 W-6 经桥的 `/blob` 给 | W-4（查询表）、W-6 以后 |
| W-8 | 身份：第一次的一次性码、建管理员账号的引导、用户名和密码登录、记住 30 天、`connect_bare`（随用户系统重画） | 不去掉：桥还替页面出示本机令牌。用测试的头验 | 用户系统以后 |
| W-9 | 网页软件：crate `miyu-web`、`serve` 和 `open`、单实例、端口、页面文件、Host 和 Origin、WebSocket 照转、空闲退出、跳转页、主程序的 `miyu web` | 页面能经网页软件打开、登录、连核心。本机文件、blob 还靠桥（W-10），这一步演示两边都能开 | W-8 以后 |
| W-10 | 网页软件的媒体地址 `/media`：票据、分段给 | 桥整个删掉（`web-demo/bridge/`）；页面宿主的 `browser.js` 改用 `/media` | W-6、W-9 以后 |
| W-11 | 打包和安装：发行时网页软件单独一个包（第 6 题），发布前检查包里有程序和页面、程序报的版本对 | 无 | W-10 以后；页面随 M9 进主仓库，没进之前只备好打包，不发 |

### 起草时定的

技术细节照推荐定了，2026-10-01 随图纸批准。标「随用户系统重看」的几行到那时再认。

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 网页软件是一个独立的头进程，自己起停（单实例、空闲退出），不是核心拉起的扩展 | 核心重启时页面不断：网页软件接着连回核心，页面的 WebSocket 一直在（项目主人 2026-09-30 要的「核心重启以后页面自己连回来」）。核心不管它的起停，核心里也就没有它 | 核心拉起它当扩展（核心一退它也退，页面跟着断；核心要管它的命） |
| 2 | 主程序留一个 `miyu web`，只找主程序旁边的 `miyu-web`、把参数交给它 | 设计 22 第五节有这条命令；主程序里不放网页的代码。没装的说怎么装 | 主程序里写全部（主程序就带着网页的代码）。只有 `miyu-web` 一个命令（和别的命令不在一处） |
| 3 | 一个标签页一条核心连接，网页软件不合并 | 照桥；核心分得清是哪个终端做的（web/architecture.md「多用户、多终端」第 7 条） | 合成一条（核心分不出终端，回应要网页软件自己分） |
| 4 | （随用户系统重看）网页软件转发的连接用 `connect_bare`，代码里不读本机令牌；只有 `miyu-web open` 要登录码那一下照终端出示 | 第 1 题推荐的做法落到代码上：网页软件被攻破、有 bug，也成不了你。测试照源码查 | 照 `connect` 读出令牌、不用它（拿在手里就可能被用） |
| 5 | （随用户系统重看）登录码、登录令牌都是 32 个随机字节、64 位小写十六进制；登录令牌只存 SHA-256 | 随机的 256 位猜不到，哈希够了；设计 06 U4 的 argon2id 是给人起的密码的 | 登录令牌也用 argon2id（慢，白花力气） |
| 6 | （随用户系统重看）握手三选一：`token`、`code`、`login` | 身份只在握手时定，一处验（`protocol.md`「握手」）；网页软件原样转，不用懂 | 先连上、再 `account.login`（握手以后还有一段没认人的状态） |
| 7 | （随用户系统重看）设计 04 方法表的 `account.login_link` 改叫 `account.login_code`，只交登录码 | 核心不知道网页开在哪个端口，出不了链接；链接由网页软件拼 | 核心出整个链接（核心要知道网页的事） |
| 8 | （随用户系统重看）登录令牌存在 `home/<账号>/logins.json` | 人的数据放进人的家目录（设计 07 第二节第 1 条）；核心重启不丢，页面自己连得回来 | 只在内存（核心一重启，开着的页面都要重新 `miyu web`） |
| 9 | 附件由页面经核心的分块上传直接传，网页软件不收附件 | 一条路：远程的头、桌面端都一样；网页软件不存临时文件、不用「传完删」 | 页面 POST 给网页软件、它再分块转给核心（多一份代码、多一份临时文件，原来的交代写着网页软件收附件，这里改了） |
| 10 | 分块上传三个方法，顺序写，跟着连接走，60 秒不写作废，暂存在 `blobs/tmp/`，一块最多 512 KiB | 顺序写最简单，断了照 `received` 接着传；跟着连接走就不会留下没人管的暂存；放在 `blobs/tmp/` 改名进位置和 `Blobs::put` 同一个办法；512 KiB 的 base64 放得进 1 MiB 的一行 | 一个 `blob.put` 加分块的格（一个方法两种用法）。乱序写 |
| 11 | 分块读也是一块最多 512 KiB，`length` 写 0 只问大小 | 同上；问大小不另开方法 | 另开 `blob.stat`、`fs.stat` |
| 12 | blob 照属主给，不照会话 | 核心认账号，账号的 blob 本来就是它的（设计 06 U2）。桥照会话查，是因为它不知道是谁；照会话查要读整份日志 | 照会话查（分享 U12 来了再加） |
| 13 | （随用户系统重看）媒体地址用票据：带登录令牌 POST 换一张，票据只管一个资源、12 小时不用作废、只在网页软件的内存里。不用 cookie，登录令牌不写进地址 | `127.0.0.1` 上 cookie 不分端口：模型写一张 `http://127.0.0.1:<别的端口>/` 的图，浏览器就把 cookie 带给那个端口，沙盒里的命令能在那里听着（RFC 6265 第 8.5 节）。登录令牌写进地址，复制一下图片地址就漏了，贴进对话还会进日志 | cookie。地址里带登录令牌。核心发票据（核心就有了只为浏览器的东西） |
| 14 | 链接卡片的图存成账号的 blob，经同一个 `/media` 给；网页软件不另做图片代理 | 抓网上东西的只有核心一处，地址闸只有一份；终端要显示卡片的图也照 `blob.get` | 网页软件自己代理图片（再写一份地址闸）。图只在核心内存里（要另开读图的方法） |
| 15 | 主机信息放进握手的回应 `host`，不另开方法 | 头一连上就要（写 `~/…`、认平台），少一个来回；`workspace` 跟着账号变，握手时已经知道是谁 | 另开 `host.info` |
| 16 | `fs.list`、`fs.find` 的 `path` 一律用 `/`，`full` 照平台 | 头拿 `path` 显示、打分，拿 `full` 交回核心；两种写法各管一样 | 都照平台（头要分平台打分） |
| 17 | 数据根不列不读，账号的工作区除外 | 照 `fs.md` 的边界表；桥整个数据根都不列，默认的会话就在工作区里，它列不出来 | 照桥 |
| 18 | 找文件的清单没建完先给建好的那部分，头隔 200 毫秒再问 | 照终端演示：大目录不卡；不用推送，查询就是查询 | 等建完再回（大目录要等好几秒）。建好了推送 |
| 19 | 可选软件包登记查询：端点多一张查询表，核心起来时照编进来的包往里登记 | 加东西只登记，不改中心；没编进来就没有这一行，回 `unknown_method` | 端点里写死 `#[cfg]` 的分支 |
| 20 | `mermaid`、`net` 是编译时的开关，发行版默认打开 | 没装的核心里真的没有这块代码；现在还没有软件包管理，编译时的开关就是「装」。有了 `miyu pkg` 再照设计 10 第四节停用 | 单独的 worker 程序（多一个进程、一套 stdio 的说法、一份起停；画图的库是纯 Rust，用不着）。一直编进去、停用了不构造（代码还在） |
| 21 | mermaid 先做一个只出 SVG 的方法 `mermaid.render`，`view.detail` 随 M9 | 视图投影还没有，头现在就要图；M9 的 `view.detail` 用同一个画法、同一份缓存，头到时候换不换都行 | 现在就做 `view.detail`（要先有视图条目，那是 M9 的一大块） |
| 22 | mermaid 的颜色用三种记号色，回应里写明，头自己换 | 一张 SVG 给所有头，缓存只照源码；网页换成 CSS 变量，终端换成主题色再栅格化 | 头传颜色进来（缓存要照颜色分，同一张图画好几次） |
| 23 | mermaid 这一块先写在这一页，施工 W-4 时另立 `docs/blueprint/mermaid.md`，这一页只留指过去的一句 | 一页一个部件，和 crate 对得上；现在写在一起，项目主人一次批 | 现在就另立一页 |
| 24 | HTTP 用 `hyper`（只开 HTTP/1 的服务端），WebSocket 用 `tokio-tungstenite` | 桥是手写的解析，正式的软件不手写协议；两个都是 MIT，树里已经有 `tokio`。许可证门禁查 | 手写（照桥）。`axum`（多一层用不上的路由） |
| 25 | 网页软件的数（端口、空闲多久、票据、媒体类型、内容安全策略）放在 `resources/web/web.json`，不进核心的配置清单 | 进了核心的配置清单，核心就有了为网页写的东西；软件包的清单能声明配置项以后再搬（设计 05 第二节） | 核心替它声明（照 `tui.startup` 的先例） |
| 26 | 网页软件只听 `127.0.0.1`，Host 只认三种写法，Origin 对上才接 WebSocket | 远程默认关（设计 04 P6）；Host 挡 DNS rebinding，Origin 挡别的网站 | 也听 `::1`（`localhost` 两种都通，代价是两个监听） |
| 27 | 页面文件不要登录也给 | 里面没有秘密；登录要先有页面 | 页面也要票据 |
| 28 | 网页软件起停照核心的样子：`run/web.lock`、`run/web`、`ready` 那一行 | 头已经会拉起核心、等那一行，同一套代码 | 一直前台跑（照桥，关了终端就没了） |
| 29 | `fs.*` 的出厂数写在代码里，配置那一步能改 | 照 `jobs.output_chars` 的放法；这几个数终端演示定过（设计 13 H14） | 现在就做成配置项（每一项要三种语言的名字和说明） |
| 30 | `logins.json` 最多 64 行，过期的写的时候删 | 文件不会一直长；写的时候顺手删，不另起清理 | 不设上限 |
| 31 | `link.preview` 在后台的任务里抓，回应照 `id` 对上，不挡这个连接后面的请求 | 抓一页要几秒；一个连接上的请求本来一条条办，页面上别的都会卡住 | 照一条条办（卡片多的时候整个页面卡几秒） |
| 32 | `human.get` 每次现读资源目录 | 一个连接只要一次，读几份 JSON 很快；开发时改了资源不用重启核心 | 核心起来时读好记着 |

### 项目主人拍板的

2026-10-01 问过的六题，项目主人当天答了：

1. **本机打开的网页，身份由谁验**：核心验，网页软件只转发、不读本机令牌。项目主人另外定了登录的样子：第一次是一个一次性码，进网页有一个建管理员账号的引导；建好了码作废，以后用用户名和密码登录。现在还没有用户系统，W-8 到 W-11 等它（「是什么」末尾）。
2. **网址和登录记多久**：固定端口（出厂值写在网页软件的数据里，`--port` 能换），浏览器记住登录 30 天。
3. **登录码怎么交给浏览器**：不用跳转页。一次性码只用一次、建了管理员账号就作废，在命令行里出现没关系；以后用用户名和密码登录，命令行里没有能登录的东西。
4. **网页能看本机哪些文件**：和她读文件一样，整盘能看，数据根不行（账号的工作区可以）。
5. **链接卡片走不走代理**：走，和请求模型一样照环境变量（「怎么走」第八条第 5 款）。代理那头把名字解析到内网的口子认了。
6. **网页软件怎么装**：同一个仓库，单独的程序 `miyu-web` 加页面文件，发行时单独一个包（AUR、deb、rpm、Homebrew 各一个 `miyu-web`），安装脚本问一句装不装。

先后：W-1 到 W-7 现在做，W-8 到 W-11 等用户系统（项目主人 2026-10-01 定）。

### 要改的设计

项目主人批准以后照这个改，这一页不改设计文件本身。

**`04-核心协议.md` P5「网页的页面由谁提供」**
- 原文要点：已定「核心里的可选 HTTP 模块」，只在启用网页时构造，页面文件从安装目录读；未选「独立的网页网关进程」，理由是多一个进程、多一跳转发；重开条件「网页需要的能力让核心明显变重」。
- 改成：已定「网页界面是一个单独的软件」（程序 `miyu-web` 和页面文件），装了才有；它是一个头，自己开端口、给页面，把 WebSocket 转成核心协议，经本机套接字（Windows 上是命名管道）连核心；核心里没有为网页写的代码，网页要的、终端也用得上的做成通用的协议方法（2026-10-01 项目主人定）。未选「核心里的 HTTP 模块」：没开网页也带着它的代码，网页的东西进核心就是核心往上依赖一个界面。原来的顾虑「多一个进程、多一跳转发」：本机多一跳，代价小。重开条件：实测本机多一跳的延迟人感觉得到。

**`04-核心协议.md` 其余几处**
- 第二节传输表「浏览器、远程：WebSocket，每帧一条」改成：浏览器连网页软件（WebSocket，每帧一条），网页软件连核心（本机套接字、命名管道，一行一条）。「各平台的坑」最后一条「核心必须自己检查 Origin 头」改成：网页软件检查 Origin 和 Host。
- 第四节身份：「本机连接要…出示本机令牌」改成：本机的头出示本机令牌；浏览器出示一次性登录码或者登录令牌；都由核心在握手时验，网页软件只转发、不读本机令牌。
- 第五节 mermaid：「头经 `view.detail` 按需取」后面补：视图投影做出来以前经 `mermaid.render` 取；画图是可选的软件包 `mermaid`，没装的头显示源码。
- 第九节方法表：加 `human.get`、`fs.list`、`fs.find`、`fs.realpath`、`fs.read`、`blob.open`、`blob.write`、`blob.close`、`blob.get`、`mermaid.render`、`link.preview`；`account.login_link` 改叫 `account.login_code`；`account.logout`（表里已经有）写明多一格 `all`。
- 第十节「后续再定」的「附件的大小上限，以及分块上传」：定了，上限照 `blob.put` 的 20 MiB，分块三个方法。

**`21-网页.md` 第一节第 3 条**
- 原文要点：页面由核心里可选的 HTTP 模块提供，只有开了网页才构造它；页面文件放在资源目录里，不编进二进制。
- 改成：网页界面是一个单独的软件，装了才有：程序 `miyu-web` 加页面文件，页面文件放在它的资源目录 `web/` 里，不编进二进制。它是一个头，经核心协议连核心。

**`21-网页.md` X5「页面文件放在哪」**
- 原文要点：已定「放在资源目录里，由核心的 HTTP 模块提供，不编进二进制」；未选编进二进制。
- 改成：已定「放在网页软件的资源目录 `web/pages/` 里，由网页软件提供，不编进二进制」；开发时 `MIYU_WEB_PAGES` 指到别处。未选：编进二进制（照旧）；由核心的 HTTP 模块提供（P5 改了）。

**`21-网页.md` X6「本机怎么证明是你」**（随用户系统改，照第 1、3 题重写）
- 原文要点：`miyu web` 打开一条一次性的登录链接，用过就换成登录令牌；远程用账号密码；链接 5 分钟、一次；令牌在 `#` 后面，页面拿到就从地址栏抹掉；未选本机不设防。
- 改成：保留这几条，写明谁做哪一段：`miyu web` 照终端的样子出示本机令牌，向核心要一次性登录码（`account.login_code`）；网页软件拼成链接，经数据根里的跳转页交给浏览器，登录码不进命令行（第 3 题）；页面握手时出示登录码，核心验过换成登录令牌，30 天（第 2 题）；网页软件只转发，不读本机令牌（第 1 题）。

**`21-网页.md` 第七节「本机打开」「远程」**
- 原文要点：本机打开靠 `miyu web` 的一次性链接（浏览器读不到本机令牌）；远程必须登录，核心检查 WebSocket 的 Origin 头。
- 改成：本机打开同 X6；网页软件只听回环地址，核对 Host、Origin，不设 cookie，本机文件和 blob 经票据地址给（`/media`）。远程：每条连接的身份由核心在握手时验，网页软件只转发，不能拿本机令牌替远程的人登录（2026-10-01 项目主人定）；Origin 由网页软件查。

**别的设计**（照改，细节见「要跟着改的别的页」）
- `12-进程形态与分发.md`：第一节的图里「浏览器里的网页 → 核心」改成经网页软件，「网页头的页面由核心顺带提供」改掉；第三节 R2 加「网页界面是单独的程序 `miyu-web`，随网页软件装」，资源目录那一句的「网页」改成网页软件自己的资源。
- `22-命令行.md` 第五节 `miyu web`：交给网页软件，没装的说怎么装。
- `06-多用户与身份.md` 第二节、U4：随用户系统那次讨论改（第一次一次性码建管理员、以后密码登录、记住 30 天）。
- `10-自带软件.md` 第四节：可选软件包加「画 mermaid」；`net` 多一样「链接卡片 `link.preview`」。
- `13-终端界面.md` 第九节：SVG 由核心经 `mermaid.render` 出（`view.detail` 随 M9）。
- `01-架构.md` 第九节：登记 `miyu-mermaid`、`miyu-net`（第 3 层）、`miyu-web`（第 5 层）。
- `07-存储.md` 第二节：`home/<账号>/logins.json`，`run/web.lock`、`run/web`、跳转页。
- `24-威胁模型.md` 第二节：加三行。本机别的进程（包括沙盒里的命令）连网页软件的端口：页面谁都拿得到、里面没有秘密，WebSocket、媒体要登录令牌或票据，Host、Origin 挡 DNS rebinding 和别的网站；挡不住：浏览器里的页面有能执行脚本的漏洞。进程列表里的一次性码：只用一次、建了管理员就作废（第 3 题，随用户系统）。网页软件被攻破：它没有本机令牌，只看得到经它转的登录令牌。

### 要跟着改的别的页

这次不改，施工时照步改：

- `protocol.md`：握手（参数三选一、回应的 `host` 和 `login`、第 3 条）；方法表、每个新方法一段；「一个连接」第 1 条（`link.preview` 在后台答的例外）；出错的表、给人看的字、运行日志、守着它的。W-1 到 W-8。
- `ipc.md`：`connect_bare`、`connect_or_start_bare`；「还没有的」第 1 条改成指到这一页（WebSocket、Origin 在网页软件）。W-8。
- `core.md`：起来的先后里登记可选软件包、清分块上传的暂存；「在哪」加 `packages.rs`。W-4、W-5、W-7。
- `store.md`：blob 的分块暂存和读一段（第 9、10 条）；`logins.json`；`run/` 下网页软件的三样。W-5、W-6、W-8、W-9。
- `store/resources.md`：`Human` 交出模板原文；资源目录里多 `software/mermaid/`、`software/net/`、`web/`；「还没有的」那条「网页、字体这类资源」改掉。W-1、W-4、W-7、W-9。
- `fs.md`：列一层、建清单、打分、从最近在的一层换真实位置、读一段。W-2、W-3、W-6。
- `http.md`：写明 `miyu-net` 另有自己的客户端（不走代理的钉地址；走代理的先在本机解析过闸），不经 `miyu-http`；代理照同一套环境变量。W-7。
- `log.md`：目标多 `miyu::web`、`miyu::net`、`miyu::mermaid`；网页软件自己的 `state/logs/web.log`。W-4、W-7、W-9。
- `licenses.md`：新依赖 `mermaid-rs-renderer` 和它带的字体库、`hyper`、`tokio-tungstenite`，`ignore` 挪进 `miyu-fs`。W-2、W-4、W-9。
- `cli/main.md` 加子命令 `web`；新页 `cli/web.md`。W-9。
- 新页：`mermaid.md`（W-4，这一页第五条挪过去）、`net.md`（W-7，第八条挪过去）、网页软件一页 `web-ui.md`（W-9，第九、十、十一条挪过去；名字施工时定，不和 proto/web-demo 分支的 `web.md` 撞）。README 的页表跟着加。
- README 的页表：这一页一行（这次加了），施工时照做好的改状态。
- proto/web-demo 分支的 `docs/blueprint/web.md`：「在哪」的桥那一行、「连核心」、给人看的字、`@` 选文件、mermaid、读本机文件、附件、链接卡片几节照新方法改；「和设计 21 的出入」那张表照这一页改。`docs/blueprint/web/architecture.md`「宿主」：`urls` 改成 `/media` 的票据、`files.stage` 改成分块上传、「头这边顶替的查询」那一行删掉。网页演示的会话改。
- proto/tui-demo 分支的 `docs/blueprint/tui.md`：「图片、公式和 mermaid 图」（SVG 由核心出，「和设计 13 的出入」里 mermaid 在头里出图那一条删掉）、「`@` 文件列表」（经 `fs.list`、`fs.find`）、给人看的字（经 `human.get`，`src/language.rs` 不再照 `MIYU_RESOURCES` 读）。终端演示的会话改。
- 设计文件：见「要改的设计」。`docs/designs/26-提示词.md` 不动：这条线不加给模型看的字。
- `docs/construction/README.md` 第三节、`施工图.html`：这条线的步子。批准以后。
