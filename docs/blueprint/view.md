## 视图投影

状态：定稿，2026-10-09 主会话起草，同一天和终端界面、网页的会话对过形状（施工 9-8 上起）。设计 `04-核心协议.md` 第五节 P3、`01-架构.md` D1：核心决定「显示什么」，头决定「怎么画」。

### 是什么

把会话的事件（落了盘的和瞬时的）算成头直接画的一条条**条目**：种类、状态、属于哪一轮、标题那一句、收起那一行的字。原来每个头自己从事件算（终端约 3300 行：整理正文和时间线 2650、时间线的字 700），两个头各算一遍、字还得对齐；搬进核心以后只有一份。

不搬的：怎么画（布局、颜色、图标、动效、预览几行、滚动、同一时刻只转一处），人亲手点开收起，底栏、运行状态行的词库、侧边栏，预览工作区放哪些文件，待办（另有流）。

分步（主会话定）：9-8 上 投影本身；9-8 中 `view.page` 交条目、握手、`view.detail` 多交的；9-8 下 视图流；9-8 补 会话状态 `view.status`（补上本会话的，补下整棵会话树，2026-10-10 和两个头对过）。原来排的 9-8 再补「Markdown 由核心解析」拿掉了：核心只交 Markdown 原文，解析和渲染都归头（2026-10-10 项目主人定：核心保持输出文字）；两个头照一页共用的约定解析，理解才一致。

### 在哪

| 文件 | 干什么 |
|---|---|
| `crates/miyu-view/src/projector.rs`、`projector/`（新） | 投影这台小机器：喂一条事件交出它引起的变化（`Change`）；`users.rs` 消息和排着的话，`turns.rs` 开轮、收尾、每次请求，`blocks.rs` 回复的一块块和起止，`tools.rs` 调工具的一步、确认、提问，`notices.rs` 旁白，`undo.rs` 撤销恢复 |
| `crates/miyu-view/src/entry.rs`、`notice.rs`、`change.rs`、`id.rs` | 条目、旁白、变化、编号的格 |
| `crates/miyu-view/src/summary.rs`、`title.rs`、`estimate.rs`、`explain.rs`、`words.rs` | 收起那一行、标题那一句、照参数估的行数、出错说明；给人看的字的接口（`Words`、`Texts`）和工具分类（`Kinds`） |
| `crates/miyu-view/tests/` | 照内核的替身跑真会话，每个测试都对照视图流和翻页最后一样（`support/mod.rs` 的 `same`）；`random.rs` 三百份随机剧本；`notices.rs` 另有切在派任务之后的一页（9-8 中）、回报的退出码和用时（9-8 三补）；`lines.rs` 另有改了文件的一步带 `files`（9-8 三补） |
| `crates/miyu-endpoint/tests/view_entries.rs`、`view_page.rs`、`view_detail.rs` | 真核心走一遍（9-8 中）：握手报版本；交条目的页、边界照旧、字照连接的语言；更早派出的任务照样有标题；调用的结果原文、压缩的摘要 |
| `crates/miyu-view/tests/status.rs`（9-8 补上） | 闲着、在跑、在等人（确认、提问到了结）；正在写、在调工具；重试和都在冷却出现又消失；上下文用了多少；任务从派出到了结、停了的为什么 |
| `crates/miyu-endpoint/tests/view_tree.rs`（9-8 补下） | 派出去的子代理在跑一轮：那一项带 `busy`、`spawned`、`running_deep`、`usage`，整份带 `running_deep`、`usage_tree`；子代理派了孙代理也数进来；连子代理一起的用量 |
| `crates/miyu-endpoint/tests/view_status.rs`（9-8 补上） | 订阅回应带整份状态；跑一轮推 `running` 再推 `idle`、用量跟着变、没变的不推；切权限也推 |
| `crates/miyu-endpoint/tests/view_stream.rs`、`src/subscriptions/view/tests.rs` | 视图流（9-8 下）：回应排在推送前面、推送拼到那一页上和翻页一样、`turn.started` 另推；改了语言之后的照新的字；带 `after` 的拒、退订了不推；接着的 `view.append` 并成一条 |
| `resources/core/view.json` | 工具算哪一类（命令、编辑、子代理、留言）、参数里哪一格是会话编号：数据，不登记 |
| `crates/miyu-endpoint/src/view/` | `view.page` 多交条目、`view.detail` 多交的（9-8 中，`project.rs` 接上投影要的字和改了多少行）；视图流订阅时的最新一页（9-8 下，`newest`） |
| `crates/miyu-endpoint/src/subscriptions/view.rs`、`connection/streams.rs` 的 `subscribe_view` | 视图流（9-8 下）：订阅、转发、合并 `view.append`、换字 |
| `resources/core/human/<语言>.json` | `said` 的 `view/…`：收起那一行的字（照网页演示 `timeline.summary` 搬来，两个头一字不差）、准备中的显示名、「会话 短编号」「父会话」、出错说明（照终端的 `error_classes`、`status_hints`） |

