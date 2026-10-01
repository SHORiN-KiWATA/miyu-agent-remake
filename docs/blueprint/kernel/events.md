## 事件

### 是什么

事件是已经发生的一件事，追加进会话的日志，一条一行 JSON，以后不改、不删；撤销、压缩也是追加一条新的。内核认识 23 种，每一种有自己的 `body`；不认识的原样留着。另有四种瞬时事件，只推给连着的头，不进日志。

这一页写外壳、一行怎么读写、有哪些种类、瞬时事件、格式出错。每一种 `body` 的每一格见 `kernel/events-bodies.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/event.rs` | 外壳 `Event`；种类表 `Body`（宏 `bodies!`，加一种只加一行）；`Body::KINDS`、`Body::kind`；一行怎么读写 |
| `crates/miyu-kernel/src/event/session.rs`、`turn.rs`、`restore.rs`、`message.rs`、`tool.rs`、`question.rs`、`context.rs`、`model.rs`、`effect.rs`、`job.rs`、`peer.rs` | 各种 `body`（`kernel/events-bodies.md`） |
| `crates/miyu-kernel/src/event/transient.rs` | 瞬时事件：外壳 `Transient` 和四种 `body` |
| `crates/miyu-kernel/src/format_error.rs` | 编号、名字、时刻写法不对时的报错 `FormatError` |
| `docs/designs/samples/events/`、`docs/designs/samples/transient/` | 样本：每一种一份 |

外壳里的几种写法（序号、时刻、种类、`by`、命令编号）见 `kernel/ids.md`，内容块见 `kernel/blocks.md`。追加一条之前照账本查的规矩（序号接不接得上、`turn` 对不对、调用对不对得上）见 `kernel/history.md`，这一页只管写法。

### 对外的样子

**外壳** `Event`：

| 格 | 类型 | JSON 里 | 是什么 |
|---|---|---|---|
| `seq` | 序号 | 必有 | 会话内第几条，从 1 起，一条接一条，内核追加时给 |
| `at` | 时刻 | 必有 | 发生的时刻，取自执行器送进来的那条输入 |
| `kind` | 事件种类 | 必有 | 这一条是哪一种；代码里没有这一格，由 `body` 的种类写出 |
| `turn` | 回合编号 | 可以没有 | 所属回合。`turn.started` 写它自己的序号；回合进行中造的，写那个回合；别的不写 |
| `by` | 「谁」 | 必有 | 这件事由谁引起，取自连接，不取自正文 |
| `cause` | 命令编号 | 可以没有 | 引起它的命令，用于去重和追踪 |
| `body` | 这一种的内容 | 必有 | `kernel/events-bodies.md` |

- `Event::to_line()`：写成一行，不带换行。换行由存日志的那一层加（`store.md`）。
- `Event::from_line(line)`：读一行，交回事件或 serde_json 的错误。
- `Body::kind()`：外壳里 `kind` 那一格写的名字；不认识的种类，是读到的那个名字。
- `Body::KINDS`：内核认识的全部种类名，照下表的先后，样本测试拿它查每一种都有样本。

**种类**：照 `Body::KINDS` 的先后。

