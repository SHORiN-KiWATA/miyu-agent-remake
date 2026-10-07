## 事件的 `body`

### 是什么

内核认识的 24 种事件，每一种的 `body`：每一格叫什么、是什么写法、有没有、没有时怎么写。外壳、一行怎么读写、瞬时事件见 `kernel/events.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/event/session.rs` | `session.created`、`session.policy_changed`、`session.meta_changed`、`session.recapped`（施工 3-8 四补）；权限 `Permission`、级别 `Level` |
| `crates/miyu-kernel/src/event/turn.rs` | `turn.started`、`turn.ended`（`EndReason`）、`turn.reverted`、`turn.unreverted` |
| `crates/miyu-kernel/src/event/restore.rs` | `files.restored`（`Restored`、`RestoreAction`、`RestoreOutcome`） |
| `crates/miyu-kernel/src/event/message.rs` | `message.user`、`message.assistant`、`message.withdrawn` |
| `crates/miyu-kernel/src/event/tool.rs` | `tool.result`（`ToolStatus`、给人看的说法 `Said`）、`tool.approval_requested`、`tool.approval_decided`（`Decision`） |
| `crates/miyu-kernel/src/event/effect.rs` | 效果 `Effect`：`file.read`、`file.changed`、`file.trashed`、`job.started`（`JobStarted`、`JobKind`，施工 7-1）、`job.messaged`（`JobMessaged`，施工 7-7）、`peer.watch`（`PeerWatch`，施工 C-1）、`todo.written`（`TodoWritten`、`Todo`、`TodoStatus`，施工 D-3） |
| `crates/miyu-kernel/src/event/question.rs` | `question.asked`、`question.answered`；回答对不对得上 `fits` |
| `crates/miyu-kernel/src/event/context.rs` | `context.injected`、`context.compacted`、`context.compaction_paused`（`PauseReason`） |
| `crates/miyu-kernel/src/event/model.rs` | `model.called`（`FirstDifference`、`Usage`、`BlockSpan`、`CallResult`、`CallError`、`ErrorClass`，辅助请求的用途 `Purpose`、是不是辅助请求 `aside()`，施工 3-8 四补） |
| `crates/miyu-kernel/src/event/job.rs` | `job.reported`（`JobReason`）、`child.reported`（`ChildReason`）（施工 7-1） |
| `crates/miyu-kernel/src/event/peer.rs` | `peer.idle`（`PeerIdle`、`IdleReason`，施工 C-1） |
| `crates/miyu-kernel/src/event/image.rs` | `image.described`（`ImageDescribed`，施工 8-17） |

每一种的样本在 `docs/designs/samples/events/<种类>.jsonl`。

### 对外的样子

表里「有没有」一格的四种写法：

| 写法 | 读 | 写 |
|---|---|---|
| 必有 | 没有就报错 | 总写 |
| 可以没有 | 没有、写成 `null`，都当没有 | 没有就不写 |
| 不写是假 | 没有当假 | 是真才写 `true` |
| 空的不写 | 没有当空的 | 空的不写 |

编号、名字、时刻的写法见 `kernel/ids.md`；内容块、原样的 JSON、取值见 `kernel/blocks.md`。整数都是不带负号、不带小数点的（`-1`、`1.5` 读不进来），只有 `job.reported` 的 `exit_code` 带符号（施工 7-1）。写出去，格的先后照表里的先后。

**`session.created`**：会话创建。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `owner` | 账号 | 必有 | 会话的属主 |
| `venue` | 场所 | 必有 | 会话所在的场所。本机开的会话是 `local` |
| `policy` | 内容哈希 | 必有 | 开始时的策略快照（`policy.md`） |
| `permission` | 权限 | 必有 | 开始时的权限 |
| `oneshot` | 布尔 | 不写是假 | 一次性的：`miyu ask` 开的；`miyu ask --continue` 接的是最新的这种（`cli/ask.md`） |
| `cwd` | 字符串 | 可以没有 | 开会话时实际干活的目录，人看到的那种写法（施工 4-9 再补三上）。之前的日志没有 |
| `parent` | 会话编号 | 可以没有 | 父会话：派它的那个会话（`agents.md`，施工 7-1）。主会话没有 |
| `depth` | 整数（`u32`） | 可以没有 | 第几层：父会话的加一。主会话是第 0 层，不写。和 `parent` 同有同无、至少是 1，由账本查（`kernel/history.md`） |
| `model` | 字符串 | 可以没有 | 会话用哪个模型（施工 8-8，`models.md`「事件」）：造会话时解析好的引用，模型 `<供应商>/<模型>` 或池 `@<池>`（施工 8-8 造的可能是挡位换成的那时的值）。协议造的照 `session.create` 的 `model`，没写的照那时的 `models.chat`；子会话照 `subagent` 的 `pool`（`@<池>`，施工 8-8 补；8-8 是 `tier`），没写的照父会话那时的。那时连 `models.chat` 都没配的不写。内核只记不解读 |

子会话不写 `oneshot`：`--continue`、`miyu undo` 找「最近一次 `miyu ask` 开的」不会找到它（`agents.md`）。以前的日志没有 `parent`、`depth`、`model` 几格，原样一个字节不变；没有 `model` 的，路由照载入那一刻的 `models.chat`（`models.md`「怎么走」第一条第 7 条）。样本两条都带 `model`（施工 8-8）。

**权限**（`session.created`、`session.policy_changed` 里的 `permission`）：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `level` | 取值 | 必有 | 常用的那一级：`workspace` 工作区，`full` 完全放开 |
| `read_only` | 布尔 | 必有 | 只读开关：开着的时候实际的级别就是只读，关掉回到常用的那一级 |

两格都写，少一格读不进来。不认识的级别按最严的算（`kernel/blocks.md` 第 19 条）。每一级能做什么，见 `session/guard.md`。

