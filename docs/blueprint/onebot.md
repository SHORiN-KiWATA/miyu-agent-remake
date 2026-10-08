## 通讯平台的桥 `miyu-onebot`

### 是什么

软件包 `miyu-onebot`：经 OneBot v11 接 QQ 的桥，和终端界面、网页平级的一个头（`docs/designs/18-通讯平台.md` 第三节、Q17）。它把 QQ 上的人接进场所会话，把她的回复发回 QQ；要不要开口、限流、出站这些和平台无关的部分在群聊内核 `miyu-chat`（`chat.md`），这里只管 QQ 这一头和跟核心的那一头。

状态：图纸，随施工 O-8 起草（2026-10-07）。O-8 只有骨架：主人的私聊、只有文字（第一条）；O-8 补照 `chat.md` 第七条第 1 条改了编号的拼法（第一条第 7、8 条）；WebUI（第二条）随 O-16、O-17 起草，O-16 做了骨架和「连接」页，O-17 做了「主人与自己人」页。群、图片和文件、斜杠命令、出站链与出站队列、`start/stop/status/logs`、WebUI 随后面的步子。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-onebot/`（第 5 层，头） | 桥 |
| `crates/miyu-onebot/src/main.rs` | 程序的入口：`miyu-onebot serve`：找资源目录、读给人看的字，装运行日志、读配置和 `bridge.json`；`miyu-onebot web [--print]`（O-16）：读配置，交给 `open.rs`（令牌没设的照样往下走，O-16 补） |
| `crates/miyu-onebot/src/settings.rs` | 起来时读的三项（`onebot.web` 随 O-16）：照核心登记的清单读系统配置、密钥文件（`miyu-endpoint` 的 `Config::load`，只读），令牌照引用取，读成三种：取到了、没写引用、写了引用取不到（`Token`，O-16 补二；没取到的不算起不来）；握手以前说话的语言；重读用的 `Reload` |
| `crates/miyu-onebot/src/tuning.rs` | 读 `bridge.json`：桥自己的数（O-16 多 `web` 一格，补二多 `reload_seconds`） |
| `crates/miyu-onebot/src/serve.rs` | 起来：连核心、开两个监听（NapCat 的、WebUI 的），把几样接起来；`/apply` 开好的新监听换掉旧的（O-16 补二）；核心断了、跟核心的那一头崩了就退 |
| `crates/miyu-onebot/src/current.rs`（O-16 补二） | 桥手里的令牌：起来时读到的那个；每次重读配置（`/status`、`/token`、`/apply`、NapCat 对不上时）照读到的换上；NapCat 对不上时的重读有节流 |
| `crates/miyu-onebot/src/web.rs`（O-16） | WebUI：核对 Host，`/ws` 原样转给核心、`/status`、`/token`、`/apply`（补二）、`/human`、页面文件；底子是共用的 `miyu-webserve`（`webserve.md`） |
| `crates/miyu-onebot/src/web/login.rs`、`web/checked.rs`（O-16） | 登录令牌拿去和核心握手验、验过的记一阵（只记哈希）；`/status`、`/token`、`/apply` 都照它（补二从 `status.rs` 挪出来） |
| `crates/miyu-onebot/src/web/status.rs`、`web/token.rs`、`web/apply.rs`（O-16，后两个补二） | `/status`：NapCat 的状态、两个端口、令牌设没设、平台的名字（O-17）；`/token`：令牌的值；`/apply`：重读配置、换端口 |
| `crates/miyu-onebot/src/web/human.rs`（O-16） | `/human`：登录以前页面要的字 |
| `crates/miyu-onebot/src/open.rs`（O-16） | `miyu-onebot web`：桥在不在跑、要一次性码、开浏览器 |
| `crates/miyu-onebot/src/listen.rs` | NapCat 反连进来的那一下：路径、令牌（对不上时重读配置再比，O-16 补二）、升级 |
| `crates/miyu-onebot/src/listen/connection.rs`、`listen/bots.rs` | 一条 NapCat 的连接：回应、私聊、别的事件各交给谁，连上就问 `get_version_info`（回的实现、版本记在这条连接上，O-16 的 `/status` 用）；一个机器人号一条连接，新的顶掉旧的 |
| `crates/miyu-onebot/src/onebot.rs`、`onebot/text.rs`、`onebot/calls.rs` | OneBot v11 的事件和动作：认一帧（私聊带上事件的 `time`）、读出私聊的文字、写 `send_private_msg`、调用和回应按 `echo` 配对；平台的名字 `qq`（`PLATFORM`）只写在 `onebot.rs`。三个小函数照它拼编号：`private_venue(号) -> Result<VenueId, FormatError>`、`person(号) -> Result<ExternalId, FormatError>` 经群聊内核拼（`Venue::new`、`miyu_chat::person`），`command_id(机器人的号, 消息编号, 时刻) -> String`（第 7、8 条） |
| `crates/miyu-onebot/src/core.rs`、`core/route.rs` | 跟核心的那一头：握手、`venue.session`、带 `as` 的 `session.send`、订阅、她的回复发回去 |
| `crates/miyu-onebot/src/texts.rs` | 说给人听的字：挑哪一句、换进什么字段，字照 `Human::load` 读（「给人看的字」） |
| `resources/software/onebot/bridge.json` | 桥自己的数：认的路径、调用等多久、两个队列多长、接不了连接歇多久、令牌对不上时多久才重读（「对外的样子」） |
| `resources/software/onebot/human/{zh,en,ja}.json` | 桥说给人听的字（「给人看的字」）；WebUI 页面的字（`web/` 开头，O-16；`web/people/` 开头的 O-17，第二条「给人看的字」） |
| `resources/software/onebot/web/`（O-16） | WebUI 的页面：`index.html`、`app.js`（登录、骨架、「连接」页）、`people.js`（「主人与自己人」页，O-17）、`style.css`，原生 JS 的模块（第二条「施工时定的」第 25 条） |
| `xtask/src/ledger.rs` | 登记簿门禁豁免 `software/onebot/bridge.json` 这一份文件：是数据，不发给模型；O-16 再豁免 `software/onebot/web/` 这一个目录：给浏览器的 |
| `crates/miyu-core/src/settings.rs` | `onebot.listen`、`onebot.web`（O-16）、`onebot.token`、`onebot.trusted`（O-17）四项配置（`OnebotSettings`）：权宜，照 `tui.startup` 的先例先由核心声明，以后桥的配置整体挪进软件包清单的 `[settings]` 时，同一个提交删掉核心这一份；生效时机：两个端口 `head_start`，令牌、自己人 `now`（O-16 补二、O-17） |
| `crates/miyu-config/src/item/settings.rs`、`value.rs` | `settings!` 里整数可以写默认值；单个密钥的设置类型 `Option<Reference>`（O-8 加，两项要用） |
| `resources/core/human/{zh,en,ja}.json` | 四项配置给人看的字（`config.items`，`onebot.trusted` 随 O-17）、组 `onebot`（`config.groups`）；补二：地址写 `/ws`，端口说明补「在 QQ 桥的网页上改、保存的，当场生效」，令牌说明改成没设照样起来、NapCat 下一次连进来照新的 |

### 一、骨架：主人的私聊（施工 O-8）

**对外的样子**

配置（系统配置，桥起来时读。`onebot.token` 的生效时机是 `now`：改了不用重启，NapCat 下一次连进来就照新的（第 2 条）；两个端口是 `head_start`：用命令行改的桥下次起来时生效，在 WebUI 的「连接」页保存的当场换（第二条 `/apply`）。O-16 补二，第二条「施工时定的」第 24 条）：

| 键 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `onebot.listen` | 整数 1024 到 65535 | 8301 | NapCat 反连进来的端口，只听本机 `127.0.0.1` |
| `onebot.token` | 密钥（`{ secret = … }` 或 `{ env = … }`） | 没有 | NapCat 连进来时出示的访问令牌；没设、取不到，桥照样起来，NapCat 连进来一律 401；设了、换了不用重启（第 1、2 条，O-16 补二） |

桥自己的数在资源目录的 `software/onebot/bridge.json`，不进配置清单（照网页软件的 `web.json`）：

| 格 | 出厂 | 说明 |
|---|---|---|
| `paths` | `["/onebot/v11/ws", "/ws"]` | NapCat 连得进来的路径（第 2 条） |
| `call_timeout_seconds` | 10 | 一次 OneBot 调用等回应最多几秒（第 4 条） |
| `write_queue` | 64 | 往一条 NapCat 的连接写，最多攒几帧没写出去；满了写的一方等着。至少 1 |
| `inbound_queue` | 256 | 读出来的私聊最多攒几条没交给跟核心的那一头；满了读 NapCat 的那一头等着。至少 1 |
| `accept_retry_millis` | 100 | 接不了 TCP 连接（打开的文件太多这类）时歇几毫秒再接，不空转 |
| `reload_seconds` | 1 | NapCat 出示的令牌对不上（或者桥手里还没有令牌）时重读配置；离上一次这样重读不到这么多秒的不读，照手里的比（第 2 条，O-16 补二） |
| `web` | 见第二条「对外的样子」 | WebUI 的数（O-16）：`csp`、`types`、`status_cache_seconds` |

多一格、少一格、队列写 0、读不了：起不来（「出错」）。

命令：`miyu-onebot serve`：前台跑，Ctrl+C、SIGTERM 停（`miyu-onebot web` 见第二条）。9-4 以前由人手动起；9-4 以后由核心拉起，`miyu onebot start/stop` 开关。

NapCat 那边要配成「反向 WebSocket」（NapCat 的网络配置里叫「WebSocket 客户端」），地址 `ws://127.0.0.1:<onebot.listen>/ws`（`/onebot/v11/ws` 也认，WebUI 上写短的那个，O-16 补二），访问令牌和 `onebot.token` 一样，消息格式选数组（字符串格式也认，第 6 条）。

