## 通讯平台的桥 `miyu-onebot`

### 是什么

软件包 `miyu-onebot`：经 OneBot v11 接 QQ 的桥，和终端界面、网页平级的一个头（`docs/designs/18-通讯平台.md` 第三节、Q17）。它把 QQ 上的人接进场所会话，把她的回复发回 QQ；要不要开口、限流、出站这些和平台无关的部分在群聊内核 `miyu-chat`（`chat.md`），这里只管 QQ 这一头和跟核心的那一头。

状态：图纸，随施工 O-8 起草（2026-10-07）。O-8 只有骨架：主人的私聊、只有文字（第一条）；O-8 补照 `chat.md` 第七条第 1 条改了编号的拼法（第一条第 7、8 条）。群、图片和文件、斜杠命令、出站链与出站队列、`start/stop/status/logs`、WebUI 随后面的步子。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-onebot/`（第 5 层，头） | 桥 |
| `crates/miyu-onebot/src/main.rs` | 程序的入口：`miyu-onebot serve`：找资源目录、读给人看的字，装运行日志、读配置和 `bridge.json`，令牌没设的说清怎么设 |
| `crates/miyu-onebot/src/settings.rs` | 起来时读的两项：照核心登记的清单读系统配置、密钥文件（`miyu-endpoint` 的 `Config::load`，只读），令牌照引用取；握手以前说话的语言 |
| `crates/miyu-onebot/src/tuning.rs` | 读 `bridge.json`：桥自己的数 |
| `crates/miyu-onebot/src/serve.rs` | 起来：连核心、开监听，把几样接起来；核心断了、跟核心的那一头崩了就退 |
| `crates/miyu-onebot/src/listen.rs` | NapCat 反连进来的那一下：路径、令牌、升级 |
| `crates/miyu-onebot/src/listen/connection.rs`、`listen/bots.rs` | 一条 NapCat 的连接：回应、私聊、别的事件各交给谁，连上就问 `get_version_info`；一个机器人号一条连接，新的顶掉旧的 |
| `crates/miyu-onebot/src/onebot.rs`、`onebot/text.rs`、`onebot/calls.rs` | OneBot v11 的事件和动作：认一帧（私聊带上事件的 `time`）、读出私聊的文字、写 `send_private_msg`、调用和回应按 `echo` 配对；平台的名字 `qq`（`PLATFORM`）只写在 `onebot.rs`。三个小函数照它拼编号：`private_venue(号) -> Result<VenueId, FormatError>`、`person(号) -> Result<ExternalId, FormatError>` 经群聊内核拼（`Venue::new`、`miyu_chat::person`），`command_id(机器人的号, 消息编号, 时刻) -> String`（第 7、8 条） |
| `crates/miyu-onebot/src/core.rs`、`core/route.rs` | 跟核心的那一头：握手、`venue.session`、带 `as` 的 `session.send`、订阅、她的回复发回去 |
| `crates/miyu-onebot/src/texts.rs` | 说给人听的字：挑哪一句、换进什么字段，字照 `Human::load` 读（「给人看的字」） |
| `resources/software/onebot/bridge.json` | 桥自己的数：认的路径、调用等多久、两个队列多长、接不了连接歇多久（「对外的样子」） |
| `resources/software/onebot/human/{zh,en,ja}.json` | 桥说给人听的字（「给人看的字」） |
| `xtask/src/ledger.rs` | 登记簿门禁豁免 `software/onebot/bridge.json` 这一份文件：是数据，不发给模型 |
| `crates/miyu-core/src/settings.rs` | `onebot.listen`、`onebot.token` 两项配置（`OnebotSettings`）：权宜，照 `tui.startup` 的先例先由核心声明，9-1 挪进软件包的清单 |
| `crates/miyu-config/src/item/settings.rs`、`value.rs` | `settings!` 里整数可以写默认值；单个密钥的设置类型 `Option<Reference>`（O-8 加，两项要用） |
| `resources/core/human/{zh,en,ja}.json` | 两项配置给人看的字（`config.items`）、组 `onebot`（`config.groups`） |

### 一、骨架：主人的私聊（施工 O-8）

**对外的样子**

配置（系统配置，`head_start`：桥下次起来时生效）：

| 键 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `onebot.listen` | 整数 1024 到 65535 | 8301 | NapCat 反连进来的端口，只听本机 `127.0.0.1` |
| `onebot.token` | 密钥（`{ secret = … }` 或 `{ env = … }`） | 没有 | NapCat 连进来时出示的访问令牌；没设，桥不起来 |

桥自己的数在资源目录的 `software/onebot/bridge.json`，不进配置清单（照网页软件的 `web.json`）：

| 格 | 出厂 | 说明 |
|---|---|---|
| `paths` | `["/onebot/v11/ws", "/ws"]` | NapCat 连得进来的路径（第 2 条） |
| `call_timeout_seconds` | 10 | 一次 OneBot 调用等回应最多几秒（第 4 条） |
| `write_queue` | 64 | 往一条 NapCat 的连接写，最多攒几帧没写出去；满了写的一方等着。至少 1 |
| `inbound_queue` | 256 | 读出来的私聊最多攒几条没交给跟核心的那一头；满了读 NapCat 的那一头等着。至少 1 |
| `accept_retry_millis` | 100 | 接不了 TCP 连接（打开的文件太多这类）时歇几毫秒再接，不空转 |

多一格、少一格、队列写 0、读不了：起不来（「出错」）。

命令：`miyu-onebot serve`：前台跑，Ctrl+C、SIGTERM 停。9-4 以前由人手动起；9-4 以后由核心拉起，`miyu onebot start/stop` 开关。

NapCat 那边要配成「反向 WebSocket」，地址 `ws://127.0.0.1:<onebot.listen>/onebot/v11/ws`，访问令牌和 `onebot.token` 一样，消息格式选数组（字符串格式也认，第 6 条）。

