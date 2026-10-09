## 会话怎么走

### 是什么

一个会话是一个状态机（`Session`）：送进一条输入，出来一串动作。它不做 I/O，不读时钟：要追加的事件、要回应的命令、要推给头的事件、要跑的挂接点、要发的请求、要跑的工具，都写成动作交给执行器；执行器做完，把结果当成新的输入送回来。事件的时刻取自引起它的那条输入。

这一页写命令、回合、请求、重试、工具、排队、回报、打断、切权限级别、改标题和置顶、换模型、回顾、起标题、删不删得了、重启和载入。执行前的链、确认、提问在 `asking.md`；账本、有效历史、撤销和恢复在 `history.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/session.rs` | `Session`：造会话、分派输入、收命令、造事件、落盘以后推送和回应 |
| `crates/miyu-kernel/src/session/input.rs`、`action.rs` | 输入、命令；动作、结局、原因码 |
| `crates/miyu-kernel/src/session/policy.rs`、`recent.rs` | 冻结在会话上的策略；最近接受的命令编号 |
| `crates/miyu-kernel/src/session/turn.rs`、`call.rs`、`spans.rs`、`retry.rs` | 开回合、发请求、结束回合；收回复、记 `model.called`、回复每一块的起止（施工 2-3 补）；出错再来 |
| `crates/miyu-kernel/src/session/compaction.rs` | 压缩这一步：到没到线、替代到哪、发摘要请求、收回来写 `context.compacted`（`compaction.md`，施工 6-2 上） |
| `crates/miyu-kernel/src/session/manual.rs` | 手动压缩单开的那一轮：收命令、替代到哪、那一轮发摘要请求（`compaction.md` 第七条，施工 6-8） |
| `crates/miyu-kernel/src/session/redo.rs` | 重做：撤最后一轮、重发开它的话、开新的一轮（`history.md`「重做」，施工 4-7 再补） |
| `crates/miyu-kernel/src/session/clear.rs` | 清空上下文单开的那一轮：收命令、上下文是不是本来就空、一批写开头、空的检查点、结束（`compaction.md` 第十四条，施工 6-8 补） |
| `crates/miyu-kernel/src/session/recap.rs` | 回顾：收命令、照落了盘的有效历史组装、交回上一句、并进在路上的、收回报、记 `model.called` 和 `session.recapped`（「回顾」，施工 3-8 四补） |
| `crates/miyu-kernel/src/session/aside.rs` | 辅助请求在路上的那一次（回顾、起标题共用，施工 3-8 五补从 `recap.rs` 分出来）：照用途和名字认回报、收增量、说完了算出正文和那条 `model.called`；不带回合编号的事件怎么造 |
| `crates/miyu-kernel/src/session/sight.rs` | 替它看图：什么时候转述、出 `Describe`、收回来记 `image.described`、这个会话转述过哪些图、把转述放进请求、人这一轮最近说的那一句（「替它看图」，施工 8-17） |
| `crates/miyu-kernel/src/session/title.rs` | 起标题：该不该起（从日志一条条算）、落了盘以后发请求、收回来记 `model.called` 和 `session.meta_changed`、标题怎么截（「起标题」，施工 3-8 五补） |
| `crates/miyu-kernel/src/session/replies.rs` | 命令的回应什么时候回：记下编号、等事件落盘、接受过的编号再来（施工 3-8 四补从 `session.rs` 挪出来） |
| `crates/miyu-kernel/src/session/limits.rs` | 给头看的限额 `ContextLimits`：窗口、压缩线（施工 6-3 补）；在跑的回合和它的 `cause`（`turn_cause()`，施工 8-9） |
| `crates/miyu-kernel/src/session/tools.rs`、`step.rs` | 这一步的调用：先查、派、收结果、补结果；每个调用走到了哪、轮到谁 |
| `crates/miyu-kernel/src/session/queue.rs`、`interrupt.rs` | 排队的消息；打断 |
| `crates/miyu-kernel/src/session/jobs.rs` | 回报到了：记下，开一轮、排着还是只记下（施工 7-2，`agents.md` 第三条）；子会话重交的认出来（施工 7-6） |
| `crates/miyu-kernel/src/session/messages.rs` | 子代理的留言到了：认出是这个会话派的哪个子代理，照回报的规矩记下、开不开一轮；交给执行器的派出去的子代理（施工 7-7，`agents.md` 第六条）；别的 harness 发来的话照同一条路走（施工 7-10） |
| `crates/miyu-kernel/src/session/peers.rs` | 别的会话发来的话过防刷屏：一字不差的、限速、没听到的上限（施工 C-2，`cross-session.md` 第五条），过了的照同一条路走。空了的通知：收、作废、不在了，「空了」、通知那一行、在等哪几个（施工 C-6，`cross-session.md` 第六条） |
| `crates/miyu-kernel/src/session/report.rs` | 向上回报：子会话欠不欠着父会话一份回报、什么时候报、报什么、正文怎么截（施工 7-6，`agents.md` 第二条、第八条） |
| `crates/miyu-kernel/src/session/permission.rs` | 切权限级别、请求之前查事实 |
| `crates/miyu-kernel/src/session/meta.rs` | 改标题、置顶；现在的标题、置顶（施工 3-8 三补） |
| `crates/miyu-kernel/src/session/configure.rs` | 换模型：会话的引用和最近一次换模型写在第几条（`Reference`），收命令，回合开始退回默认的记下（施工 8-10，`models.md`「怎么走」第六条）。8-18 曾在这里加过会话给每个模型记的思考强度，8-18（补）去掉了 |
| `crates/miyu-kernel/src/session/send.rs` | 发一条消息（施工 8-10 从 `session.rs` 挪出来，那边放不下了） |
| `crates/miyu-kernel/src/session/restart.rs`、`load.rs` | 有计划的重启；从日志载入（认出哪次压缩还算数，`history.md`「载入」）、崩了的收尾、重启后接着干 |
| `crates/miyu-kernel/src/accumulate.rs` | 增量拼成回复 |
| `crates/miyu-kernel/src/tool.rs`、`tool/texts.rs` | 访问类别、参数修正；内核替工具写的几句 |
| `crates/miyu-kernel/src/facts.rs` | 环境、权限、会话编号三块事实（权限切了用切换那一份，施工 2-7 补），回复被截断的那一句；有效历史里模块注入过的那几块（`present`，施工 R-4 上） |
| `resources/core/tool-results/`、`resources/core/facts/reply-cut.txt` | 给模型看的字 |
| `resources/core/human/{zh,en}.json` | 内核写的结果给人看的说法 |

### 对外的样子

| 函数 | 做什么 |
|---|---|
| `Session::create(session, id, by, at, created, policy, environment)` | 造会话：追加第 1 条 `session.created`，`cause` 是 `id`，出来一个 `Append`；落了盘回应 `id`。开始时的权限取自 `created.permission`。`session` 是这个会话自己的编号（日志所在的目录就叫它），事实 `session` 写它（施工 1-13 再补） |
| `Session::load(session, events, at, policy, environment)` | 从日志载入会话 `session`，出来会话和要补的动作；载入不了的是 `LoadError`（「载入和崩溃」） |
| `handle(input)` | 送进一条输入，出来一串动作 |
| `last_job_number()` | 日志里用过的任务编号最后一段最大的数（施工 7-5；照最后一段数，施工 7-1 补）：撤掉的回合里派的、不认识的种类也算，一个都没派过的是 0。只读。执行器照它往下领号，派子代理、后台命令共用一串（`session/tools.md`「派子代理」、第 5 条，施工 7-3） |
| `waiting_children()` | 欠着一份回报的子代理的子会话，照任务编号：派出去、一次都还没回报过的（施工 7-6），最近一次回报以后又留过言的（`job.messaged`，施工 7-7）。撤掉的回合里派的也在，被停掉的报过了、不在。只读。执行器载入以后照它叫起子会话（`session/actor.md` 第 2 条） |
| `subagents()` | 这个会话派出去的子代理，照任务编号（施工 7-7）：子会话、被停掉了没有（以 `stopped`、`undone` 报过）。派它的那一轮撤掉了的不在：她看不到派它的调用，也就不是她的；做完了、崩了报过的照样在。只读。执行器派每一次调用之前抄一份交给 `send_message`（`session/tools.md`「父子之间留言」） |
| `watching()`、`watch_hours()` | 在等哪几个会话的通知、各从哪一刻算起，照编号；订了多久作废（施工 C-6，账本的 `watching()`、策略的 `peers.watch_hours`）。只读。等的这一边的执行器每送完一批照它订、计时（`session/tools.md`「订、计时、再订」） |
| `vacant()`、`last_line()` | 「空了」：`idle()`，而且派的子代理都不欠回报（`waiting_children()` 是空的），没收到「要重启了」；最近结束的那一轮最后一条有字的回复的第一行，截到 `peers.status_chars`，超了接 `…`，没说话的没有（施工 C-6，「空了的通知」第 5 条）。只读。被等的那一边的 actor 照它发通知（`session/actor.md`「被等的名单」） |
| `idle()` | 空闲：没有回合在进行，没有结束了、`turn.ended` 还没落盘的回合，没在读回日志、改回文件。核心照它决定能不能空闲退出（后台命令另由执行器的任务表算，`core.md`） |
| `landed()` | 落了盘的最后一条（施工 3-8 六补）：`Stored` 送进来那一刻就推送了，所以也是推过的最后一条；还没落过盘的没有，载入的是日志里最后一条。只读。会话 actor 订阅时照它定补发补到哪一条（`session/actor.md` 第 6 条） |
| `deletable()` | 删得了没有（施工 3-8 三补）：空闲的删得了；正在读回日志、改回文件的是 `Restoring`；别的不空闲（有回合在进行、`turn.ended` 还没落盘）是 `TurnRunning`。只读。会话 actor 照它答应删、停下，挪目录是会话表的事（`protocol.md` 的 `session.delete`） |
| `context_used()` | 这时的上下文用量（施工 8-15）：照有效历史组装这时的请求，用和压缩线同一个算法估（`compaction.md` 第一条）。没交过限额的、策略里没有压缩的没有。只读，不出动作。执行器派 `session_usage` 时向它要一份（`tools/session_usage.md`） |
| `context_limits()` | 给头看的限额 `ContextLimits`（施工 6-3 补）：`window` 上下文窗口，`compaction_line` 压缩线，和内核判到线用的是同一条（`compaction.md` 第二条第 2 条）。没交过限额的、没报窗口的，两格都没有；策略里没有压缩的、算不出正数的，没有压缩线。只读，不出动作。协议照它回 `subscribe`（`protocol.md`） |
| `turn_cause()` | 在跑的回合的编号和它的 `cause`；没有在跑的回合的没有（施工 8-9）。只读，不出动作。会话 actor 推 `model.changed` 时照它写 `turn`、`cause`（`models.md`「瞬时事件」） |
| `reference()` | 会话现在的引用：模型或 `@池`（施工 8-10，「换模型」第 1 条）；没有记下的没有。只读，不出动作。会话 actor 载入时照它造路由（`session/actor.md` 第 8 条第 2 款） |

**输入**（`Input`）：

| 输入 | 带着 | 见 |
|---|---|---|
| `Command(Received)` | `id` 命令编号、`by` 谁发的（取自连接）、`at` 到的时刻、`command` | 「命令和回应」 |
| `Stored { at, upto }` | 落完盘的时刻（执行器的时钟），落了盘的最后一条的序号。时刻是给「接着发请求」时熔断要写的事件用的（施工 6-6 上） | 「命令和回应」第 8 条 |
| `Environment(Environment)` | `offset` 时区、`cwd` 工作目录（头报的、人看到的写法）、`dirs` 加进来的目录（施工 5-10 上） | 换掉会话的环境，什么都不出；下一个边界才用 |
| `Limits(Limits)` | `model` 发给哪个端点的哪个模型、`window` 上下文窗口、`max_output` 最大输出，没报的是 `None`（施工 6-2 上）；`images` 一张图怎么算（`estimate::ImagePrice`，驱动交的，没有的照策略里的固定数，施工 6-3 上）；`blind` 看不了图（施工 8-17，池里有一个成员看不了就算） | 换掉会话的模型限额，什么都不出；只在内存里，载入以后执行器再交一次。没交过的不主动压缩（`compaction.md` 第二条） |
| `TurnStartHooksDone { at, turn, injected, replaced, policy, prepare }` | 哪个回合；各模块的注入 `Injection { module, fact }`，照固定的先后；执行器重新解析时钉着的没了、退回了默认的 `Replaced { from, to }`（原来的、退回的，施工 8-10），没有的是没有；人格的文件改了的新快照的哈希（施工 P-1 再补）；这一轮开不开提前压好（`compaction.prepare`，施工 6-11 上，`compaction.md` 第十五条） | 「回合」第 4 条，「换模型」第 3 条 |
| `RequestSent { at, seen, model, request }` | 哪次请求；发给了哪个端点的哪个模型（`Model { endpoint, model }`）；驱动编码以后的请求字节的哈希 | 「收回复」 |
| `ModelDelta { at, seen, delta }` | 一段增量：`Start { index, kind }`、`Text { index, text }`、`Private { index, private }`、`End { index }` | 「收回复」 |
| `ModelEnded { at, seen, usage, cost, error, wait_ms, excess, failover }` | 用量；金额（施工 8-15：执行器照价格算好的，原样记进 `model.called` 的 `cost`，内核不碰价格）；出错的分类和原话；供应商说要等多少毫秒（换了端点的是别的候选都在冷却时要等多久）；超长的超了多少 token（施工 6-6 中，不进日志）；端口换了端点（`failover`，施工 8-9，不进日志）。没发出去就失败的不报 `RequestSent`，直接报这一条 | 「收回复」「出错再来」 |
| `Woke { at, seen }` | 为哪一次请求等的；等停着的，是那一步回复的序号 | 「出错再来」「打断」第 7 条 |
| `ToolDone { at, call_id, error, blocks, duration_ms, human, effects, stopped }` | 出没出错、给模型看的内容、用时、给人看的说法、效果；叫它停以后停在了改之前的，`stopped` 是真的 | 「调工具」「打断」第 7 条 |
| `ToolProgress { at, call_id, text }` | 一段输出 | 「调工具」 |
| `ToolGuarded { at, call_id, verdict }`、`ToolAsks { at, call_id, questions }` | 链的结论；一组题 | `asking.md` |
| `Restored { at, files }` | 改回文件每一步的结局 | `history.md` |
| `ReadBack { at, from, events }` | 从第 `from` 条读回的日志，连到最后一条（施工 6-9） | 对得上正在读回的那一次，才记撤销（`history.md`「撤掉压缩」）；对不上的不理 |
| `Reread { at, seen, files }` | 哪一次摘要请求；压完要重读的文件，一个一项，照交出去的先后：读到了（`blob`、原文）、太大、读不到（施工 6-5） | 记在那次摘要请求上，什么都不出（`compaction.md` 第九条） |
| `Recalled { texts }` | `Recall` 读出来的原文，照 blob 找（施工 6-5；6-9 起是 `Recall` 的回报） | 放进有效历史，什么都不出（`history.md`「重读的原文」） |
| `Restarting { at }` | 要重启了 | 「有计划的重启」 |
| `JobEnded { at, by, cause, reported }` | 后台命令结束了：`reported` 是 `job.reported` 的 `body`，`by`、`cause` 由执行器照原因填（施工 7-2） | 「回报」 |
| `WatchEnded { at, session, reason }` | 等的会话等不到了：`expired` 到点了、`gone` 不在了（施工 C-6，执行器交） | 「空了的通知」第 3、4 条；读回日志的时候到的先放着 |
| `Described { at, blob, seen }` | 替它看图回来了（施工 8-17）：哪一张图；成了的是替它看的端点和模型、转述的原文（`(Model, String)`），没成的是没有 | 「替它看图」第 4 到 6 条；读回日志的时候到的先放着 |
| `Watched { watched }` | 有没有头订阅着这个会话（施工 7-2 加的输入）：会话 actor 在拿着订阅的头从没有到有、从有到没有时交（施工 7-9，`session/actor.md` 第 3 条） | 只在内存里，什么都不出，不进日志；造会话、载入以后当没人看着（「回报」第 6 条） |
| `AsideSent { at, purpose, upto, model, request }`、`AsideDelta { at, purpose, upto, delta }`、`AsideEnded { at, purpose, upto, usage, cost, error }` | 辅助请求的三种回报（`cost` 施工 8-15）（施工 3-8 四补的回顾；五补起回顾、起标题共用，原来叫 `RecapSent` 这几个）：用途和它照到的那一条合起来是名字；和主请求的三种一样，只是说完了不带要等多久、超了多少。用途不认识的不理 | 「回顾」第 6、7 条，「起标题」第 5、6 条 |