**怎么走**

1. **起来**：找资源目录、读给人看的字；读系统配置的几项；读 `bridge.json`。用本机套接字连核心，没在跑就拉起（照终端界面）；握手的回应没带 `language`（`protocol.md`「握手」说一定带）是协议不对，照连不上核心说、退出码 1。然后开两个监听（NapCat 的、WebUI 的）；端口被占了说清是哪个端口，退出码 1。令牌没设、引用的密钥或环境变量取不到：照样起来，两个端口照开，说完在哪等 NapCat 再说一句 `notice/no-token`（到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上），运行日志记一行 `WARN no token yet`。NapCat 这时连进来一律 401；人在 WebUI 里生成令牌以后，NapCat 下一次连就通，不用重启（第 2 条；18 第三节「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」，O-16 补、补二）。
2. **连进来**：只认 `bridge.json` 的 `paths`（出厂 `/onebot/v11/ws` 和 `/ws` 两个），别的 404。令牌照 `Authorization: Bearer <令牌>`、`Authorization: Token <令牌>` 或查询参数 `access_token` 取，和桥手里的按常数时间比。对不上、桥手里还没有的，重读一次配置（同 `/status` 的重读，`Config::load`）再比一次，对上了照新的用，以后都照它比（`current.rs`，O-16 补二）。这样重读有节流：离上一次这样重读不到 `reload_seconds`（出厂 1 秒）的不读，照手里的比，乱连的不能把读配置变成负担。还对不上 401。令牌换了以后，已经连着的那一条不断（它握手时出示的是当时对的），下次重连照新的。WebUI 每次读配置（第二条 `/status`、`/token`、`/apply`），桥手里的令牌也跟着换上。不是 WebSocket 的升级请求 400；算出来的 `Sec-WebSocket-Accept` 放不进回应的头（照说不会），也是 400，不升级。机器人的号照 `X-Self-ID` 头取，没有的等第一条事件的 `self_id`。同一个号再连进来，新的顶掉旧的。
3. **对端是谁**：连上以后调一次 `get_version_info`，把实现的名字和版本记进运行日志（18 第十三节：排查时先确认连着的是哪个实现）。quirk 表随后面的步子。
4. **调用**：发出去的动作带 `echo`（桥自己编，同一条连接里不重），等带同样 `echo` 的回应，等了 `call_timeout_seconds`（出厂 10 秒）还等不到算失败；连接断了，在等的都算失败。
5. **收私聊**：`post_type = message`、`message_type = private` 的才看；别的事件（群消息、通知、请求、心跳、生命周期）记一行调试日志就丢。
6. **读出文字**：`message` 是段的数组时，取 `text` 段的 `data.text` 依次接起来；是字符串（CQ 码）时，去掉 `[CQ:…]`，再把 `&#91;`、`&#93;`、`&#44;`、`&amp;` 换回来。图片、表情这些别的段这一步不管。接出来的字去掉首尾空白是空的，不送。
7. **找会话**：`venue.session {venue: "qq:private:<user_id>", kind: "private", peer: "qq:<user_id>"}`。场所编号由群聊内核的 `Venue::new("qq", VenueKind::Private, <user_id>)` 拼，平台上的人由 `miyu_chat::person("qq", <user_id>)` 拼，桥不手拼（`chat.md` 第七条第 1 条）；拼不出来的（号是整数，照说不会）记一行运行日志 `WARN`、这条不送，不 panic。桥在内存里记着「场所 → 会话编号」，每个场所只在桥起来以后第一次来消息时问；回 `session_not_found` 这类会话不在了的，忘掉、再问一次。回 `no_system_account`（不是主人，O-4 以前没有系统账号）：这个人的消息这一步不接，同一个人只记一行运行日志。
8. **发进去**：`session.send {session, text, as: {external: "qq:<user_id>"}}`（`as.external` 和第 7 条的 `peer` 是同一个，照第 7 条拼），请求的 `id`（命令编号）是 `qq:<机器人的号>:<message_id>:<time>`（`chat.md` 第七条第 1 条）：断线重连、平台重发，核心都只收一次（`venues.md`）。`time` 是事件里平台给的时刻，整数秒，写成整数的字符串也认（和号一样）；加上它，NapCat 重置本地库以后消息编号重号，新消息也不会被当成重发吞掉。没带 `time`、读不出来的照 `0` 拼，消息照送（「施工时定的」第 20 条）。回的是重了，当成功。
9. **订阅**：找回一个会话以后订阅它的事件流，不写 `after`：只要订阅以后的新事件，桥重启不会把以前的回复再发一遍。桥起来以前她说的、桥断着时她说的，这一步不补发（出站队列随后面的步子）。
10. **发回去**：事件流里每来一条 `message.assistant`，取它的 `text` 块依次接起来（思考、工具调用不发），去掉首尾空白，不是空的就 `send_private_msg {user_id, message: [{type: "text", data: {text}}]}` 发回那个私聊。哪条连接：这个会话是哪个机器人号收进来的，就用那个号现在的连接；没连着的，记一行运行日志，丢掉（出站队列随后面的步子）。
11. **断开**：NapCat 断了，等它自己重连，桥不退。核心断了，桥照人的语言说一句、退出码 1：9-4 以前由人重起，9-4 以后核心拉起。跟核心的那一头崩了、发回话的任务崩了（都是 bug），说一句带原话、退出码 1，运行日志记一行 `ERROR`。
12. **运行日志**：照 `miyu-log` 写 `state/logs/onebot.log`，满了照核心的换法。消息正文不进运行日志，只记场所、消息编号、字数（`28-运行日志.md`）。

**样子**：桥起来时在标准错误上说一行「在 127.0.0.1:8301 等 NapCat 连进来」，照握手回的语言（令牌没设的接着再说 `notice/no-token` 那一句，O-16 补二）；NapCat 连上、断开各一行。

**出错**

| 情况 | 说什么 | 退出码 |
|---|---|---|
| 令牌没设、取不到（O-16 补：不算错） | `notice/no-token`，照常跑，NapCat 连进来 401，设了就通（第 1、2 条，补二） | 不退 |
| 端口被占 | 哪个端口被占了、改 `onebot.listen` | 1 |
| 连不上核心、握手的回应没带 `language` | 连不上核心，带原因 | 1 |
| 核心断了 | 核心不在了 | 1 |
| 跟核心的那一头崩了、发回话的任务崩了（是 bug） | 出了错、停下，带原话 | 1 |
| 别的原因起不来：听不了、起不了运行时、数据根用不了、`bridge.json` 读不进来 | 起不来，带原话 | 1 |
| 找不到资源目录、给人看的字读不懂 | 这时还没有字可用：`miyu-onebot: <原话>` | 1 |
| 用法不对 | 用法 | 2 |

**给人看的字**（标准错误上）：放在 `resources/software/onebot/human/{zh,en,ja}.json` 的 `said` 里，照核心的格式和 `Human::load` 的读法（`store/resources.md`「怎么走」第 3 条），说法的编号是 `software/onebot/<编号>`；`texts.rs` 只挑哪一句、换进什么字段，换不出来的（是 bug）印出编号和字段、记一行运行日志。`ja.json` 照英文写，和核心拒绝时的话一样（O-8 施工时定）。读配置以前照系统的语言，读了配置照 `ui.language`，握手以后照核心回的 `language`；换成的那种语言的字读不懂，说一行原话、接着照原来的说。

