## 施工单 W-7（补）：压缩的页面和 head 后面的标签

状态：待施工（2026-10-02，项目主人在终端界面实测链接卡片时撞到，终端界面的会话转来；主会话查了原因、定了修法）。

### 目的

两种站做不出卡片（运行日志 `link preview failed … why=no_preview`）：

1. **B 站**：B 站不管请求带不带 `Accept-Encoding`，都用 gzip 压着发页面（主会话 2026-10-02 用 curl 查过：不带 `Accept-Encoding` 也回 `content-encoding: gzip`）。`miyu-net` 用的 reqwest 没开解压，拿到的是压过的字节，里面自然找不到标签。终端那边以为是反爬的空壳，不是：解压以后有 `og:title`、`<title>`。
2. **YouTube**：`og:*` 那几个 `<meta>` 放在 `</head>` 后面（`</head>` 在第 71.8 万字节左右，`og:title` 在 77.3 万左右），整页 2.12 MB。我们读到 `</head>` 或者 `<body` 就停，一个都没拿到。

修法（主会话定）：
- **解压**：`miyu-net` 的客户端打开 reqwest 的解压（`gzip`、`brotli`、`deflate`、`zstd` 几个特性），自动带 `Accept-Encoding`、自动解开。页面最多读多少（`max_bytes`，2 MiB）照**解开以后**的字节算，压缩炸弹也挡得住；图同理。
  - reqwest 的特性在整个工作区是合起来的：开了以后，`miyu-http`（请求模型、拉目录）的客户端也会自动带 `Accept-Encoding`。为了请求模型那条路一个字节都不变，`miyu-http` 造客户端时明确关掉（`no_gzip`、`no_brotli`、`no_deflate`、`no_zstd`），写注释说为什么。
  - 新特性带进来的依赖过许可证门禁。
- **head 后面的标签**：读到 `</head>` 或者 `<body` 时，要是已经找到 `og:title`，照旧停；还没找到的，接着往下读，只看 `<meta …>` 和 `<link rel=icon …>`，找到 `og:title` 就停，最多读到 `max_bytes`。正文别的东西照旧不看。
  - 只有 `<title>`、没有 `og:title` 的普通网页，照旧读到 `</head>` 停（`<title>` 已经够做卡片），不为它们多下载。判断写成：读到 `</head>`/`<body` 时，有 `og:title` 停；没有 `og:title` 但有 `<title>` 也停；两个都没有才接着读。
- `link_preview.json` 里的数不变。

### 蓝图改哪几节

- `docs/blueprint/net.md`：「怎么走」讲页面读到哪停的那一款、讲客户端的那一段（解压、上限照解开以后的算）；「施工时定的」记这一次。
- `docs/blueprint/http.md`「客户端」：写明 `miyu-http` 关掉自动解压、为什么。
- 先例：`crates/miyu-net/src/body.rs`（读页面、读到哪停）、`fetch/clients.rs`（造客户端）、`html.rs`；`crates/miyu-http/src/client.rs`。

### 不做什么

- 给某个站写专门的规则（B 站的接口之类）：不用，解压就够了。
- 维基百科在项目主人那里连不上：那是网络，不是这一步的事。

### 验收

1. 测试（先写，退回改之前的代码要红）：
   - 本机假服务器回一份 gzip 压过的页面（带 `content-encoding: gzip`），请求不带也照样压：卡片做得出；br 压过的同样；
   - 解开以后超过 `max_bytes` 的：照上限停，不整份解开（测一个很小的压缩包解开后很大的）；
   - `og:*` 放在 `</head>` 后面、`<head>` 里没有 `<title>` 的：读下去找到，卡片做得出；`og:*` 放在一个超过 `max_bytes` 的位置：照上限停、`no_preview`；
   - 只有 `<title>` 的普通页面：读到 `</head>` 就停（假服务器记下被读了多少，或者 `</head>` 后面跟一大段让测试看得出没读）；
   - `miyu-http` 的客户端不带 `Accept-Encoding`（假服务器记下请求头）：请求模型那条路没变；请求形状探针零变化。
2. 给模型看的字：没有。
3. 真网络（主会话合并前做）：一个 B 站视频页、一个 YouTube 频道页都做得出卡片。
4. 手写变异 10 个左右，挑关键的，全被逮住；`CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑全绿。
