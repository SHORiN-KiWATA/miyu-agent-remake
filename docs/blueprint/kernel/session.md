## 会话怎么走

### 是什么

一个会话是一个状态机（`Session`）：送进一条输入，出来一串动作。它不做 I/O，不读时钟：要追加的事件、要回应的命令、要推给头的事件、要跑的挂接点、要发的请求、要跑的工具，都写成动作交给执行器；执行器做完，把结果当成新的输入送回来。事件的时刻取自引起它的那条输入。

这一页写命令、回合、请求、重试、工具、排队、打断、切权限级别、重启和载入。执行前的链、确认、提问在 `asking.md`；账本、有效历史、撤销和恢复在 `history.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/session.rs` | `Session`：造会话、分派输入、收命令、造事件、落盘以后推送和回应 |
| `crates/miyu-kernel/src/session/input.rs`、`action.rs` | 输入、命令；动作、结局、原因码 |
| `crates/miyu-kernel/src/session/policy.rs`、`recent.rs` | 冻结在会话上的策略；最近接受的命令编号 |
| `crates/miyu-kernel/src/session/turn.rs`、`call.rs`、`retry.rs` | 开回合、发请求、结束回合；收回复、记 `model.called`；出错再来 |
| `crates/miyu-kernel/src/session/compaction.rs` | 压缩这一步：到没到线、替代到哪、发摘要请求、收回来写 `context.compacted`（`compaction.md`，施工 6-2 上） |
| `crates/miyu-kernel/src/session/tools.rs`、`step.rs` | 这一步的调用：先查、派、收结果、补结果；每个调用走到了哪、轮到谁 |
| `crates/miyu-kernel/src/session/queue.rs`、`interrupt.rs` | 排队的消息；打断 |
| `crates/miyu-kernel/src/session/permission.rs` | 切权限级别、请求之前查事实 |
| `crates/miyu-kernel/src/session/restart.rs`、`load.rs` | 有计划的重启；从日志载入、崩了的收尾、重启后接着干 |
| `crates/miyu-kernel/src/accumulate.rs` | 增量拼成回复 |
| `crates/miyu-kernel/src/tool.rs`、`tool/texts.rs` | 访问类别、参数修正；内核替工具写的几句 |
| `crates/miyu-kernel/src/facts.rs` | 环境、权限两块事实，回复被截断的那一句 |
| `resources/core/tool-results/`、`resources/core/facts/reply-cut.txt` | 给模型看的字 |
| `resources/core/human/{zh,en}.json` | 内核写的结果给人看的说法 |

### 对外的样子

| 函数 | 做什么 |
|---|---|
| `Session::create(id, by, at, created, policy, environment)` | 造会话：追加第 1 条 `session.created`，`cause` 是 `id`，出来一个 `Append`；落了盘回应 `id`。开始时的权限取自 `created.permission` |
| `Session::load(events, at, policy, environment)` | 从日志载入，出来会话和要补的动作；载入不了的是 `LoadError`（「载入和崩溃」） |
| `handle(input)` | 送进一条输入，出来一串动作 |
| `idle()` | 空闲：没有回合在进行，没有结束了、`turn.ended` 还没落盘的回合，没在改回文件。核心照它决定能不能空闲退出 |

**输入**（`Input`）：

