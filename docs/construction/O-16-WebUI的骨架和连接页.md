## 施工单 O-16：WebUI 的骨架和连接页

状态：已完成（2026-10-07 起草；页面的样子照 `onebot.md` 第二条，项目主人过目；项目主人 2026-10-08 在浏览器里试过，试出的四处在「补二」里改了）。

### 目的

`miyu-onebot` 自己的 WebUI 的骨架，加第一页「连接」：NapCat 连没连上、要在 NapCat 里填什么、换令牌、改两个端口。做完以后，接 NapCat 不用再手改配置文件、手存密钥。接在 O-8 后面（分支从 `step/O-8-桥` 开），O-8 先合。

### 蓝图改哪几节

- `docs/blueprint/onebot.md` 第二条（已经起草：对外的样子、样子、怎么走、施工时定的），做完照实补「在哪」「出错」「给人看的字」「守着它的」。
- 新页 `docs/blueprint/webserve.md`：共用的底子是什么、在哪、每个函数从 `miyu-web` 的哪里搬来（一张表，W-11 变基时照它挪）、守着它的。
- `docs/blueprint/web-ui.md`「在哪」：搬走的那几样写明现在在 `miyu-webserve`。
- `docs/designs/01-架构.md` 第九节：第 3 层登记 `miyu-webserve`（不是纯逻辑）。第 3 层不许依赖 `miyu-web`、`miyu-onebot`。
- `crates/miyu-core/src/settings.rs`、`resources/core/human/{zh,en,ja}.json`、配置的样本：加 `onebot.web`，照 `onebot.listen` 的先例（权宜，9-1 挪进清单）。
- `xtask/src/ledger.rs`：`resources/software/onebot/web/` 豁免（给浏览器的，不给模型看；照 `web/` 的先例）。
- `resources/software/onebot/human/{zh,en,ja}.json`：页面上的字。

### 不做什么

- 「主人与自己人」页：O-17。场所、平台工具、插件、日志几页：群接通以后。
- 重启按钮：9-4 以后。
- 「接平台」能力的检查：多用户那一段。
- 不碰核心的登录（`miyu-endpoint` 的 `login.rs`）。

### 验收

1. `miyu-web` 一个字节不变：它原有的测试、`web-module.md`、`web-ui.md`「守着它的」里列的响应头和内容安全策略的样本测试，照过。抽出去的函数原样搬，不顺手改名、改行为。
2. 测试先写，退回改之前要红：Host、Origin、页面路径、`/ws` 转发（照网页软件的测试搬一份对着 `miyu-onebot` 跑）、`/status` 不带令牌 401、带错的 401、带对的回状态、60 秒内不再握手、`restart_needed`、端口被占说清。
3. 手写变异 15 个左右，全被逮住。
4. `CARGO_BUILD_JOBS=5 cargo xtask check` 全过；跑之前看一眼有没有别的门禁在跑，撞上了不算、重跑。
5. 分支推上去，三台机器的 CI 和长跑全绿。
6. 项目主人在浏览器里试：`miyu-onebot serve` 起来，`miyu-onebot web` 打开，登录，看 NapCat 的状态；换一个令牌，抄进 NapCat，重启桥，NapCat 连上。

### 搬家表

（和 `docs/blueprint/webserve.md`「搬家表」一字不差，2026-10-07 补：核心的主会话接着做 W-11 打包时，照它把 W-11 改过 `serve.rs`、`ws.rs` 的几十行挪进 `miyu-webserve`。）

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
| `src/open.rs` 的 `Core`、`Core::call` | `src/open.rs` 的 `Core`、`Core::call` | `pub` |
| `src/open.rs` 的 `Core::connect` | `src/open.rs` 的 `Core::connect` | 参数 `launch: &Launch` 改成 `core: &CoreCommand`，多一个 `kind`（握手报的头，网页软件报 `miyu-web`，和原来一样）；握手回的语言存成原样的字，`miyu-web` 照它算 `Language` |
| `src/open.rs` 的 `locale` | `src/open.rs` 的 `locale` | `pub` |
| `tests/ws.rs` 的 `the_forwarding_code_never_reads_the_local_token` | 不搬，照搬过去的位置查 `miyu-webserve` 的 `ws.rs`、`pages.rs`、`respond.rs`、`lib.rs` | |