### 对外的样子

**握手**（9-8 中）：`hello` 的回应多 `view: 1`（视图投影的版本）。没有这一格的是旧核心，头照旧自己算。

**条目**：一条一个 JSON 对象，共有的格：

| 格 | 写法 | 说明 |
|---|---|---|
| `id` | 字符串 | 稳定编号：跨页、跨重连、核心重启都不变（下面「编号」） |
| `kind` | 下面几种之一 | |
| `turn` | 回合编号 | 属于哪一轮；不属于哪一轮的不写 |
| `hidden` | `true` | 撤销藏起来的；显示着的不写 |
| `at` | 时刻，同事件的 `at` | 开始的时刻：流式来的照第一段增量，翻页算的照引出它的那条事件 |

**编号**：照日志里的位置算，谁算都一样。

| 条目 | 编号 |
|---|---|
| 一条消息（`message.user` 和别处来的话） | `m<序号>` |
| 模型回复里的一块（正文、思考、工具调用） | `b<seen>.<块号>`：`seen` 是那次请求的，块号同 `model.delta` 的 `index`；流式时就有，落了盘照 `message.assistant` 的 `seen` 和 `indexes` 算出同一个 |
| 时间线的一段 | `g` 加它第一步的编号去掉 `b`，例如 `g41.0` |
| 压缩那一条 | `c<序号>`：替代到的那一条（`compaction.progress` 的 `seen`、摘要请求的 `seen`、`context.compacted` 的 `upto` 是同一个），流式的进度、摘要请求的记录、落了盘的检查点改的是同一条 |
| 别的（一轮的收尾、旁白） | `e<序号>`：引出它的那一条事件 |
| 只在视图流里有的旁白（瞬时事件引出的：出错换了模型） | `x<序号>.<第几个>`：排在序号是它的那一条后面的第几个瞬时旁白。不进日志，翻页、重新订阅都没有它 |

**`message.assistant` 多一格 `indexes`**：每一块在流里是第几块（`model.delta` 的 `index`），照 `blocks` 的先后。空块、打断时丢掉的半截调用不进回复（`kernel/session.md`「收回复」），后面的块的位置和块号就错开了；只有错开时才写，和位置一样的不写，以前的日志没有这一格、照位置算。

**种类**：