| 编号 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `notice/listening` | 起来了 | 在 127.0.0.1:{port} 等 NapCat 连进来。 | Waiting for NapCat on 127.0.0.1:{port}. |
| `notice/connected-as`、`notice/connected` | 连上了 | NapCat 连上了（{bot}）。 | NapCat connected ({bot}). |
| `notice/disconnected-as`、`notice/disconnected` | 断开了 | NapCat 断开了（{bot}），等它重连。 | NapCat disconnected ({bot}); waiting for it to reconnect. |
| `notice/no-token`（O-16 补，原来的 `unready/no-token`；补二改了说法） | 令牌没设：照样起来，跟在 `notice/listening` 后面 | 还没设令牌（onebot.token），NapCat 连进来会被拒。到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上。 | No token (onebot.token) yet, so NapCat will be refused. Generate one on the Connection page of the web UI and put it into NapCat; it connects within seconds. |
| `failure/port-in-use` | 端口被占 | 端口 {port} 被占了。换一个：miyu config set --system onebot.listen <端口>，NapCat 那边跟着改。 | Port {port} is in use. Pick another: miyu config set --system onebot.listen <port>, and change NapCat to match. |
| `failure/core` | 连不上核心 | 连不上核心：{reason} | Could not reach the core: {reason} |
| `failure/core-gone` | 核心断了 | 核心不在了，QQ 桥停下。 | The core went away; the QQ bridge stops. |
| `failure/crashed` | 跟核心的那一头、发回话的任务崩了 | QQ 桥出了错，停下：{reason} | The QQ bridge hit a bug and stops: {reason} |
| `failure/start` | 别的原因起不来 | QQ 桥起不来：{reason} | The QQ bridge could not start: {reason} |
| `unready/bad-port` | 端口读不出来（走不到） | onebot.listen 读不出来，照 miyu config check --system 查一下。 | onebot.listen could not be read; check it with miyu config check --system. |
| `usage` | 用法不对 | 用法：miyu-onebot serve \| miyu-onebot web [--print] | usage: miyu-onebot serve \| miyu-onebot web [--print] |
| `no-log` | 运行日志装不上（照样跑） | 运行日志写不了：{reason} | The run log cannot be written: {reason} |

号没认出来的（没带 `X-Self-ID`、还没来事件）用不带 `-as` 的那一句，不写括号那一段。

**守着它的**（`crates/miyu-onebot/tests/`，O-8；假的 NapCat 是测试里的一个 WebSocket 客户端，核心照别的头的测试起一个真的）

- 令牌：三种出示法都认；不对 401（差一个字、多一个字、空的也算）；没出示 401。路径不对 404。端口被占了说是哪个端口。（`listen.rs`）
- 算出来的 `Sec-WebSocket-Accept` 放不进头的回 400、不说升级；放得进的回 101，值照 RFC 6455 的例子。（`src/listen/tests.rs`）
- 发回话的任务崩了，交回「出了错」带原话，桥停下；好好结束的接着办。（`src/core/route/tests.rs`）
- 握手的回应没带 `language`：照连不上核心退，原因里写明。假的核心只回 `hello`。（`core.rs`）
- 主人的私聊走一遍：假 NapCat 发一条私聊，核心里那个场所会话收到一条 `by` 是主人本人、带 `via` 的消息，命令编号是 `qq:<机器人的号>:<消息编号>:<时刻>`；她回了一句，假 NapCat 收到 `send_private_msg`，字对得上。用核心的测试模型替身。（`private.rs`，下面三条同）
- 同一条消息发两次，会话里只有一条；同一个消息编号、时刻不同的是两条，都送进去、都回。没带 `time` 的照样送进去，命令编号的时刻是 `0`。
- 不是主人的私聊：不进任何会话（主人的会话已经有了也不进），不回话。
- 段的数组、CQ 字符串两种格式；只有图片的消息不送。同一个号再连进来，新的顶掉旧的，旧的断开不拿掉新的。
- 思考、工具调用不发回去；空的回复不发。（`replies.rs`，下面一条同）
- 桥重启以后，以前的回复不再发一遍。
- 调用等了给的时限（测试给 3 秒，和出厂的不一样）还等不到算失败，到时以前还在等；连接断了在等的算失败；同时在等的几个照 `echo` 各拿各的。（`calls.rs`，钟停住，照停住的钟算）
- 出厂的 `bridge.json` 读得进、数和上面的表一样；队列写 0、多一格、少一格、不是 JSON、没有文件，都读不进来，说是哪个文件。（`tuning.rs`）
- 三种语言里桥说的每一句都换得出来；中文照上面的表、字段换进去；日文和英文一字不差；换语言照新的说；读配置以前照系统的语言。（`texts.rs`）
- 文字怎么读出来：别的段跳过；CQ 码去掉，`&amp;` 最后换。（`text.rs`）
- 编号：场所、平台上的人和群聊内核拼的一样（`qq:private:<号>`、`qq:<号>`，解得回原样）；命令编号带时刻，同一个消息编号、时刻不同的两条编号不同；`time` 是整数、写成整数的字符串都认，没带、读不出（`null`、不是数的字、小数）的是 `0`。（`ids.rs`）
- 读配置：令牌读成三种：没写引用、写了引用取不到（密钥没存、环境变量没设）、取到了，都照样读得出来（O-16 补、补二）；照密钥文件、环境变量取；端口不写是 8301；握手以前的语言照 `ui.language`。（`settings.rs`）
- 令牌没设（O-16 补、补二）：桥照样起来，两个端口都开，先说在哪等 NapCat、再说 `notice/no-token`；NapCat 连进来 401；经核心的 `secret.set`、`config.set` 写进令牌（照页面的办法），不重启，NapCat 下一次连就通。再换一个：新的连得进、旧的 401，已经连着的那一条照样收发。真的程序 `miyu-onebot serve` 也这样起来（标准错误、运行日志各一句），改配置文件以后 `/status` 的 `token` 从 `none` 变 `missing`、`set`，NapCat 不用重启就连上，`/token` 交出值、运行日志里没有这个值；`miyu-onebot web --print` 照常印出网址。（`no_token.rs`）
- 令牌对不上时的重读有节流（O-16 补二）：一直拿错的连，`reload_seconds` 里只读一次配置。（`apply.rs`）

**施工时定的**（O-8）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | 只要两项配置：端口、令牌；机器人的号照 NapCat 报的 | 握手时 NapCat 自己报，现在用不上第三项 | 先声明 `onebot.self_id` |
| 2 | 默认端口 8301 | 网页软件是 8300，挨着好记；旧版 QQ 和网页共用 8300，NapCat 的端口要跟着网页变（18 第三节） | 和网页共用 |
| 3 | NapCat 必须带令牌；没设时桥照样起来，NapCat 的端口照开、连进来一律 401，设了不用重启（O-16 补，2026-10-07；补二，2026-10-08 项目主人定） | 18 第三节「默认只接本机，并且要带访问令牌」「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」：第一次用在 WebUI 里生成令牌，不用先上命令行；端口先开着，令牌一设 NapCat 下一次连就通（第二条「施工时定的」第 18 条） | 本机连进来的可以不带；令牌没设就不起来（O-8 原来的做法：WebUI 也打不开，和 18 第三节对不上）；令牌没设不开 NapCat 的端口（O-16 补的做法：设了要重启才开） |
| 4 | 订阅不写 `after` | 桥重启不重发旧回复；漏发的由以后的出站队列补 | 记住看到哪条、重启接着推 |
| 5 | 核心断了桥就退 | 9-4 以后核心拉起，崩了照退避重起；桥里不另写一套重连 | 桥自己重连核心 |
| 6 | 桥自己读系统配置和密钥文件，用核心那一份读配置的代码（`Config::load`，照核心登记的全部清单），只读 | 密钥从不经协议交出去（`config.md` 第九条），令牌要用值；读配置在连核心以前。**权宜**：为此依赖了 `miyu-core`、`miyu-endpoint`，和 18 第一节「不依赖核心的 crate」不一样；9-4 以后由核心拉起时，把这个包的设置连同密钥的值经标准输入交给它，到时去掉这两个依赖。代价：桥读配置只认核心登记的清单，系统配置里写了软件包的键（例如 `web.port`）时，`onebot.log` 记一条不认识的键的 `WARN`，值照读；随这段权宜在 9-4（下）一起去掉（2026-10-08） | 经 `config.get` 读：拿不到密钥的值 |
| 7 | 两项放在高级页的「QQ 桥」组（`onebot`），排在「运行日志」后面 | 页的先后不变；一个桥一组，以后的几项跟着进来 | 放进权限页「平台账号」组：端口不是权限 |
| 8 | 会话不在了：`session_not_found`、`session_stopped` 都忘掉、再问一次 `venue.session`、再发一次，只重来一次；推来 `resync` 的再订阅一次（不写 `after`） | 会话被删、停了都是这个场所以后还要说话；只重来一次不会打转 | 只认 `session_not_found`：会话停了以后这个私聊就一直发不进去 |
| 9 | 发进去的字照原样；去掉首尾空白是空的才不送 | `session.send` 照原样成一块文字 | 先去掉首尾空白再发 |
| 10 | 回话照她说的先后放进写队列，等 NapCat 回应的那一步交给别的任务 | 一句话的回应要等，几句话的先后不能乱 | 每句回话一个任务从头发：先后靠调度 |
| 11 | 查询参数里的令牌照原样比，不做百分号解码 | NapCat 用头出示；朴素 | 照 URL 解码 |
| 12 | 自己编的命令编号（`venue.session`、`subscribe`）带一段随机前缀 | 同一个编号核心交回上一次的结果，桥重启以后从 1 数起会撞上 | 从 1 数起 |
| 13 | 握手报 `head.kind = "onebot"`、`caps.input = false` | 场所会话没人能确认（`venues.md`） | 照终端报能输入 |
| 14 | 跟核心的那一头崩了、发回话的任务崩了（是 bug）：说一句带原话、退出码 1 | 桥这时已经不能干活（发回话的任务只等回应、记日志，它崩了说明那一头的代码有错），照实报，不装作还在跑 | 记一行接着跑；当成核心断了退（原因说错了，排查走错路） |
| 15 | 桥自己的数（路径、调用时限、两个队列、接不了连接歇多久）放在 `resources/software/onebot/bridge.json`，照 `web.json` 读；登记簿门禁只豁免这一份文件 | 数是数据，住在代码之外；不进配置清单：没有人要改它们，进了清单就要给人看的名字、说明和设置页的位置。只豁免一份文件：这个包以后要放给模型看的字（群聊），整个包豁免会漏查 | 写成代码里的常量；进 `onebot.*` 配置；照 mermaid、net 豁免整个 `software/onebot/` |
| 16 | 平台的名字 `qq` 只写一处（`onebot.rs` 的 `PLATFORM`），场所、平台上的人、命令编号都经三个小函数拼（前两个经群聊内核拼，第 20 条） | 散在几处，改一处漏一处，拼出来的场所和找会话用的对不上 | 每处自己 `format!` |
| 17 | 给人看的字放 `resources/software/onebot/human/{zh,en,ja}.json`，照核心的格式和 `Human::load` 读；`ja.json` 照英文写；找不到资源目录、字读不懂时印原话 | 文字是数据；照核心的格式，门禁（`human_languages.rs`）一起查三种语言的键、字段对得上；日文没有专门写的，和核心拒绝时的话一样照英文。资源目录、字本身出问题时没有字可用，只能印原话 | 留在 `texts.rs` 里；不写 `ja.json` 让 `Human::load` 退到英文（门禁要求三份都在） |
| 18 | 握手的回应没带 `language`：照连不上核心退，退出码 1 | `protocol.md`「握手」说回应一定带；没带是协议不对，照实说 | 当成英文接着跑 |
| 19 | 算出来的 `Sec-WebSocket-Accept` 放不进头：回 400、不升级 | 回 101 却不带它，NapCat 握手失败，这一头却当升级成了 | 照样回 101 |
| 20 | 场所、平台上的人经群聊内核拼（`Venue::new`、`miyu_chat::person`），拼不出来的记一行 `WARN`、这条不送；命令编号末尾加事件的 `time`，没带、读不出的照 `0` 拼（照 `chat.md` 第七条第 1 条改，2026-10-07） | 编号的规矩只有一份，桥拼的和群聊内核解的对得上。加时刻：NapCat 重置本地库以后消息编号会重号，只拼编号的话新消息被当成重发吞掉（O 线自查第 17 条）。没带 `time` 照 `0`：OneBot v11 每个事件都带 `time`，没带是实现不合规；照 `0` 拼消息照样送，去重退回只看消息编号，和加时刻以前一样，不比不收差 | 没带的不收：主人的话被吞了还看不出来；照桥收到时的本机时刻拼：同一条重发过来时刻不同，去重失效 |