| 输入 | 带着 | 见 |
|---|---|---|
| `Command(Received)` | `id` 命令编号、`by` 谁发的（取自连接）、`at` 到的时刻、`command` | 「命令和回应」 |
| `Stored { upto }` | 落了盘的最后一条的序号 | 「命令和回应」第 8 条 |
| `Environment(Environment)` | `offset` 时区、`cwd` 工作目录（头报的、人看到的写法）、`dirs` 加进来的目录（施工 5-10 上） | 换掉会话的环境，什么都不出；下一个边界才用 |
| `Limits(Limits)` | `model` 发给哪个端点的哪个模型、`window` 上下文窗口、`max_output` 最大输出，没报的是 `None`（施工 6-2 上）；`images` 一张图怎么算（`estimate::ImagePrice`，驱动交的，没有的照策略里的固定数，施工 6-3 上） | 换掉会话的模型限额，什么都不出；只在内存里，载入以后执行器再交一次。没交过的不主动压缩（`compaction.md` 第二条） |
| `TurnStartHooksDone { at, turn, injected }` | 哪个回合；各模块的注入 `Injection { module, fact }`，照固定的先后 | 「回合」第 4 条 |
| `RequestSent { at, seen, model, request }` | 哪次请求；发给了哪个端点的哪个模型（`Model { endpoint, model }`）；驱动编码以后的请求字节的哈希 | 「收回复」 |
| `ModelDelta { at, seen, delta }` | 一段增量：`Start { index, kind }`、`Text { index, text }`、`Private { index, private }`、`End { index }` | 「收回复」 |
| `ModelEnded { at, seen, usage, error, wait_ms }` | 用量；出错的分类和原话；供应商说要等多少毫秒。没发出去就失败的不报 `RequestSent`，直接报这一条 | 「收回复」「出错再来」 |
| `Woke { at, seen }` | 为哪一次请求等的；等停着的，是那一步回复的序号 | 「出错再来」「打断」第 7 条 |
| `ToolDone { at, call_id, error, blocks, duration_ms, human, effects, stopped }` | 出没出错、给模型看的内容、用时、给人看的说法、效果；叫它停以后停在了改之前的，`stopped` 是真的 | 「调工具」「打断」第 7 条 |
| `ToolProgress { at, call_id, text }` | 一段输出 | 「调工具」 |
| `ToolGuarded { at, call_id, verdict }`、`ToolAsks { at, call_id, questions }` | 链的结论；一组题 | `asking.md` |
| `Restored { at, files }` | 改回文件每一步的结局 | `history.md` |
| `Reread { at, seen, files }` | 哪一次摘要请求；压完要重读的文件，一个一项，照交出去的先后：读到了（`blob`、原文）、太大、读不到（施工 6-5） | 记在那次摘要请求上，什么都不出（`compaction.md` 第九条） |
| `Recalled { texts }` | 最近一个检查点里重读的文件的原文，照 blob 找（施工 6-5） | 载入以后、别的输入之前交；什么都不出 |
| `Restarting { at }` | 要重启了 | 「有计划的重启」 |

**命令**（`Command`）：

| 命令 | 协议里的方法 | 带着 | 见 |
|---|---|---|---|
| `Send { blocks, urgent }` | `session.send` | 内容块；`urgent` 急着插话 | 「发一条消息」 |
| `Interrupt { queued }` | `session.interrupt` | `Queued::Send` 排着的接着发，`Queued::Return` 退回 | 「打断」 |
| `SetPermission { level, read_only }` | `session.set_permission_level` | 常用的那一级、只读开关，不改的是 `None` | 「切权限级别」 |
| `Answer { call_id, answer }` | `session.answer` | `Answer::Approval { decision, reason }` 或 `Answer::Questions(回答)` | `asking.md` |
| `Revert { turn }`、`Unrevert` | `session.revert`、`session.unrevert` | 从哪一轮起，`None` 是最后一轮 | `history.md` |

**动作**（`Action`）：

| 动作 | 带着 | 执行器做什么 |
|---|---|---|
| `Append(事件)` | 这一批事件 | 一次写入、一次同步，送回 `Stored` |
| `Reply { id, outcome }` | 命令编号、结局 | 交给发命令的连接 |
| `Push(事件)` | 落了盘的事件 | 推给订阅了的头 |
| `PushTransient(Transient)` | 一条瞬时事件 | 推给头，不落盘，不等 |
| `RunTurnStartHooks { turn }` | 回合 | 叫各模块，等齐或超时，送回 `TurnStartHooksDone`；一个模块都没挂也回一次 |
| `RunTurnEndHooks { turn }` | 回合 | 广播，不等，不送回 |
| `CallModel { seen, request, changed }` | 看到第几条（也是这次请求的名字）、统一的请求、和上一次比第一处不同 | 交给驱动发出去；送回 `RequestSent`、`ModelDelta`、`ModelEnded` |
| `CancelModel { seen }` | 哪次请求 | 掐掉，不送回；之后到的不理 |
| `Wake { at, seen }` | 什么时候、为哪次请求 | 到点送回 `Woke` |
| `GuardTool { call_id, name, args, cwd, dirs, permission }` | 修正过的参数、这一轮的工作目录和加进来的目录、实际生效的那一级 | 过执行前的链，送回 `ToolGuarded`（`asking.md`） |
| `RunTool { call_id, name, args, cwd, dirs, permission }` | 修正过的参数、这一轮的工作目录和加进来的目录（施工 5-10 上）、派出去那一刻实际生效的那一级（施工 5-4 上：执行器照它写沙盒的规格） | 跑；送回 `ToolProgress`、`ToolAsks`、`ToolDone` |
| `AnswerTool { call_id, answers }` | 人的回答 | 交给在等的调用（`asking.md`） |
| `CancelTool { call_id }` | 哪次调用 | 掐掉，不送回；之后到的不理 |
| `StopTool { call_id }` | 哪次改文件的调用 | 叫它停：停在改之前，或者做完；照常送回 `ToolDone`，停在改之前的带 `stopped`（「打断」第 7 条） |
| `Restore { steps }` | 改回的几步 | 改回文件，送回 `Restored`（`history.md`） |
| `Reread { seen, paths, limit }` | 哪一次摘要请求（排在它的「请求模型」前面）；要重读的文件，真实的位置，照先后；单个最多多少字节（施工 6-5） | 读完、存成 blob 再做下一个动作，送回 `Reread`（`compaction.md` 第九条） |