**命令**（`Command`）：

| 命令 | 协议里的方法 | 带着 | 见 |
|---|---|---|---|
| `Send { blocks, urgent, venue }` | `session.send` | 内容块；`urgent` 急着插话；`venue` 通讯平台上的一条消息的那几格（施工 O-13 上） | 「发一条消息」 |
| `Respond { to, facts }` | `session.respond` | 开这一轮（正在跑的并进去，施工 O-14 下）的那几条旁听消息的序号、几块事实（施工 O-14 上） | 「照记下的几条开一轮」 |
| `Append { event }` | `events.append` | 扩展自己的 `ext.*`，或者 `venue.recalled`、`venue.delivered`（施工 O-13 上；别的种类经不了这条路，类型就不收） | 「桥记的事件」 |
| `Interrupt { queued }` | `session.interrupt` | `Queued::Send` 排着的接着发，`Queued::Return` 退回，`Queued::Keep` 留着（施工 O-6） | 「打断」 |
| `SetPermission { level, read_only }` | `session.set_permission_level` | 常用的那一级、只读开关，不改的是 `None` | 「切权限级别」 |
| `SetMeta { title, pinned }` | `session.set_meta` | 新的标题（空的是去掉标题）、置顶，不改的是 `None`（施工 3-8 三补） | 「改标题、置顶」 |
| `Configure { model }` | `session.configure` | 换成的引用：模型或 `@池`（施工 8-10），协议那一头已经查过 | 「换模型」 |
| `Answer { call_id, answer }` | `session.answer` | `Answer::Approval { decision, reason }` 或 `Answer::Questions(回答)` | `asking.md` |
| `Revert { turn }`、`Unrevert` | `session.revert`、`session.unrevert` | 从哪一轮起，`None` 是最后一轮 | `history.md` |
| `Redo { text, attachments }` | `session.redo` | 开这一轮的那一句里的字、附件各换成的块，`None` 是照原来的，两样都没有的原样重发（施工 4-7 再补） | `history.md`「重做」 |
| `Compact { instructions }` | `session.compact` | 人附的要求，`None` 是没附（施工 6-8） | 「手动压缩」 |
| `Clear` | `session.clear` | 没有（施工 6-8 补） | 「清空」 |
| `Report(回报)` | 没有：子会话的执行器经端口交（施工 7-6，`session/actor.md`「向上回报」） | `child.reported` 的 `body`；发命令的一方是子会话（施工 7-2） | 「回报」 |
| `Recap` | `session.recap` | 没有（施工 3-8 四补） | 「回顾」 |
| `Ran { text, command }` | `command.run`（命令本身照它自己的编号先交，这一条用派生的编号 `<编号>/ran`） | 人打的原文、执行了的命令的正名（施工 O-6） | 「斜杠命令」 |
| `PeerIdle { status }` | 没有：被等的会话的执行器经端口交（施工 C-6，`session/actor.md`「被等的名单」） | 它最近结束的那一轮最后一条有字的回复的第一行，没说话的没有；发命令的一方是被等的会话 | 「空了的通知」 |

**动作**（`Action`）：

| 动作 | 带着 | 执行器做什么 |
|---|---|---|
| `Append(事件)` | 这一批事件 | 一次写入、一次同步，送回 `Stored` |
| `Reply { id, outcome }` | 命令编号、结局 | 交给发命令的连接 |
| `Push(事件)` | 落了盘的事件 | 推给订阅了的头 |
| `PushTransient(Transient)` | 一条瞬时事件 | 推给头，不落盘，不等 |
| `RunTurnStartHooks { turn, model, present }` | 回合；会话现在的引用，没有记下的是没有（施工 8-10）；有效历史里由模块注入的每一块（`Present { module, kind, refs }`，照日志的先后，`facts::present`，施工 R-4 上） | 先照这一轮的配置重新解析引用（`session/actor.md` 第 8 条），再叫各模块，模块照 `present` 看这一段上下文里已经有了什么，等齐或超时，送回 `TurnStartHooksDone`，退回了默认的带着 `replaced`；一个模块都没挂也回一次 |
| `RunTurnEndHooks { turn }` | 回合 | 广播，不等，不送回 |
| `CallModel { seen, request, changed }` | 看到第几条（也是这次请求的名字）、统一的请求、和上一次比第一处不同 | 交给驱动发出去；送回 `RequestSent`、`ModelDelta`、`ModelEnded` |
| `CancelModel { seen }` | 哪次请求 | 掐掉，不送回；之后到的不理 |
| `Wake { at, seen }` | 什么时候、为哪次请求 | 到点送回 `Woke` |
| `GuardTool { call_id, name, args, cwd, dirs, permission }` | 修正过的参数、这一轮的工作目录和加进来的目录、实际生效的那一级 | 过执行前的链，送回 `ToolGuarded`（`asking.md`） |
| `RunTool { call_id, name, args, cwd, dirs, permission, cause, asked }` | 修正过的参数、这一轮的工作目录和加进来的目录（施工 5-10 上）、派出去那一刻实际生效的那一级（施工 5-4 上：执行器照它写沙盒的规格）、这一轮的 `cause`（施工 7-3：它起的后台命令自己退出了，`job.reported` 的 `cause` 照它，「回报」第 2 条）、是谁要她做的（施工 O-2 下，`asked.rs`：开回合的触发那一条的 `by`，请求新看到的排队消息、并进来的群消息、别的 harness 和别的会话发来的话里最新的那一条换上它，后台命令、子代理的回报不换；`providers.md`「是谁要的」） | 跑；送回 `ToolProgress`、`ToolAsks`、`ToolDone` |
| `AnswerTool { call_id, answers }` | 人的回答 | 交给在等的调用（`asking.md`） |
| `CancelTool { call_id }` | 哪次调用 | 掐掉，不送回；之后到的不理 |
| `StopTool { call_id }` | 哪次改文件的调用 | 叫它停：停在改之前，或者做完；照常送回 `ToolDone`，停在改之前的带 `stopped`（「打断」第 7 条） |
| `Restore { steps }` | 改回的几步 | 改回文件，送回 `Restored`（`history.md`） |
| `Reread { seen, paths, limit }` | 哪一次摘要请求（排在它的「请求模型」前面）；要重读的文件，真实的位置，照先后；单个最多多少字节（施工 6-5） | 读完、存成 blob 再做下一个动作，送回 `Reread`（`compaction.md` 第九条） |
| `ReadBack { from }` | 从第几条起（施工 6-9） | 只读地读日志，从第 `from` 条到最后一条，读完才收收件箱，送回 `ReadBack`；读不了的，会话停下（`history.md`「撤掉压缩」） |
| `Report(Upward)` | 子会话交给父会话的一份回报（施工 7-6）：报的是哪一轮 `turn`、`reason`（`done` 或 `aborted`）、正文 `text`、`truncated`、`person` | 补上任务编号、子会话，经端口交给父会话，命令编号 `<子会话>/report/<turn>`，不送回（`session/actor.md`「向上回报」） |
| `StopJobs { jobs, by, cause }` | 撤销（重做的撤销那一半）撤掉的那几轮派出去、还在跑的任务，照编号；撤销的人和命令（施工 7-8） | 后台命令整组杀掉、存好输出，`job.reported`（`undone`，`by`、`cause` 照这里的）经收件箱交回（`JobEnded`）；子代理经会话表停下，回报由子会话交来（`Report`，`undone`）；已经结束了的不管。不送回、不等（`history.md`「撤销」第 5 条，`agents.md` 第七条第 1 条） |
| `Recall { blobs }` | 检查点里重读的文件的 blob，照先后（施工 6-9） | 照 blob 读出原文，读完才收收件箱，送回 `Recalled`；读不出来的、不是 UTF-8 的不交（`history.md`「重读的原文」） |
| `Aside { purpose, upto, request }` | 用途（`recap`、`title`、`compaction`：提前压好，施工 6-11 上）、照到第几条（和用途合起来是这一次的名字）、统一的请求（施工 3-8 四补；五补起回顾、起标题共用，原来叫 `Recap`） | 交给这个会话的模型发出去，回报另走一路（和主请求的 `seen` 不撞）；送回 `AsideSent`、`AsideDelta`、`AsideEnded`。不叫停：会话停了，执行器放下它就停了（`session/actor.md` 第 7 条） |
| `Describe { blob, request }` | 哪一张图（也是这一次的名字）、转述的请求（`Assembler::describe`，施工 8-17） | 经一次性入口发给这一轮的 `models.vision`（`session/actor.md` 第 8 条），送回 `Described`。叫不停：之后到的照样送回 |

**结局**（`Outcome`）：`Accepted { events }` 接受，附上它产生的事件的序号，照先后；`Recapped { text, upto, cached }` 回顾好了：那一句、照到第几条、是不是交回的上一句（施工 3-8 四补，「回顾」）；`Rejected { reason }` 拒绝，什么都没产生（回顾没写成的例外：记了一条出错的 `model.called`）。原因码是稳定的英文（`Reason::code`）：

| 原因码 | `Reason` | 什么时候 |
|---|---|---|
| `empty_message` | `EmptyMessage` | 发来的消息一块内容都没有、也没带场所的东西（施工 O-13 补）；重做换过的那一句一块都不剩（施工 4-7 再补） |
| `not_running` | `NotRunning` | 没有回合在进行，打断不了 |
| `unknown_level` | `UnknownLevel` | 要切到的级别不认识 |
| `not_asking` | `NotAsking` | 这个调用不在等这种回答（`asking.md`） |
| `unknown_decision` | `UnknownDecision` | 确认的选项不认识 |
| `no_rule` | `NoRule` | 请求没提放行规则，却选了 `session`、`workspace` |
| `unexpected_reason` | `UnexpectedReason` | 不是拒绝，却带了理由 |
| `bad_answer` | `BadAnswer` | 回答对不上题目 |
| `turn_running` | `TurnRunning` | 有回合在进行时撤销（`history.md`，下同）、重做（施工 4-7 再补）、手动压缩（施工 6-8）、清空（施工 6-8 补）；`deletable()` 说有回合在进行（施工 3-8 三补） |
| `unknown_turn` | `UnknownTurn` | 要撤的那一轮没有，或者已经撤掉了 |
| `nothing_to_unrevert` | `NothingToUnrevert` | 没有能恢复的撤销 |
| `restoring` | `Restoring` | 撤销、恢复还没做完：正在读回更早的日志，或者正在改回文件；`deletable()` 也照它说 |
| `nothing_to_revert` | `NothingToRevert` | 不写回合编号的撤销，一轮都没有 |
| `nothing_to_compact` | `NothingToCompact` | 手动压缩时没有能压的（「手动压缩」，施工 6-8） |
| `nothing_to_clear` | `NothingToClear` | 清空时上下文本来就是空的（「清空」，施工 6-8 补） |
| `unknown_job` | `UnknownJob` | 子会话交来的回报对不上一个还会报的子代理（「回报」第 4 条，施工 7-2） |
| `not_redoable` | `NotRedoable` | 重做时最后一轮不是人说的话开的，或者一轮都没有（`history.md`「重做」，施工 4-7 再补） |
| `nothing_to_recap` | `NothingToRecap` | 回顾时她一个带正文的回复都没有；快照里没有回顾的字（「回顾」第 2 条，施工 3-8 四补） |
| `recap_failed` | `RecapFailed` | 回顾没写成：请求出了错、回复里没有正文（「回顾」第 7 条，施工 3-8 四补） |
| `too_many_messages` | `TooManyMessages` | 别的会话发来的话：这个发话方在窗口里已经记下了够数的几句（「别的会话发来的话」第 2 条，施工 C-2）。只回给核心里别的会话，不经协议给头，下同 |
| `duplicate_message` | `DuplicateMessage` | 别的会话发来的话：这个发话方在窗口里发过一字不差的一句（施工 C-2） |
| `inbox_full` | `InboxFull` | 别的会话发来的话：还没听到的别的会话的话已经够数了（施工 C-2） |
| `not_ambient` | `NotAmbient` | `Respond` 的 `to` 里有不是这个会话里旁听的 `message.user` 的；拒绝的 `about` 是那几条（施工 O-14 上） |
| `already_answered` | `AlreadyAnswered` | `Respond` 的 `to` 里有已经当过触发的（撤掉的回合当过的也算）；拒绝的 `about` 是那几条（施工 O-14 上） |
| `unknown_watch` | `UnknownWatch` | 空了的通知：这边不在等它（没订过、订它的那一轮撤掉了、已经收到过、作废了），或者发命令的不是一个会话（施工 C-6） |

**策略**（`Policy`）：造会话、载入时由执行器照策略快照造好交进来，会话里不再变。

| 格 | 是什么 | 现在交进来的 |
|---|---|---|
| `assembler` | 组装请求的做法（`Assembler`，要能挪到别的线程） | `miyu-assemble` 的默认组装 |
| `facts` | 事实的模板：环境、权限、会话编号、切了级别以后的权限、回复被截断 | `resources/core/facts/` 的五份；以前造的快照没有会话编号那一份的，没有：不注入那一块（施工 1-13 再补）；没有切换那一份的，没有：切了照旧写平常那一份（施工 2-7 补） |
| `tools` | 工具名到 `ToolRule { access, parameters }`：访问类别、参数格式 | 快照的工具面 |
| `step_limit` | 一个回合最多请求几次模型，`None` 不限 | `None`（`miyu-policy` 的 `compose`） |
| `tool_texts` | 内核替工具写的 13 句（`ToolTexts`） | `resources/core/tool-results/` |
| `attended` | 有没有人能确认、回答 | 造会话的那个连接握手时的 `caps.input` |
| `resumes` | 有计划的重启打断了一轮，连着接着干几次 | 3 |
| `reports` | 子会话回报的正文怎么截（`Reports`，施工 7-6）：`chars` 最多几个字，`omitted` 截在中间的那一行（字段 `count`） | 快照里的 `jobs.report_chars`（出厂 30000）、`core/jobs/subagent-omitted.txt`；以前造的快照没有的，照出厂的数、空的那一行 |
| `compaction` | 压缩用的数：`reserve_cap` 输出预留的上限、`margin` 余量、`tail` 尾巴的上限、`price` 估算时一张图、一个文件各算多少；`None` 不主动压 | 20000、13000、16000、各 2000（`miyu-policy` 的 `compose`，施工 6-2）；以前造的快照里没有的是 `None` |
| `titles` | 起标题的两个数（`Titles`，施工 3-8 五补）：`tries` 一个会话最多试几次，`chars` 标题最多几个字；`None` 不起标题 | 快照里的 `title.tries`、`title.chars`（出厂 2、50），快照里还得有起标题的字；以前造的快照里没有的是 `None` |
| `peers` | 别的会话发来的话怎么防刷屏（`Peers`，施工 C-2）：`burst` 同一个发话方一个窗口里最多几句、`window` 窗口多少秒、`unread` 没听到的最多几句 | 快照里的 `peers`（出厂 5、600、50，`cross-session.md`「对外的样子」）；以前造的快照没有的，照出厂的数 |

### 怎么走

**命令和回应**：

