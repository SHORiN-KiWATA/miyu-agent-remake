## 提供者：扩展登记工具、核心反向调用

### 是什么

核心自带的工具编译在核心里（`tools/`）。通讯平台的桥要给她「发群消息」「撤回」这类只有它做得到的工具，以后头也会有（终端集成）。提供者经协议的 `provide` 把自己的工具登记进核心的工具目录；她调到这件工具时，核心反向调用 `tool.call` 交给那个连接去跑，等它回结果（`05-内核接口.md` 第四节「内核空间模块与外部扩展」、第六节「工具的规格」、第八节「启用、注册与目录快照」，`04-核心协议.md` 第九节「方法一览」的「提供者」「反向调用」）。

状态：图纸，施工 O-2 上起（2026-10-09 主会话写；O-2 由主会话拆成上、中、下，补另起）。上、中、下做了。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/wire.rs` | 读进来的一行多一种：对核心发出去的请求的回应 |
| `crates/miyu-endpoint/src/reverse.rs` | 反向调用：核心发给一个连接的请求（编号 `core-<n>`），等它的回应；连接断了，在等的都了结（施工 O-2 上） |
| `crates/miyu-endpoint/src/provide.rs` | `provide`：查规格、登记进目录；提供者表：哪个包现在由哪个连接提供（施工 O-2 上） |
| `crates/miyu-endpoint/src/provide/remote.rs` | 提供者的一件工具（`Tool` 的实现）：执行时经提供者表找到那个连接、反向调用 `tool.call`（施工 O-2 上） |
| `crates/miyu-tool/src/catalog.rs` | 目录多一样：换掉一个包的工具，交回新的目录（施工 O-2 上）；记着哪几个包是提供者登记的（施工 O-2 中） |
| `crates/miyu-tool/src/shelf.rs` | 目录架子：核心现在的目录，换一次是一代；核心、执行器、权限策略、换快照的那一段拿着同一个（施工 O-2 中） |
| `crates/miyu-endpoint/src/provide/cache.rs` | 登记缓存：`provide` 成了写下，起来时、开扩展时照它先登记（施工 O-2 中） |
| `crates/miyu-session/src/actor/persona.rs` | 回合开头换快照：目录换了代，照现在的登记重新筛提供者的工具（施工 O-2 中） |
| `crates/miyu-kernel/src/session/asked.rs` | 是谁要她做的：回合记下她这时在回应的那一条的 `by`，派工具时带上（施工 O-2 下） |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展多一步 `serve`：登记以后答 `tool.call`（施工 O-2 上）；`silent` 不答（施工 O-2 下） |

### 对外的样子

**`provide {tools}`**（施工 O-2 上）：

1. 只收核心拉起的扩展的连接（`hello` 时认出是哪个包的）；别的连接回 `not_a_provider`。头扮演提供者随 O-2（下）。
2. `tools` 是这个包现在提供的全部工具，每件 `{name, description, input_schema, access, venues}`：
   - `name` 只用英文字母、数字、`_`、`-`，1 到 64 个字符；
   - `input_schema` 是 `{"type":"object",…}`；
   - `access` 是 `read`、`write`、`execute`、`network`、`outbound`、`venue` 之一（`venue` 施工 O-31 前，见下面「在场所里做的事」）；
   - `timeout_ms` 可以不写：等它答多久，1000 到 600000 毫秒，不写是 60000（施工 O-2 下）；
   - `venues` 是给哪种会话：`local`（本机的）、`private`（通讯平台的私聊）、`group`（群）里的一个或几个，不能是空的（05 第六节的 `venues`；2026-10-09 和通讯平台的会话定：「发给任意好友或群」只给本机，`skip_reply` 只给场所）。
3. 登记进工具目录，归这个包：换掉它上一次登记的那几件。名字撞上核心自带的、别的包的、写法不对的：整个不收，`bad_tool`，`data` 是 `{"tool": 名字, "problem": "duplicate" | "name" | "parameters" | "access" | "venues" | "timeout" | "feature"}`。`feature`（施工 T-2）：清单写了几个功能、这一件哪个功能都没列，或者写了空的 `[features]`；归法同预设认工具归哪个功能（`Features::of_tool`：列了的照列的，只有一个功能的都归它，没写 `[features]` 的整个包算一个）。归不上的预设开关不了它。回应 `{"tools": 件数}`。
4. 记下这个包现在由这个连接提供。连接断了、扩展崩了，工具照旧留在目录里（`05-内核接口.md` 第九节：工具从目录里消失会改变请求字节），被调到时回「暂时不可用」。
5. 这一次登记的原文写进登记缓存 `state/providers/<包>.json`（施工 O-2 中，下面「登记缓存」）。

**在场所里做的事**（访问类别 `venue`，施工 O-31 前，2026-10-10 核心定；`kernel/tools.md`、`session/guard.md` 第四条）：通讯平台上的动作（撤回、禁言、戳一戳这些）登记成 `venue`。权限策略只看会话在不在场所里：场所会话里放行、不问人，本机的会话里拒绝（`not_in_venue`）。场所会话没人能确认，`network`、`outbound` 在那里一律被拒，所以不能拿它们登记要在群里用的动作。

1. **谁能叫、能动谁是提供者的义务**：登记成 `venue` 的工具，核心不替它挡人。提供者照 `tool.call` 的 `by`（`by.role` 是 `manager` 的是管理的人）、`owner`、`venue.binding`（这个平台身份是不是终端管理员，`venues.md`「问对应表」：动谁的时候要认目标）自己挡；挡下的照工具结果回一句，`error` 是真。
2. **出去的动作不许记成 `read`**：`read` 是不碰这台机器、不出 Miyu 的。`send_message` 用 `read` 是因为它只发给她能看到的会话，不出 Miyu（`tools/send_message.md`）；`skip_reply` 只让桥这一轮不发，也不出去。撤回、禁言、戳一戳会在平台上留下后果，是 `venue`。

**`extension.disable`**（施工 O-2 中）：关掉的扩展的工具出目录，开着的会话下一个回合换掉；缓存留着。关是人的决定，和重启以后关着的包不读缓存一致；崩了、断了是一时的，不出目录。

**造会话时的工具面**：照这时的目录，提供者的工具照它的 `venues` 挑这个会话是哪种（本机的、私聊、群）。提供者的工具归它的包，预设照包开关，和核心自带的一样。

**包的工具变了，开着的会话下一个回合换上**（施工 O-2 中，`kernel/session.md`「换策略快照」）：会话记着它的快照照的是目录的第几代；回合开头目录换了代，照现在的目录、快照里的预设重新筛工具面：

1. 提供者的包里的那几件照现在的登记：新登记的加上，改了说明、参数的照新的，不再给这个会话的拿掉。
2. 核心自带的照旧快照里的原样（说明、`subagent` 能选的池、改过名的旧名字都不变），新装的自带工具不加：它们只随程序升级变，前缀稳定第一；预设改了时才照现在的目录加（施工 P-2 下）。
3. 目录里没有了的：会话上一次对过的那一代里是提供者的拿掉（扩展关掉了、不再登记它）；别的照旧留着，调到时暂时不可用（施工 4-2：程序升级拿掉的；载入的会话不知道上一次对的是哪一代，认不出来的都留着）。
4. 拼出来和原来一样的（只给群的工具、预设关着的包）什么都不记；不一样的照换人格的办法换上，记 `session.policy_changed`。
5. 载入的会话不知道快照照的是哪一代，第一个回合照现在的目录对一次。以前造的快照找不回预设的（没有指纹）不跟。
6. 程序升级过的老会话（核心的字变了）照样跟（施工 P-1 三补）：工具面真的变了才换，换的时候新的核心的字一起换上；光是升级、只有装了没开的那一行变了的不换。

**登记缓存**（施工 O-2 中）：`provide` 成了，这个包的参数原样写进 `state/providers/<包>.json`（派生的数据：一样的不写，先写临时文件再改名）。核心起来拉起扩展以前、`extension.enable` 拉起以前，开着的、批过的包照缓存先登记进目录，连接还没来，调到的暂时不可用；扩展连上来再 `provide`，照它的换掉。不缓存的话，核心重启以后、扩展重新登记以前跑的那一轮会把它的工具换掉，登记了又换回来，每次重启断两次缓存。缓存读不成、写错了、照它登记不上（和别的撞名）的不用，记一行 `WARN`；包卸了的不读，文件留着不管。

**给模型看的字**：出厂的包（随 Miyu 装的，通讯平台的桥）的工具说明放在资源目录 `software/<包>/tools/<名字>.json`，和核心自带的一个写法（`{description, parameters}`），扩展启动时经 `miyu_tool::load::spec` 读它们再 `provide`；登记进 `26-提示词.md` 第十节（登记簿门禁照资源目录查，不用另做），包自己的工具面预算照记忆的办法在它自己的 crate 里守（`crates/miyu-memory/tests/budget.rs`，量法记进 `10-自带软件.md` 第九节）（2026-10-09 和通讯平台的会话定）。第三方的扩展不在仓库里，不管。

**后台页的方法**（施工 F-6 中）：扩展另用 `package.methods` 登记它的后台页要调的方法，核心反向发 `method.call`，和登记工具的 `provide` 分开、不缓存，见 `package-pages.md`。

**`tool.call`**（反向调用，核心发给提供者，施工 O-2 上）：`{"jsonrpc":"2.0","id":"core-<n>","method":"tool.call","params":{…}}`，`params`：

| 格 | 是什么 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数，修正过的 JSON 对象 |
| `cwd` | 这一轮的工作目录 |
| `by` | 是谁要她做的（施工 O-2 下，下面「是谁要的」）：那一条的 `by` 原样，例如 `{"kind":"external","venue":…,"id":"qq:…","role":"manager"}`；没有触发的回合不写 |
| `owner` | 核心认不认 `by` 是主人本人（施工 O-2 下）：本机的人、私聊里对应表认出的本人、群里对应表里有的外部身份是；没有 `by` 的不是 |

回应 `{"result": {"blocks": [内容块…], "error": 布尔}}`：内容块照内核的写法（`text`、`image`），`error` 不写是假。回应是 JSON-RPC 的错误、写法不对的：算这次调用出错，原话写进结果。

**是谁要的**（施工 O-2 下，`asked.rs`）：回合记下她这时在回应的那一条的 `by`。开回合时是触发的那一条（照记下的几条开的是最后一条）；之后每次请求前，请求新看到的排队消息、并进来的群消息（`turn.joined` 的最后一条）、别的 harness 和别的会话发来的话里最新的那一条换上它；后台命令、子代理的回报是她自己派出去的事，不换。多条触发、并进来的几条照最后一条（2026-10-09 和通讯平台的会话定）：她这时看到的最新那一条才是这次调用要回应的。提供者照 `owner`、`by.role` 自己挡（群里的禁言、撤回、踢人只给主人、管理的人）。

**超时、`tool.cancel`**（施工 O-2 下）：到了登记时写的 `timeout_ms` 没回的，这次调用交回「没在 N 秒内答完、可能做了一部分」（`core/tool-results/timed-out.txt`，字段 `name`、`seconds`，秒数往上取整；提供者的工具自己读，不进快照的核心字：进了以后以前造的会话换不了快照）。核心发通知 `{"jsonrpc":"2.0","method":"tool.cancel","params":{"session":…,"call_id":…}}`（不带 `id`，不等回应）：超时、叫它停（打断改东西的调用；照样等它回，内核到点掐掉）、掐掉（打断别的、关核心，这次调用的等待被丢掉）时各发一次，回应先到的不发。提供者收到了就放下那次调用；已经做了的收不回，回的时候照实说。

### 怎么走

1. **读回应**：读进来的一行有 `result` 或 `error`、没有 `method` 的，是回应：照 `id` 找到核心发出去的那一条，交给等它的；找不到的不理。
2. **发反向调用**：每个连接一张「发出去还没回」的表；编号照连接从 `core-1` 数起。连接断了，表里在等的都了结成「连接断了」，往这个连接写的那一头也放掉：提供者表里存着这个连接，它要是还攥着写的那一头，写的任务就一直不结束，核心察觉不到扩展退出，崩了不重新拉起、端口被占退出了还显示在跑（施工 O-2 再补，通讯平台的会话 O-26 查出来的）。
3. **执行提供者的工具**：执行器照目录找到这件（`RemoteTool`）；它照包查提供者表：没有连接的，交回「暂时不可用」（`core/tool-results/unavailable.txt`）；有的，发 `tool.call`、等回应、到点算超时，超时和叫停时发 `tool.cancel`（施工 O-2 下）。不等了的调用从「发出去还没回」的表里拿掉。
4. **目录换代**：核心的目录放在架子上（`Shelf`），`provide`、读缓存、关扩展、装卸内置包（施工 F-5 中）各换一代，两处同时换的一个接一个；以后造的会话、载入的会话照新的，开着的会话下一个回合照上面「包的工具变了」换上；执行、判权限照现在的那一份找工具。

### 出错

| 什么时候 | 原因码 | 说明 |
|---|---|---|
| 不是核心拉起的扩展 | `not_a_provider` | 头扮演提供者随 O-2（下） |
| 规格不对、撞名、时限出了范围、归不上功能（施工 T-2） | `bad_tool` | `data.tool`、`data.problem` |
| 参数写错 | `bad_params` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 对核心发出去的请求的回应认得出来：`result`、`error` 原样交出，`id` 要是字符串、要是 2.0 |
| `crates/miyu-endpoint/tests/provide_exit.rs`（施工 O-2 再补） | 登记过工具的扩展一直退出：每次都察觉、照退避重新拉起，五次停下；端口被占退出的记成 `config_error` |
| `crates/miyu-endpoint/src/reverse/tests.rs` | 反向调用发出去的一行带 `core-<n>`、方法、参数；对上编号的回应交给等它的，对不上的不理；连接断了，在等的和以后发的都了结 |
| `crates/miyu-endpoint/src/provide/tests.rs` | 访问类别（施工 O-31 前起认 `venue`）、给哪种会话不认识的、空的拒；提供者表照包记，重新登记的换掉旧的；没有连接的、发不出去的暂时不可用，说法同执行器的 |
| `crates/miyu-tool/src/catalog/tests.rs` 的 `replacing_a_package_keeps_the_others_and_checks_the_new_ones` | 换掉一个包的工具：别的包的照留，原来那份不动，撞名、写法不对的整个不收 |
| `crates/miyu-session/src/agents/tests.rs` | 工具面照会话在哪挑提供者的工具：本机的、私聊、群 |
| `crates/miyu-endpoint/tests/provide_venue.rs`（施工 O-31 前） | 契约：登记成 `venue` 的工具收；本机的会话调到它，权限策略当场拒绝、写 `not-in-venue` 那一句，扩展收不到 `tool.call` |
| `crates/miyu-endpoint/tests/provide_features.rs`（施工 T-2） | 写了几个功能的，登记没列的工具整个不收（`feature`）、只登记列了的照收；空的 `[features]` 一件都不收；只写了一个功能的都收 |
| `crates/miyu-endpoint/tests/provide.rs` | 契约：扩展登记、撞名的整个不收、头不是提供者；新造的会话工具面里有给本机的、没有只给群的；`tool.call` 带会话、调用编号、参数、是谁要的、是不是主人；结果、错误、写法不对的各自交回；不答的到点超时、扩展收到 `tool.cancel`；扩展关掉了，下一个回合没有它的工具 |
| `crates/miyu-kernel/src/session/tests/respond/asking.rs`、`origin/tests.rs` 的 `the_owner_is_a_person_or_someone_on_the_owner_table` | 是谁要的：开回合的触发、照记下的几条开的最后一条、并进来的和排着队的被请求看到以后换上，回报不换、别的 harness 换；主人的判法 |
| `crates/miyu-endpoint/src/provide/tests/remote_tests.rs`、`reverse/tests.rs` | `tool.call` 带 `by`、`owner`；超时交回那一句、发 `tool.cancel`；叫它停发、照样等它回；掐掉发；回应先到的不发；不等了的调用从表里拿掉，通知不带编号、断了不发 |
| `crates/miyu-tool/src/shelf/tests.rs`、`catalog/tests.rs` 的 `the_catalog_knows_which_tools_came_from_a_provider` | 架子换一次是一代、拿着同一个的都看到，换不成的不动；目录记着提供者的包的工具 |
| `crates/miyu-session/src/actor/persona/shelf_tests.rs` | 目录换代：提供者的工具加上、改掉、去掉；自带的照旧、不新加（包里多了一件的）、改过名的照旧名字，目录里没有了的照旧留着；新装上的内置包的工具加进来、卸掉的照旧留着（施工 F-5 中）；只给群的不动本机的会话；载入的对一次；找不回预设的不跟 |
| `crates/miyu-endpoint/tests/provide_later.rs` | 契约：先造的会话下一个回合有扩展的工具、调得通、记 `session.policy_changed`，缓存照登记的原样；关掉以后下一个回合拿掉；重启以后扩展没登记，照缓存在目录里，载入的第一个回合对上、调到的暂时不可用，新造的带上；关着的包不读缓存 |

### 还没有的

- O-2（补）：执行中的进度 `tool.progress`；头扮演提供者；提供者登记命令、挂接点。