留在 `miyu-web` 的：单实例、听端口、`run/web`、那一行、空闲退出（`Site` 的数忙、`idle_for`、`Busy`）、`handle` 的分派和 Host 不对时的那一行运行日志、`/media`、`web.json`、`open` 的分派和给人看的字、`stopped`。

### 风险

- 抽共用底子动了 `miyu-web`：照验收第 1 条全套照过；W-11（打包，停着）改过 `serve.rs`、`ws.rs`，以后变基会撞，`webserve.md` 里那张搬家表就是给它用的（已告诉核心的主会话）。
- 浏览器里的页面没有自动测试：页面的逻辑尽量薄，读写都是核心现成的方法；项目主人试一次兜底。

### 补：没令牌也起来（2026-10-07）

**目的**：照 `18-通讯平台.md` 第三节「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」。原来令牌没设，`serve` 和 `web` 都退出（退出码 1），WebUI 打不开，第一次用要先在命令行 `miyu login onebot`、`miyu config set`。补了以后：令牌没设、引用的密钥或环境变量取不到，`serve` 照常连核心、开 WebUI，不开 NapCat 的端口，标准错误、运行日志各说一句；`web` 照常打开；人在「连接」页按「生成」，抄进 NapCat，重启桥。是照设计改，不是新的产品决定。端口写错（`onebot.listen`、`onebot.web` 读不出来）照旧起不来。

**改了哪几节**（`docs/blueprint/onebot.md`）：

- 「在哪」：`main.rs`、`settings.rs`、`serve.rs` 三行。
- 第一条：「对外的样子」`onebot.token` 那一行；「怎么走」第 1 条；「样子」；「出错」去掉「令牌没设」退出码 1 那一行，换成不算错的一行；「给人看的字」`unready/no-token` 换成 `notice/no-token`（意思改成去 WebUI 生成）；「守着它的」读配置那一条改、加一条；「施工时定的」第 3 条改成「NapCat 必须带令牌；没设时桥照样起来，只开 WebUI」，第 6 条去掉「令牌没设不连核心」。
- 第二条：「状态」；「对外的样子」`/status`（`listen` 可以是 `null`）、`miyu-onebot web` 两行；「怎么走」第 1 条、第 3 条（连接页：没开端口那一句、「生成」按钮）；「出错」原来「`miyu-onebot web`：令牌没设」那一行；「给人看的字」页面的字多两样；「样子」的实际；「守着它的」加一条；「施工时定的」加第 14 到 16 条。

**验收**：

1. 测试先写，退回改之前要红：桥在没令牌时照常起来、不说在哪等 NapCat、NapCat 的端口连不上、`/status` 的 `listen` 是 `null`；`restart_needed` 起来时没令牌、现在有了是真，一直没有是假；真的程序 `miyu-onebot serve` 没令牌照常起来（标准错误、运行日志各一句），`miyu-onebot web --print` 没令牌照常印网址。
2. 手写变异 8 个左右，全被逮住。
3. `CARGO_BUILD_JOBS=5 cargo xtask check` 全过。
4. 项目主人在浏览器里试：没设令牌时 `miyu-onebot serve` 起来、`miyu-onebot web` 打开，「连接」页令牌那一行「没设」、按钮「生成」，NapCat 那一行说没开端口；按「生成」，新令牌显示一次、页顶提示重启；抄进 NapCat，重启桥，NapCat 连上。

### 补二：项目主人试过以后的四处（2026-10-08）

项目主人 2026-10-08 在自己的 NapCat 上试了 O-8、O-16：私聊来回通过；页面上卡了四处，照他说的改。

**目的**

1. 令牌随时能看、能复制，不做「只显示一次」（2026-10-08 项目主人定，推翻 2026-10-07「只在刚生成时显示一次」）。
2. 没令牌时页面写清先做什么，「生成」放显眼处（试的时候不知道该干嘛）。
3. 改了令牌、端口当场生效，不用重启桥（试的时候要重启才连上）。
4. 页面上的地址显示短的 `ws://127.0.0.1:<端口>/ws`（两个路径桥都认，短的好填）。

**蓝图改哪几节**（`docs/blueprint/onebot.md`）

