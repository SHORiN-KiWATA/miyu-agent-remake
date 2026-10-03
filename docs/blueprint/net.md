## 网：链接卡片

### 是什么

核心去抓别人家网页的地方。做成可选的软件包 `net`：crate `miyu-net`，经 `miyu-core` 的 cargo 开关 `net` 编进来，发行版默认打开。现在只有一件事：`link.preview`，头给一个网址，核心抓下那一页的标题、简介、图，交回一张卡片。认得的几个站（B 站、YouTube、MediaWiki 站）照各自的办法取，卡片多出是什么（视频、文章）、视频多长、作者是谁；人机验证、挡爬虫的页面不出卡片（W-7 再补）。以后 `web_fetch` 用同一份抓取和地址闸（设计 10 第四节）。没编进来的核心里没有这块代码，`link.preview` 回 `unknown_method`，头照原样留着链接。

地址闸是安全边界：网址是模型写的、别人发来的，核心照它去连，等于把 SSRF 的靶子摆在核心上。闸只有这一份，网页软件不另做图片代理（`web-module.md`「起草时定的」第 14 条）。

这一页原来写在 `web-module.md`「八、链接预览」，施工 W-7 时搬过来。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-net/`（第 3 层，执行器） | 地址闸、钉住解析好的地址、代理、自己跟跳转、读到 `</head>`、挖元数据、认图 |
| `crates/miyu-net/src/lib.rs` | `LinkPreview`：懒读规矩、记着抓过的、图存成 blob、交回卡片 |
| `crates/miyu-net/src/rules.rs` | 读 `link_preview.json` |
| `crates/miyu-net/src/guard.rs` | 地址闸三层：样子、解析、IP 段 |
| `crates/miyu-net/src/proxy.rs` | 这一跳走不走代理：照环境变量，和请求模型同一套读法 |
| `crates/miyu-net/src/fetch.rs`、`fetch/clients.rs` | 一跳一跳地抓：每一跳过闸、钉地址或者交给代理、自己跟跳转；客户端复用 |
| `crates/miyu-net/src/body.rs` | 读到 `</head>` 或者 `<body` 为止，YouTube 读到要的几样都有为止；读一张图、一份接口的回应；照开头的魔数认图 |
| `crates/miyu-net/src/html.rs` | 挖元数据：标题、简介、图、站名、图标；找某一个标签的某一格（站要的那几样） |
| `crates/miyu-net/src/sites.rs` | 按站取：照地址认站、挑读页面的办法、人机验证页、把各站取到的合成一张卡片（W-7 再补） |
| `crates/miyu-net/src/sites/bilibili.rs` | B 站：认 BV、av、短链；读页面脚本里的视频数据 |
| `crates/miyu-net/src/sites/mediawiki.rs` | MediaWiki 站：认站、页面名、`api.php` 在哪、读回应、第一段 |
| `crates/miyu-net/src/sites/youtube.rs` | YouTube：ISO 8601 的时长换成秒 |
| `crates/miyu-net/src/rules/sites.rs` | 读 `link_preview.json` 的 `api`、`sites`、`challenge` 三段 |
| `crates/miyu-net/src/remember.rs` | 抓过的记多久、满了整个清空 |
| `crates/miyu-net/src/testkit.rs` | 测试用的口子：回环当公网、只照一张表解析、代理照给的值。`testkit` 开关打开才编进去 |
| `crates/miyu-endpoint/src/queries.rs` | 登记的查询多一种「在后台答」：`register_background` |
| `crates/miyu-endpoint/src/connection.rs` | 在后台答的方法交给一个后台任务，回应照 `id` 对上；连接断了这些任务一起停 |
| `crates/miyu-core/src/packages.rs`、`packages/net.rs` | 登记 `link.preview`；参数怎么读、卡片怎么写成回应 |
| `resources/software/net/link_preview.json` | 规矩：时限、上限、请求头、记多久、客户端留多久；认得的站、人机验证页的认法（W-7 再补） |

### 对外的样子

**`link.preview`**：握手以后，都能调。

| 参数 | 类型 | 说明 |
|---|---|---|
| `url` | 字符串，必写 | 网址 |

回应二选一：

```json
{"card":{"description":"…","icon":{"blob":"sha256:…","media_type":"image/x-icon"},"image":{"blob":"sha256:…","media_type":"image/png"},"kind":"page","site":"GitHub","title":"…","url":"https://github.com/…"}}
{"card":{"author":"某个 UP 主","description":"…","duration":213,"icon":null,"image":{"blob":"sha256:…","media_type":"image/jpeg"},"kind":"video","site":"哔哩哔哩","title":"…","url":"https://www.bilibili.com/video/BV…"}}
{"card":null,"why":"no_preview"}
```

- `card.url` 是跟完跳转、最后落到的那一页。`description` 没有的是空字。
- `image`、`icon` 可以是 `null`。图是这个账号的 blob，头照 `blob.get` 读，网页照 `/media` 给（W-10）。
- `kind` 一定有：`video`（B 站的视频、YouTube 上读到了时长的页）、`article`（MediaWiki 站上的页）、`page`（别的）。
- `duration`（视频多少秒，正整数）、`author`（B 站的 UP 主、YouTube 的频道名）没有的不写（W-7 再补，「怎么走」第 12 条）。
- `why`：`not_a_url`（读不成地址）、`unsupported_scheme`（不是 http、https）、`no_preview`（不是网页、没有标题、地址过不了闸、跳转太多，下次也一样）、`unreachable`（超时、连不上、对方回 4xx、5xx，过会儿可能就好了）。做不出卡片是正常的结果之一，不是出错。
- 在后台答：这个连接上后面的请求不等它，回应照 `id` 对上（「怎么走」第 11 条）。

样本 `resources/software/net/link_preview.json`：

```json
{
  "remember": {
    "found_seconds": 21600,
    "no_preview_seconds": 900,
    "unreachable_seconds": 45,
    "entries": 512
  },
  "page": {
    "timeout_seconds": 12,
    "max_bytes": 2097152,
    "accept": "text/html,application/xhtml+xml"
  },
  "image": {
    "timeout_seconds": 8,
    "max_bytes": 3145728,
    "accept": "image/webp,image/png,image/jpeg,image/gif,image/x-icon,image/*;q=0.8,*/*;q=0.5"
  },
  "api": {
    "timeout_seconds": 12,
    "max_bytes": 2097152,
    "accept": "application/json"
  },
  "redirects": 5,
  "user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36",
  "accept_language": "zh-CN,zh;q=0.9,en;q=0.8",
  "clip": {
    "title": 120,
    "description": 300,
    "site": 60,
    "author": 60
  },
  "clients": {
    "seconds": 120,
    "keep": 32
  },
  "sites": {
    "bilibili": {
      "hosts": ["bilibili.com"],
      "short_hosts": ["b23.tv"],
      "site": "哔哩哔哩",
      "gone_titles": ["视频去哪了呢？_哔哩哔哩_bilibili"]
    },
    "youtube": {
      "hosts": ["youtube.com", "youtu.be"]
    },
    "mediawiki": {
      "hosts": {
        "wikipedia.org": "/w/api.php",
        "wiki.archlinux.org": "/api.php"
      },
      "thumbnail": 640
    }
  },
  "challenge": {
    "titles": ["Just a moment...", "Attention Required! | Cloudflare", "安全检查", "Access denied"],
    "headers": {
      "cf-mitigated": "challenge"
    },
    "statuses": [403, 503]
  }
}
```

### 怎么走

规矩照网页演示的桥（proto/web-demo 分支 `web-demo/bridge/src/link_preview/`），桥照的是旧版。和桥不一样的地方照这一页：代理（第 5 条）、图存成 blob（第 8 条）、在后台答（第 11 条）。

1. 可选软件包 `net`：抓取和地址闸在 crate `miyu-net`，经 `miyu-core` 的 cargo 开关 `net` 编进来，发行版默认打开。核心起来时经 `crates/miyu-core/src/packages.rs` 往查询表里登记 `link.preview`；没编进来的回 `unknown_method`。第一次调才读 `link_preview.json`，读不懂的记一条 `WARN not ready`，回 `internal_error`，这个核心的生命周期里不再试（照 `mermaid.md` 第 2 条）。
2. 地址：去掉前后空白读成网址，读不成 `not_a_url`；不是 `http`、`https` 的 `unsupported_scheme`。这两种不抓、不记。
3. 地址闸，每一跳都过：
   1. 样子：只认 `http`、`https`；不带用户名、密码；`localhost`、`*.localhost`、`*.local` 不去（末尾带点的也算）；主机写的就是 IP 的照第 3 层判（`127.1`、`0x7f000001`、`2130706433` 这些写法读网址时已经规整成 `127.0.0.1`）。
   2. 解析：每一个解析出来的地址都要是公网的，有一个不是就整个不去。解析好的地址钉进这一跳的连接：查过的就是连上的，中间没有第二次解析（防 DNS rebinding）。解析的时限和这一跳一样。
   3. IP 段：回环、私网、链路本地、运营商级 NAT（`100.64.0.0/10`）、唯一本地、组播、未指定、广播、`0.0.0.0/8`、`192.0.0.0/24`、`240.0.0.0/4`、文档的段（`192.0.2.0/24`、`198.51.100.0/24`、`203.0.113.0/24`、`2001:db8::/32`、`3fff::/20`）、站点本地 `fec0::/10`、本地 NAT64 `64:ff9b:1::/48`、丢弃 `100::/64` 都不是公网。测性能的段 `198.18.0.0/15` 算公网（第 5 条）。里面嵌着 IPv4 的 IPv6（映射 `::ffff:a.b.c.d`、NAT64 `64:ff9b::/96`、6to4 `2002::/16`）照那个 IPv4 判；IPv4 兼容的写法（`::a.b.c.d`）不去。
   4. 过不了闸：`no_preview`。
4. 跳转自己跟，最多 5 跳：头一次加 5 次跳转，第 6 个回应还是跳转的 `no_preview`。`Location` 照这一跳的地址接成绝对的，每一跳重新过闸、重新挑走不走代理。跳转没写 `Location`、写的读不成地址：`unreachable`。
5. 代理（2026-10-01 项目主人定）：和请求模型一样照环境变量走（`HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`，小写的也认，`http.md`「客户端」第 5 条）。核心第一次调 `link.preview` 时读一次。
   - 这一跳要走代理的：先在本机解析一遍，过第 2 层（有一个不是公网的就不去），再交给代理连，不钉地址。代理那头怎么解析我们管不着，这个口子项目主人认了。
   - 不走代理的（没设，或者在 `NO_PROXY` 里）：照第 2 层钉住解析好的地址。本机解析不出来的：`unreachable`。
   - 本机解析不出来、代理那头解析得出来的（被污染的域名常这样）：照样交给代理，第 1 层、第 3 层照常过。
   - 测性能的段 `198.18.0.0/15` 当公网：Clash、mihomo、sing-box、Surge 的假地址（fake-ip）默认就在这一段，开着这类代理的机器上每个域名都解析到这里，连上的其实是代理软件，它照域名去连。不放过这一段，这些机器上一张卡片都出不来。
   - 代理本身在哪不过闸：它是人自己设的，常在本机。
6. 页面：一跳 12 秒（连上、发出、读完都算在里面）；对方回 4xx、5xx 的 `unreachable`（人机验证页除外，第 13 条）；回的不是 HTML（`Content-Type` 里没有 `html`，没写的也算）的 `no_preview`；读到 `</head>` 或者 `<body` 就停（不分大小写，跨在两块之间的也认得），最多 2 MiB（**照解开以后的字节算**，W-7 补）。照 UTF-8 读，读不了的字换成替换符。
   - 读到 `</head>`、`<body` 这个记号时：这一截里已经有 `<title>` 或者不空的 `og:title` 了，就在那儿截住，跟以前一样；两个都没有的，不截，接着往下读，只看 `<meta …>`、`<link rel=icon …>`，找到不空的 `og:title` 就停，最多读到上限（YouTube 把 `og:*` 放在 `</head>` 后面，`</head>` 在第 71.8 万字节、`og:title` 在第 77.3 万字节上，W-7 补 2026-10-02 施工时定）。这个判断边读边做，不会为了判断把整段重新扫一遍。
   - B 站不管请求带不带 `Accept-Encoding` 都压着发页面：`miyu-net` 的客户端开着 reqwest 的 `gzip`、`brotli`、`deflate`、`zstd` 特性，自动带上 `Accept-Encoding`、自动解开（W-7 补）。字节上限照**解开以后**的算，一个压得很小、解开很大的包照样在上限停，不会先整份解开再截（async-compression 的解码器本身是边读边解的流，不是一口气摊开）。
7. 挖元数据：标题、简介、图照 `og:*`、`twitter:*`、`<title>` 和 `<meta name=description>` 的先后（`og:*` 不管写在第几行都先算）；同一样写了好几遍的，排在前面的算。站名照 `og:site_name`，没有用主机名去掉 `www.`；图标照 `rel` 里有 `icon` 的、`apple-touch-icon`，都没有试 `/favicon.ico`；相对地址照最后落到的那一页算；常见的实体（`&amp;`、`&#8212;` 这些）解开；空白收拢，标题最多 120 个字、简介 300、站名 60，超出的截断加 `…`。一格里留着没填的模板（`{$0}`、`{$1}` 这样，`{$` 加数字加 `}`）的，那一格当没有（W-7 再补：B 站删了的视频页，og 的简介是没填的模板）。没有标题的 `no_preview`。
8. 图：一张 8 秒、最多 3 MiB（对方报的长度超了的不读，读的超了整张不要），跳转、闸、代理照页面一样走。只收照开头的魔数认得出的五种（PNG、JPEG、GIF、WebP、ICO），对方说是什么类型不算；不收 SVG（它能带脚本）。存成这个账号的 blob，回应里写哈希和认出来的类型。图和图标一起抓。抓不到、认不出、存不进的那一格是 `null`，卡片照样成立。
9. 抓过的记在核心的内存里，照地址（读成网址以后的写法）记：抓到了的记 6 小时，`no_preview` 记 15 分钟，`unreachable` 记 45 秒；最多 512 条，满了整个清空。记着的卡片指的 blob 没了的，那一格交 `null`。
10. 请求头（`User-Agent`、`Accept`、`Accept-Language`）、时限、上限、记多久都在 `resources/software/net/link_preview.json`。客户端照「怎么连」（钉的哪个主机、哪几个地址；不钉的；走哪个代理）复用 120 秒、最多 32 个，满了整个清空：每一跳新造一个要重新装证书、从零握手（旧版实测一跳约 0.2 秒）。
11. 不碰会话。在后台的任务里抓，不挡这个连接上后面的请求的读：回应照 `id` 对上，不经会话的订阅，直接放进写队列（`protocol.md`「一个连接」第 1 条的例外）。连接断了，它在后台答的任务一起停。
12. 按站取（W-7 再补，2026-10-02 项目主人要的站，办法主会话定）：认得的站照自己的办法取，不认得的照第 6、7 条。站的规矩在 `link_preview.json` 的 `sites`，代码里只有「怎么取」。
    - 认主机：主机（小写、去掉末尾的点）和列着的一样，或者以「`.` 加列着的」结尾，就算。
    - 站的请求都经这一页的抓取：每一跳过闸（第 3 条）、挑走不走代理（第 5 条）、自己跟跳转（第 4 条），客户端照第 10 条复用。接口照 `api` 的时限、上限、`Accept`；回的不是 2xx、读不成 JSON、超了上限，都算接口没成。
    - 接口没成、退回读页面的，记一条 `WARN link site fallback`（「出错」）。
    1. **B 站**（`sites.bilibili`）：
       1. 认：主机是 `hosts` 里的，路径是 `/video/BV` 加 10 个字母数字，或者 `/video/av` 加数字（后面可以再跟 `/`、`?` 这些）。主机是 `short_hosts` 里的（`b23.tv`）：先照第 4 条跟完跳转，看落到的地址，再照前一句认；跟跳转出了错的，照那个错交。
       2. 认出来的整页读完（不在 `</head>` 停：视频数据在页面后半截的脚本里，整页一二百 KB），最多 2 MiB。
       3. 页面标题（收拢空白以后）在 `gone_titles` 里的（B 站「视频没了」的那一页，2026-10-03 项目主人实测）：`no_preview`。
       4. 页面脚本里 `window.__INITIAL_STATE__=` 后面那个 JSON 对象的 `videoData`：标题 `title`、简介 `desc`、图 `pic`（照第 8 条抓、存成 blob）、作者 `owner.name`、时长 `duration`（秒，正整数才写）。`title` 是空的、读不成 JSON 的，当没有这份数据。
       5. 没有这份数据的、数据里没给的那几格：照页面（第 7 条），作者照 `<meta name="author">`。站名先照 `og:site_name`，再照 `site`。`kind` 总是 `video`；卡片的地址是落到的那一页。
       6. 不调接口 `x/web-interface/view`：没登录的请求它回 HTTP 412，带不带 `buvid3` 都一样（2026-10-03 项目主人实测）。
    2. **YouTube**（`sites.youtube`）：
       1. 认：主机是 `hosts` 里的（`youtu.be` 跳到 `youtube.com`，照第 4 条跟）。
       2. 读页面不照第 6 条在 `</head>`、`<body` 那儿停：YouTube 的 `og:*`、时长、频道名都在 `</head>` 后面。读到不空的 `og:title`、`<meta itemprop="duration">`、`<link itemprop="name">` 三样都见到了为止，三样不齐的读到完或者读到 2 MiB。普通网页照旧，不为它们多读。
       3. 挖元数据照第 7 条，读到的整段都看（不在 `</head>` 停）。时长照第一个 `<meta itemprop="duration" content="…">`：ISO 8601 的时长（`PT3M33S`、`PT1H2M3S`、`P1DT2H`，只认天、时、分、秒，整数）换成秒，读不了、是 0 的不写。作者照第一个 `<link itemprop="name" content="…">`，没有的不写。
       4. 读到了时长的 `kind` 是 `video`（看的是一个视频）；没读到的（频道、列表）是 `page`。
    3. **MediaWiki 站**（`sites.mediawiki`）：
       1. 认：先照第 6、7 条抓页面；主机是 `hosts` 里的，或者 `<head>` 里有 `<meta name="generator" content="MediaWiki …">`（照开头的 `MediaWiki` 认，不分大小写），就算。
       2. 页面名：页面里 `"wgPageName":"…"`（MediaWiki 每一页都写在 `RLCONF` 里）那个 JSON 字符串。没有的，照普通网页交卡片，`kind` 还是 `article`。
       3. `api.php` 在哪：`hosts` 里列着的，照落到的那一页的协议、主机、端口接上列着的路径；没列着的，照 `<link rel="EditURI" href="…">` 去掉问号后面的；都没有的，照普通网页交卡片。
       4. 调 `api.php?action=query&format=json&formatversion=2&redirects=1&prop=extracts|pageimages&exintro=1&explaintext=1&piprop=thumbnail&pithumbsize=<thumbnail>&meta=siteinfo&siprop=general&titles=<页面名>`：标题 `query.pages[0].title`、简介 `extract`、图 `thumbnail.source`、站名 `query.general.sitename`。
       5. 回应里没有 `extract` 的（站上没装 TextExtracts，ArchWiki 就是）：再调 `api.php?action=parse&format=json&formatversion=2&redirects=1&prop=text&section=0&page=<页面名>`，简介照 `parse.text` 里第一段去掉标签以后不空的 `<p>`：只认正文最外层的段落（`mw-parser-output` 那一层，没有它包着的是整段的最外层），框、表格、列表里的不算（ArchWiki 顶上「Related articles」框里的 `<p>`，2026-10-03 项目主人实测）；`<sup>`、`<style>`、`<script>` 连里面的字一起去掉，实体解开，空白收拢。
       6. 合成卡片：接口给了的照接口，接口没给的那几格照页面（第 7 条）。站名先照 `og:site_name`，再照接口的 `sitename`，再照页面标题「 - 」后面那一截，最后才是主机名。用的是页面的标题的：最后一个「 - 」连后面那一截去掉（`Rust - 维基百科，自由的百科全书` 变成 `Rust`）。`kind` 是 `article`。
       7. 接口没成：照页面合成卡片（上一条），`kind` 还是 `article`。
