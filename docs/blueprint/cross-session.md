## 跨会话

### 是什么

她能看到、读到你别的会话，能给它们发话，能请它们「空下来了告诉我一声」。M7 做的是一棵树上相邻两层之间的留言（子代理和父会话，`agents.md` 第六条）。这一页管不在一棵树上的主会话之间：列出来、读、发话、等它空下来。

- **列会话**：新的一件工具 `sessions`，第一行是她自己，下面是你别的主会话，最近有动静的在前。
- **读别的会话**：`history` 多一格 `session`，和翻自己的日志一样找、读。
- **发话、订「空了告诉我」**：留言的工具改名 `send_message`（原来叫 `message_agent`），`to` 多认会话的短编号，多一格 `notify_when_idle`。
- **收话**：那边照「别处来的」规矩收，和别的 harness 发来的话一样（`agents.md` 第十一条第 4 条），渲染成带标签的一块。
- **防刷屏**：同一个发话方限速、一模一样的不收、排着的有上限、一句话有长度上限。

状态：图纸，2026-10-01 起草，主会话审过一轮（同一天），待项目主人批准。定了的六条和防刷屏在 `docs/designs/29-跨会话.md` 第一、二节，这一页把第三节留下的技术细节定下来（末尾「起草时定的」）。每一节标着由哪一步做，步子见末尾「施工步子」（编号先用 C-1 到 C-7，主会话定正式编号）。做完一步，这一页照做好的样子改写那几节，相关的几页跟着改（末尾「要跟着改的别的页」）。

### 在哪