1. 每收到一次命令，回应一次：接受，或者拒绝。
2. 拒绝的当场回应，什么都不追加。编号不记：同一个编号再来，重新判。
3. 接受的，等它产生的事件都落了盘才回应。附上的序号：发消息的只有 `message.user` 那一条，子会话交来回报的只有 `child.reported` 那一条（由它开的那一轮不在里面）；打断、回答确认的是这一次追加的全部；回答提问、切权限级别和换模型（施工 8-10）、改标题和置顶的是 `question.answered`、`session.policy_changed`、`session.meta_changed` 那一条；切到和现在一样的级别、改成和现在一样的标题和置顶、换成和现在一样的模型，空的，当场回；撤销、恢复见 `history.md`；重做的是 `turn.reverted`、`files.restored`（有的话）和重发的每一句，新的一轮的开头不在里面（`history.md`「重做」）；手动压缩、清空的是那一轮的 `turn.started`；记下斜杠命令的是 `command.ran` 那一条（施工 O-6）。回顾回的是 `Recapped`，等的是那条 `session.recapped`（「回顾」）。
4. 记着最近接受的 1024 个编号（`recent::CAPACITY`）和它们的序号，满了丢最早的。载入时照日志重建，和没重启时回的一样（施工 2-1 补，2026-10-07 项目主人定统一）：`cause` 是它的那几条，在它开的头一个 `turn.started` 前面截住，头一条就是 `turn.started` 的（手动压缩、清空）只留它。以前载入以后回的是 `cause` 是它的全部，连同它开的那一轮，重启前后不一样。同一个编号再来，不再生效：它的事件都落了盘的，当场照上一次回应；还没有的，等落了盘再回，来几次回几次。回顾不记，也不查：同一个编号再来就是再要一次（施工 3-8 四补，「回顾」第 1 条）。
5. 正在读回日志（撤掉压缩的撤销、重做）、正在改回文件（撤销、恢复、重做以后）的时候，接受过的编号照第 4 条；别的命令一律拒绝，`restoring`。
6. 命令产生的事件：`at` 是命令到的时刻，`by` 是发命令的一方，`cause` 是命令编号。一条输入产生的几条事件，时刻相同。等执行器回来才记的（撤掉压缩的撤销、改回文件的结局），时刻是回来的那一刻（`history.md`）。
7. 造一条事件：序号照账本给；`turn.started` 的 `turn` 是它自己的序号，两种回报一律不带（「回报」第 3 条），别的事件在回合进行中带上这个回合，空闲时没有；先过账本（`history.md`），再进有效历史，等落盘。
8. **落了盘**（`Stored { at, upto }`）：追加过、还没落盘的事件里，序号不超过 `upto` 的算落了盘；一条都没有的，什么都不做。有的，照这个先后出：`Push` 这些事件；`Reply` 事件全落了盘的命令，照收到的先后；`RunTurnEndHooks` `turn.ended` 落了盘的回合；向上回报（「向上回报」第 4 条）；起标题（「起标题」第 1 条，施工 3-8 五补）；然后回合往下走（派工具，或者跑回合开始的挂接点，或者发请求）。

**发一条消息**（`Send`）：

1. 一块内容都没有、也没带场所的东西（`venue.media`）：拒绝，`empty_message`。只有带的东西（图、表情、文件）的场所消息照收，记成没有内容块的 `message.user`（施工 O-13 补）。
2. 发命令的是这个会话派的子代理（`by` 是它的子会话，账本认得出，被停掉的、撤掉的回合里派的也认）：照「子代理的留言」，不走下面几条（施工 7-7）。`by` 是 `harness` 的：照「别的 harness 发来的话」，也不走下面几条（施工 7-10）。`by` 是别的会话的（一个会话，既不是这个会话的父会话，也不是它派的子代理）：照「别的会话发来的话」，也不走下面几条（施工 C-2）。
3. 空闲时：追加 `message.user`，同一批开一个回合（「回合」第 1 条）。这条消息不带回合编号。急着插话的也一样。
4. 回合进行中：追加 `message.user`，带上这个回合，排进队（「排队的消息」）。这一步里在等人的调用作废（`asking.md`「在等的怎么了结」）；急着插话的，再跳过还没跑的（「急着插话」）。都在同一批；叫停的 `CancelTool` 排在 `Append` 后面。
5. 场所里旁听的（`venue.ambient`，施工 O-13 上）：追加 `message.user`，不带回合编号，不开回合，回合进行中也不排进队、不作废在等的、不插话；回合说完了不因它接着开一轮。

**照记下的几条开一轮**（`Respond`，施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：

1. `to` 照序号排好、去重；空的拒 `empty_message`。
2. 有不是旁听的 `message.user` 的拒 `not_ambient`，有当过触发的拒 `already_answered`（账本答，`kernel/history.md`）：拒绝的 `about` 是不合的那几条，先查前一种。
3. 正在跑一轮的并进去（施工 O-14 下）：`facts` 照先后记下，带这个回合，`by` 是发命令的一方；再记 `turn.joined {triggers}`，带这个回合，`by` 是内核，`cause` 是这个命令；照回合中途来的回报排进队（`Waker::Joined`）：下一次请求就有它，这一轮没再请求就结束的接着开一轮、由它触发，打断时不退回、不接着开；这一步里在等人的调用作废（同「回合进行中」的人的话），叫停的 `CancelTool` 排在 `Append` 后面。落了盘回应，附这一批的序号。
4. 闲着的：开一轮（「开回合」），`turn.started` 带 `triggers`（排好的序号），`trigger` 是最后一条，`cause` 是这个命令；内核的事实照旧，`facts` 照先后接在它们后面，`by` 是发命令的一方、带这个回合。落了盘回应，附这一批的序号。不过回合闸（还没有）。
5. 渲染：开的那几条在回合开始的地方，不在它们自己的位置；并进来的在 `turn.joined` 的位置（`kernel/request.md`「群聊近况」）。

**桥记的事件**（`Append`，施工 O-13 上，`docs/blueprint/chat.md` 第七条第 3 条第 2 项）：追加那一条，不带回合编号，任何时候都收，不开回合、不打断。`ext.*` 记成内核不认识的种类（原样留着，投影跳过）；`venue.recalled`、`venue.delivered` 记成认识的，投影这一步先跳过（O-13 下渲染）。

**回合**：回合编号是它 `turn.started` 的序号。回合走到的阶段：

| 阶段 | 在等什么 | 然后 |
|---|---|---|
| `Opening { opened }` | 开头那一批落盘到 `opened` | `RunTurnStartHooks`，到 `Hooking` |
| `Hooking` | 回合开始的挂接点跑完 | `Ready` |
| `Ready` | 追加过的事件都落了盘 | `CallModel`，到 `Asking`。手动压缩那一轮开头就在这里，发的是摘要请求（「手动压缩」） |
| `Looking { waiting }` | 看不了图，这一次请求里的几张图在等转述（施工 8-17） | 都回来了回到 `Ready`：转述落了盘再组装 |
| `Asking(请求)` | 执行器的三种回报。可能是压缩的摘要请求（`compaction.md` 第三条） | `Settling`；摘要请求取到了摘要的，回 `Ready` |
| `Waiting { after }` | 为 `after` 那次请求的 `Woke` | `Ready` |
| `Swapping(摘要)` | 到线换上提前压好的那一份，等执行器重读（施工 6-11 上，`compaction.md` 第十五条第 3 条） | 重读回来了写压缩，回 `Ready`。打断、重启照 `Ready` 收拾，那一份扔掉 |
| `Awaiting(压什么)` | 到线时提前压的那一次还在路上，等它（施工 6-11 下，`compaction.md` 第十五条第 4 条） | 它每来一段正文推一次进度；回来了用得上的照 `Swapping` 换上，用不上的回 `Ready`、落了盘当场压。打断、重启照 `Ready` 收拾，它接着在后台跑 |
| `Settling` | 只在处理一条输入的当中出现，什么输入都不收 | 结束、`Tools`，或者 `Waiting` |
| `Tools(这一步)` | 这一步的调用都有了结果 | `Ready`，或者结束 |

1. **开回合**：追加 `turn.started`（`trigger` 是触发它的那条，`cwd` 是会话现在的环境里的工作目录（施工 4-9 再补三上），`dirs` 是加进来的目录，没有就不写（施工 5-10 上），`by` 是内核，`cause` 是触发它的那条的 `cause`），紧跟着环境、权限、会话编号三块事实里变了的。这时实际生效的权限换成现在的（空闲时放宽的，这时生效）；这一轮的工作目录、加进来的目录取会话现在的环境，这一轮里不变。手动压缩单开的那一轮另见「手动压缩」：没有 `trigger`、不追加事实、不跑挂接点。
2. **事实**：环境一块（`kind` 是 `env`：这一刻到小时、时区、工作目录）、权限一块（`permission`：实际生效的那一级）、会话编号一块（`session`：这个会话自己的编号，快照里没有这份模板的不查，施工 1-13 再补），`by` 是内核，照这个先后。和有效历史里内核记的同一类最近一块逐字节一样的，不追加：编号一个会话里不变，所以只在第一轮、压缩以后、撤掉带着它的那一轮以后追加。权限那一块比的是级别：级别变了、她看到过上一块的，用切换那一份写，带上一块的级别（施工 2-7 补）。写法、比法见 `kernel/request.md`「事实」。
3. 开头那一批落了盘，出 `RunTurnStartHooks`，带着会话现在的引用（施工 8-10）和有效历史里模块注入过的那几块（`present`，施工 R-4 上：和会话编号那一块同一个找法，检查点以前的、撤掉的回合里的不算，内核自己注入的不算），一个回合一次。
4. **挂接点跑完了**：回合对得上、正在等挂接点的才收；别的（打断以后迟到的、第二次来的、别的回合的、空闲时来的）不理。带着 `replaced` 的先照「换模型」第 3 条记一条（施工 8-10）。注入照交回来的先后追加成 `context.injected`，`by` 是各自的模块，`cause` 是回合的；这一轮里切过权限级别的，再查一遍事实（「切权限级别」第 6 条）。
5. **发请求**：到了 `Ready`，追加过的事件都落了盘。拿有效历史组装；看不了图、请求里有还没转述过的图的，先转述，回合停在 `Looking`，转述落了盘再组装（「替它看图」，施工 8-17）；组装出来的请求放进这个会话转述过的图的转述。再问熔断（`session/breaker.rs`，`compaction.md` 第二条第 5 条、第十条，施工 6-6 上）：暂停着、放不下的，记一条没发出去的 `model.called`，结束回合 `error`；压完很快又到线第 3 次的，写 `context.compaction_paused`，回到 `Ready`。用量过了压缩线的，先发摘要请求（`compaction.md` 第二、三条：名字是替代到的 N，不算请求数，也记进「上一次的指纹」），这一次的主请求等压完再组装。没过线的：`seen` 是落了盘的最后一条；算出请求的指纹，和这个会话上一次组装的比出第一处不同（工具面、system，或者第几条消息，从 0 数起，和它的角色；上一次有、这一次少了的，从少了的那一条算），只是接着加的是 `None`。上一次的指纹只在内存里：造会话、载入以后的第一次都是 `None`（`kernel/request.md`「第一处不同」）。不是重试的，这一轮的请求数加一。急着插话的记号、排着队的清单、这一轮排着的回报清掉：听到了。出 `CallModel`。
6. **结束回合**：追加 `turn.ended`，会话空闲；它落了盘才出 `RunTurnEndHooks`。还有排着队的消息、没听到的回报，同一批接着开下一轮；重启、崩了收尾的不开（「排队的消息」）。

| 结束的原因 | 什么时候 | `by` |
|---|---|---|
| `completed` | 回复里没有工具调用；手动压缩写完了 `context.compacted`；清空（同一批） | 内核 |
| `error` | 出了不重试的错，或者重试够了、要等的太久 | 内核 |
| `step_limit` | 这一步的调用齐了，请求数到了上限 | 内核 |
| `interrupted` | 打断 | 打断的人 |
| `restarted` | 有计划的重启 | 内核 |
| `aborted` | 载入时发现上次崩在回合里 | 内核 |

**收回复**：三种回报都带着 `seen`，不是在路上的那一次的，不理。

1. `RequestSent`：记下时刻、模型、请求字节的哈希。报两次的只认第一次。摘要请求的，推一条 `written` 是 0 的 `compaction.progress`（施工 6-3 下）。
2. `ModelDelta`：摘要请求的增量照样交给累积器，不推 `model.delta`，正文块的每一段推一条 `compaction.progress`（`compaction.md` 第三条第 8 条）。还没报发出去就来了增量，按出错算：分类 `bad_stream`，原话「请求还没发出去就来了增量」。交给累积器，对不上的也按 `bad_stream` 算，原话是累积器的报错（「出错」）。出错的照下面第 3 条收拾，再出 `CancelModel`。收下的推一条 `model.delta`（`by` 是那个模型，`cause` 是回合的，`body` 是 `seen`、第几块、这一段）；私有数据收下，不推。第一段增量到的时刻记下。累积器收下的，照流里的块编号记下每一块第一段、最后一段增量到的时刻（时钟往回拨了的，最后一段不往回挪）；收块的 `End` 不算，驱动流完了才一起收块（施工 2-3 补）。
3. `ModelEnded`：
   1. 没发出去、也没带出错的，按出错算：`bad_stream`，「请求还没发出去就说完了」。没发出去的不写回复。
   2. 摘要请求不写回复：正常说完的，收到的拼好交给组装取出摘要，取到了写 `context.compacted`，推 `compaction.done`（`compaction.md` 第三条第 6、11 条），手动压缩的接着结束回合，`completed`（`compaction.md` 第七条第 6 条）；调了工具的、取不出来的，按出错算，`bad_summary`，不再来。别的出错照下面第 5、6 条。
   3. 发出去了的主请求，收到的拼成回复。正常说完：每一块照收到的拼，没收全的工具调用也留下。出错：只留收全了的工具调用（和打断一样），再把工具调用全去掉。一个字都没有的正文块不要；没有字、也没有私有数据的思考块不要。工具调用编号 `call_<这条回复的序号>_<k>`，`k` 从 1 数留下的（累积器见 `kernel/request.md`）。累积器交回的每一块带着它在流里是第几块，出错时去掉工具调用也连着这个编号一起去；每一块的起止照这个编号对上，照落盘的块的先后（施工 2-3 补）。
   4. 拼出来一块都没有的不写回复；正常说完的，按出错算：`empty_reply`，「回复里一个块都没有」。
   5. 有的写成 `message.assistant`：`seen` 是这次请求的，出错的多写 `"interrupted":true`，`by` 是那个模型，`cause` 是回合的。
   6. 接着追加 `model.called`（下表），`by` 是内核，`cause` 是回合的。
   7. 出错的：摘要请求报 `context_too_long` 的，先截掉最老的几组再发（`compaction.md` 第三条第 10 条，施工 6-6 中）；fork 式的摘要回复里调了工具、快照里有隔离式那句 system、还没改走过的，改发隔离式（第三条第 7 条，施工 6-6 下），两样都记在回合上、落了盘再发；主请求报 `context_too_long`、一个字都没收到、这一步还没被动压过的，落了盘先压再重发（被动压缩，`compaction.md` 第六条，施工 6-7）；能再来就等着再来（「出错再来」），不能的结束回合，`error`。收到的半截照样留在日志里。自动压缩的摘要请求这样结束的，是一次失败：最近一次压缩以后写下的数到 3 次，在 `turn.ended` 前面写 `context.compaction_paused`（`compaction.md` 第十条第 4 条）。
   8. 正常说完的：这一步连着出错的次数清零。回复里有工具调用，调工具；没有，结束回合，`completed`。

| `model.called` 的格 | 写什么 |
|---|---|
| `seen` | 这次请求的 `seen` |
| `endpoint`、`model`、`request` | 发出去了的才有：`RequestSent` 报的 |
| `messages` | 统一的请求里有几条消息 |
| `first_difference` | `CallModel` 的 `changed` |
| `usage` | `ModelEnded` 带的；打断的没有 |
| `cost` | `ModelEnded` 带的，原样（施工 8-15）；打断的没有 |
| `first_token_ms` | 发出去到第一段增量；没发出去、一段增量都没来的没有。时钟往回拨了算 0 |
| `duration_ms` | 发出去到说完（或者打断）；没发出去的没有。时钟往回拨了算 0 |
| `blocks` | 写成了回复的：回复里每一块的起止，从发出去算起，时钟往回拨了算 0（第 2 条、第 3 条第 3 款）。没写回复的没有 |
| `result` | `interrupted` 被打断，`error` 出错，别的 `ok` |
| `error` | 出错的分类和原话 |
| `compaction` | 摘要请求的：哪一种压缩（现在有 `auto`、`manual`）；主请求没有 |

**出错再来**：