**结局**（`Outcome`）：`Accepted { events }` 接受，附上它产生的事件的序号，照先后；`Rejected { reason }` 拒绝，什么都没产生。原因码是稳定的英文（`Reason::code`）：

| 原因码 | `Reason` | 什么时候 |
|---|---|---|
| `empty_message` | `EmptyMessage` | 发来的消息一块内容都没有 |
| `not_running` | `NotRunning` | 没有回合在进行，打断不了 |
| `unknown_level` | `UnknownLevel` | 要切到的级别不认识 |
| `not_asking` | `NotAsking` | 这个调用不在等这种回答（`asking.md`） |
| `unknown_decision` | `UnknownDecision` | 确认的选项不认识 |
| `no_rule` | `NoRule` | 请求没提放行规则，却选了 `session`、`workspace` |
| `unexpected_reason` | `UnexpectedReason` | 不是拒绝，却带了理由 |
| `bad_answer` | `BadAnswer` | 回答对不上题目 |
| `turn_running` | `TurnRunning` | 有回合在进行时撤销（`history.md`，下同） |
| `unknown_turn` | `UnknownTurn` | 要撤的那一轮不在有效历史里 |
| `compacted` | `Compacted` | 要撤的那一轮压缩进了摘要 |
| `nothing_to_unrevert` | `NothingToUnrevert` | 没有能恢复的撤销 |
| `restoring` | `Restoring` | 正在改回文件 |
| `nothing_to_revert` | `NothingToRevert` | 不写回合编号的撤销，一轮都没有 |

**策略**（`Policy`）：造会话、载入时由执行器照策略快照造好交进来，会话里不再变。

| 格 | 是什么 | 现在交进来的 |
|---|---|---|
| `assembler` | 组装请求的做法（`Assembler`，要能挪到别的线程） | `miyu-assemble` 的默认组装 |
| `facts` | 事实的模板：环境、权限、回复被截断 | `resources/core/facts/` 的三份 |
| `tools` | 工具名到 `ToolRule { access, parameters }`：访问类别、参数格式 | 快照的工具面 |
| `step_limit` | 一个回合最多请求几次模型，`None` 不限 | `None`（`miyu-policy` 的 `compose`） |
| `tool_texts` | 内核替工具写的 13 句（`ToolTexts`） | `resources/core/tool-results/` |
| `attended` | 有没有人能确认、回答 | 造会话的那个连接握手时的 `caps.input` |
| `resumes` | 有计划的重启打断了一轮，连着接着干几次 | 3 |
| `compaction` | 压缩用的数：`reserve_cap` 输出预留的上限、`margin` 余量、`tail` 尾巴的上限、`price` 估算时一张图、一个文件各算多少；`None` 不主动压 | 20000、13000、16000、各 2000（`miyu-policy` 的 `compose`，施工 6-2）；以前造的快照里没有的是 `None` |

### 怎么走

**命令和回应**：

