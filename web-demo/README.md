## 网页演示程序

和后端并行做的网页头，连真核心。长什么样、为什么这样，见蓝图 `docs/blueprint/web.md`。

原生 JavaScript，按 ES 模块组织，不用框架、不用构建（设计 21 X2）。不进仓库的 workspace，门禁不查它。

### 怎么跑

浏览器只会说 WebSocket，核心听的是本机的套接字，中间要一个桥（`bridge/`）：它给页面文件，把 `/ws` 转到核心，
替页面出示本机令牌。在工作树根目录：

```sh
cargo build -p miyu
cargo build --manifest-path web-demo/bridge/Cargo.toml
MIYU_HOME=<单独的测试目录> MIYU_RESOURCES=$PWD/resources MIYU_CORE_BIN=$PWD/target/debug/miyu \
  DEEPSEEK_API_KEY=<key> MIYU_DEV_BASE_URL=<地址，写到 /v1> MIYU_DEV_MODEL=<模型> MIYU_DEV_WINDOW=300000 \
  web-demo/bridge/target/debug/miyu-web-bridge 8766
```

- 打开桥打出来的链接（`http://127.0.0.1:8766/#k=…`）。口令这一次启动有效，没有它连不上。最后那个数是端口，不写是 8765。
- 变量写在命令前面，不要 `export`；key 和地址只放在命令里，不写进任何文件。
- 核心没在跑时桥用 `MIYU_CORE_BIN` 拉起来；核心只在起来那一刻读环境变量，改了变量先停核心。桥停了核心不跟着停。
- 新会话在起桥的那个目录里干活。
- 时间线上工具的显示名、结果那一句由桥照 `MIYU_RESOURCES` 读（`web.human`）；换了桥的代码要重新编、重启桥。
- 桥还顶着核心以后的网页模块做几样（蓝图 `web.md` 最后一节）：画 mermaid 图（`web.mermaid`）、给本机文件和 blob（`/file`、`/blob`，带口令，数据根不给）、抓链接卡片（`web.link_preview`、`/link-image`，要能联网，内网地址不抓）。
- 预览工作区：让她用 `write` 把产物写进这个会话工作目录下的 `artifacts/`，写完右边自动打开（窗口够宽时）。

### 测试

纯逻辑（事件 → 条目、时间线的段和字、差异、左栏的一项、框下面那一行、数的写法、回答的 Markdown 怎么解析和上色、预览工作区放什么、媒体卡片）有测试，拿仓库里事件的样本当底料，不开浏览器：

```sh
node --test web-demo/tests
cargo clippy --manifest-path web-demo/bridge/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path web-demo/bridge/Cargo.toml
```

只在开发时用 Node；页面本身不需要 Node。

### 拷进来的库（`vendor/`）

回答的 Markdown 用（蓝图 `web.md`「她的回答：Markdown」第 8 条），照旧版网页 `~/Documents/github/Miyu/web/vendor/` 的那几份原样拷来，
一个字没改；页面只从本机取，不往外连：

| 库 | 版本 | 许可证 | 拷了什么 | 怎么用 |
|---|---|---|---|---|
| [Prism](https://prismjs.com) | 1.29.0 | MIT | `prism/prism.min.js`：旧版挑的几种语言拼成一个文件，文件头写着怎么重拼 | `data-manual` 载进来，只调 `Prism.tokenize`（`src/markdown/highlight.js`） |
| [KaTeX](https://katex.org) | 0.18.4 | MIT | `katex/katex.min.js`、`katex/katex.min.css`、`katex/fonts/*.woff2` | `katex.render`（`src/markdown/math.js`） |