1. 可以再来的分类：`retryable`、`rate_limited`、`bad_stream`、`empty_reply`，和 `cooling`（端口没发出去就说完的：候选不止一个、全在冷却，施工 8-9，`models.md`「怎么走」第五条第 6 条）。`context_too_long`、`auth`、`content_policy`、`other`、`no_model`（端口没发出去就说完的：没有能用的模型，施工 8-6，`models.md`「怎么走」第一条第 7 条）、不认识的，不再来。端口说换了端点（`failover` 是真，施工 8-9，`models.md`「怎么走」第五条第 3 条）的，不管分类都再来。
2. 这一步已经连着再来的不到 5 次（`RETRY_LIMIT`）才再来：最多再来 5 次，第 6 次出错结束回合。
3. 等多久：带着 `wait_ms` 的照它；没带的，换了端点的等 0 毫秒（当场再来，照样走下面的第 5、6 条：推 `status`、交 `Wake`，施工 8-9），别的第 1 到第 5 次各等 1、2、4、8、16 秒（`backoff`）。要等的超过 120000 毫秒（`WAIT_LIMIT_MS`），不再来，结束回合，`error`。换端点也算一次再来，数进第 2 条的 5 次。
4. 收到了半截、写成了回复的，后面追加一条事实 `reply_cut`（`by` 是内核，`cause` 是回合的），每次都追加，不和以前的比。
5. 回合停在 `Waiting`，出 `Append`、一条瞬时的 `status`（`by` 是内核，`cause` 是回合的，`body` 是 `seen` 和 `retry`：第几次、上限 5、等多少毫秒、分类、原话，出的错带着 HTTP 状态码的也带上，换了端点的多一格 `failover: true`，施工 8-9）、`Wake`（这一刻加上要等的毫秒；超出能写的时刻，就是这一刻）。
6. `Woke`：正在等的就是这个 `seen`，回到 `Ready`；这一轮里切过权限级别的先查一遍事实；然后照「回合」第 5 条再组装一次。别的都不理。什么都没收到、等的时候也没来别的事的，有效历史里只多了 `model.called`，默认的组装不渲染它，再来的请求和上一次一样。
7. 再来的那一次不算进请求数。再来的是摘要请求的，不标「下一次是重试」：它后面那一次主请求照常算一步。
8. 等着的时候打断，这一轮结束；有计划的重启，照重启收拾；来了消息，照排队。

**调工具**：

1. 回复里的调用照先后查，当场记的结果和回复同一批：
   1. 这次请求发出以后来过急着插话的：每个调用都记「已跳过」（`skipped`），`by` 是说话的人，`cause` 是那条消息的命令，不再往下查。
   2. 工具面上没有这个名字：`error`，那一句是 `unknown`。
   3. 参数修正不了：`error`，`not-an-object`。
   4. 实际生效的是只读，工具要写入（访问类别 `write`，或者不认识的）：`denied`，`read-only`。
   5. 别的修正好参数，排进这一步。
   6. 2 到 4 的 `by` 是内核，`cause` 是回合的。都拦下了的，这一步当场齐了（第 7 条）。
2. **修正参数**（`tool::repair`）：去掉空白是空的，当 `{}`。不是 JSON 对象的，修正不了。参数格式读不出来、没有 `properties` 的，原文照交。顺着 `properties`（对象的各格）和 `items`（数组的每一项）往下走（施工 4-9 再补二），每一格 `type` 写成一个字符串、模型给的值也是字符串的，去掉前后空白再还原：`array` 以 `[` 开头、读得成数组的；`object` 以 `{` 开头、读得成对象的；`integer` 读得成 64 位整数的；`number` 读得成有限小数的；`boolean` 是 `true`、`false` 的，大小写都收。还原出来的、本来就是的对象和数组，照它的声明接着往下修。换了一格就把整个对象重写一遍（紧凑的 JSON，键照名字排）；一个都没换，原文照交。`string` 和别的类型一个字节都不碰。修正只用在执行上，日志里的回复照模型给的原文。
3. **回复落了盘才派**。轮到的交给执行前的链（`GuardTool`，`asking.md`），带上这一轮的工作目录、实际生效的那一级。人允许了的，那条决定落了盘才 `RunTool`；人答完了的，那条回答落了盘才 `AnswerTool`。一次出的动作里，先是 `RunTool`、`AnswerTool`，再是 `GuardTool`，各自照调用的先后。
4. **轮到谁**：照调用的先后。只读（`read`）的，前面没有还没结果的非只读调用就轮到；不是只读的，前面的都有了结果才轮到，它没结果，后面的都等。过链的、等人的、允许了还没派的、在跑的、问着人的，都占着位置。
5. **结果**（`ToolDone`）：只收这一步里在跑的调用（派出去了的、问着人的、答完了等落盘的）和停着的（「打断」第 7 条）；别的不理。追加 `tool.result`：`status` 照 `error` 是 `error` 或 `ok`，内容、用时、说法、效果照交的，`by` 是那次调用，`cause` 是回合的。没叫它停却交回停在改之前的（带 `stopped`）：记 `cancelled`，那一句是「已取消，跑到一半」，照内核写的（第 8 条）；执行器只在叫它停以后才这样交，这一条是兜底。然后派后面能派的。
6. **输出**（`ToolProgress`）：只收在跑的调用的，推一条 `tool.progress`，`by` 是那次调用，`cause` 是回合的。
7. **这一步齐了**：请求数到了步数上限，结束回合，`step_limit`；不然回到 `Ready`，这一轮里切过权限级别的先查一遍事实，落了盘请求下一次。上限只在一步齐了时查：第一次请求总会发，上限是 0 和 1 一样。
8. **内核写的结果**：`blocks` 是一块文字（那一句），`human` 是那一句的说法，没有用时、没有效果。

**排队的消息**：

1. 回合进行中来的消息带着这个回合，记在这一轮的队里，下一次请求里就有它；发请求时队清空。回合中途到的、会叫醒她的回报另排一队，一样在发请求时清空（「回报」第 5 条）。
2. 回合结束时队里还有的，同一批接着开下一轮：由最后那一条触发，`cause` 是它的；`at` 是结束上一轮的那条输入的时刻。走完、出错、到上限、打断时接着发，都一样。还没听到的回报也照这一条，和排着的消息比，由后来的那一条触发；打断结束的、没人看着的一次性会话，回报不接着开（施工 7-2）。
3. 打断时退回的：追加 `message.withdrawn`，列出队里的，照先后，`by` 是打断的人，`cause` 是打断的命令，排在 `turn.ended` 前面；队里没有的，不追加。退回以后不接着开。排着的回报不撤回：不是人说的话。留着的（`Queued::Keep`，施工 O-6 的 `/stop`）：队清空，什么都不追加，也不接着开；排着的那几条照样在日志里，下一句话开的那一轮看得到。
4. 有计划的重启、崩了收尾，不接着开（「有计划的重启」「载入和崩溃」）。

**回报**（施工 7-2，`agents.md` 第三条）：

1. 两种：命令 `Report`，子会话交来的回报，记 `child.reported`，`by` 是发命令的子会话，`cause` 是这个命令；输入 `JobEnded`，执行器交来的后台命令结束，记 `job.reported`，`by`、`cause` 照交来的。时刻是到的那一刻。
2. `job.reported` 的 `by` 照原因（2026-09-30 定），执行器照这个填：`exited` 是起它的那次调用（`cause` 是那次调用的）；`stopped` 是停它的人（`cause` 是停它的命令），或者停它的那次 `jobs` 调用；`undone` 是撤销的人（`cause` 是撤销的命令）；`restarted`、`aborted` 是内核。
3. 回报一律不带回合编号（2026-09-30 定）：它不属于哪一轮。带了这一轮的编号，撤这一轮时会跟着被拿走，和「别处来的留着」冲突（`history.md`「拿走什么」）。回合中途到的，照它在日志里的位置和请求看到的范围排（`History::ordered`），下一次请求就在那一步的工具结果后面（`request.md`「回报」）。
4. 先过账本（`history.md`「账本查的规矩」）：对不上的，`Report` 拒绝，`unknown_job`；`JobEnded` 不理；都什么都不记。改回文件、读回日志的时候来的 `Report` 照别的命令拒绝，`restoring`。
5. 记下以后看叫不叫醒她。只记下、不叫醒的：`job.reported` 的 `undone`、`restarted`、`aborted`，和带 `by_model` 的 `stopped`（她自己停的）；`child.reported` 的 `undone`、`aborted`，和带 `by_model` 的 `stopped`（施工 7-4）；派它的那一轮撤掉了的（有效历史的「派出去过的任务」标着撤掉了，`history.md`；这种回报不渲染，开了轮她也看不到）。别的叫醒她，被人停掉的子代理也叫醒（`agents.md` 第三条第 4 条），不认识的原因也叫醒：
   - 她正忙（有回合在进行）：排进这一轮的回报队，下一次请求算听到了（「回合」第 5 条）；回合结束时还没听到的，照「排队的消息」第 2 条接着开下一轮。
   - 她闲着（没有回合在进行，`turn.ended` 没落盘的也算），这时开得了：由它开一轮（`trigger` 是它，`cause` 是它的），和它同一批。
   - 她闲着，这时开不了：记在一边（第 7 条）。
6. 这时开不了：收到过「要重启了」（施工 7-3，「有计划的重启」第 5 条）；还能恢复撤销（账本的 `last_reverted`，`history.md`）；正在改回文件（读回日志的时候回报到不了这一步：`JobEnded` 先放着，第 8 条，`Report` 被拒绝，第 4 条）；没人看着的一次性会话，就是 `session.created` 带 `oneshot`、这时没有头订阅着（`Watched` 交的，造会话、载入以后当没人看着；换成有人看着也不因为以前的开轮，2026-09-29 项目主人定）。
7. 记在一边的：下一轮开始时她一起看到，它们本来就在有效历史里；随便哪一轮开了（手动压缩、清空单开的那一轮也算，清空的跟着清出了上下文）就清掉，不再由它们另开一轮。恢复了撤销（没有要改回的文件的，记下 `turn.unreverted` 时；有的，记下 `files.restored` 时），这时开得了、里面有派它的那一轮还在的，由最后那一条开一轮，和那一条同一批；恢复的回应不附它们。由最后那一条开，和「排队的消息」第 2 条一样（`02-内核.md` 第六节「照排着的接着开」）：触发的那一条挪到回合开始的地方（`request.md`「组装」第 6 条），由最早的开，后来的几条反倒排在它前面（2026-09-30 施工 7-2 定）。
8. 读回日志的时候到的 `JobEnded` 先放着，读回来、记下撤销以后再照上面记：读回的那一段要连到追加过的最后一条（`history.md`「撤掉压缩」）。执行器读完才收收件箱，这是兜底。
9. 载入：记在一边的照日志算回来，是最后一次 `turn.started` 以后、没有回合开着时到的、会叫醒她的（不看派它的那一轮撤没撤，开的时候再看）；载入不因为没听到的回报开轮，恢复了撤销再照第 7 条。
10. 子会话重交的（施工 7-6）：这个子代理最近一次回报就是这个命令编号交来的，接受，照上一次回应（附那一条的序号），什么都不记，编号记进最近的命令编号。子会话载入时再交它最后报的那一份（「向上回报」第 6 条），记着的最近 1024 个编号以外的也认得出：账本给每个子代理记着最近一次回报的命令编号和序号。

**子代理的留言**（施工 7-7，`messages.rs`，`agents.md` 第六条第 3 条）：这个会话派的子代理用 `send_message` 发给父会话的话。父会话发给子代理的留言在子会话那一头就是父会话发来的消息，照「发一条消息」，内核不另认。

1. 认：命令 `Send`，`by` 是这个会话派的一个子代理的子会话（账本照 `job.started` 记着的会话认，被停掉的、撤掉的回合里派的也认）。父会话发来的（子会话收到的交代、留言）照「发一条消息」；别的会话发来的照「别的会话发来的话」（施工 C-2）。
2. 记：`message.user`，`by` 是那个子会话，`cause` 是这个命令，不带回合编号：它是别处来的，撤哪一轮都不拿走它（`history.md`「拿走什么」第 3 条），打断时也撤回不了（账本只认带着回合编号的排队的消息）。落了盘回应，只附这一条的序号。急着插话的记号不看。
3. 叫不叫醒她，照「回报」第 5 到第 9 条，当它是一条会叫醒她的回报：派它的那一轮撤掉了的不叫醒；正忙排进这一轮的回报队，下一次请求听到，回合结束时还没听到的接着开，打断结束的、没人看着的一次性会话不由它接着开；闲着、这时开得了由它开一轮；开不了的记在一边；载入时照日志算回来。
4. 它不是人说的话：不作废这一步里在等人确认、在等人回答的调用（「发一条消息」第 4 条只管人发来的）；打断退回时不撤回（「排队的消息」第 3 条）；也不让子会话欠父会话一份回报（「向上回报」第 1 条）。
5. 发出去的那一头：`send_message` 发给子代理的那次调用报效果 `job.messaged`，账本记下它欠一份回报（`history.md`「账本记下的变化」），「向上回报」第 2 条照它等。

**别的 harness 发来的话**（施工 7-10，`messages.rs`，`agents.md` 第十一条第 4 条）：命令 `Send`，`by` 是 `harness`（`protocol.md` 的 `session.send` 带 `from`）。它和子代理的留言一样是别处来的，照「子代理的留言」第 2 到 4 条走（2026-09-30 主会话定）：

1. 记 `message.user`，`by` 是 `harness`，`cause` 是这个命令，不带回合编号；落了盘回应，只附这一条的序号；急着插话的记号不看。
2. 当它是一条会叫醒她的回报：正忙排进这一轮的回报队，下一次请求听到，回合结束时还没听到的接着开，打断结束的不接着开；闲着、这时开得了由它开一轮（`trigger` 是它，`cause` 是这个命令）；开不了的记在一边（没人看着的一次性会话、还能恢复撤销、正在改回文件、要重启了）；载入时照日志算回来。它不是哪个任务的，没有「派它的那一轮撤掉了」这回事，一律叫醒。
3. 它不是人说的话：不作废在等人确认、在等人回答的调用，打断两种都不撤回，撤销不带走（`history.md`「拿走什么」第 3 条），它开的那一轮重做不了（`history.md`「重做」第 2 条），发到子会话里不让子会话欠父会话一份回报、也不算人插过话（「向上回报」第 1 条）。打断不撤回它：撤回的话头会放回人的输入框，那不是人打的字。

**别的会话发来的话**（施工 C-2，`peers.rs`、`messages.rs`，`cross-session.md` 第四条、第五条）：命令 `Send`，`by` 是一个会话。

1. 认，照账本（`history.md`「最近收下的别的会话的话」）分三种：`by` 是这个会话的父会话（`session.created` 的 `parent`）的，照「发一条消息」，子会话欠一份回报；是这个会话派的子代理的子会话的（被停掉的、撤掉的回合里派的也认），照「子代理的留言」；别的，是别的会话发来的话，照下面几条。
2. 先过防刷屏，照账本算，时刻用这条命令的 `at`，含正好一个窗口以前的那一刻；过不了的拒绝，什么都不记：
   1. 这个发话方在过去 `peers.window` 秒里记下过内容块一字不差（照日志里的写法写成 JSON，比 SHA-256）的一句：`duplicate_message`。它先查，不占限速的数。
   2. 这个发话方在过去 `peers.window` 秒里记下了 `peers.burst` 句：`too_many_messages`。
   3. 还没听到的别的会话的话（不分发话方）已经有 `peers.unread` 句：`inbox_full`。「听到」是主对话的请求（不是回顾这类辅助请求、不是压缩的摘要请求、不是暂停着没发出去的那一条）或者回复看到了它。
3. 过了的照「别的 harness 发来的话」第 1 到 3 条收：记 `message.user`，`by` 是那个会话，`cause` 是这个命令，不带回合编号；落了盘回应，只附这一条的序号；急着插话的记号不看；当它是一条会叫醒她的回报，一律叫醒；不作废在等人的题，打断两种都不撤回，撤销不带走，它开的那一轮重做不了；发到子会话里不欠父会话回报。
4. 里面的斜杠命令不执行：它就是一块字。
5. 载入：数照日志算回来（账本），重启以后不会清零；记在一边的照「回报」第 9 条算回来。
6. 父子之间的留言不受第 2 条管：子代理干活时问得多。别的 harness 发来的话也不受（`cross-session.md`「还没有的」）。

**空了的通知**（施工 C-6，`peers.rs`，`cross-session.md` 第六条）：她订了别的会话「空了告诉我」（工具结果的效果 `peer.watch`，账本记着在等哪几个，`history.md`「在等的通知」），那个会话空下来时交来命令 `PeerIdle`，等不到了执行器交 `WatchEnded`。