- 第一条：
  - 「怎么走」第 1 条：NapCat 的端口照常开，令牌没设、取不到时一律 401（不再「不开端口」）。
  - 第 2 条（连进来）：令牌对不上或者还没有时，桥重读一次配置（同 `/status` 的重读，`Config::load`）再核对一次；对上了照新的用，以后都照它比。重读有节流（同一秒里只读一次），免得乱连的把读配置当成负担。换了令牌以后，已经连着的那一条不断（它握手时出示的是旧的、当时对），下次重连照新的。
  - 失败表、给人看的字：`notice/no-token` 改成「还没设令牌，NapCat 连进来会被拒。到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上。」这类意思。
- 第二条：
  - `/status`：`listen` 照实际听的端口（不再是 `null`）；多一格 `token`（`"set"`、`"none"`、`"missing"`：设了、没写引用、引用的密钥没存）；`restart_needed` 去掉。
  - 新的 `GET /token`：同 `/status` 要登录令牌（`Authorization: Bearer`），回 `{"token": "<值>"}`，没有的回 `{"token": null}`；`Cache-Control: no-store`；运行日志只记取过，不记值。
  - 新的 `POST /apply`：同样要登录令牌；桥重读配置，令牌照新的；两个端口变了的关掉旧的、开新的；回 `{"listen": 端口, "web": 端口}`，开不了新端口的回 409 和哪个端口被占，旧的照旧开着。
  - 「连接」页：
    - 令牌那一行平时「已设 ········」，「显示」取 `/token` 显示出来（再按收起），「复制」取 `/token` 放进剪贴板，「换一个」先问一句（「NapCat 那边也要跟着换」）再生成；没设时只有一个显眼的「生成」（主按钮的样子）。生成、换完直接显示出来，不再写「只显示这一次」。
    - 没令牌时页顶一段三步：① 生成令牌；② 把地址和令牌填进 NapCat 的「WebSocket 客户端」，消息格式选数组；③ 等几秒，这里变成「已连上」。连上以后这段不显示。
    - 端口「保存」以后调 `/apply`：NapCat 的端口变了，提示去 NapCat 里改地址；WebUI 的端口变了，页面跳到新地址（登录记在浏览器里、按地址分，要重新登录一次，页面先说一句）。
    - 「改了端口或令牌，重启以后生效」那一行去掉。
    - 地址照 `ws://127.0.0.1:<onebot.listen>/ws`。
  - 施工时定的：「只显示一次」那一条改写（2026-10-08 项目主人定：本机、要登录、只给管理员的页面，方便优先；令牌是桥自己的凭据，由桥给登录的管理员看，不经核心交出去，和 `config.md` 第九条不冲突）；「restart_needed」那一条改写。

**验收**

1. 先写测试，退回改之前要红：没令牌时 NapCat 连进来 401；之后写进令牌（测试里照页面的办法 `secret.set` + `config.set`），不重启，NapCat 下一次连就通；换令牌以后旧的不收、新的收、已经连着的不断；`/token` 不带、带错登录令牌 401，带对的拿到值；`/apply` 改 NapCat 的端口以后旧的连不上、新的连得上，端口被占回 409、旧的照旧；`/status` 的 `token` 三种。
2. 手写变异 10 个左右，全被逮住。
3. 截图：没令牌时的三步、显示令牌、换令牌的确认（亮色、暗色）。
4. 门禁八项全过；三台机器的 CI 和长跑全绿。
5. 项目主人看一眼：不用重启就连上；令牌随时能看能复制。

**验收结果（2026-10-08）**