**`session.policy_changed`**：换了策略快照，或者换了权限，或者换了模型，也可以一起换。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `policy` | 内容哈希 | 可以没有 | 新的策略快照，下一个回合开始时生效（还没有哪里写，见「还没有的」） |
| `permission` | 权限 | 可以没有 | 新的权限：收紧的当场生效，放宽的下一次请求时生效（`kernel/session.md`） |
| `model` | 字符串 | 可以没有 | 换成的模型引用（施工 8-10，`models.md`「事件」）：模型 `<供应商>/<模型>` 或池 `@<池>`，下一个回合开始时生效。人换的 `by` 是人，`cause` 是 `session.configure`；钉着的没了、退回默认的 `by` 是内核，带着回合 |
| `replaced` | 字符串 | 可以没有 | 钉着的引用没了、内核退回默认时写：原来那个（施工 8-10）。只和 `model` 一起出现，账本查（`replaced comes only with model`） |

施工 8-18 曾在这里加过 `effort`（会话给一个模型记的思考强度，`{"model": 字符串, "level": 字符串或 null}`）；8-18（补）去掉了这一层，思考强度改在配置里（`models.md`「怎么走」第十一条）。

几格都没有的 `{}` 也读得进来。内核切权限时只写 `permission`，换模型时只写 `model`，退回默认时写 `model`、`replaced`（`kernel/session.md`「换模型」）。以前的日志没有 `model`、`replaced`，照没有读；带着 8-18 那阵子写的 `effort` 的也照样读得进（格式只加不改，不认识的字段不管），内核不理它。样本里 139 号是人换的，144 号是 143 号回合开始时池没了、退回的。

**`session.meta_changed`**：改了哪项写哪项。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `title` | 字符串 | 可以没有 | 新的标题：去掉了前后空白，1 到 200 个字（Unicode 字符）；空的 `""` 是去掉标题 |
| `pinned` | 布尔 | 可以没有 | `true` 置顶，`false` 取消置顶 |

- 标题的写法由协议端点管（`protocol.md` 的 `session.set_meta`）：头写的去掉前后空白再量，空的、超过 200 个字的不收；头写 `null` 去掉标题，这里记成空的 `""`，事件里从不写 `null`（上面「可以没有」的格写成 `null` 当没有）。读的时候不查长短：以后放宽了，老的照样读得进来（施工 3-8 三补）。
- 现在的标题、置顶是日志里的这些一条条盖上去的结果：没写的格照旧；撤掉的回合里的也算（`kernel/session.md`「改标题、置顶」）。
- 谁改的看 `by`：人改的（`session.set_meta`）`by` 是人、`cause` 是那个命令；内核自己起的标题（施工 3-8 五补，`kernel/session.md`「起标题」）`by` 是内核，只写 `title`，没有 `cause`、不带 `turn`，前面紧跟着那一次起标题请求的 `model.called`（`purpose` 是 `title`），同一批追加。内核起的标题取回复的第一行，超过 50 个字的截掉，所以也在 1 到 200 个字里。

**`session.recapped`**：一句回顾（施工 3-8 四补，`kernel/session.md`「回顾」）。头要的（`protocol.md` 的 `session.recap`），推给所有订阅着的头；不进她的上下文（渲染时不出，`kernel/request.md`「组装」），`history` 也不列。`by` 是内核，`cause` 是要它的那个命令（在路上又来的几个并进去，照第一个），不带 `turn`。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `text` | 字符串 | 必有 | 那一句：她写的，去掉了前后空白，不是空的 |
| `upto` | 序号 | 必有 | 照到第几条：喂进回顾请求的最新那一条消息，在这一条之前，账本查（`kernel/history.md`） |

- 有效历史里最近一条的 `upto` 和下一次要照到的一样，下一次 `session.recap` 直接交回它的 `text`，不再请求。
- 它前面紧跟着那一次回顾请求的 `model.called`（`purpose` 是 `recap`），同一批追加。

**`turn.started`**：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `trigger` | 序号 | 可以没有 | 引起这一轮的那条事件：人发来的消息；排着队接着开的，是最后一条排着队的消息；重启以后接着干的，是那时排着队的最后一条，没有排着队的就是那条 `turn.ended`（`kernel/session.md`）。是什么引起的，看那条事件的种类。人要的压缩、人要的清空单开的那一轮不是哪一条引起的，没有（`compaction.md` 第七条、第十四条，施工 6-8、6-8 补）；以前的日志里都有 |
| `cwd` | 字符串 | 可以没有 | 这一轮开始时会话的工作目录，照会话的环境，人看到的那种写法（施工 4-9 再补三上）。之前的日志没有。核心重启以后载入会话，照它找回工作目录（`protocol.md`） |
| `dirs` | 字符串的数组 | 可以没有 | 这一轮加进来的目录，照头报的原样（施工 5-10 上）。没有加进来的目录就不写，所以原来的日志一个字节不变 |

**`turn.ended`**：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `reason` | 取值 | 必有 | 为什么结束，下表 |

| `reason` | 是什么 |
|---|---|
| `completed` | 走完了：模型说完了，没有要执行的工具 |
| `interrupted` | 被人打断 |
| `error` | 出错。细节在那一次请求的 `model.called` 里 |
| `step_limit` | 走到了步数上限 |
| `aborted` | 核心崩了，没走完：载入时补上 |
| `restarted` | 被有计划的重启打断：再起来时接着干 |

**`turn.reverted`**、**`turn.unreverted`**：

| 种类 | 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|---|
| `turn.reverted` | `turns` | 回合编号的列表 | 必有 | 撤掉的回合：某一轮，和它以后还在有效历史里的每一轮，照先后 |
| `turn.unreverted` | `turns` | 回合编号的列表 | 必有 | 恢复的回合：照最近那一条 `turn.reverted` 原样写 |

**`files.restored`**：撤销、恢复时改回文件的结局。`files` 是一步一项的列表，必有，照做的先后。它不进上下文；`cause` 是撤销、恢复的那个命令。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `result` | 序号 | 必有 | 照哪一条 `tool.result` |
| `effect` | 整数（`u32`） | 必有 | 照那一条的第几个效果，从 0 数起 |
| `path` | 字符串 | 必有 | 改的是哪里：效果里记的那个路径 |
| `action` | 取值 | 必有 | 做了什么：`write` 写回一份内容；`trash` 移进回收站；`untrash` 从回收站移回原处 |
| `outcome` | 取值 | 必有 | 结局，下表 |
| `found` | 内容哈希 | 可以没有 | 内容被改过（`changed`）时，现在那个文件的内容的哈希；现在那里是目录、链接的没有 |
| `trash` | 字符串 | 可以没有 | 移进了回收站（`trash` 成了）时，在回收站里的新位置：下一次撤销照它移回来 |
| `hash` | 内容哈希 | 可以没有 | 移回来（`untrash` 成了）的是一个文件时，它的内容的哈希：恢复时照它核对，再移进回收站 |
| `error` | 字符串 | 可以没有 | 出错（`failed`）时，系统的原话 |