1. 收：命令 `PeerIdle`，`by` 是一个会话，账本说这时在等它：记 `peer.idle`（`session` 是它，`reason` 是 `idle`，`status` 照交来的），`by` 是它，`cause` 是这个命令，不带回合编号；落了盘回应，只附这一条的序号。不在等的、`by` 不是会话的拒绝 `unknown_watch`，什么都不记。同一个编号再来照上一次回应。
2. 叫醒：当它是一条会叫醒她的回报，照「别的 harness 发来的话」第 2 条：正忙排进这一轮的回报队，闲着、这时开得了由它开一轮（`trigger` 是它），开不了的记在一边，载入时照日志算回来。它不是人说的话：不作废在等人的题、打断不撤回、撤销不带走（`history.md`「拿走什么」第 3 条），它开的那一轮重做不了（`history.md`「重做」第 2 条）。
3. 作废：输入 `WatchEnded`（`expired`），账本说还在等、`at` 不早于起算时刻加 `peers.watch_hours` 小时（含正好那一刻）的，记 `peer.idle`（`reason` 是 `expired`），`by` 是内核，`cause` 是订它的那一轮的（账本记着订的那条结果的 `cause`），不带回合编号，只记下、不叫醒。不在等的、还没到点的不理：又订过一次的，旧的计时到了不算。
4. 不在了：输入 `WatchEnded`（`gone`），账本说还在等的，照第 3 条记（`reason` 是 `gone`），不看时刻。别的原因不理。读回日志的时候到的先放着，读回来再记（照后台命令结束）。
5. 「空了」（`vacant()`）：`idle()`，而且派的子代理都不欠回报，也没收到「要重启了」（那一轮再起来接着干）；后台命令不算。通知那一行（`last_line()`）：「向上回报」第 3 条记的最近结束的那一轮最后说的话，整段去掉前后空白取第一行，再去掉这一行的前后空白，超过 `peers.status_chars` 个字的截到那么多个字、接 `…`；一个字都没说的没有。被等的那一边照这两样发通知，内核不知道谁在等它（名单在 actor 的内存里，`session/actor.md`「被等的名单」）。
6. 压缩、清空、撤销认「上一轮」：`peer.idle` 和回报一样算（还没听到的留在检查点后面，`compaction.md` 第三条第 2 条；清空看上下文空不空时算一条；不带回合编号的触发，`history.md`「拿走什么」）。

**向上回报**（施工 7-6，`report.rs`，`agents.md` 第二条、第八条）：子会话（`session.created` 带 `parent`）的一轮结束时报给父会话。主会话什么都不欠。

1. 欠不欠，从日志一条条算：每追加一条记一次，载入时照日志再走一遍，算出来的一样。
   - 欠下：`by` 是父会话的 `message.user` 进了日志：派它的交代、父会话的留言（7-7），开了一轮的、排进正在进行的那一轮的都算。`by` 是人的、是它自己的子代理的留言（7-7），它自己的子代理的回报，不欠：人开的那一轮人就在旁边看着，下一层的事是它和下一层之间的。
   - 人插过话：欠着的这几轮里有 `by` 是人的 `message.user`（开这一轮的、排进来的），一轮结束时并进欠着的那一份，报的时候 `person` 写 `true`。人开的一轮里进了父会话的留言，那一轮就欠下了，也注明。
2. 该报了：欠着，没有回合开着，它自己派出去的子代理都不欠它回报（账本的 `waiting_children` 是空的：都至少报过一次，被停掉、崩了的也算报过；留了言的，留言以后又报过，施工 7-7），最近结束的那一轮不是 `interrupted`（停在半路，等人说话或者被停）、`restarted`（再起来接着干）。
3. 报什么（`Upward`）：`turn` 是最近结束的那一轮；正文是那一轮最后一条有字的回复里的正文块，接起来，思考不要；都是空白的不算说了话，那一轮一个字都没说的是空的。超过 `reports.chars` 个字的，留头尾各一半（单数的尾巴多一个），中间接 `\n` 和截在中间的那一行（`count` 是省掉的字数），`truncated`。那一轮是 `aborted` 结束的（崩了，或者第 6 条没接着干的），`reason` 是 `aborted`；别的都是 `done`（出错、到了步数上限的也是）。报了就不再欠。
4. 什么时候交：「落了盘」里，回合结束的挂接点后面：追加过的事件都落了盘、该报了，出 `Report`。一轮结束和接着开的下一轮在同一批里的，那时回合开着，不报；孙代理的回报叫醒它开的那几轮都结束了才报。闲着时到了一条不叫醒它的回报（崩了、撤销停掉的孙代理），落了盘就报，报最近结束的那一轮的话。收到过「要重启了」的不交（「有计划的重启」第 5 条）：要关了，交过去会把正在停的父会话重新载入；再载入时照日志算出该报、照第 6 条交。
5. 回报送出去不送回：执行器经端口交给父会话，父会话落了盘就算送到；父会话没了的丢掉（`session/actor.md`「向上回报」）。
6. 载入：照日志走一遍时，每一条之前（接着开的一轮的 `turn.started` 除外）看该不该报，该报的当它报过了，记下最后报的那一份；走完再看一次。交回的动作里，`Recall` 后面是最后报的那一份（再交一次：送到一半崩了的不漏，父会话照命令编号认出重的，「回报」第 10 条）。崩在那一轮里的，补的 `turn.ended`（`aborted`）落了盘照第 4 条报 `aborted`。最后结束的那一轮是被有计划的重启打断的、这次没接着干（接够了 `resumes`、撤掉了），当它崩了，当场照 `aborted` 报：它再也不会走完，父会话不会一直等。

**打断**（`Interrupt`）：

1. 没有回合在进行：拒绝，`not_running`。
2. 请求在路上：收到的半截只留收全了的工具调用，发出去了、不是空的才写成回复（`"interrupted":true`）；`model.called` 的 `result` 是 `interrupted`。回复里留下的调用各补「已取消，没跑过」（`cancelled-before`）。出 `CancelModel`。半截回复和 `model.called` 的 `cause` 是回合的。
3. 调工具：还没有结果的：
   1. 派出去了的、改文件的（访问类别 `write`）：叫它停（`StopTool`），先不记结果，改成「停着」（第 7 条）。
   2. 派出去了的别的、答完了等落盘的：补「已取消，跑到一半」（`cancelled-running`），出 `CancelTool`。
   3. 问着人的：补 `question-interrupted`，出 `CancelTool`。
   4. 还没派的（等着的、在过链的、等人确认的、允许了还没派的）：补「已取消，没跑过」。
4. 别的阶段：什么都没发出去，直接结束。之后迟到的挂接点结果、`Woke` 不理。开头还没落盘就被打断的，回合开始的挂接点不再跑，回合结束的照样跑。
5. 收尾（没有停着的就当场收尾，有的等第 7 条）：`queued` 是退回的，先撤回排着的（「排队的消息」第 3 条）。然后 `turn.ended`（`interrupted`）；接着发的，队里有的接着开下一轮。排着的回报不撤回，也不由它接着开：人刚叫停，留到下一轮（「回报」第 5 条）。
6. 补的结果、撤回、`turn.ended`，`by` 是打断的人，`cause` 是打断的命令。回应附上这一次追加的全部，接着开的下一轮的也在里面；有停着的，回应在第一批落了盘就回，之后追加的 `cause` 也是打断的命令。叫停的动作排在 `Append` 后面。
7. **停着的**：这一轮先不结束，等它们交回来，最多 10 秒（出 `Wake`，到点送回 `Woke`）：
   1. 交回来停在改之前的（`stopped`）：补「已取消，跑到一半」，`by` 是打断的人，`cause` 是打断的命令。
   2. 交回来已经改完的：照工具交的记，和平常的结果一样（`ok`、`error`，带效果），`by` 是那次调用。
   3. 都交回来了，收尾（第 5 条）。
   4. 到了 10 秒，或者又打断了一次：还没交回来的补「已取消，跑到一半」，出 `CancelTool`，收尾；之后才到的不理。
   5. 等着的时候来的消息照排队；撤销、恢复照「有回合在进行」拒绝。

**手动压缩**（`Compact`，施工 6-8，`compaction.md` 第七条）：

1. 有回合在进行：拒绝，`turn_running`。
2. 照 `compaction.md` 第七条第 2 条定出 N；N 以前没有能压的（策略里没有压缩、没交过限额的也算）：拒绝，`nothing_to_compact`。
3. 要求去掉前后空白是空的，当没附。
4. 追加 `turn.started`：没有 `trigger`，`cwd`、`dirs` 照会话现在的环境，`by` 是内核，`cause` 是这个命令。不追加事实，不换实际生效的权限，回合直接到 `Ready`，不跑回合开始的挂接点。回合上记着 N 和要求。它落了盘就回应，附上它的序号。
5. 到了 `Ready`：不问熔断（暂停着也压），照这时的有效历史组装一次算压之前的用量，发摘要请求（名字是 N，`compaction` 是 `manual`），出 `CallModel`（前面照样可能有 `Reread`）。出错再来的，到点回到 `Ready` 照同一个 N、同样的要求再发。
6. 取到了摘要：`context.compacted`（`trigger` 是 `manual`，`instructions` 是要求，不写 `refills`）、`turn.ended`（`completed`）同一批，推 `compaction.done`；不查事实。排着的同一批接着开下一轮。
7. 失败结束的，不数进熔断（`compaction.md` 第十条第 3 条）；打断、重启照平常的规矩收拾，不写 `context.compacted`。
8. 这一轮进行中照常：来的消息排队，急着插话的也只是排着（摘要请求不截断，回来就结束了），撤销、恢复、手动压缩、清空照「有回合在进行」拒绝。

**清空**（`Clear`，施工 6-8 补，`compaction.md` 第十四条）：

1. 有回合在进行：拒绝，`turn_running`。
2. 上下文本来就是空的（有效历史里没有检查点，或者最近的是清空的，它后面也没有人的消息、回复、工具结果、回报）：拒绝，`nothing_to_clear`。
3. 同一批追加三条，`by` 都是内核，`cause` 都是这个命令：`turn.started`（没有 `trigger`，`cwd`、`dirs` 照会话现在的环境）、`context.compacted`（`trigger` 是 `clear`，`upto` 是这时追加过的最后一条，`summary` 是空的，别的格没有）、`turn.ended`（`completed`）。不追加事实，不换实际生效的权限，不跑回合开始的挂接点，不请求模型；记在一边的回报清掉（「回报」第 7 条）。
4. 都落了盘才回应，附上 `turn.started` 的序号；`turn.ended` 落了盘照常出 `RunTurnEndHooks`。
5. 回合只在收这条命令的当中开着，不会被打断、重启打断；载入时和别的走完了的回合一样。

**斜杠命令**（`Ran`，施工 O-6，`protocol.md` 的 `command.run`）：

1. 命令由协议端点认、判谁能用、执行（`/clear` 交 `Clear`，`/stop` 交 `Interrupt { queued: Keep }` 再叫执行器停掉后台的），执行了以后再交 `Ran` 记一条 `command.ran`；被拒的端点不交，什么都不记。
2. 什么时候来都收：记一条，`by` 是说命令的人，`cause` 是这个命令，回合进行中的带上这个回合；落了盘回应，附上这一条。同一个编号再来不再记。
3. 它不进她的上下文：渲染时不出，也不改「接着写的记号」（`kernel/request.md`「组装」）。

**回顾**（`Recap`，施工 3-8 四补，`04-核心协议.md` 第九节 `session.recap`，`kernel/request.md`「回顾的请求」）：

1. 什么时候来都收，有回合在进行时也收；只有正在读回日志、改回文件时照「命令和回应」第 5 条拒。不记编号：回顾只是要一句话，同一个编号再来就是再要一次，没有新内容的照样交回上一句。
2. 照这一刻落了盘的有效历史（`History::until(落了盘的最后一条)`）交组装器组装回顾的请求（`Assembler::recap`），它同时交回照到的那一条：喂进去的最新那一条消息。组装不出来的：拒绝，`nothing_to_recap`。她一个带正文的回复都没有的组装不出来；快照里没有回顾的字的（以前造的，只在开发时的旧会话里有）也组装不出来，照 `nothing_to_compact` 的先例不另开原因码。
3. 有效历史里最近一条 `session.recapped` 照到的就是这一条：交回它，`cached` 是真，不请求；它落了盘才回（多半早落了）。只看最近那一条：撤掉后来的几轮，照到的回到从前，最近那条照到的不是它，照样再请求一次。照到的是喂进去的最新那一条消息，所以改标题、工具结果、`model.called` 这些都不算新内容。
4. 有一次在路上的：并进去，等它说完一起回，不另请求。一次只有一个在路上。
5. 不然出 `Aside { purpose: recap, upto, request }`，名字是照到的那一条。不碰「上一次请求」：主请求照旧和上一次主请求比第一处不同；不算步数，不推增量、进度。
6. 执行器的三种回报带着用途和 `upto`，不是在路上的那一次的不理（照用途分给回顾、起标题，`aside.rs`）。发出去了报两次的只认第一次。还没报发出去就来了增量、增量对不上的，记下错，后面的增量不收，等说完了照出错收：不叫停，叫停以后还到的回报会串进下一次回顾。
7. 说完了：正文块连起来、去掉前后空白，思考、工具调用不要。追加一条 `model.called`：`seen` 是照到的那一条，`purpose` 是 `recap`，`messages` 是请求的条数，端点、模型、请求字节的哈希、用量、`first_token_ms`、`duration_ms` 照常，没有 `first_difference`、`blocks`、`compaction`；`by` 是内核，`cause` 是等着的第一个命令，不带回合编号。
   - 写成了（没出错，正文不是空的）：后面紧跟着 `session.recapped`（`text`、`upto`；`by`、`cause`、`turn` 同上），同一批；等着的都回 `Recapped { text, upto, cached: false }`，这一条落了盘才回。
   - 没写成（执行器报了错；还没报发出去就说完了、增量对不上，`bad_stream`；回复里没有正文，`empty_reply`）：只追加那一条 `model.called`；等着的都拒绝，`recap_failed`，那一条落了盘才回（先见结果，后见回应）。不再来：头要再要一次就是。
8. 有计划的重启、会话停了：在路上的那一次跟着丢掉，等着的命令收不到内核的回应（执行器放下了它们，协议上是会话停了）。载入以后没有在路上的。
9. 回顾不进她的上下文：两条都不带回合编号，撤哪一轮都不会跟着拿走；渲染时不出，也不改别的怎么排（`kernel/request.md`「组装」）；账本不把它当成听到了排着的话（`history.md`）；用量的锚、压缩的边界不看它（`compaction.md` 第一条、第三条第 2 条）。

**起标题**（施工 3-8 五补，`title.rs`，`kernel/request.md`「起标题的请求」）：照 Claude Code，人手动起名以外，没起名的会话核心自己起一个短标题（2026-10-01 项目主人定）。

1. **什么时候起**：「落了盘」里，一轮的 `turn.ended` 落了盘，这一轮有她带正文的回复（最近一条正文去掉空白不是空的回复在这一轮），并且：
   - 标题从没动过：日志里没有带 `title` 的 `session.meta_changed`，人改的、去掉的、内核起的都算动过。只置顶的不算。
   - 不是子会话（`session.created` 带 `parent`）：派它时的标题就是它的名字。
   - 不是一次性的会话（`oneshot`，`miyu ask` 开的）：多半是脚本在跑，每一轮多一次请求不划算（2026-10-01 项目主人定）。
   - 试过的次数不到策略里的 `titles.tries`（出厂 2）：从日志数，`purpose` 是 `title` 的 `model.called` 有几条，载入时照样数回来。
   - 没有一次在路上；没收到过「要重启了」；策略里有 `titles`。
2. 照这一刻落了盘的有效历史交组装器组装（`Assembler::title`），它同时交回照到的那一条：第一个回答的序号。组装不出来的（快照里没有起标题的字、有效历史里没有带正文的回答）不起，也不算试过。
3. 出 `Aside { purpose: title, upto, request }`，名字是照到的那一条。和回顾一样不碰「上一次请求」，不算步数，不推增量；一次只有一个在路上，没有命令等着它。
4. 在路上的跟着有计划的重启、会话停了丢掉，不算试过（日志里没有它的 `model.called`），下一轮答完再试。
5. 执行器的三种回报照回顾的规矩收（「回顾」第 6 条）。
6. **说完了**：正文块连起来、去掉前后空白，思考、工具调用不要。追加一条 `model.called`：`seen` 是照到的那一条，`purpose` 是 `title`，别的格照回顾的（「回顾」第 7 条）；`by` 是内核，没有 `cause`（没有谁要它），不带回合编号。
   - 起成了（没出错，正文不是空的）、标题这时还没动过：同一批接着追加 `session.meta_changed`，只写 `title`，`by` 是内核，没有 `cause`、回合编号。标题取正文的第一行，去掉前后空白；超过 `titles.chars` 个字（Unicode 字符，出厂 50）的截到那么多个、再去掉末尾的空白，不加省略号（2026-10-01 项目主人定）。会话现在的标题跟着换（「改标题、置顶」第 1 条），推给订阅着的头，`session.list` 里带上它。
   - 在路上时人改了名（标题动过了）：只记那一条 `model.called`，不盖掉人起的。
   - 没起成（执行器报了错；还没报发出去就说完了、增量对不上，`bad_stream`；回复里没有正文，`empty_reply`，原话 `the title reply has no text`）：只记那一条 `model.called`，算试过一次，不马上再来，下一轮答完再试；两次都没起成就不再试。每一次执行器记一行 `title failed`（`session/actor.md` 第 7 条），第二行就是不再试的那一行。