施工时照这个放（C-1 到 C-6）：

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/id.rs` | `SessionId::short()`：会话的短编号（C-1） |
| `crates/miyu-kernel/src/event/peer.rs` | 事件 `peer.idle`（C-1） |
| `crates/miyu-kernel/src/event/effect.rs` | 效果 `peer.watch`（C-1） |
| `crates/miyu-kernel/src/ledger/peers.rs` | 账本：最近收下的别的会话的话（防刷屏用），在等哪几个会话的通知（C-1、C-2） |
| `crates/miyu-kernel/src/session/peers.rs` | 收别的会话发来的话、防刷屏三条（C-2）。收空了的通知、作废、不在了，「空了」的判断，最后回复的第一行（C-6） |
| `crates/miyu-policy/src/peers.rs` | 策略数据 `peers.*` 的出厂值（C-2、C-6） |
| `crates/miyu-assemble/src/peers.rs` | 两种渲染：别的会话发来的话、空了的通知（C-2、C-6），标签那一块照 `tag.rs` |
| `crates/miyu-endpoint/src/list.rs` | 列会话多算三样：工作目录、忙不忙、最近一次动静，`session.list` 和 `sessions` 共用（C-3） |
| `crates/miyu-endpoint/src/spawn.rs` | 会话表那一头的 `SessionPort` 多几样：列主会话、认短编号、只读地开别的会话的日志（C-3、C-4），发话时交回对方有没有人看着（C-5），订、发通知（C-6） |
| `crates/miyu-session/src/spawn.rs` | `SessionPort` 多的那几样定义在这一层，会话表在上一层实现 |
| `crates/miyu-tool/src/sessions.rs` | 交给 `sessions`、`history` 的端口 `SessionsPort`：列、认、开日志（C-3、C-4） |
| `crates/miyu-tool/src/messages.rs` | `Recipient` 多 `Session`，`NotSent` 多几种（C-5）。只订不发时认出会话、交回整个编号（C-6） |
| `crates/miyu-session/src/messages.rs` | 执行器：认会话编号、送、订（C-5、C-6） |
| `crates/miyu-session/src/peers.rs` | 执行器：每送完一批，照内核新多出来的在等的去订、计时，到点交作废（C-6） |
| `crates/miyu-session/src/actor/watchers.rs` | 被等的那一边：谁在等，每送完一批看空没空，空了发通知（C-6） |
| `crates/miyu-session/src/agents.rs` | 工具面：`sessions` 只给本机的主会话（C-3） |
| `crates/miyu-basesystem/src/sessions.rs` | `sessions`：参数、一行一个、分页（C-3） |
| `crates/miyu-basesystem/src/history.rs`、`history/entry.rs` | 「谁」多一种 `session <短编号>`（C-2），多一格 `session`（C-4） |
| `crates/miyu-basesystem/src/send_message.rs` | 原来的 `message_agent.rs` 改名（C-5）。`to` 认会话编号、长度上限（C-5），`notify_when_idle`（C-6） |
| `resources/software/basesystem/tools/{sessions,history,send_message}.json` | 说明和参数格式 |
| `resources/software/basesystem/{sessions,history,send_message}/*.txt` | 结果里给她看的几句 |
| `resources/core/peers/*.txt` | 两种标签和通知里的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

分层照 `01-架构.md`，和子代理那一套一样。内核不碰别的会话，只从日志算状态、交出动作。工具只拿端口，不认识会话表。会话表在 `miyu-endpoint`，比会话 actor 高一层，端口由 `miyu-session`、`miyu-tool` 定义，`miyu-endpoint` 在核心启动时装上（`agents.md`「在哪」）。

### 对外的样子

**策略数据**（快照里的默认值，配置那一步能改，`14-配置.md`）：

| 名字 | 默认 | 管什么 | 谁用 |
|---|---|---|---|
| `peers.burst` | 5 | 同一个发话方在一个窗口里最多几句（第五条第 1 款） | 收话那边的内核（C-2） |
| `peers.window` | 600 秒 | 限速、去重看多久以内的（第五条第 1、2 款） | 同上 |
| `peers.unread` | 50 | 还没听到的别的会话来的话最多几句（第五条第 3 款） | 同上 |
| `peers.watch_hours` | 12 | 订了多久没等到通知就作废（第六条第 8 款） | 等的那边的内核（C-6） |
| `peers.status_chars` | 200 | 通知里带的那一行最多几个字（第六条第 6 款） | 被等的那边的内核（C-6） |
| `peers.message_chars` | 100000 | 一句话最多几个字（第五条第 4 款） | 发话那边的工具（C-5），出厂值在 `crates/miyu-basesystem/src/send_message.rs`，照 `jobs.output_chars` 的放法，不进快照 |

- 前五个在快照里是 `peers`（C-2、C-6 各加自己用的）。以前造的快照里没有的，照出厂值读：防刷屏不能因为会话旧就不管。
- 数是估的，待 C-7 实测（「起草时定的」第 11 条）。

**会话的短编号**（C-1，写进 `kernel/ids.md`）：会话编号最后 8 个字符，就是最后一段的后 8 位十六进制。`0192f3a0-1111-7abc-8def-001122334455` 的短编号是 `22334455`。

- 为什么取后面：会话编号是 UUIDv7，前 12 位十六进制是造的那一毫秒，前 8 位约 65 秒才变一次，同一分钟里开的几个会话前 8 位一样。最后 32 位是随机数（`uuid` 1.26 的 `ContextV7`：42 位计数器之外的位补随机数）。
- 从编号算得出，不另存。给模型看的、头显示的、标签里的，都是这一个写法。
- 撞了：列表里有两个会话的后 8 位一样，这几个写后 12 位（整个最后一段）。还一样的写整个编号。标签里一律写 8 位，从 `by` 算，不看撞没撞（前缀要稳）。
- 认的时候收整个编号，或者至少 8 位的小写十六进制，照后缀对（第三条第 1 款）。

**「谁」**（C-1）：别的会话发来的话，`message.user` 的 `by` 沿用 `session`（`{"kind":"session","id":<发话的会话>}`），不加新种类。收话的会话照关系分三种：

| `by` 是 | 是什么 | 照哪条收 |
|---|---|---|
| 这个会话的父会话（`session.created` 的 `parent`） | 交代、留言 | 「发一条消息」，子会话欠一份回报（`kernel/session.md`） |
| 这个会话派的子代理的子会话（`job.started` 的 `session`） | 子代理的留言 | 「子代理的留言」（`kernel/session.md`） |
| 别的会话 | 别的会话发来的话 | 这一页第四条 |

- `ids.md` 里 `session` 那一行本来就写着「另一个会话」。关系在日志里都查得到，旧核心照样读得懂。

**效果** `peer.watch`（C-1、C-6，`tool.result` 的 `effects`）：`send_message` 订了「空了告诉我」，那次调用报一条。这就是订的记录，账本照它算在等哪几个会话。

```json
{"kind":"peer.watch","session":"0192f3a0-2222-7abc-8def-5566778899aa"}
```

- `session`：被等的会话的整个编号。
- 账本查：合会话编号的写法，不是这个会话自己。

**事件** `peer.idle`（C-1、C-6）：等的那个会话空下来了，或者等不到了。记在等的那一边。

```json
{"session":"0192f3a0-2222-7abc-8def-5566778899aa","reason":"idle","status":"CI 修好了：macOS 上的临时目录换成了真实路径。"}
{"session":"0192f3a0-2222-7abc-8def-5566778899aa","reason":"expired"}
```

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `session` | 会话编号 | 必有 | 等的是哪个会话 |
| `reason` | `idle`、`expired`、`gone` | 必有 | 空下来了，到点作废了，那个会话不在了。不认识的原样留着 |
| `status` | 字符串 | 可以没有 | `idle` 的才有：它最近结束的那一轮最后一条有字的回复的第一行。没说话的不写 |

- `by`：`idle` 的是那个会话，编号和 `session` 一样。`expired`、`gone` 的是内核。
- `cause`：`idle` 的是交来通知的那个命令。`expired`、`gone` 的是订它的那一轮的 `cause`。
- 不带回合编号：它是别处来的，撤哪一轮都不拿走（第七条第 2 款）。
- 账本查：这时在等 `session` 的通知。`by` 是会话的，编号要和 `session` 一样。
- 名字不叫 `session.*`：渲染表里 `session.*` 一律不进上下文（`kernel/request.md`「组装」第 3 条）。

**内核多的输入和原因码**（C-2、C-6，写进 `kernel/session.md`）：

- 命令 `PeerIdle { status }`：被等的会话交来的通知，`by` 是它（C-6）。
- 输入 `WatchEnded { session, reason, at }`：执行器交的到点、不在了，`reason` 是 `expired` 或者 `gone`（C-6）。
- 查询 `watching()`：在等哪几个会话、各从什么时刻算起（C-6）。`last_line()`：最近结束的那一轮最后回复的第一行，截到 `peers.status_chars`（C-6）。
- 拒绝的原因码：`too_many_messages`、`duplicate_message`、`inbox_full`（C-2），`unknown_watch`（C-6）。这几种只会回给核心里别的会话，不经协议给头。

**工具**：

- 新的一件 `sessions`（C-3），一页 `tools/sessions.md`（施工时建）。
- `history` 多一格 `session`（C-4，`tools/history.md`）。
- 留言的工具 `message_agent` 改名 `send_message`（照 Claude Code 的 `SendMessage`，2026-10-01 项目主人定）。C-5 改名，和说明第一句、`to` 一起改，只冷一次缓存。以前造的会话快照里冻着 `message_agent`（前缀不能变），它们发来的 `message_agent` 调用照样执行、认成同一件，给人看的字两个键都在，照施工 7-5 再补给 `agent` 改名 `subagent` 的做法。结果里的几句从 `message_agent/` 挪到 `send_message/`，字节不变，登记簿换路径。`tools/message_agent.md` 改成 `tools/send_message.md`。
- `send_message` 的 `to` 多认会话编号（C-5），`message` 改成可以不写，多一格 `notify_when_idle`（C-6）。
- 派子代理的工具 `agent` 另开小单改名 `subagent`（施工 7-5 再补，2026-10-01 项目主人定），这一页照新名字写。
- 说明的原文见「样子」，每份待量 token，量了进登记簿（`26-提示词.md` 第十节），受工具面的预算管（`10-自带软件.md` 第九节）。估的份量（待量）：`sessions` 约 100，`send_message` 多约 45，`history` 多约 18。主会话的工具面一共多约 165，子会话多约 63（没有 `sessions`）。

**协议**（`protocol.md`）：

- `session.list` 的每一项多三格（C-3）：`cwd` 会话的工作目录（头报来的写法），`busy` 这时有回合在进行（是的才写 `true`，和 `pinned` 一样），`last_active` 日志最后一条事件的时刻。和 `sessions` 是同一个函数算的。排序不改，还是照编号倒着排。

```json
{"busy":true,"cwd":"~/src/miyu","last_active":"2026-10-01T06:03:12.345Z","oneshot":false,"parent":null,"session":"0192f3a0-2222-7abc-8def-5566778899aa","title":"修 CI"}
```

- 推送不加方法：`by` 是别的会话的 `message.user`、事件 `peer.idle`、效果 `peer.watch` 照原样推。头照「给人看的字」显示。C-2、C-3、C-6 合了，照规矩把 sha 和形状告诉两个头（终端界面、网页）。
- `session.redo` 第 3 条多两种 `not_redoable`：最后一轮是别的会话发来的话开的、是空了的通知开的（C-2、C-6）。
- 协议上不开「以哪个会话的身份发话」的口子。别的会话的话只从 `send_message` 来，在核心里经会话表送。

### 怎么走

**一、列会话**（`sessions`，C-3）

1. 只有本机的主会话工具面里有它（第九条）。
2. 执行器经会话表要一份会话：属主和这个会话一样、`session.created` 不带 `parent`（主会话）、没删的，不含它自己。每个会话交编号、标题、工作目录、忙不忙、最近一次动静。和 `session.list` 同一个函数算（`protocol.md`「`session.list`」第 2、3 条：读第一条，再整份读一遍盖上标题），多算三样：
   - 工作目录：日志里最后一条带 `cwd` 的 `turn.started` 的，没有就照 `session.created` 的。和「会话表」第 5 条同一个认法。
   - 最近一次动静：日志最后一条事件的 `at`。日志坏了的，照坏的那一段以前的。
   - 忙：它在会话表里，这时有回合在进行（回合结束了 `turn.ended` 还没落盘、正在改回文件也算）。和 `Core::idle` 看的是同一样。没载入的都是闲。在等人确认、等人回答的也算忙。
3. 照最近一次动静排，新的在前。一样的照编号倒着排。从第 `offset` 个起（从 0 数，不写是 0），最多 `limit` 个（不写是 20）。
4. 第一行是她自己：`you.txt`，写她的短编号。接着一个会话一行：有标题的 `listed.txt`，没有的 `listed-untitled.txt`。`state` 是 `busy` 或者 `idle`。时刻照这个会话的时区写到分钟，和 `history` 一样。
5. 后面还有的，末尾接 `more.txt`。一个别的会话都没有的，第一行后面接 `none.txt`。`offset` 过了结尾的，第一行后面接 `past-end.txt`。
6. 短编号撞了的照「对外的样子」放长。
7. `limit` 不是正整数、`offset` 是负数、类型不对：参数不对，端口不问。别的参数不认，也不报错。
8. 没有端口的（测试里的假调用）：只交 `none.txt`，不写第一行。
9. 访问类别 `read`，不报路径，不报效果。叫停：读下一个会话的日志之前看一眼。
10. 现在每列一次都把每个会话的日志整份读一遍，和 `session.list` 一样。慢了随会话列表的索引一起换（「还没有的」）。

**二、读别的会话**（`history` 的 `session`，C-4）

1. 写了 `session` 的，先照第三条第 1 款认它，和 `send_message` 用同一个认法。认成她自己的，照没写。找不到（`no-session.txt`）、对得上不止一个（`ambiguous.txt`）、这个会话不能读别的会话（`not-here.txt`，第九条）：各一句，出错，一条日志都不读。
2. 认出来的，经会话表只读地开那个会话的日志（`ReadLog`），不载入它。它正在跑也能读：最后一段没写完的半行跳过（`tools/history.md`「交给工具的」）。
3. 别的全照 `tools/history.md`：哪些算一条、找、读、筛、分页、整页上限、叫停。时刻照她这个会话的时区。
4. 「谁」照那个会话看：`user` 是那个会话里的人，`assistant` 是那个会话里的她。那个会话收到的别的会话的话写 `session <短编号>`（第八条第 1 款）。
5. 一次只读一个会话（「定的」第 5 条）。
6. 读不了日志：照 `history` 原来的那一句。

**三、发话**（`send_message` 的 `to` 写会话编号，C-5）

1. 认 `to`，照这个先后：
   1. `parent`：发给父会话，照 `tools/send_message.md`。
   2. 合任务编号写法的（`j1`、`j2.1`）：发给自己派的子代理，照 `tools/send_message.md`。
   3. 别的当会话编号认：整个编号，或者至少 8 位的小写十六进制。别的写法（大写、带空格、不到 8 位）照找不到。这个会话不能发给别的会话的（第九条），交回 `not-here.txt`，不去找。
   4. 在这个会话能看到的主会话里找（第一条第 2 款那一批，加上它自己）：整个编号相同，或者编号以它结尾。一个都没有：`no-session.txt`。不止一个：`ambiguous.txt`。是它自己：`self.txt`。
2. 长度：`message` 超过 `peers.message_chars`（100000 个字）的，发出去之前拒，`too-long.txt`。父子之间的留言也照它（「起草时定的」第 13 条）。
3. 经会话表把话送过去：命令 `Send`，一块字，原样。`by` 是这个会话。命令编号 `<这个会话>/message/<调用编号>`，和父子之间同一个写法。对方没载入的，会话表先载入它。
4. 回应：
   - 接受，对方这时是没人看着的一次性会话（`oneshot`，没有头订阅着，会话表照它的 `Handle` 看）：`held.txt`。它不开轮，话记下了，有人接着说时它一起看到（第四条第 3 款）。
   - 接受，别的：`sent.txt`，和父子之间同一句。
   - 拒绝 `too_many_messages`：`too-many.txt`。`duplicate_message`：`duplicate.txt`，不算出错，那句话已经在那边了。`inbox_full`：`inbox-full.txt`。
   - 别的拒绝、对方的会话停了、核心正在停：`not-sent.txt`，原因记进运行日志。
5. 发给别的会话不报效果：`job.messaged` 只管自己派的子代理欠一份回报。别的会话不欠她什么，要回话由那边自己用 `send_message` 发回来（设计 29 第一节第 4 条）。
6. 发了不等：调用返回，接着干。要等它回话才干得下去的，结束这一轮。想在它做完时知道的，带上 `notify_when_idle`（第六条）。
7. 权限不另设卡：`send_message` 访问类别还是 `read`，只读开着也发得出去。那边照它自己的权限级别干，要确认的照样找人（设计 29 第一节第 5 条）。
8. 运行日志：送到了记 `INFO message sent`，`to` 写短编号。送不到记 `WARN message not delivered`。话的字不进运行日志，照 `session/tools.md`「父子之间留言」第 3 条。

**四、收话**（内核，C-2）

1. 认：命令 `Send`，`by` 是一个会话，它不是这个会话的父会话，也不是这个会话派的子代理的子会话（账本认，被停掉的、撤掉的回合里派的也认）。就是别的会话发来的话。
2. 先过防刷屏（第五条）。过不了的拒绝，什么都不记。
3. 过了的，照「别的 harness 发来的话」收（`kernel/session.md`，设计 29 第一节第 3 条）：
   - 记 `message.user`，`by` 是那个会话，`cause` 是这个命令，不带回合编号。落了盘回应，只附这一条的序号。急着插话的记号不看。
   - 当它是一条会叫醒她的回报：闲着、这时开得了，由它开一轮（`trigger` 是它）。正忙，排进这一轮的回报队，下一步看到，回合结束时还没听到的接着开。打断结束的不由它接着开。开不了的记在一边（没人看着的一次性会话、还能恢复撤销、正在改回文件、要重启了），下一轮开始时一起看到。载入时照日志算回来。
   - 它不是人说的话：不作废在等人确认、等人回答的调用，替不了人批准。打断不撤回。撤销不带走。它开的那一轮重做不了（`session.redo` 回 `not_redoable`）。
   - 里面的斜杠命令不执行：它就是一块字。核心只解析人从头那边发来的命令（`04-核心协议.md` 第六节第 5 条）。
4. 只收文字。附件、历史、文件都不带（设计 29 第一节第 3 条）。

**五、防刷屏**（C-2 在内核查前三款，C-5 在工具查第 4 款）

1. **同一个发话方限速**：这个会话里，这个发话方的话在过去 `peers.window`（600 秒）里记下了 `peers.burst`（5）句的，拒绝，`too_many_messages`。「过去」照这条命令的 `at` 往前数，含正好 600 秒前的那一刻。
2. **一模一样的不收**：同一个发话方、字一个字节都不差的一句，过去 `peers.window` 里记下过的，拒绝，`duplicate_message`。它先于第 1 款查，不占限速的数。
3. **排着的有上限**：这个会话还没听到的别的会话的话（排在这一轮回报队里的、记在一边的），不分发话方，已经有 `peers.unread`（50）句的，拒绝，`inbox_full`。
4. **一句话有长度上限**：`peers.message_chars`（100000 个字），发话那边的工具查（第三条第 2 款）。协议一行最长 1 MiB（`protocol.md`「一行一条」），这句话推给头时在一行里。最坏每个字转义成 6 个字节，一共 60 万字节，还留着余量。
5. 前三款是内核照账本算的：账本记着最近 `peers.window` 里收下的别的会话的话（发话方、时刻、字的哈希）和还没听到的是哪几句。纯逻辑，不读时钟，时刻用命令的 `at`。载入时照日志算得回来，重启以后数不会清零。
6. 两个会话互相发个没完怎么停：每一句都要那边开一轮才有回话。第 6 句被拒，发话的这一轮拿到的是被拒的那一句，那边不会被叫醒，来回就断了。被拒的那一句写明把剩下的并成一句、过会儿再发。
7. 只管别的会话发来的。父子之间的留言不受「5 句」管：子代理干活时问得多。它只受长度上限管（第 4 款）。别的 harness 发来的话不在这里管（「还没有的」）。

**六、空了告诉我**（`notify_when_idle`，C-6）

1. **订**：`send_message` 写 `notify_when_idle: true`，`to` 是别的会话。
   - 带 `message` 的：先照第三条发话，送到了（`sent`、`held`、`duplicate` 都算）再订。发话被拒、送不到的，整次不订。
   - 不带 `message` 的：只订。那边不开轮、不花 token。
   - `to` 是子代理、`parent` 的：整次拒，`watch-peers-only.txt`，留言也不发（照 Claude Code）。
   - `message` 不写、`notify_when_idle` 也不是 `true` 的：参数不对。
2. **这边记下**：端口认出那个会话（和发话同一个认法），那次调用报效果 `peer.watch`，结果接一句 `watching.txt`。工具自己不去订。效果落了盘，账本记着在等那个会话，从这条效果的时刻算起。已经在等它的又订一次，从新的时刻重新算。
3. **那边记下**：这边每送完一批，执行器照内核的 `watching()` 看有没有新多出来的在等的，新多出来的经会话表交给那个会话的 actor 一个「订」，不经内核、不进它的日志。订的那次调用落了盘、载入以后、恢复撤销以后，走的都是这一条路。落了盘才订，通知就不会赶在这边记下在等以前到（施工 7-6 撞过的那种先后）。那个会话这时已经空了（第 4 款），当场发通知。不然记进它 actor 的名单，同一个会话只记一个。它没载入的，会话表先载入它：刚载入、没有要接着干的，就是空的，当场发。
4. **「空了」**：内核说空闲（没有回合在进行、没有结束了 `turn.ended` 还没落盘的、没在读回日志、改回文件），而且它派的子代理都不欠它回报（`waiting_children()` 是空的）。和 `miyu ask` 等到的是同一个时刻（`agents.md` 第十一条第 1 条）：子代理报上来、被叫醒的那几轮也做完了才算。后台命令不算。
5. **发**：被等的会话的 actor 每送完一批看一次。空了、名单不空的，给名单上的每一个发通知，清空名单。发通知不挡着 actor：一个一个起任务发。先把通知交出去，再写「没有在跑的回合」，免得核心在通知的路上空闲退出。
6. **通知**：经会话表交给等的那个会话一个命令 `PeerIdle`，`by` 是被等的会话，命令编号 `<被等的会话>/idle/<等的会话>/<被等的会话这时日志最后一条的序号>`。带 `status`：它最近结束的那一轮最后一条有字的回复的第一行，去掉前后空白，超过 `peers.status_chars`（200）个字的截断、末尾接 `…`，由它的内核交（`last_line()`，和向上回报拿正文是同一个认法，`kernel/session.md`「向上回报」第 3 条）。一个字都没说的不带。
7. **这边收**：账本说在等它的，记 `peer.idle`（`reason` 是 `idle`）。不在等的，拒绝 `unknown_watch`，什么都不记（订它的那一轮撤掉了、已经收到过、作废了）。被拒的那一边记一行运行日志，不再发。记下的照第四条第 3 款叫不叫醒她：闲着、开得了开一轮，正忙下一步看到，开不了的记在一边。
8. **作废**：执行器照 `watching()`，每一个在起算以后 `peers.watch_hours`（12 小时）到点时，交内核 `WatchEnded`（`expired`，带这一刻）。计时和第 3 款的订同时起。内核照账本还在等、确实到了点的，记 `peer.idle`（`reason` 是 `expired`，`by` 是内核）。只记下，不叫醒：它不是做出来的结果，照 `undone`、`aborted` 的回报（`agents.md` 第三条第 3 条）。
9. **重启**：被等的那边的名单只在内存里，核心一停就没了。等的这边载入以后，账本里在等的都算新多出来的，照第 3 款再订（同一个会话只记一个，不会重）、重新计时。已经到点的不订，当场交 `WatchEnded`（`expired`）。订的时候那个会话已经不在了（删了）：交 `WatchEnded`（`gone`），记 `peer.idle`（`reason` 是 `gone`，`by` 是内核），只记下。
10. 被等的会话被删了：它 actor 的名单跟着没了，不另发。等的这边到点作废，或者再订时发现它不在了（「还没有的」）。
11. 核心空闲退出不受它影响：等的这边闲着，不算忙。被等的那边忙着，核心本来就不退。它空了，通知当场就发了。
12. 只有本机的主会话能订，只能订本机的主会话（第九条）。

**七、撤销、删除、压缩**（C-2、C-6）

1. 发出去的话撤不回：这边撤销那一轮，那边照样收着，和父子之间的留言一样。
2. 收到的话、`peer.idle` 都不带回合编号，撤哪一轮都不拿走（别处来的留着，`kernel/history.md`「拿走什么」第 3 条）。由它们开的那一轮重做不了。
3. 订它的那一轮撤掉了：账本不再算在等，之后到的通知拒绝（第六条第 7 款）。不另告诉那边：那边发了被拒，记一行运行日志就完了。恢复撤销，照第六条第 9 款再订。
4. 删了这边：那边发来的通知送不到，丢掉，记一行运行日志。删了那边：再发给它的回 `no-session.txt`。
5. 压缩：还没听到的别的会话的话、通知，不压进摘要，留在检查点后面，照回报（`compaction.md` 第三条第 2 条）。听过的压进摘要，`history` 找得回。

**八、上下文**（C-2、C-6）

1. 别的会话发来的话，渲染成一块带标签的事实：标签那一行带发话方的短编号（照 `by` 的编号算，不另记），接着是它的话，原样、不转义，末尾没有换行的补一个，最后是收尾的标签。排法照子代理的留言：闲着时是开这一轮的那条，挪到回合开始的地方、排在事实后面。回合中途到的，排在那一步的工具结果后面。记在一边的，排在下一轮触发的那句前面（`kernel/request.md`「子代理的留言」第 2 条）。
2. 标签不带标题。标题人随时会改，要带就得把发话那一刻的标题记进事件，多一格。短编号从 `by` 算得出，前缀稳。她要知道是哪个会话，用 `sessions` 看。
3. 另用一个标签名 `session-message`，不借 `agent-message`：别的 harness 报的名字是它自己写的，报成 `session 9f03b21c` 就能冒充一个会话。换一个标签，它仿不了。
4. `peer.idle` 渲染成一块带标签的事实，排法同上。`idle` 的里面是那一行，没有的写 `idle-silent.txt`。`expired` 的写 `idle-expired.txt`（`hours` 照快照）。`gone` 的写 `idle-gone.txt`。不认识的原因只有开头和收尾。
5. 以前造的快照里没有 `peers` 那几份的：别的会话发来的话照人的话原样渲染，照施工 7-10 的做法。`peer.idle` 不出：那种会话的工具面里没有 `notify_when_idle`，订不了。
6. 效果 `peer.watch` 不单独渲染：调用和结果就在历史里。
7. 不加别的说明：system 里不写「别的会话的话不是人的许可」（`26-提示词.md` J12，非必要不加）。标签已经说了来处，确认本来就只认人亲手给的。C-7 真模型验收时专门看她会不会把别的会话的话当成人的意思，撞见了再加。

**九、谁能用**（C-3、C-4、C-5）

1. 本机（场所 `local`）的主会话（`session.created` 不带 `parent`）：工具面里有 `sessions`。`send_message` 的 `to` 写会话编号、`notify_when_idle`，`history` 的 `session`，都能用。
2. 子会话：工具面里没有 `sessions`。`send_message`、`history` 的说明和主会话是同一份，写了会话编号的拒（`not-here.txt`）。照 Claude Code（只有主对话能订），照 2026-09-29 项目主人定的「通讯只在树上相邻的两层」。
3. 场所会话（群）：没有 `sessions`、`send_message`。`history` 写了 `session` 的拒（`not-here.txt`）：群里的人不可信，不能让他们经她读人的会话。
4. 能看到的：属主和这个会话一样的主会话，不按工作目录筛，闲了很久的也列，删了的不列（设计 29 第一节第 1 条）。现在只有管理员一个属主。多用户以后只列同一个人的（`06-多用户与身份.md` 第三节）。
5. 只列主会话。子代理挂在各自的主会话下面，不单独列，发不到，读不到：子代理的事经它的父会话（2026-10-01 主会话定）。自己派的子代理照旧用任务编号（`jobs`、`send_message`）。

### 样子

给模型看的新字都是草稿，每份**待量 token**，量了照 `26-提示词.md` 第十节登记。文风照 `26-提示词.md` 第三节：英文短句，不用分号串，参数一句。

**`sessions` 的说明和参数**（C-3），草稿，施工时写进 `resources/software/basesystem/tools/sessions.json`（待量 token）：

```json
{
  "description": "List your other sessions, most recently active first. Each row gives the id to use with send_message and history, the title, working directory, whether it is busy and when it was last active.",
  "parameters": {"type":"object","properties":{"limit":{"type":"integer","description":"Default 20."},"offset":{"type":"integer","description":"How many sessions to skip."}}}
}
```

说明里点名的 `send_message`、`history` 在有 `sessions` 的会话里都在（`26-提示词.md` J4）。

**`send_message` 改成这样**（C-5 改名、改说明第一句和 `to`，C-6 加 `notify_when_idle`、`message` 改成可以不写），待量 token：

```json
{
  "description": "Send a message to a subagent you started, to your parent with `to: parent`, or to another of your sessions by its id. The other side reads it at its next step, or starts a new turn with it if idle. Send only what they need to know now, such as a question or a finding that changes their plan, since your final report goes up on its own.",
  "parameters": {"type":"object","properties":{"to":{"type":"string","description":"The job id of your subagent, such as j1, parent, or a session id."},"message":{"type":"string","description":"The message to send, which can be left out with notify_when_idle."},"notify_when_idle":{"type":"boolean","description":"Get one notice when that other session next finishes its work."}},"required":["to"]}
}
```

- 名字从 `message_agent` 改成 `send_message`。第一句多了「or to another of your sessions by its id」，第二、三句一字不改（第三句是 7-7 防刷屏叫醒父会话的那一句，登记在案）。
- 说明不点名 `sessions`：子会话和主会话共用这一份，子会话里没有 `sessions`（J4）。

**`history` 多一格**（C-4），待量 token：

```json
"session":{"type":"string","description":"Another session's id, to read that session instead of this one."}
```

**`sessions` 的结果**，例子：

```text
You are session 22334455.
9f03b21c "修 CI" in ~/src/miyu: busy, last active 2026-10-01 14:03
0c5d77aa (untitled) in ~/notes: idle, last active 2026-09-30 22:41
(Showing 1-2 of 5. Use offset=2 to see more.)
```

**`send_message` 发给别的会话、订了**，例子：

```text
Message sent to 9f03b21c.
You will get a notice when 9f03b21c is next idle.
```

**`history` 读别的会话**，例子（那个会话收到过 `22334455` 发来的一句）：

```text
#88 2026-10-01 14:10 session 22334455: …迁移写完了，按会话分区…
#61 2026-10-01 13:02 user: …先把 CI 修好再说…
```

**别的会话发来的话**渲染出来（C-2），例子：

```text
<session-message from="22334455">
迁移写完了，按会话分区。你那边的导出可以接上了。
</session-message>
```

**空了的通知**渲染出来（C-6），例子：

```text
<session-idle session="9f03b21c" reason="idle">
CI 修好了：macOS 上的临时目录换成了真实路径。
</session-idle>
```

```text
<session-idle session="9f03b21c" reason="expired">
No notice came within 12 hours, so the request was dropped.
</session-idle>
```

给她的字，每一份以一个换行结尾，全部待量 token，施工时登记：

| 什么时候 | 文件 | 原文 | 步 |
|---|---|---|---|
| 列会话，第一行 | `sessions/you.txt` | `You are session {id}.` | C-3 |
| 列会话，有标题的一行 | `sessions/listed.txt` | `{id} "{title}" in {cwd}: {state}, last active {time}` | C-3 |
| 列会话，没标题的一行 | `sessions/listed-untitled.txt` | `{id} (untitled) in {cwd}: {state}, last active {time}` | C-3 |
| 列会话，后面还有 | `sessions/more.txt` | `(Showing {from}-{to} of {total}. Use offset={next} to see more.)` | C-3 |
| 列会话，没有别的 | `sessions/none.txt` | `You have no other sessions.` | C-3 |
| 列会话，`offset` 过了结尾 | `sessions/past-end.txt` | `(You have {total} other sessions. Offset {offset} is past the end.)` | C-3 |
| 读别的会话，「谁」那一格 | `history/session.txt` | `session {id}` | C-2 |
| 读别的会话，找不到 | `history/no-session.txt` | `No session has the id "{session}".` | C-4 |
| 读别的会话，对得上不止一个 | `history/ambiguous.txt` | `"{session}" matches more than one session. Use the full id.` | C-4 |
| 这个会话不能读别的会话 | `history/not-here.txt` | `This session cannot read other sessions.` | C-4 |
| 发话，对方没人看着 | `send_message/held.txt` | `Message saved for {to}. Nobody is watching that one-shot session, so it reads this only when someone continues it.` | C-5 |
| 发话，找不到 | `send_message/no-session.txt` | `No session has the id "{to}".` | C-5 |
| 发话，对得上不止一个 | `send_message/ambiguous.txt` | `"{to}" matches more than one session. Use the full id.` | C-5 |
| 发话，是自己 | `send_message/self.txt` | `"{to}" is this session.` | C-5 |
| 这个会话不能发给别的会话 | `send_message/not-here.txt` | `This session cannot message or watch other sessions.` | C-5 |
| 太长 | `send_message/too-long.txt` | `The message has {chars} characters, over the limit of {limit}. Send a shorter one.` | C-5 |
| 限速 | `send_message/too-many.txt` | `Too many messages to {to} just now. Put the rest into one message and send it later.` | C-5 |
| 一模一样的 | `send_message/duplicate.txt` | `{to} already has this exact message.` | C-5 |
| 对方没看的太多 | `send_message/inbox-full.txt` | `{to} has too many unread messages. Send again after it has read them.` | C-5 |
| 订了 | `send_message/watching.txt` | `You will get a notice when {to} is next idle.` | C-6 |
| 订子代理、父会话 | `send_message/watch-peers-only.txt` | `notify_when_idle works only for other sessions.` | C-6 |
| 别的会话发来的话，开头 | `core/peers/message-open.txt` | `<session-message from="{id}">` | C-2 |
| 收尾 | `core/peers/message-close.txt` | `</session-message>` | C-2 |
| 通知，开头 | `core/peers/idle-open.txt` | `<session-idle session="{id}" reason="{reason}">` | C-6 |
| 通知，那一轮没说话 | `core/peers/idle-silent.txt` | `It ended its turn without saying anything.` | C-6 |
| 通知，作废 | `core/peers/idle-expired.txt` | `No notice came within {hours} hours, so the request was dropped.` | C-6 |
| 通知，不在了 | `core/peers/idle-gone.txt` | `The session no longer exists.` | C-6 |
| 通知，收尾 | `core/peers/idle-close.txt` | `</session-idle>` | C-6 |

- 送到了的那一句沿用 `send_message/sent.txt`，`to` 是她写的原样。
- 换进句子的字段照模板的规矩转义（`kernel/request.md`「模板」）：标题、工作目录、她写的编号里的引号、换行写不进这一行。话本身、通知里的那一行不转义，和回报的正文一样。
- 样本（施工时造，门禁逐字节比）：`docs/designs/samples/peers/message.txt`、`idle.txt`、`idle-expired.txt`，一段有别的会话来话、有通知的会话的每一次请求在 `docs/designs/samples/probe/peers/`。

### 出错

工具的出错都标成出错，不报效果。`duplicate.txt`、`held.txt` 不是出错。

| 工具 | 什么时候 | 给她的字 | 说法 |
|---|---|---|---|
| `sessions` | 参数不对 | `common/bad-args.txt` | `common/bad-args` |
| `history` | 找不到 | `history/no-session.txt` | `history/no-session`，字段 `session` |
| `history` | 对得上不止一个 | `history/ambiguous.txt` | `history/ambiguous`，字段 `session` |
| `history` | 这个会话不能读别的会话 | `history/not-here.txt` | `history/not-here` |
| `send_message` | 参数不对（少了 `to`，`message` 和 `notify_when_idle` 都没有） | `common/bad-args.txt` | `common/bad-args` |
| `send_message` | 找不到、对得上不止一个、是自己 | `no-session.txt`、`ambiguous.txt`、`self.txt` | `send_message/no-session`、`ambiguous`、`self`，字段 `to` |
| `send_message` | 这个会话不能发给别的会话 | `not-here.txt` | `send_message/not-here` |
| `send_message` | 太长 | `too-long.txt` | `send_message/too-long`，字段 `chars`、`limit` |
| `send_message` | 限速、对方没看的太多 | `too-many.txt`、`inbox-full.txt` | `send_message/too-many`、`inbox-full`，字段 `to` |
| `send_message` | 订子代理、父会话 | `watch-peers-only.txt` | `send_message/watch-peers-only` |
| `send_message` | 送不到 | `not-sent.txt` | `send_message/not-sent`（原来的） |

内核拒绝的原因码（`kernel/session.md`「出错」），只回给核心里别的会话，不经协议：

| 原因码 | 什么时候 |
|---|---|
| `too_many_messages` | 同一个发话方在窗口里到了上限 |
| `duplicate_message` | 同一个发话方在窗口里发过一字不差的 |
| `inbox_full` | 还没听到的别的会话的话到了上限 |
| `unknown_watch` | 通知来了，这边不在等它 |

运行日志（目标 `miyu::session`，一律英文）：

| 级别 | 行 | 什么时候 |
|---|---|---|
| `INFO` | `message sent to=<短编号>` | 发给别的会话，送到了（C-5，和父子之间同一行） |
| `WARN` | `message not delivered to=<短编号> error=…` | 送不到（C-5） |
| `INFO` | `idle notice sent to=<短编号>` | 被等的这边发了通知（C-6） |
| `WARN` | `idle notice refused to=<短编号> code=…` | 通知被拒、送不到（C-6） |
| `WARN` | `watch not placed session=<短编号> error=…` | 订的时候没订上：对方的会话停了、核心正在停（C-6）。不在了的记 `gone`，不记这一行 |

### 给人看的字

工具的显示名和结果那一句（`resources/software/basesystem/human/{zh,en}.json`）：

| 说法 | 中文 | 英文 | 步 |
|---|---|---|---|
| `sessions` 的显示名 | 列会话，不跟参数，符号 `≡` | Sessions | C-3 |
| `send_message` 的显示名 | 留言，后面跟 `to` 的值，符号 `↗`，和原来的一样。`message_agent`、`send_message` 两个键都在 | Message | C-5 |
| `sessions/listed`（`count`） | 列出 {count} 个会话 | Listed {count} sessions | C-3 |
| `sessions/none` | 没有别的会话 | No other sessions | C-3 |
| `sessions/past-end`（`total`） | 一共 {total} 个别的会话，已经列完了 | All {total} other sessions are listed | C-3 |
| `history/no-session`（`session`） | 没有会话 {session} | No session {session} | C-4 |
| `history/ambiguous`（`session`） | {session} 对得上不止一个会话 | {session} matches more than one session | C-4 |
| `history/not-here` | 这里不能读别的会话 | Cannot read other sessions here | C-4 |
| `send_message/held`（`to`） | 存下了：{to} 是没人看着的一次性会话 | Saved: {to} is a one-shot session nobody is watching | C-5 |
| `send_message/no-session`（`to`） | 没有会话 {to} | No session {to} | C-5 |
| `send_message/ambiguous`（`to`） | {to} 对得上不止一个会话 | {to} matches more than one session | C-5 |
| `send_message/self`（`to`） | {to} 就是这个会话 | {to} is this session | C-5 |
| `send_message/not-here` | 这里不能给别的会话发话 | Cannot message other sessions here | C-5 |
| `send_message/too-long`（`chars`、`limit`） | 太长了：{chars} 个字，最多 {limit} | Too long: {chars} characters, at most {limit} | C-5 |
| `send_message/too-many`（`to`） | 刚给 {to} 发得太多了 | Too many messages to {to} just now | C-5 |
| `send_message/duplicate`（`to`） | {to} 刚收到过一样的话 | {to} already got the same message | C-5 |
| `send_message/inbox-full`（`to`） | {to} 没看的话太多了 | {to} has too many unread messages | C-5 |
| `send_message/watching`（`to`） | {to} 空下来时告诉她 | Will tell her when {to} is idle | C-6 |
| `send_message/watch-peers-only` | 只能等别的会话空下来 | Only other sessions can be watched | C-6 |

头在时间线上显示的：灰色一行预览，点开看全文（「定的」第 2 条）。预览照这几句：

| 什么 | 中文 | 英文 |
|---|---|---|
| 别的会话发来的话 | 来自会话 {id}「{title}」：{第一行} | From session {id} "{title}": {first line} |
| 同上，没标题 | 来自会话 {id}：{第一行} | From session {id}: {first line} |
| 通知，空了 | 会话 {id} 空下来了 | Session {id} is idle |
| 通知，作废 | 等了 {hours} 小时，会话 {id} 没空下来，不等了 | Stopped waiting: session {id} was not idle within {hours} hours |
| 通知，不在了 | 会话 {id} 不在了 | Session {id} is gone |

- 头要标题的，照 `session.list` 取。头认「别的会话」照 `by` 的编号：不是这个会话的父会话（`session.list` 的 `parent`），也不是它派的子代理（推过来的 `job.started`）。

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 | 步 |
|---|---|---|
| `crates/miyu-kernel/src/id/tests.rs` | 短编号：取后 8 位，测试里写死的编号、真造的编号 | C-1 |
| `crates/miyu-kernel/src/event/peer/tests.rs`、`event/effect/tests.rs` | `peer.idle`、`peer.watch` 读写一字不差，不认识的原因原样留着 | C-1 |
| `crates/miyu-kernel/src/ledger/tests/peers.rs` | `peer.watch` 的编号是自己的拒。`peer.idle` 只认在等的、`by` 对得上。撤掉订它的那一轮就不算在等。再订从新时刻算 | C-1、C-6 |
| `crates/miyu-kernel/tests/samples.rs` | 样本读写一字不差 | C-1 |
| `crates/miyu-kernel/src/session/tests/scenario/peers.rs` | 别的会话发来的话：认三种关系。闲着开一轮、正忙下一步听到、最后一步里到的接着开、不带回合编号、打断不撤回不接着开、不作废在等人的题、没人看着只记下、能恢复撤销时记在一边、载入算回来、撤销不带走、重做不了。限速第 6 句拒、窗口过了又收、正好 600 秒那一刻、一字不差的拒且不占数、没听到的第 51 句拒、听到以后又收、重启以后数照日志算回来。旧快照没有 `peers` 照出厂值 | C-2 |
| `crates/miyu-kernel/src/session/tests/scenario/watch.rs` | 空了的通知：在等的记下、叫醒。不在等的拒。作废只在到点以后、只记下不叫醒。`gone` 只记下。「空了」要子代理都报完。`last_line()` 的截法、没说话的 | C-6 |
| `crates/miyu-kernel/src/session/tests/random/` | 随机输入里别的会话的话、通知和回报、撤销交错 | C-2、C-6 |
| `crates/miyu-assemble/src/peers/tests.rs`、`tests/probe_peers.rs` | 两种标签和样本一字不差。排在哪。旧快照照人的话原样、通知不出。请求形状和同一份剧本换成人说的比，只多标签那几段 | C-2、C-6 |
| `crates/miyu-basesystem/src/history/tests/peers.rs` | 「谁」写 `session <短编号>`，筛 `user` 时在里面 | C-2 |
| `crates/miyu-endpoint/src/list/tests.rs`、`tests/list.rs` | 三格新字段：工作目录的认法、最近动静、忙不忙。`session.list` 带着它们。日志坏了的照样列 | C-3 |
| `crates/miyu-basesystem/tests/sessions.rs` | 输出一字不差、第一行是自己、未命名的、分页、过了结尾、没有别的、撞了放长、参数不对、没有端口。每种说法两种语言都换得出字 | C-3 |
| `crates/miyu-session/tests/sessions.rs` | 只列同一个属主的主会话、不列自己、不列子会话、不列删了的。工具面：本机主会话有 `sessions`，子会话、群没有 | C-3 |
| `crates/miyu-basesystem/src/history/tests/other.rs`、`crates/miyu-session/tests/history_other.rs` | 读别的会话：只读、不载入、在跑的也读、时区照自己的。找不到、撞了、子会话和群拒 | C-4 |
| `crates/miyu-basesystem/tests/send_message.rs` | `to` 的认法和先后。长度上限（父子之间也管）。每种拒绝、`held`、`duplicate` 的说法。子会话写会话编号拒。说明里那几句在 | C-5 |
| `crates/miyu-session/tests/messages_peer.rs` | 执行器：命令编号、`by`、没载入的先载入、没人看着的一次性会话交回 `held`、三种拒绝对上三句、运行日志不带话的字 | C-5 |
| `crates/miyu-endpoint/tests/peers.rs` | 真核心两个主会话：A 列出、读 B、发话，B 被叫醒、回话，A 被叫醒。两边互相发到第 6 句断开 | C-5 |
| `crates/miyu-session/tests/watch.rs` | 被等的这边：已经空着当场发、忙完才发、子代理没报完不发、同一个会话只记一个、先交通知再报空闲。等的这边：效果落了盘才订、到点交 `expired`、载入再订、恢复撤销再订、找不到交 `gone` | C-6 |
| `crates/miyu-endpoint/tests/watch.rs` | 真核心：只订不开轮、那边做完这边被叫醒、带话的先送话再等、核心重启以后等的这边载入再订照样等到 | C-6 |
| `crates/miyu-basesystem/tests/budget.rs`、`xtask/src/ledger.rs` | 工具面的预算。新字的指纹和登记簿对得上 | C-3 到 C-6 |
| 真模型实测 | 施工步子 C-7 | C-7 |

### 出处

- `docs/designs/29-跨会话.md`：第一节六条（2026-10-01 项目主人定），第二节防刷屏（主会话照推荐定），第三节留给这一页的技术细节。
- `agents.md` 第六条（父子之间留言）、第九条第 4、5 条（两种标签）、第十一条第 4 条（别的 harness 发来的话照「别处来的」收）、第三条第 3 条（没人看着的一次性会话只记下，2026-09-29 项目主人定）、「是什么」（通讯只在树上相邻的两层，2026-09-29 项目主人定）。
- Claude Code 官方文档 Message your other Claude Code sessions（`ListAgents`、`SendMessage`、`notify_when_idle`，收到的一方怎么对待，排着的上限 50，12 小时作废，只有主对话能订），工具参考里 `ListAgents`、`SendMessage` 两行。
- `06-多用户与身份.md` 第三节（会话只有属主能看）。`04-核心协议.md` 第五、六节。
- `26-提示词.md` 第三节（文风）、J4（引用必须在场）、J12（非必要不加，加了登记）。`10-自带软件.md` 第九节（工具面的预算）。

### 还没有的

- 场所会话（群、私聊）列不列、能不能被发话：随通讯平台。
- 多用户：只列同一个人的，随多用户那一步。现在只有管理员。
- 一次搜所有会话：定了一次只搜一个（「定的」第 5 条），有了会话列表的索引再说。现在每列一次都整份读每个会话的日志。
- 读自己派的子代理的整份日志：`jobs` 只给最近的回答，`history` 的 `session` 只认主会话。
- 列表里分出「在等人回答」：现在算忙。
- 通知里带那一轮是怎么结束的（出错、被打断）。
- 被等的会话删了当场通知：现在等的这边到点作废，或者再订时发现。
- 别的 harness 发来的话照防刷屏：现在只有协议一行的上限。父子之间的留言定了只受长度上限管，不受限速（第五条第 7 款）。
- 以前造的快照的会话收到别的会话的话，照人的话原样显示：快照里没有标签那两份（第八条第 5 款）。第一次发布以前，这种会话只有开发时造的。
- 「别的会话的话不是人的许可」这句说明没加（第八条第 7 款）：C-7 实测撞见她把别的会话的话当成人的意思，再加、量、登记。
- 头怎么显示（视图投影，M8），会话树里画出谁在等谁。
- 自动起标题：施工 3-8 五补做。做了以后多数会话会有标题，列表里「未命名」的少了。
- 别的机器上的会话：不做。一个核心就是一台机器。

### 施工步子

编号先用 C-1、C-2，主会话定正式编号。每一步一个工作树、一张施工单，照施工方案的规矩。

| 步 | 名字 | 做什么 | 先后 |
|---|---|---|---|
| C-1 | 跨会话的事件和编号 | 短编号（`kernel/ids.md`）。事件 `peer.idle`、效果 `peer.watch` 的类型、读写、样本。`by` 是会话的三种关系写进蓝图。账本的规矩（在等哪几个会话、`peer.idle` 只认在等的）。照 7-1 的做法，这一步只有类型和账本 | 第一步 |
| C-2 | 收别的会话发来的话 | 内核认出别的会话，照「别处来的」收。防刷屏前三条，策略数据 `peers.burst`、`window`、`unread`。渲染 `<session-message>`、样本、请求形状探针。`history` 的「谁」。`session.redo` 不能重做它开的那一轮。合了告诉两个头 | C-1 以后 |
| C-3 | 列会话 | 会话表列会话多三格，`session.list` 带上。`SessionsPort`。`sessions` 工具和它的字。只给本机主会话。工具面预算、登记。合了告诉两个头 | C-1 以后，能和 C-2 同时做 |
| C-4 | 读别的会话 | `history` 多 `session`：认编号、只读地开别的会话的日志、子会话和群拒 | C-3 以后（认编号用 C-3 那一份） |
| C-5 | 发给别的会话 | 留言的工具改名 `send_message`（老会话照认 `message_agent`）。`to` 认会话编号，`message` 还是必填（C-6 才改成可以不写）。长度上限（父子之间一起）。每种拒绝的回执、没人看着的一次性会话只存下。子会话拒。说明改一句、量、登记。真核心两个会话来回说 | C-2、C-3 以后 |
| C-6 | 空了告诉我 | `notify_when_idle`。被等的那边 actor 记名单、空了发通知。这边记 `peer.idle`、叫醒。12 小时作废。载入、恢复撤销以后再订。找不到记 `gone`。策略数据 `watch_hours`、`status_chars`。渲染、样本。合了告诉两个头 | C-5 以后 |
| C-7 | 跨会话验收 | 真模型（开发端点、`deepseek-v4.1-flash`），两个主会话 A、B，各起一个标题：在 A 里让她列出会话、认出 B。读 B 说过的一件事。给 B 发一句要它回答的话，B 被叫醒、用 `send_message` 回话，A 被叫醒。A 订「空了告诉我」，B 做一件长一点的活（跑测试），做完 A 被叫醒、通知里带着那一行。子代理里用会话编号被拒。顺带看：两边互相发个没完会停下。她不把 B 的话当成人的许可。两轮请求的缓存命中不掉。防刷屏的数合不合适 | 最后 |

### 起草时定的

技术细节照推荐定了，主会话审过一轮（2026-10-01）。第 4 条和第 24 到 27 条原来是给项目主人的题，主会话照推荐定了（下一节）。

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 列会话新开一件 `sessions`，读并进 `history`（多 `session`），发和订并进 `send_message`（`to` 认会话编号，多 `notify_when_idle`） | 域内聚合、域间分名：翻日志是 `history` 的事，另开一件要把它七个参数再背一遍（估多 150 token）。给谁发话都是留言，Claude Code 的 `SendMessage` 也是一件管子代理、队友、别的会话。列会话是新的一件事，藏进参数里她想不起来（旧版 `kb:` 前缀的教训）。基础系统从 13 件变 14 件，`10-自带软件.md` 的决定跟着改 | 三件全新（估多两三百 token）。列会话并进 `jobs`（`jobs` 管她派出去的，会话不是她派的）。`history` 写 `session: list` 列会话 |
| 2 | 列会话的工具叫 `sessions`（主会话认了，2026-10-01） | 和 `jobs` 一个样子：一个名词，列她手里有的。Miyu 里 agent 指子代理，不叫 `list_agents` | `list_sessions`。照 Claude Code 叫 `list_agents` |
| 2a | 留言的工具在 C-5 改名 `send_message`（2026-10-01 项目主人定），派子代理的另开小单改名 `subagent`（施工 7-5 再补，项目主人定） | 照 Claude Code 的 `SendMessage`。和说明第一句、`to` 同一步改，只冷一次缓存。老会话冻着旧名字，照认成同一件 | 留着 `message_agent` |
| 3 | 短编号取会话编号最后 8 位。撞了放长。认的时候照后缀 | UUIDv7 前 8 位约 65 秒才变，同一分钟开的会话前 8 位一样。最后 32 位是随机的。从编号算得出，不另存 | 前 8 位。照造的先后编 `s1`、`s2`（要另存计数，删了会重编）。编号的哈希 |
| 4 | 没标题的，她看到 `(untitled)`，人看到「未命名」（主会话照推荐定，2026-10-01） | 设计 29 第一节第 1 条。施工 3-8 五补会给会话自动起标题，多数会话会有标题，用不着每行再带一截字 | 带第一句话的开头 |
| 5 | `by` 沿用 `session`，照关系分三种 | `session` 本来就是「另一个会话」。关系在日志里查得到，不加种类，旧核心读得懂 | 新加一种 `peer` |
| 6 | 标签 `<session-message from="短编号">`，不带标题 | 短编号从 `by` 算得出，前缀稳。标题会改，要带就得多一格记发话那一刻的。另用标签名，别的 harness 仿不了 | 借 `<agent-message from=…>`（harness 能冒充）。带标题 |
| 7 | 「空了」= 内核空闲，而且它派的子代理都不欠它回报。后台命令不算 | 和 `miyu ask` 等到的是同一个时刻。只看回合的话，它派了子代理就先报空，她拿到的是「在等子代理」 | 只看回合（Claude Code 的做法） |
| 8 | 订阅：等的这边日志里一条效果，被等的那边只在 actor 内存里记。效果落了盘，执行器照账本新多出来的去订，载入、恢复撤销以后走同一条路 | 单订不花 token，也不进那边的日志。等的这边的日志是唯一的真相，撤销、作废都照它算。落了盘才订，通知不会赶在这边记下在等以前到，不用像 7-6 的回报那样退避着重交 | 两边都记日志（那边多一种事件，第 25 条定了不要） |
| 9 | 通知带那一轮最后回复的第一行，200 字以内 | 照 Claude Code 的一行状态，她多半不用再去读 | 什么都不带。带整段回复（像子代理的回报） |
| 10 | 12 小时作废，等的这边计时、内核照时刻查。作废、不在了只记下不叫醒 | 12 小时是设计 29 定的照 Claude Code。作废不是做出来的结果，照 `undone`、`aborted` 的回报 | 作废也叫醒她 |
| 11 | 防刷屏：同一个发话方 10 分钟最多 5 句。10 分钟内一字不差的不收。没听到的最多 50 句。一句最多 100000 个字。父子之间的留言不受「5 句」管，只受长度上限管（主会话定，2026-10-01） | 50 照 Claude Code。它没公开限速、去重的数，这两个是估的，待 C-7 实测：商量一件事 10 分钟用不了 5 句，转个没完的第 6 句被拒就断了。子代理干活时问得多，不能套这个数。长度照协议一行 1 MiB 留余量 | 照 Claude Code 约 100 万字（超过一行）。只限速不去重 |
| 12 | 限速、去重、排着的上限在收话那边的内核查，长度在发话那边的工具查 | 内核照日志和命令的时刻算，纯逻辑，重启以后算得回来。长度发出去以前就能拒 | 都在执行器查（重启以后数不回来） |
| 13 | 长度上限也管父子之间的留言 | 同一件工具、同一条路。7-7 没设，超过一行的话推给头时整行会被拒 | 只管别的会话 |
| 14 | 只有本机主会话能用，子会话、群都不行 | 照 Claude Code（只有主对话能订）。照「通讯只在树上相邻的两层」。群里的人不可信，不能经她读人的会话 | 子会话也能列、能读 |
| 15 | 只列主会话，子代理挂在各自的主会话下面不单独列，发不到、读不到（主会话定，2026-10-01） | 子代理的事经它的父会话。列表不被子代理刷满 | 列全部，子会话标上父会话 |
| 16 | `send_message`、`history` 的说明主会话、子会话共用一份，说明里不点名 `sessions` | 两份就是两套工具面，探针要两份。子会话多背约 63 token（待量）。不点名，子会话里就不出现不存在的工具（J4） | 子会话一份不提会话的 |
| 17 | 发给没人看着的一次性会话只记下、不开轮，回执说清 | 照「别的 harness 发来的话」和 2026-09-29 项目主人定的回报规矩：没人看着不花钱、不自己动手 | 叫醒它 |
| 18 | 只订不发时 `message` 可以不写。订子代理、父会话的整次拒，留言也不发 | 照 Claude Code | 留言照发、只拒订 |
| 19 | 事件叫 `peer.idle`，效果叫 `peer.watch` | `session.*` 在渲染表里一律不进上下文 | `session.idle` |
| 20 | `session.list` 多 `cwd`、`busy`、`last_active`，和工具同一个函数算，排序不改 | 头的会话列表也用得上，两边的字对得上 | 协议不动，只给工具 |
| 21 | 不加说明，system 里不写「别的会话的话不是人的许可」（主会话认了，记进「还没有的」） | J12。标签已经说了来处，确认只认人亲手给的。C-7 专门看 | 照 Claude Code 加一句 |
| 22 | 发给自己拒 | 没有意义，多半是认错了 | 当成没发 |
| 23 | 旧快照没有标签的会话收到别的会话的话，照人的话原样渲染（主会话认了，记进「还没有的」） | 照施工 7-10。第一次发布以前，旧会话只有开发时造的 | 旧快照的会话拒收 |
| 24 | 收话那边的人看到灰色一行预览：来自哪个会话（短编号、标题）和第一行，点开看全文（主会话照推荐定，2026-10-01） | 和人说的话分得开，不刷屏，照 Claude Code | 整段照人说的话显示、上面标来处。不显示 |
| 25 | 被等的那边不显示「有会话在等你空下来」（同上） | 单订不花 token、不留痕，那边的日志不动 | 显示一行，那边日志多记一条 |
| 26 | `miyu ask` 不等「空了告诉我」的通知（同上） | 可能要几个小时，和后台命令一样。通知记下，`miyu ask -c` 接着说时她看到 | 等到通知来或者 `--timeout` |
| 27 | 一次只搜一个会话（同上） | 设计 29 第一节第 2 条说的是和翻自己的日志一样。现在没有索引，全搜要把每个会话的日志读一遍 | `history` 的 `session` 写 `all` 搜全部 |

### 定的（2026-10-01）

原来是给项目主人的五道题（人会看到、会感觉到差别的），主会话照推荐定了，理由在「起草时定的」第 4、24 到 27 条。

1. 没标题的会话，列表里写「未命名」，不带第一句话的开头。施工 3-8 五补会给会话自动起标题。
2. 别的会话发来的话，收话那边的人看到灰色一行预览，点开看全文。没选：整段照人说的话显示，不显示。
3. 被等的那边不显示「有会话在等你空下来」。没选：显示一行。
4. `miyu ask` 不等「空了告诉我」的通知。没选：等到通知来或者 `--timeout`。
5. 一次只搜一个会话。没选：`history` 的 `session` 写 `all` 搜全部。

### 要跟着改的别的页

这次不改，施工时照步改：

- `tools/message_agent.md` 改名 `tools/send_message.md`：是什么、对外的样子（名字、老会话照认旧名字、说明、`to` 多一种、`message` 不再必填、`notify_when_idle`）、怎么走、样子（新的几句）、出错、给人看的字、守着它的、还没有的（删掉跨会话那条）。别的页里引用 `message_agent` 的照新名字改（`agents.md`、`session/tools.md`、`kernel/…`、`26-提示词.md`、`10-自带软件.md`）。C-5、C-6。
- `tools/history.md`：参数 `session`、怎么走第 2 条（读哪份日志）、「谁」多 `session <短编号>`、出错、给人看的字、守着它的。C-2、C-4。
- `tools/sessions.md`：新页。C-3。
- `tools/interface.md`：`Call` 多 `sessions` 端口，`MessagePort` 的 `Recipient`、`NotSent` 多几种，只订不发时认出会话。C-3 到 C-6。
- `kernel/ids.md`：短编号，`by` 的 `session` 那一行写三种关系。C-1。
- `kernel/events.md`：种类表加 `peer.idle`。C-1。
- `kernel/events-bodies.md`：`message.user` 的说明多别的会话，`peer.idle`，效果 `peer.watch`。C-1。
- `kernel/session.md`：「发一条消息」第 2 条，新两节「别的会话发来的话」「空了的通知」，输入、命令、查询、原因码，守着它的。C-2、C-6。
- `kernel/history.md`：账本在等的通知、`peer.idle` 只认在等的、最近收下的别的会话的话。「拿走什么」「重做」多两种别处来的。C-1、C-2、C-6。
- `kernel/request.md`：渲染表 `message.user` 多一种、`peer.idle` 一行。新两段。`Texts` 多 `peers`。样子的表。C-2、C-6。
- `session/actor.md`：被等的名单、每批以后看空没空、先交通知再报空闲。C-6。
- `session/tools.md`：工具面（`sessions` 只给本机主会话），「父子之间留言」扩成认会话编号，订、计时、再订。C-3、C-5、C-6。
- `protocol.md`：`session.list` 三格，`session.redo` 第 3 条，守着它的。C-2、C-3、C-6。
- `policy.md`：快照多 `peers`，`Texts` 多 `peers`，旧快照照出厂值。C-2、C-6。
- `agents.md`：第六条和「还没有的」指到这一页。C-5。
- `compaction.md` 第三条第 2 条：还没听到的别的会话的话、通知也留在检查点后面。C-2、C-6。
- `cli/ask.md`「等子代理」：写明不等「空了告诉我」的通知（「定的」第 4 条）。C-6。
- `prompts.md`：门禁生成，跟着新字。
- `docs/designs/26-提示词.md` 第十节登记簿（新字各一行、量法那几段），附录（`sessions` 一行，`send_message`、`history` 改）。C-2 到 C-6。
- `docs/designs/10-自带软件.md` 第三节（13 件变 14 件）、第五节（效果 `peer.watch`）、第九节（预算）、第十一节决定。C-3、C-6。
- `docs/designs/03-事件模型.md` 第三节（`peer.idle`）、`08-上下文投影.md` 第三节（两种新的块）。C-1、C-2、C-6。
- `docs/designs/29-跨会话.md` 第三节：改成指到这一页，第四节写做到哪了。批准以后。
- `docs/construction/README.md` 第三节、`施工图.html`：这条线的步子。批准以后。