| `outcome` | 是什么 |
|---|---|
| `restored` | 改回了，或者现在已经是要改成的样子 |
| `changed` | 内容被改过了，不是她留下的样子 |
| `missing` | 东西没了 |
| `occupied` | 原处被占了 |
| `gone` | 回收站里已经没有了 |
| `unsaved` | 要写回的内容当时没存下来 |
| `unavailable` | 回收站收不了：不删 |
| `failed` | 出错了 |

除了 `restored`，都是没动。几步怎么算、谁来做，见 `kernel/history.md`、`session/actor.md`。

**`message.user`**：人发来的消息，或者另一个会话发来的消息：父会话发给子会话的交代、留言（`by` 是父会话），子代理发给父会话的留言（`by` 是子会话，不带回合编号，施工 7-7，`kernel/session.md`「子代理的留言」），别的会话发来的话（`by` 是那个会话，不带回合编号，施工 C-2，`kernel/session.md`「别的会话发来的话」）。`by` 是会话的照关系分三种，见 `kernel/ids.md`「谁」。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `blocks` | 内容块的列表 | 必有 | 消息的内容：文字、图片、文件。现在经协议发来的只有一块文字（`protocol.md`） |

**`message.assistant`**：模型一次响应的完整内容，工具调用也在里面。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `blocks` | 内容块的列表 | 必有 | 照模型给出的先后：文字、思考、工具调用 |
| `seen` | 序号 | 必有 | 发这次请求时，日志到第几条为止：这条回复是看着它们写的 |
| `interrupted` | 布尔 | 不写是假 | 响应中途被人打断了，或者出错断了：`blocks` 只有收到的部分 |

- 被打断的：正文、思考收到多少留多少，工具调用只留参数收全了的；出错断了的，工具调用一个不留（`kernel/session.md`）。
- 一个块都没有的，不写成回复。
- 请求在路上的时候到的事件，序号比 `seen` 大，投影时排在这条回复后面（`kernel/request.md`）。

**`message.withdrawn`**：撤回排着队、她还没听到的消息。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `messages` | 序号的列表 | 必有 | 撤回了哪几条 `message.user`，照序号的先后 |

撤回的消息和这一条都不进有效历史（`kernel/history.md`）。

**`tool.result`**：一个工具调用的结果。结果照到的先后记进日志，投影时照调用的先后排。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `call_id` | 调用编号 | 必有 | 哪一次调用的结果。在哪条消息里、是第几个，编号本身写着 |
| `status` | 取值 | 必有 | 结果怎样，下表 |
| `blocks` | 内容块的列表 | 必有 | 给模型看的内容。内核写的是一句英文（`kernel/tools.md`） |
| `duration_ms` | 整数 | 可以没有 | 执行用了多少毫秒：执行器量的，从开始执行到结束，等人确认不算。没真执行过的没有 |
| `human` | 说法 | 可以没有 | 给人看的说法，不发给模型。工具没交的、老日志里的没有 |
| `effects` | 效果的列表 | 空的不写 | 给内核和头看的效果，不发给模型，照工具交的先后。内核自己写的结果没有 |

| `status` | 是什么 |
|---|---|
| `ok` | 成功 |
| `error` | 失败：工具执行了，但是出了错，错在哪写在 `blocks` 里。内核执行之前就拦下的（没有这件工具、参数不是 JSON 对象）、执行器替工具说的（现在用不了、崩了）也是它 |
| `cancelled` | 已取消：回合被打断、有计划地重启、崩了以后载入时，还没有结果的调用 |
| `denied` | 被拒绝：执行之前被人、执行前的链或者内核拦下了，谁拦的看 `by` |
| `skipped` | 已跳过：人急着插话；等人回答的时候来了一句话；这里没有人能回答 |

执行器交回的只有 `ok` 和 `error`，另外三种是内核写的。哪一种情况写哪一句、`by` 是谁，见 `kernel/tools.md`。

**说法**（`tool.result` 的 `human`）：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `key` | 字符串 | 必有 | 哪一句。前一截是字放在哪：`core` 是内核的，`software/<软件包>` 是软件包的；后一截是那里 `human/<语言>.json` 的 `said` 下的名字。例如 `core/tool-results/unattended`、`software/basesystem/read/lines` |
| `fields` | 字符串到字符串 | 空的不写 | 换进去的字段，值都是字符串。写出去照字段名排 |

- 记进日志以后原样回放；头照自己的语言换成字，换一种界面语言照样换得出（`store.md`）。
- 代码里 `Said::new(key)` 造一句，`.with(字段, 值)` 再换进一个字段。

**效果**（`tool.result` 的 `effects`）：每一项用 `kind` 分开，`kind` 写在最前。