7. 起标题不进她的上下文：两条都不带回合编号，撤哪一轮都不会跟着拿走；渲染时不出，也不挡接着写（`kernel/request.md`「组装」「接着写」）；账本、用量的锚、压缩的边界照带 `purpose` 的规矩不看它。

**替它看图**（施工 8-17，`sight.rs`，`models.md`「怎么走」第十三条，`kernel/request.md`「替它看的图」）：主对话的模型看不了图时，`models.vision` 替它看，转述记成 `image.described`，以后每次请求照它把图换成字。

1. **什么时候**：「发请求」那一步组装完，限额说看不了图（`Limits.blind`；没交过限额的不算）。请求里 user、tool 消息的每一张不同的图（照 blob）：转述过的、正在转的、这一轮转述没成的跳过，剩下的每一张出一个 `Describe`。这一次请求要的图还有在路上的（连同别的回合发出去、还没回来的），回合停在 `Looking`，等它们都回来；一张都不用等的照常往下走。快照里没有转述的字的（`Assembler::describe` 交回没有）不转述。主请求报了超长、先压的那一次（被动压缩）不查：那一次请求已经发过。
2. **转述的请求**：组装器照这张图和人这一轮最近说的那一句组装（`Assembler::describe`）。那一句由内核找：这一轮开头的触发那一条起（没有触发的从 `turn.started` 起），有效历史里最后一条字不空的 `message.user`，字块照先后接起来、去掉前后空白，谁发的都算。
3. **在路上的**：照 blob 记着发它的那一轮的 `cause`，只在内存里；载入、重启以后没有。
4. **回来了**（`Described`）：不是在路上的那一张的不理。读回日志的时候到的先放着，读完再收。
   - 成了、这张图还没转述过：记一条 `image.described`（`blob`、端点、模型、转述），`by` 是内核，不带回合编号，`cause` 是发它的那一轮的。
   - 没成：不记；有回合开着的，记在这一轮上，这一轮里不再试，下一轮再试。
5. **回到「准备好」**：回合在 `Looking`、等的图都回来了，回到 `Ready`；这一轮里切过级别的，这时查一遍事实（「切权限级别」第 6 条）。追加的都落了盘才重新组装、发请求：转述先落盘、再进请求。
6. **打断、重启**：`Looking` 时打断的，照 `Ready` 那样直接结束这一轮，没有要叫停的（一次性入口叫不停）；之后才回来的照第 4 条收，成了的照样记。有计划的重启照 `Ready` 那样收尾。
7. **转述过哪些**：日志里每一条 `image.described` 都算，活着时每追加一条记一次，载入时照日志再走一遍；同一张图记了两条的用先记的。撤销、压缩都不删：撤掉的回合里、压缩以前转述过的图，再出现照样用。
8. **进请求**：组装出来的主请求、摘要请求，把里面出现的每一张图的转述放进 `described`（`kernel/request.md`「替它看的图」）；驱动照这一次端点看不看得了图挑原图还是转述（`drivers/openai-chat.md` 第 9 条）。
9. `image.described` 不进她的上下文、不渲染，也不挡接着写（`kernel/request.md`「组装」第 7 条）；账本不另查（`kernel/events-bodies.md`）。

**待办**（施工 D-3，`todos.rs`、`history/todos.rs`，`tools/todowrite.md`）：

1. 当前的清单是有效历史里最近一份没撤掉的 `todo.written`（`todowrite` 那次调用的效果）。有效历史另记一张表：每写一次记一项、带着写它的那一轮，压缩不丢（压缩换掉了那一条也在），撤销、恢复照 `turn.reverted`、`turn.unreverted` 改「撤掉了没有」，和派出去过的任务一个办法（`kernel/history.md`）。载入时照整份日志重算。
2. 交给头：`Session::todos()` 是当前的整份（没写过、清空了、写过的都撤掉了的是空的），执行器造会话、载入时放进给头看的那一份，`subscribe` 照它回 `todos`（`protocol.md`）。
3. 变了才推：每次落了盘、推完事件以后（「落了盘」），比当前的和上次告诉头的，不一样的推一条瞬时的 `todos.changed`、记下这一份；还没落盘的事件里有换待办、撤销、恢复的，等它们落了盘再比（先落盘，后推送）。造会话时告诉过头的是空的；载入时当已经告诉过当前的那一份。执行器收到它，给头看的那一份跟着换。推的是空的、而且最近写的那一份就是它（没撤掉）、是因为全部做完而清空的，带上 `done`：`todo.written` 的 `done`，头让人看到最后一项打勾再收起（施工 D-3 补，2026-10-07 网页转来）；撤销退回的、恢复回来以前更早的、她写空的不带。
4. 压缩时，当前的不空的写进检查点里代码写的几段（`compaction.md` 第八条第 4 条）。
5. 不进她的上下文：她只在工具结果、检查点里看到；不每轮提醒（2026-10-07 项目主人定）。

**急着插话**（`Send` 带 `urgent`，回合进行中）：

1. 在回合上记下谁说的、哪个命令，到下一次请求发出为止。又来一句急着插话的，换成后来的那一句。
2. 正在调工具的：还没跑过的（等着的、在过链的、等人确认的、允许了还没派的）各补「已跳过」（`skipped`），`by` 是说话的人，`cause` 是这条消息的命令；在跑的照常跑完。这一步因此齐了的，往下走。
3. 请求在路上的：不截断，回复到了，里面的调用全部跳过（「调工具」第 1 条）。

**切权限级别**（`SetPermission`）：

1. 级别不认识：拒绝，`unknown_level`。
2. 照现在的合出新的：没写的那一格照旧。和现在一样的：接受，什么都不记，编号照样记下。
3. 不一样的：追加 `session.policy_changed`，`permission` 两格都写，`by` 是切的人，回合进行中的带上这个回合。
4. 宽窄：只读 0，工作区 1，完全放开 2，不认识的级别按 0 算。新的比实际生效的窄，当场生效；收紧成只读的，这一步里还没跑过、要写入的调用（工具要写入的，或者请人确认的是写入的）当场补 `denied`（`read-only`），`by` 是内核，`cause` 是回合的，这一步因此齐了的往下走。已经在跑的不动。收紧成工作区的，什么都不拦。没被拦下的调用，已经交给链的，链照交出去时的那一级判（结论回来时内核照现在的只读再查，`asking.md`）；还没交的，轮到时照新的那一级过链。
5. 不比实际生效的窄的（放宽的，或者一样宽的，例如只读开着时改常用的那一级），实际生效的那一级等下一次查事实才换。
6. **查事实**：回合进行中切过的，下一次请求之前查一遍，就是挂接点跑完、这一步齐了、重试到点的时候；切的时候正在 `Ready` 的，当场查。查的时候实际生效的换成现在的；环境那一块写这一轮的工作目录和会话现在的时区，会话编号那一块照旧写会话自己的；和最近一块一样的不追加，来回切了一圈的也就不追加。回合之间切的，下一个回合开始时查。权限那一块级别变了、有效历史里有上一块的，用切换那一份写，`previous` 是上一块的级别，她看得出是人切的、从哪一级切过来（施工 2-7 补，`kernel/request.md`「事实」第 2 条）。
7. **撤销不改现在的权限**：撤掉的回合里切的照样算（「载入和崩溃」第 2 条照整份日志算；活着时撤销也不动它）。撤掉的只是她看到过的那几块，下一个边界照有效历史里还剩的最近一块比（施工 2-7 补核对过）。`history` 因此也列撤掉的回合里切的（`tools/history.md`）。

**改标题、置顶**（`SetMeta`，施工 3-8 三补，`meta.rs`）：

1. 现在的标题、置顶从日志算：`session.meta_changed` 一条条盖上去（人改的、内核起的都算，施工 3-8 五补），写了的格换成它的，标题写空的是去掉。造会话时没有标题、没置顶。撤掉的回合里改的也算：改名不是对话的一部分。载入时照样一路算回来（「载入和崩溃」第 2 条）。
2. 和现在一样的格去掉：标题照原样比（去掉空白、量长短是协议端点的事，`protocol.md` 的 `session.set_meta`），空的和没有标题一样。一格都不剩的：接受，什么都不记，编号照样记下。
3. 还剩的：追加一条 `session.meta_changed`，只写还剩的那几格，`by` 是改的人，回合进行中的带上这个回合，空闲时没有。什么时候来都收，改回文件的时候照「命令和回应」第 5 条拒。
4. 不影响回合、不影响请求：标题、置顶不进有效历史，渲染时不看它（`kernel/request.md`）。

**换工作区**（`SetWorkspace`，施工 9-7 上，`workspace.rs`）：

1. 工作区是会话的属性：会话的环境（工作目录、加进来的目录）只有这个命令换。执行器报环境的那一条输入（`Input::Environment`）照旧收，施工 9-7 上起执行器不再送（`Handle::environment` 没人调了，2026-10-08 项目主人定删了，施工「收尾 10-08」删掉）。
2. 和现在一样的（工作目录一样、`dirs` 没写或一样）：接受，什么都不记，编号照样记下。
3. 不一样的：环境当场换（写了 `dirs` 的整份换掉），追加一条 `session.workspace_changed`（`dirs` 没变的不写），`by` 是换的人，回合进行中的带上这个回合；落了盘才回应，附上它的序号。这一轮里照旧用开轮时取的那一份，下一轮开始照新的，变了的事实照旧只注入变了的。
4. 判太不太宽、存不存在是协议那一头的事（`protocol.md` 的 `session.set_workspace`），内核照原样记。

**换模型**（`Configure`，施工 8-10，`configure.rs`，`models.md`「怎么走」第六条）：

1. 会话的引用从日志算：`session.created` 的 `model`，被后来带 `model` 的 `session.policy_changed` 盖掉，最后那个就是。撤掉的回合里换的也算：换模型不是对话的一部分，照「改标题、置顶」第 1 条。以前的日志没有 `model` 的，没有引用：跟着 `models.chat`（执行器那一头记在内存里）。载入时照样一路算回来（「载入和崩溃」第 2 条）。
2. 和现在的引用一样的：接受，什么都不记，编号照样记下。不一样的：追加一条 `session.policy_changed`，只写 `model`，`by` 是换的人，回合进行中的带上这个回合，空闲时没有；落了盘才回应，附上它的序号。什么时候来都收，改回文件的时候照「命令和回应」第 5 条拒。内核只存字，不解读。
3. **回合开始**：`RunTurnStartHooks` 带着现在的引用，执行器照这一轮的配置重新解析。交回的 `replaced`（钉着的没了，退回了这一轮的 `models.chat`）：`from` 正是现在的引用、`to` 和它不一样的，在注入前面追加一条 `session.policy_changed`，`model` 是 `to`，`replaced` 是 `from`，`by` 是内核，`cause` 是回合的，带着回合；对不上的不理：交回来之前人又换了，人换的算数，下一轮再解析（`models.md`「施工时定的」8-10）。
4. 回合进行中换的，这一轮照开始时的，下一轮开始才交出去。手动压缩、清空单开的那一轮不跑挂接点，也就不交、不重新解析（`compaction.md`）。
5. 换了模型，自动压缩的暂停解除：写在最近一条带 `model` 的 `session.policy_changed` 前面的暂停、失败不再算（`compaction.md` 第十条第 6 条，`breaker.rs`）。
6. 施工 8-18 曾在这里加过「思考强度」一条：会话给每个模型记一格，从日志算。8-18（补）去掉了：思考强度改在配置里（`models.md`「怎么走」第十一条），内核不再记这一格，以前的日志里带 `effort` 的照读得进（不认识的字段不管），不拼进来。

**换策略快照**（施工 P-1 再补，`policy.rs`，`02-内核.md` K3，`personas.md`「怎么走」第 6 条）：人格的文件改了，下一个回合换上；预设的文件改了也是（施工 P-2 下，`presets.md`「改了文件」），包的工具变了也是（施工 O-2 中，`providers.md`「目录换代」），几样一起改了的换一次。

1. 执行器在回合开始的挂接点里看人格、预设的文件改了没有、工具目录换没换代（`miyu-session` 的 `actor/persona.rs`）；拼出来和原来一样的什么都不交；程序升级过（核心的字变了）、人格预设都没改、工具面也没变的照旧不换，不为升级断一次缓存（施工 P-1 三补）；换的时候执行器取自快照的几份字（驱动的占位、替工具写的两句、权限策略的几句，`lettering.rs`、`ModelPort::retext`）一起换成新快照的，升级过的老会话也换得了；改了、换得了的，照新的拼好快照、存成 blob，造好策略先交给内核放着（`Session::stage_policy`），再交 `TurnStartHooksDone`，带上新快照的哈希 `policy`。
2. 内核收到带 `policy` 的、又有放着的那一份：换模型那一条（有的话）后面追加一条 `session.policy_changed`，只写 `policy`，`by` 是内核，`cause` 是回合的，带着回合；换上新策略。缺一样的什么都不记，放着的扔掉。回合对不上的、迟到的照第 3 条不理，放着的留到下一次交回时扔掉。
3. 回合开头的事实是照旧策略查的：换上以后照新的再查一次角色扮演提示（`FactTemplates::reminder`），到了轮数的跟着记，排在换快照那一条后面：旧人格没有提示、新的有，这一轮就带上。环境、权限、编号的模板换不换都一样（执行器只在核心的字没变时才换），不重查。
4. 这一轮第一次请求就照新的拼：system、示范对话变了，前缀在这里断一次（K3）。估用量的锚照「换模型」作废：倒着找锚时先碰到带 `policy` 的这一条，就当没有锚，整份估到下一次说完（`estimate.rs`）。
5. 撤掉那一轮不换回去：换快照不是对话的一部分，照「换模型」第 1 条。有效历史里没了这一条，执行器手里的快照照旧是新的。
6. 载入时内核不看它：策略由执行器照整份日志里最近一条带 `policy` 的 `session.policy_changed` 取快照造好交进来，没有的照 `session.created` 的（`session/actor.md`）。

**有计划的重启**（`Restarting`）：

1. 没有回合在进行，什么都不做。
2. 在等停着的（「打断」第 7 条）：那次打断照样算数，先照第 7 条第 4 款收尾，不等了；不然这一轮以 `restarted` 结束，再起来会接着干。收尾时接着开了下一轮的，再照下面收拾那一轮；没开的，到这里为止。
3. 照打断收拾：请求在路上的截下半截（「打断」第 2 条），出 `CancelModel`；在跑的、问着人的、答完了等落盘的出 `CancelTool`，不等（停着的已经照第 2 条掐掉了）。还没有结果的调用，包括半截回复里留下的，都补 `cancelled`，那一句是 `restarted`。
4. `turn.ended` 的原因是 `restarted`。`by` 都是内核，`cause` 是回合的。排着的不接着开。
5. 收到它以后要关了（施工 7-3）：之后到的回报只记下、不开轮。执行器停下之前交来的后台命令结束（它照 `agents.md` 第八条第 2 条记的 `restarted`，和正好在这时自己退出的）照常记下，再起来时接着干的那一轮、或者下一轮开始时她一起看到。只在内存里。

**载入和崩溃**（`Session::load`）：