| `kind` | 格 |
|---|---|
| `user` | `text`；`attachments`（照 `message.user` 的附件块：`kind`（`image`、`file`）、`name`、`media_type`、`blob`，图有 `width`、`height`，带了路径的有 `path`；大小头照 `blob` 取）；`from`（别处来的话的来处：别的会话、子代理、父会话、别的 harness、场所里的人，和 `by` 一个写法）；`level`（发出去那一刻的权限级别）；`queued: true`（排着、她还没听到，听到了换成不写）；`withdrawn: true`（排着的被退回了：`message.withdrawn`，打断时没听到的那几条；不画，视图流里推到的那一刻头把字放回输入框，翻页、重新订阅看到的不放回） |
| `reply` | `text`（Markdown 原文）；`open: true`（还在写，写完不写） |
| `group` | 时间线的一段：`steps`（几步的编号，照先后；多了一步，这一段跟着 `view.update`，`steps` 和收起那一行一起变）；`open: true`（还在进行：她没开口、这一轮没结束）；`summary`、`summary_en`（收起那一行，下面「收起那一行」）；`took_ms`（从第一步开始到最后一步结束） |
| `thought` | 一步思考：`group`；`text`；`open: true`（还在想）；`took_ms` |
| `tool` | 一步工具：`group`；`call`（调用编号，流式时还没有）；`name`；`title`（下面「标题那一句」）；`state`；`args`（参数原文，流式时边收边接）；`diff`（编辑、写入的 `{added, removed}`，有了结果以后是真数，之前照参数估、带 `estimated: true`）；`images`（结果里的图：`[{blob, media_type, width, height}]`，照先后，头照 `blob.get` 取；`read` 读了图片的那一步）；`job`（派子代理、后台命令的任务编号）；`to_title`（留言发给子代理的，那个子代理的标题：派它那一步的 `description`）；`approval`（要人确认的：`access`、`rule`、`detail` 照 `tool.approval_requested` 原样，定了以后多 `decision`、`reason`、`by`；确认记在这一步上，不另起旁白）；`took_ms`；`files`（改了哪些文件，照结果的效果：`[{path, action}]`，`path` 是换成真实位置以后的绝对路径，`action` 是 `created`（改之前没有这个文件）、`changed`（改了已有的）或 `trashed`（移进回收站），照先后；读的不算，没有的不写。施工 9-8 三补，网页的预览工作区照它认产物、差异卡片照它写「新建」「修改」） |
| `end` | 一轮的收尾：`reason`（`completed`、`interrupted`、`error`、`restarted`……同 `turn.ended`）；`model`、`endpoint`；`took_ms`；`usage`（这一轮加起来，写法同 `model.called`）；出错的 `error`（`class`、`status`、`message`）和 `explain`（出错说明，照连接的语言） |
| `notice` | 旁白：`what` 是下面几种之一，带各自的格 |

`tool` 的 `state`：`preparing`（还在写参数）、`running`（参数写完了、还没结果；排着队的也是它，头照「同一时刻只转一处」自己挑）、`ok`、`error`、`denied`、`cancelled`、`skipped`，后五种同 `tool.result` 的状态。这一轮结束时还没结果的记 `cancelled`。

`notice` 的 `what`（第一步先这几种，结构化，字由头写）：

| `what` | 格 |
|---|---|
| `compaction` | `trigger`（`auto`、`manual`、`overflow`；在压时照进度带的，施工 6-11 再补）；`instructions`（手动压缩附的要求）；`state`（`running`、`done`、`failed`）；`written`、`expected`、`seen`（摘要写到哪了，同 `compaction.progress`，只在 `running` 时有，视图流里照进度 `view.update`；`written` 变小是重来）；`prepared: true`（换上的是提前压好的，不出进度；施工 6-11 三补起照 `context.compacted` 的 `prepared`，翻页也有，头照它不画结果行）；`before`、`after`（只在视图流里有：`compaction.done` 是瞬时的，翻页没有）；`took_ms`、`usage`；失败的 `error`（`class`、`status`、`message`）和 `explain`。摘要原文不进条目，点开时 `view.detail {session, compaction: <序号>}` 取。手动压缩那一轮的用时、用量接在这一条上，不另起 `end` |
| `cleared` | 上下文清空了；那一轮不另起 `end` |
| `paused` | 暂停了自动压缩（`context.compaction_paused`）：`reason`、`failures`、`entry`，原样 |
| `reverted` | 撤销了几轮：`turns`、`said`（撤掉的第一句的头一行）；`files`（改回了哪些文件：照 `files.restored`，每项 `path`、`outcome`）；`jobs`（停掉的后台任务的编号）；撤掉的条目另外 `hidden` |
| `job` | 一件后台任务了结了：`job`、`job_kind`（`agent`、`command`；不叫 `what`，旁白的种类占了那个名字）、`title`、`mark`（`done`、`failed`、`stopped`）、`reason`（回报里的原样）、`report`（子代理的报告全文）；后台命令的 `command`（派它那次调用的命令原文）、`output`（`{blob, chars}`，照回报，头点开时照 `blob.get` 取）、`exit_code`、`signal`、`took_ms`（照回报，没有的不写；施工 9-8 三补） |
| `answered` | 一组题答了：`questions`（问的那一组，照 `question.asked`）、`answers`（照 `question.answered`）、`by` |
| `recap` | 一段回顾：`text`、`covers`（讲到的最后一轮，撤了那一轮跟着藏） |
| `model` | 换了模型：`endpoint`、`model`；`from`（换之前的 `{endpoint, model}`）；`why`：`failover`（出错换了，瞬时的 `model.changed`，只在视图流里有，带换之前最近一次出错的 `class`）、`replaced`（钉着的没了、退回默认的，照 `session.policy_changed` 的 `replaced`）。回合开始重新解析的（`why: turn`）不出旁白 |
| `workspace` | 换了工作区：`cwd`、`dirs`，原样（照 `session.workspace_changed`，家目录写 `~` 由头做）。「她在哪干活」是头拿自己的目录比的，留在头里 |
| `peer` | 等的会话空下来了、等不到了（`peer.idle`）：`session`、`reason`（`idle`、`expired`、`gone`）、`status`。终端现在不画，网页画 |