### 二、WebUI（施工 O-16 起）

`miyu-onebot` 自己的配置页（`docs/designs/18-通讯平台.md` 第三节、Q18）：桥进程开一个只听本机的 HTTP 端口，给页面、把 `/ws` 原样转给核心。页面自己说核心协议：用户名、密码登录（核心验，`web-module.md` 第一条 W-8），改配置、存密钥调核心现成的 `config.set`、`secret.set`，校验只在核心那一处。只有桥自己知道的由桥另给：只读的 `/status`（NapCat 连没连上、是哪个实现、令牌设没设）、`/token`（令牌的值），和让桥当场照新配置的 `/apply`（O-16 补二）。终端界面、网页软件的设置页里只有一行「QQ：打开 miyu-onebot」，不画 QQ 的配置。

状态：图纸，2026-10-07 起草，项目主人过目（登录用 Miyu 网页的账号、先做两页，照推荐定）。O-16 做好了骨架和「连接」页（施工时补的见「施工时定的」第 7 到 13 条；没设令牌也起来、在「连接」页生成第一个，见第 14 到 16 条）。2026-10-08 项目主人在自己的 NapCat 上试过以后改了四处（补二，第 17 到 24 条）：令牌随时能看能复制、没令牌时页顶写清三步、改了令牌和端口当场生效不用重启、地址写短的 `/ws`。O-17 做好了「主人与自己人」页（施工时补的见「施工时定的」第 25 到 36 条）；场所和规则、平台工具、插件、日志几页等群接通以后再做。

**共用的底子**：给页面文件、核对 Host 和 Origin、`/ws` 原样转给核心、安全响应头、带一次性码打开浏览器，这几样网页软件（`web-ui.md` 第一条、第二条）已经写好了，从 `miyu-web` 抽进一个第 3 层的新 crate，`miyu-web` 和 `miyu-onebot` 都用它，`miyu-web` 的行为一个字节不变（18 第三节「抽成两边共用的库，不抄一份」）。这个 crate 叫 `miyu-webserve`，在第 3 层，交出什么、从哪搬来见 `webserve.md`（「施工时定的」第 1 条）。

**对外的样子**

| 什么 | 说明 |
|---|---|
| 配置 `onebot.web` | WebUI 的端口，整数 1024 到 65535，出厂 8302（网页软件 8300、NapCat 8301，挨着好记），只能写在系统配置；在「连接」页保存当场换（`/apply`），命令行改的桥下次起来时生效（O-16 补二）。照 `onebot.listen` 的先例先由核心声明，9-1 挪进清单 |
| 配置 `onebot.trusted`（O-17） | 自己人：平台身份的列表（`["qq:20017"]`），元素是文字、最多 128 个字；没有默认值，不写的当没有自己人；只能写在系统配置；生效时机 `now`。在「主人与自己人」页上整张写回。现在桥还不读（桥接群、算「发的人是谁」时读，`chat.md`「发的人是谁」），照 `onebot.listen` 的先例先由核心声明 |
| `GET /` 和页面文件 | 页面在 `resources/software/onebot/web/`，规矩照网页软件（`..`、跑到目录外的、不是普通文件的 404；类型照扩展名；响应头带 `nosniff`、`no-referrer`、`no-cache`、内容安全策略） |
| `GET /ws` | 原样转给核心，和网页软件的 `/ws` 一样（一帧一行、Origin 要对、1 MiB 上限、核心断了关 1012） |
| `GET /status` | 只读，`Authorization: Bearer <登录令牌>`；桥拿这个令牌去和核心握手，核心认了才回，不认 401；连不上核心 502；`GET` 以外 405。每次重读一次配置（`Config::load`），桥手里的令牌照读到的换上（第一条第 2 条）。回 `{"napcat": {"connected": 布尔, "implementation": 字, "version": 字, "self_id": 字}, "listen": 端口, "web": 端口, "token": "set" \| "none" \| "missing", "platform": "qq"}`，`Cache-Control: no-store`；没连着的 `napcat` 只有 `connected: false`；连着、还没问到是哪个实现的，没有 `implementation`、`version`。连着几个号的，说号最小的那一个。`listen`、`web` 是实际听的端口（`/apply` 换过的照换过的）。`token`：取到了值、没写引用、写了引用取不到（引用的密钥没存、环境变量没设）；配置读不出来（走不到）的照桥手里的说（O-16 补二：去掉 `restart_needed`，`listen` 不再有 `null`，「施工时定的」第 6、14 条）。`platform` 是桥的平台名（第一条的 `PLATFORM`），「主人与自己人」页照它拼 `qq:<号>`（O-17，「施工时定的」第 26 条） |
| `GET /token` | 要登录，和 `/status` 一样（不带、带错 401，连不上核心 502，`GET` 以外 405）。重读一次配置，回桥手里的令牌 `{"token": "<值>"}`，没有的 `{"token": null}`；`Cache-Control: no-store`；运行日志只记一行 `INFO web token read`，不记值（O-16 补二，「施工时定的」第 17 条） |
| `POST /apply` | 要登录，同上；`POST` 以外 405。桥重读配置，令牌照新的；两个端口里和上一次照的（配置里写的那个）不一样的，先开新的，都开上了才关旧的、换上新的，回 `{"listen": 端口, "web": 端口}`（实际听的）。新的开不了的回 409 `{"in_use": 端口}`，两个端口都不换、旧的照旧开着，令牌照样换上；配置读不出来（走不到）回 500。已经连着的（NapCat 的那一条、页面正在用的连接）不断（O-16 补二，「施工时定的」第 19 条） |
| `GET /human` | 登录以前页面要的字（O-16 施工时补，「施工时定的」第 7 条）：不要登录，`{"language": <桥的语言>, "said": {<编号>: <模板>}}`，只有 `software/onebot/web/` 开头的；`GET` 以外 405 |
| `miyu-onebot web [--print]` | 桥要在跑（「施工时定的」第 5 条），令牌设没设都一样：开浏览器到 WebUI；还没设过登录密码的，照 `miyu web` 带上一次性码（`web-ui.md` 第二条）；`--print`、交不给浏览器的印网址 |
| 给人看的字 | 页面里的字照 `human.get` 取（核心和软件包的 `human/*.json` 合成的一份，`web-module.md`），桥的字在 `software/onebot/human/` 里，编号前缀 `software/onebot/`；登录以前照 `/human` |