| 种类 | 是什么 | `by` | `turn` | 样本 |
|---|---|---|---|---|
| `session.created` | 会话创建 | 造会话的人 | 不带：它是第 1 条 | `session.created.jsonl` |
| `session.policy_changed` | 换了策略快照，或者换了权限 | 切权限的人 | 回合进行中切的带上 | `session.policy_changed.jsonl` |
| `session.meta_changed` | 改了标题、置顶；内核自己起的标题也是它（施工 3-8 五补，`kernel/session.md`「起标题」） | 改的人；内核起的是内核 | 人在回合进行中改的带上；内核起的不带（它不属于哪一轮） | `session.meta_changed.jsonl`；内核起的样子见探针存档 `docs/designs/samples/probe/title/log.jsonl` |
| `session.recapped` | 一句回顾（施工 3-8 四补，`kernel/session.md`「回顾」）：推给头，不进上下文 | 内核 | 不带：它不属于哪一轮，撤哪一轮都不会跟着拿走（和回报一样） | `session.recapped.jsonl` |
| `turn.started` | 回合开始 | 内核 | 它自己的序号 | `turn.started.jsonl` |
| `turn.ended` | 回合结束 | 内核；被打断的，是打断的人 | 必带 | `turn.ended.jsonl` |
| `turn.reverted` | 撤销了几个回合 | 撤销的人 | 不带：有回合在进行时撤不了 | `turn.reverted.jsonl` |
| `turn.unreverted` | 恢复了最近一次撤销的回合 | 恢复的人 | 不带：撤了以后开过回合就恢复不了 | `turn.unreverted.jsonl` |
| `files.restored` | 撤销、恢复时改回文件的结局 | 和那条撤销、恢复一样 | 不带 | `files.restored.jsonl` |
| `message.user` | 人发来的消息，或者另一个会话发来的消息 | 发消息的 | 回合进行中来的带上：排着队 | `message.user.jsonl` |
| `message.assistant` | 模型一次响应的完整内容，工具调用也在里面 | 模型 | 必带 | `message.assistant.jsonl` |
| `message.withdrawn` | 撤回排着队、她还没听到的消息 | 打断的人 | 必带 | `message.withdrawn.jsonl` |
| `tool.result` | 一个工具调用的结果 | 那次调用、内核、人或者模块（`kernel/tools.md`） | 必带 | `tool.result.jsonl` |
| `tool.approval_requested` | 请人确认一次工具调用 | 提问的模块 | 必带 | `tool.approval_requested.jsonl` |
| `tool.approval_decided` | 人对确认请求的决定 | 回答的人 | 必带 | `tool.approval_decided.jsonl` |
| `question.asked` | 一个在跑的调用请人回答一组题 | 那次调用 | 必带 | `question.asked.jsonl` |
| `question.answered` | 人对一组题的回答 | 回答的人 | 必带 | `question.answered.jsonl` |
| `context.injected` | 注入进上下文的一块事实 | 内核；回合开始的挂接点交回来的，是交它的模块（现在的执行器一块都不交） | 回合进行中注入的带上 | `context.injected.jsonl` |
| `context.compacted` | 压缩的检查点 | 内核 | 带上：压缩发生在哪一轮 | `context.compacted.jsonl` |
| `context.compaction_paused` | 暂停了自动压缩（施工 6-6 上） | 内核 | 必带 | `context.compaction_paused.jsonl` |
| `model.called` | 一次模型请求的记录 | 内核 | 回合进行中的带上；回顾、起标题的请求不带（施工 3-8 四补、五补） | `model.called.jsonl` |
| `job.reported` | 后台命令结束了（施工 7-1） | 内核，`by` 照原因记（施工 7-2，`kernel/session.md`「回报」第 2 条） | 内核记的不带（2026-09-30 定），账本不另查 | `job.reported.jsonl` |
| `child.reported` | 子会话的回报（施工 7-1） | 内核，`by` 是那个子会话，账本查（施工 7-2） | 内核记的不带（2026-09-30 定），账本不另查 | `child.reported.jsonl` |
| `peer.idle` | 等的那个会话空下来了，或者等不到了（施工 C-1，`cross-session.md`） | `idle` 的是那个会话，`expired`、`gone` 的是内核，账本查 | 不带：别处来的，撤哪一轮都不拿走；账本不另查 | `peer.idle.jsonl` |

- 「—」是现在还没有哪里写这一种：读得懂、账本查得了、投影认得，就是不产生（下面「还没有的」）。
- `turn` 那一列的「必带」「它自己的序号」「不带」，账本在追加时查：`turn.started` 的 `turn` 要是它自己的序号；带 `turn` 的要是正在进行的那个回合；「必带」的九种不带就不收；`turn.reverted`、`files.restored` 在有回合进行时不收（`kernel/history.md`）。
- 模块自己的种类写成 `ext.<模块>.<种类>`，内核不认识，照不认识的种类处理。
- `job.reported`、`child.reported` 渲染成带标签的事实（施工 7-2，`kernel/request.md`「回报」）。
- `peer.idle` 由被等的会话交来的命令 `PeerIdle`、执行器交的 `WatchEnded` 写，空下来了的叫醒她，渲染成带标签的一块（施工 C-6，`cross-session.md` 第六条、第八条，`kernel/session.md`「空了的通知」）。

**瞬时事件** `Transient`：外壳和持久事件同一种写法，只少了 `seq`，它不进日志。`cause` 留着：一个命令引起的事，从持久的到瞬时的，一路追得下去。