**标题那一句**（`tool` 的 `title`）：`{"name", "object", "said"}`，照连接的语言。派子代理的 `object` 是描述（`description`），任务编号在 `job`，两个头都写成「任务编号 · 描述」，还没派出去的只有描述；点开的交代是 `args` 里的 `prompt`。留言（`send_message`）的 `object` 照 `to`：子代理写任务编号（标题另在 `to_title`），父会话写「父会话」，别的会话写「会话 短编号」；送到了不写 `said`（和对象重了）。`name` 是显示名（`human` 的 `tools.<名字>.name`）；`object` 是对象（`subject`：路径在家目录里的写 `~/…`，命令写短标题 `description`，参数带会话编号的另给 `session` 短编号）；`said` 是结果那一句（`tool.result` 的 `human` 换成字，换不成的不写）。准备中的 `name` 是「准备执行命令」这种。

**收起那一行**：`summary` 照连接的语言，`summary_en` 固定英文（界面语言是自动的头用英文那份）。一行拆成几段，带颜色的单拎出来：

```json
[{"text": "Ran 2 commands"}, {"text": " · 1 edit "}, {"text": "+3", "tone": "added"}, {"text": " "}, {"text": "-1", "tone": "removed"}, {"text": " · "}, {"text": "1 err", "tone": "error"}, {"text": " · 5s"}]
```

`tone`：`added`、`removed`、`error`。这一段只有一条命令、它出错了的，整行红：`group` 多 `failed: true`。字和数的规矩照终端蓝图 `tui.md`「时间线」第 17 条，原样搬进核心。

**`view.page`**（9-8 中）多一格 `view`：写 `true` 的回应把 `events` 换成 `entries`（这一页的条目，照显示的先后，读的那一刻的状态），别的格照旧。排着队的 `user` 照它被听到的位置排；撤销藏起的照样在、带 `hidden`。更早的一页拼上来，头照 `id` 去重。

**`view.detail`**（9-8 中）多交 `output`：这次调用的结果原文（工具的输出，整份）。另认 `{session, compaction: <序号>}`：交回那一次压缩的 `summary`（检查点的摘要原文）。

**视图流**（9-8 下）：`subscribe {session, stream: "view"}`，回应带最新一页的条目（同 `view.page`），之后的推送和这份在会话 actor 的同一步里拿，不重不漏：