1. 日志整份一条条过账本：一条都没有，`LoadError::Empty`；有一条过不了，`LoadError::Broken`，写明第几条、违反了哪一条。再照 `history.md`「载入」从还算数的最近一次压缩起重建有效历史（施工 6-9）；那个检查点重读过文件的，交回的动作里第一个是 `Recall`。读进来的都算落了盘。
2. 现在的权限：`session.created` 的，被后来带 `permission` 的 `session.policy_changed` 盖掉，照整份日志算，撤掉的回合里切的也算；实际生效的就是它。现在的标题、置顶照「改标题、置顶」第 1 条算（施工 3-8 三补）；会话的引用、最近一次换模型照「换模型」第 1 条算（施工 8-10）；起标题的账（动过没有、试过几次、子会话）照「起标题」第 1 条一路算回来，没有在路上的（施工 3-8 五补）。会话编号取交进来的 `session`：执行器照会话的目录交，和造会话时交的是同一个，所以载入以后编号那一块和最近一块一样，不重发（施工 1-13 再补）。
3. 最近的命令编号：每个 `cause` 和 `cause` 是它的那几条，照编号第一次出现的先后记进去，多过 1024 个的留后面的。同一个编号再来，回应附上这些，和当时的不一定一样。没产生事件的（切成和当时一样的级别）不在里面，再来重新判。
4. **崩了**：日志停在一个没结束的回合里。还没有结果的调用照编号的先后各补 `cancelled`（`restarted`），再追加 `turn.ended`（`aborted`）；`by` 是内核，`cause` 是这一轮 `turn.started` 的，时刻是载入的 `at`。在等的确认、题跟着了结。不接着开，等人开口；排着的留在日志里，没听到的回报也是（「回报」第 9 条）。
5. **接着干**：最后结束的一轮是 `restarted`、它有 `trigger` 的（没有的是手动压缩那一轮，再起来不接着压，和崩了一样等人开口，施工 6-8），就自动开一轮：由那时排着的最后一条触发，`cause` 是它的；没有排着的，由那条 `turn.ended` 触发，`cause` 是它的。
6. 连着被重启打断的轮数超过 `resumes`（3）的，不接：接着干开的那一轮再被打断，接着数；别的回合一开，从 0 数。最后结束的那一轮以后撤销过的（`turn.reverted`），不接。
7. 补的、开的事件在返回的一个 `Append` 里，排在 `Recall` 后面；没有要补的，不出 `Append`。
8. 改回文件做到一半停了的（日志里有撤销、恢复，没有 `files.restored`），不补、不重做。
9. **没结束的后台命令**（施工 7-3，`agents.md` 第八条第 1 条，不变量 8）：有 `job.started`（`command`）、还没有结束记录的，进程跟着崩了的核心没了，照编号各补一条 `job.reported`：`aborted`，`by` 是内核，没有 `cause`、回合编号，时刻是载入的 `at`，不写用时、输出（什么时候没的不知道）。只记下、不开轮（「回报」第 5 条）。排在第 4 条收尾的前面、`Recall` 后面，和它们在同一个 `Append` 里。子代理不补：子会话照会话的规矩自己收尾、回报（第 10 条）。有计划地停下的，执行器已经记了 `restarted`，这里没有要补的。
10. 子会话：最后报过的那一份再交一次，崩了、重启以后没接着干的那一轮照 `aborted` 报（「向上回报」第 6 条，施工 7-6）。

### 样子：给模型看的

内核写的结果，原文在 `resources/core/tool-results/<名字>.txt`，末尾带一个换行，照抄进结果；字段照模板的规矩转义（`08-上下文投影.md` 第五节）。造策略时十三句各读成模板，拿各自的字段（`unknown`、`not-an-object` 是 `name`，`denied-with-reason` 是 `reason`，别的没有）试换一次：写坏了、要了别的字段的，造不出来（`TemplateError`）。token 数、指纹在 `26-提示词.md` 第十节的登记簿里。确认、提问的六句在 `asking.md`。同一个目录里的 `unavailable`、`crashed` 两句是执行器写的，不是内核（`kernel/tools.md`）。

| 名字 | 什么时候 | 状态 | 原文 |
|---|---|---|---|
| `unknown` | 工具面上没有这个名字 | `error` | `There is no tool named "{name}".` |
| `not-an-object` | 参数不是 JSON 对象 | `error` | `The arguments for "{name}" are not a JSON object.` |
| `read-only` | 只读时拦下要写入的 | `denied` | `The call was not run: the session is read-only.` |
| `cancelled-before` | 打断时还没跑过的 | `cancelled` | `The call was cancelled before it ran: the user interrupted the turn.` |
| `cancelled-running` | 打断时跑到一半的 | `cancelled` | `The call was cancelled while it was running: the user interrupted the turn. It may have been partly done.` |
| `skipped` | 急着插话，还没跑的；等人确认时来了一句话 | `skipped` | `The call was skipped: the user sent a new message.` |
| `restarted` | 有计划的重启、崩了收尾时没有结果的 | `cancelled` | `The call was cancelled: Miyu restarted before it finished. It may have been partly done.` |

回复被截断以后再来的那一块事实（`resources/core/facts/reply-cut.txt`，`kind` 是 `reply_cut`）：

```text
<reply-cut>The reply above was cut off before it was finished. The user has already seen it. Continue from exactly where it stopped, without repeating it.</reply-cut>
```

### 出错

| 什么时候 | 怎么说 |
|---|---|
| 内核自己造的事件过不了账本 | 停下（panic）：「the kernel's own event failed the ledger, a kernel bug: <账本的报错>」 |
| 载入：日志是空的 | `LoadError::Empty`：「the log has no events」 |
| 载入：有一条过不了账本 | `LoadError::Broken`：「the log is broken: event <n> cannot be appended: <违反了哪一条>」（`history.md`） |

请求出错时内核自己写的原话，记进 `model.called` 的 `error.message`，给查问题的人看，不进请求：

| 分类 | 原话 |
|---|---|
| `bad_stream` | 请求还没发出去就来了增量；请求还没发出去就说完了 |
| `bad_stream` | 模型的增量对不上，第 <n> 块：这一块已经开始过了、跳过了编号，块要一块接一块地开始、这一块还没开始、这一块已经收全了、正文块没有私有数据、私有数据来了两次 |
| `empty_reply` | 回复里一个块都没有；the recap reply has no text（回顾的回复里没有正文，施工 3-8 四补） |

### 给人看的字

内核写的结果带着说法 `core/tool-results/<名字>`，没有字段的不带字段，`unknown`、`not-an-object` 带 `name`。字在 `resources/core/human/{zh,en}.json` 的 `said` 里，编号去掉 `core/`：

| 名字 | 中文 | 英文 |
|---|---|---|
| `unknown` | 没有这件工具 | no such tool |
| `not-an-object` | 参数不是一个 JSON 对象 | the arguments are not a JSON object |
| `read-only` | 只读，没写 | read-only, not written |
| `cancelled-before` | 打断了，没跑 | interrupted before it ran |
| `cancelled-running` | 打断了，跑到一半 | interrupted while running |
| `skipped` | 跳过了 | skipped |
| `restarted` | Miyu 重启了，没跑完 | Miyu restarted before it finished |

