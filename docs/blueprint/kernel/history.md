## 账本、有效历史、撤销和恢复

### 是什么

日志只追加。每一条事件追加之前，先交给账本照规矩查一遍：新写的和从磁盘载入的走同一条路，违反的不追加。查过的交给有效历史：投影要用的那一段，从最近一次压缩算起，去掉撤销掉的回合、撤回的消息，照每次请求当时看到的样子排好。

撤销、恢复也是追加一条事件。撤掉的那几轮改过文件的，内核照效果算出改回的几步，交给执行器，结局记成一条 `files.restored`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/ledger.rs` | 账本：查规矩、记下变化 |
| `crates/miyu-kernel/src/history.rs` | 有效历史：收事件、压缩、撤回、排先后 |
| `crates/miyu-kernel/src/history/undo.rs` | 撤掉的拿走、放回；跟着撤的话 |
| `crates/miyu-kernel/src/session/revert.rs` | 撤销、恢复两个命令，改回文件的来回 |
| `crates/miyu-kernel/src/session/restore.rs` | 改回的几步怎么算 |
| `crates/miyu-kernel/src/event/restore.rs` | `files.restored` 的每一格 |

### 对外的样子

**账本**（`Ledger`）：`Ledger::default()` 是一个还没有事件的会话。只记查规矩要用的几样，不留事件本身，不随日志变长：下一条的序号、正在进行的回合、上一条回复的序号、这一轮还没有结果的调用和其中在等确认的、在等回答的、排着队的消息、最近一次压缩到哪、那以后开过还没撤掉的回合、还能恢复的几次撤销各撤了哪几轮。

| 方法 | 交回什么 |
|---|---|
| `append(&event)` | 查过、记下；违反的交回 `LedgerError { seq, why }`，账本不变 |
| `next_seq()` | 下一条该是几号，从 1 起 |
| `open_turn()` | 正在进行的回合，没有就是空闲 |
| `pending_calls()` | 正在进行的回合里还没有结果的调用，照编号的先后 |
| `queued()` | 正在进行的回合里排着队的消息，照先后 |
| `turns_from(turn)` | 还在有效历史里的 `turn`，和它以后还在的每一轮，照先后；`turn` 不在的，没有 |
| `last_turn()` | 还在有效历史里的最后一轮 |
| `compacted()` | 最近一次压缩替代到哪 |
| `last_reverted()` | 最近一次还能恢复的撤销撤了哪几轮 |

**有效历史**（`History`）：

| 方法 | 交回什么 |
|---|---|
| `append(event)` | 收一条账本查过的事件 |
| `checkpoint()` | 最近一次压缩的那一条 `context.compacted`；投影里排在最前 |
| `events()` | 检查点之后还有效的事件，照日志的先后 |
| `ordered()` | 同一些事件，照每次请求看到的范围排好 |
| `last_undone()` | 最近一次还能恢复的撤销拿走的事件，照日志的先后 |

**命令**：`Revert { turn }`（`session.revert`，从哪一轮起，`None` 是最后一轮）记 `turn.reverted { turns }`；`Unrevert`（`session.unrevert`）记 `turn.unreverted { turns }`。拒绝的原因码见下面「撤销」「恢复」。

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
| 序号是下一个 | 序号应该是 <下一个> |
| 第 1 条是 `session.created` | 第 1 条应该是会话创建 session.created |
| `session.created` 只能是第 1 条 | 会话创建只能是第 1 条 |
| `turn.started` 的 `turn` 是它自己的序号 | 回合开始的 turn 应该是它自己的序号 |
| `turn.started` 时没有别的回合在进行 | 回合 <编号> 还没有结束 |
| `turn.started` 的 `trigger` 在它之前 | trigger 应该是回合开始之前的一条 |
| 带 `turn` 的，是正在进行的那个回合 | 回合 <编号> 不是正在进行的回合 |
| `message.assistant`、`tool.result`、`tool.approval_requested`、`tool.approval_decided`、`question.asked`、`question.answered`、`message.withdrawn`、`turn.ended` 必须带 `turn` | <种类> 只在回合里发生，要带上 turn |
| `message.assistant` 的 `seen` 在它之前 | seen <n> 应该在这条回复之前 |
| `seen` 不早于上一条回复 | seen <n> 早于上一条回复 <n>：后一次请求一定看过前一条回复 |
| 回复里第 k 个工具调用编号是 `call_<这一条的序号>_<k>`，k 从 1 起 | 第 <k> 个工具调用的编号应该是 call_<序号>_<k>，写的是 <编号> |
| `tool.result`、`tool.approval_requested`、`question.asked` 对得上这一轮还没有结果的调用 | <编号> 不是一个还在等结果的调用：没有这个调用，或者它已经有了结果 |
| `tool.approval_requested` 的调用没有在等的请求 | <编号> 已经有一个在等的请求 |
| `tool.approval_decided` 对得上一个在等的请求 | <编号> 不是在等确认的调用：没请人确认过、已经决定过，或者它已经有了结果 |
| `question.asked` 的调用没有在等的题 | <编号> 已经有一组在等的题 |
| `question.answered` 对得上一组在等的题 | <编号> 不是在等人回答的调用：没问过、已经答过，或者它已经有了结果 |
| `turn.ended` 时这一轮的调用都有了结果 | 回合结束时，调用 <编号最小的那个> 还没有结果 |
| `context.compacted` 的 `upto` 在它之前 | upto <n> 应该在这一条之前 |
| `upto` 不早于上一次压缩的 | upto <n> 早于上一次压缩的 <n>，压缩只前进 |
| `model.called` 的 `seen` 在它之前 | seen <n> 应该在这一条之前 |
| `message.withdrawn` 的列表不是空的 | 撤回的列表是空的 |
| 撤回的每一条都是这一轮里排着队的消息，不重复 | 第 <n> 条不是正在进行的回合里排着队的消息：不是消息、已经被请求看到过、不在这个回合里，或者撤回过了 |
| `turn.reverted` 时没有回合在进行 | 回合 <编号> 还在进行，撤销不了 |
| 撤销的列表不是空的 | 撤销的列表是空的 |
| 撤的每一轮都在有效历史里 | 回合 <编号> 不在有效历史里：不存在、在最近一次压缩之前，或者已经撤掉了 |
| 撤的正好是第一轮和它以后还在的每一轮，照先后 | 要从回合 <编号> 起往后全撤，照先后：<几个编号，用「、」连> |
| `turn.unreverted` 时有能恢复的撤销 | 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过 |
| 恢复的正好是最近一次撤销的那几轮 | 恢复的应该是最近一次撤销的那几轮：<几个编号> |
| `files.restored` 时没有回合在进行 | 回合 <编号> 还在进行，改回文件只在撤销、恢复以后 |

不认识的种类（例如模块的 `ext.*`）只查序号和 `turn`。报错的全文是「第 <序号> 条事件不能追加：<why>」。

**账本记下的变化**：

| 事件 | 记下 |
|---|---|
| 每一条 | 下一条的序号加一 |
| `turn.started` | 它是正在进行的回合，也是还在有效历史里的一轮；还能恢复的撤销、排着队的都清掉 |
| `message.user`，带着正在进行的回合 | 排进队 |
| `model.called`、`message.assistant` | 序号不大于它的 `seen` 的出队。回复还记下它是上一条回复，里面的工具调用都等结果 |
| `tool.result` | 这个调用有了结果，在等的请求、题跟着了结 |
| `tool.approval_requested`、`tool.approval_decided` | 这个调用开始、不再等确认 |
| `question.asked`、`question.answered` | 这个调用开始、不再等回答 |
| `message.withdrawn` | 撤回的出队 |
| `turn.ended` | 没有回合在进行，队清空 |
| `context.compacted` | 记下替代到哪；开始于它之前（序号不大于 `upto`）的回合不再能撤；还能恢复的撤销清掉 |
| `turn.reverted` | 撤的几轮不再在有效历史里；记下这一次撤销 |
| `turn.unreverted` | 最近一次撤销去掉，那几轮回来 |

**有效历史收事件**：

1. `context.compacted`：它换成检查点，旧的检查点和序号不大于 `upto` 的事件丢掉；放在一边的撤销丢掉。被动压缩保下来的尾巴（`upto` 之后的）留着。
2. `turn.reverted`：撤掉的几轮拿走，放在一边（下面「拿走什么」）；这一条本身不留。
3. `turn.unreverted`：最近一次放在一边的放回，照序号排；这一条本身不留。
4. `message.withdrawn`：列出的那几条 `message.user` 去掉；这一条本身不留。
5. `turn.started`：放在一边的撤销丢掉，这一条照留。
6. 别的照先后留着，`model.called`、确认、提问、`files.restored` 也在里面：进不进请求是组装的事。
7. 内存里只留这一段：压缩一次，丢掉更早的；撤掉的下一轮开始、压缩了才丢。

**拿走什么**：撤掉的几轮里带着它们回合编号的事件，加上这几轮接过去的、人亲口说的话（`message.user`，`by` 是有账号的人）：

1. 每一轮的触发，还在有效历史里、是人亲口说的，拿走。
2. 触发它的是上一轮排着的消息（它带着上一轮的编号），上一轮又不在这次撤的里面：上一轮结束时还排着的、人亲口说的，也拿走。「还排着的」是带着上一轮编号、序号大于上一轮的请求看到过的最后一条的 `message.user`；请求看到哪里，看上一轮的 `model.called` 和回复的 `seen`，取最大的；上一轮一次都没请求过的，它里面的 `message.user` 都算。
3. 别处来的留着：子代理、后台命令、定时触发、群里别人说的、另一个会话发来的。触发不是 `message.user` 的（例如重启以后接着干的那一轮，由 `turn.ended` 触发）不拿别的。
4. 崩了的那一轮留下的排着的消息，归那一轮：后来人开口开的一轮是由新消息触发的，撤它不带走它们。

**照请求看到的范围排**（`ordered`）：

1. 每条回复记着它的请求看到了第几条为止（`seen`），账本保证一次比一次大。以回复为界切段：第 0 段是第一条回复看到的那些，第 k 段是第 k 条回复看到的之后、第 k+1 条看到的为止，最后一段到末尾。
2. 每一段里，先是这一段开头的那条回复，接着是它的工具结果，照调用的先后；然后是这一段里别的事件，照日志的先后。第 0 段照日志的先后。
3. 请求在路上时到的事件（例如人又说了一句），因此排在这次请求的回复和它的结果后面。

**撤销**（`Revert`）：

1. 有回合在进行：拒绝，`turn_running`。头先打断再撤。
2. 不写回合编号的，撤还在有效历史里的最后一轮；一轮都没有（没说过话、都撤掉了、都压缩进了摘要）：拒绝，`nothing_to_revert`。
3. 那一轮不在有效历史里：压缩过、它的序号不大于最近一次压缩的 `upto` 的，拒绝，`compacted`；别的拒绝，`unknown_turn`。写的序号不是一轮的开头也照这一条判。
4. 记一条 `turn.reverted`：`turns` 是那一轮和它以后还在有效历史里的每一轮，照先后；`by` 是撤销的人，`cause` 是这个命令，不带回合编号。
5. 算改回的几步（下面「改回的几步」）。没有要改的：它落了盘就回应，附上它的序号。
6. 有要改的：出 `Append` 和 `Restore { steps }`，会话进入改回文件。这时来的命令，接受过的照上一次回应，别的拒绝，`restoring`；这个撤销命令自己的编号这时还没记下，它再来也是 `restoring`。会话不算空闲。
7. `Restored` 回来：记一条 `files.restored`，`files` 照交回的原样记，`by`、`cause` 和撤销那一条一样，不带回合编号；两条都落了盘才回应，附上两条的序号。不在改回文件时来的 `Restored` 是过时的，不理。
8. 撤了就跟没说过一样：请求里不写撤销过什么。

**恢复**（`Unrevert`）：

1. 没有能恢复的撤销（没撤过，或者撤了以后开过回合、压缩过）：拒绝，`nothing_to_unrevert`。有回合在进行时一定是这一种：那一轮是撤销以后开的。
2. 连着撤了几次的，一次恢复一次，从最近的往前。
3. 先算改回的几步，再记一条 `turn.unreverted`：`turns` 照那一次撤销原样写，`by` 是恢复的人，`cause` 是这个命令。之后和撤销的第 5 到 7 条一样。
4. 恢复只在下一轮开始之前，中间没发过请求：撤掉的回到原来的位置，撤销以后记下的 `files.restored` 留在后面，默认的组装不渲染它，下一次请求接着撤销前的那一次往下长。

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

**载入**：日志一条条过账本、进有效历史，撤销、恢复、撤回、压缩照样做一遍：载入以后照样能恢复。日志里有撤销、恢复、没有 `files.restored` 的（改到一半停了），不补、不重做。

### 出错

账本拦下的是内核的 bug 或者坏了的日志，报错是中文，给查问题的人看：「第 <序号> 条事件不能追加：<why>」，`why` 见上面的表。内核自己造的过不了，内核当场停下；载入时过不了，载入不了（`session.md`「出错」）。

命令的拒绝：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `turn_running` | 有回合在进行，撤销不了：先打断再撤。 | A turn is running; interrupt it before undoing. |
| `unknown_turn` | 没有这一轮，或者它已经撤掉了。 | There is no such turn, or it has already been undone. |
| `compacted` | 这一轮已经压缩进摘要了，撤不回来。 | That turn is already compacted into the summary and cannot be undone. |
| `nothing_to_unrevert` | 没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。 | There is nothing to redo: nothing was undone, or a turn or compaction came since. |
| `restoring` | 正在改回文件，等它做完再来。 | Files are being restored; try again when that is done. |
| `nothing_to_revert` | 没有能撤销的回合。 | There is no turn to undo. |

给人看的话由核心照头的语言配（`crates/miyu-endpoint/src/refusal.rs`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/ledger/tests.rs` | 一整个会话追加得进；序号；只有第 1 条是会话创建；回合开始；`turn` 是正在进行的；调用编号；结果要有在等的调用；回合结束时调用都有结果；压缩只前进；回复、`model.called` 的 `seen`；只能撤回排着的；请求和决定、题和回答跟着调用 |
| `crates/miyu-kernel/src/ledger/tests/undo.rs` | 只撤压缩以后的；撤一轮和它以后的全部；回合进行中不能撤；只恢复最近一次；下一轮开始、压缩以后不能恢复；改回文件只在回合之间 |
| `crates/miyu-kernel/src/history/tests.rs` | 压缩重开有效历史；被动压缩的尾巴；最新的检查点换掉旧的；照请求看到的范围排（图上那一轮、请求在路上时来的话、压缩以后的尾巴）；撤回的和撤回本身都不留 |
| `crates/miyu-kernel/src/history/tests/undo.rs` | 撤掉回合和触发它的话；撤以后的几轮；别处来的留着；接过去的排着的一起撤；上一轮听到过的留着；出错的请求也算听到过；崩了的排着的归那一轮；恢复放回原处、一次一次地恢复；下一轮、压缩丢掉放在一边的 |
| `crates/miyu-kernel/src/session/tests/revert.rs` | 撤最后一轮、从前面的一轮撤；回合进行中拒绝；不在有效历史里、压缩过的拒绝；恢复以后请求接着往下长；两次撤销一次一次恢复；下一轮以后没得恢复；载入以后一样；撤过的重启轮不接 |
| `crates/miyu-kernel/src/session/tests/restore.rs` | 改过文件的撤销等改完才回应、改的时候拒绝命令、不算空闲；恢复一样；没改过文件的照旧；过时的结局不理 |
| `crates/miyu-kernel/src/session/restore/tests.rs` | 撤销倒着来、只读的跳过；恢复正着来、只把真移回来的再移进去；来回以后用最新的位置；做成了的结局 |
| `crates/miyu-kernel/src/session/tests/scenario.rs` | 撤销、恢复、再说一句；压缩以后事实重新注入 |
| `crates/miyu-kernel/src/session/tests/random/watch/undo.rs`、`watch/restore.rs`、`random/restoring.rs` | 随机输入里撤销、恢复、改回文件照规矩接受或拒绝；请求照撤销、恢复以后的历史；只交出改过的文件；结局只记一条；过时的不理 |
| `crates/miyu-kernel/src/facts/tests.rs`、`crates/miyu-kernel/tests/sample_facts.rs` | 事实照有效历史比：压缩、撤销以后重新注入 |

### 出处

- `02-内核.md` 第九节「日志追加时查的规矩」、第六节「撤销与恢复」「排队的消息」。
- `03-事件模型.md` 第六节「照每次请求看到的范围排」、第七节「压缩、撤销、分叉」「有效历史」、第三节 `files.restored`。
- `07-存储.md` 第七节：内存里的东西随上下文窗口走，不随日志走。
- `10-自带软件.md` 第七节「撤销」「改回文件的细则」、B7。
- `04-核心协议.md` 第九节：`session.revert`、`session.unrevert` 的参数、回应、原因码。

### 还没有的

- 分叉（`03-事件模型.md` 第七节）：压缩进摘要的回合要从那里分叉才回得去，现在没有分叉。
- 压缩（`09-压缩.md`，M6）：账本、有效历史认 `context.compacted`，可还没有谁写它。
- 撤销时一起停下这几轮派出去的子代理、后台命令；能恢复时来了它们的回报要不要先不开回合（`02-内核.md` 第七节、第六节「撤销与恢复」，M7）。
- 隐私抹除（`03-事件模型.md` 第七节）：存储层的事，不是事件。