| 推送 | 什么时候 |
|---|---|
| `view.add {session, entry, after}` | 新的一条，排在 `after` 那一条后面（`null` 是最前） |
| `view.update {session, entry}` | 一条换成这样：状态变了（开始、结果到了、出错、被打断、写完、听到了）。位置变了的带 `after` |
| `view.append {session, id, text}` | 正在写的 `reply`、`thought` 的字，正在写参数的 `tool` 的 `args`：接在后面 |
| `view.hidden {session, ids, hidden}` | 撤销、恢复：这几条藏起、显示回来，编号不删 |
| `view.remove {session, id}` | 拿掉一条：流式时开了、落了盘的回复里却没有的块（出错时去掉的、打断时丢掉的半截工具调用；空了的那一段跟着拿掉），一轮结束时还在压的压缩（被打断了，翻页本来就没有它）。正文、思考出了字才开条目，空块不会推 |

慢的头：同一条的 `view.append` 先合并；还放不下推 `resync`（`stream: "view"`），头重新订阅拿新的一页（`04-核心协议.md` 第七节）。订阅时的那一页和订阅在会话 actor 的同一步里拿（`subscribe_after(0)` 交回的订阅和日志），回应排在所有推送前面；之后的推送接着喂同一台投影（9-8 下，`crates/miyu-endpoint/src/subscriptions/view.rs`）。连接的 `ui.language` 改了，下一批起换字（`Projector::retext`）。

### 会话状态（施工 9-8 补上，2026-10-10 和两个头对过）

视图流订阅的回应带一格 `status`；之后一批推送算完，和上一份比，变了推整份 `view.status {session, status}`，不推增量、不按秒推。视图流不推原始事件，在跑没跑、在等什么都看它。

| 格 | 说明 |
|---|---|
| `state` | `idle`、`running`、`waiting`（有没了结的确认、提问） |
| `waiting` | `[{what: "approve"\|"ask", entry, call}]`：没了结的确认、提问，照先后；要确认的工具和参数、题目在那一步的条目里（`args`、`approval`）。没有的不写 |
| `since` | 这一轮开始的时刻；用时由头算。没在跑的不写 |
| `doing` | 在跑时正在做什么：`{what: "thinking"\|"writing"\|"tool", entry}`；`{what: "retrying", attempt, limit, at, class, message, status?, failover?}`（`at` 是再试的时刻，照 `status` 瞬时事件）；`{what: "compacting", entry, trigger, written, expected}`（`trigger` 是 `auto`、`manual`、`overflow`，施工 6-11 三补：头照它分，自动的只写一行，手动的画进度）。没在跑的不写 |
| `context` | `{window?, compaction_line?, used?}`：限额同订阅回应的 `limits`；`used` 是最近一次主请求的输入（三项加起来），压完的照 `compaction.done` 的 `after`，压缩落了盘还没有新请求的不写 |
| `speed` | `{output, ms}`：最近一次主请求的输出 token、首字到结束的毫秒数（没报首字的照整次）；每秒多少由头算 |
| `cooling_until` | 候选都在冷却时最早恢复的时刻（照 `class: cooling` 的重试算），说成了一次就不写。只认订阅以后看到的 |
| `preparing` | 后台在提前压：`{seen, since}`（施工 6-11 三补）。起压时（`compaction.started`）有，那一次说完了（成没成都算）、换上了、压过了以后不写。头看它从没有变成有，弹一句「已触发上下文压缩」。只认订阅以后看到的，重连的不补 |
| `usage` | 本会话累计，写法同 `usage.query` 的一行（含子代理的随 9-8 补下） |
| `model` | 同订阅回应的 `model`（带思考强度 `effort`） |
| `permission`、`todos`、`workspace` | 同订阅回应的那几格 |
| `persona`、`preset` | 同订阅回应的那几格，订阅时定下 |
| `jobs` | 这个会话直接派出去的任务：在跑的和最近做完的 20 个，照派出的先后。一项 `{job, what, title, state, started, ended?, session?, command?, exit_code?, signal?, why?}`：`what` 是 `command`、`agent`；`state` 是 `running`、`done`（命令退出码 0；子代理那一轮结束了，还能留言叫醒，叫醒了回到 `running`）、`failed`（退出码不是 0、被信号杀了）、`stopped`（`why` 是 `stopped` 人停或她自己停的、`undone` 撤销时停的、`restarted` 有计划重启时停的）、`aborted`（核心崩了，断了）。翻页、订阅时切点前派出、了结的照日志补上 |