`bridge.json` 的 `web`（O-16）：

| 格 | 出厂 | 说明 |
|---|---|---|
| `csp` | `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'` | 页面的 `Content-Security-Policy`：只连自己、不许被框起来、不许内联的脚本和样式 |
| `types` | `html`、`js`、`css` 三种，带 `charset=utf-8` | 扩展名到媒体类型；表里没有的给 `application/octet-stream`（不猜） |
| `status_cache_seconds` | 60 | `/status`、`/token`、`/apply` 验过的登录令牌记几秒（「施工时定的」第 3 条） |

**样子**（宽屏；窄屏左栏收成抽屉。颜色、字体、按钮照网页软件，看着是一家的）

```
┌──────────────────────────────────────────────────────────────────┐
│ miyu-onebot   ● NapCat 已连上              admin ▾   [回到 Miyu] │
├────────────┬─────────────────────────────────────────────────────┤
│ 连接       │ 连接                                                │
│ 主人与自己人│                                                     │
│ ── 以后 ── │ NapCat   ● 已连上  NapCat 4.8.2 · 机器人 qq:12345   │
│ 场所       │                                                     │
│ 平台工具   │ NapCat 那边要填的                                   │
│ 插件       │   反向 WebSocket 地址  ws://127.0.0.1:8301/ws  [复制] │
│ 日志       │   访问令牌            已设 ········  [显示] [复制] [换一个]     │
│            │   消息格式            数组                          │
│            │                                                     │
│            │ 端口                                                │
│            │   NapCat 连进来  [ 8301 ]   WebUI  [ 8302 ]  [保存] │
└────────────┴─────────────────────────────────────────────────────┘
```

```
没设令牌时，「连接」页的上半截（O-16 补二）

┌ 先做这三步 ─────────────────────────────────────────────────┐
│ ① 生成令牌：按下面「访问令牌」那一行的「生成」。              │
│ ② 在 NapCat 的网络配置里加一个「WebSocket 客户端」，填上下面的 │
│    地址和令牌，消息格式选数组。                               │
│ ③ 等几秒，下面 NapCat 那一行变成「已连上」。                  │
└─────────────────────────────────────────────────────────────┘
NapCat   ● 没连上

NapCat 那边要填的
  反向 WebSocket 地址  ws://127.0.0.1:8301/ws  [复制]
  访问令牌            没设                    [生成]（主按钮）
```

```
按了「显示」、刚生成完（O-16 补二）

  访问令牌            3f9c…（64 位十六进制）  [收起] [复制] [换一个]
```

```
主人与自己人（O-17）

┌ 主人 ──────────────────────────────────────────────────┐
│ 这些号的私聊就是本机账号本人；在群里多管理命令的权限。     │
│  QQ 号            本机账号                               │
│  [ 10001     ]    [ admin ▾ ]   [删]                     │
│  [ 1000x     ]    [ admin ▾ ]   [删]  号只能是数字，不以 0 开头 │
│  [ 10001     ]    [ admin ▾ ]   [删]  这个号重复了        │
│  [加一个]                                       [保存]   │
│  没成：<核心的原话>                                      │
│  ▸ 主人的号就是你本人：先知道这三条代价（点开是 18 第三节的三条） │
└─────────────────────────────────────────────────────────┘
┌ 自己人 ────────────────────────────────────────────────┐
│ 私聊里能叫她，不限流，睡着时私聊也放行。                  │
│ QQ 桥接通群以后才照这张表认人，现在先记下。               │
│  [ 20017     ]   [删]                                    │
│  [ 10001     ]   [删]  已经是主人                        │
│  [加一个]                                       [保存]   │
└─────────────────────────────────────────────────────────┘
```

```
表空着时（O-17）

主人      还没有主人。按「加一个」，填上你自己的 QQ 号，再按「保存」：以后你私聊她，她就认得是你。
          [加一个]                                  [保存]（灰着）
自己人    还没有自己人。
          [加一个]                                  [保存]（灰着）
```

**怎么走**

1. **起来**：`miyu-onebot serve` 起来时连 NapCat 的监听之外，再开 `127.0.0.1:<onebot.web>`；端口被占了照 `onebot.listen` 被占一样说清、退出码 1。令牌没设的两个也照开（第一条第 1 条，O-16 补、补二）：第一次用，人在这里生成令牌。Host、Origin 照网页软件核对。
2. **登录**：页面连 `/ws`，握手带用户名和密码（或者记住的登录令牌，30 天，照网页软件），核心验。没有「接平台」能力的账号（`06-多用户与身份.md` 第四节），页面说进不来、断开。还没设过密码的，页面说「在终端里运行 `miyu-onebot web`」。
3. **连接页**：
   - 没令牌时（`/status` 的 `token` 不是 `set`）页顶一段三步：① 生成令牌；② 把地址和令牌填进 NapCat 的「WebSocket 客户端」，消息格式选数组；③ 等几秒，NapCat 那一行变成「已连上」。出来了就留着（令牌设好了第一步打勾），NapCat 连上了才收起，这一页里不再出来（O-16 补二，「施工时定的」第 22 条）。
   - NapCat 的状态照 `/status`，每 5 秒取一次。
   - 地址照 `ws://127.0.0.1:<onebot.listen>/ws` 拼（短的那个，O-16 补二），「复制」把它放进剪贴板。
   - 令牌照 `/status` 的 `token`（O-16 补二，「施工时定的」第 16、17 条）：`set` 的那一行「已设 ········」（照环境变量的写「照环境变量 <名字>」），三个按钮：「显示」取 `/token` 把值显示在那一行（按钮变「收起」，再按收起）；「复制」取 `/token` 放进剪贴板；「换一个」先在页面里的对话框问一句（NapCat 那边也要跟着换；「取消」「换一个」，先停在「取消」上）再生成。`none`、`missing` 的那一行说没设、引用的取不到，只有一个主按钮「生成」，不先问。生成：页面生成 32 个随机字节（十六进制），`secret.set` 存成 `onebot`，`config.set` 把 `onebot.token` 写成 `{ secret = "onebot" }`，再取 `/token` 显示出来：取的时候桥重读配置，令牌当场照新的（第一条第 2 条）。
   - 端口：`config.set` 写 `onebot.listen`、`onebot.web`；写错的核心回问题，页面照原话说。写好了调 `/apply`：NapCat 的端口换了，说一句去 NapCat 里改地址；WebUI 的端口换了，先说一句（登录记在浏览器里、按地址分，到新地址要重新登录一次），再跳到新地址；被占了说哪个端口被占、桥照旧用原来的（O-16 补二，「施工时定的」第 21 条）。
4. **主人与自己人页**（O-17）：
   - 进这一页时 `config.get` 读一遍（不写 `keys`：全部，键里有人起的名字的照真的键列；「连接」页也照它）。平台的名字照 `/status` 的 `platform`；还没取到 `/status` 的先说「正在连」，取到了再画（「施工时定的」第 26、33 条）。
   - **主人**：`external.bindings` 下键是 `<平台>:<号>` 的每一格，一行一个号对一个账号；别的平台的不画、不碰。号只收数字，平台前缀由页面照桥的平台名拼（`qq:<号>`）。账号的下拉：协议里没有列账号的方法，下拉里是握手回的账号（核心里固定是 `admin`），加上表里已经写着的别的账号（手写的照原样留着，「施工时定的」第 27 条）；新加的一行照握手回的账号。保存时照改动发一条 `config.set`（`layer: "system"`，几项一起）：加的、改了账号的写 `external.bindings."qq:<号>" = "<账号>"`，删的恢复默认（`unset: true`，去掉这一格）。只写系统配置。
   - **自己人**：`onebot.trusted`，平台身份的列表，一行一个号；保存时整张写回（`value` 是整张列表；别的平台的身份照原样留在前面，「施工时定的」第 29 条）。同一个号也在主人表里的（照主人表里现在的行，没存的也算），那一行旁边说一句「已经是主人」，照样能存：主人的权限包含自己人的，不拦，免得改表时卡住。
   - 号：去掉前后的空白；空着的行不算（保存时当没有这一行，也不标）；不是 1 到 20 位数字、以 0 开头的标出来；同一张表里号重复的都标出来；有标着的不让存（「施工时定的」第 28 条）。表里手写的 `qq:` 后面不是数字的照样画出来、标着，删掉或者改对了才能存。
   - 两张表各有一个「保存」，没改动、有标着的行时灰着。存好了在那一张表下面说「存好了」，照 `config.get` 重读，只重画这一张表，另一张没存的改动留着（第 31 条）。核心回问题（`config_invalid`、还没设好密码的 `setup_first` 这些）照原话说在那一张表下面（`web/failed`），表里的东西不丢。
   - 表空着时：主人表写一句先做什么（「加一个」、填上自己的号、「保存」），自己人表写「还没有自己人」。主人表下面折起来一段「号被盗的代价」（`<details>`，18 第三节的三条），给人看的字。
   - 生效：`external.bindings` 核心当场照新的认（`config.md` 那一行的 `now`）；`onebot.trusted` 现在桥还不读（桥接群、算「发的人是谁」时才读），生效时机写 `now`，到时桥照 O-16 的办法当场重读，自己人那张表下面先写一句「QQ 桥接通群以后才照这张表认人」（第 32 条），桥接群那一步去掉。页面都不提示重启。
   - 左栏的「主人与自己人」点得进，「连接」点得回；在哪一页只记在页面里，重新载入回到「连接」（第 34 条）。窄屏点了左栏的一页，抽屉收起。