| 格 | 类型 | JSON 里 |
|---|---|---|
| `at` | 时刻 | 必有 |
| `kind` | `model.delta`、`tool.progress`、`status`、`compaction.progress`、`compaction.done` 五种之一 | 必有 |
| `turn` | 回合编号 | 没有就不写 |
| `by` | 「谁」 | 必有 |
| `cause` | 命令编号 | 没有就不写 |
| `body` | 下面五种之一 | 必有 |

- `Transient::to_line()`：写成推给头的一行，不带换行。
- 只写不读：内核只推。读回来的那一半，做到头读它们的时候再写（M8）。

| 种类 | `body` | `by` |
|---|---|---|
| `model.delta` | 模型输出的一段增量：`seen` 这次请求看到了第几条为止，和这次响应最后写成的回复的 `seen` 一样；`index` 第几块，从 0 数起；再加下面五种写法之一 | 模型 |
| `tool.progress` | 工具执行中的一段输出：`call_id` 哪一次调用，`text` 一段输出。结果以 `tool.result` 为准，这些只给人看着它在跑 | 那次调用 |
| `status` | 出了错，等着重试：`seen` 哪一次请求；`retry` 里 `attempt` 这是第几次重试（从 1 数起）、`limit` 一共最多几次（现在是 5，`kernel/session.md`）、`wait_ms` 等多久（毫秒）、`class` 出错的分类、`message` 出错的原话、`status` 出错的 HTTP 状态码（照那一次的 `model.called` 带过来，没有的不写；施工 3-5 三补） | 内核 |
| `compaction.progress` | 摘要写到哪了（施工 6-2 上）：`seen` 哪一次摘要请求（它替代到的那一条）、`written` 到这时收到的正文字数（草稿加摘要，照 Unicode 字符数）、`expected` 估计要写多少字（压缩前的用量，夹在 20000 到 80000 之间） | 内核 |
| `compaction.done` | 压好了（施工 6-3 下）：`seen` 哪一次摘要请求；`trigger` 哪一种压缩，`auto`、`manual`，和那一条 `context.compacted` 一样（施工 6-8：运行日志照它写）；`before` 压之前的用量（自动的是过了线的那一次主请求算出的，手动的是那一轮开头落了盘时照有效历史组装一次算的）、`after` 压完的用量（照这时的有效历史组装一次算的），都是估算，和压缩线同一个算法；`usage` 摘要请求的用量、`duration_ms` 它的用时，照它的 `model.called`，没有就不写 | 内核 |

`model.delta` 的那一段增量：

| 增量 | 写成 |
|---|---|
| 一块开始了：正文 | `"start":"text"` |
| 一块开始了：思考 | `"start":"reasoning"` |
| 一块开始了：工具调用 | `"start":"tool_call","name":<工具名>` |
| 这一块的一段字 | `"text":<那一段>` |
| 这一块收全了 | `"end":true` |

驱动私有数据那一种增量不推：头用不着，只进最后的那条回复。

### 怎么走

**写一行**：

1. 紧凑的 JSON，不加空格，不带换行。
2. 字段的顺序固定：`seq`、`at`、`kind`、`turn`、`by`、`cause`、`body`。瞬时事件是 `at`、`kind`、`turn`、`by`、`cause`、`body`。
3. `turn`、`cause` 没有就不写，从不写 `null`。
4. `body` 只写它自己的内容，种类写在外壳的 `kind` 里。
5. 实际写不出来的情况没有：里面只有字符串、数字和原样的 JSON（`to_line` 里的 `expect`）。

**读一行**：

6. `seq`、`at`、`kind`、`by`、`body` 缺了哪一格，报错。
7. `turn`、`cause` 没有、写成 `null`，都当没有。
8. 外壳里同一格写了两次，报错。
9. 外壳上多出来、不认识的格，不管：读进内存时丢掉，写出去不再有。内存里的事件不带原文，原文在日志文件里（`store.md`）。
10. `body` 先原样读下来，看过 `kind` 再照那一种读。
11. 认识的种类，`body` 照那一种读；读不出来就是坏数据，报错写明是哪一种的 `body`，不当成不认识的。`body` 里多出来的格同样不管。
12. 不认识的种类，包括不认识的 `ext.*`：`Body::Unknown`，记着种类名和原样的 `body`，写出去一字不差，空格都不变。投影跳过它（`crates/miyu-assemble/src/render.rs`，`kernel/request.md`）。
13. 内核自己写出去的每一行，读回来再写出去一字不差：样本测试守着。
14. 这一层只查写法。时刻的先后不查：时钟可能往回拨。别的规矩在追加时由账本查（`kernel/history.md`）。

