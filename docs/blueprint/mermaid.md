## mermaid：源码画成 SVG

### 是什么

mermaid 源码画成 SVG 由核心做一次：同一张图，终端和网页看到的是同一份 SVG，画图的内存只在核心里。做成可选的
软件包 `mermaid`：crate `miyu-mermaid`，经 `miyu-core` 的 cargo 开关 `mermaid` 编进来，发行版默认打开。没编
进来的核心里没有这块代码，`mermaid.render` 回 `unknown_method`，头照代码块显示源码。第一次调才初始化（读
`style.json`、探一次系统的字体库），之后一直留着，直到核心退出。

这一页原来写在 `web-module.md`「五、mermaid」，施工 W-4 时搬过来（`web-module.md`「起草时定的」第 23 条）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-mermaid/`（第 3 层，执行器） | 画 SVG、懒读 `style.json`、独立探一次字体库、缓存、崩了当画不出 |
| `crates/miyu-mermaid/src/lib.rs` | `Mermaid`：公开的家底和 `render`；渲染器的主题怎么改（三种记号色、字体） |
| `crates/miyu-mermaid/src/style.rs` | 读 `style.json` |
| `crates/miyu-mermaid/src/fonts.rs` | 独立探一次系统的字体库找不找得到字 |
| `crates/miyu-endpoint/src/queries.rs` | 可选软件包登记的查询：方法名到怎么答的一张表，`QueryError`（软件包用得上的几种拒绝，不认得 JSON-RPC 的错误码） |
| `crates/miyu-core/src/packages.rs` | 照编进来的可选软件包（cargo 开关）往查询表里登记；`mermaid.render` 的参数怎么读、`Mermaid` 的结果怎么翻成回应或者 `QueryError` |
| `resources/software/mermaid/style.json` | 字体、三种记号色、源码的上限、记几张 |

### 对外的样子

**`mermaid.render`**：握手以后，都能调。

| 参数 | 类型 | 说明 |
|---|---|---|
| `source` | 字符串，必写 | mermaid 源码 |

回应 `{"marks": {"label": <色>, "line": <色>, "text": <色>}, "svg": <SVG 的字>}`：SVG 里字、线、连线标签垫底
用的三种记号色，头照它们换成自己的颜色（「怎么走」第 7 条）。

```json
{"id":"m1","jsonrpc":"2.0","result":{"marks":{"label":"#070809","line":"#040506","text":"#010203"},"svg":"<svg …"}}
```

### 怎么走

1. 画 mermaid 在核心里：代码只有一份，同一张图终端和网页看只画一次，画图的内存只在核心里。做成可选的软件包
   `mermaid`：crate `miyu-mermaid`，经 `miyu-core` 的 cargo 开关 `mermaid` 编进来，发行版默认打开。没编进来
   的核心里没有这块代码，`mermaid.render` 回 `unknown_method`，头照代码块显示源码。
2. 第一次调才初始化：读 `resources/software/mermaid/style.json`、读系统的字体库（画图的库要量字的宽）。之后
   一直留着，直到核心退出。
3. 源码去掉前后空白。空的：`bad_params`。超过 64 KiB：`mermaid_too_long`。
4. 缓存：照源码的 SHA-256，最多 64 张，满了丢最久没用的。设计说「源码加尺寸」：SVG 不分尺寸，缓存只照源码；
   尺寸只在终端栅格化时用，那一步在头里。
5. 一次画一张（一把锁），在阻塞线程里画。画图的库崩了（panic），当画不出。
6. 画不出：`mermaid_failed`，`data.detail` 是画图的库的原话（英文）。
7. 样子照终端演示和桥：从画图的库的暗色主题改起，底和框都不填色；字、线、连线标签垫底先填三种图里不会自己
   出现的记号色，回应的 `marks` 写明是哪三种。网页把它们换成页面的 CSS 变量（换主题不用重画），终端换成主题
   色再栅格化。字体照 `style.json` 的一串（正文常用的几种，最后是 `sans-serif`）。
8. 用的库照终端演示和桥：`mermaid-rs-renderer` 0.3.1，同一个版本，关掉它默认的功能（命令行、PNG 输出），只
   要出 SVG 的那一半：编译快、依赖少。
9. 视图投影（M9）做出来以后，`view.detail` 取 mermaid 那一项，照同一个画法、同一份缓存给。

### 出错

协议的拒绝照 `protocol.md`「出错」：`code` 是 `-32010`，原因码在 `data.reason`。

| 原因码 | 什么时候 |
|---|---|
| `mermaid_too_long` | 源码超过 64 KiB |
| `mermaid_failed` | 画不出；`data.detail` 是画图的库的原话 |

`bad_params`：`source` 是空的（去掉前后空白以后）。`internal_error`：`style.json` 读不懂、这台机器上一种
字体都读不到（画图的库初始化不了）。

运行日志（目标 `miyu::mermaid`，一律英文）：

| 级别 | 行 | 什么时候 |
|---|---|---|
| `WARN` | `not ready error=…` | 画图的库初始化不了：`style.json` 读不懂，或者这台机器上一种字体都读不到。只记第一次，不是核心重启不会再试 |

### 给人看的字

协议的拒绝照握手时的语言，中文、英文各一句：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `mermaid_too_long` | 这张图的源码太长了。 | The diagram source is too long. |
| `mermaid_failed` | 这张图画不出来。 | The diagram could not be drawn. |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-mermaid/src/tests.rs` | 记号色都换得掉、底和框不填色；同一份源码第二次不重画、满了丢最早的那一张；空的、太长、画不出各说一句（库的原话不是空的）；造出 `Mermaid` 不碰磁盘，第一次调才读 `style.json`、探字体；`style.json` 读不懂当没法画；panic 的几种payload 都翻成字符串；真的 `style.json`、一张流程图一张时序图都画得出 |
| `crates/miyu-mermaid/src/style/tests.rs` | 读得进来、少一格或者多一格不认的、数是 0 或者字体表是空的都读不了 |
| `crates/miyu-mermaid/src/fonts/tests.rs` | 这台机器上读得到至少一种字体（真机验，三个平台都该过） |
| `crates/miyu-endpoint/src/queries/tests.rs` | 没登记的方法交回 `None`；登记过的名字正好对上才找得到；同一个名字登记两次当场 panic |
| `crates/miyu-core/tests/packages.rs` | 真核心走一遍：一张流程图、一张时序图都出 SVG，回应的 `marks` 和 SVG 里用的三种记号色对得上；同一份源码两次拿到一样的回应；空的 `bad_params`、超过上限 `mermaid_too_long`、画不出 `mermaid_failed`（`data.detail` 不是空的）；查询表没登记的方法（没编进来某个软件包的情形）回 `unknown_method` |