5. **运行日志**：照桥的 `state/logs/onebot.log`，只记 WebUI 起来、登录成功或失败的次数、取过令牌（`/token`，O-16 补二）、`/apply` 换了哪个端口，不记密码、令牌、一次性码。

**在哪**、**改了谁**：见第一条「在哪」标 O-16 的几行；共用的底子 `crates/miyu-webserve/`（`webserve.md`，第 3 层，从 `miyu-web` 原样搬来，搬家表在那一页）。

**出错**

| 情况 | 怎么办 |
|---|---|
| WebUI 的端口被占（`serve`） | `failure/web-port-in-use`：哪个端口被占了、改 `onebot.web`；退出码 1 |
| `onebot.web` 读不出来（走不到） | `unready/bad-web-port`，退出码 1 |
| Host 不对 | 403，运行日志 `WARN web rejected host=… origin=…` |
| `/ws` 的 Origin 不对、连不上核心 | 照网页软件：403；`web.error` 通知再关（`web-ui.md` 第一条第 7 款） |
| `/status`、`/token`、`/apply` 没带、带错登录令牌 | 401，运行日志 `WARN web login refused`（带对的、要和核心握手的记 `INFO web login accepted`）；连不上核心 502，`WARN core unreachable` |
| `/apply`：新端口开不了 | 409 `{"in_use": 端口}`，两个端口都不换、旧的照旧开着；`WARN apply port in use`（O-16 补二） |
| 重读配置读不出来（走不到） | 桥手里的令牌不动，`WARN config not readable`；`/apply` 回 500（O-16 补二） |
| `/human` 字读不懂 | 500，`WARN human not read` |
| `miyu-onebot web`：WebUI 的端口上连不上 | `open/not-running`，退出码 1 |
| `miyu-onebot web`：连不上核心、要不到一次性码 | `failure/core`，退出码 1 |
| 令牌没设（`serve`、`web`） | 不算错（O-16 补、补二）：`serve` 照常开两个端口，说 `notice/no-token`，运行日志 `WARN no token yet`，NapCat 连进来 401；`web` 照常打开，人在「连接」页生成令牌，NapCat 下一次连就通 |

**给人看的字**：标准错误上的（`serve`、`web` 说的）放在 `said` 里，和第一条一样照 `Texts` 换；页面里的放在同一份文件里、编号 `web/` 开头，页面自己换字段（`{字段}`、`{{`、`}}`，照 `Human::say`）。`ja.json` 照英文写（第一条「施工时定的」第 17 条）。

| 编号 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `notice/web` | 起来了 | QQ 桥的网页在 http://127.0.0.1:{port}，用 miyu-onebot web 打开。 | The QQ bridge web page is at http://127.0.0.1:{port}; open it with miyu-onebot web. |
| `failure/web-port-in-use` | WebUI 的端口被占 | 端口 {port} 被占了。换一个：miyu config set --system onebot.web <端口>。 | Port {port} is in use. Pick another: miyu config set --system onebot.web <port>. |
| `unready/bad-web-port` | 读不出来 | onebot.web 读不出来，照 miyu config check --system 查一下。 | onebot.web could not be read; check it with miyu config check --system. |
| `open/not-running` | 桥不在跑 | 127.0.0.1:{port} 上没有 QQ 桥的网页。先运行 miyu-onebot serve；改过 onebot.web 的，重启它。 | No QQ bridge web page on 127.0.0.1:{port}. Start it with miyu-onebot serve; if you changed onebot.web, restart it. |
| `open/first` | 还没设过密码 | 还没设过网页的登录密码：带着一次性码打开网页，在网页上设用户名和密码。 | No web password yet: opening the web page with a one-time code to set a username and password. |
| `open/opened` | 交给了浏览器 | QQ 桥的网页开在 {url}，已经交给浏览器打开。 | The QQ bridge web page is at {url} and has been opened in your browser. |
| `open/print-hint` | 带了码 | 浏览器没打开的话，用 miyu-onebot web --print。 | If the browser did not open, use miyu-onebot web --print. |
| `open/open-this` | `--print` | 在浏览器里打开： | Open this in a browser: |
| `open/code-warning` | `--print` 带了码 | 这个链接 5 分钟内有效，只能用一次，别发给别人。 | This link works once within 5 minutes. Do not share it. |

页面的字（`web/…`，中文；英文见 `resources/software/onebot/human/en.json`）：登录（`web/login/*`：登录、用户名、密码、「还没设过密码的，在终端里运行 miyu-onebot web。」）、设密码（`web/setup/*`）、`web/loading`、`web/core-lost`、`web/retry`、左栏（`web/nav/*`、`web/later`、`web/not-yet`）、`web/menu`、`web/sign-out`、没令牌时的三步（`web/guide/*`，O-16 补二）、NapCat 的状态（`web/napcat/*`：已连上、没连上、看不到桥的状态，`{implementation} {version} · 机器人 {bot}`）、要填的（`web/fill/*`，地址的写法 `ws://127.0.0.1:{port}/ws` 也在这里）、令牌（`web/token/*`：已设、没设、引用的取不到、照环境变量、显示、收起、换一个、生成、换之前问一句）、对话框的「取消」（`web/cancel`）、端口（`web/ports/*`：两格的名字，NapCat 的端口换了、WebUI 的端口换了要重新登录、被占了）、`web/copy`、`web/copied`、`web/save`、`web/saved`、`web/failed`、`web/brand`；「主人与自己人」页（`web/people/*`，O-17，中文照下表，存好了、没成照上面的 `web/saved`、`web/failed`）。补二去掉了 `web/napcat/closed`（没开端口）、`web/token/new`、`web/token/new-hint`（只显示这一次）、`web/restart`（重启以后生效）。

| 编号（`web/people/` 后面） | 中文 | 英文 |
|---|---|---|
| `owners/title` | 主人 | Owners |
| `owners/hint` | 这些号的私聊就是本机账号本人；在群里多管理命令的权限。 | Private chats from these accounts are the local account itself; in groups they also get the admin commands. |
| `owners/empty` | 还没有主人。按「加一个」，填上你自己的 QQ 号，再按「保存」：以后你私聊她，她就认得是你。 | No owners yet. Press Add, enter your own QQ number and press Save: from then on she knows it is you in private chat. |
| `number` | QQ 号 | QQ number |
| `account` | 本机账号 | Local account |
| `add` | 加一个 | Add |
| `remove` | 删 | Remove |
| `bad-number` | 号只能是数字，不以 0 开头 | Digits only, not starting with 0 |
| `duplicate` | 这个号重复了 | This number is listed twice |
| `already-owner` | 已经是主人 | Already an owner |
| `trusted/title` | 自己人 | Friends |
| `trusted/hint` | 私聊里能叫她，不限流，睡着时私聊也放行。 | They can call her in private chat, with no rate limit, even while she sleeps. |
| `trusted/later` | QQ 桥接通群以后才照这张表认人，现在先记下。 | The QQ bridge uses this list once groups are connected; it is kept until then. |
| `trusted/empty` | 还没有自己人。 | No friends yet. |
| `risk/title` | 主人的号就是你本人：先知道这三条代价 | An owner account is you: know these three costs |
| `risk/stolen` | QQ 号被盗，就等于这台机器在你的权限下被别人用。 | If the QQ account is stolen, someone else uses this machine with your permissions. |
| `risk/forged` | 「这是主人发的」只能听 NapCat 报上来的：NapCat 被人拿下、或者本身不怀好意，就能冒充你的私聊。 | "This is from the owner" rests on what NapCat reports: a compromised or malicious NapCat can fake your private chats. |
| `risk/injection` | 网页、文件里藏着的提示词注入，能让她在沙盒允许的范围里乱改工作区的文件；越过沙盒的操作在私聊里一律做不了。 | Prompt injection hidden in web pages or files can make her change workspace files within what the sandbox allows; nothing beyond the sandbox can be done from a private chat. |

**样子**的实际：宽屏顶栏「miyu-onebot ● NapCat 已连上 … admin ▾」，`admin ▾` 里是「退出登录」；左栏「连接」「主人与自己人」两页点得进（O-17），「以后」下面四页是灰的、点不了；「连接」页三张卡片（NapCat、NapCat 那边要填的、端口）。令牌没设的（O-16 补、补二）：页顶多一张「先做这三步」的卡片，令牌那一行「没设」、一个主按钮「生成」；生成以后令牌那一行变「已设」，值直接显示在那一行（按钮是「收起」「复制」「换一个」），三步的第一步打勾，NapCat 连上以后三步收起。窄屏（720px 以下）左栏收成抽屉，顶栏左边一个按钮开它。亮暗两色跟着系统（`prefers-color-scheme`），颜色照网页软件的「晨光」和「tokyonight」。图里的「回到 Miyu」O-16 没画（「施工时定的」第 12 条）。