1. 每收到一次命令，回应一次：接受，或者拒绝。
2. 拒绝的当场回应，什么都不追加。编号不记：同一个编号再来，重新判。
3. 接受的，等它产生的事件都落了盘才回应。附上的序号：发消息的只有 `message.user` 那一条；打断、回答确认的是这一次追加的全部；回答提问、切权限级别的是 `question.answered`、`session.policy_changed` 那一条；切到和现在一样的级别，空的，当场回；撤销、恢复见 `history.md`。
4. 记着最近接受的 1024 个编号（`recent::CAPACITY`）和它们的序号，满了丢最早的。同一个编号再来，不再生效：它的事件都落了盘的，当场照上一次回应；还没有的，等落了盘再回，来几次回几次。
5. 正在改回文件的时候（撤销、恢复以后），接受过的编号照第 4 条；别的命令一律拒绝，`restoring`。
6. 命令产生的事件：`at` 是命令到的时刻，`by` 是发命令的一方，`cause` 是命令编号。一条输入产生的几条事件，时刻相同。
7. 造一条事件：序号照账本给；`turn.started` 的 `turn` 是它自己的序号，别的事件在回合进行中带上这个回合，空闲时没有；先过账本（`history.md`），再进有效历史，等落盘。
8. **落了盘**（`Stored { upto }`）：追加过、还没落盘的事件里，序号不超过 `upto` 的算落了盘；一条都没有的，什么都不做。有的，照这个先后出：`Push` 这些事件；`Reply` 事件全落了盘的命令，照收到的先后；`RunTurnEndHooks` `turn.ended` 落了盘的回合；然后回合往下走（派工具，或者跑回合开始的挂接点，或者发请求）。

**发一条消息**（`Send`）：

1. 一块内容都没有：拒绝，`empty_message`。
2. 空闲时：追加 `message.user`，同一批开一个回合（「回合」第 1 条）。这条消息不带回合编号。急着插话的也一样。
3. 回合进行中：追加 `message.user`，带上这个回合，排进队（「排队的消息」）。这一步里在等人的调用作废（`asking.md`「在等的怎么了结」）；急着插话的，再跳过还没跑的（「急着插话」）。都在同一批；叫停的 `CancelTool` 排在 `Append` 后面。

**回合**：回合编号是它 `turn.started` 的序号。回合走到的阶段：

| 阶段 | 在等什么 | 然后 |
|---|---|---|
| `Opening { opened }` | 开头那一批落盘到 `opened` | `RunTurnStartHooks`，到 `Hooking` |
| `Hooking` | 回合开始的挂接点跑完 | `Ready` |
| `Ready` | 追加过的事件都落了盘 | `CallModel`，到 `Asking` |
| `Asking(请求)` | 执行器的三种回报。可能是压缩的摘要请求（`compaction.md` 第三条） | `Settling`；摘要请求取到了摘要的，回 `Ready` |
| `Waiting { after }` | 为 `after` 那次请求的 `Woke` | `Ready` |
| `Settling` | 只在处理一条输入的当中出现，什么输入都不收 | 结束、`Tools`，或者 `Waiting` |
| `Tools(这一步)` | 这一步的调用都有了结果 | `Ready`，或者结束 |

1. **开回合**：追加 `turn.started`（`trigger` 是触发它的那条，`cwd` 是会话现在的环境里的工作目录（施工 4-9 再补三上），`dirs` 是加进来的目录，没有就不写（施工 5-10 上），`by` 是内核，`cause` 是触发它的那条的 `cause`），紧跟着环境、权限两块事实里变了的。这时实际生效的权限换成现在的（空闲时放宽的，这时生效）；这一轮的工作目录、加进来的目录取会话现在的环境，这一轮里不变。
2. **事实**：环境一块（`kind` 是 `env`：这一刻到小时、时区、工作目录）、权限一块（`permission`：实际生效的那一级），`by` 是内核。和有效历史里内核记的同一类最近一块逐字节一样的，不追加。写法、比法见 `kernel/request.md`「事实」。
3. 开头那一批落了盘，出 `RunTurnStartHooks`，一个回合一次。
4. **挂接点跑完了**：回合对得上、正在等挂接点的才收；别的（打断以后迟到的、第二次来的、别的回合的、空闲时来的）不理。注入照交回来的先后追加成 `context.injected`，`by` 是各自的模块，`cause` 是回合的；这一轮里切过权限级别的，再查一遍事实（「切权限级别」第 6 条）。
5. **发请求**：到了 `Ready`，追加过的事件都落了盘。拿有效历史组装；先问熔断（`session/breaker.rs`，`compaction.md` 第二条第 5 条、第十条，施工 6-6 上）：暂停着、放不下的，记一条没发出去的 `model.called`，结束回合 `error`；压完很快又到线第 3 次的，写 `context.compaction_paused`，回到 `Ready`。用量过了压缩线的，先发摘要请求（`compaction.md` 第二、三条：名字是替代到的 N，不算请求数，也记进「上一次的指纹」），这一次的主请求等压完再组装。没过线的：`seen` 是落了盘的最后一条；算出请求的指纹，和这个会话上一次组装的比出第一处不同（工具面、system，或者第几条消息，从 0 数起，和它的角色；上一次有、这一次少了的，从少了的那一条算），只是接着加的是 `None`。上一次的指纹只在内存里：造会话、载入以后的第一次都是 `None`（`kernel/request.md`「第一处不同」）。不是重试的，这一轮的请求数加一。急着插话的记号、排着队的清单清掉。出 `CallModel`。
6. **结束回合**：追加 `turn.ended`，会话空闲；它落了盘才出 `RunTurnEndHooks`。还有排着队的，同一批接着开下一轮；重启、崩了收尾的不开（「排队的消息」）。

