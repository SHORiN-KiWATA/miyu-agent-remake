## 分身：子代理和后台命令

### 是什么

她能把活分出去，自己空出来：**后台命令**是一条放到后台跑的 shell 命令（跑测试、起服务），**子代理**是由她的会话派生的一个子会话，拿着一段交代去做一件事。两样都在后台跑，派出去的那次调用当场返回一个任务编号；做完了回报自己送来，她闲着就由回报开一轮，正忙就在下一步看到。不设「等它做完」，也不用她去查（`02-内核.md` 第七节，2026-09-26 项目主人定）。

子会话是一个普通的会话：自己的日志、自己的回合、自己的权限，属主是派它的那个人。子代理本身也是一个 agent，父子之间就是两个 agent 在通讯，只是多了一层从属：父会话派它、给它留言，它给父会话留言、做完了回报父会话。

通讯只在树上相邻的两层之间（2026-09-29 项目主人定）：孙代理和子代理说话，子代理和父说话，孙代理不能越过子代理找父，兄弟之间也不直接说，要协调经它们的父转。

状态：图纸（2026-09-29 定），M7 照它施工（施工方案第三节 M7 那张表）。做好了的：7-1 事件的类型、读写、账本的规矩、样本（`kernel/events-bodies.md`、`kernel/ids.md`、`kernel/history.md`）；7-2 内核收得下两种回报、到了开不开一轮（第三条，`kernel/session.md`「回报」），渲染成带标签的事实（第九条第 1 条，`kernel/request.md`「回报」），由执行器替身交；7-3 真的后台命令（第四条、第八条：`tools/shell.md`「后台」、`session/tools.md` 第 5 条、`core.md`「停下」、`kernel/session.md`「载入和崩溃」）；7-5 派子代理（第一条：`agent`、造子会话、交代送进去、深度上限，子会话的场所说明，第九条第 3 条）；7-6 子会话向上回报（第二条、第八条：内核 `kernel/session.md`「向上回报」，执行器 `session/actor.md`「向上回报」）；7-4 `jobs` 和 `job.stop`（第五条：列出、读、停，后台命令和子代理都管，`tools/jobs.md`）。7-7 父子之间留言（第六条：`message_agent`（`tools/message_agent.md`），内核收子代理的留言 `kernel/session.md`「子代理的留言」，渲染 `kernel/request.md`「子代理的留言」，执行器 `session/tools.md`「父子之间留言」，效果 `job.messaged`）；3-8 三补 父会话被删，子会话一起停、一起挪走，删一个子会话照人停掉它报给父会话（第七条第 5、6 条：`protocol.md` 的 `session.delete`）。做完一步，这一页照做好的样子改写那几节，相关的几页（`kernel/events-bodies.md`、`kernel/session.md`、`kernel/history.md`、`kernel/request.md`、`session/actor.md`、`protocol.md`、`core.md`、`cli/ask.md`、`tools/`）跟着改。

### 在哪