| `kind` | 格 | 是什么 |
|---|---|---|
| `file.read` | `path`，必有 | 读了一个文件：换成真实位置以后的绝对路径 |
| | `lines`，可以没有 | 读了第几行到第几行，`[从, 到]`，从 1 数起，含两头；正好两个整数。一行都没显示的（空文件、过了结尾）没有 |
| | `hash`，必有 | 读的时候整份文件的内容哈希，读了一段的也是整份的：改之前照它核对 |
| `file.changed` | `path`，必有 | 改了一个文件（新建、覆盖、编辑） |
| | `before` | 改前的内容的哈希。新建的写 `null`；没有这一格的，也当新建读 |
| | `after`，必有 | 改后的内容的哈希 |
| `file.trashed` | `path`，必有 | 移进了回收站：移走之前的位置 |
| | `trash`，必有 | 回收站里的位置，各平台自己的写法：撤销时照它移回来 |
| `job.started` | `job`，必有 | 任务编号（`kernel/ids.md`）：派出去的那次调用报一条，这就是任务开始的记录，不另记事件（`agents.md`，施工 7-1） |
| | `what`，必有 | `command` 后台命令，`agent` 子代理；不认识的原样留着 |
| | `title`，必有 | 调用时给的 `description`，头显示用 |
| | `session`，可以没有 | 子代理的会话编号：`agent` 必有，`command` 没有，不认识的种类不管，由账本查 |
| `job.messaged` | `job`，必有 | 给这个任务编号的子代理留了言（施工 7-7，`agents.md` 第六条）：`send_message` 那次调用报一条，它欠一份回报。对得上这个会话派的一个子代理，由账本查 |
| `peer.watch` | `session`，必有 | 订了别的会话的「空了告诉我」（施工 C-1，`cross-session.md`「效果 peer.watch」）：被等的会话的整个编号。`send_message` 写 `notify_when_idle` 的那次调用报一条（施工 C-6），这就是订的记录，账本照它算在等哪几个会话、从这条结果的时刻算起；不是这个会话自己，由账本查 |
| `todo.written` | `todos`，必有：照先后的每一项 `{"content": 一句话, "status": "pending"、"in_progress"、"completed"}`，不认识的状态原样留着 | `todowrite` 换上的整份待办（施工 D-3，`tools/todowrite.md`）：全部做完、清空的是空列表。`done`，可以没有（施工 D-3 补）：因为全部做完而清空的，清空前那一份，写法同 `todos`；空的不写。有效历史另记一张表，当前的清单是最近一份没撤掉的（`kernel/session.md`「待办」）；不发给模型 |

- 改前改后的内容由执行器存成 blob，效果里是它们的哈希（`session/actor.md`）。
- 缺了 `kind`、认识的种类缺了必有的格、哈希或者任务编号不合写法的，读不进来。
- 不认识的种类，例如第三方的工具报来的，整块原样留着，内核不解读。
- 撤销、恢复照效果改回文件（`kernel/history.md`）；她看过的文件也照效果记（`session/actor.md`）。`job.started`、`job.messaged`、`peer.watch`、`todo.written` 不改回什么，撤销时也不算改过文件。
- `job.started` 的编号整份日志里不重复，撤掉的回合里的也算，由账本查（`kernel/history.md`）。

**`tool.approval_requested`**：请人确认一次调用。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `call_id` | 调用编号 | 必有 | 请人确认的是哪一次调用 |
| `access` | 访问类别 | 必有 | 要的是哪一类访问（`kernel/tools.md`）。收紧成只读时，要写入的当场拦下 |
| `rule` | 原样的 JSON | 可以没有 | 提的放行规则：选本会话都允许、这个工作区以后都允许时，放行的就是它。没提的，只能选允许这一次或者拒绝 |
| `detail` | 原样的 JSON | 可以没有 | 给头看的：为什么要问 |

`rule`、`detail` 的写法由提问的模块定，内核原样记，不看里面（权限策略写的样子见 `session/guard.md` 第五条）。例如越过沙盒跑一条命令（施工 D-4）：`{"call_id":"…","access":"execute","detail":{"command":"touch ~/Downloads/x.txt","sandbox":false,"title":"Create a file","tool":"shell"}}`，没有 `rule`：只能选允许这一次或者拒绝。

**`tool.approval_decided`**：人对一个请求的决定。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `call_id` | 调用编号 | 必有 | 决定的是哪一次调用的请求 |
| `decision` | 取值 | 必有 | `once` 允许这一次；`session` 本会话都允许；`workspace` 这个工作区以后都允许；`deny` 拒绝，拒绝以后她接着干 |
| `reason` | 字符串 | 可以没有 | 拒绝的理由，会写进给她看的结果。只跟着拒绝 |

**`question.asked`**：一个在跑的调用请人回答一组题。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `call_id` | 调用编号 | 必有 | 问的是哪一次调用 |
| `questions` | 题的列表 | 必有 | 一组题，照先后。几道都行，内核不设上限 |

一道题：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `header` | 字符串 | 可以没有 | 顶上那一排标签里的短名字 |
| `question` | 字符串 | 必有 | 问的话 |
| `options` | 选项的列表 | 空的不写 | 几个选项，也可以一个都没有：人总能自己写 |
| `multiple` | 布尔 | 不写是假 | 能多选的写 `true` |

一个选项：`label` 字符串，必有，一行标题，回答里写的就是它；`description` 字符串，可以没有，一行说明；`preview` 字符串，可以没有，一段文字画（代码片段、界面草图），头照等宽字显示，内核原样记（施工 D-2，2026-09-29 项目主人定）。

**`question.answered`**：人对一组题的回答。`call_id` 必有，回答的是哪一次调用的题；`answers` 必有，照题目的先后一道一条：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `picked` | 字符串的列表 | 空的不写 | 选了哪几项，写选项的标题 |
| `text` | 字符串 | 可以没有 | 自己写的 |
| `notes` | 字符串 | 可以没有 | 补的一句备注，不选「其他」也能写（施工 D-2，照 Claude Code，2026-09-29 项目主人定） |

`picked`、`text` 都没有，就是这道没答。

**`context.injected`**：注入进上下文的一块事实。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `kind` | 事实块的类别 | 必有 | 这一块的类别。内核注入的有 `env`、`permission`、`session`（施工 1-13 再补）、`reply_cut` 四类（`kernel/request.md`） |
| `text` | 字符串 | 必有 | 发给模型的原文：标签外壳、转义都已经做好，以后一字不改地回放 |

放在请求里的哪个位置，由投影照日志的先后、回合的触发和类别推出来，事件里不写。

**`context.compacted`**：压缩的检查点。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `upto` | 序号 | 必有 | 检查点替代到哪个序号为止，这一条也替代掉。之后的事件照常排在检查点后面 |
| `summary` | 字符串 | 必有 | 摘要的正文，模型写的，草稿已经剥掉，内核不解读。清空的是空的，照样写出这一格：只有 `trigger` 是 `clear` 的能空，账本查（`kernel/history.md`，施工 6-8 补） |
| `trigger` | `auto`、`manual`、`overflow`、`clear` | 可以没有 | 为什么压：到线了、人要的、供应商报超长、人要清空。以前的日志里没有，当作 `auto`；不认识的原样留着。现在内核写 `auto`（施工 6-2 上）、`manual`（施工 6-8）、`overflow`（施工 6-7）、`clear`（施工 6-8 补，`compaction.md` 第十四条） |
| `instructions` | 字符串 | 可以没有 | 手动压缩时人附的要求，原样；没附的、只有空白的没有（`compaction.md` 第七条第 3 条，施工 6-8） |
| `notes` | 字符串 | 可以没有，没有就是空的 | 代码写的几段：读过、改过的文件清单，取回指路，太大没重读的（`compaction.md` 第八条）。写的时候拼好，以后逐字节回放（施工 6-5） |
| `restored` | 数组 | 可以没有，没有就是空的 | 压后重读的文件，照渲染的先后，一个一项：`path` 照清单的写法、`blob` 原文的哈希、`tokens` 估出来的（施工 6-5） |
| `refills` | 整数 | 可以没有 | 压完很快又到线，连着的第几次；不是的没有（`compaction.md` 第十条第 5 条，施工 6-6 上） |