1. 测试先写，退回改之前全红（13 个；节流那一个因为 `bridge.json` 还没有 `reload_seconds`，夹具 `reload_every` 先写成 panic 算红），改完 56 个全过，连跑 52 遍没有红：
   - `no_token.rs`：没令牌时两个端口都开、先说在哪等 NapCat 再说 `notice/no-token`、NapCat 401、`/status` 的 `listen` 是实际端口、`token` 是 `none`；真的核心上照页面的办法 `secret.set` + `config.set` 写进令牌，不重启 NapCat 下一次连就通；再换一个，新的进得来、旧的 401、已经连着的那一条照样收发（私聊来回一趟）；真的程序 `serve`：标准错误三句、改配置文件 `/status` 的 `token` 从 `none` 到 `missing` 到 `set`、NapCat 不重启就连上、`/token` 交出值、运行日志有 `no token yet`、`web token read`，没有令牌的值，`web --print` 照常。
   - `token.rs`：不带、带错（多一个字、少一个字、`Token` 方案、空的）401 且不带值；带对的拿到值、`no-store`、`nosniff`；`POST` 405；没写引用、取不到的是 `null`。
   - `apply.rs`：不带、带错 401，`GET` 405，什么都没变回原来两个端口；换 NapCat 的端口：新的连得上、旧的关掉、`/status` 跟着说、再照一次不换；换 WebUI 的端口：新地址上页面和 `/status` 照常、旧的关掉；被占的（NapCat 的、WebUI 的、两个都变而 WebUI 的被占）回 409 和端口，旧的照旧、开好的新端口放掉；`/apply` 换令牌：旧的 401、新的进得来、连着的那一条还在；拿错的令牌连五次只重读一次配置，对得上的不读，`/apply` 照读。
   - 改的：`status.rs`（`token` 代替 `restart_needed`，去掉要不要重启那一个）、`settings.rs`（令牌三种）、`tuning.rs`（`reload_seconds` 是 1、少了读不进来）、`texts.rs`（`notice/no-token` 的新说法）。
2. 手写变异 14 个，全被逮住：节流失效、对不上从不重读、重读了不换上令牌、手里没令牌时谁都放进来、令牌设了反而说没设、`/apply` 每次都重开端口、`/apply` 开上一个算一个、换了 WebUI 端口不改 Host 核对的端口、`/apply` 不记下这一次照的端口、`/token` 不核对登录令牌、`/status` 不重读配置、`/status` 把没写引用说成取不到、取不到的令牌读成没写引用、给页面的 JSON 能缓存。
3. 截图（无头 Chromium，真的核心和桥，用完停掉）：`/tmp/claude-1000/-home-shorin-Documents-github-miyu-agent-remake/1dce34c0-a651-41a2-97ec-7cae11e86ec6/scratchpad/o16-fix2/` 里的 `guide-{light,dark}.png`（没令牌时的三步）、`token-shown-{light,dark}.png`（显示令牌，第一步打勾）、`confirm-{light,dark}.png`（换令牌之前问一句），另有 `connected-light.png`（用页面上显示的令牌连假 NapCat，不重启就连上、三步收起）、`ports-moved-light.png`（页面上改 NapCat 的端口）。同一趟里在页面上改 WebUI 的端口：先说一句、跳到新地址、要重新登录，旧的两个端口关掉。
4. `CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过（第一趟「文档」红在公开模块的说明链到了私有的 `Swap`，改了；「测试」红在 `miyu-endpoint` 的 `extensions::always_ones_start_with_the_core_and_a_new_core_follows_the_switches` 等超时，这一步没碰它，重跑过）。
5. 三台机器的 CI：变基到 main 以后全绿（run 1221）。
6. 项目主人让主会话自己验（2026-10-08）：测试数据根起真桥，拿掉令牌引用时照 NapCat 的样子握手 401；不重启、写回令牌引用以后再握手 101，日志 `token changed`、`napcat connected`；错的令牌 401。项目主人自己的 NapCat 这段时间没来重连 8301（它对这一路停了重试），正常在 NapCat 里改令牌、保存会当场重连，不受影响。

**追加（2026-10-08 主会话定）**：O-8 合进 main（`2e49080e`）以后，O-16 接到 main 上（`git rebase --onto origin/main 86578a3d`）。核心里替桥声明的三项跟着补二改：`onebot.token` 的生效时机改 `now`，说明写「NapCat 下一次连进来就照新的」，没设令牌那一句改成照样起来、NapCat 会被拒；`onebot.listen`、`onebot.web` 留 `head_start`，说明补「在 QQ 桥的网页上改、保存的，当场生效」，地址写 `/ws`（`crates/miyu-core/src/settings.rs`、`resources/core/human/{zh,en,ja}.json`，样本照 `MIYU_SAMPLE_WRITE=1` 重写，`config.md` 的配置项表、样本块、说明的表跟着改；`onebot.md` 第二条「施工时定的」第 24 条）。