原因码给人看的那句话由核心照头的语言配（`crates/miyu-endpoint/src/refusal.rs`，`protocol.md`）。`unknown_level` 还没配专门的话，照「被拒绝了。」「Refused.」说（确认、提问的五个见 `asking.md`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/session/tests.rs` | 造会话落了盘才回应；消息落了盘才回应；空消息；同一个编号落盘前后再来；拒绝过的重新判；落盘到一半只回应落全了的；落盘超出追加过的；只记最近 1024 个 |
| `crates/miyu-kernel/src/session/tests/limits.rs` | 给头看的限额（施工 6-3 补）：没交过的两格都没有；压缩线照窗口、最大输出算（最大输出比预留的上限大、小、没有）；窗口太小没有压缩线；策略里没有压缩的只有窗口；再交一次照新的 |
| `crates/miyu-kernel/src/session/tests/idle.rs` | 有回合、`turn.ended` 没落盘都不算空闲（改回文件时不算空闲在 `session/tests/restore.rs`，`history.md`） |
| `crates/miyu-kernel/src/session/tests/turn.rs` | 空闲时消息和回合的开头同一批；挂接点等开头落盘；挂接点跑完才请求；注入照交回的先后；中途的消息并进这一轮；挂接点跑的时候来的消息，落了盘才请求；对不上的挂接点结果不理；报的最后一个环境才注入 |
| `crates/miyu-kernel/src/session/tests/dirs.rs` | 加进来的目录（施工 5-10 上）：`turn.started` 带着它、没有就不写这一格；判权限、派工具都带上；回合中途报来的下一轮才用 |
| `crates/miyu-kernel/src/session/tests/reply.rs` | 一整轮；`model.called` 的每一格；下一轮只注入变了的；不再来的错结束回合、留半截；出错的半截里收全的调用也不留；等一会儿再来；没发出去的没有端点和用时；执行器违约按出错算；过时的回报不理；私有数据留在回复里不推；有工具调用的回合不结束 |
| `crates/miyu-kernel/src/session/tests/difference.rs` | 只是接着加的没有第一处不同；改了 system 的是第一处不同 |
| `crates/miyu-kernel/src/session/tests/tools.rs` | 一步跑完再请求；非只读的一个一个来；没有的工具、坏参数当场回；修正只用在执行上；步数上限在最后一步跑完后结束；推工具的输出；对不上的结果不理；回合带着开始时的工作目录 |
| `crates/miyu-kernel/src/session/tests/redo.rs`、`scenario/redo.rs` | 重做（施工 4-7 再补，`history.md`「守着它的」） |
| `crates/miyu-kernel/src/session/tests/recap.rs`、`scenario/recap.rs`（施工 3-8 四补） | 回顾：单独一次请求，两条不带回合编号、不开回合，回应那一句（去掉前后空白、思考不要），`model.called` 带 `purpose`、`seen` 是照到的、没有第一处不同和块的起止；没有新内容的交回上一句、不请求，说了新的一句再请求；一个回复都没有（说了、没回复的也算）拒绝；出错、只有思考、只有工具调用的没写成、不再来；在路上又要的一起回、只请求一次；照落了盘的算、那一句落了盘才回；对不上的回报不理、发出去了报两次认第一次；还没发出去就来增量、没发出去就说完、增量对不上的没写成；同一个编号再来就是再要一次；有回合在进行时照收，这一轮照常、之后的主请求照旧接着上一次主请求往后长；排着的那句照样撤得回；只记下的那句回顾照到了、手动压缩照样留着它；撤掉后来的一轮照撤完的再请求，前两句留在日志里 |
| `crates/miyu-kernel/src/session/tests/title.rs`、`scenario/title.rs`（施工 3-8 五补） | 起标题：`turn.ended` 落了盘才起、照落了盘的有效历史组装、回合结束的挂接点照跑；第一轮答完起一次，两条都是内核记的、不带回合编号和 `cause`，标题取第一行，`model.called` 带 `purpose`、`seen` 是第一个回答、没有第一处不同和块的起止，起过的不再起，会话上的标题跟着换；超过 50 个字截到 50、去掉末尾空白、不加省略号，按字数不按字节；人起过名、去掉过的不起，只置顶的照起；子会话、一次性的会话不起；没答出正文的那一轮不起，下一轮答了照第一轮的算；出错、只有思考的算没起成，两次就不再起；试过几次载入以后照日志数，在路上丢了的不算；在路上时人改了名的只记请求、不盖掉；一次只有一个在路上；撤掉那一轮标题照样在；要重启了不起；名字、用途对不上的回报不理，说完了再报一次不理，没有命令等着它 |
| `crates/miyu-kernel/src/session/tests/clear.rs`、`scenario/clear.rs` | 清空（施工 6-8 补）：一批三条、不请求模型、不跑回合开始的挂接点、跑回合结束的，落了盘才回应；有回合在进行、本来就空（没说过话、刚清过）的拒绝，清完又说了一句的收；只有一份摘要、只有一条回报也清；记在一边的回报跟着清掉；同一个编号再来；下一轮只看到清空以后的、事实照常注入；撤掉回到清空以前、恢复又清空；暂停着也收、清完到线照常自动压；载入以后一样 |
| `crates/miyu-kernel/src/session/tests/compact.rs`、`scenario/manual.rs` | 手动压缩（施工 6-8）：空闲时收、开的那一轮没有触发、不注入、不跑挂接点，落了盘才回应；有回合在进行时拒绝（改回文件时拒绝在 `restore.rs`）；没有能压的四种；摘要请求带着要求；成了同一批结束、排着的接着开；失败不数不暂停；重试同一个 N；打断、重启；暂停着也收、成了以后到线照常自动压 |
| `crates/miyu-kernel/src/session/tests/scenario/reports.rs`、`scenario/reports_undo.rs`（施工 7-2） | 回报：闲着时开一轮、`trigger`、`cause` 是它，回应只附它；`by`、`cause` 照交来的，不带回合编号；正忙时下一步听到、不另开；最后一步里到的接着开；和排着的消息比由后来的开；只记下的六种不开、下一轮开始时在请求里；被人停掉的照样开；打断时不撤回、不接着开，接着发的由排着的消息开；没人看着的一次性会话只记下，有头订阅着照常开、头退了回合结束也不开；对不上的拒绝、不理；能恢复撤销时只记下，恢复以后由最后那条开、回应不附它；派它的那一轮撤掉了的不开，恢复了跟着回来；人说了下一句一起听到；载入不开轮、载入以后照样能由恢复开、载入以后当没人看着，回合里到的、开过一轮的载入以后不算记在一边的；派它的那一轮还撤着的，恢复了后来那一轮也不开；手动压缩那一轮也清掉记在一边的；派它的那一条压缩掉了、撤掉压缩读回重建以后照常开；恢复要改回文件的，改完了才由它开；由回报接着开的一轮撤掉，带走上一轮排着的话、回报留着；读回日志时到的后台命令结束，读回来记了撤销再记 |
| `crates/miyu-kernel/src/session/tests/scenario/models.rs`（施工 8-6、8-9、8-10） | 端口当场说完的 `no_model`：这一轮以出错结束，不再来，不推重试的状态，照这个名字写进日志；端口说换了端点的（施工 8-9）不管分类当场再来（等 0 毫秒）、照它说的等、数进 5 次，状态带 `failover`；`cooling` 照它说的等最早恢复的、没说的照退避、超过 2 分钟的不等，照这个名字写进日志；换模型（施工 8-10）：记一条、一样的不记、下一个回合开始交出去，回合进行中换的下一轮才交，撤掉的回合里换的也算、崩了载入照样算，退回默认的记在注入前面、`cause` 是回合的、以后钉在退回的上面，`models.chat` 也没有的不记 |
| `crates/miyu-kernel/src/session/tests/scenario/vision.rs`（施工 8-17） | 替它看图：看不了图的先转述、内核记一条不带回合编号、`cause` 是这一轮的转述、落了盘才请求、请求的 `described` 是它；同一张图只转述一次，第二轮、崩了载入、撤销、压缩以后照样用；看得了图的、没交过限额的不转述；没成的不记、这一轮不再试、主请求照发、下一轮再试；转述带人这一轮最近说的那一句，这一轮只发了图的不带上一轮的话；看图时打断直接结束、回来的照样记；看图时切了级别，事实排在转述后面、请求前面；快照里没有转述的字的不转述 |
| `crates/miyu-kernel/src/session/tests/present.rs`、`scenario/present.rs`、`random/watch/present.rs`（施工 R-4 上） | `RunTurnStartHooks` 的 `present`：模块交的一块下一轮在里面、`refs` 原样记原样交回，清空、撤掉带着它的那一轮以后不在，内核自己的不算；压缩时它在被压掉的那一段里的，压完的下一轮不在，在留下的那一截里的（压的那一轮开头交的）照样在；旧的 `context.injected`（没有 `refs`）读写一字不差。随机输入里挂接点有时交一块模块的事实（带 `refs`），看守每次收到 `RunTurnStartHooks` 照自己的有效历史算一遍 `present` 对上 |
| `crates/miyu-kernel/src/session/tests/random.rs` 的替它看图（施工 8-17，`random/sighting.rs`、`watch/sight.rs`） | 八个种子里的一个：看不了图才转述，转述过的、在路上的不再转；转述是内核记的、不带回合编号、就是送回去的那一句；请求的 `described` 正好是日志里这几张图的；看不了图的主请求里的图都转述过或者这一轮没成；对不上的转述不理；回合过去了才回来的照样记 |
| `crates/miyu-kernel/src/session/tests/configure.rs`（施工 8-10） | 换模型记一条、落了盘才回应、同一个编号再来不再记；一样的当场回、编号照记；造会话记下的引用交出去、没有的交没有；回合进行中的带上回合、回合照常；载入照日志算、撤掉的回合里换的也算；改回文件时拒；退回对不上的不记 |
| `crates/miyu-kernel/src/session/tests/scenario/breaker.rs` 的 `a_new_model_lifts_the_pause_and_failures_count_from_zero`、`scenario/manual.rs` 的 `compacting_and_clearing_after_a_model_change_do_not_resolve_again`（施工 8-10） | 换了模型暂停解除、到线又压、换过去以后再失败从 0 数、载入以后一样、换成一样的不解除；换模型以后手动压缩、清空那一轮不交引用 |
| `crates/miyu-kernel/src/session/tests/scenario/harness.rs`（施工 7-10） | 别的 harness 发来的话：闲着开一轮、`trigger` 和 `cause` 照它、回应只附它、不带回合编号；正忙下一次请求听到、结束时不再开；最后一步里到的接着开；打断两种都不撤回、不接着开；不作废在等人回答的题；没人看着的一次性会话只记下；能恢复撤销时记在一边、载入以后恢复了接着开；撤掉它开的那一轮它留着，它开的那一轮重做不了；子会话里收到的不欠父会话回报 |
| `crates/miyu-kernel/src/session/tests/scenario/peers.rs`（施工 C-2） | 别的会话发来的话：三种关系（人的、父会话的话排进这一轮，子代理、别的会话的不带回合编号）；闲着开一轮、`trigger` 和 `cause` 照它、回应只附它；正忙下一次请求听到、结束时不再开；最后一步里到的接着开；打断两种都不撤回、不接着开；不作废在等人回答的题；没人看着的一次性会话只记下；能恢复撤销时记在一边、载入以后恢复了接着开；撤掉它开的那一轮它留着，它开的那一轮重做不了；子会话里收到的不欠父会话回报 |
| `crates/miyu-kernel/src/session/tests/scenario/watch.rs`（施工 C-6） | 空了的通知：在等的记下、叫醒、`cause` 照命令；不在等的拒、什么都不记；撤掉订它的那一轮不等了、恢复了又等；作废到点才记（含正好那一刻）、只记下、`cause` 是订它的那一轮的；又订从新的时刻算；`gone` 只记下；执行器交的 `idle` 不理；`vacant()` 要子代理都报完、后台命令不算；`last_line()` 的截法；它开的那一轮重做不了、撤销不带走；能恢复撤销时记在一边、载入以后恢复了接着开 |
| `crates/miyu-kernel/src/session/tests/scenario/flood.rs`（施工 C-2） | 防刷屏：第 6 句拒、被拒的什么都不记、别的发话方照收；正好 600 秒那一刻还算、过了 1 毫秒就收；一字不差的拒、不占数、别的发话方发同样的照收、窗口过了又收；没听到的第 51 句拒、听到以后又收；重启以后限速、一字不差、没听到的数照日志算回来；子代理的留言不受「5 句」管 |
| `crates/miyu-kernel/src/session/tests/scenario/messages.rs`（施工 7-7） | 子代理的留言：闲着开一轮、`trigger` 和 `cause` 照它、回应只附它；正忙下一次请求听到、不带回合编号；最后一步里到的接着开；打断两种都不撤回、不接着开；不作废在等人回答的题；没人看着的一次性会话只记下、有头订阅着照常开；派它的那一轮撤掉了的不开轮；能恢复撤销时记在一边、载入以后恢复了接着开；由它接着开的一轮撤掉，人的话跟着撤、留言留着；载入以后下一轮听到；不是她派的会话发来的是别的会话发来的话（施工 C-2），不带回合编号；`subagents()` 的每一种；中间一层答完孙代理的留言不报、孙代理报完再报；孙代理在回报里问、它留言答了（`job.messaged`）也等孙代理再报 |
| `crates/miyu-kernel/src/session/tests/scenario/upward.rs`、`scenario/upward_load.rs`、`session/report/tests.rs`（施工 7-6） | 向上回报：做完交代报那一轮最后说的、只报一次；主会话不报；人自己开的不报；人开的一轮里进了父会话的留言报、注明人；交代那一轮里人插过话注明；超长的截头尾、中间那一行；出错报半截、没说话的正文空、步数上限报最后说的；孙代理都报完、被叫醒的那一轮结束才报一次；孙代理崩了闲着当场报；它自己的子代理开的不报；崩在那一轮里载入报 `aborted`；报过的载入再交一次；重启接着干、做完再报，没接着干的报 `aborted`；打断的等下一轮；补的一批没全落盘不报；收到「要重启了」以后不交、再载入时交一次；父会话挤掉最近 1024 个编号以后照样认出重交的，换编号的是新的一份。截正文：到上限原样、头尾各一半、单数尾巴多一个、空的那一行只留换行 |
| `crates/miyu-kernel/src/session/tests/queue.rs` | 最后一步里来的开下一轮；由最后一条触发；出错、到上限的也接着开；被后一步听到的不再开；打断接着发、退回、留着（施工 O-6：不撤回、不接着开，下一句开的那一轮看得到）；没排着的不写撤回；触发不算排队 |
| `crates/miyu-kernel/src/session/tests/respond.rs`（施工 O-14 上） | 照记下的几条开一轮：`triggers` 排好去重、`trigger` 是最后一条、桥的事实接在内核的后面；空的拒；不是旁听的、当过触发的拒，带上是哪几条；载入以后照样认得当过触发的 |
| `crates/miyu-kernel/src/session/tests/respond/joining.rs`（施工 O-14 下） | 正在跑一轮时：桥的事实、`turn.joined` 带这个回合记下、回应附序号；下一步听到的不另开；没再请求就结束的接着开一轮、由 `turn.joined` 触发；打断（接着发、退回）不退回不接着开；在等的那一问作废 |
| `crates/miyu-kernel/src/session/tests/interrupt.rs` | 空闲时打断被拒；请求前、请求中、调工具时打断；什么都没收到不写回复；急着插话的三种时候；空闲时急着插话开回合；在跑的写叫它停、排在后面的当场补、10 秒以后叫醒、改完了的带着效果记、收了尾到点不理 |
| `crates/miyu-kernel/src/session/tests/landed.rs`（施工 3-8 六补） | `landed()`：造会话那一条落盘以前没有；追加了、还没落盘的不算，落一部分走一部分；载入的是日志里最后一条 |
| `crates/miyu-kernel/src/session/tests/meta.rs`（施工 3-8 三补） | 改名记一条、落了盘才回应、同一个编号再来不再记；只写变了的格，和现在一样的（含没有标题时去掉、没置顶时取消、两格都不写）当场回应、编号照记；去掉标题记成空的；回合进行中的带上回合；载入以后照日志算回来，撤掉的回合里改的也算；`deletable()`：空闲的删得了，开了回合、`turn.ended` 没落盘的是有回合在进行，改回文件的时候是正在改回；斜杠命令记一条 `command.ran`、带着说命令的人和起因、同一个编号再来不再记（施工 O-6） |
| `crates/miyu-kernel/src/session/tests/scenario/permission_changed.rs`（施工 2-7 补） | 切了级别以后的那一块：空闲时切的下一轮开头用切换那一份；来回切了一圈的不注入；回合中途切的这一轮下一次请求之前注入；连着切、压缩以后、撤掉带着上一块的几轮以后、撤掉的回合里切的载入以后照样算、崩了载入以后不重发、以前的快照 |
| `crates/miyu-kernel/src/session/tests/scenario/session_fact.rs`（施工 1-13 再补） | 会话编号：第一轮排在环境、权限后面注入，第二轮不注入；压缩以后再注入一次；撤掉带着它的那一轮以后下一轮重新注入；崩了载入以后编号一样、不重发；子会话写它自己的编号；以前的快照没有模板的不注入 |
| `crates/miyu-kernel/src/session/tests/permission.rs` | 空闲时切、切成一样的、不认识的级别；收紧成只读拦下回复里的、这一步里等着的写入；放宽等下一次请求；来回切不注入；挂接点前后切；收紧成工作区什么都不拦；只读下改常用的那一级；事实写这一轮的工作目录 |
| `crates/miyu-kernel/src/session/tests/restart.rs` | 重启照打断收拾；接着干；排着的由最后一条开；连着 4 次不接；走完一轮、你开口以后从头数 |
| `crates/miyu-kernel/src/session/tests/scenario/commands.rs`（施工 7-3） | 用过的最大任务编号：没派过的 0，撤掉的回合里派的也算，载入以后照日志算回来；崩了载入补 `aborted`（只补后台命令、`by` 是内核、没有 `cause`、回合编号、用时、输出，不开轮，再载入不再补），结束过的不补，补的排在那一轮收尾前面；要重启了以后到的结束只记下、不开轮，接着干的那一轮看得到它 |
| `crates/miyu-kernel/src/session/tests/tools.rs` 的 `a_dispatched_call_carries_the_cause_of_its_turn`（施工 7-3） | 派出去的调用带着这一轮的 `cause`，链当场放行的、等人允许了的一样 |
| `crates/miyu-kernel/src/session/tests/load.rs` | 走完的载入一样往下走；坏日志拒绝；崩在哪都收尾、等你开口；崩之前的命令不再生效；生效的权限回来；检查点重读过文件的，第一个动作是 `Recall`（施工 6-9；撤掉压缩的撤销载入以后见 `history.md`「守着它的」） |
| `crates/miyu-kernel/src/session/tests/spans.rs`、`scenario/spans.rs`（施工 2-3 补） | 回复每一块的起止：思考、正文、工具调用各一块照增量的时刻，收块不算；驱动流完了才一起收块、字交错着来、私有数据、时钟往回拨；出错收的半截、打断收的半截只记留下的，空块不记；出错没收到字的没有这一格 |
| `crates/miyu-kernel/src/session/tests/scenario.rs`、`scenario/retrying.rs`、`scenario/stopping.rs` | 执行器替身（`testkit`）把真会话一整轮一整轮地跑：两个读一起跑、中间来一句；只读拦写入；步数上限和失败的请求；重试的每一种（原样再来、半截接着说、半截的调用丢掉、带着 HTTP 状态码、照供应商等、5 次放弃、不该再来的、等的时候打断、重启、切级别、不算步数、说完清零）；打断接着发、重启接着干、崩了等你；停着的写：停在改之前、改完了、到 10 秒、又打断一次、等的时候来的消息排队和撤销被拒、等的时候重启（退回的不再接着干，接着发的交给下一轮） |
| `crates/miyu-kernel/src/session/tests/random.rs` 和 `random/` | 三百例随机输入（CI 另跑两万例），每一步查：不变量（第 5 条一个会话查不了）；块的起止写了回复的才有、和回复的块一块一项，摘要请求没有（施工 2-3 补）；挂接点、请求、派工具、步数上限、只读的规矩；打断时停着的（叫它停只在打断里、停着的交回来才收尾、到点和又打断就不等，十个种子里一个多调写文件的专走这里）；崩了、重启了载入以后照规矩走；回报（施工 7-2，另一串随机数，七个种子里一个是一次性的会话）：对不上的拒绝、不理，开轮、排着、只记下、回合结束接着开、恢复撤销以后接着开照「回报」的规矩（`random/watch/reports.rs`）；清空（施工 6-8 补，另一串随机数）：照规矩收下或者拒绝，收下的一批三条（`random/watch/clear.rs`）；重做（施工 4-7 再补，和撤销同一串随机数）：照规矩收下或者拒绝，收下的撤最后一轮、重发撤掉的人的话、由最后一句开一轮（`random/watch/redo.rs`）；改标题、置顶（施工 3-8 三补，另一串随机数，每一例最后喂一次：夹在中间会让难得走到的几条路走不到）；打断时排着的留着（施工 O-6，种子除以 4 余 1 的退回换成留着，别的种子的输入不变，订别的会话的那批种子不在里面）：那一轮结束以后不接着开、不撤回；记下斜杠命令（施工 O-6，跟在改标题后面喂）；回顾（施工 3-8 四补，另一串随机数，四个种子里一个）：照落了盘的有效历史判出拒绝、交回、并进、请求，请求照重建的有效历史组装，替身送的回报照内核的规矩算出写没写成，两条不带回合编号、回应照判出来的，排着的、回报、压缩的边界都不看它；有回顾在路上时不崩，跑完以前让它说完（`random/watch/recap.rs`、`random/recapping.rs`）；别的会话发来的话（施工 C-2，另一串随机数，三个发话方、四句话，随机的策略把数调小成 3 句、20 秒、5 句）：防刷屏照看守自己数的拒、原因对得上，收下的不带回合编号、同一批里不作废不撤回，叫不叫醒她照回报的规矩（`random/watch/peers.rs`、`random/peering.rs`）；出错再来（施工 8-9 加换端点、全在冷却）：再来的只有能再来的分类和端口说换了端点的，换了端点、全在冷却的没到 5 次、要等的不超过 2 分钟就一定再来，推的 `status` 带的 `failover` 照端口说的，换了端点又没说等多久的等 0 毫秒（`random/watch/model.rs`、`random/endings.rs`）；换模型（施工 8-10，另一串随机数、另一串命令编号，四个种子里一个）：一样的不记，不一样的只记一条，回合开始交的是看守照日志算的引用，挂接点结果里的退回对得上才记、记在注入前面，熔断只看换过去以后的（`random/watch/configure.rs`、`random/configuring.rs`；施工 8-18 曾在这个种子里再多生成一串思考强度的命令，8-18（补）去掉了）；每条路、每一种输入都走到过 |
| `crates/miyu-kernel/src/tool/tests.rs`、`tool/texts/tests.rs` | 参数修正的每一种，嵌套的对象、数组里的也修；访问类别不认识的算写入；那几句的字段转义、每句带说法 |
| `crates/miyu-kernel/src/accumulate/tests.rs` | 拼回复、调用编号、空块、交错的字、截断只留收全的调用、增量对不上的六种；每一块带着它在流里是第几块（施工 2-3 补） |
| `crates/miyu-kernel/tests/resources.rs` | 出厂的那几句读得进来，带字段的换出来一字不差 |
| `crates/miyu-kernel/tests/transient_sample.rs` | `model.delta`、`tool.progress`、`status` 写出去和样本一字不差 |

### 出处

- `02-内核.md` 第四节（输入、动作、命令怎么写；执行器怎么回动作；原因码）、第六节（回合怎么开、请求怎么发；回复怎么收、回合怎么结束；工具怎么调、下一步怎么走；打断和急着插话；排队的消息；权限级别怎么切；载入、崩溃、重启）、第九节（不变量怎么查）。
- `03-事件模型.md` 第三节（模型调用怎么写）、第五节（增量和累积器怎么写、瞬时事件的外壳）。
- `05-内核接口.md` 第五节（挂接点）、第六节（工具的规格：访问类别、修正畸形参数）。
- `07-存储.md` S4：先落盘，后推送、后回应。
- `08-上下文投影.md` 第五节（环境和状态的事实怎么写）、C10。
- `11-权限与沙盒.md` 第二节（三个权限级别，收紧当场、放宽下一步）。
- `15-模型与供应商.md` M5：重试 5 次。
- `26-提示词.md` J3（内核写的英文只说发生了什么）、第十节（登记簿）、J6（辅助请求各自声明用途，施工 3-8 四补）。
- `04-核心协议.md` 第九节 `session.recap`：回顾是核心的协议，照 codex 的做法单独发一次辅助请求（2026-10-01 项目主人定）。
- 起标题：照 Claude Code，人手动起名以外，没起名的会话核心自己起；标题 3 到 7 个词、第一行、最多 50 个字，一次性的会话不起（2026-10-01 项目主人定，施工 3-8 五补）。

### 还没有的


- 别的挂接点：命令进入、模型输出后、工具执行后、订阅（`05-内核接口.md` 第五节）。现在只有回合开始、回合结束和执行前的链；回合开始、回合结束还没有模块挂，会话 actor 交回空的注入（`crates/miyu-session/src/actor.rs`）。
- 压缩（`compaction.md`）：`context_too_long` 现在结束回合，不先压缩（被动压缩，6-7）；截掉最老的再试、隔离式回退（6-6 下）；撤销能撤掉压缩（6-9）。
- 子代理、后台命令（`02-内核.md` 第七节，M7）：回报到了开不开轮施工 7-2 做好了，施工 7-3 起有真的后台命令，载入时给没结束的补 `aborted`；子会话向上回报施工 7-6 做好了；撤销时一起停下施工 7-8 做好了（`history.md`「撤销」第 5 条）。
- 等第一个字时的心跳 `status`（`03-事件模型.md` 第五节）；中途连上的头拿「到目前为止的内容」（M8）。
- `session.answer` 协议收了（施工 D-1，`protocol.md`）；`session.set_permission_level` 协议收了（施工 3-8 再补，`protocol.md`）。
- 没人盯着的场所的步数上限，随预设定（`02-内核.md` 第六节「工具怎么调、下一步怎么走」第 6 条）。