**`context.compaction_paused`**：暂停了自动压缩（`compaction.md` 第十条，施工 6-6 上）。不进上下文；有效历史里、最近一次压缩那一条以后写下了它，就是暂停着。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `reason` | `failures`、`too_large` | 必有 | 连续失败；压完很快又到线。不认识的原样留着 |
| `failures` | 整数 | 可以没有 | `failures` 的：连着失败了几次 |
| `entry` | 序号 | 可以没有 | `too_large` 的：估得最大的那一条 |

摘要外面那层包装、重读的文件那一块的头尾是投影的模板（`resources/core/checkpoint-open.txt`、`checkpoint-close.txt`、`checkpoint-end.txt`、`compaction/restored-*.txt`），不存在这里；重读的原文在 blob 里。

**`model.called`**：一次模型请求的记录，出错的也记。写在这次请求的回复后面；这一轮就此结束的，`turn.ended` 跟在它后面。压缩的摘要请求也记一条，`seen` 是它替代到的 N，没有回复；取到了摘要的，`context.compacted` 跟在它后面（`compaction.md` 第三条第 12 条）。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `seen` | 序号 | 必有 | 这次请求看到了第几条为止，也是这次请求的名字。有回复的，和回复的 `seen` 一样 |
| `endpoint` | 供应商 | 可以没有 | 发给了哪个供应商。没发出去就失败了的没有 |
| `model` | 模型 | 可以没有 | 发给了哪个模型。同上 |
| `request` | 内容哈希 | 可以没有 | 驱动编码以后的请求字节的 SHA-256。同上：这三格是执行器报「发出去了」时一起报来的，没报过的三格都没有 |
| `messages` | 整数 | 必有 | 统一的请求里有几条消息 |
| `first_difference` | 第一处不同 | 可以没有 | 和这个会话上一次请求比，第一处不同在哪。只是接着加的、前面没有请求可比的（载入以后的第一次也是）没有 |
| `usage` | 用量 | 可以没有 | 供应商没报的没有；被打断的没有 |
| `cost` | 金额 | 可以没有 | 这一次花了多少（施工 8-15，`models.md`「事件」）：执行器照价格、倍率算好，随说完了交来，内核原样记下。没报用量的、哪一项用了却没有价格的、单写了思考价的、被打断的没有；以前的日志没有这一格 |
| `first_token_ms` | 整数 | 可以没有 | 从请求发出去到第一段增量的毫秒数。没发出去的、一段增量都没来的没有 |
| `duration_ms` | 整数 | 可以没有 | 从请求发出去到说完的毫秒数，被打断的算到打断为止。没发出去的没有 |
| `blocks` | 块的起止的数组 | 可以没有 | 回复里每一块从哪一刻开始、到哪一刻收全，照这次请求写成的 `message.assistant` 的块的先后，一块一项。没写回复的没有；以前的日志没有这一格（施工 2-3 补） |
| `result` | 取值 | 必有 | `ok` 说完了；`error` 出错；`interrupted` 被人打断 |
| `error` | 出错 | 可以没有 | 出错的分类、原话，有的话还有 HTTP 状态码；只在出错时有 |
| `compaction` | `auto`、`manual`、`overflow` | 可以没有 | 这是哪一种压缩的摘要请求；主请求没有。以前的日志没有这一格（施工 6-6 上） |
| `purpose` | `recap`、`title` | 可以没有 | 辅助请求的用途（施工 3-8 四补，`26-提示词.md` J6）：回顾，起标题（施工 3-8 五补）。主请求、摘要请求没有；以前的日志没有这一格。不认识的原样留着，也算辅助请求 |

- 带 `purpose` 的是辅助请求（`ModelCalled::aside()`）：它和主对话无关，她没在这次请求里听到什么，它报的用量也不是主对话的大小。所以用量的锚（`compaction.md` 第一条）、排着的话她听到没有（`kernel/history.md`）、压缩的边界（`compaction.md` 第三条第 2 条）、渲染时回合开始的那几块（`kernel/request.md`「组装」）都不看它。它不带 `turn`、没有回复，`seen` 是它照到的那一条，`first_difference`、`blocks` 没有。回顾的 `cause` 是要它的命令；起标题的没有 `cause`（内核自己要的）。
- `first_difference` 在代码里装在盒子里（施工 3-8 四补）：它多半没有；`purpose` 加进来以后 `model.called` 比别的种类大出两百字节，clippy 的 `large_enum_variant` 拦下了。JSON 的写法不变。`cost` 照样装在盒子里（施工 8-15）。

金额（施工 8-15，代码在 `event/cost.rs`）：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `amount` | 数 | 必有 | 这一次花了多少，币种照 `currency`，不取整 |
| `currency` | 字 | 必有 | 那一份价格的币种：目录的是 `USD`，手写的照写的 |
| `price` | `{input, output, cache_read, cache_write}` | 必有 | 实际用的那一档价格，每一百万 token，只写有的几项 |
| `multiplier` | 数 | 必有 | 乘的倍率 |
| `source` | 字 | 必有 | 价格从哪来：`catalog:<供应商>/<模型>`、`config:<文件>:<行>`、`local` |
| `above` | 整数 | 可以没有 | 用了按上下文分档的价格：是超过多少 token 的那一档 |

- 这几个数在代码里是 `Real`：双精度，照位比相等（`ModelCalled`、内核的输入都要能比）。写出去整数值写成整数（`1` 不写 `1.0`），别的照最短能读回原值的写法。

