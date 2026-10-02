## 施工单 W-7（补）：压缩的页面和 head 后面的标签

状态：施工完，待主会话实测（2026-10-02，项目主人在终端界面实测链接卡片时撞到，终端界面的会话转来；主会话查了原因、定了修法）。

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

### 验收结果（2026-10-02）

1. **代码改了哪几处**：
   - `crates/miyu-net/Cargo.toml`：reqwest 多开 `gzip`、`brotli`、`deflate`、`zstd` 四个特性。
   - `crates/miyu-http/src/client.rs`：`build()` 里加 `.no_gzip().no_brotli().no_deflate().no_zstd()`，带注释说明是防着 `miyu-net` 那四个特性把这个客户端也带起来。
   - `crates/miyu-net/src/body.rs`：新加私有结构 `HeadSignals`，边读边记「看到 `<title` 了没有」「看到不空的 `og:title` 了没有」，一个标签只处理一次（游标往前挪，不整段重扫）。`read_head` 改成：读到记号那儿先只喂记号前面那一截给 `HeadSignals` 判断够不够；够了照旧截断、停；不够的记下记号位置、继续读，这回喂全部攒到的字节，一旦 `og_title` 变 `true`（并且已经过了记号）就停，字节上限的检查完全不变。
   - `crates/miyu-net/src/html.rs`：`extract()` 里原来碰到 `/head`、`body` 标签名无条件 `break` 的那一行，改成只在「已经挖到 `og:title` 或者 `<title>`」时才 `break`；新加 `pub(crate) fn meta_is_og_title(tag)`，判断一个 `<meta>` 标签是不是不空的 `og:title`（`property` 或者 `name` 当键，`body.rs` 和 `extract()` 共用这个判断，不重复写一遍属性解析）。
   - `crates/miyu-net/src/fetch.rs`：`page()` 的文档注释提一句新行为、指到 `body::read_head`。
2. **测试**（先写骨架，但由于「读到哪停」这部分代码量不大，是边写测试边写实现的，退回去逐个验证过——见下「变异」和每条测试后面附的红绿记录）：
   - `crates/miyu-net/src/body/tests.rs` 新加 6 条：`HeadSignals` 的 `<title>`、不空的 `og:title` 各自够不够；空内容的 `og:title` 不算够；`name`/`property` 互换和属性先后不管；标签已经处理过不重扫（断言 `scanned` 往前挪）；标签跨在两次喂的接缝上，第一次没收尾不算、第二次收尾了才算。
   - `crates/miyu-net/src/html/tests.rs` 新加 3 条：`<head>` 里什么都没有、`og:title` 挪到 `<body>` 后面的找得到（连 `og:image` 一起）；`<head>` 里已经有 `<title>` 的不去正文找（正文里的 `og:title` 不算）；`meta_is_og_title` 本身的键/退路/空内容判断。
   - `crates/miyu-net/tests/support/mod.rs` 新加：`Reply::html_encoded(text, "gzip"|"br")`（压一份、带上 `Content-Encoding`）、`Reply::with_header`、`Seen.accept_encoding`（记请求头）、`gzip()`/`brotli()` 两个压缩的小工具函数（新 dev-dependencies：`flate2`、`brotli`，都是 reqwest 开的那几个特性本来就带进依赖图的包）。
   - `crates/miyu-net/tests/preview.rs` 新加 4 条：
     - `gzip_and_br_pages_are_decoded_even_without_asking_for_it`：假服务器不管请求头，一律压着回，gzip、br 各测一个站，卡片都做得出。
     - `a_small_gzip_bomb_is_capped_at_two_mib_decoded_not_fully_inflated`：一份压得很小（20 MiB 的 `x` 压完不到 200 KiB）、解开很大的页面，标题在前面的（早于 2 MiB）读得到——证明真的解开了，不是读到压过的字节就直接判「没有标题」；标题在填料后面、总共远超 2 MiB 的读不到——证明上限照解开以后的字节算，不是照 `Content-Length` 这个压过的小数。
     - `an_og_title_after_head_is_found_and_past_the_cap_is_not`：`<head>` 空的、`og:title` 挪到 `<body>` 后面的找得到；同样的样子但 `og:title` 挪到 2 MiB 以后的，照上限停，`no_preview`。
     - `an_early_og_title_does_not_stop_reading_the_rest_of_the_head`：`og:title` 排在 `<head>` 里靠前，`og:image` 排在后面、隔着 256 KiB 的填料——这条不在原计划的验收清单里，是写变异（见下第 10 条）时顺手发现的一个真实的行为缺口，补了测试再补了判断（`marker.is_some()` 这个条件）。
   - `crates/miyu-http/tests/send.rs`、`get.rs` 各加一句断言：`request.header("accept-encoding")` 是 `None`。这两条用 `-p miyu-http -p miyu-net` 一起编才有意义——单独 `-p miyu-http` 时 reqwest 根本没编 gzip 这几个特性，断言恒真，不能证明什么；门禁的「测试」一项是 `cargo test --workspace`，天然覆盖这种情况。
   - 请求形状探针（`crates/miyu-assemble/tests/probe*.rs`）本身比对的是组装出来的请求字节，不摸真的 HTTP 头，这一步没碰那部分代码；单独 `cargo test -p miyu-assemble --quiet` 跑过一遍，104+9+… 全过，字节样本一个没变。请求模型这条路字节不变，真正靠的是上面两条 `accept-encoding` 断言。