**守着它的**（`crates/miyu-onebot/tests/`，O-16；核心是 `miyu-ipc` 的监听当替身，照网页软件）

- Host 只认三种写法，NapCat 的端口、别的端口都 403，`/status`、`/human`、`/ws` 也一样；页面不出页面目录（`..`、`%2e%2e`、链接、目录、`%zz`）；响应头一个不少、内容安全策略照 `bridge.json`、从不设 cookie；`HEAD`、别的方法 405；类型照表、表里没有的不猜。`/ws` 的 Origin 不对 403（没有、别的网站、`https`、`null`、NapCat 的端口）；两头一帧一行照转、一个字节不改（凭据、中文、空白、很长的一行）；核心断了 1012；连不上核心发 `web.error` 再关。WebUI 的端口被占说是哪个端口。`/human` 照桥的语言、只给 `web/` 开头的。（`web.rs`）
- `/status`：不带、带错（差一个字、多一个字、空的、别的方案）401；带对的回状态（两个实际的端口、`token`）；记着的不再握手，错的每次都握、都 401，过了时候再握；NapCat 连上以后说是哪个实现、哪个号，断了说没连着。（`status.rs`）
- 令牌没设（O-16 补、补二）：两个端口都开，`/status` 的 `listen` 是实际的端口、`token` 照配置文件说 `none`、`missing`、`set`；真的程序 `serve`、`web --print` 照常走（第一条「守着它的」，`no_token.rs`）。
- `/token`（O-16 补二）：不带、带错登录令牌 401，`GET` 以外 405；带对的拿到值，`no-store`；没有的是 `null`。（`token.rs`）
- `/apply`（O-16 补二）：不带、带错 401，`POST` 以外 405；换 NapCat 的端口：回新端口，旧的连不上、新的连得上，`/status` 跟着说；换 WebUI 的端口：新地址上页面、`/status` 照常，旧的连不上；新端口被占回 409 和是哪个，旧的照旧、两个都不换；换令牌：旧的 401、新的进得来、已经连着的那一条还在。（`apply.rs`）
- 验过的登录令牌：记一阵、到点就忘、别的令牌不算；只记 SHA-256，没有原文。（`src/web/tests.rs`）
- `miyu-onebot web`：桥不在跑说先 `serve`、退出码 1、不开浏览器；没设过密码的带 `#setup=`，设过的不带；`--print`、交不给浏览器的印网址和提醒。（`open.rs`）
- 出厂的 `bridge.json` 的 `web`：三种类型、策略只连自己不许被框、60 秒；多一格少一格读不进来。（`tuning.rs`）读配置：`onebot.web` 不写是 8302。（`settings.rs`）新加的每一句三种语言都换得出来。（`texts.rs`）
- 「主人与自己人」页（O-17，`people.rs`；真的核心加真的桥，浏览器经 `/ws` 照页面的样子发）：还没设好密码的连接（一次性码）改系统配置回 `setup_first`、什么都没写；设好以后加一个主人（`external.bindings."qq:<号>"`）、删一个主人（恢复默认）、整张写回自己人，换一条连接 `config.get` 读得回，桥收到新主人的私聊、记成管理员本人；自己人写进个人设置、写成一个字、元素是空的字，`config_invalid`，什么都没变。`/status` 带 `platform`（`status.rs`）。`onebot.trusted` 读得出、不写是没有、只能写系统配置、写错的报问题（`crates/miyu-core/tests/settings.rs`）。
- 页面没有自动测试：`node --check` 查 `app.js`、`people.js` 的写法；无头浏览器截图（补二：没令牌时的三步、显示令牌、换令牌的确认，亮暗各一套；O-17：两张表空着、填了几行（号不对、重复、已经是主人）、核心回问题、窄屏，亮暗各一套）；项目主人在浏览器里试一次（O-16 施工单验收第 6 条、O-17 第 5 条）。