第一处不同：

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `part` | 取值 | 必有 | `tools` 工具面；`system`；`message` 一条消息 |
| `index` | 整数 | 可以没有 | 第几条消息，从 0 数起。只有 `message` 有 |
| `role` | 取值 | 可以没有 | 那一条的角色：`user`、`assistant`、`tool`；这一次少了的，是上一次那一条的角色。只有 `message` 有 |

先比工具面，再比 system，再一条条比消息（`kernel/request.md` 的指纹）。

块的起止：`start_ms` 这一块第一段增量到的时刻，`end_ms` 它最后一段增量到的时刻，两格都必有，都是从请求发出去算起的毫秒数，和 `first_token_ms` 同一个起点。收块的 `End` 不算：驱动流完了才一起收块（`drivers/openai-chat.md`「收尾」第 2 条），算上它，每一块都收在流的末尾。时钟往回拨了，早于发出去的算 0，`end_ms` 不往回挪。被打断、出错收的半截，照留下的那几块记；流里有、回复里不要了的块（空块、没收全的工具调用、出错时去掉的工具调用）不记。形状 `[{"start_ms":640,"end_ms":2310},{"start_ms":2330,"end_ms":2980}]`。头照它写「已思考 N 秒」：思考那一块的 `end_ms` 减 `start_ms`。以前的日志没有这一格，照读，写出去还是没有（施工 2-3 补）。

用量：`uncached` 没命中缓存的输入、`cache_read` 缓存读取、`cache_write` 缓存写入、`output` 输出，四格都必有，都是 token 数。

出错：`class` 分类、`message` 原话，两格都必有；原话给查问题的人看，不进上下文。`status` 是供应商回的 HTTP 状态码，整数，可以没有：连不上的、流里报的错、内核自己查出来的都没有。形状 `{"class":"other","message":"HTTP 404: …","status":404}`。头照它分 429、402、404 说人话，不从原话里抠；分类不看它。以前的日志没有这一格，照读，写出去还是没有（施工 3-5 三补）。

| `class` | 是什么 | 谁分的 |
|---|---|---|
| `retryable` | 可重试 | 驱动 |
| `rate_limited` | 限速 | 驱动 |
| `context_too_long` | 上下文超长 | 驱动 |
| `auth` | 认证失败 | 驱动 |
| `content_policy` | 被内容策略拦截 | 驱动 |
| `other` | 其他：驱动分不进上面五种的 | 驱动 |
| `bad_stream` | 增量对不上，或者执行器的回报先后不对：驱动或执行器的错；流里有一段不是 JSON 的，驱动也分成它 | 内核；驱动 |
| `empty_reply` | 回复里一个块都没有；回顾的回复里没有正文（施工 3-8 四补） | 内核 |
| `bad_summary` | 摘要请求的回复里取不出摘要：空的，或者调了工具（施工 6-2 上） | 内核 |
| `compaction_paused` | 自动压缩暂停着，这一次请求明知放不下，没发（施工 6-6 上） | 内核 |
| `no_model` | 没有能用的模型：`models.chat` 没配、会话的引用解析不出也退不回去、那一家用不了、key 一个都取不到。没发出去，没有 `endpoint`、`model`、`request`，不再来（施工 8-6，`models.md`「事件」） | 执行器（会话的路由） |
| `cooling` | 候选不止一个，全在冷却：没发出去，没有 `endpoint`、`model`、`request`；原话写每个候选为什么、到什么时候。能再来：等到最早恢复的那一个（施工 8-9，`models.md`「怎么走」第五条第 6 条） | 执行器（会话的路由） |

**`job.reported`**：后台命令结束了（施工 7-1，`agents.md`）。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `job` | 任务编号 | 必有 | 哪一个后台命令：对得上一条 `command` 的 `job.started`，由账本查 |
| `reason` | 取值 | 必有 | 为什么结束，下表 |
| `exit_code` | 带符号的整数（`i32`） | 可以没有 | 退出码，照系统交回的原样（`ExitStatus::code()`）：Windows 上的 NTSTATUS 是负的，例如 `-1073741819`。被信号杀掉的没有 |
| `signal` | 整数（`u32`） | 可以没有 | 杀掉它的信号的编号（Unix），和前台 `shell` 写的 `Killed by signal N` 一样 |
| `by_model` | 布尔 | 不写是假 | 不是人直接停的：她自己用 `jobs` 停的，或者它所在的会话被父会话停下、连它一起停的（施工 7-4）。只跟着 `stopped`，不叫醒她 |
| `duration_ms` | 整数 | 可以没有 | 从起进程到结束的毫秒数。载入时补的 `aborted` 没有：进程什么时候没的不知道 |
| `output` | 内容哈希 | 可以没有 | 整份输出存成的 blob。没存下来的没有 |
| `chars` | 整数 | 可以没有 | 整份输出有多少个字（Unicode 字符），和 `output` 一起有 |

| `reason` | 是什么 |
|---|---|
| `exited` | 自己退出了，被信号杀掉的也算 |
| `stopped` | 被停掉了：人停的，或者她自己用 `jobs` 停的、连着父会话一起停的（`by_model`） |
| `undone` | 撤销派它的那一轮时停掉的 |
| `restarted` | 有计划的重启停掉的 |
| `aborted` | 核心崩了，进程跟着没了：载入时补（不变量 8） |

不认识的原样留着。哪一种都算结束：之后这个任务不再报。`by` 照原因记（2026-09-30 定，`kernel/session.md`「回报」第 2 条）：`exited` 是起它的那次调用，`stopped` 是停它的人或者停它的那次 `jobs` 调用，`undone` 是撤销的人，`restarted`、`aborted` 是内核。

**`child.reported`**：子会话的回报（施工 7-1，`agents.md` 第二条）。`by` 是那个子会话（`{"kind":"session","id":<子会话>}`），由账本查。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `job` | 任务编号 | 必有 | 哪一个子代理：对得上一条 `agent` 的 `job.started`，由账本查 |
| `session` | 会话编号 | 必有 | 子会话，和那条 `job.started` 记的一样 |
| `reason` | 取值 | 必有 | 下表 |
| `text` | 字符串 | 必有 | 那一轮最后的回答，超过 `jobs.report_chars` 的留头尾各一半。一个字都没说就结束的是空的 |
| `truncated` | 布尔 | 不写是假 | 正文截过 |
| `person` | 布尔 | 不写是假 | 那一轮里人插过话，或者那一轮是人开的、里面进过父会话的留言（`agents.md` 第二条第 4 条） |
| `by_model` | 布尔 | 不写是假 | 同 `job.reported`：只跟着 `stopped`，不是人直接停的（她用 `jobs` 停的、连着父会话一起停的），不叫醒她（施工 7-4） |