| 结束的原因 | 什么时候 | `by` |
|---|---|---|
| `completed` | 回复里没有工具调用 | 内核 |
| `error` | 出了不重试的错，或者重试够了、要等的太久 | 内核 |
| `step_limit` | 这一步的调用齐了，请求数到了上限 | 内核 |
| `interrupted` | 打断 | 打断的人 |
| `restarted` | 有计划的重启 | 内核 |
| `aborted` | 载入时发现上次崩在回合里 | 内核 |

**收回复**：三种回报都带着 `seen`，不是在路上的那一次的，不理。

1. `RequestSent`：记下时刻、模型、请求字节的哈希。报两次的只认第一次。摘要请求的，推一条 `written` 是 0 的 `compaction.progress`（施工 6-3 下）。
2. `ModelDelta`：摘要请求的增量照样交给累积器，不推 `model.delta`，正文块的每一段推一条 `compaction.progress`（`compaction.md` 第三条第 8 条）。还没报发出去就来了增量，按出错算：分类 `bad_stream`，原话「请求还没发出去就来了增量」。交给累积器，对不上的也按 `bad_stream` 算，原话是累积器的报错（「出错」）。出错的照下面第 3 条收拾，再出 `CancelModel`。收下的推一条 `model.delta`（`by` 是那个模型，`cause` 是回合的，`body` 是 `seen`、第几块、这一段）；私有数据收下，不推。第一段增量到的时刻记下。
3. `ModelEnded`：
   1. 没发出去、也没带出错的，按出错算：`bad_stream`，「请求还没发出去就说完了」。没发出去的不写回复。
   2. 摘要请求不写回复：正常说完的，收到的拼好交给组装取出摘要，取到了写 `context.compacted`，推 `compaction.done`（`compaction.md` 第三条第 6、11 条）；调了工具的、取不出来的，按出错算，`bad_summary`，不再来。别的出错照下面第 5、6 条。
   3. 发出去了的主请求，收到的拼成回复。正常说完：每一块照收到的拼，没收全的工具调用也留下。出错：只留收全了的工具调用（和打断一样），再把工具调用全去掉。一个字都没有的正文块不要；没有字、也没有私有数据的思考块不要。工具调用编号 `call_<这条回复的序号>_<k>`，`k` 从 1 数留下的（累积器见 `kernel/request.md`）。
   4. 拼出来一块都没有的不写回复；正常说完的，按出错算：`empty_reply`，「回复里一个块都没有」。
   5. 有的写成 `message.assistant`：`seen` 是这次请求的，出错的多写 `"interrupted":true`，`by` 是那个模型，`cause` 是回合的。
   6. 接着追加 `model.called`（下表），`by` 是内核，`cause` 是回合的。
   7. 出错的：能再来就等着再来（「出错再来」），不能的结束回合，`error`。收到的半截照样留在日志里。自动压缩的摘要请求这样结束的，是一次失败：最近一个检查点以后数到 3 次，在 `turn.ended` 前面写 `context.compaction_paused`（`compaction.md` 第十条第 4 条）。
   8. 正常说完的：这一步连着出错的次数清零。回复里有工具调用，调工具；没有，结束回合，`completed`。

| `model.called` 的格 | 写什么 |
|---|---|
| `seen` | 这次请求的 `seen` |
| `endpoint`、`model`、`request` | 发出去了的才有：`RequestSent` 报的 |
| `messages` | 统一的请求里有几条消息 |
| `first_difference` | `CallModel` 的 `changed` |
| `usage` | `ModelEnded` 带的；打断的没有 |
| `first_token_ms` | 发出去到第一段增量；没发出去、一段增量都没来的没有。时钟往回拨了算 0 |
| `duration_ms` | 发出去到说完（或者打断）；没发出去的没有。时钟往回拨了算 0 |
| `result` | `interrupted` 被打断，`error` 出错，别的 `ok` |
| `error` | 出错的分类和原话 |
| `compaction` | 摘要请求的：哪一种压缩（现在只有 `auto`）；主请求没有 |