**瞬时事件**：

15. `model.delta`：驱动交来的增量，照收到的先后一段推一条。私有数据不推；对不上的（累积器不收的）不推，这次请求按出错算（`kernel/session.md`）。`index` 是驱动给的块编号，`seen` 是这次请求的。
16. `tool.progress` 只推在跑的调用的；不是这一步在跑的，不推（`kernel/session.md`）。
17. `status` 在一次请求出了可以重试的错、要等一会儿再试时推一条（`kernel/session.md`）。
18. `compaction.progress` 在摘要请求报发出去时先推一条 `written` 是 0 的（施工 6-3 下：头一收到就能印「正在压缩」），之后正文块每来一段推一条；摘要请求不推 `model.delta`（`compaction.md` 第三条第 8 条）。
19. `compaction.done` 在取到摘要、写下 `context.compacted` 的同时推一条；压缩中途被打断、出错的不推（`compaction.md` 第三条第 11 条）。

### 样子

一条事件的样子，就是日志里的那一行。样本：

- `docs/designs/samples/events/<种类>.jsonl`：内核认识的每一种一份，文件名是种类名加 `.jsonl`。内容就是日志里的那几行，这一种在样本会话里出现几次就写几行，以一个换行结尾，没有空行。几份样本讲的是同一个会话：序号不重复，时刻跟着序号不往回走。只有一条例外：带 `parent` 的那一条 `session.created` 是它派的子代理的会话日志里的第 1 条（施工 7-1），把样本当一个会话用的测试都跳过它。样本会话在 126、134 号订了两个别的会话的「空了告诉我」（`tool.result` 的效果 `peer.watch`），130 号等到了第一个空下来，138 号第二个 12 小时没等到、作废（施工 C-1；排在回顾的 121、122 号后面）；给她看的那两句、给人看的说法照 `cross-session.md`「样子」写（施工 C-6 定了）。
- `docs/designs/samples/transient/model.delta.jsonl`、`tool.progress.jsonl`、`status.jsonl`、`compaction.progress.jsonl`、`compaction.done.jsonl`：样本会话里 44 号请求的回复一段段推给头的样子、那次 `read` 执行中的一段输出、44 号请求出了限速的错等 1 秒再试（第二条是 117 号请求的限速，带着 429，和 118 号 `model.called` 对得上；44 号那一条写在施工 3-5 下，还没有 `status` 那一格）、54 号压缩写摘要时的两段进度、一次压好了（81 万压到 3 万）。瞬时事件内核不读，测试在代码里照着造，写出去和样本一字不差。

### 出错

读一行出错，`Event::from_line` 交回 serde_json 的错误。原话里有：

| 哪里错 | 原话里有 |
|---|---|
| 不是 JSON | serde_json 的原话 |
| 缺了外壳的一格 | `missing field` 和那一格的名字，例如「missing field `seq`」 |
| 外壳的同一格写了两次 | `duplicate field` 和那一格的名字 |
| 序号、回合编号是 0 | 「bad seq: starts at 1 (got "0")」 |
| 时刻、种类、命令编号不合写法 | `FormatError` 的那一句，例如「bad time: …」「bad event kind: at least two parts separated by dots (got "message")」 |
| `by` 坏了 | 「missing field `kind`」、`FormatError` 的那一句，或者 serde_json 的原话（`kernel/ids.md` 第 19 条） |
| 认识的种类，`body` 读不出来 | 「body of <种类> not readable: <serde_json 的原话>」，例如「body of message.user not readable: missing field `blocks` …」 |

serde_json 在每一句后面加上 ` at line <几> column <几>`（没有测试证实）。

**`FormatError`**：编号、名字、时刻读不进来时报的（`kernel/ids.md` 列了每一种报的话）。

| 格 | 是什么 |
|---|---|
| `what` | 读的是什么，例如 `session id`、`seq`、`call id`、`time` |
| `text` | 读到的原文。超过 80 个字符的，只留前 80 个，后面加 `…` |
| `why` | 错在哪，例如 `must be 36 characters` |