3. **给模型看的字**：没有新加。没碰 `tools/descriptions/*.json`、提示词、任何模型可见的字符串。
4. **真网络**：留给主会话合并前做（施工单第 3 条本来就写明「主会话合并前做」），这一步没有自己连外网。
5. **手写变异 10 个，全部逮住**（一次改一处，逮住就复原，`diff` 核对过复原后和改前逐字节一样）：

   | # | 改的地方 | 怎么改 | 逮住它的测试 |
   |---|---|---|---|
   | 1 | `body.rs::HeadSignals::enough` | `self.title \|\| self.og_title` 改成 `false` | `body::tests::head_signals_catch_a_title_tag`、`head_signals_catch_a_non_empty_og_title` |
   | 2 | `body.rs::HeadSignals::inspect` | `self.title = true;` 删掉 | `body::tests::head_signals_catch_a_title_tag` |
   | 3 | `body.rs::HeadSignals::inspect` | `html::meta_is_og_title(...)` 前面加 `false &&` | `head_signals_catch_a_non_empty_og_title`、`head_signals_accept_name_and_reordered_attributes`、`head_signals_do_not_rescan_a_tag_already_processed`、`head_signals_hold_an_unterminated_tag_for_the_next_feed`（4 条） |
   | 4 | `html.rs::meta_is_og_title` | 去掉 `.or_else(\|\| attribute(&attrs, "name"))` | `body::tests::head_signals_accept_name_and_reordered_attributes`、`html::tests::meta_is_og_title_checks_the_key_and_a_non_empty_content` |
   | 5 | `html.rs::meta_is_og_title` | 去掉内容不空那半句，只留 `key == "og:title"` | `body::tests::head_signals_ignore_an_empty_og_title`、`html::tests::meta_is_og_title_checks_the_key_and_a_non_empty_content` |
   | 6 | `html.rs::extract` 新加的 `break` 条件 | `\|\|` 改成 `&&` | `html::tests::a_plain_title_in_head_stops_the_scan_before_the_body`（标题会被正文里的 `og:title` 顶掉） |
   | 7 | 同上 | `document_title` 错写成 `twitter_title` | 同上 |
   | 8 | `body.rs::read_head` | `let room = max.saturating_sub(body.len());` 改成 `let room = max;`（上限不再随读到的字节收紧） | `preview.rs` 三条：`an_og_title_after_head_is_found_and_past_the_cap_is_not`、既有的 `the_head_is_read_until_its_end_and_at_most_two_mib`、`a_small_gzip_bomb_is_capped_at_two_mib_decoded_not_fully_inflated`——连既有的测试也红了，没有削弱原来的上限测试 |
   | 9 | `miyu-http/src/client.rs::build` | 去掉 `.no_zstd()`，只留前三个 | `miyu-http` 的 `send.rs`、`get.rs` 两条断言（都要 `-p miyu-http -p miyu-net` 一起编） |
   | 10 | `body.rs::read_head` | 停下来的判断 `marker.is_some() && signals.og_title` 去掉 `marker.is_some() &&` | 当时用现成的测试一个都没逮住——因为本机假服务器的小页面都在一次 TCP 读里到齐，「提前收手」和「读到流尾」看不出差别。补了 `an_early_og_title_does_not_stop_reading_the_rest_of_the_head`（`og:title` 早、`og:image` 晚、隔着 256 KiB 填料）才逮住：没有这个条件，`og:title` 一找到就收手，`<head>` 里排在后面的字段会丢 |

   第 10 条是唯一一次「先没逮住、再补测试」，补的测试已经留在 `preview.rs` 里（见上「测试」第 4 条），不是临时脚本。