**出错再来**：

1. 可以再来的分类：`retryable`、`rate_limited`、`bad_stream`、`empty_reply`。`context_too_long`、`auth`、`content_policy`、`other`、不认识的，不再来。
2. 这一步已经连着再来的不到 5 次（`RETRY_LIMIT`）才再来：最多再来 5 次，第 6 次出错结束回合。
3. 等多久：带着 `wait_ms` 的照它；没带的，第 1 到第 5 次各等 1、2、4、8、16 秒（`backoff`）。要等的超过 120000 毫秒（`WAIT_LIMIT_MS`），不再来，结束回合，`error`。
4. 收到了半截、写成了回复的，后面追加一条事实 `reply_cut`（`by` 是内核，`cause` 是回合的），每次都追加，不和以前的比。
5. 回合停在 `Waiting`，出 `Append`、一条瞬时的 `status`（`by` 是内核，`cause` 是回合的，`body` 是 `seen` 和 `retry`：第几次、上限 5、等多少毫秒、分类、原话）、`Wake`（这一刻加上要等的毫秒；超出能写的时刻，就是这一刻）。
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

1. 回合进行中来的消息带着这个回合，记在这一轮的队里，下一次请求里就有它；发请求时队清空。
2. 回合结束时队里还有的，同一批接着开下一轮：由最后那一条触发，`cause` 是它的；`at` 是结束上一轮的那条输入的时刻。走完、出错、到上限、打断时接着发，都一样。
3. 打断时退回的：追加 `message.withdrawn`，列出队里的，照先后，`by` 是打断的人，`cause` 是打断的命令，排在 `turn.ended` 前面；队里没有的，不追加。退回以后不接着开。
4. 有计划的重启、崩了收尾，不接着开（「有计划的重启」「载入和崩溃」）。

**打断**（`Interrupt`）：

1. 没有回合在进行：拒绝，`not_running`。
2. 请求在路上：收到的半截只留收全了的工具调用，发出去了、不是空的才写成回复（`"interrupted":true`）；`model.called` 的 `result` 是 `interrupted`。回复里留下的调用各补「已取消，没跑过」（`cancelled-before`）。出 `CancelModel`。半截回复和 `model.called` 的 `cause` 是回合的。
3. 调工具：还没有结果的：
   1. 派出去了的、改文件的（访问类别 `write`）：叫它停（`StopTool`），先不记结果，改成「停着」（第 7 条）。
   2. 派出去了的别的、答完了等落盘的：补「已取消，跑到一半」（`cancelled-running`），出 `CancelTool`。
   3. 问着人的：补 `question-interrupted`，出 `CancelTool`。
   4. 还没派的（等着的、在过链的、等人确认的、允许了还没派的）：补「已取消，没跑过」。
4. 别的阶段：什么都没发出去，直接结束。之后迟到的挂接点结果、`Woke` 不理。开头还没落盘就被打断的，回合开始的挂接点不再跑，回合结束的照样跑。
5. 收尾（没有停着的就当场收尾，有的等第 7 条）：`queued` 是退回的，先撤回排着的（「排队的消息」第 3 条）。然后 `turn.ended`（`interrupted`）；接着发的，队里有的接着开下一轮。
6. 补的结果、撤回、`turn.ended`，`by` 是打断的人，`cause` 是打断的命令。回应附上这一次追加的全部，接着开的下一轮的也在里面；有停着的，回应在第一批落了盘就回，之后追加的 `cause` 也是打断的命令。叫停的动作排在 `Append` 后面。
7. **停着的**：这一轮先不结束，等它们交回来，最多 10 秒（出 `Wake`，到点送回 `Woke`）：
   1. 交回来停在改之前的（`stopped`）：补「已取消，跑到一半」，`by` 是打断的人，`cause` 是打断的命令。
   2. 交回来已经改完的：照工具交的记，和平常的结果一样（`ok`、`error`，带效果），`by` 是那次调用。
   3. 都交回来了，收尾（第 5 条）。
   4. 到了 10 秒，或者又打断了一次：还没交回来的补「已取消，跑到一半」，出 `CancelTool`，收尾；之后才到的不理。
   5. 等着的时候来的消息照排队；撤销、恢复照「有回合在进行」拒绝。

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
6. **查事实**：回合进行中切过的，下一次请求之前查一遍，就是挂接点跑完、这一步齐了、重试到点的时候；切的时候正在 `Ready` 的，当场查。查的时候实际生效的换成现在的；环境那一块写这一轮的工作目录和会话现在的时区；和最近一块一样的不追加，来回切了一圈的也就不追加。回合之间切的，下一个回合开始时查。