### 出处

- `04-核心协议.md` 第五节「mermaid 图由核心出 SVG」、P5、P6；`10-自带软件.md` 第四节「可选软件包」；
  `13-终端界面.md` 第九节「mermaid 也显示成图」。
- `web-module.md`「是什么」「五、mermaid」（2026-10-01 项目主人定，原文搬到这一页）、「起草时定的」第 19 到
  23 条（可选软件包登记查询的做法、cargo 开关、先只做 `mermaid.render`、三种记号色、这一页独立成页）。
- proto/tui-demo 分支 `tui-demo/src/figures/mermaid.rs`（画 SVG 的那一半、三种记号色的做法，搬过来；栅格化
  留在头里）；proto/web-demo 分支 `web-demo/bridge/src/mermaid.rs`（三种记号色换成页面 CSS 变量的做法）。

### 还没有的

- `view.detail`（M9 的视图投影）：到时候照同一份缓存给（「怎么走」第 9 条）。
- 终端栅格化、点开看大图：在头里，不在核心里。
- 两个演示自己画 SVG 的那一半、依赖 `mermaid-rs-renderer`：合了主会话通知它们的会话自己删（终端）、删桥的
  `mermaid.rs`（网页）。

### 起草时定的

技术细节照推荐定了，这几条是 `web-module.md`「起草时定的」第 19 到 23 条，原文留在那一页；施工 W-4 时另加
的几条跟在后面。

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 可选软件包登记查询：端点多一张查询表，核心起来时照编进来的包往里登记 | 加东西只登记，不改中心；没编进来就没有这一行，回 `unknown_method` | 端点里写死 `#[cfg]` 的分支 |
| 2 | `mermaid`、`net` 是编译时的开关，发行版默认打开 | 没装的核心里真的没有这块代码；现在还没有软件包管理，编译时的开关就是「装」 | 单独的 worker 程序（多一个进程、多一套说法）。一直编进去、停用了不构造 |
| 3 | mermaid 先做一个只出 SVG 的方法 `mermaid.render`，`view.detail` 随 M9 | 视图投影还没有，头现在就要图；M9 的 `view.detail` 用同一个画法、同一份缓存 | 现在就做 `view.detail`（要先有视图条目） |
| 4 | mermaid 的颜色用三种记号色，回应里写明，头自己换 | 一张 SVG 给所有头，缓存只照源码；网页换成 CSS 变量，终端换成主题色再栅格化 | 头传颜色进来（缓存要照颜色分） |
| 5 | mermaid 这一页独立成页 | 一页一个部件，和 crate 对得上 | 继续写在 `web-module.md` 里 |
| 6 | 核心独立探一次系统的字体库（`miyu-mermaid/src/fonts.rs`），不借画图库内部那一份（施工 W-4） | `mermaid-rs-renderer` 自己找不到字体时只退化成按字数估算宽度，从不报错；`internal_error`（「读不到字体」）要核心自己能判断，只能自己再查一次 | 借画图库画一张探路图、照它有没有报错来判（它不会报错，这条路走不通） |
| 7 | 没能找到稳定触发 `mermaid-rs-renderer` 内部 panic 的源码，没在真机测到一次真的 panic（施工 W-4） | 试过几十种边角情况（空白、半截语法、递归很深的箭头、二进制字节）都只报解析错误，没有一次崩；崩溃的捕获（`catch_unwind`）靠代码审查，加一个独立单测验 panic payload 的几种类型都翻译得出原话 | 为了测它去改画图库的源码造一次真的崩溃（这个库不是我们维护的，不改第三方代码） |
| 8 | `style.json` 没有 `note` 字段，`fonts`、`marks`、`max_source`、`keep` 都是纯数据，读的时候拒绝不认识的格（施工 W-4） | 两个演示的同名资源文件里带了一句 `note`，但这个仓库没有先例；数据文件越简单越好读、越不容易写错 | 照抄演示的格式，加一格 `note`（又要在 Rust 结构体里接一个没人用的字段） |
| 9 | 懒初始化只试一次：第一次 `render` 读 `style.json`、探字体，成不成都记进去，这个核心的生命周期里不会再重试；失败的 `WARN` 只记这一次（施工 W-4） | 图纸写「之后一直留着」，没说失败要不要重试；一直重试一个本来就读不懂的文件、一直探一个确实没有字体的机器没有意义，还会把日志刷满 | 每次调用失败都重试一次（装环境的人改好了字体，不用重启核心）：图纸没这么要求，多了一份不确定性 |
| 10 | 运行日志的目标用 `miyu::mermaid`，不是 `miyu::endpoint`（施工 W-4） | 和以后的 `net` 包用 `miyu::net` 一个道理：软件包自己的问题，用自己的目标，`web-module.md` 的组合日志表里 `link preview failed` 那一行也特地标了「目标 `miyu::net`」 | 照 `web-module.md` 日志表默认的 `miyu::endpoint`（那张表是 W-1 到 W-11 合在一起的参考表，没给 mermaid 单独标目标是起草时的疏漏） |
| 11 | 「没编进来回 `unknown_method`」不额外编一份关掉 cargo 开关的核心，改测查询表本身（施工 W-4，`web-module.md`「验收」允许的两种办法之一） | 关掉开关要单独一次 `cargo test --no-default-features` 构建，门禁（`cargo xtask check`）不跑这个组合；查询表没登记这个方法名，和这个方法所在的软件包没编进来，端点看到的是同一个结果 | 在 CI 里加一条额外的构建组合（门禁要跟着改，`cargo xtask check` 的「八项」变九项） |
| 12 | 可选软件包登记查询的接口（`queries.rs`）：`Queries::register` 收 `Fn(Arc<Core>, Value) -> impl Future<Output = Result<Value, QueryError>>`，`QueryError` 是给软件包用的最小拒绝集合，不直接暴露 `Refusal`（施工 W-4） | `Refusal`（JSON-RPC 错误码、`message` 的中英文）是端点内部的细节，`miyu-core` 这样的外部 crate 不该认得这些；`QueryError` 只有参数不对、内部出错、一个原因码（可带一格 `data`）三种，够 `mermaid.render`、以后的 `link.preview` 用 | 把 `Refusal` 整个公开给软件包用（软件包要学一整套协议层的东西） |