| `reason` | 是什么 |
|---|---|
| `done` | 那一轮结束了：走完的，出错、到了步数上限结束的也算 |
| `stopped` | 被停掉了 |
| `undone` | 撤销父会话派它的那一轮时停掉的 |
| `aborted` | 核心崩了，它那一轮没走完：载入时补 |

- 不认识的原样留着。
- 子会话交来的命令编号是 `<子会话>/report/<报的那一轮>`（施工 7-6）：`cause` 就是它，子会话的哪一轮看它；同一份再交，父会话照它认出是重的，不再记（`kernel/session.md`「回报」第 10 条）。
- 一个子代理可以报好几次：父会话留言叫醒它，那一轮结束时再报（`agents.md` 第六条）。以 `stopped`、`undone` 报过的不再报，被停掉的不会再起来；`aborted` 以后还能再报（`agents.md` 第八条），不认识的也不拦。

**`peer.idle`**：等的那个会话空下来了，或者等不到了（施工 C-1，`cross-session.md`「事件 peer.idle」），记在等的那一边。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `session` | 会话编号 | 必有 | 等的是哪个会话 |
| `reason` | 取值 | 必有 | 下表 |
| `status` | 字符串 | 可以没有 | `idle` 的才有：它最近结束的那一轮最后一条有字的回复的第一行，截到 `peers.status_chars`。没说话的不写 |

| `reason` | 是什么 | `by` |
|---|---|---|
| `idle` | 空下来了 | 那个会话 |
| `expired` | 订了 `peers.watch_hours`（12 小时）没等到，作废了 | 内核 |
| `gone` | 那个会话不在了 | 内核 |

- 不认识的原样留着：新版本才有的，也算等到了头，账本不查它的 `by`（`kernel/history.md`）。
- `by` 由账本查：这时在等 `session`，`idle` 的是那个会话、`expired`、`gone` 的是内核（`kernel/history.md`）。
- `cause`：`idle` 的是交来通知的那个命令，`<被等的会话>/idle/<等的会话>/<等的那一边这次订的起算时刻，Unix 毫秒>`；`expired`、`gone` 的是订它的那一轮的 `cause`（施工 C-6，`cross-session.md`「起草时定的」第 53 条）。
- 不带回合编号：它是别处来的，撤哪一轮都不拿走（`cross-session.md` 第七条第 2 款）。账本照「带 `turn` 的是正在进行的那个回合」查，不另立规矩，和两种回报一样。
- 名字不叫 `session.*`：渲染表里 `session.*` 一律不进上下文（`kernel/request.md`「组装」第 3 条）。渲染成带标签的一块（施工 C-6，`kernel/request.md`「空了的通知」）。

**`image.described`**：一张图的转述（施工 8-17，`models.md`「怎么走」第十三条）。主对话的模型看不了图，`models.vision` 替它看过，内核记下；以后每次请求照它把图换成这段字。

| 格 | 写法 | 有没有 | 是什么 |
|---|---|---|---|
| `blob` | 内容的哈希 | 必有 | 哪一张图：图片块的 `blob` |
| `endpoint` | 供应商编号 | 必有 | 替它看的供应商：一次性入口真发给的那一家 |
| `model` | 模型名 | 必有 | 替它看的模型 |
| `text` | 字符串 | 必有 | 转述的原文，去掉了前后空白。内核不记空的，读的时候不查 |

- `by` 是内核，`cause` 是发这次转述的那一轮的（那一轮没有 `cause` 的不写）。账本不另查。
- 不带回合编号：转述挂在图上，不属于哪一轮，撤哪一轮都不拿走。压缩也不拿走它：内核照日志里的每一条算这个会话转述过哪些图，不看有效历史（`kernel/session.md`「替它看图」）。
- 不渲染：它不是对话的一部分，转述经统一的请求的 `described` 进请求（`kernel/request.md`「替它看的图」）。
- 同一张图记了两条的（照理不会有），用先记的那一条。

：一律不带（2026-09-30 定）：回报不属于哪一轮，带了这一轮的编号，撤这一轮时会跟着被拿走，和「别处来的留着」冲突（`kernel/history.md`「拿走什么」）。账本照「带 `turn` 的是正在进行的那个回合」查，不另立规矩。谁写、到了开不开一轮见 `kernel/session.md`「回报」，渲染成什么样见 `kernel/request.md`「回报」（施工 7-2）。

### 怎么走

1. 「必有」的没有，报「body of <种类> not readable: missing field `<格>` …」；某一格不合写法，报那一格的错，前面同样带着「body of <种类> not readable: 」（`kernel/events.md`「出错」）。
2. 「不写是假」「空的不写」的格写成 `null`，读不进来：只有「可以没有」的格（和 `file.changed` 的 `before`）把 `null` 当没有（照 serde 的读法推的，没有测试证实）。
3. 空的列表格式上读得进来。空的撤销、撤回的列表，账本不收；空的消息，发的时候就拒绝（`kernel/history.md`、`kernel/session.md`）。
4. 回答对不对得上题目（`fits`）：几道题几条；选的都是那道题选项的标题；同一条里不重复；不能多选的至多选一项。自己写的不查。对不上的回答，收命令时就拒绝，写不进日志（`kernel/asking.md`）。
5. `turn.started`、`message.assistant`、`tool.result` 这些种类之间怎么对得上（有 `trigger` 的 `trigger` 在前、`seen` 在前、调用编号接得上、结果对得上一个还在等的调用），追加时由账本查（`kernel/history.md`）。任务的几种也是（施工 7-1）：`job.started` 的编号不重复、`agent` 带会话、`command` 不带；两种回报对得上一个派出去的任务；`session.created` 的 `parent`、`depth` 同有同无。跨会话的也是（施工 C-1）：`peer.watch` 订的不是这个会话自己；`peer.idle` 对得上在等的会话，`by` 对得上原因。