**有计划的重启**（`Restarting`）：

1. 没有回合在进行，什么都不做。
2. 在等停着的（「打断」第 7 条）：那次打断照样算数，先照第 7 条第 4 款收尾，不等了；不然这一轮以 `restarted` 结束，再起来会接着干。收尾时接着开了下一轮的，再照下面收拾那一轮；没开的，到这里为止。
3. 照打断收拾：请求在路上的截下半截（「打断」第 2 条），出 `CancelModel`；在跑的、问着人的、答完了等落盘的出 `CancelTool`，不等（停着的已经照第 2 条掐掉了）。还没有结果的调用，包括半截回复里留下的，都补 `cancelled`，那一句是 `restarted`。
4. `turn.ended` 的原因是 `restarted`。`by` 都是内核，`cause` 是回合的。排着的不接着开。

**载入和崩溃**（`Session::load`）：

1. 日志一条条过账本、进有效历史：一条都没有，`LoadError::Empty`；有一条过不了，`LoadError::Broken`，写明第几条、违反了哪一条。读进来的都算落了盘。
2. 现在的权限：`session.created` 的，被后来带 `permission` 的 `session.policy_changed` 盖掉；实际生效的就是它。
3. 最近的命令编号：每个 `cause` 和 `cause` 是它的那几条，照编号第一次出现的先后记进去，多过 1024 个的留后面的。同一个编号再来，回应附上这些，和当时的不一定一样。没产生事件的（切成和当时一样的级别）不在里面，再来重新判。
4. **崩了**：日志停在一个没结束的回合里。还没有结果的调用照编号的先后各补 `cancelled`（`restarted`），再追加 `turn.ended`（`aborted`）；`by` 是内核，`cause` 是这一轮 `turn.started` 的，时刻是载入的 `at`。在等的确认、题跟着了结。不接着开，等人开口；排着的留在日志里。
5. **接着干**：最后结束的一轮是 `restarted`，就自动开一轮：由那时排着的最后一条触发，`cause` 是它的；没有排着的，由那条 `turn.ended` 触发，`cause` 是它的。
6. 连着被重启打断的轮数超过 `resumes`（3）的，不接：接着干开的那一轮再被打断，接着数；别的回合一开，从 0 数。最后结束的那一轮以后撤销过的（`turn.reverted`），不接。
7. 补的、开的事件在返回的一个 `Append` 里；没有要补的，不出动作。
8. 改回文件做到一半停了的（日志里有撤销、恢复，没有 `files.restored`），不补、不重做。

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
| `empty_reply` | 回复里一个块都没有 |

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
| `crates/miyu-kernel/src/session/tests/idle.rs` | 有回合、`turn.ended` 没落盘都不算空闲（改回文件时不算空闲在 `session/tests/restore.rs`，`history.md`） |
| `crates/miyu-kernel/src/session/tests/turn.rs` | 空闲时消息和回合的开头同一批；挂接点等开头落盘；挂接点跑完才请求；注入照交回的先后；中途的消息并进这一轮；挂接点跑的时候来的消息，落了盘才请求；对不上的挂接点结果不理；报的最后一个环境才注入 |
| `crates/miyu-kernel/src/session/tests/dirs.rs` | 加进来的目录（施工 5-10 上）：`turn.started` 带着它、没有就不写这一格；判权限、派工具都带上；回合中途报来的下一轮才用 |
| `crates/miyu-kernel/src/session/tests/reply.rs` | 一整轮；`model.called` 的每一格；下一轮只注入变了的；不再来的错结束回合、留半截；出错的半截里收全的调用也不留；等一会儿再来；没发出去的没有端点和用时；执行器违约按出错算；过时的回报不理；私有数据留在回复里不推；有工具调用的回合不结束 |
| `crates/miyu-kernel/src/session/tests/difference.rs` | 只是接着加的没有第一处不同；改了 system 的是第一处不同 |
| `crates/miyu-kernel/src/session/tests/tools.rs` | 一步跑完再请求；非只读的一个一个来；没有的工具、坏参数当场回；修正只用在执行上；步数上限在最后一步跑完后结束；推工具的输出；对不上的结果不理；回合带着开始时的工作目录 |
| `crates/miyu-kernel/src/session/tests/queue.rs` | 最后一步里来的开下一轮；由最后一条触发；出错、到上限的也接着开；被后一步听到的不再开；打断接着发、退回；没排着的不写撤回；触发不算排队 |
| `crates/miyu-kernel/src/session/tests/interrupt.rs` | 空闲时打断被拒；请求前、请求中、调工具时打断；什么都没收到不写回复；急着插话的三种时候；空闲时急着插话开回合；在跑的写叫它停、排在后面的当场补、10 秒以后叫醒、改完了的带着效果记、收了尾到点不理 |
| `crates/miyu-kernel/src/session/tests/permission.rs` | 空闲时切、切成一样的、不认识的级别；收紧成只读拦下回复里的、这一步里等着的写入；放宽等下一次请求；来回切不注入；挂接点前后切；收紧成工作区什么都不拦；只读下改常用的那一级；事实写这一轮的工作目录 |
| `crates/miyu-kernel/src/session/tests/restart.rs` | 重启照打断收拾；接着干；排着的由最后一条开；连着 4 次不接；走完一轮、你开口以后从头数 |
| `crates/miyu-kernel/src/session/tests/load.rs` | 走完的载入一样往下走；坏日志拒绝；崩在哪都收尾、等你开口；崩之前的命令不再生效；生效的权限回来 |
| `crates/miyu-kernel/src/session/tests/scenario.rs`、`scenario/retrying.rs`、`scenario/stopping.rs` | 执行器替身（`testkit`）把真会话一整轮一整轮地跑：两个读一起跑、中间来一句；只读拦写入；步数上限和失败的请求；重试的每一种（原样再来、半截接着说、半截的调用丢掉、照供应商等、5 次放弃、不该再来的、等的时候打断、重启、切级别、不算步数、说完清零）；打断接着发、重启接着干、崩了等你；停着的写：停在改之前、改完了、到 10 秒、又打断一次、等的时候来的消息排队和撤销被拒、等的时候重启（退回的不再接着干，接着发的交给下一轮） |
| `crates/miyu-kernel/src/session/tests/random.rs` 和 `random/` | 三百例随机输入（CI 另跑两万例），每一步查：不变量（第 5 条一个会话查不了）；挂接点、请求、派工具、步数上限、只读的规矩；打断时停着的（叫它停只在打断里、停着的交回来才收尾、到点和又打断就不等，十个种子里一个多调写文件的专走这里）；崩了、重启了载入以后照规矩走；每条路、每一种输入都走到过 |
| `crates/miyu-kernel/src/tool/tests.rs`、`tool/texts/tests.rs` | 参数修正的每一种，嵌套的对象、数组里的也修；访问类别不认识的算写入；那几句的字段转义、每句带说法 |
| `crates/miyu-kernel/src/accumulate/tests.rs` | 拼回复、调用编号、空块、交错的字、截断只留收全的调用、增量对不上的六种 |
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
- `26-提示词.md` J3（内核写的英文只说发生了什么）、第十节（登记簿）。