| `running_deep` | 这个会话在跑的任务一共几个，连子孙的（施工 9-8 补下） |
| `usage_tree` | 连子代理一起累计用了多少，写法同 `usage`：自己的加上每个子代理那一支的（施工 9-8 补下） |

子代理任务（`jobs` 里 `what: agent` 的）另带四格（施工 9-8 补下，2026-10-10 和终端界面对过）：`busy`（子会话这会儿有没有在跑一轮）、`spawned`（它自己派出、还在跑的有几个）、`running_deep`（它那一支在跑的一共几个，不算它自己）、`usage`（它那一支一共用了多少，照账本连子会话一起查）。子代理此刻在做哪一步不在这里：要的头订阅那个子会话的视图流，看它的 `status.doing`。

1. 投影算得出的那一半（`state`、`waiting`、`since`、`doing`、`used`、`speed`、`cooling_until`、`preparing`、`jobs`）在 `miyu-view` 的 `projector/status.rs`，喂事件时顺手记；别的向会话要，在 `miyu-endpoint` 的 `view/status.rs` 拼。
2. 一批里有落了盘的事件才向会话 actor 重要一份「当前的」（用量、权限、工作区）；只有增量的不打扰它。
3. 会话树在 `view/tree.rs` 量：只看载入了的会话（在跑的子代理一定载入着，没载入的那一支算没有在跑的），往下最多走 8 层；用量照账本（`usage::branch`）。任务表变了（编号、状态），或者有子代理时别的会话动了（会话列表报的：一轮开始、空下来、改名、删了，`Listing::touched`；子代理新派的孙会话还不知道是谁的，所以不只看已知的子孙）才重量，量完照样和上一份比，变了才推。

### 怎么走

1. 投影是一台小机器：事件照先后一条条喂进去（落了盘的照序号，瞬时的照到的先后），每喂一条交出这一条引起的变化（加、换、接字、藏起）。从哪一条开始喂都一样：编号只看日志里的位置，不看喂过多少。
2. 一页的条目：先照切点前的日志学派出去的后台任务（`Projector::learn`：只记标题、命令，不出条目，9-8 中），再照这一页的事件喂一遍，取最后的样子；视图流：每个订阅一台，喂它转发的每一条，变化照上面的表推。语言照这个连接的（`hello` 第 6 条），改了 `ui.language` 的从下一条起照新的；已经交过的不重推，头换了语言重新订阅、重新要页，换掉手里的。
3. 时间线的一段：思考、调工具记进在进行的那一段；她开口说话、插进一条旁白、这一轮结束，这一段结束（终端蓝图「时间线」第 1、21 条）。一块什么时候算完：这一块的 `end`，或者同一次请求里下一块开始了，先到的算（同第 10 条）。
4. 只算显示什么，不管画成什么样。终端、网页各自的配置（收不收、铺不铺开、预览几行）留在头里。
5. 视图流和翻页最后一样：块的开始时刻、思考用了多久照 `model.called` 的 `blocks`（请求发出去的时刻加 `start_ms`），压缩那一条的开始时刻照摘要请求发出去的时刻；只在视图流里有的只有 `x…` 旁白和压缩前后的用量。测试每一份都对照。
6. 排着的话照 `seen` 认被听到：哪一次请求（第一段增量、落了盘的回复、`model.called`，先到的算）看到了它，它就挪到末尾、前面那段收起。开轮时开它的那几条和排在它前面还排着的一起归到这一轮；本来不排着的（空闲时说的、旁听的）只记上这一轮、不挪。用了斜杠命令、没在回答的，留着的排队话挪到末尾、不归到哪一轮（`/stop`）。
7. 一段的 `group` 在加步、步变了状态时跟着 `view.update`；一段收起时算 `took_ms`（第一步开始到最后一步结束：思考照起止，工具照结果到的时刻）。

### 还没有的

- 两个头共用的 Markdown 约定（公式分隔符、认哪些 HTML、没写完的块怎么算、本机路径），主会话起草。
- 运行状态行的词库挂在人格上，等项目主人定。