施工时照这个放：

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/event/effect.rs` | 效果 `job.started` |
| `crates/miyu-kernel/src/event/job.rs` | 事件 `job.reported`、`child.reported` |
| `crates/miyu-kernel/src/id/job.rs` | 任务编号 `JobId`（施工 7-1） |
| `crates/miyu-kernel/src/ledger/jobs.rs` | 账本里任务的几条规矩（施工 7-1，`kernel/history.md`） |
| `crates/miyu-kernel/src/session/jobs.rs` | 回报到了：记下，开一轮、排着还是只记下（施工 7-2）；用过的最大编号、载入时给没结束的后台命令补 `aborted`（施工 7-3）；撤销时停掉随 7-8 |
| `crates/miyu-kernel/src/history/jobs.rs` | 有效历史记着的派出去过的任务：标题、种类、派它的那一轮撤掉了没有（施工 7-2） |
| `crates/miyu-assemble/src/jobs.rs` | 两种回报渲染成带标签的事实（施工 7-2） |
| `crates/miyu-kernel/src/session/report.rs` | 子会话一轮结束时要不要向上回报、回报什么（施工 7-6） |
| `crates/miyu-tool/src/jobs.rs` | 交给工具的任务端口 `JobPort`：交出起好的后台命令、拿回编号（施工 7-3）；列出、读输出、停（施工 7-4） |
| `crates/miyu-tool/src/agents.rs` | 交给 `agent` 的端口：派子代理（`AgentPort`，施工 7-5，`tools/interface.md`） |
| `crates/miyu-session/src/jobs.rs`、`jobs/` | 执行器的任务表：后台进程活过那一次调用，输出落盘，结束了告诉会话，有计划地停下先记 `restarted` 再整组杀（施工 7-3）；派出去的任务照日志记着（`roster.rs`），列出、读、停（`query.rs`、`stop.rs`），子代理在做什么照它的日志看（`peek.rs`）（施工 7-4） |
| `crates/miyu-session/src/actor/halt.rs` | 人用 `job.stop` 停一个、父会话停下这个会话时连它派的全停（施工 7-4） |
| `crates/miyu-session/src/spawn.rs` | 造子会话、给别的会话发命令的端口（`SessionPort`），会话表造会话、载入时交进来（施工 7-5）；停下子会话、看它在做什么（施工 7-4） |
| `crates/miyu-session/src/agents.rs` | 执行器派子代理：照父会话填好子会话，经 `SessionPort` 造出来、送交代（施工 7-5，`session/tools.md`「派子代理」） |
| `crates/miyu-session/src/report.rs` | 执行器交回报：补上任务编号、子会话，经 `SessionPort` 交给父会话；父会话载入以后叫起还没回报的子会话（施工 7-6） |
| `crates/miyu-session/src/job_ids.rs` | 领任务编号：一个会话一份，照日志里用过的往下数，后台命令和子代理共用（施工 7-5、7-3） |
| `crates/miyu-endpoint/src/spawn.rs` | 会话表那一头的 `SessionPort`（施工 7-5）：停下子会话、照它的日志看它（施工 7-4） |
| `crates/miyu-endpoint/src/sessions.rs` | 会话表造子会话（施工 7-5）；`session.send` 的 `from`；删会话连子会话（`sessions/delete.rs`，施工 3-8 三补） |
| `crates/miyu-endpoint/src/methods.rs` | `job.stop`（施工 7-4） |
| `crates/miyu-endpoint/src/lib.rs` 的 `Core::idle` | 空闲判断算上后台命令（施工 7-3）和在跑的子会话 |
| `crates/miyu-basesystem/src/shell/background.rs` | `shell` 的 `run_in_background`：交出去的输出、进程（施工 7-3） |
| `crates/miyu-basesystem/src/{jobs,agent,message_agent}.rs` | 三件新工具：`agent`（施工 7-5）、`jobs`（施工 7-4，`tools/jobs.md`）、`message_agent`（施工 7-7，`tools/message_agent.md`） |
| `crates/miyu-tool/src/messages.rs` | 交给 `message_agent` 的端口：父子之间留言（`MessagePort`，施工 7-7，`tools/interface.md`） |
| `crates/miyu-session/src/messages.rs` | 执行器送留言：照内核交的派出去的子代理认编号，经 `SessionPort` 送过去（施工 7-7，`session/tools.md`「父子之间留言」） |
| `crates/miyu-kernel/src/session/messages.rs` | 子代理的留言到了父会话：照回报的规矩开不开一轮；交给执行器的派出去的子代理（施工 7-7，`kernel/session.md`「子代理的留言」） |
| `crates/miyu-cli/src/ask/follow/` | `miyu ask` 等子代理回报、`--timeout`、`--from` |
| `resources/software/basesystem/tools/{jobs,agent,message_agent}.json` | 说明和参数格式 |
| `resources/core/jobs/*.txt` | 回报的写法（施工 7-2，十一份）、子代理的场所说明（7-5）、子代理留言的标签（7-7，两份）这几段给模型看的字 |

分层照 `01-架构.md`：内核不碰进程和别的会话，只从日志算状态、交出动作；工具只拿端口，不认识会话表；会话表在 `miyu-endpoint`，比会话 actor 高一层，所以造子会话的端口由 `miyu-session` 定义、`miyu-endpoint` 在核心启动时装上（`00-设计理念.md` 第四节「依赖与接口的规矩」：下层定义窄接口，上层实现）。

### 对外的样子

**策略数据**（快照里的默认值，配置那一步能改，`14-配置.md`）：

| 名字 | 默认 | 管什么 |
|---|---|---|
| `jobs.depth` | 2 | 派生的深度上限：主会话是第 0 层，它派的子代理是第 1 层，子代理派的孙代理是第 2 层（2026-09-29 项目主人定：子代理能派孙代理）。到了上限的会话工具面里没有 `agent`，`message_agent` 留着、只能发给父（第一条第 6 条）。出厂值在 `crates/miyu-policy/src/jobs.rs`（`JOB_DEPTH`，施工 7-5）：只在造会话定工具面时用，不进快照 |
| `jobs.report_chars` | 30000 | 回报的正文最多几个字，多了留头尾各一半（第二条第 3 条）。出厂值在 `crates/miyu-policy/src/jobs.rs`（`REPORT_CHARS`），快照里是 `jobs.report_chars`（施工 7-6）。停掉子代理时交回的回报也照它截，用的是同一份（施工 7-4） |
| `jobs.output_chars` | 30000 | `jobs` 读输出一页最多几个字，照 `read` 的分页、停在整行上（施工 7-4 定：读得到后面，不用头尾截）。出厂值在 `crates/miyu-basesystem/src/jobs/page.rs`（`LIMIT`），配置那一步能改 |

同时跑几个不设上限（2026-09-29 项目主人定）：她自己掌握派几个。

**任务编号**：一个会话里从 1 数起，后台命令和子代理共用一串，写成 `j1`、`j2`。只在这个会话里有意义；子会话自己派的另从 `j1` 数。短，她写得对；头要找子会话，看 `job.started` 里的会话编号。

**效果** `job.started`（`tool.result` 的 `effects`，`kernel/events-bodies.md`）：派出去的那次调用报一条，这就是开始的记录，不另记事件（原设计的 `child.spawned` 不做：一件事一条记录，派它的调用本身就在历史里，`08-上下文投影.md` 第三节）。

```json
{"kind":"job.started","job":"j2","what":"agent","title":"查 CI 为什么红","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"}
{"kind":"job.started","job":"j1","what":"command","title":"跑全部测试"}
```

- `what`：`command` 后台命令，`agent` 子代理。
- `title`：调用时给的 `description`，头显示用。
- `session`：子代理的会话编号；后台命令没有（账本查）。
- `job`：编号整份日志里不重复，撤掉的回合里的也算（账本查，`kernel/history.md`）。

**效果** `job.messaged`（施工 7-7）：给自己派的子代理留了言，`message_agent` 那次调用报一条。它不是开始的记录（开始的记录只有 `job.started`），是「这个子代理欠一份回报」的记录：这个会话照它等，它报了才把自己的活向上报（第二条第 2 条）。发给父的留言不报：父会话不欠谁的。账本照 7-1 的规矩查（`kernel/history.md`「账本查的规矩」）。

```json
{"kind":"job.messaged","job":"j2"}
```

- `job`：对得上这个会话派的一个子代理（账本查，`kernel/history.md`）。

**事件** `job.reported`：后台命令结束了。

```json
{"job":"j1","reason":"exited","exit_code":0,"duration_ms":81234,"output":"sha256:…","chars":48213}
```

- `reason`：`exited` 自己退出（被信号杀掉的也算，写 `signal`）；`stopped` 被停掉（附 `by_model`：不是人直接停的写 `true`：她自己用 `jobs` 停的，或者它所在的会话被父会话停下、连它一起停的，施工 7-4）；`undone` 撤销那一轮停掉的；`restarted` 有计划的重启停掉的；`aborted` 核心崩了，进程跟着没了（载入时补，不变量 8）。
- `output`：整份输出存成 blob；`chars` 是它有多少个字。

**事件** `child.reported`：子会话的回报。

```json
{"job":"j2","session":"01a0d78c-…","reason":"done","text":"CI 红是因为……"}
```

- `reason`：`done` 做完了；`stopped` 被停掉（附 `by_model`，同 `job.reported`：她自己用 `jobs` 停的、连着父会话一起停的写 `true`，人用 `job.stop` 停的不写，施工 7-4）；`undone` 撤销那一轮停掉的；`aborted` 核心崩了、它那一轮没走完。
- `text`：那一轮最后的回答，超过 `jobs.report_chars` 的留头尾（`truncated` 写 `true`）。
- `person`：这一轮里人插过话，或者这一轮是人开的、里面进过父会话的留言（第二条第 4 条）。
- `truncated`、`person`、`by_model` 是假的不写（`kernel/events-bodies.md`）。
- `by` 是子会话（`session`），`cause` 是子会话交来的那个命令。账本查：对得上一个 `agent` 的 `job.started`、会话一样、`by` 是它；以 `stopped`、`undone` 报过的不再报（`kernel/history.md`）。

**事件** `session.created` 多两格：`parent` 父会话的编号，`depth` 第几层。主会话不写。两格同有同无，`depth` 至少是 1（账本查）。子会话不写 `oneshot`：`--continue`、`miyu undo` 找「最近一次 `miyu ask` 开的」不会找到它。

**事件** `turn.started`：`trigger` 可以是 `child.reported`、`job.reported` 的序号。回报开的那一轮 `cause` 照回报那条事件的。

**`by` 多一种** `harness`：别的 harness 发来的话（第十一条），带 `name`，是它自己报的名字，不可信，给模型看之前照不可信的文本处理。写法照短名字（1 到 128 字节、没有控制字符），收的时候先去掉控制字符、截到 128 字节（`kernel/ids.md`）。

**协议**（`protocol.md`）：

- `job.stop {session, job}`：停掉一个任务（施工 7-4）。子代理连它派的一起停。回应 `{}`；没有这个任务、已经结束了的，拒绝，原因码 `unknown_job`。
- `session.send` 多一格 `from`：别的 harness 报的名字，有它的记成 `harness`，没有的照旧记成本人。
- `session.list` 的每一项多 `parent`：子会话写父会话的编号，主会话写 `null`（施工 7-5）。

**工具**：`shell` 加 `run_in_background`（`tools/shell.md`「后台」，施工 7-3 做好了）；新的三件 `jobs`（7-4，`tools/jobs.md`）、`agent`（7-5，`tools/agent.md`）、`message_agent`（7-7，父子两个方向，`tools/message_agent.md`）各一页（`tools/`）。说明的原文施工时定，量 token、进登记簿，受工具面的预算管；参数照 `26-提示词.md` 附录的草稿，`agent` 先只声明 `description`、`prompt`（「还没有的」）。

**命令行**：`miyu ask` 等子代理回报完才退、`--timeout`、`--from`（`cli/ask.md`，第十条、第十一条）。

### 怎么走

**一、派子代理**（`agent`，施工 7-5 做好了；工具见 `tools/agent.md`，执行器见 `session/tools.md`「派子代理」）

1. 她调 `agent`，带 `description`（短标题）和 `prompt`（整段交代）。执行器领一个任务编号，照父会话填好一个子会话，经会话表的端口（`SessionPort`）造出来：
   - 属主是父会话的属主，场所照父会话的，工作目录、加进来的目录照父会话这一轮的。
   - 权限：常用的那一级、只读开关都照派出去那一刻实际的抄。只读开关以后一直跟着父会话，父会话开关，子会话跟着变并注入一条（`11-权限与沙盒.md` 第二节），随 7-8；这之前抄的是派出去那一刻的。本会话、这个工作区的放行规则执行前的链还不认，M7 不抄（`kernel/events-bodies.md`「还没有的」）。
   - 有没有人能确认照父会话的：`miyu ask` 派出去的没人能确认，要确认的照样拒绝、说几步没做（`22-命令行.md` O3）。
   - 人格是软件工程师，预设照父会话的（现在还没有预设）；不召回也不记下记忆（`16-人格与预设.md`、`17-记忆.md`）。模型照父会话这时用的：现在一个核心一个模型，子会话用的就是它。
   - `depth` 是父会话的加一。
   - `session.created` 由父会话造（`by` 是它），`cause` 是 `<父会话>/<编号>`，写 `parent`、`depth`，不写 `oneshot`；system 在人设后面接上子会话的场所说明（第九条第 3 条）。子会话进了会话表，和头造的一样照编号找得到。
2. 子会话造好、落了盘，`prompt` 作为父会话发来的 `message.user`（`by` 是父会话，命令编号 `<父会话>/<编号>/prompt`）送进去，开它的第一轮。原样送，不加包装：子会话的场所说明（第九条）已经告诉它交代来自父会话。
3. 调用返回：派出去了、编号、标题（输出的写法见 `tools/agent.md`），效果 `job.started`。不等它。
4. 同一步里调几次就派几个，并行跑：`agent` 的访问类别是读，连着的一起派，各领各的编号。
5. 只有本机（场所 `local`）的会话能派：场所会话里没有 `agent`（第一版群里不能派，`18-通讯平台.md`）；外部身份只从场所会话进来，也就派不了（`11-权限与沙盒.md`）。
6. 到了深度上限的会话，工具面里没有 `agent`：工具面是造会话时定的，一个会话里不变（`08-上下文投影.md` C4），调了也只会被拒，不如不给。`message_agent` 留着，它只能发给父。主会话没有父，`to` 写 `parent` 的照不认识的编号拒。
7. 造不成、交代送不进去、核心正在停：这一次调用交回派不了，原因记进运行日志（`session/actor.md`「运行日志」），别的会话照常。领了的编号不回收；子会话造好了、交代没送进去的，留着一个空的子会话。

**二、子会话怎么回报**（施工 7-6 做好了，细则见 `kernel/session.md`「向上回报」，执行器见 `session/actor.md`「向上回报」）

1. 子会话的一轮结束时，看要不要向上回报：这一轮是父会话开的（派它的第一轮、父会话留言开的），或者这一轮里进过父会话的留言。人切进子会话自己开的那一轮、里面没有留言的，不回报：人就在旁边看着（M8）。由它自己的子代理的留言、回报开的那一轮，不向上回报：那是它和下一层之间的事，不是交给父会话的结果（第二条第 2 条照样管着：下一层都报完了，它才把整件活报上去）。欠没欠从日志算：父会话发来的 `message.user`（`by` 是父会话）进了日志就欠下一份，报了就不欠；交代、留言（施工 7-7）一样。
2. 还有它自己派出去、欠着它一份回报的子代理，先不报；它们都报完了、被这些回报叫醒的几轮也结束了，再一起报。欠着的是一次都没回报过的，和最近一次回报以后又给它留过言的（效果 `job.messaged`，施工 7-7：孙代理停下来在回报里问、它留言答了，孙代理做完再报，它才把整件活报上去；不这样，它答完那一轮就把「在等孙代理」当最后的回答报上去了）。留言那次调用发出以后、结果记下以前就到了的回报，算回了这句留言：送到了，孙代理手快、做完先报上来了，不能让它一直等一份不再来的回报。崩了、被撤销停掉的孙代理的回报不叫醒它：它闲着的，当场报最近那一轮的话。
3. 报的是结束的那一轮最后的回答（照 Claude Code：只交最后的回答）：那一轮最后一条有字的回复里的正文。超过 `jobs.report_chars` 的留头尾各一半，中间一行写省了多少个字（`core/jobs/subagent-omitted.txt`，和 `shell` 截输出的那一行同一句），渲染时注明截过、全文用 `jobs` 的 `output` 看。这一轮以出错、步数上限结束的，照样报，`reason` 是 `done`，正文是它最后说的；一个字都没说的，正文空着，由渲染写明它没说话就结束了。被打断的那一轮不报：它停在半路，等人说话（人开的下一轮结束时报，注明人）或者被停。
4. 人插过话：欠着的这几轮里有人发来的消息，或者人开的那一轮里进过父会话的留言，`person` 写 `true`，渲染时注明，免得父会话对不上自己派的活。
5. 回报是一个命令：子会话的执行器经端口交给父会话，`by` 是子会话，命令编号 `<子会话>/report/<报的那一轮>`。父会话落了盘就算送到。父会话已经被删了的，丢掉（子会话本该一起停了），记一行运行日志。父会话拒绝说对不上任务（`unknown_job`）的，退避着再交，100 毫秒起、每次翻倍，一共等到 30 秒：子代理做得快，可能在父会话记下派它的那次调用（`job.started`）之前就报上来了，等一会儿它就在账上了；等满了还是这样的才丢掉、记一行（2026-09-30 施工 7-6 定：丢了回报，父会话和以后等回报的 `miyu ask` 会一直等，自己好不了）。别的拒绝不再交。任务编号照子会话 `session.created` 的 `cause`（`<父会话>/<编号>`）读回，不另记一格（2026-09-30 施工 7-6 定：造它的命令编号本来就是这个写法，一件事一条记录）。

**三、回报、后台命令结束到了父会话**（施工 7-2 做好了，细则见 `kernel/session.md`「回报」）

1. 记一条 `child.reported` 或 `job.reported`，不带回合编号（2026-09-30 定：带了会跟着那一轮被撤掉）。`job.reported` 的 `by` 照原因记：自己退出的是起它的那次调用，被停掉的是停它的人或者那次 `jobs` 调用，撤销停掉的是撤销的人，重启、崩了的是内核（2026-09-30 定）。
2. 她闲着：由它开一轮（`trigger` 是它）。她正忙：照排队的消息，在下一步的边界看到（`02-内核.md` 第六节「排队的消息」）。
3. 不开轮、只记下的几种（下一轮开始时她一起看到）：
   - 她自己用 `jobs` 停掉的（`by_model`，后台命令、子代理都是，子代理的施工 7-4 加）：她知道自己停了。连着父会话一起停的也带 `by_model`（第五条第 4 条）。
   - `undone`、`restarted`、`aborted`：不是做出来的结果。
   - 这时还能恢复撤销：先记下不开轮，恢复了或者人说了下一句，照排着的接着开，由最后那一条（opencode v2 的做法，`02-内核.md` 第六节第 4 条）。
   - 派它的那一轮撤掉了的：回报不渲染（第九条），开了轮她也看不到（施工 7-2 定）。
   - 没人看着的一次性会话（`miyu ask` 开的，这时没有头订阅着它：`miyu ask` 已经退出了，或者按 Ctrl+C 不等了）：只记下，下次 `miyu ask -c` 接着说时她一起看到。命令照常跑完，不会没人看着花钱、自己动手（2026-09-29 项目主人定，照 codex 的信箱；opencode、dsh 会叫醒，Claude Code、codex 退出时干脆停掉）。有没有头订阅着由会话 actor 在订阅、退订时告诉内核，不进日志：开没开轮本身记在日志里，载入时照日志。
4. 被人停掉的子代理（`job.stop`）照样开一轮：设计写明「父会话不会白等」。
5. 叫醒不设连着几次的上限（2026-09-29 项目主人定）：并行派出去几个、先后回报，就是连着叫醒几次，设了闸反而扣住后面的。dsh 连着叫醒 3 次以后只记下；真撞见她自己转个没完再加。

**四、后台命令**（`shell` 的 `run_in_background`）

（施工 7-3 做好了，细则见 `tools/shell.md`「后台」、`session/tools.md` 第 5 条）

1. 写了 `run_in_background: true` 的，照前台一样判权限、进沙盒、起进程（同一个 shell、同一份环境变量白名单），起来了交给任务端口，当场返回编号（`started.txt`），效果 `job.started`（标题是 `description`）。`timeout` 不管后台的。
2. 进程活过这一次调用，由执行器的任务表管（核心里一张，所有会话共用）：编号照日志往后数，撤掉的回合里用过的不再用；输出照前台的合法化（合进一根管道、照 UTF-8 解、`\r\n` 换 `\n`）一直写进会话目录下的 `jobs/<编号>.out`，不截；结束了整份存成 blob、交给会话记 `job.reported`（`exited`，退出码或信号、用时、字数；`by` 是起它的那次调用，`cause` 是那一轮的），落了盘才从表里拿掉。
3. 权限保持起的时候的那一级：已经在跑的进程，沙盒收不紧（`11-权限与沙盒.md` 第二节）。
4. 停：整组杀掉（Unix 进程组，Windows 杀整棵树），和前台超时一样；已经结束了的不再按编号杀。命令自己退出以后，Unix 上组里剩下的也杀掉，和前台一样。
5. 核心有在跑的后台命令就不空闲退出（`12-进程形态与分发.md` R1）；有计划地退出，先给每个在跑的记 `restarted`、落了盘再整组杀（R5）。会话 actor 因为别的停了（写不进去、panic），它的后台命令整组杀掉、不记，再载入时补 `aborted`。
6. 一直开着的服务不结束，也就不回报。

**五、`jobs`**（施工 7-4 做好了，工具见 `tools/jobs.md`，执行器见 `session/tools.md` 第 6 条）

1. `list`：这个会话派出去、还没结束的全部，和最近结束的 5 个，照编号排：编号、种类、标题、状态（`running` 或者最后那条回报的 `reason`）、用时。执行器照日志记着派出去的任务（`job.started`、两种回报），载入时从日志建，之后每落一批盘跟着记；撤销、压缩不动它：撤掉的回合里派的照样在跑、照样列出来。
2. `output`：后台命令读到这时的输出（结束了、存成了 blob 的读 blob，别的读 `jobs/<编号>.out`），照 `read` 分页，一页最多 `jobs.output_chars` 个字，带往下读的指路；子代理读它最近的回答和这一步在跑的工具，经会话表照它的日志看，不载入它。还在跑的末尾说一句。
3. `stop`：
   - 后台命令：还在表里、还没人报过结束的，拿着表的锁记成报了（之后它自己退出了也不再报：只认先到的那一个），整组杀掉，关上输出、存成 blob，记 `job.reported`（`stopped`，`by_model`，`by` 是这次 `jobs` 调用，`cause` 是那一轮的；带到这时的用时、输出，没有退出码、信号）。
   - 子代理：经会话表停下子会话（打断它这一轮、排着的退回，再停掉它派出去、还没结束的，连它们派的；打断记成父会话发的，命令编号 `<父会话>/<编号>/stop`），再把 `child.reported`（`stopped`，`by_model`，正文是它这一轮最后的回答，截法和向上回报的是同一份：超过 `jobs.report_chars` 留头尾、中间一行写省了多少个字）作为子会话交来的回报送进父会话（`by` 是子会话，命令编号 `<父会话>/<编号>/stopped`），账本照旧查得上。
   - 她停的都不叫醒她（第三条第 3 条）；已经结束了的（子代理报过 `done` 也算）说一句、出错。
4. 连着一起停的：子会话里被停掉的后台命令、孙代理，记在子会话里，一律带 `by_model`（后台命令那条的 `by`、`cause` 是父会话和那条打断），不叫醒子会话：它自己也被停了（施工 7-4 定：`by_model` 从「她自己停的」扩成「不是人直接停的」）。
5. 人停（协议 `job.stop`，`protocol.md`）：和她停的走同一条路，后台命令那条 `by` 是人、`cause` 是那条命令，都不带 `by_model`，叫醒她（第三条第 4 条）。后台命令的回报当场落了盘才回应（先见结果，后见回应）；子代理的回报经会话表送回父会话，落了盘才回应。
6. 说明里写明做完会自己报、不用轮询（旧版实测，`26-提示词.md` 附录）。

**六、父子之间留言**（`message_agent`，施工 7-7 做好了；2026-09-29 项目主人定：子代理也是 agent，父子之间来回说话。工具见 `tools/message_agent.md`，内核见 `kernel/session.md`「子代理的留言」，执行器见 `session/tools.md`「父子之间留言」）

1. `to` 只能写两种：自己派的、还没被停掉的子代理的编号；`parent`，发给自己的父。别的都拒，拒的时候说清为什么：主会话没有父；不是她派的（没派过、派的是后台命令、派它的那一轮撤掉了）；被停掉了（以 `stopped`、`undone` 报过）。孙代理不能越过子代理找父、兄弟之间不直接说，都落在「不是她派的」：她只认得自己派的编号。做完了报过的子代理照样收留言：它的会话还在，留言开它的下一轮（`10-自带软件.md` 第三节）。
2. 发给子代理：作为父会话发来的 `message.user`（`by` 是父会话，命令编号 `<父会话>/message/<调用编号>`）送进去，原样一块字，和交代一样走「发一条消息」。它在跑，下一步看到（排队的消息）；闲着，开一轮，结束时照样回报（第二条第 1 条）。那次调用报效果 `job.messaged`：它欠一份回报。
3. 发给父：作为子会话发来的 `message.user`（`by` 是子会话，命令编号 `<子会话>/message/<调用编号>`）送进父会话。父会话照回报的规矩收（第三条）：不带回合编号；正忙，下一步看到，最后一步里到的，回合结束时接着开；闲着，开一轮；开不了的只记下（没人看着的一次性会话、还能恢复撤销、要重启了）；派它的那一轮撤掉了的不叫醒。它不是人说的话：打断时不撤回、不由它接着开，不作废在等人回答的题，撤销时留着。渲染时注明是哪个子代理（编号、标题）发来的（第九条第 5 条）。父会话被它开的那一轮不向上回报（第二条第 1 条）。
4. 发了不等：调用返回送到了，接着干。要等回答才干得下去的，结束这一轮、说它在等什么（这一轮是父会话开的，它就照样回报）；父会话的答复用 `message_agent` 送回来，开它的下一轮。
5. 只发对方现在就得知道的：要对方拿主意的问题、会改变对方安排的发现。进展不用报，做完了回报自己会送。说明里写这一句（`26-提示词.md` 附录的草稿，一字不差，登记），模型滥发、一有进展就叫醒父会话，是这一条要防的；真模型实测时专门看（施工单「验收」第 2 条）。
6. 执行器认 `to` 照的是派这次调用那一刻内核交的派出去的子代理（`Session::subagents`）：那以后才派的，她还不知道编号；送出去之前刚被停掉的，送过去它照样收，父会话不再认它的回报，和她停它之前刚发出去一样。
7. 送不到（对方拒收、对方的会话停了、核心正在停）：交回送不到，原因记进运行日志（`session/actor.md`「运行日志」）。

**子代理有事要问人**（2026-09-29 定）：

- 提问走父会话当中介：子代理没有 `ask_user`（M8 做提问工具时，子会话不给，照 Claude Code、codex）。要人拿主意的，问父会话（写进回报，或者用 `to: parent` 中途问）；父会话自己答得了就答，答不了去问人，再把答复送回来。父会话知道交代和人的意图，多数问题它就答了；几个子代理同时有问题，它能合成一次问人；人的答复它也知道，不会和子代理对不上。
- 确认权限直接找人，不经父会话：放行只能是人亲手给的，核心不看聊天放行，父会话替人说允许等于模型给自己放行。`miyu ask` 里照旧拒绝；终端界面里在同一个抽屉问，标明是哪个子代理（`13-终端界面.md`，M8）。

**七、撤销、打断、删除**

1. 撤销一轮，这一轮和跟着撤的几轮派出去、还没结束的一起停下，记 `undone`，不叫醒。撤销的回应列出停掉了哪几个，头照它说一句（`cli/undo.md`）。它们已经做的改动不在撤销范围里（`10-自带软件.md` 第七节）。
2. 撤掉的那几轮派出去的任务，回报不进上下文：派它的调用已经不在有效历史里了。撤销以前就到了、她已经看到的回报照留（别处来的留着，`02-内核.md` 第六节）。
3. 恢复撤销：停掉的不会再起来，记下的 `undone` 照留。
4. 打断父会话的回合，不停子代理、不停后台命令（`02-内核.md` 第七节）。打断子会话的一轮，它停在半路、闲着；不回报，等人说话或者被停。
5. 父会话被删（`session.delete`，施工 3-8 三补，原来排在 7-8）：它派出去的子会话一层层往下停，不问忙不忙；它们和它自己的后台命令整组杀掉、不记回报；子会话的目录和它的一起挪进回收处，各是各的（`protocol.md` 的 `session.delete`、`store.md` 第 12 条）。子会话是照磁盘上 `session.created` 的 `parent` 认的，没在跑的不载入。这时还在交的回报：父会话没了，丢掉（第二条第 5 条）；这时在派的孙代理：父会话已经不在会话表里，不再造（`protocol.md`「会话表」第 7 条）。
6. 删一个子会话本身、它的父会话还在（施工 3-8 三补，2026-09-30 主会话定）：等于人先停掉它再删，照第五条人用 `job.stop` 停子代理的那条路：打断它这一轮、停掉它派的，父会话记 `child.reported`（`stopped`，`by` 是子会话，不带 `by_model`），叫醒父会话；再照第 5 条停它的子会话、后台命令，挪进回收处。报过 `done` 以后父会话又给它留了言、又在等它的（`job.messaged`，施工 7-7），也照这样停、报：执行器的名册把它记回还在跑（`session/tools.md` 第 6 条第 4 款）。父会话已经不在的（删了、连带删的）不送。

**八、载入和重启**

1. 崩了以后载入：有 `job.started`、没有结束记录的后台命令，进程已经没了，补 `job.reported`（`aborted`，`by` 是内核，不变量 8；施工 7-3，`kernel/session.md`「载入和崩溃」第 9 条）。子会话照会话的规矩补 `turn.ended`（`aborted`）；那一轮该向上回报的，照第二条报，`reason` 是 `aborted`，父会话不会一直等（施工 7-6）。
2. 有计划的重启：后台命令记 `restarted`（`by` 是内核，带用时、到这时的输出）、落了盘再杀，这时正好自己退出了的照常记 `exited`，都只记下、不开轮（施工 7-3，`session/actor.md` 第 9 条、`kernel/session.md`「有计划的重启」第 5 条）；子会话被打断的那一轮，重启以后接着干（`kernel/session.md`「载入和崩溃」），做完照常回报；接够了次数、没接着干的，当它崩了，照 `aborted` 报（施工 7-6）。
3. 谁来载入子会话（施工 7-6）：会话照需要才载入，崩了、重启以后没人叫它，它就一直不补报。父会话载入时，执行器把派出去、欠着它一份回报的子会话（一次都还没回报过的，留了言还没报的，施工 7-7）叫起来（会话表载入它们，在跑的不动）；子会话载入时照样叫起它自己的，一层一层下去。回报本身也会叫起父会话：交给没在跑的会话的命令，会话表先载入它。
4. 送到一半崩了（施工 7-6 定）：子会话那一轮结束、落了盘、回报还没落进父会话的日志。子会话每次载入都把它最后报的那一份再交一次，命令编号照报的那一轮，同一份同一个编号；父会话认得出重的（最近 1024 个命令编号，和账本给每个子代理记着的最近一次回报的命令编号），照上一次回应、不再记。所以不漏：崩了以后再交；不重：父会话认出来。`child.reported` 不另加一格记子会话的回合：命令编号里已经有了，`cause` 就是它。

**九、上下文**

1. `child.reported`、`job.reported` 渲染成一块带标签的事实：她闲着时就是开这一轮的那条消息，正忙时排在那一步的工具结果后面（`08-上下文投影.md` 第三节）。写法、标签见 `kernel/request.md`「回报」，原文在 `resources/core/jobs/`，登记在 `26-提示词.md` 第十节（施工 7-2）。
   - 后台命令结束只写结束了、退出码（或信号、为什么停）、用时、输出有多少字，看输出用 `jobs` 的 `output`，不带输出本身（照 Claude Code、dsh）。实测里她每次都接着去读的，再考虑带上结尾几行。
   - 子代理的回报带正文（照 Claude Code、opencode：子代理的通知直接带最后的回复）。
2. `job.started` 不单独渲染：派它的调用和结果就在历史里。
3. 子会话的 system 里多一段场所说明：你是被派出来的，交代来自父会话，最后的回答就是交回去的回报，做完不用去查、不用等（旧版的交付约定，`26-提示词.md` 第五节）。原文 `resources/core/jobs/subagent-venue.txt`，造子会话时接在人设后面、空一行（`policy.md`「拼」），登记在 `26-提示词.md` 第十节（施工 7-5）。能派子代理的会话，system 里写能选的人格、预设（M7 只有软件工程师一个，不写，随预设那一步）。
4. 别的 harness 发来的话渲染时注明来处和名字，照外来的话对待（第十一条）。
5. 子代理发给父会话的留言注明是哪个子代理发来的：一块带标签的事实，标签带编号、标题，里面是它的话，原样（施工 7-7，`kernel/request.md`「子代理的留言」，原文 `core/jobs/subagent-message-{open,close}.txt`，登记在 `26-提示词.md` 第十节）。派它的那一轮撤掉了的不渲染，和它的回报一样。父会话发给子代理的交代、留言原样送，子会话的场所说明已经说了交代来自父会话。

**十、压缩**

1. 检查点里由代码写一段：还在跑的后台命令和子代理（编号、种类、标题），和已经到了、她还没看到的回报（`09-压缩.md` 第八条）。
2. 回报照常在尾巴里；被压进摘要的，`history` 找得回。

**十一、`miyu ask` 和别的 harness**

1. `miyu ask` 等到这一轮结束、这个会话派出去的子代理都回报完、被回报叫醒的几轮也结束，才退出；印的是这期间每一轮的回复，照先后。后台命令不等（`22-命令行.md` 第三节）。
2. `--timeout <时长>`：从发出算到全部了结，到时间打断、不再等，退出码 3。
3. 等子代理时按 Ctrl+C：不等了，后台的照常跑。
4. 别的 harness 发消息：`miyu ask -s <会话编号> --from <名字> <话>`（2026-09-29 项目主人定）。记成 `harness`；她在忙就排队；等这一轮答完印回答，和平常的 `miyu ask -s` 一样。会话编号由人交给它。`--from` 不写 `-s` 的开一个新会话，也记成它发的。

### 样子

事件的样本在 `docs/designs/samples/events/`（施工 7-1）：`tool.result.jsonl` 的 102、103 号带着 `job.started`（一个后台命令、一个子代理；子代理那一句是 `agent` 真交回的（施工 7-5），后台命令那一句是施工 7-3 定的 `started.txt`），`job.reported.jsonl`、`child.reported.jsonl` 是它们的回报，`session.created.jsonl` 的第二行是那个子代理的会话的第一条（`cause` 是父会话发来的造会话的命令，施工 7-5）。

子代理这张脸的请求形状（施工 7-5）：`docs/designs/samples/probe/subagent/`，父会话的交代开了第一轮，她读一个文件再作答；和同一份剧本的主会话比，每一次请求只多 system 里的场所说明。

回报渲染出来的样子（施工 7-2）：`kernel/request.md`「样子：给模型看的字」，每种原因各一份样本在 `docs/designs/samples/reports/`，一段有回报的会话的每一次请求在 `docs/designs/samples/probe/reports/`。

子代理的留言（施工 7-7）：`message.user.jsonl` 的 111 号是样本会话派的子代理 `j2` 发来的一句（`by` 是它的子会话，不带回合编号），`tool.result.jsonl` 的 114 号是她用 `message_agent` 答它、带着 `job.messaged`；渲染出来的样子在 `docs/designs/samples/reports/subagent-message.txt`。

施工时照每一步补：`miyu ask` 等子代理时印的样子（给项目主人看过再定）。

### 给人看的字

`jobs` 的结果那一句见 `tools/jobs.md`「给人看的字」，`unknown_job` 的说法见 `protocol.md`「给人看的字」（施工 7-4）。施工时照每一步补：撤销停掉任务的那一句、`miyu ask` 等子代理的旁白，两种语言。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/event/effect/tests.rs`、`event/job/tests.rs`、`event/session/tests.rs`、`id/tests.rs`、`origin/tests.rs` | 三种新东西、两格新字段、任务编号、`harness` 读写一字不差，不认识的取值原样留着（施工 7-1） |
| `crates/miyu-kernel/src/ledger/tests/jobs.rs`、`jobs/reports.rs` | 账本的任务规矩（施工 7-1） |
| `crates/miyu-kernel/tests/samples.rs` | 样本读写一字不差，子代理的几条对得上（施工 7-1） |
| `crates/miyu-kernel/src/session/tests/scenario/reports.rs`、`reports_undo.rs` | 回报到了开一轮、排着、只记下、接着开，撤销、恢复、载入、读回日志的时候到的（施工 7-2，`kernel/session.md`「守着它的」） |
| `crates/miyu-kernel/src/session/tests/random/watch/reports.rs`、`random/reporting.rs` | 随机输入里的回报（施工 7-2） |
| `crates/miyu-kernel/src/history/tests/jobs.rs` | 有效历史记着的派出去过的任务；由回报接着开的一轮撤掉时拿走什么（施工 7-2） |
| `crates/miyu-assemble/src/jobs/tests.rs`、`tests/probe.rs` | 回报的写法和样本一字不差、排在哪；有回报的会话的请求形状（施工 7-2）；子代理这张脸的请求形状，和主会话只差场所说明（施工 7-5） |
| `crates/miyu-basesystem/tests/agent.rs` | `agent` 的参数、交给端口、结果和效果、派不了（施工 7-5，`tools/agent.md`） |
| `crates/miyu-session/tests/spawn.rs`、`spawn_log.rs`，`src/job_ids.rs` | 子会话抄对了父会话的每一样、交代记成父会话发的、一步里派几个；编号接着日志往下数、不回收；只有本机、没到深度上限的会话有 `agent`，子会话的 system 接上场所说明；运行日志（施工 7-5） |
| `crates/miyu-endpoint/tests/spawn.rs` | 真核心走一遍：替身模型在子会话里答话，子会话的日志、快照、请求，`session.list` 的 `parent`（施工 7-5） |
| `crates/miyu-kernel/src/ledger/tests/jobs.rs` | 用过的最大任务编号：撤掉的、不认识的种类也算（施工 7-5） |
| `crates/miyu-basesystem/tests/background.rs`、`shell_sandbox.rs` | `shell` 的后台命令：真的进程、三个平台（施工 7-3，`tools/shell.md`「守着它的」） |
| `crates/miyu-session/src/jobs/tests.rs`、`job_ids.rs`、`tests/jobs.rs` | 任务表、编号、会话里真的后台命令、停下记 `restarted`、载入补 `aborted`、沙盒（施工 7-3，`session/tools.md`「守着它的」） |
| `crates/miyu-kernel/src/session/tests/scenario/commands.rs` | 用过的最大编号、载入补 `aborted`、要重启了以后只记下（施工 7-3） |
| `crates/miyu-core/tests/serve.rs` | 有后台命令不空闲退出；收到停的信号先记 `restarted` 再杀（施工 7-3） |
| `crates/miyu-kernel/src/session/tests/scenario/upward.rs`、`upward_load.rs`、`session/report/tests.rs` | 子会话向上回报的每一种、孙代理都报完再报、截头尾、`person` 的两种、出错和步数上限照报、空的、载入补 `aborted`、再交一次、父会话认出重交的（施工 7-6，`kernel/session.md`「守着它的」） |
| `crates/miyu-session/tests/report_up.rs` | 执行器交回报：命令编号、`by`、任务编号照造它的命令读回；载入再交一次；父会话载入以后叫起还没回报的子会话（施工 7-6） |
| `crates/miyu-endpoint/tests/reports.rs` | 真核心走一遍两层：孙代理报给子代理、叫醒它，子代理再报给主会话、叫醒她（施工 7-6） |
| `crates/miyu-basesystem/tests/jobs.rs`、`src/jobs/page/tests.rs` | `jobs` 三个动作的输出一字不差、分页、说法（施工 7-4，`tools/jobs.md`） |
| `crates/miyu-session/tests/jobs_stop.rs`、`jobs_stop/agents.rs`、`src/jobs/roster/tests.rs`、`peek/tests.rs` | 会话里列出、读、停：她停的不叫醒、人停的叫醒；停和自己退出撞在一起只认先到的；子代理经会话表停、回报记成它交来的；父会话停下时连它派的一起停、都不叫醒；列出来的照日志、最近结束的 5 个；子代理在做什么；回报正文照向上回报的截法截（施工 7-4） |
| `crates/miyu-endpoint/tests/job_stop.rs` | 真核心：人停子代理，子会话那一轮被打断，父会话记下回报，回应 `{}`；没有、停过的、编号不合写法、没有这个会话的拒绝，两种语言（施工 7-4） |
| `crates/miyu-kernel/src/session/tests/scenario/messages.rs`（施工 7-7） | 子代理的留言到了：闲着开一轮、正忙下一步听到、最后一步里到的接着开、不带回合编号、打断不撤回不接着开、不作废在等人的题、没人看着只记下、派它的那一轮撤掉了的不开轮、能恢复撤销时记在一边、载入算回来、由它接着开的一轮撤掉时留言留着；交给执行器的派出去的子代理；中间一层答了孙代理的留言不向上报、孙代理报完再报，孙代理在回报里问、它留言答了也一样（`kernel/session.md`「守着它的」） |
| `crates/miyu-kernel/src/ledger/tests/jobs.rs` 的 `a_message_to_a_subagent_makes_it_owe_a_report`、`a_report_that_arrives_while_the_message_is_on_its_way_answers_it`，`event/effect/tests.rs` | `job.messaged`：只能给这个会话派的子代理，留了言它欠一份回报、报了就不欠，调用发出以后到的回报算回了；读写一字不差（施工 7-7） |
| `crates/miyu-assemble/src/jobs/tests/messages.rs` | 子代理的留言渲染成带标签的一块，和样本一字不差；别人发的原样；排在哪；派它的那一轮撤掉了的不渲染；旧快照没有标签的只剩它的话（施工 7-7） |
| `crates/miyu-basesystem/tests/message_agent.rs`、`crates/miyu-session/tests/messages.rs`、`messages_log.rs` | `message_agent` 和执行器这一头（施工 7-7，`tools/message_agent.md`「守着它的」） |
| `crates/miyu-endpoint/tests/messages.rs` | 真核心走一遍三层：孙代理问子代理、停下来等，子代理答它，孙代理做完报上来，子代理把整件活报给主会话，只报一次（施工 7-7） |
| `crates/miyu-kernel/src/session/tests/scenario/reports.rs` 的 `a_subagent_she_stopped_herself_only_records`，`event/job/tests.rs` | 她停的子代理只记下；`child.reported` 的 `by_model` 读写一字不差；随机测试里子代理的回报也有她停的（`random/reporting.rs`）（施工 7-4） |

以后照每一步补。至少：内核的单元测试和场景（开始、结束、撤销停掉、载入补中断、子会话回报的条件）；工具面的预算；真核心走一遍后台命令。

### 出处

- `02-内核.md` 第六节（排队的消息、撤销）、第七节（多会话并行）、不变量 8。
- `03-事件模型.md` 第三节（三种事件，`child.spawned` 改由效果记）。
- `08-上下文投影.md` 第三节。`09-压缩.md` 第八、九节。
- `10-自带软件.md` 第三节、第五节。`11-权限与沙盒.md` 第二节。`12-进程形态与分发.md` R1、R5。
- `22-命令行.md` 第三节、第四节。`26-提示词.md` 附录。

### 还没有的

- 挡位、人格、预设：`agent` 的 `tier`、`persona`、`preset` 三个参数随配置和预设那一步加，加的时候工具面变一次（`15-模型与供应商.md`、`16-人格与预设.md`）。
- 会话树、切进子会话、确认抽屉里标明是谁在问：终端界面（M8）。
- 资源调度器：并发上限、限速、优先级（`02-内核.md` 第七节）。
- 放行规则跟着子会话抄：执行前的链认放行规则以后。
- `miyu session show`、`miyu stdio`：管理命令那一步。
- 会话 actor 在订阅、退订时交 `Watched`（`kernel/session.md`「回报」第 6 条）：随 `miyu ask` 等子代理（7-9）；这之前一次性的会话一直当没人看着，回报只记下。