### 还没有的

- 别的挂接点：命令进入、模型输出后、工具执行后、订阅（`05-内核接口.md` 第五节）。现在只有回合开始、回合结束和执行前的链；回合开始、回合结束还没有模块挂，会话 actor 交回空的注入（`crates/miyu-session/src/actor.rs`）。
- 换策略快照（`02-内核.md` K3）：内核不写带 `policy` 的 `session.policy_changed`，载入时也不看它。改标题、置顶（`session.meta_changed`）没有命令。
- 压缩（`compaction.md`）：`context_too_long` 现在结束回合，不先压缩（被动压缩，6-7）；截掉最老的再试、隔离式回退（6-6 下）；手动压缩（6-8）；撤销越过压缩（6-9）。
- 子代理、后台命令（`02-内核.md` 第七节，M7）：由回报开的回合，撤销时一起停下，载入时给它们补中断。
- 等第一个字时的心跳 `status`（`03-事件模型.md` 第五节）；中途连上的头拿「到目前为止的内容」（M8）。
- 协议上还没有 `session.set_permission_level`、`session.answer`（`04-核心协议.md` 第九节）：内核有这两个命令，核心还不收。
- 没人盯着的场所的步数上限，随预设定（`02-内核.md` 第六节「工具怎么调、下一步怎么走」第 6 条）。