**怎么走**

1. **起来**：找资源目录、读给人看的字；读系统配置的两项；令牌没设、取不到，照人的语言说清怎么设（`miyu config set` 和 `miyu login` 的密钥写法），退出码 1。读 `bridge.json`。用本机套接字连核心，没在跑就拉起（照终端界面）；握手的回应没带 `language`（`protocol.md`「握手」说一定带）是协议不对，照连不上核心说、退出码 1。然后开监听；端口被占了说清是哪个端口，退出码 1。
2. **连进来**：只认 `bridge.json` 的 `paths`（出厂 `/onebot/v11/ws` 和 `/ws` 两个），别的 404。令牌照 `Authorization: Bearer <令牌>`、`Authorization: Token <令牌>` 或查询参数 `access_token` 取，按常数时间比；对不上 401。不是 WebSocket 的升级请求 400；算出来的 `Sec-WebSocket-Accept` 放不进回应的头（照说不会），也是 400，不升级。机器人的号照 `X-Self-ID` 头取，没有的等第一条事件的 `self_id`。同一个号再连进来，新的顶掉旧的。
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

**样子**：桥起来时在标准错误上说一行「在 127.0.0.1:8301 等 NapCat 连进来」，照握手回的语言；NapCat 连上、断开各一行。

**出错**

| 情况 | 说什么 | 退出码 |
|---|---|---|
| 令牌没设、取不到 | 怎么设令牌 | 1 |
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
| `unready/no-token` | 令牌没设 | 还没设 QQ 桥的令牌（onebot.token）。先存一个：miyu login onebot，再写进系统配置：miyu config set --system onebot.token '{ secret = "onebot" }'。NapCat 那边填同一个令牌。 | The QQ bridge token (onebot.token) is not set. Save one with miyu login onebot, then put it in the system config: miyu config set --system onebot.token '{ secret = "onebot" }'. Give NapCat the same token. |
| `failure/port-in-use` | 端口被占 | 端口 {port} 被占了。换一个：miyu config set --system onebot.listen <端口>，NapCat 那边跟着改。 | Port {port} is in use. Pick another: miyu config set --system onebot.listen <port>, and change NapCat to match. |
| `failure/core` | 连不上核心 | 连不上核心：{reason} | Could not reach the core: {reason} |
| `failure/core-gone` | 核心断了 | 核心不在了，QQ 桥停下。 | The core went away; the QQ bridge stops. |
| `failure/crashed` | 跟核心的那一头、发回话的任务崩了 | QQ 桥出了错，停下：{reason} | The QQ bridge hit a bug and stops: {reason} |
| `failure/start` | 别的原因起不来 | QQ 桥起不来：{reason} | The QQ bridge could not start: {reason} |
| `unready/bad-port` | 端口读不出来（走不到） | onebot.listen 读不出来，照 miyu config check --system 查一下。 | onebot.listen could not be read; check it with miyu config check --system. |
| `usage` | 用法不对 | 用法：miyu-onebot serve | usage: miyu-onebot serve |
| `no-log` | 运行日志装不上（照样跑） | 运行日志写不了：{reason} | The run log cannot be written: {reason} |

号没认出来的（没带 `X-Self-ID`、还没来事件）用不带 `-as` 的那一句，不写括号那一段。令牌那一句的花括号在文件里写成 `{{`、`}}`（模板的写法）。

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
- 读配置：令牌没设、引用的密钥没存的起不来；照密钥文件、环境变量取；端口不写是 8301；握手以前的语言照 `ui.language`。（`settings.rs`）

**施工时定的**（O-8）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | 只要两项配置：端口、令牌；机器人的号照 NapCat 报的 | 握手时 NapCat 自己报，现在用不上第三项 | 先声明 `onebot.self_id` |
| 2 | 默认端口 8301 | 网页软件是 8300，挨着好记；旧版 QQ 和网页共用 8300，NapCat 的端口要跟着网页变（18 第三节） | 和网页共用 |
| 3 | 令牌必须设 | 18 第三节「默认只接本机，并且要带访问令牌」 | 本机连进来的可以不带 |
| 4 | 订阅不写 `after` | 桥重启不重发旧回复；漏发的由以后的出站队列补 | 记住看到哪条、重启接着推 |
| 5 | 核心断了桥就退 | 9-4 以后核心拉起，崩了照退避重起；桥里不另写一套重连 | 桥自己重连核心 |
| 6 | 桥自己读系统配置和密钥文件，用核心那一份读配置的代码（`Config::load`，照核心登记的全部清单），只读 | 密钥从不经协议交出去（`config.md` 第九条），令牌要用值；读配置在连核心以前（令牌没设不连核心）。**权宜**：为此依赖了 `miyu-core`、`miyu-endpoint`，和 18 第一节「不依赖核心的 crate」不一样；9-4 以后由核心拉起时，把这个包的设置连同密钥的值经标准输入交给它，到时去掉这两个依赖。代价：桥读配置只认核心登记的清单，系统配置里写了软件包的键（例如 `web.port`）时，`onebot.log` 记一条不认识的键的 `WARN`，值照读；随这段权宜在 9-4（下）一起去掉（2026-10-08） | 经 `config.get` 读：拿不到密钥的值 |
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