- 写成一句：`bad <what>: <why> (got <text>)`，`text` 带着引号，照 Rust 的调试写法转义（换行写成 `\n`，引号写成 `\"`）。例：`bad session id: must be 36 characters (got "x")`。
- 从 `miyu_kernel::FormatError` 拿得到。在 JSON 里读的时候，它成了 serde_json 的报错。
- 报错是英文，给查问题的人看，写进运行日志（`28-运行日志.md` LG1；施工 4-9 再补四中：原来是中文）。要报给模型的错另写（`26-提示词.md` J3），不把它原样转给模型。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/event/tests.rs` | 图纸上的两行读写一字不差（`lines_from_the_drawing_round_trip`）；认识的读成对应的类型；第 12 条（`an_unknown_kind_keeps_its_body_byte_for_byte`）；第 2 条字段顺序（`fields_are_written_in_the_drawing_order`）；第 3、7 条（`optional_fields_missing_or_null_read_as_absent`）；第 9 条（`new_fields_on_the_envelope_are_ignored`）；「出错」表里的几种（`broken_lines_say_what_is_wrong`） |
| `crates/miyu-kernel/src/event/transient/tests.rs` | 图纸上的那一行照写；四样增量各自的写法；没有 `turn`、`cause` 的不写；`tool.progress` 的写法 |
| `crates/miyu-kernel/tests/samples.rs` | 第 13 条：每一份样本的每一行读写一字不差、认得出种类、种类和文件名对得上（`every_sample_round_trips_as_its_own_kind`）；认识的每一种都有样本（`every_known_kind_has_a_sample`）；几份样本讲同一个会话，序号不重复、时刻不往回走（`samples_tell_one_session_in_order`）；子代理的样本对得上：回报的会话、`by` 就是派它的 `job.started` 记的，子会话的第一条带着父会话、第 1 层（`the_child_in_the_samples_is_the_one_the_parent_started`，施工 7-1）；空了的通知对得上：等的会话前面订过，`idle` 的 `by` 是它、作废的是内核（`the_notices_in_the_samples_answer_the_watches`，施工 C-1） |
| `crates/miyu-kernel/tests/transient_sample.rs` | 三份瞬时样本在代码里照着造、写出去一字不差；推给头的几段增量交给累积器，拼出来的就是日志里 45 号回复的内容块 |
| `crates/miyu-kernel/src/test_support.rs` 的 `read_body`，各种 `body` 的测试都用它 | 每一种读写一字不差、认得出种类 |
| `crates/miyu-kernel/src/id/tests.rs` 的 `error_says_what_why_and_what_was_read`、`long_text_in_errors_is_cut` | `FormatError` 那一句的样子、80 个字符 |

### 出处

- `03-事件模型.md` 第一节：三条总则。
- `03-事件模型.md` 第二节：公共字段、「外壳的写法」。
- `03-事件模型.md` 第三节：事件种类，持久与瞬时怎么分，「样本文件」。
- `03-事件模型.md` 第五节「瞬时事件的外壳」：外壳少了 `seq`，`model.delta`、`tool.progress`、`status` 的 `body`。
- `03-事件模型.md` 第八节：格式演进，日志存原文。
- `03-事件模型.md` E3（用 JSON）、E6（模块自己的种类带命名空间）。
- `02-内核.md` 第九节「日志追加时查的规矩」：追加时照账本查，只查日志自己，不查时刻的先后。

### 还没有的

- `job.reported`、`child.reported`：内核收得下、渲染得出（施工 7-2），子会话交来 `child.reported`（施工 7-6），还没有真的执行器交来 `job.reported`（7-3）。原来的 `child.spawned` 不做了：派它的那次调用的效果 `job.started` 就是开始的记录（`03-事件模型.md` 第三节）。
- `session.policy_changed` 只写过换权限；换策略快照（目录变了、配置改了，下一个回合开始时换）还没有（`05-内核接口.md` 第八节，`02-内核.md` K3）。
- 模块自己的事件种类 `ext.*`：还没有模块定义（E6）。
- `status` 的别的状态，例如等第一个字时的心跳（`03-事件模型.md` 第五节）。
- 中途连上的头先拿「到目前为止的内容」：做视图投影时加（`03-事件模型.md` 第五节，M8）。
- 隐私抹除：清空某几条的内容、留下墓碑，是存储层的事（`03-事件模型.md` 第七节）。