**施工时定的**（O-16，2026-10-07；补二，2026-10-08；O-17 第 25 到 36 条，2026-10-08）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | 共用的底子叫 `miyu-webserve`，第 3 层：给页面文件（路径、类型、安全响应头）、核对 Host 和 Origin、`/ws` 原样转给核心、带一次性码开浏览器；函数原样从 `miyu-web` 搬过去，不改名、不改行为 | 18 第三节「抽成两边共用的库，不抄一份」；名字说清它是给网页的那一层 | 叫 `miyu-webkit`（和浏览器引擎撞名）；抄一份进桥 |
| 2 | 前端照网页软件：原生 JS，`index.html`、`app.js`、`style.css` 三个文件，页面里的字照 `human.get` | 和网页软件一家，不加构建工具 | 引一套前端框架 |
| 3 | `/status` 验过的登录令牌记 60 秒（只记令牌的哈希） | 页面每 5 秒取一次，不用每次都和核心握手 | 每次都握手；不验 |
| 4 | 登录成功就算能进：核心现在只有管理员一个账号，「接平台」这项能力随多用户那一段 | 不为以后写代码；到时照能力判 | 现在就做能力检查 |
| 5 | `miyu-onebot web` 要桥已经在跑；不在跑的说先 `miyu-onebot serve` | 9-4 以前桥由人手动起，不在后台自己拉起 | 照 `miyu web` 在后台拉起 serve |
| 6 | `/status` 每次重读一次配置：`token` 照读到的说，桥手里的令牌跟着换上（补二改写，2026-10-08 项目主人定「改了当场生效」：去掉 `restart_needed`） | 页面要照实说令牌设没设；读到了就用，令牌改了不等 NapCat 来碰 | 和起来时读到的比、说要不要重启（O-16 原来的 `restart_needed`）；订阅配置的推送 |
| 7 | 登录以前页面的字由桥的 `/human` 给：照桥的语言（握手回的）读 `software/onebot/human/<语言>.json`，只交 `web/` 开头的，样子和 `human.get` 的 `said` 一样；登录以后照 `human.get` 换一遍（施工时补，2026-10-07） | 登录以前页面还没和核心握手，调不了 `human.get`，登录表单的字没处来；字还是住在 `human/*.json` 里 | 登录表单的字写进 `index.html`（字不是数据）；登录以后也只用 `/human`（和图纸「照 `human.get`」两样） |
| 8 | `/status` 验登录令牌时握手报 `head.kind = "onebot"`，等回应照 `bridge.json` 的 `call_timeout_seconds`，握完就关；连不上核心回 502 | 不另开一个数；502 照网页软件 `/media` 连不上核心 | 每次留着连接复用（照 `/media`：页面 5 秒一次，又有 60 秒的记着，省不了几条） |
| 9 | 连着几个机器人号的，`/status` 说号最小的那一个；还没问到是哪个实现的，不写 `implementation`、`version` | 图纸的 `napcat` 是一个；不写比写空的字清楚 | 列出全部（图纸的样子要改） |
| 10 | `miyu-onebot web` 认 `--print`；判桥在不在跑，照 `onebot.web` 连一下 `127.0.0.1` 的端口 | snap 装的浏览器打不开时要能印网址（照 `miyu web`）；桥不写 `run/` 里的地址，连一下最朴素 | 不认 `--print`；桥像网页软件那样写 `run/onebot` |
| 11 | 页面：登录令牌记在浏览器的 `localStorage`（`miyu-onebot.login`，带核心给的过期时刻）；握手报 `head.kind = "onebot-web"`；`/status` 每 5 秒、断了每 5 秒重连，写在 `app.js` 顶上；NapCat 的地址照 `onebot.listen` 和 `web/fill/url` 那一句拼；端口照 `input` 交给核心读；退出登录调 `account.logout`（只作废这一个） | 图纸「记住的登录令牌，30 天，照网页软件」「每 5 秒」；页面的节奏是页面自己的，没有数据文件可放；地址的写法和配置说明里的那句一样是给人看的字 | 每 5 秒写进 `bridge.json` 再经 `/status` 交给页面（`/status` 的样子要改） |
| 12 | 图里的「回到 Miyu」O-16 不画 | 页面不知道网页软件在哪：它的端口在 `web.json`、地址在 `run/web`，都不归桥管 | 写死 8300（网页软件 `--port` 换了就错） |
| 13 | 新加 `Unready::BadWebPort`、`Failure::WebPortInUse`、`Notice::Web`，各一句话 | 端口被占要说改哪一项（`onebot.web`，不是 `onebot.listen`） | 共用 `listen` 那一句 |
| 14 | 令牌没设也开 NapCat 的端口，连进来一律 401；`/status` 的 `listen` 总是实际听的端口（补，2026-10-07；补二改写，2026-10-08 项目主人定） | 设了令牌不用重启就要能连，端口得先开着；令牌对不上本来就回 401 | 令牌没设不开 NapCat 的端口、`listen` 写 `null`（补的做法：设了要重启才开） |
| 15 | 令牌没设那一句从读配置的 `Unready::NoToken` 挪成起来以后说的 `Notice::NoToken`，编号 `unready/no-token` 改成 `notice/no-token`（补，2026-10-07）；补二：跟在 `Notice::Listening` 后面说（语言照 `Listening` 带的换，它自己不再带）；`Settings::token` 是 `Token`（取到了、没写引用、取不到三种） | 没设已经不是起不来；这一句在握手以后说，照核心回的语言；`/status` 要分清没写引用和取不到 | 留在 `Unready`、读配置时就说（照系统的语言说，又和后面几句不是一种语言）；`Option<Secret>`（分不出两种没有） |
| 16 | 页面：令牌设没设照 `/status` 的 `token`：`set` 的（照环境变量的也是）「显示」「复制」「换一个」，换之前问；`none`、`missing` 只有一个主按钮「生成」，不问（补，2026-10-07；补二改写，2026-10-08） | 没有旧的要换掉，问「换吗」不对；环境变量设没设桥看得到，页面不用猜 | 一直叫「换一个」；页面照 `secret.list` 自己算（看不到环境变量，只能当它有） |
| 17 | 令牌随时能看、能复制：`/token` 由桥交给登录了的管理员；页面不再写「只显示这一次」（补二，2026-10-08 项目主人定，推翻 2026-10-07「只在刚生成时显示一次」） | 本机、要登录、只给管理员的页面，方便优先；令牌是桥自己的凭据，桥手里本来就有，由桥交给登录了的管理员，不经核心协议交出去，和 `config.md` 第九条（密钥不经协议交出去）不冲突 | 只在刚生成时显示一次（试的时候抄错了、关了页面，只能再换一个） |
| 18 | 令牌当场生效：NapCat 出示的对不上（或者桥手里还没有）时重读一次配置再比，节流 `reload_seconds`（出厂 1 秒）；WebUI 每次读配置（`/status`、`/token`、`/apply`）也照读到的换上；已经连着的那一条不断（补二，2026-10-08 项目主人定「不用重启」，做法照推荐） | 令牌经核心写进密钥文件和系统配置，桥没有推送可听；NapCat 几秒重连一次，对不上时读一下最省事；节流：乱连的不能把读配置变成负担；连着的那一条握手时出示的是当时对的，断了它 NapCat 那边还没改，白断 | 订阅核心的配置推送（多一条订阅，值还是要自己读）；每次连进来都读；定时读；换了令牌断开连着的 |
| 19 | `/apply`：两个端口照配置里写的比上一次照的（不是实际听的），变了的先开新的，都开上了才关旧的；有一个开不了回 409，两个都不换，令牌照样换上；同时来的几个 `/apply` 一个一个办（补二，2026-10-08） | 开不了新端口时旧的照旧，桥不会落到哪个端口都不听；写 0 的（测试）每次都是 0，不当成变了；令牌和端口互不牵连 | 先关旧的再开新的；开上一个算一个；照实际听的比（写 0 的每次都当变了） |
| 20 | 页面上的地址写短的 `ws://127.0.0.1:<端口>/ws`（补二，2026-10-08 项目主人定） | 两个路径桥都认，短的好填 | 照 OneBot 的惯例写 `/onebot/v11/ws` |
| 21 | 端口保存以后调 `/apply`：WebUI 的端口换了，页面先说一句（`alert`）再跳到新地址，在新地址上重新登录；被占了（409）页面说清，不把配置改回去（补二，2026-10-08） | 登录令牌记在浏览器里、按地址分，新地址上没有；不在地址里带登录令牌（会进浏览历史）。被占时配置里已经是新端口，说清被占、桥照旧用原来的，人换一个再存就够；改回去要多一次 `config.set` | 地址里带着登录令牌跳过去；409 时页面把配置改回原来的 |
| 22 | 没令牌时页顶的三步：`/status` 的 `token` 不是 `set` 时出来，出来了就留到 NapCat 连上（令牌设好了第一步打勾）；令牌早就设好、只是没连上的不出来（补二，2026-10-08） | 生成以后还有两步要做，这时收起，人不知道下一步；设好过的人不用再看一遍 | 只看 `token`（生成完就收起）；只看连没连上（设好过、NapCat 暂时断了也出来） |
| 23 | 换令牌之前问的那一句用页面里的对话框（`<dialog>`），不用浏览器的 `confirm`（补二，2026-10-08） | 跟着页面的亮暗色、字体，和页面是一家；截图看得到（浏览器自己的对话框不进页面的截图） | 浏览器的 `confirm`（O-16 原来的做法） |
| 24 | 配置清单里 `onebot.token` 的生效时机改 `now`，说明写「改了以后，NapCat 下一次连进来就照新的」；`onebot.listen`、`onebot.web` 留 `head_start`，说明补「在 QQ 桥的网页上改、保存的，当场生效」；地址的说法写 `/ws`；没设令牌那一句改成「QQ 桥照样起来，NapCat 连进来会被拒」（补二，2026-10-08 主会话定；`OnebotSettings` 是 O-8 权宜放在核心里的，核心同意过由桥这边改） | 令牌改了确实当场生效（第 18 条），说「下次启动时」会让人白重启；命令行改的端口还是要重启或在网页上按保存，`head_start` 照实说，网页上的另补一句 | 令牌留 `head_start`（说得不对）；端口改 `now`（命令行改的不会当场换） |
| 25 | 页面拆成模块：`app.js`（登录、骨架、「连接」页）、`people.js`（「主人与自己人」页，`app.js` 引进来）；`index.html` 照 `type="module"` 载 `app.js`（O-17） | 一页一个文件，不出包揽一切的大文件；原生的模块不用构建工具，内容安全策略 `script-src 'self'` 照样放行，`.js` 的类型表里本来就有 | 都写在 `app.js` 里（八百多行）；两个普通脚本经全局变量共用 |
| 26 | 平台的名字照 `/status` 多的一格 `platform`（第一条的 `PLATFORM`）；页面拼 `<平台>:<号>`，认的也只认这个平台的（O-17） | 平台的名字只写一处（第一条「施工时定的」第 16 条）；页面本来每 5 秒取 `/status` | 页面里再写一份 `qq` |
| 27 | 账号的下拉：握手回的账号，加上表里已经写着的别的账号；新加的一行照握手回的（O-17） | 协议里没有列账号的方法，核心现在只有 `admin`（第 4 条），照实；手写的别的账号不能因为页面不认就被改掉 | 请核心加 `account.list`（多用户那一段的事）；写死 `admin` |
| 28 | 号去掉前后空白，照 `1` 到 `9` 开头、一共 1 到 20 位数字认；空着的行不算、不标；同一张表里重复的标出来；有标着的不让存（O-17） | 桥照整数拼 `qq:<号>`，`0123` 写进去永远对不上；刚按「加一个」的空行就标红扰人 | 收任意字；空行也标；以 0 开头的也收 |
| 29 | 只画、只改这个平台的：主人表里别的平台的键不画、不碰；自己人整张写回时，别的平台的身份照原样留在前面。手写的 `qq:` 后面不是数字的照样画出来、标着（O-17） | 页面只管这个桥的平台，不把别处写的冲掉；坏的那一格让人看见、改对或者删掉 | 整张写回时只写页面上的（别的平台的被冲掉）；坏的不画（存的时候被悄悄删掉） |
| 30 | 「已经是主人」照主人表里现在的行（没存的也算）（O-17） | 人改着两张表时当场看得到 | 只照存好的 |
| 31 | 存好了照 `config.get` 重读，只重画存的那一张表；另一张没存的改动留着（O-17） | 两张表各存各的 | 整页重读（另一张没存的改动丢了） |
| 32 | 自己人那张表下面写一句「QQ 桥接通群以后才照这张表认人，现在先记下」，桥接群那一步去掉（O-17） | 现在桥还不读 `onebot.trusted`（O-8 只接主人的私聊），不说的话人加了自己人、私聊没反应，以为坏了 | 不说 |
| 33 | 读配置的 `load` 不写 `keys`（全部），两页共用一份（O-17，「连接」页原来只要三项） | 主人表有哪些号事先不知道，只能全要；一份 `items` 两页都用，不各读各的 | 两页各读各的；主人表照 `config.schema` 先问有哪些键（没有这种问法） |
| 34 | 在哪一页只记在页面里，重新载入回到「连接」（O-17） | 朴素；地址栏的 `#` 给一次性码用 | 照地址栏的 `#people` 记 |
| 35 | 「不是管理员的账号改系统配置被拒」照实测：核心现在只有管理员一个账号（第 4 条），没有这种账号；测的是还没设好密码的连接（一次性码）改系统配置回 `setup_first`、什么都没写，页面照原话说在表下面（O-17） | 照核心现在的样子测，不为测造一个核心里没有的账号；多用户那一段有了别的账号再补 | 造一个假的账号（核心里没有） |
| 36 | `onebot.trusted` 的元素是文字，最多 128 个字（和主人表的 `<external>` 一样长）；没有默认值（`none`），不写的页面当空表（O-17） | 元素是平台身份，照 `<external>` 的长度；宏里只有密钥的列表认 `[]` 当默认值，要 `[]` 得改 `miyu-config` | 默认值 `[]`；照号只收数字（规矩在页面，平台无关的规矩在桥那边，施工单「要定的」第 4 条） |