13. 人机验证、挡爬虫的页面（W-7 再补，2026-10-02 项目主人提，认法主会话定）：认出来的不出卡片，`no_preview`，照第 9 条记 15 分钟，运行日志照第 1 条记 `link preview failed`。认法在 `link_preview.json` 的 `challenge`，以后加不用改代码：
    1. 页面的回应带着 `headers` 里的哪一个头、值也对上（名字不分大小写，值去掉两头空白、不分大小写）；
    2. 页面回的状态码在 `statuses` 里（403、503），`Content-Type` 里又有 `html`；
    3. 挖出来的标题（收拢空白以后）和 `titles` 里的哪一个完全一样。
    图、接口的回应不认这一条：图抓不到那一格是 `null`，接口没成照第 12 条退回。
14. 卡片上的字照第 7 条收拢、截断，作者最多 60 个字（`clip.author`）。站取到的字和页面挖到的一样处理，模板那一条（第 7 条）也照样。

### 出错

协议的拒绝照 `protocol.md`「出错」：`bad_params`（`url` 没写、不是字符串）、`internal_error`（`link_preview.json` 读不懂）。做不出卡片不是拒绝，是 `why`。

运行日志（目标 `miyu::net`，一律英文）：

| 级别 | 行 | 什么时候 |
|---|---|---|
| `WARN` | `link preview failed host=… why=…` | 抓了、没做成卡片（`no_preview`、`unreachable`）：只写主机名，不写地址。记着的、读不成地址的、不是 http 的不记 |
| `WARN` | `link site fallback site=… host=…` | 认得的站，接口没成、退回读页面（第 12 条）：现在只有 `site=mediawiki`。只写主机名 |
| `WARN` | `link image not stored error=…` | 抓到的图存不进 blob：那一格交 `null` |
| `WARN` | `not ready error=…` | `link_preview.json` 读不懂。只记第一次 |