### 守着它的

| 测试 | 守哪几种 |
|---|---|
| `crates/miyu-kernel/src/event/session/tests.rs` | 会话的四种：图纸上的写法、一次性的写与不写、每一级读成自己那一种、不认识的级别原样留着、权限两格都要写、坏的说是哪一种；子会话的 `parent`、`depth` 读写一字不差，主会话不写这两格（施工 7-1）；`session.recapped` 两格都要写（施工 3-8 四补）；`session.policy_changed` 的 `model`、`replaced` 读写一字不差、以前的日志照读、`null` 当没有、不是字的读不进来（施工 8-10）；以前日志里带 `effort` 的照读得进、内核不理它（施工 8-18 加，8-18（补）去掉） |
| `crates/miyu-kernel/src/event/turn/tests.rs` | 回合的四种：图纸上的写法、没有 `trigger` 的不写这一格（施工 6-8）、每种结束原因、不认识的原样留着、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/restore/tests.rs` | `files.restored` 的每一格读写一字不差；新的 `action`、`outcome` 原样留着 |
| `crates/miyu-kernel/src/event/message/tests.rs` | `message.assistant` 图纸上的写法、`seen` 必有、`interrupted` 只在是真时写；`message.withdrawn` 的写法和序号从 1 起 |
| `crates/miyu-kernel/src/event/tool/tests.rs` | `tool.result` 的五种状态、不认识的原样留着、没真执行过的没有用时、说法怎么记；确认的两种：每种决定、没写规则、说明、理由的不写这几格；坏的说是哪一种 |
| `crates/miyu-kernel/src/event/effect/tests.rs` | 四种效果读写一字不差；没显示行的不写 `lines`；新建的 `before` 写成 `null`、没写的当新建；不认识的原样留着；`job.started` 不认识的 `what` 原样留着、命令不写 `session`；`job.messaged` 读写一字不差（施工 7-7）；`peer.watch` 读写一字不差（`a_watch_on_another_session_round_trips`，施工 C-1）；坏的读不进来 |
| `crates/miyu-kernel/src/event/job/tests.rs` | 两种回报（施工 7-1）：图纸上的写法读写一字不差、每种 `reason` 读成自己那一种、不认识的原样留着、不写是假的几格是假时不写、没有的格不写、负的退出码、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/image/tests.rs`（施工 8-17） | `image.described`：图纸上的一行读写一字不差、四格都要写、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/peer/tests.rs`（施工 C-1） | `peer.idle`：图纸上的两行读写一字不差、每种 `reason` 读成自己那一种、不认识的原样留着、`status` 写成 `null` 的不写、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/question/tests.rs` | 提问的两种：图纸上的写法、没写的格子不写、第 4 条对不对得上题目、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/context/tests.rs` | 上下文的几种：图纸上的写法、手动压缩带着要求（施工 6-8）、清空的空摘要照样写出 `summary`（施工 6-8 补）、坏的说是哪一种 |
| `crates/miyu-kernel/src/event/model/tests.rs` | `model.called` 图纸上的写法；没发出去就失败的只有知道的几格；每种出错的分类；出错带着 HTTP 状态码、没有这一格的旧日志照读（施工 3-5 三补）；块的起止读写一字不差、没有这一格的旧日志照读（施工 2-3 补）；第一处不同的写法；`purpose` 读写一字不差、不认识的原样留着、带了的才是辅助请求（施工 3-8 四补） |
| `crates/miyu-kernel/tests/samples.rs` | 每一种的样本读写一字不差；换模型的几条一条接一条：退回的带着回合、原来的正是前面换成的（施工 8-10） |
| `crates/miyu-kernel/tests/resources.rs` 的 `the_sample_denial_is_the_sentence_with_the_reason` | 样本里 71 号被人拒绝的结果，就是资源里带理由的那一句 |

### 出处

- `03-事件模型.md` 第三节：「会话与回合的事件怎么写」「消息和工具结果怎么写」（含效果）「确认的事件怎么写」「提问的事件怎么写」「上下文的事件怎么写」「模型调用怎么写」。
- `03-事件模型.md` E5：策略快照按内容哈希存，会话里记引用。
- `10-自带软件.md` 第五节：效果是标准接口；第七节「改回文件的细则」。
- `26-提示词.md` 第三节：给人看的字和给模型看的字分两份（`human`）。
- `11-权限与沙盒.md` 第二节：三个级别和只读开关，四个选项；A13：拒绝以后她接着干。
- `agents.md`「对外的样子」：效果 `job.started`、`job.reported`、`child.reported`、子会话的 `parent`、`depth`（施工 7-1）；`03-事件模型.md` 第三节：派子代理不另记 `child.spawned`。
- `04-核心协议.md` 第九节 `session.recap`：那一句另记一条事件推给所有头，不进她的上下文；请求记进 `model.called`（2026-10-01 项目主人定）。`26-提示词.md` J6：辅助请求各自声明用途（`purpose`，施工 3-8 四补）。
- `cross-session.md`「效果 peer.watch」「事件 peer.idle」（施工 C-1）。
- `10-自带软件.md` 第三节末尾「替不能看图的模型看图」：转述记进日志、挂在这张图上，同一张图只转述一次（施工 8-17）。

### 还没有的

- `session.created` 的分叉来源：做分叉时加（`03-事件模型.md` 第七节）。
- `tool.result` 里大输出的全文（`03-事件模型.md` 第三节，`08-上下文投影.md` C9）。
- `job.started` 的后台命令由 `shell` 写、`job.reported` 由执行器的任务表交、载入时内核补 `aborted`（施工 7-3）；子代理的 `job.started`、`session.created` 的 `parent`、`depth` 由派子代理写（施工 7-5），`child.reported` 由子会话交（施工 7-6）。
- 选了「本会话都允许」「这个工作区以后都允许」的，决定记下了，执行前的链还不照它放行；工作区的那种还要存进工作区的配置（`02-内核.md` 第六节「确认怎么走」第 3 条，M5）。
- `session.policy_changed` 的 `policy`：换策略快照（目录变了、配置改了）还没有，内核只写过换权限、换模型（`05-内核接口.md` 第八节，`02-内核.md` K3）。
