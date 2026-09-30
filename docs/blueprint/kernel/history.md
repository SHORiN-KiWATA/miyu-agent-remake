## 账本、有效历史、撤销和恢复

### 是什么

日志只追加。每一条事件追加之前，先交给账本照规矩查一遍：新写的和从磁盘载入的走同一条路，违反的不追加。查过的交给有效历史：投影要用的那一段，从最近一次压缩算起，去掉撤销掉的回合、撤回的消息，照每次请求当时看到的样子排好。

撤销、恢复也是追加一条事件。撤掉的那几轮改过文件的，内核照效果算出改回的几步，交给执行器，结局记成一条 `files.restored`。撤掉的那几轮派出去、还在跑的任务，内核交给执行器停下，回报记 `undone`（施工 7-8）。

撤销能撤掉压缩：压缩跟着它所在的回合撤掉，有效历史回到前一次还算数的压缩。更早的那一段不在内存里，内核叫执行器从磁盘读回来，照它重建有效历史；恢复把撤掉的压缩放回来，不读磁盘（施工 6-9，`compaction.md` 第十一条）。

重做是一个命令做完的撤销加重发（施工 4-7 再补）：照撤销的规矩撤掉最后一轮，再把开它的那几句人的话原样（或者开这一轮的那一句换成改过的）追加成新的消息，开新的一轮（下面「重做」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/ledger.rs` | 账本：查规矩、记下变化 |
| `crates/miyu-kernel/src/ledger/undo.rs` | 账本里撤销、恢复的几条：撤的是哪几轮、能不能恢复 |
| `crates/miyu-kernel/src/ledger/jobs.rs` | 账本里任务的几条：编号不重复、回报对得上派出去的任务、子会话的 `parent`、`depth`（施工 7-1） |
| `crates/miyu-kernel/src/history.rs` | 有效历史：收事件、压缩、撤回、排先后、落到检查点上 |
| `crates/miyu-kernel/src/history/undo.rs` | 撤掉的拿走、放回；跟着撤的话 |
| `crates/miyu-kernel/src/history/jobs.rs` | 派出去过的任务：标题、种类、派它的那一轮撤掉了没有（施工 7-2）；在哪几轮里派的（施工 7-8） |
| `crates/miyu-kernel/src/session/jobs.rs` 的 `stop_undone` | 撤销时停哪几个（施工 7-8）：撤掉的那几轮里派的、还在跑的 |
| `crates/miyu-kernel/src/session/revert.rs` | 撤销、恢复两个命令，改回文件的来回，撤掉压缩时读回日志的来回，取回重读的原文 |
| `crates/miyu-kernel/src/session/redo.rs` | 重做（施工 4-7 再补）：能不能重做，撤销记下以后重发开那一轮的话、开新的一轮 |
| `crates/miyu-kernel/src/session/load.rs` | 载入：整份过账本，从还算数的最近一次压缩起重建有效历史（`kernel/session.md`） |
| `crates/miyu-kernel/src/session/restore.rs` | 改回的几步怎么算 |
| `crates/miyu-kernel/src/event/restore.rs` | `files.restored` 的每一格 |

### 对外的样子

**账本**（`Ledger`）：`Ledger::default()` 是一个还没有事件的会话。只记查规矩要用的几样，不留事件本身：下一条的序号、正在进行的回合、上一条回复的序号、这一轮还没有结果的调用和其中在等确认的、在等回答的、排着队的消息、开过还没撤掉的回合（压缩以前的也在）、还算数的几次压缩各在哪一轮、替代到哪、还能恢复的几次撤销各撤了哪几轮和跟着撤掉的压缩、派出去过的任务（施工 7-1：编号，是后台命令还是子代理，子代理的会话，后台命令结束了没有，子代理还会不会再报；撤掉的回合里派的也在）。

- 从账本读的两样（施工 7-3）：`last_job()` 派出去过的任务编号最后一段最大的数（施工 7-1 补：子会话的编号带着前缀，以前的日志里又有不带的，照整个编号排最大的那个不一定数得最大，`j5.8` 排在 `j7` 前面），撤掉的回合里派的也算，没派过的是 0（`Session::last_job_number` 交给执行器往后数）；`running_commands()` 还没报过结束的后台命令，照编号（载入时补 `aborted`，`kernel/session.md`「载入和崩溃」第 9 条）。施工 7-7 多三样：`waiting_children()` 欠着一份回报的子代理的子会话（一次都没报过的，留了言还没报的）；`subagent_in(会话)` 在那个会话里跑的子代理的编号（认子代理发来的留言，被停掉的、撤掉的回合里派的也认）；`subagents()` 派出去过的子代理：编号、子会话、被停掉了没有。施工 7-8 多一样：`running_jobs()` 还在跑的任务，照编号：还没报过结束的后台命令，没被停掉、欠着一份回报的子代理（一次都没报过的，报过以后又被留了言的）；撤掉的回合里派的也在。撤销停哪几个、检查点里列哪几个、撤销的回应列哪几个都照它。
- 回合的编号一轮一个（8 个字节），压缩一次一项：撤销能撤掉压缩、撤到压缩以前的回合，压缩以前的回合也要记着（施工 6-9）。任务一个一项：编号不回收要看整份日志（施工 7-1）。账本只随回合数、任务数长，不随日志的字节长：十万轮约 0.8 MB，一个活动会话的预算是 5 MB（`23-性能预算.md`，2026-09-29 项目主人定）。
- 还没撤掉的回合照先后排：撤销从某一轮起拿走后面的全部，恢复原样放回，开一轮接在最后。

| 方法 | 交回什么 |
|---|---|
| `append(&event)` | 查过、记下；违反的交回 `LedgerError { seq, why }`，账本不变 |
| `next_seq()` | 下一条该是几号，从 1 起 |
| `open_turn()` | 正在进行的回合，没有就是空闲 |
| `pending_calls()` | 正在进行的回合里还没有结果的调用，照编号的先后 |
| `queued()` | 正在进行的回合里排着队的消息，照先后 |
| `turns_from(turn)` | 还没撤掉的 `turn`，和它以后还没撤掉的每一轮，照先后，压缩以前的也算；`turn` 撤掉了、不是一轮的开头的，没有 |
| `last_turn()` | 还没撤掉的最后一轮 |
| `compacted()` | 还算数的最近一次压缩替代到哪；没有还算数的压缩就没有 |
| `read_back_from(turn)` | 从 `turn` 起撤，会撤掉还算数的压缩（它所在的那一轮不早于 `turn`）的：要从第几条读回，是撤完以后还算数的最近一次压缩替代到的下一条，一次都没有的是第 1 条。撤不到压缩的，没有（施工 6-9） |
| `last_reverted()` | 最近一次还能恢复的撤销撤了哪几轮 |

**有效历史**（`History`）：

| 方法 | 交回什么 |
|---|---|
| `append(event)` | 收一条账本查过的事件 |
| `checkpoint()` | 最近一次压缩的那一条 `context.compacted`；投影里排在最前 |
| `events()` | 检查点之后还有效的事件，照日志的先后 |
| `ordered()` | 同一些事件，照每次请求看到的范围排好 |
| `last_undone()` | 最近一次还能恢复的撤销拿走的事件，照日志的先后 |
| `until(upto)` | 截到第 `upto` 条的有效历史：检查点照留，之后的事件只留第 `upto` 条及以前的，放在一边的撤销不要。压缩的摘要请求照它组装（施工 6-2 上） |
| `recall(texts)`、`recalled(blob)` | 现在这个检查点里重读的文件的原文，照 blob 找：压完时内核照执行器交回的放进来，别的时候照 `Input::Recalled` 放进来（下面「重读的原文」）；换了检查点就清掉。组装时照它取（施工 6-5，`compaction.md` 第九条） |
| `dispatched_in(turns)` | 在这几轮里派出去过的任务的编号，照编号（施工 7-8：撤销这几轮时停掉还在跑的） |
| `dispatched(job)` | 派出去过的任务（`Dispatched`）：`what` 种类、`title` 标题、`session` 子代理的会话（施工 7-7）、`undone` 派它的那一轮撤掉了（下面「派出去过的任务」，施工 7-2）；没派过的没有 |
| `subagent(会话)` | 在那个会话里跑的子代理：编号和它的 `Dispatched`（施工 7-7）。组装照它认出子代理发来的留言；不是这个会话派的子代理的没有 |
| `note(event)` | 只记派出去的任务，不留这一条：载入时重建的那一段以前的事件照它过（施工 7-2） |
| `jobs_from(before)` | 派出去过的任务照 `before` 那一份的：撤掉压缩时换了一份有效历史（施工 7-2） |
| `whole()` | 一份留着一切的：压缩替代掉的不丢，`context.compacted` 自己也照先后留在 `events()` 里，没有检查点；撤销、恢复、撤回照同一套规矩算，撤掉的回合里的压缩跟着拿走。`history` 照它算哪些还算数（施工 6-4，`tools/history.md`）；从日志的一段重建也从它起（施工 6-9） |
| `settle()` | 落到检查点上（下面「落到检查点上」），交回检查点换了没有。留着一切的那一份调过它，就成了平时那一份（施工 6-9） |

**命令**：`Revert { turn }`（`session.revert`，从哪一轮起，`None` 是最后一轮）记 `turn.reverted { turns }`；`Unrevert`（`session.unrevert`）记 `turn.unreverted { turns }`；`Redo { text, attachments }`（`session.redo`，施工 4-7 再补；开这一轮的那一句里的字、附件各换成的块，`None` 是照原来的，两样都是 `None` 的原样重发）记 `turn.reverted { turns }`，再记重发的几句 `message.user` 和新的一轮的开头。拒绝的原因码见下面「撤销」「恢复」「重做」。

**改回文件**：动作 `Restore { steps }`，输入 `Restored { at, files }`。一步是 `Step { result, effect, path, action }`：照第 `result` 条 `tool.result` 的第 `effect` 个效果（从 0 数起），改效果里记的 `path`：

| `StepAction` | 做什么 | 动手之前 |
|---|---|---|
| `Write { expect, content }` | 写回 `content`（blob 的哈希） | 原处是 `expect` |
| `Trash { expect }` | 移进回收站 | 原处是 `expect` |
| `Untrash { from }` | 从回收站的 `from` 移回原处 | 原处空着，`from` 还在 |

`Expect`：`Absent` 空着；`Content(哈希)` 一个文件，内容是这个哈希；`Present` 有东西就行，文件、目录都算。`Step::restored()` 是这一步照做成了的结局：`action` 照这一步，`outcome` 是 `restored`，别的附项都空。

`files.restored` 的一项（`Restored`）：`result`、`effect`、`path`；`action` 是 `write`、`trash`、`untrash`；`outcome` 是 `restored`（改回了，或者已经是要改成的样子），或者没动的 `changed`、`missing`、`occupied`、`gone`、`unsaved`、`unavailable`、`failed`；附项 `found`（`changed` 时现在的哈希）、`trash`（移进回收站成了时的新位置）、`hash`（移回来的是文件时它的哈希）、`error`（`failed` 时系统的原话）。内核只读 `result`、`effect`、`action`、`outcome`、`trash`、`hash`，别的原样记。

### 怎么走

**账本查的规矩**：只看这一条和它之前的日志，不看策略，不看时钟：时刻的先后不查。照下表的先后查，第一条违反的报出来。

| 规矩 | 违反时说的（`why`） |
|---|---|
| 序号是下一个 | seq should be <下一个> |
| 第 1 条是 `session.created` | the first event should be session.created |
| `session.created` 只能是第 1 条 | session.created can only be the first event |
| `turn.started` 的 `turn` 是它自己的序号 | turn.started should have its own seq as turn |
| `turn.started` 时没有别的回合在进行 | turn <编号> has not ended |
| `turn.started` 有 `trigger` 的，`trigger` 在它之前；没有的不查（手动压缩单开的那一轮，施工 6-8） | trigger should be an event before the turn started |
| 带 `turn` 的，是正在进行的那个回合 | turn <编号> is not the running turn |
| `message.assistant`、`tool.result`、`tool.approval_requested`、`tool.approval_decided`、`question.asked`、`question.answered`、`message.withdrawn`、`turn.ended`、`context.compacted` 必须带 `turn`（`context.compacted` 施工 6-9 起：压缩跟着它所在的回合撤） | <种类> happens only in a turn and needs turn |
| `message.assistant` 的 `seen` 在它之前 | seen <n> should come before this reply |
| `seen` 不早于上一条回复 | seen <n> is before the previous reply <n>: a later request always sees the earlier reply |
| 回复里第 k 个工具调用编号是 `call_<这一条的序号>_<k>`，k 从 1 起 | tool call <k> should have id call_<序号>_<k>, got <编号> |
| `tool.result`、`tool.approval_requested`、`question.asked` 对得上这一轮还没有结果的调用 | <编号> is not a call waiting for a result: no such call, or it already has a result |
| `tool.approval_requested` 的调用没有在等的请求 | <编号> already has a pending approval request |
| `tool.approval_decided` 对得上一个在等的请求 | <编号> is not waiting for approval: never asked, already decided, or it already has a result |
| `question.asked` 的调用没有在等的题 | <编号> already has pending questions |
| `question.answered` 对得上一组在等的题 | <编号> is not waiting for answers: never asked, already answered, or it already has a result |
| `turn.ended` 时这一轮的调用都有了结果 | call <编号最小的那个> has no result when the turn ends |
| `context.compacted` 的 `upto` 在它之前 | upto <n> should come before this event |
| `upto` 不早于还算数的最近一次压缩的：撤掉的压缩不算，撤掉以后再压可以比它早 | upto <n> is before the last compaction's <n>; compaction only moves forward |
| `summary` 是空的，`trigger` 得是 `clear`：别的压缩取不到摘要算失败，写不成检查点（施工 6-8 补，`compaction.md` 第十四条） | the summary is empty; only a clear has an empty summary |
| `model.called` 的 `seen` 在它之前 | seen <n> should come before this event |
| `message.withdrawn` 的列表不是空的 | the list of withdrawn messages is empty |
| 撤回的每一条都是这一轮里排着队的消息，不重复 | event <n> is not a queued message of the running turn: not a message, already seen by a request, not in this turn, or already withdrawn |
| `turn.reverted` 时没有回合在进行 | turn <编号> is still running; nothing can be undone |
| 撤销的列表不是空的 | the list of undone turns is empty |
| 撤的每一轮都还没撤掉，压缩以前的也算 | turn <编号> is not in the current history: no such turn, or already undone |
| 撤的正好是第一轮和它以后还在的每一轮，照先后 | undo every turn from <编号> on, in order: <几个编号，用 `, ` 连> |
| `turn.unreverted` 时有能恢复的撤销 | nothing to redo: no undo yet, or a turn or a compaction came after it |
| 恢复的正好是最近一次撤销的那几轮 | redo the turns of the latest undo: <几个编号> |
| `files.restored` 时没有回合在进行 | turn <编号> is still running; files are restored only after an undo or a redo |
| `session.created` 的 `depth` 至少是 1（施工 7-1） | depth should be at least 1 |
| `session.created` 的 `parent`、`depth` 同有同无：子会话两格都有，主会话都没有 | parent and depth go together: a child session has both, the main session neither |
| `tool.result` 效果里的 `job.messaged` 对得上一个 `agent` 的 `job.started`（施工 7-7）：留言只能给这个会话派的子代理 | job <编号> was messaged but is not a subagent: no such job, or it is not an agent |
| `tool.result` 效果里 `job.started` 的编号整份日志里没用过：撤掉的回合里的也算，同一条结果里也不重复（编号不回收）；照整个编号比，`j2` 和 `j2.1` 是两个（施工 7-1 补） | job <编号> is already taken: job ids are never reused, even after an undo |
| `job.started` 的 `agent` 带 `session` | job <编号> is an agent and needs session |
| `job.started` 的 `command` 不带 `session` | job <编号> is a command and has no session |
| `job.reported` 对得上一个 `command` 的 `job.started` | job <编号> is not a background command: no such job, or it is not a command |
| 这个后台命令还没报过结束 | job <编号> has already ended |
| `child.reported` 对得上一个 `agent` 的 `job.started` | job <编号> is not a subagent: no such job, or it is not an agent |
| `child.reported` 的 `session` 和那条 `job.started` 记的一样 | job <编号> runs in session <记的>, not <这一条写的> |
| `child.reported` 的 `by` 是那个子会话 | child.reported for job <编号> should be by session <记的> |
| 以 `stopped`、`undone` 报过的不再报：被停掉的不会再起来。别的（`done`、`aborted`、不认识的）报过以后还能再报：留言叫醒它，它会再报 | job <编号> was stopped or undone and cannot report again |

- 不认识的种类（例如模块的 `ext.*`）只查序号和 `turn`。报错的全文是「event <序号> cannot be appended: <why>」。
- `job.started` 只出现在 `tool.result` 的效果里：效果只有工具结果有，写法本身就保证了，不另查。工具结果照上面先查调用对不对得上，再查它的效果。
- 两种回报带不带 `turn` 不另立规矩：带的要是正在进行的那个回合，照上面那一条；它们不在「必须带 `turn`」的那几种里。内核记的一律不带（2026-09-30 定，`kernel/session.md`「回报」第 3 条）。

**账本记下的变化**：

| 事件 | 记下 |
|---|---|
| 每一条 | 下一条的序号加一 |
| `turn.started` | 它是正在进行的回合，也是还没撤掉的一轮，接在最后；还能恢复的撤销、排着队的都清掉 |
| `message.user`，带着正在进行的回合 | 排进队 |
| `model.called`、`message.assistant` | 序号不大于它的 `seen` 的出队。回复还记下它是上一条回复，里面的工具调用都等结果 |
| `tool.result` | 这个调用有了结果，在等的请求、题跟着了结 |
| `tool.approval_requested`、`tool.approval_decided` | 这个调用开始、不再等确认 |
| `question.asked`、`question.answered` | 这个调用开始、不再等回答 |
| `message.withdrawn` | 撤回的出队 |
| `turn.ended` | 没有回合在进行，队清空 |
| `context.compacted` | 记下这一次压缩：在哪一轮、替代到哪，它是还算数的最近一次；还能恢复的撤销清掉。压缩以前的回合照旧能撤（施工 6-9） |
| `turn.reverted` | 撤的几轮撤掉了；在这几轮里的压缩不再算数；记下这一次撤销，连同跟着撤掉的那几次压缩 |
| `turn.unreverted` | 最近一次撤销去掉，那几轮和跟着撤掉的压缩回来 |
| `tool.result` 效果里的 `job.started` | 记下这个任务：编号，是后台命令还是子代理，子代理的会话（施工 7-1）。撤销、恢复、压缩都不动它 |
| `job.reported` | 这个后台命令结束了，不管 `reason` 是哪一种 |
| `tool.result` 效果里的 `job.messaged` | 这个子代理欠一份回报（施工 7-7）。留言那次调用发出以后（序号大于那次调用所在的回复）已经到了回报的，算回了，不欠：送到了，它手快、先报上来了，不能让父会话一直等一份不再来的回报 |
| `child.reported` | 记下这是它最近一次回报（命令编号、序号），留过言的不再欠（施工 7-6、7-7） |
| `child.reported`，`reason` 是 `stopped`、`undone` | 这个子代理不会再报 |

**有效历史收事件**：

1. `context.compacted`：它换成检查点，旧的检查点和序号不大于 `upto` 的事件丢掉；放在一边的撤销丢掉。被动压缩保下来的尾巴（`upto` 之后的）留着。
2. `turn.reverted`：撤掉的几轮拿走，放在一边（下面「拿走什么」）；这一条本身不留。
3. `turn.unreverted`：最近一次放在一边的放回，照序号排；放回的里面有 `context.compacted` 的（撤掉压缩的撤销放在一边的），再落到检查点上。这一条本身不留。
4. `message.withdrawn`：列出的那几条 `message.user` 去掉；这一条本身不留。
5. `turn.started`：放在一边的撤销丢掉，这一条照留。
6. 别的照先后留着，`model.called`、确认、提问、`files.restored` 也在里面：进不进请求是组装的事。
7. 内存里只留这一段：压缩一次，丢掉更早的；撤掉的下一轮开始、压缩了才丢。撤掉压缩的撤销、载入，照下面「从日志的一段重建」。
8. 每一条先记派出去的任务（下面「派出去过的任务」），再照上面收。

**派出去过的任务**（施工 7-2）：渲染回报要标题、种类；派它的那一轮撤掉了的，回报不渲染、不叫醒她（`agents.md` 第七条第 2 条）。这些在派它的那条 `tool.result` 里，那一条会被压缩换掉、被撤销拿走，回报却可能在那以后才到，所以另记一张表：

1. `tool.result` 效果里的 `job.started`：记下编号、种类、标题、子代理的会话（施工 7-7：认它发来的留言、标签里写它的编号标题，`subagent(会话)`）、那条结果所在的回合。
2. `turn.reverted`：在撤的那几轮里派的，标成撤掉了；`turn.unreverted`：在恢复的那几轮里派的，去掉这个标。撤销恢复不了了也照样标着。
3. 压缩、落到检查点上不动它；`until`、`after` 截出来的那一份带着它。一个任务一项，随任务数长。
4. 载入时重建的那一段以前的事件照 `note` 过一遍，表才是全的；撤掉压缩时换的那一份照原来的（`jobs_from`），读回的那一段再收一遍，结果一样。

**落到检查点上**（`settle`，施工 6-9）：

1. 事件里有 `context.compacted` 的：最近的那一条当检查点，换掉原来的；事件只留序号大于它的 `upto`、不是 `context.compacted` 的；重读的原文清掉。交回换了。
2. 没有的，什么都不动，交回没换。平时那一份的事件里不会有压缩（收到压缩时已经换成检查点），只有恢复放回来的、重建时留着一切收进来的才有。
3. 放在一边的不动：里面的压缩，等恢复放回来再落。

**从日志的一段重建**（施工 6-9：撤掉压缩的撤销、载入）：

1. 这一段从还算数的最近一次压缩替代到的下一条起，一次都没有的从第 1 条起，到日志的最后一条。撤销时照撤完以后算（账本的 `read_back_from`），载入时照整份日志过完账本以后算（`compacted()`）。
2. 从 `whole()` 起，一条条收：撤销、恢复、撤回照同一套规矩，撤掉的回合里的压缩跟着拿走，放在一边。撤掉压缩的那一次撤销，它的 `turn.reverted` 也这样收。派出去过的任务照「派出去过的任务」第 4 条。
3. 收完落到检查点上：还算数的最近一次压缩当检查点。这一段里比它还早的压缩（上一次留下的尾巴里，序号比它的 `upto` 大的）一起丢掉。
4. 为什么从那一条起就够：检查点一换，它替代到的以前的就都丢了；这一段里的撤销、恢复、撤回，碰到那以前的也只是碰到早晚要丢的。算「上一轮还排着的」看的是那一轮的请求看到了哪里，看到它们的请求都在它们后面，也在这一段里。所以没有撤掉过压缩的日志，重建出来的检查点、事件、放在一边的，和一条条收过来的一样。

**拿走什么**：撤掉的几轮里带着它们回合编号的事件，加上这几轮接过去的、人亲口说的话（`message.user`，`by` 是有账号的人）：

1. 每一轮的触发，还在有效历史里、是人亲口说的，拿走。
2. 触发它的是上一轮排着的消息（它带着上一轮的编号），或者上一轮里到的回报、子代理的留言（它们不带回合编号，上一轮就是紧挨着这一轮开头结束的那一轮：它的 `turn.ended` 正好是前一条，施工 7-2、7-7），上一轮又不在这次撤的里面：上一轮结束时还排着的、人亲口说的，也拿走。回报、子代理的留言自己是别处来的，留着（第 3 条）。「还排着的」是带着上一轮编号、序号大于上一轮的请求看到过的最后一条的 `message.user`；请求看到哪里，看上一轮的 `model.called` 和回复的 `seen`，取最大的；自动压缩暂停着、明知放不下没发出去的那一条 `model.called`（分类 `compaction_paused`）不算，排着的话她没听到，由下一轮接过去（施工 6-8 随机长跑撞到，和 6-6 上排队的规矩对齐）；上一轮一次都没请求过的，它里面的 `message.user` 都算。
3. 别处来的留着：子代理、后台命令、定时触发、群里别人说的、另一个会话发来的。触发不是 `message.user` 的（例如重启以后接着干的那一轮，由 `turn.ended` 触发）、没有触发的（手动压缩、清空单开的那一轮，施工 6-8、6-8 补）不拿别的。
4. 崩了的那一轮留下的排着的消息，归那一轮：后来人开口开的一轮是由新消息触发的，撤它不带走它们。
5. 触发它的是没有回合编号的 `message.user`（空闲时说的、重做重发的）：和它同一个 `cause`、也没有回合编号的、人亲口说的 `message.user` 也拿走（施工 4-7 再补）。重做一次重发几句，它们的 `cause` 都是重做的命令，只有最后一句开了这一轮；不拿走，撤这一轮会留下前面几句，再重做也只重发最后一句。空闲时说的一句话自己就开一轮，同一个 `cause` 的只有它。

**照请求看到的范围排**（`ordered`）：

1. 每条回复记着它的请求看到了第几条为止（`seen`），账本保证一次比一次大。以回复为界切段：第 0 段是第一条回复看到的那些，第 k 段是第 k 条回复看到的之后、第 k+1 条看到的为止，最后一段到末尾。
2. 每一段里，先是这一段开头的那条回复，接着是它的工具结果，照调用的先后；然后是这一段里别的事件，照日志的先后。第 0 段照日志的先后。
3. 请求在路上时到的事件（例如人又说了一句），因此排在这次请求的回复和它的结果后面。

**撤销**（`Revert`）：

1. 有回合在进行：拒绝，`turn_running`。头先打断再撤。
2. 不写回合编号的，撤还没撤掉的最后一轮；一轮都没有（没说过话、都撤掉了）：拒绝，`nothing_to_revert`。
3. 没有那一轮、它已经撤掉了：拒绝，`unknown_turn`。写的序号不是一轮的开头也照这一条判。压缩以前的回合照样能撤（施工 6-9，`compaction.md` 第十一条）。
4. 撤的几轮里有还算数的压缩的（账本的 `read_back_from` 有）：先读回更早的日志，读回来了再记（下面「撤掉压缩」）。没有的，当场记。
5. 记一条 `turn.reverted`：`turns` 是那一轮和它以后还没撤掉的每一轮，照先后；`by` 是撤销的人，`cause` 是这个命令，不带回合编号。
   - 撤掉的那几轮里派出去、还在跑的任务（账本的 `running_jobs` 里，有效历史照回合认出是这几轮派的，施工 7-8）：同时出 `StopJobs { jobs, by, cause }`，照编号，`by`、`cause` 是撤销的人和命令，排在 `Append` 后面，有改回文件的排在 `Restore` 前面（停下的命令不会再动文件）；读回日志的，排在 `Recall` 后面。一个都没有的不出。执行器停好了交回报：后台命令的是 `JobEnded`（`undone`），子代理的是子会话交来的 `Report`（`undone`），都照「回报」只记下、不叫醒（`kernel/session.md`）。撤销不等它们，照第 6 到 8 条回应（`agents.md` 第七条第 1 条）。
6. 算改回的几步（下面「改回的几步」）。没有要改的：它落了盘就回应，附上它的序号。
7. 有要改的：出 `Append` 和 `Restore { steps }`，会话进入改回文件。这时来的命令，接受过的照上一次回应，别的拒绝，`restoring`；这个撤销命令自己的编号这时还没记下，它再来也是 `restoring`。会话不算空闲。
8. `Restored` 回来：先对照交出去的几步：一步一项、先后一样，每一项的 `result`、`effect`、`path`、`action` 和那一步一样；移进回收站成了的（`trash` 那一步 `restored`）要带着 `trash`。对不上的那一项改成 `failed`，`error` 写 `executor report did not match`；少了的照那一步补一项 `failed`，多出来的不要。然后记一条 `files.restored`，`files` 照对过的记，`by`、`cause` 和撤销那一条一样，不带回合编号；两条都落了盘才回应，附上两条的序号。不在改回文件时来的 `Restored` 是过时的，不理。
9. 撤了就跟没说过一样：请求里不写撤销过什么。

**重做**（`Redo { text, attachments }`，施工 4-7 再补，2026-09-30 项目主人定：一个命令，只重做最后一轮）：

1. 有回合在进行：拒绝，`turn_running`。
2. 最后一轮（账本的 `last_turn`）是人说的话开的才能重做：它的 `turn.started` 有 `trigger`，触发的那一条是人亲口说的 `message.user`（`by` 是有账号的人）。一轮都没有（没说过话、都撤掉了），或者最后一轮是回报叫醒的、另一个会话的话开的、手动压缩、清空单开的、重启以后接着干的：拒绝，`not_redoable`，什么都不记。
3. 开这一轮的那一句照第 6 条换过以后一块都不剩（例如原来只有字，字换成空的）：拒绝，`empty_message`，什么都不记。
4. 第 2、3 条照有效历史看。那一轮的开头、触发的那一条压缩掉了的（那一轮中途压过，撤它要读回日志），读回来以后照重建的那一份再看；那时不能的，照样拒绝 `not_redoable`、`empty_message`，读回来的不用、有效历史不换。
5. 撤销那一半照「撤销」：记一条 `turn.reverted`，`turns` 只有最后一轮，`by` 是重做的人，`cause` 是这个命令；撤到还算数的压缩的先读回（「撤掉压缩」），改过文件的先改回、记 `files.restored`；那一轮派出去、还在跑的任务一起停（撤销第 5 条的 `StopJobs`，施工 7-8）。读回、改回的时候照撤销拒绝别的命令，`restoring`。
6. 重发：撤掉拿走的（「拿走什么」）里人亲口说的 `message.user`，除了带着那一轮编号的（那一轮中途来的话），照日志的先后，一句一条追加：就是排着接过来的几句和开这一轮的那一句（开这一轮的在最后）。内容块、`by` 照原来的，附件跟着；`cause` 是这个命令；不带回合编号。开这一轮的那一句照 `text`、`attachments` 换：两样都没有的原样；有的，字是 `text`（没有的照原来那一句的文字块），后面接附件 `attachments`（没有的照原来那一句里文字以外的块，照原来的先后），空的就是不要，和 `session.send` 写的一样字在前、附件在后。排着接过来的几句照原样。
7. 由开这一轮的那一句的新的那一条开新的一轮（「回合」第 1 条，`kernel/session.md`）：`cause` 是这个命令，事实照常比着注入。
8. 没有要改回的文件：`turn.reverted`、重发的几句、新的一轮的开头同一批追加。有的：`turn.reverted` 先追加，改完了 `files.restored`、重发的几句、新的一轮的开头同一批追加，时刻是改完回来的那一刻。
9. 都落了盘才回应，附上 `turn.reverted`、`files.restored`（有的话）、重发的每一句的序号，照先后；新的一轮的开头不在里面，和发消息一样（`kernel/session.md`「命令和回应」第 3 条）。
10. 新的一轮开了，撤销恢复不了（「恢复」第 1 条，`02-内核.md` 第六节「撤销与恢复」第 4 条）。它的请求和撤掉的那一轮的第一次请求一字不差：撤掉的那一轮的事实跟着撤了，新的一轮照同样的比法注入；开这一轮的那一句照样挪到回合开始的地方，排着接过来的几句照样在前面（`kernel/request.md`「组装」第 6 条）。上一轮是打断、出错结束的，它结束的那一句在重发的几句前面，这时不完全一样；隔了一个整点、中间切过权限、换过工作目录的，事实跟着变。
11. 改回文件、读回日志做到一半崩了的，载入以后照「载入」第 4 条不补、不重做：日志里是一次没做完的撤销，没有重发。

1. 出「读回日志」`ReadBack { from }`：`from` 是账本的 `read_back_from`。会话进入读回：这时来的命令，接受过的照上一次回应，别的拒绝，`restoring`；这个撤销命令的编号这时还没记下，它再来也是 `restoring`。会话不算空闲。
2. 执行器只读地读日志，从第 `from` 条到最后一条，送回 `ReadBack { at, from, events }`。
3. 对得上的才收：在读回、`from` 一样、`events` 从第 `from` 条起一条接一条，连到追加过的最后一条。对不上的当过时的不理，接着等。
4. 收了：有效历史照「从日志的一段重建」换掉，`turn.reverted` 在落到检查点上之前收进去。它照撤销的第 5 条记，时刻是 `ReadBack` 到的时刻。
5. 接着照撤销的第 6 到 8 条：改回的几步照这一次放在一边的算，东西现在在哪照重建出来的有效历史里的 `files.restored` 找。
6. 新的检查点重读过文件的，同时出 `Recall`（下面「重读的原文」），排在 `Append` 后面。
7. 执行器读不了日志的（磁盘出错、日志坏了），会话停下（`session/actor.md`）：这个撤销收到「会话停了」，下次用到时从磁盘重新载入，坏了的日志在那时报出来。

**恢复**（`Unrevert`）：

1. 没有能恢复的撤销（没撤过，或者撤了以后开过回合、压缩过）：拒绝，`nothing_to_unrevert`。有回合在进行时一定是这一种：那一轮是撤销以后开的。
2. 连着撤了几次的，一次恢复一次，从最近的往前。
3. 先算改回的几步，再记一条 `turn.unreverted`：`turns` 照那一次撤销原样写，`by` 是恢复的人，`cause` 是这个命令。之后和撤销的第 6 到 8 条一样。撤销停掉的任务不再起来，也不交停任务的动作（施工 7-8）。
4. 恢复只在下一轮开始之前，中间没发过请求：撤掉的回到原来的位置，撤销以后记下的 `files.restored` 留在后面，默认的组装不渲染它，下一次请求接着撤销前的那一次往下长。
5. 那一次撤销撤掉了压缩的：撤掉的压缩跟着回来，不读磁盘、不请求模型。放回来的里面有压缩，有效历史落到最近的那一次上（「有效历史收事件」第 3 条），它重读过文件的，同时出 `Recall`（施工 6-9）。

**重读的原文**（施工 6-9，`compaction.md` 第九条「内核和执行器怎么交接」第 5 条）：

1. 检查点换了、原文不在内存里的时候，内核出「取回原文」`Recall { blobs }`：新检查点 `restored` 里每一份的 blob，照先后。有三种时候：载入以后、撤掉压缩的撤销、恢复了压缩。新检查点没有重读过文件的（没有检查点的也是），不出。
2. 压缩的时候不出：原文照 `Reread` 交回的记下。
3. 执行器照 blob 读出原文，送回 `Recalled { texts }`，读不出来的、不是 UTF-8 的不交；做完才收收件箱，所以这之后的请求照原文组装。
4. `Recalled` 什么都不出，原文放进有效历史（`recall`）。晚到的、不是现在这个检查点的，照 blob 找不到它，渲染不用；下一次换检查点时清掉。没交回的那一份，渲染时整块不写。

**改回的几步**：

1. 撤销：照撤掉的那几轮里每一条 `tool.result` 的每个效果，照日志的先后、效果的先后排好，倒过来：

| 效果 | 一步 |
|---|---|
| `file.changed`，有改前的 | `Write`：原处要是改后的，写回改前的 |
| `file.changed`，新建的（改前是 `null`） | `Trash`：原处要是改后的，移进回收站 |
| `file.trashed`，现在在回收站的某处 | `Untrash`：从那里移回来 |
| `file.trashed`，现在已经在原处 | 没有这一步 |
| `file.read`、不认识的 | 没有这一步 |

2. 恢复：同样那些效果（最近一次撤销拿走的），照先后正着来：

| 效果 | 一步 |
|---|---|
| `file.changed` | `Write`：原处要是改前的（新建的要空着），写回改后的 |
| `file.trashed`，上一次撤销真移回来了 | `Trash`：原处要是移回来的那个文件（记着哈希的），没记哈希的（目录）有东西就行 |
| `file.trashed`，没移回来 | 没有这一步 |
| `file.read`、不认识的 | 没有这一步 |

3. **一个效果现在在哪**：照有效历史里的 `files.restored` 从前往后找，每一项只看结局是 `restored` 的：`untrash` 成了的，在原处（记着它的 `hash`）；`trash` 成了、记着新位置的，在回收站的那个位置。后面的盖掉前面的；一次都没改成过的，在效果里记的回收站位置。结局不是 `restored` 的不算：东西还在上一次的地方。
4. 执行器照先后一步一步先核对再动手，一步一项交回结局（`10-自带软件.md` 第七节）。

**载入**（施工 6-9 改成两遍）：

1. 整份日志一条条过账本：坏日志在这里拦下；过完，账本认出哪几次压缩还算数。
2. 有效历史照「从日志的一段重建」：从还算数的最近一次压缩替代到的下一条起。撤销、恢复、撤回、压缩照样做一遍：载入以后照样能恢复，撤掉了压缩的那一次也能。
3. 那个检查点重读过文件的，载入交回的动作里第一个是 `Recall`。
4. 日志里有撤销、恢复、没有 `files.restored` 的（改到一半停了），不补、不重做。

### 出错

账本拦下的是内核的 bug 或者坏了的日志，报错是英文，给查问题的人看，写进运行日志（施工 4-9 再补四中：原来是中文）：「event <序号> cannot be appended: <why>」，`why` 见上面的表。内核自己造的过不了，内核当场停下；载入时过不了，载入不了（`session.md`「出错」）。

命令的拒绝：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `turn_running` | 有回合在进行：先打断，或者等它做完。 | A turn is running; interrupt it or wait for it to finish. |
| `unknown_turn` | 没有这一轮，或者它已经撤掉了。 | There is no such turn, or it has already been undone. |
| `nothing_to_unrevert` | 没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。 | There is nothing to restore: nothing was undone, or a turn or compaction came since. |
| `restoring` | 正在撤销、恢复，等它做完再来。 | An undo or restore is still in progress; try again when it is done. |
| `nothing_to_revert` | 没有能撤销的回合。 | There is no turn to undo. |
| `not_redoable` | 无法重做 | Cannot redo. |

给人看的话由核心照头的语言配（`crates/miyu-endpoint/src/refusal.rs`）。`not_redoable` 一个原因码管两种（最后一轮不是人的话开的、一轮都没有），说法 2026-09-30 项目主人定，头当一条提示通知显示。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/ledger/tests.rs` | 一整个会话追加得进；序号；只有第 1 条是会话创建；回合开始；`turn` 是正在进行的；调用编号；结果要有在等的调用；回合结束时调用都有结果；压缩只前进，撤掉的压缩不算；不带 `turn` 的压缩不收；回复、`model.called` 的 `seen`；只能撤回排着的；请求和决定、题和回答跟着调用 |
| `crates/miyu-kernel/src/ledger/tests/manual.rs` | 没有 `trigger` 的回合开始也收，别的回合的规矩照查（施工 6-8）；摘要是空的只许清空，没写原因、别的几种、不认识的都拦下（施工 6-8 补） |
| `crates/miyu-kernel/src/ledger/tests/jobs.rs`、`jobs/reports.rs`、`jobs/numbers.rs` | 施工 7-1 的每一条各一个被拦下的例子、一个放行的例子：编号不重复（同一条里、后来的、撤掉的回合里的；照整个编号比、用过的最大编号照最后一段数，`jobs/numbers.rs` 的 `prefixed_job_ids_count_by_their_last_part`，施工 7-1 补）；`agent` 带会话、`command` 不带、不认识的种类不管；后台命令只报一次结束、回报对不上的；子代理的回报对得上会话和 `by`、报好几次、停了的不再报、`aborted` 以后还能报；两种回报带 `turn` 的要是正在进行的那一轮；子会话的 `depth`、`parent` |
| `crates/miyu-kernel/src/ledger/tests/jobs.rs` 的 `a_message_to_a_subagent_makes_it_owe_a_report`、`a_report_that_arrives_while_the_message_is_on_its_way_answers_it`（施工 7-7） | `job.messaged` 只能给这个会话派的子代理（后台命令、不认识的种类、没派过的拦下）；留了言欠一份回报、报了不欠；调用发出以后到的回报算回了；`subagent_in`、`subagents` |
| `crates/miyu-kernel/src/ledger/tests/undo.rs` | 压缩以前的也能撤，撤的范围里的压缩不再算数，恢复了跟着回来；`read_back_from` 从哪一条起、撤不到压缩的没有；撤一轮和它以后的全部；回合进行中不能撤；只恢复最近一次；下一轮开始、压缩以后不能恢复；改回文件只在回合之间 |
| `crates/miyu-kernel/src/history/tests.rs` | 压缩重开有效历史；被动压缩的尾巴；最新的检查点换掉旧的；照请求看到的范围排（图上那一轮、请求在路上时来的话、压缩以后的尾巴）；撤回的和撤回本身都不留 |
| `crates/miyu-kernel/src/history/tests/undo.rs` | 撤掉回合和触发它的话；重做一起重发的几句一起撤、别的命令的不撤（施工 4-7 再补）；没有触发的那一轮不拿别的（施工 6-8）；暂停着没发出去的请求不算听到过（施工 6-8）；撤以后的几轮；别处来的留着；接过去的排着的一起撤；上一轮听到过的留着；出错的请求也算听到过；崩了的排着的归那一轮；恢复放回原处、一次一次地恢复；下一轮、压缩丢掉放在一边的 |
| `crates/miyu-kernel/src/session/tests/scenario/undo_jobs.rs`、`random/watch/jobs.rs` 的 `stop_checked`（施工 7-8） | 撤销停掉那几轮派出去、还在跑的：`StopJobs` 的编号、`by`、`cause`，排在改回文件前面；结束了的、别的回合派的不停，报过以后又被留了言的照停；`undone` 只记下；恢复不停也不起；重做一样；随机测试里停的正好是那几个（`agents.md`「守着它的」） |
| `crates/miyu-kernel/src/history/tests/jobs.rs`（施工 7-2） | 派出去过的任务：标题、种类压缩掉派它的那一条也在；撤掉派它的那一轮标上、恢复去掉、恢复不了了照样标着；只记任务不留事件，换一份照原来的；由上一轮里到的回报接着开的一轮撤掉，带走上一轮排着的话、回报留着；闲着时由回报开的一轮撤掉，不拿别的 |
| `crates/miyu-kernel/src/history/tests/settle.rs`（施工 6-9） | 落到检查点上：最近的压缩当检查点、比它早的一起丢、原文清掉、没有压缩的不动；从日志的一段重建：撤掉的回合里的压缩放在一边，恢复放回来换检查点；没有撤掉过压缩的日志，重建的和一条条收的一样；留着一切的那一份恢复了压缩照先后留成一条 |
| `crates/miyu-kernel/src/session/tests/revert.rs` | 撤最后一轮、从前面的一轮撤；回合进行中拒绝；没有、撤掉了的拒绝；恢复以后请求接着往下长；两次撤销一次一次恢复；下一轮以后没得恢复；载入以后一样；撤过的重启轮不接 |
| `crates/miyu-kernel/src/session/tests/restore.rs` | 改过文件的撤销等改完才回应、改的时候拒绝命令（手动压缩也拒绝，施工 6-8）、不算空闲；恢复一样；没改过文件的照旧；过时的结局不理；交回的少了一项，补一项 `failed` |
| `crates/miyu-kernel/src/session/restore/tests.rs` | 撤销倒着来、只读的跳过；恢复正着来、只把真移回来的再移进去；来回以后用最新的位置；做成了的结局；对照交回的结局：对得上的照原样，移进回收站成了没带位置的、先后反了的、做的不是那一步的、编号路径对不上的 `failed`，少了的补、多出来的不要 |
| `crates/miyu-kernel/src/session/tests/redo.rs`、`scenario/redo.rs`（施工 4-7 再补） | 重做最后一轮：一批里撤掉、原话再发、新开一轮，落了盘才回应、附撤销和重发的几句；换了话的只换开这一轮的那一句、附件照带；只换附件的字照原来的、不要附件的只剩字、字和附件都换的；换过一块都不剩的拒 `empty_message`（原来只有字换成空的、原来只有图不要附件）；排着接过来的几句一起重发、照先后；重做过的再重做、再撤销都带着全部几句；改过文件的先改回再发；撤掉压缩的先读回再发，读回以后才看得出不能重做的照样拒绝；回报叫醒的、手动压缩、清空、重启接着干的、没说过话、都撤掉了的拒 `not_redoable`，有回合在进行的拒 `turn_running`；重做以后不能恢复；同一个编号再来照上一次回应；载入以后一样 |
| `crates/miyu-assemble/tests/redo.rs`（施工 4-7 再补） | 重做以后新的一轮的第一次请求和撤掉的那一轮的第一次请求一字不差（统一的请求、编码成线上的字节都比）：空闲时说的一句、排着接过来的几句 |
| `crates/miyu-kernel/src/session/tests/revert/compaction.rs`（施工 6-9） | 撤掉压缩：先读回、读回的时候拒绝命令、不算空闲；读回来的对不上的（少一条、起点不对、中间断了）不理；撤销记在读回来的那一刻；回到前一个检查点、一次都没有的从头；撤不到压缩的不读；改回的文件照读回的那一段算；恢复不读磁盘、不请求模型、放回压缩；一次撤掉几次压缩；载入时认出哪次还算数，载入以后照样能恢复；不带回合的压缩载入不了 |
| `crates/miyu-kernel/src/session/tests/scenario/rebuild.rs` | 检查点换了取回重读的原文：撤到没有检查点的不取，恢复了、载入以后、撤掉后来的一次回到它的，都取回它那几份（施工 6-9） |
| `crates/miyu-kernel/src/session/tests/scenario.rs` | 撤销、恢复、再说一句；压缩以后事实重新注入（替身的压缩单开一轮，施工 6-9） |
| `crates/miyu-kernel/src/session/tests/random/watch/undo.rs`、`watch/redo.rs`、`watch/restore.rs`、`random/restoring.rs`、`random/undoing.rs`（施工 6-9；重做施工 4-7 再补） | 随机输入里撤销、恢复、重做、改回文件照规矩接受或拒绝；重做重发的正好是撤掉的人的话、由最后一句开一轮；撤销、恢复、压缩随机交错：撤掉压缩的先读回、恢复不读、换回来的检查点取回原文；请求照撤销、恢复以后的历史；只交出改过的文件；结局只记一条；过时的、对不上的读回不理 |
| `crates/miyu-kernel/src/session/tests/random/watch/jobs.rs`（施工 7-1） | 执行器替身在工具结果里派任务，后台命令和子代理轮着来，编号接着用过的最大的往下数，每隔两个接在前缀 `j9` 后面（带前缀的和不带的混在一份日志里，施工 7-1 补）：随机的撤销、恢复、压缩、崩了载入里账本照收（载入时整份日志再过一遍）。两种回报施工 7-2 接上（`watch/reports.rs`，`kernel/session.md`「守着它的」） |
| `crates/miyu-kernel/src/facts/tests.rs`、`crates/miyu-kernel/tests/sample_facts.rs` | 事实照有效历史比：压缩、撤销以后重新注入 |

### 出处

- `02-内核.md` 第九节「日志追加时查的规矩」、第六节「撤销与恢复」「排队的消息」。
- `03-事件模型.md` 第六节「照每次请求看到的范围排」、第七节「压缩、撤销、分叉」「有效历史」、第三节 `files.restored`。
- `07-存储.md` 第七节：内存里的东西随上下文窗口走，不随日志走；撤销撤掉压缩时临时从磁盘读回。
- `09-压缩.md` 第九节、Z10：压缩能撤销，撤掉压缩时读回更早的一段（施工 6-9；蓝图 `compaction.md` 第十一条）。
- `10-自带软件.md` 第七节「撤销」「改回文件的细则」、B7。
- `04-核心协议.md` 第九节：`session.revert`、`session.unrevert`、`session.redo` 的参数、回应、原因码。
- `agents.md`「对外的样子」：任务的几条规矩（施工 7-1）。

### 还没有的

- 分叉（`03-事件模型.md` 第七节）：想回到压缩以前、又想留着后来的几轮，要从那里分叉，现在没有分叉。
- 隐私抹除（`03-事件模型.md` 第七节）：存储层的事，不是事件。