### 给人看的字

没有新的。`why` 是数据，头照它自己说。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-net/src/guard/tests.rs` | IP 段的表（照桥的 `guard.rs` 测试，`198.18.0.0/15` 改成公网，映射、NAT64、6to4 照里面那个 IPv4 判）；地址的样子；主机写的就是 IP 的不查 DNS；解析出一个不是公网的整个不去 |
| `crates/miyu-net/src/html/tests.rs` | 元数据的先后、站名、图标、相对地址、截断、实体；坏的标记不崩；`<head>` 里没挖到东西时接着往正文找 `og:title`；`<head>` 里已经有 `<title>` 的不去正文找；`meta_is_og_title` 的键、`name` 退路、内容不空 |
| `crates/miyu-net/src/body/tests.rs` | 只收五种图、不收 SVG；读到 `</head>`、`<body` 就停；`HeadSignals` 边读边找 `<title>`、不空的 `og:title`，标签跨在接缝上不丢、处理过的不重扫 |
| `crates/miyu-net/src/remember/tests.rs` | 三种结果各记多久；满了整个清空 |
| `crates/miyu-net/src/rules/tests.rs` | 出厂的 `link_preview.json` 读得进、数对得上；多一格、是 0 的读不了 |
| `crates/miyu-net/tests/preview.rs` | 本机的假服务器上整条走通：钉住的地址（系统解析不了的名字照样到了）、元数据、图存成 blob、`blob` 没了交 `null`、记着的不再抓；跳转每一跳过闸（内网、`localhost`、`ftp`、云的元数据地址）、最多 5 跳、没写 `Location`；不是 HTML、没写类型、没有标题、4xx、5xx；读到 `</head>`、`<body` 就停，最多 2 MiB；图只收五种、不收 SVG、最多 3 MiB（报了长度的、没报的）；`link_preview.json` 读不了。gzip、br 压过的页面不带 `Accept-Encoding` 也解得开（W-7 补）；一个压得很小解开很大的包照上限停，标题在上限前面的找得到、后面的找不到；`og:title` 挪到 `</head>` 后面的找得到，挪到上限以后的找不到；`og:title` 排在 `<head>` 里靠前、别的字段排在后面隔着一截填料的，不会因为先找到 `og:title` 就提前收手丢了后面的字段 |
| `crates/miyu-net/src/sites/*/tests.rs`、`crates/miyu-net/src/rules/tests.rs` | 认主机（一样的、子域名、像而不是的 `notbilibili.com`）；BV、av 的样子；B 站页面脚本里的 `videoData`（读不成的照页面的 `<meta name="author">`）；ISO 8601 的时长；`wgPageName` 的 JSON 字符串（转义、中文）；`EditURI`；第一段 `<p>`（空的、`<sup>` 去掉，框、表格里的不算）；接口回应读出来的几格；`sites`、`challenge` 多一格少一格读不了 |
| `crates/miyu-net/tests/sites.rs` | 本机的假服务器当 B 站、YouTube、维基百科、别的 MediaWiki 站：B 站 BV、av、短链跟跳转都认，页面脚本里有视频数据的卡片带封面、UP 主、时长、`kind: video`，没有的照页面的 og 和 `<meta name="author">`、还是 `kind: video`，不调接口，「视频没了」那一页 `no_preview`、同样的标题不在 B 站视频页上的照常、模板那一格当没有；MediaWiki 照列着的主机、照 `generator` 都认，`api.php` 的简介、缩略图、站名，没有 TextExtracts 退回第一段，标题去尾巴，`kind: article`，接口没成照页面；YouTube 读过 `</head>`、时长换成秒、频道名、读不到时长的是 `page`；人机验证的标题、头、403 和 503 的 HTML 都 `no_preview`，403 不是 HTML 的照旧 `unreachable`；不认得的站照旧 `kind: page`、不调接口；MediaWiki 站的 `EditURI` 指到内网的被闸拦住、照页面合成卡片 |
| `crates/miyu-net/tests/live.rs`（`#[ignore]`，还要设 `MIYU_NET_LIVE=1` 才连外网：CI 的长跑用 `--ignored` 跑所有标了的测试） | 真网络：B 站视频、YouTube 视频、中英维基、ArchWiki、一个普通网页各出像样的卡片（`kind` 对、有简介），卡片印出来给人看（W-7 再补，施工单验收第 3 条） |
| `crates/miyu-net/tests/proxy.rs` | 代理（照测试的口子给的值）：页面和图都经假代理、请求行写着整个地址；本机解析不出来的照样交给代理；解析出内网的、写的就是内网和回环的一律不交；`NO_PROXY` 里的直连、钉地址；不走代理、解析不出来的 `unreachable`；没开测试的口子时，回环上真在听的服务器也不去 |
| `crates/miyu-endpoint/src/queries.rs` 里的测试 | 在后台答的只有照 `register_background` 登记的；同一个名字两种登记也 panic |
| `crates/miyu-core/tests/packages.rs` | 查询表没登记 `link.preview` 回 `unknown_method`；登记了的读不成地址、不是 http 不碰网络就答；`url` 没写、不是字符串 `bad_params`；在后台答的不挡后面的请求、回应照 `id` 对上、连接断了它跟着停 |
| `crates/miyu/tests/link_preview.rs` | 真的核心、环境变量里的代理：经假代理出卡片、图用 `blob.get` 读得回来，`kind` 写着、`duration`、`author` 没有的不写（W-7 再补）；慢的 `link.preview` 不挡同一个连接上后面的请求 |

### 出处

- `web-module.md`「八、链接预览」（2026-10-01 项目主人批准，原文搬到这一页），第 5 题（链接卡片走代理，项目主人 2026-10-01 定），「起草时定的」第 14、31 条。
- `10-自带软件.md` 第四节（`net` 包，以后 `web_fetch` 用同一份）；`04-核心协议.md` 第九节。
- proto/web-demo 分支 `web-demo/bridge/src/link_preview/`（`guard.rs` 的地址表和测试、元数据的挖法、图的认法、规矩的数）、`web-demo/resources/link_preview.json`。

### 还没有的

- `web_fetch`、`web_search` 工具：用这一份的抓取和地址闸。
- 别的站（推特、知乎……）：照第 12 条同一层加。登录才看得到的内容不做。
- 网页软件的 `/media`（W-10）：卡片的图在那之前照桥的 `/blob` 给。
- 链接卡片的图随存储的回收清掉：它们是没人引用的 blob（`store.md`「还没有的」）。
- 两个演示的桥、`/link-image`：合了主会话通知它们的会话自己删。
- 多用户：图现在存进管理员的 blob（核心起来时登记就定了），有了别的账号以后照连接的账号存。

### 起草时定的

`web-module.md`「起草时定的」第 14、31 条（图存成账号的 blob、经同一个 `/media` 给；在后台答），原文留在那一页。下面是施工 W-7 时补的，2026-10-02 施工时定。

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 走不走代理用 `hyper-util` 的 `Matcher` 判（`from_env`）：reqwest 自己照环境变量走代理用的就是它 | `NO_PROXY` 的读法（域名带不带点、`*`、IP 段）和请求模型一字不差；这个库本来就在依赖图里 | 自己写一份 `NO_PROXY` 的读法（和请求模型那边早晚对不上） |
| 2 | 走代理的一跳交给 reqwest 的 `Proxy::all`，地址和认证照 `Matcher` 交回的 | 走不走代理只判一次，交给 reqwest 的就是判的结果 | 把环境变量原样交给 reqwest 再判一次（两处判，可能不一样） |
| 3 | 测试的口子是 cargo 开关 `testkit`（和这个包自己的单元测试）下才编进去的 `LinkPreview::testing`：回环当公网、只照给的一张表解析（表里没有的当解析不出来，不问系统）、代理照给的值 | 本机的假服务器在回环上，闸本来不许去；生产的构建里没有这段代码，没有后门。只照表解析，测试不碰真的 DNS | 生产代码里留一个能换的判断（谁都调得到）；测试里改环境变量（同一个进程里别的测试跟着变） |
| 4 | 解析出的是空的、解析出错、超时，三种一样当解析不出来 | 结果都是「本机不知道它在哪」：走代理的照第 5 条交给代理，不走的连不上（`unreachable`） | 空的当 `no_preview`（桥这样，可代理那头也许解析得出来） |
| 5 | 一跳的时限放在每个请求上，不放在客户端上 | 页面、图共用复用的客户端，时限不一样 | 时限照客户端分（客户端多一倍） |
| 6 | `link_preview.json` 去掉桥才有的几格（图记在内存里的几张、几个字节，浏览器缓存多久），`transient_seconds` 改叫 `unreachable_seconds`，不带 `note`，多一格、少一格都不认 | 图存成 blob 了，那几格没人用；名字和协议的 `why` 对上；`note` 照 W-4 的先例不要 | 照桥原样搬 |
| 7 | 图的 `Accept` 去掉 `image/avif`，先列收的五种 | 桥写着先要 AVIF，可 AVIF 不在收的五种里：给了 AVIF 的站，卡片上就没图 | 照桥原样（先要一种不收的） |
| 8 | 在后台答做成查询表的一种登记（`register_background`），端点照登记分；后台任务放在连接自己的一个 `JoinSet` 里，连接断了一起停 | 加东西只登记，不改中心（`web-module.md` 第 19 条）；连接走了还接着抓，核心就一直不算空闲 | 端点里写死 `link.preview` 这个名字；后台任务不管，抓完自己结束（连接断了还占着最多一分多钟） |
| 9 | 图存进哪个账号，核心起来时登记就定了：`packages::register` 多收数据根和账号，现在是 `admin` | 现在连上来的都是管理员；端点的家底不用多公开一个方法 | `Core` 公开一个取 blob 的方法，每次照连接取（多用户来了再做） |
| 10 | 运行日志只在真抓过、没做成时记 `link preview failed`；图存不进 blob 另记一行 | 记着的再记一遍只是刷屏；读不成地址的不是「抓」；存不进是磁盘出了事，不吞 | 每次都记 |
| 11 | `miyu-net` 开 reqwest 的 gzip/brotli/deflate/zstd 特性（整个工作区合起来的），`miyu-http` 的客户端明确关掉（`no_gzip`/`no_brotli`/`no_deflate`/`no_zstd`，这几个方法不管特性开没开都存在）。`</head>`、`<body` 这个记号的「要不要接着往下读」判断边读边做，一个标签只处理一次，不整段重扫（W-7 补，2026-10-02 施工时定） | B 站不管请求带不带 `Accept-Encoding` 都压着发；YouTube 把 `og:*` 放在 `</head>` 后面；请求模型那条路要一个字节不变；边读边判断才不会把一次判断变成对着攒大的 `<head>` 整段重扫几十万次（压缩炸弹、慢速攻击都靠它防） | 只给 `miyu-net` 单独开一份不同版本的 reqwest（两份证书栈、两套 TLS 初始化，没必要）；读完整个 `<head>` 再判断一次要不要接着读（正常页面也要等读完才能判断，变慢） |
| 12 | MediaWiki 站都走 `api.php`（维基百科也是），不走维基百科的 REST `page/summary`；站名放在同一个请求里（`meta=siteinfo`）（W-7 再补，2026-10-02 施工时定，下同） | 一条路认所有 MediaWiki 站，少一份代码；REST 只有维基媒体的站有，又不给站名 | 维基百科走 REST、别的走 `api.php`（两份读法；维基百科的站名还得另找） |
| 13 | 没装 TextExtracts 的，第一段照 `action=parse&section=0` 的回应找，不回头再读页面的正文 | 第一段在正文里，离 `</head>` 很远；为它把每个 MediaWiki 页面都多读一截，或者回头再抓一遍，都比多调一次接口贵。`parse` 是 MediaWiki 本身就有的，不靠扩展 | 读页面时认出是 MediaWiki 就接着读到第一段（每页都多读）；回头再抓一遍页面 |
| 14 | B 站读页面脚本里的视频数据（`__INITIAL_STATE__.videoData`），不调接口；短链先跟完跳转再认（2026-10-03 照项目主人实测改） | 接口 `x/web-interface/view` 对没登录的请求回 412，带上 `buvid3` 也一样；页面照常给，脚本里那份数据和接口的字段一样 | 先调接口、不行再读页面（每张卡多一个必挂的请求）；带登录的 cookie（要人的账号）；接口的签名（wbi，跟着 B 站改） |
| 15 | YouTube 是不是视频照读没读到时长判，不照路径 | 读到了时长才是一个视频；`/watch`、`/shorts/`、`youtu.be` 这些路径以后还会变 | 照路径列一张表 |
| 16 | 接口有自己的一份预算 `api`（时限、上限、`Accept`），数和页面一样 | 接口回的是 JSON，`Accept` 和页面的不一样；放一起改一个就动另一个 | 照页面的预算、`Accept` 写死在代码里 |
| 17 | 真网络的实测是一个标了 `#[ignore]`、还要设 `MIYU_NET_LIVE=1` 的测试（`tests/live.rs`） | 站会变，CI 不该跟着外网红；要验的时候一条命令就能在人自己的机器上看到每张卡片 | 只靠人手点网页看（不留痕）；放进 CI（外网一抖就红） |
| 18 | 没填的模板（`{$` 加数字加 `}`）对所有站、所有格都算没有 | 哪个站留着没填的模板，那一格给人看都是坏的；只认 B 站反倒要多一个判断 | 只在 B 站退回读页面时认 |