6. **门禁**：`CARGO_BUILD_JOBS=5 cargo xtask check` 跑了两遍，第一遍「格式」没过（`cargo fmt --all` 自动改了几处换行，没有逻辑改动），`cargo fmt` 以后第二遍八项全过（格式、clippy、文档、分层、纯逻辑、行数、许可证、测试）。许可证那一项顺带确认了新依赖（`async-compression`、`tower-http`、`flate2`、`brotli`、`brotli-decompressor`、`zstd`、`zstd-safe`、`zstd-sys`、`compression-codecs`、`miniz_oxide`、`crc32fast`）全是 MIT/Apache-2.0/BSD-3-Clause，和 GPL-3.0-or-later 合得到一起。
7. **三台机器的 CI 和长跑**：待推送以后看。

### 施工时定的（2026-10-02 施工时定）

- `body.rs` 判断「够不够做卡片了」用一个边读边记的小结构（`HeadSignals`），不是读完整个累积的字节串以后调一次 `extract()` 之类的函数再判断：后者在遇到坚持不给够信息的页面时，会对着越攒越大的 `<head>` 反复整段重新扫一遍（服务器分几次、小块地发数据时尤其明显），是这一步要修的「压缩炸弹」同一类资源耗尽风险。`HeadSignals` 内部记着扫到哪个字节了，喂第二次只接着扫新收尾的标签，全程一个字节只被当成标签内容看一次。
- `HeadSignals` 判断 `<meta>` 是不是 `og:title` 时，直接调 `html::meta_is_og_title`（`html.rs` 新加的 `pub(crate)` 函数），复用 `attributes()`/`attribute()` 这两个已经测过的属性解析，不在 `body.rs` 里另写一遍小的属性解析器——这两个函数本来就是模块私有、同一个 crate 内随便互相调。
- `read_head` 发现记号（`</head>`/`<body`）时，先只把记号前面那一截喂给 `HeadSignals` 做「够不够」的判断，再把全部攒到的字节（包含记号后面新收到的那部分）喂一遍：这样「要不要截断在记号这里」这个判断只看记号之前的内容，不会被同一块网络数据里记号后面碰巧出现的 `og:title`（真实场景里，正文里的小部件、分析脚本常有这类凑巧撞上的标签）提前误判成「够了」。
- `an_early_og_title_does_not_stop_reading_the_rest_of_the_head` 这条测试里，`og:title` 和 `og:image` 之间塞了 256 KiB 的填料，不是用停顿/分块发送（`Reply` 结构现在不支持），而是单纯让页面大到本机 TCP/hyper 的内部缓冲一次读不完，天然分成多次 `.chunk()`。这比给测试用的假服务器加「分片+等待」的新能力要轻：构造完测过，不加这条就确实逮不住变异 10，加了就逮得住。
- 测试用的压缩工具函数直接调 `flate2`、`brotli` 这两个包，不是另起一个脚本或者外部 `gzip`/`brotli` 命令行工具现场压：两个包已经在 `cargo tree` 的依赖图里（reqwest 开的那四个特性带进来的，`async-compression` 的后端），版本不用自己盯，许可证也已经在门禁里核过。
