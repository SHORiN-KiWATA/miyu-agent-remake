## 会话 actor

### 是什么

把内核的会话状态机接上磁盘和外面的世界：一个会话一个异步任务。人的命令、执行器的回报进它的收件箱，一条条送进内核；内核交出的动作，它照表一个个做：写盘、回应、推送、请求模型、执行工具、改回文件。执行工具、效果、她看过的、改回文件另见 `session/tools.md`，执行前的权限策略见 `session/guard.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-session/src/open.rs` | 造会话、载入：备好磁盘上的，交给内核，起 actor |
| `crates/miyu-session/src/open/error.rs` | 造不成、载入不了的几种（施工 3-8 七补从 `open.rs` 挪出来） |
| `crates/miyu-session/src/actor.rs` | actor 本身：收件箱、一批批送进内核、每个动作怎么回、停下 |
| `crates/miyu-session/src/actor/mail.rs` | 人的那条收件箱里的一封怎么办；数着拿着订阅的头，交内核 `Watched`（施工 7-9 从 `actor.rs` 挪出来） |
| `crates/miyu-session/src/actor/back.rs` | 执行器的那条收件箱里的一封，写成内核的输入（施工 C-6 从 `actor.rs` 挪出来：那个文件到了行数上限） |
| `crates/miyu-session/src/actor/watchers.rs` | 「空了告诉我」被等的这一边：谁在等这个会话空下来，每送完一批看空没空，空了发通知（施工 C-6，下面「被等的名单」） |
| `crates/miyu-session/src/peers.rs` | 「空了告诉我」等的这一边：照内核在等的去订、计时，到点、不在了交回（施工 C-6，`session/tools.md`「订、计时、再订」） |
| `crates/miyu-session/src/actor/model.rs` | 请求模型：交给端口、叫停、说完了记一行；回顾的请求也在这里（施工 3-8 四补） |
| `crates/miyu-session/src/actor/stop.rs` | 有计划地停下：要重启了、后台命令记 `restarted`、落了盘再整组杀（施工 7-3） |
| `crates/miyu-session/src/actor/store.rs` | 写盘；撤掉压缩时读回日志（施工 6-9） |
| `crates/miyu-session/src/handle.rs` | `Handle`：发命令、订阅、停下；推送和订阅；订阅放下时告诉 actor（施工 7-9） |
| `crates/miyu-session/src/backlog.rs` | 订阅时要补发的那一截：补到哪一条、在阻塞线程里读出来（施工 3-8 六补） |
| `crates/miyu-session/src/config.rs` | 会话从哪取配置（`ConfigSource`、`Configs`、`fixed`），回合开始时冻结的一份（`TurnConfig`）；造会话、载入时先取一份（施工 8-4） |
| `crates/miyu-session/src/port.rs` | 请求模型的端口：`Models`、`ModelPort`、`Reports`（辅助请求的回报另走一路，`Reports::aside`，施工 3-8 四补；五补起回顾、起标题共用，`purpose()` 交回用途）、`Cancel` |
| `crates/miyu-session/src/route.rs`、`route/send.rs`、`route/pool.rs`、`route/choice.rs`、`route/ended.rs` | 端口的真实现：每个会话的路由，照配置挑供应商、钉 key，经驱动和 HTTP 执行器请求（施工 8-6 取代 `http.rs`）；池里挑成员、池的限额（`route/pool.rs`，施工 8-8）；排候选、挑没在冷却的（`route/choice.rs`），说完了记冷却、换端点、成了才钉（`route/ended.rs`，施工 8-9） |
| `crates/miyu-session/src/clock.rs` | 会话的时钟、新的会话编号 |
| `crates/miyu-session/src/store.rs` | 写盘的端口：平时是会话日志，每落一批顺手更新会话列表的索引（`Indexed`，施工 3-8 七补），测试里换成写不进去的；也从这里读回日志（施工 6-9） |
| `crates/miyu-session/src/kinds.rs`、`lines.rs` | 运行日志里的输入、动作种类名，和几种写法 |
| `crates/miyu-session/src/blocking.rs` | 在阻塞线程里做完磁盘上的事 |
| `crates/miyu-session/src/tools.rs`、`effects.rs`、`restore.rs` | 执行工具、效果、改回文件（`session/tools.md`） |
| `crates/miyu-session/src/jobs.rs`、`job_ids.rs` | 执行器的任务表、任务编号（`session/tools.md` 第 5 条，施工 7-3）；列出、读、停（第 6 条，施工 7-4） |
| `crates/miyu-session/src/actor/halt.rs` | 停掉任务：人停一个、父会话停下时全停（施工 7-4）；撤销停掉那几轮派出去的、人删了的子代理当场记回报（施工 7-8） |
| `crates/miyu-session/src/reread.rs` | 压完重读文件、照 blob 取回原文（`compaction.md` 第九条） |
| `crates/miyu-session/src/guard.rs` | 权限策略（`session/guard.md`） |
| `crates/miyu-session/src/spawn.rs`、`agents.rs`、`job_ids.rs` | 造子会话的端口、派子代理、领任务编号（施工 7-5，`session/tools.md`「派子代理」） |
| `crates/miyu-session/src/report.rs` | 向上回报：子会话把内核交出的回报经端口交给父会话；父会话载入以后叫起还没回报的子会话（施工 7-6） |
| `crates/miyu-session/src/testkit.rs` | 测试用的、照剧本回的端口，`testkit` 开关打开才有。起标题的请求另排剧本（`Script::titles`），记在 `titled()` 里，不占主剧本；没排的只记下、不回（施工 3-8 五补：它每一轮答完自己来，不管标题的测试不用替它排） |

### 对外的样子

| 名字 | 做什么 |
|---|---|
| `create(Create)` | 造一个会话，`session.created` 落了盘才交回 `Handle` |
| `load(Load)` | 从磁盘载入一个会话，交回 `Handle` |
| `new_id(时刻)` | 一个新的会话编号 |
| `Handle` | 一个会话的收件箱，可以复制，几个头一起拿着 |
| `Pushed`、`Subscription`、`Ended`、`Stopped` | 推送、订阅、订阅断了、会话停了 |
| `Backlog` | 订阅时要补发的那一截（施工 3-8 六补）：`upto()` 补到哪一条，`read()` 在阻塞线程里读出来（第 6 条） |
| `Models`、`ForSession`、`ModelPort`、`Reports`、`Cancel` | 请求模型的端口。`ForSession` 带会话编号、造会话或载入时取的那一份配置（施工 8-6）、驱动的占位、属主的 blob，会话记着的引用、最近一次发给了谁（施工 8-8）；`ModelPort::model()` 交回的是一份（路由的会变，施工 8-6），`reference()` 交回会话这时生效的引用（施工 8-8，测试的端口交造它的会话记着的） |
| `SessionPort`、`Child`、`Lineage`、`Pending` | 造子会话、给别的会话发命令的端口（施工 7-5）：会话表实现，造会话、载入时交进来。`create(子会话)`、`command(会话, 编号, 谁, 命令)`，`open(会话)` 叫起一个会话：没在跑的照会话表的规矩载入（施工 7-6） |
| `Routes`、`IDLE` | 端口的真实现：每个会话的路由，照配置挑供应商、钉 key（施工 8-6，第 8 条）；空闲超时 180 秒 |
| `Jobs` | 执行器的任务表，核心里一张：`Jobs::new()`，`running()` 有没有在跑的后台命令（结束了、记录还没落盘的也算，施工 7-3） |
| `Unreadable` | 头读不了这个任务的输出：`Unknown` 没有这个任务，`Agent` 是子代理（施工 7-4 补） |

`Create` 的格：数据根 `root`、资源目录 `resources`、会话编号 `id`、人格 `persona`、场所 `venue`、属主 `owner`、开始时的权限 `permission`、有没有人能确认 `attended`、一次性的 `oneshot`、环境 `environment`（时区、工作目录）、造会话的命令编号 `command`、谁发的 `by`、造端口的 `models`、工具目录 `tools`、系统的家目录 `home`（读不出来的是空的）、沙盒的助手 `sandbox`（这台机器上的沙盒能用才有，施工 5-4 上）、沙盒的缓存 `sandbox_cache`（`<缓存目录>/sandbox/<属主>`，核心算不出缓存目录的没有，施工 5-4 下）、父会话和第几层 `lineage`（子会话才有，施工 7-5）、造子会话的端口 `sessions`（会话表交进来的，测试里自己造的没有，施工 7-5）、任务表 `jobs`（核心里那一张，施工 7-3）、属主的会话列表的索引 `index`（会话表交进来的，测试里自己造的可以没有，施工 3-8 七补，`store/index.md`）。`Load` 的格：`root`、`owner`、`id`、`environment`、`models`、`tools`、`home`、`sandbox`、`sandbox_cache`、`sessions`、`jobs`、`index`。

| `Handle` 的方法 | 做什么 |
|---|---|
| `id()` | 会话编号 |
| `busy()` | 有没有在跑的回合：核心看它决定能不能空闲退出（`core.md`）。停了的会话不算 |
| `oneshot()` | 一次性的会话：`miyu ask` 开的（施工 C-5，`cross-session.md` 第三条第 4 款）：`send_message` 发给它的时候，照它和 `watched()` 决定说送到了还是存下了 |
| `watched()` | 这时有没有至少一个头订阅着（施工 C-5）：和 `busy()` 一样是一面共用的旗，拿着订阅的头从没有到有、从有到没有时（见下面「人的那条收件箱」第 6 条）一起写；造会话、载入以后是假的，和内核一样当没人看着 |
| `limits()` | 给头看的限额：窗口、压缩线（`kernel/session.md` 的 `ContextLimits`）。造会话、载入时交完限额向内核要的；和 actor 共用（`Shown`），钉住的池出错换了成员（施工 8-9，第 7 条第 8 款）、回合开始重新解析换了的（施工 8-10，第 4 条「跑回合开始的挂接点」）跟着换；协议照它回 `subscribe`（`protocol.md`，施工 6-3 补） |
| `next()` | 会话接下来请求的模型（`Next`，施工 8-10）：引用、接下来发给谁（轮换的池、解析不出的没有）。和 `limits()` 住在同一份 `Shown` 里，一起写；协议照它写 `subscribe` 回应的 `model` |
| `command(编号, 谁, 命令)` | 发一个命令，等回应：接受的，它产生的事件落了盘才回；拒绝的当场回。编号由发的一方生成，同一个编号只生效一次（`kernel/session.md`） |
| `subscribe()` | 订阅：从这一刻起的推送 |
| `subscribe_after(after)` | 订阅，连同补发（协议的 `subscribe` 带 `after`，施工 3-8 六补）：交回从这一刻起推的订阅，和日志里序号大于 `after`、这一刻落了盘的那一截（`Backlog`，第 6 条） |
| `stop()` | 有计划地停下，停好了才回 |
| `stop_job(编号, 谁, 命令编号)` | 人停掉派出去的一个任务（协议的 `job.stop`，施工 7-4）：回报落了盘才回；没有、已经结束了的交回 `JobError` |
| `job_output(编号)` | 头读一条后台命令到这时为止的输出（协议的 `job.output`，施工 7-4 补）：交回读得到的字（`miyu_tool::Output`）、还在不在跑，和她用 `jobs` 读的是同一份（`session/tools.md` 第 6 条第 3 款）；没有这个任务、是子代理的交回 `Unreadable`。不进内核、不写盘 |
| `stop_jobs(谁, 命令编号)` | 停掉这个会话派出去、还没结束的全部，连它们派的（父会话停下它时，会话表经端口来调，施工 7-4）：都带 `by_model`、不叫醒它，停好了才回 |
| `delete()` | 删会话之前停下（施工 3-8 三补）：内核说删不了的交回原因（`TurnRunning`、`Restoring`），会话照常；删得了的停下，日志关了才回（第 9 条） |
| `discard()` | 同 `delete()`，只是不问删不删得了：父会话被删，子会话一起停（`agents.md` 第七条第 5 条） |
| `environment(环境)` | 环境变了：工作目录、时区 |

`Pushed` 有两种：`Events`，落了盘的几条事件，照先后；`Transient`，一条瞬时事件，不落盘。`Subscription` 有 `next()`（等下一份）、`try_next()`（不等，没到的是空的）；断了的是 `Ended::Lagged`（掉了队）或 `Ended::Stopped`（会话停了）。拿着一个 `Subscription` 就算一个在看着这个会话的头，放下它（丢掉、连接断了）自己告诉 actor（施工 7-9，第 3 条）。

端口：`Models::port(ForSession)` 给一个会话造端口，`ForSession` 带这个会话的驱动占位（取自策略快照）和属主的 blob。`ModelPort::model()` 交回端点的编号和模型名；`ModelPort::call(seen, 请求, Reports, Cancel)` 马上返回，在别的任务里发。`Reports` 有 `sent(模型, 请求字节的哈希)`、`delta(增量)`、`ended(用量, 出错, 要等多久)`；`Cancel::wait()` 等到被叫停。

### 怎么走

**1. 造会话**（`create`）

1. 在阻塞线程里依次做，哪一步不成就交回那一种错，actor 不起；已经存下的快照留着：
   1. 读出这个人格要用的原文（`store/resources.md`）。
   2. 拼策略快照：人格、有没有人能确认、工具目录里每件工具的名字、说明、参数格式、访问类别，照名字排（`policy.md`）。不在本机、到了深度上限的，工具面里不给 `subagent`；子会话读出场所说明（`core/jobs/subagent-venue.txt`，读不了的算人格读不出来），接在 system 的人设后面（施工 7-5，`session/tools.md`「工具面」）。
   3. 照快照造内核的策略、驱动的占位、替工具写的两句（`session/tools.md`）、权限策略拒绝时的三句（`session/guard.md`）。
   4. 快照存成属主的 blob：先落 blob，再写引用它的事件。
   5. 建会话目录和空的第一段（`store.md`）。
2. 造请求模型的端口。时钟从现在起。
3. `session.created` 写属主、场所、快照的哈希、开始时的权限，`oneshot` 照交进来的，子会话写 `parent`、`depth`（施工 7-5）；交给内核造会话，`cause` 是造会话的命令，连同这个会话自己的编号 `id`：事实 `session` 写它，子会话的是它自己的（施工 1-13 再补）。
   马上交给内核这个模型的限额（`Input::Limits`，端口的 `limits()`：窗口、最大输出、一张图怎么算，施工 6-3 上），在别的输入之前；什么动作都不出。接着向内核要一份给头看的限额（`context_limits()`），交回的 `Handle` 和 actor 共用它（施工 6-3 补；施工 8-9 起会变）。
4. 造权限策略、执行工具的端口（她看过的是空的；任务编号照内核的 `last_job_number()` 往下数，派子代理要照抄的那一份照交进来的，施工 7-5；actor 建它那一份任务表，和派子代理共用这一串编号，施工 7-3）、actor；子会话交回报的那一头（「向上回报」，施工 7-6）；记下造会话的命令在等回应。
5. 在会话的 span 里记一行 `created`，起 actor。
6. 等回应：`session.created` 落了盘，内核回应这个命令，交回 `Handle`。actor 在那之前停了的，交回 `CreateError::Stopped`（「出错」一节），在阻塞线程里删掉这个会话的目录：只剩一段空的第一段时才删，别的不动（施工 4-9 再补四下：原来留在磁盘上）。已经存下的快照留着：按内容存，别的会话可能也在用，回收随 blob 回收那一步。

**2. 载入**（`load`）

1. 在阻塞线程里依次做，哪一步不成就交回那一种错：
   1. 打开会话日志：自检，截掉最后一段末尾那半行（`store.md`）。
   2. 第一条要是 `session.created`；日志是空的、第一条不是它的，报错。
   3. 照它记的哈希从属主的 blob 里取快照，读懂，造策略、驱动的占位、两句、三句。
2. 时钟从日志里最后一条的时刻起：系统时间比它还早（往回拨过），照它。
3. 从日志里的效果重建她看过的（`session/tools.md`）；记下最后一条发出去了的 `model.called` 发给了谁（施工 8-8）。
4. 交给内核载入，连同会话的编号 `id`（施工 1-13 再补）：交回会话，和一串要回的动作。有计划的重启打断了的一轮接着干，崩了的那一轮标成没走完（`kernel/session.md`）。
5. 造请求模型的端口：引用照内核从日志算的（`Session::reference()`，施工 8-10：换过模型的是换过以后的），钉住的池照第 3 条记下的认回钉着的成员。
   马上交给内核这个模型的限额，同上：接着干的那一轮，发主请求之前就知道限额（施工 6-3 上）；给头看的那一份也同上（施工 6-3 补）。检查点重读过的文件，内核在那一串动作的第一个交出 `Recall`，照下面第 4 条读（施工 6-9：认哪个检查点还算数是内核的事，执行器不自己找）。
6. 造权限策略、执行工具的端口（任务编号、派子代理要照抄的那一份照日志里的 `session.created` 和快照，施工 7-5）、actor；子会话交回报的那一头（「向上回报」，施工 7-6）；记一行 `loaded`；叫起还没回报的子会话（内核的 `waiting_children()`，一个一个起任务叫、不等：会话表这时正拿着表的锁载入它，等载入完才轮得到，施工 7-6）；起 actor，先回那一串动作。
7. 马上交回 `Handle`，不等那一串动作做完。

**3. 收件箱**

1. 一个会话一个 tokio 任务，带着会话的 span：`error_span!`，目标 `miyu::session`，名字 `session`，一格 `session` 是会话编号。开在 `ERROR` 级，调到 `WARN` 也筛不掉，底下的行都带着会话编号（`log.md`）。外面再套一个看着它的任务。
2. 两条通道，都不设上限：
   - 人的：`Handle` 发来的命令、订阅、放下了订阅（施工 7-9）、停下、删之前停下（施工 3-8 三补）、环境变了、读后台命令的输出（施工 7-4 补）、别的会话在等它空下来（施工 C-6，会话表经 `Handle::watch` 交）。拿着 `Handle` 的都放下了，它就关了：订阅放下时往里送一声拿的是弱的一头，不因为还有订阅就不关。
   - 执行器的回报：请求的回报、到点了、工具的回报、后台命令结束了（施工 7-3）、等的会话等不到了（施工 C-6）。actor 自己也拿着一头，它不会自己关。
3. 两条都有的时候，先收执行器的回报：读流不断。
4. 人的一封：

   | 来的 | 怎么办 |
   |---|---|
   | 命令 | 记下等它回应的那一头，照 actor 的时钟记下到的时刻，送进内核 |
   | 订阅 | 当场交回一个订阅，连同这一刻落了盘的最后一条、日志的只读入口（施工 3-8 六补，第 6 条）。拿着订阅的头从没有变成有，送 `Watched { watched: true }` 进内核（施工 7-9），和 `Handle::watched()` 共用的那面旗一起写（施工 C-5） |
   | 放下了订阅（施工 7-9） | `Subscription` 被丢掉时自己送来（要订阅、送进来了、没等到回答就不等了的也送）。拿着订阅的头从有变成没有，送 `Watched { watched: false }` 进内核；别的不进内核。造会话、载入时是 0 个，和内核一样当没人看着（`kernel/session.md`「回报」第 6 条），`Handle::watched()` 同一时刻也是假的 |
   | 环境变了 | 送进内核：不当场注入，到下一个边界再查（`kernel/session.md`） |
   | 停下 | 第 9 条 |
   | 停掉任务（施工 7-4） | 后台命令当场在阻塞线程里杀、存，回报当场交进内核、落了盘再回；子代理另起一个任务经会话表去停，回报送回来落了盘再回：不在收件箱里等，回报才送得进来（`crates/miyu-session/src/actor/halt.rs`，`session/tools.md` 第 6 条）。人删了的子代理（施工 7-8）：它已经停了，回报当场作为子会话交来的命令交进内核、落了盘再回（`session/tools.md` 第 6 条第 10 款） |
   | 删之前停下 | 第 9 条 |
   | 读后台命令的输出（施工 7-4 补） | 当场照名册看是什么，另起一个任务开文件、交回：不进内核、不写盘，不在收件箱里等（`crates/miyu-session/src/actor/mail.rs`，`session/tools.md` 第 6 条第 3 款） |
   | 有会话在等它空下来（施工 C-6） | 记进名单（「被等的名单」），不进内核、不写盘；上不上膛照「被等的名单」第 3 款判（2026-10-01 改：不是这时已经空了就当场发） |

5. 执行器的一封，照 actor 的时钟记下到的时刻：

   | 来的 | 送进内核的 |
   |---|---|
   | 请求发出去了 | 发给了哪个模型、请求字节的哈希 |
   | 一段增量 | 增量 |
   | 请求说完了 | 先记一行收场（第 7 条），再送用量、出错（分类和原话）、供应商说要等多久 |
   | 到点了 | 为哪一次请求等的 |
   | 工具的回报 | 见 `session/tools.md`；已经叫停了的不理 |
   | 后台命令结束了（施工 7-3） | `JobEnded`：`by`、`cause`、`body` 照任务表交来的；记下它，这一批落了盘从任务表里拿掉（`session/tools.md` 第 5 条第 3 款） |
   | 等的会话等不到了（施工 C-6） | `WatchEnded`：哪个会话、`expired` 还是 `gone`（`session/tools.md`「订、计时、再订」） |

6. 一封送进内核，内核交回一串动作，照第 4 条一个个做。当场就能回的输入（落盘了、挂接点跑完了、链判完了、改回了、工具不在目录里的结果）不回收件箱，排进本地的队列，接着送，队列空了才收下一封：它们先于收件箱里的任何一封。
7. 每送完一批：交进去的后台命令结束从任务表里拿掉（这一批都落了盘了，施工 7-3）；照内核在等的去订、计时（施工 C-6，`session/tools.md`「订、计时、再订」），空了就给在等它的发通知（「被等的名单」）；最后照内核说的空不空闲，写一次「有没有在跑的回合」：通知先交出去，核心不会在通知的路上空闲退出。

**4. 每个动作怎么做**

| 动作 | 做什么 | 当场送回 |
|---|---|---|
| 追加事件 | 在阻塞线程里写一批、同步（第 5 条） | 落盘了，到这一批最后一条为止 |
| 回应命令 | 交给等这个编号的最早那一头；它不等了，丢掉；没人在等的，不理 | |
| 推送事件 | 推给订阅了的；没有订阅的，丢掉 | |
| 推送瞬时事件 | 同上；是 `status`（现在只有等着重试这一种）的，先记一行 `retrying`；是 `compaction.done` 的，先记一行 `compacted`（施工 6-3 下；`trigger` 照它的 `trigger`，施工 6-8） | |
| 跑回合开始的挂接点 | 先冻结这一轮的配置：从配置的 `watch` 取当前的一份，照会话这时的目录带上项目配置（阻塞线程里，施工 8-4，`config.md` 第八条第 3 条）。再叫端口照它重新解析内核交来的引用（施工 8-10，`ModelPort::turn`，第 8 条第 2 款）：限额和上一次交给内核的不一样的当场交 `Input::Limits`，给头看的（`Shown`）照内核算的限额、端口的引用和模型写一次；头看得到的（引用、接下来发给谁、窗口、压缩线）和这一步之前的不一样，推一条 `model.changed`，`why` 是 `turn`。现在没有模块挂它 | 挂接点跑完了，没有注入；端口退回了默认的带着 `replaced` |
| 请求模型 | 交给端口（第 7 条），带上这一轮的配置（施工 8-4）：回顾、起标题、手动压缩这些不开回合的，照上一轮的 | |
| 到点叫醒 | 起一个定时的任务，到那一刻送回「到点了」；那一刻已经过了的，马上送 | |
| 不要这次请求了 | 叫端口停下（第 7 条） | |
| 跑回合结束的挂接点 | 现在没有模块挂它，什么都不做 | |
| 过执行前的链 | 权限策略在阻塞线程里判，等它判完（`session/guard.md`） | 链判完了 |
| 执行工具 | 照调用编号和这一轮的 `cause` 造一个任务端口（施工 7-3），一起交给执行工具的端口（`session/tools.md`） | 目录里没有这件工具的：一条出错的结果 |
| 停下工具 | 掐掉跑它的任务（`session/tools.md`） | |
| 改回文件 | 在阻塞线程里一步步做完，这期间不收收件箱（`session/tools.md`） | 改回了，一步一项结局 |
| 压完重读（`Reread`，施工 6-5） | 在阻塞线程里一个一个读：照安全打开（`fs.md`），超过上限的不读完，不是普通文件、读不了、不是 UTF-8 的算读不到；读到的存进这个会话的 blob。这期间不收收件箱 | 一个一项：读到了（`blob`、原文）、太大、读不到（`compaction.md` 第九条） |
| 读回日志（`ReadBack`，施工 6-9） | 在阻塞线程里只读地一段一段读这个会话的日志（`store.md` 的 `read_segments`），只留第 `from` 条起的。这期间不收收件箱。读不了的（日志坏了、磁盘出错）：记一行 `read back failed, stopped`，会话停下（第 9 条），和写不进去一样：从磁盘重新载入最清楚 | 读回的事件，从第 `from` 条到最后一条（`kernel/history.md`「撤掉压缩」） |
| 取回原文（`Recall`，施工 6-9） | 在阻塞线程里照 blob 一个一个读这个会话的 blob，读不出来的、不是 UTF-8 的跳过。这期间不收收件箱 | 读出来的原文，照 blob 找（`kernel/history.md`「重读的原文」） |
| 把回答交给工具 | 现在没有工具会问：记一行 `ERROR` | |
| 向上回报（`Report`，施工 7-6） | 交给交回报的那一头，不等（「向上回报」）；没有那一头的（测试里自己造的子会话）交不出去 | |
| 停掉撤掉的那几轮派出去的（`StopJobs`，施工 7-8） | 后台命令当场在阻塞线程里整组杀掉、存好输出，回报经执行器的回报通道交回；子代理另起一个任务经会话表停，不等（`session/tools.md` 第 6 条第 9 款） | |

**向上回报**（施工 7-6，`report.rs`，`agents.md` 第二条第 5 条）

1. 子会话才有交回报的那一头：造会话、载入时，有父会话（`lineage`、日志里 `session.created` 的 `parent`）、有造子会话的端口、造它的命令编号读得出任务编号（`<父会话>/<编号>`，派子代理时写的）才造。有父会话、读不出任务编号的，记一行 `subagent without a job id`，它的回报交不出去。
2. 内核交出的回报补上任务编号、这个子会话，写成 `child.reported` 的正文，经端口交给父会话：命令 `Report`，`by` 是这个子会话，命令编号 `<子会话>/report/<报的那一轮>`。同一份再交（载入时），编号一样，父会话认得出是重的（`kernel/session.md`「回报」第 10 条）。
3. 一个会话一个任务，照先后一个一个交，等父会话回应再交下一个：不挡着 actor，先后不乱。actor 退出以后，已经交进来的照样交完。
4. 父会话拒绝、原因是 `unknown_job` 的，退避着再交同一份（同一个命令编号）：等 100 毫秒，每次翻倍，一共等到 30 秒（`RETRY_FIRST_MS`、`RETRY_TOTAL_MS`）。子代理做得快，回报可能赶在父会话记下派它的那次调用之前，派它的调用一落盘就对得上了（`agents.md` 第二条第 5 条，施工 7-6）。等着的时候后面的回报排着，先后不乱。
5. 父会话接受了记一行 `reported`；拒绝了（别的原因，或者 `unknown_job` 等满了）记 `report refused`，写原因码；交不到（父会话没了、核心正在停、父会话停了）记 `report not delivered`。都丢掉，不再交：父会话没了的本该一起停了（`agents.md` 第七条第 5 条）；别的拒绝再交也一样。
6. 父会话载入以后，执行器照内核的 `waiting_children()` 经端口的 `open` 叫起欠着它回报的子会话（还没回报过的，留了言还没报的，施工 7-7）（「载入」第 6 条）：崩了的由它们自己载入时补报，重启打断的接着干（`agents.md` 第八条第 3 条）。叫不起来的记一行 `subagent not woken`。

**被等的名单**（施工 C-6，2026-10-01 项目主人定改了第 3 款，`actor/watchers.rs`，`cross-session.md` 第六条第 3 到 6、9 款）

1. 谁在等这个会话空下来，只记在 actor 的内存里：等的会话，它这次订的起算时刻，和有没有「上膛」。不进内核、不进日志，核心一停就没了（等的那一边载入以后会再订）。同一个会话只记一个，后订的替掉先订的。头不显示「有会话在等你」（`cross-session.md`「定的」第 3 条）。actor 还顺手记两样内存里的状态：上一批送完时这个会话还忙不忙，和最近一次从忙变空的时刻——都不进内核、不进日志，核心一停就没了。
2. 会话表经 `Handle::watch(等的会话, 起算时刻)` 交来；会话没载入的，会话表先载入它。
3. 订进来的这一刻：这个会话正忙着，或者起算时刻不晚于它上一次从忙变空的时刻（带话又订、这边手快先忙完了一轮的情形），当场上膛；不然不上膛，留着等它下一次忙完（2026-10-01 改：空着就当场发会带出它上一轮的旧回答，和真实订阅的用意不符）。每送完一批看一次：忙着的，名单上每一项都上膛；闲着、刚才还记着忙过的，记下这一刻刚忙完。然后内核说空了（`vacant()`：空闲、派的子代理都报完了、没收到「要重启了」）、名单上有上膛的，给它们每一个发通知、清掉；没上膛的留着，等下一次忙完。
4. 发通知：一个一个起任务发，不挡着 actor。经会话表给等的那个会话一个命令 `PeerIdle`，带内核交的那一行（`last_line()`），`by` 是这个会话，命令编号 `<这个会话>/idle/<等的会话>/<起算时刻的 Unix 毫秒>`：同一次订再交一遍编号一样，那边认得出是重的。
5. 那边正在撤销、恢复（回 `restoring`）的，退避着再交同一个命令：100 毫秒起，每次翻倍，一共等到 30 秒，照向上回报。接受了记一行 `idle notice sent`；拒绝了（别的原因、`restoring` 等满了）记 `idle notice refused`，写原因码；交不到（那边停了、核心正在停）也记 `idle notice refused`，写原因。都不再发。
6. 没有会话表的端口的（测试里自己造的会话）发不出去：名单照样清空。

**5. 落盘、推送、回应**

1. 追加的一批在阻塞线程里写进会话日志、同步到磁盘（`store.md`）。写完才往下走。空的一批：什么都不做，也不送「落盘了」。
2. 写完送「落盘了」进内核。内核这才先推送这些事件，再回应事件都落了盘的命令，再跑结束了的回合的挂接点，然后回合往下走（`kernel/session.md`）。所以头见过的事件，崩了以后一定还在；回应到的时候，它产生的事件已经在推送里了。
3. 拒绝的命令没有事件，内核当场回应。
4. 这一批里有 `turn.reverted`、`turn.unreverted` 的：写完，在同一个阻塞线程里只读地读一遍整份日志，重算她看过的（`session/tools.md`）。读不了的记一行 `seen files not rebuilt`，照旧用原来的那一份。
5. 写不进去（磁盘满了、没有权限这类）：记一行 `write failed, stopped`，`kind` 写出错的种类，会话停下（第 9 条）。没落盘的不算发生：没回应过，也没推送过，下次载入照磁盘上的来。不在原地重试：内存里的会话已经往前走了，和磁盘对不上。
6. 写盘的线程 panic 了：记一行 `panicked, stopped`，会话停下。
7. 会话列表的索引（施工 3-8 七补，`store/index.md`「怎么走」第 2 条）：这一批落了盘，在同一个阻塞线程里顺手更新这个会话在索引里的那一行：`session.created` 新起一行；别的，那一行照到的正好是这一批之前的，才照这一批盖上最近一次动静、工作目录、标题、置顶，照到这一批之后。更新失败只记一行 `session index not updated`，照样算落了盘，送「落盘了」：索引是派生的，那一行停在原处，下次列会话照日志补上。没有索引的（`Create::index` 是空的）不更新。写盘的阻塞线程带着会话的 span，这一行也有会话编号。

**6. 推送和订阅**

1. 一份推送所有订阅者共用（tokio 的 broadcast），一个会话最多攒 1024 份还没被读走的（`PUSH_QUEUE`）。
2. 订阅从 actor 收到它的那一刻起推。在发命令之前订阅的，这个命令产生的事件一定先于它的回应到。
3. 补发（施工 3-8 六补）：要订阅时，actor 在收下这一封的同一步里交回三样：推送的那一头、这一刻落了盘的最后一条（内核的 `landed()`，一条都没有是 0）、日志的只读入口（和 `history` 用的是同一个，`session/tools.md`）。`subscribe_after(after)` 拿后两样造一个 `Backlog`：日志里序号大于 `after`、不大于那一条（`upto`）的事件。
   - 接得上、不重不漏：内核落盘以后推送是同一批动作，actor 回完一批动作才收下一封（第 3 条），所以这一步之前落了盘的都推过了，之后落盘的都还没推，只从这个订阅推过去。
   - 读日志不在 actor 里：拿着 `Backlog` 的一方（协议端点）调 `read()`，在阻塞线程里只读地读，一次读完交回（不分批：载入会话本来就整份读进内存，照最简单的做，施工 3-8 六补定）。读的时候会话照常跑，新推的攒在订阅里，最多 1024 份（这一节第 1 款）。`upto` 以前的都落了盘，日志只往后追加，什么时候读都一样；读到的比 `upto` 多的不要。
   - `after` 不比 `upto` 小的，不读日志，交回空的。读不了的（日志坏了、读的时候会话目录被挪走了）交回原因。
4. 读得慢、被挤掉了的：这个订阅掉了队，`Ended::Lagged`，以后一直是掉队，要重新订阅（协议里的 `resync`，`protocol.md`）。
5. 会话停了：读完已经到了的，再读是 `Ended::Stopped`。
6. `try_next` 不等：已经到了的交回，没到的交回空。协议端点收到回应时，先把到了的推送都写出去，再写回应（`protocol.md`）。

**7. 请求模型**

1. 交给端口之前记一行 `request`：`seen`、端点的编号、模型名。这一次的前缀和上一次比变了的，多一格 `changed`，写第一处不同在哪：`tools`、`system`，或者 `message:<第几条，从 0 数起>:<角色>`，角色是 `user`、`assistant`、`tool`。会话的第一次请求（载入以后的第一次也是）、只是往后接着加的，不写。
2. 记下叫停它的那一头和这一刻，交给端口，马上往下走。
3. 说完了：记一行收场，用时从交给端口算起：
   - 出错的：`failed`，`seen`、`took_ms`、`class`。
   - 说完的：`ended`，`seen`、`took_ms`，`in` 是没命中、命中、写进缓存加起来，`hit` 是命中，`write` 是写进缓存（是 0 的不写），`out` 是输出。供应商没报用量的，这四格都不写。
   - 已经叫停过的，不记。
4. 不要这次请求了：叫端口停下，记一行 `cancelled`，`seen`、`took_ms`。已经说完了的，什么都不做。
5. 会话停了也算叫停：actor 退出时放下了叫停的那一头，路上的请求跟着停下，不白花 token。
6. 重试是内核定的：能再来的错，内核推一条等着重试的状态提示、交出「到点叫醒」（`kernel/session.md`）。actor 照状态提示记一行 `retrying`：`seen`、第几次 `attempt`、最多几次 `limit`、等多久 `wait_ms`、分类 `class`；出错的原话不写，里面可能回显请求里的字。到点送回「到点了」，内核再交一次「请求模型」。
7. **辅助请求**（`Aside { purpose, upto, request }`：回顾，施工 3-8 四补，`kernel/session.md`「回顾」；起标题，施工 3-8 五补，「起标题」）：交给同一个端口，名字是用途和它照到的那一条。回报另走一路（`Reports::aside`，送回的是 `AsideSent`、`AsideDelta`、`AsideEnded`，带着用途），和主请求的 `seen` 撞了也分得开：回合进行中的主请求多半就照到那一条。一种用途一次只有一个，它的叫停那一头 actor 拿着不用（内核不叫停辅助请求），actor 退出时放下，请求跟着停。记的几行和主请求的一样，前面带用途：交给端口之前 `recap request`、`title request`（`seen` 是照到的那一条、端点、模型，没有 `changed`：它不和主请求比），说完了 `recap ended`、`recap failed`、`title ended`、`title failed`，格和第 3 条一样。起标题两次都没起成就不再试，第二行 `title failed` 就是那一行。
8. **跟着端口的限额**（施工 8-9，`models.md`「怎么走」第五条第 7 条）：每次请求说完（主请求、辅助请求都算），在送进说完了之前，比端口的 `limits()` 和上一次交给内核的。变了的当场交 `Input::Limits`（不出动作），向内核要一份给头看的限额，连同端口的引用和模型写进和 `Handle` 共用的那一份（`Shown`，施工 8-10，`subscribe` 照它答）。限额里的模型变了、不是 `none` 的（轮换的池总是 `none`，不推），推一条瞬时的 `model.changed`：`by` 是内核，`turn`、`cause` 照内核这时的回合（`turn_cause()`），`ref` 照端口的 `reference()`，`endpoint`、`model` 是新的模型，`limits` 是刚要的那一份，`why` 是 `failover`。只换 key、模型没变的限额不变，不推。

**8. 经路由请求**（`Routes`，`route.rs`、`route/send.rs`，施工 8-6 取代了照环境变量接一个端点的 `HttpModels`，`models.md`「怎么走」第一条、第四条、第五条）

1. 一个核心一份：HTTP 客户端（连接跨请求复用）、供应商的档案、模型资料、空闲超时（`core.md`「模型」）。给每个会话造一个路由，驱动的占位用这个会话快照里的。
2. 造路由时（造会话、载入）记下这个会话用的引用：`ForSession.reference`（施工 8-8：造的是解析好的 `session.created.model`，载入的照内核从日志算的，施工 8-10；没有的照那一刻的 `models.chat`），照它定限额：窗口（手写的压过模型资料）、最大输出、一张图怎么算（照档案）；钉住的池这时就钉上一个成员（载入的照 `ForSession.sent`：最近一条发出去了的 `model.called`），轮换的池取成员里小的（`models.md`「怎么走」第三条第 6、7 条，`route/pool.rs`）。解析不出的限额都没有，`request` 那一行写 `endpoint=none model=none`。钉住的池出错换了成员、成了以后，限额跟着换成它的（施工 8-9，第 7 条第 8 款）；回合开始照这一轮的配置重新解析（施工 8-10，`route/turn.rs`）：内核交了引用的换成它，没交的照路由记在内存里的；解析不出的退回这一轮的 `models.chat`、记一行 `INFO model fallback`，内核交了引用的交回 `replaced`；钉住的池钉着的成员还在的照旧；限额照解析出的重算。端口的 `reference()` 交出钉着的引用，派子代理不写池时照它抄（施工 8-8；8-8 补以前是挡位）。
3. 每一次请求照这一轮冻结的配置（`TurnConfig`，`config.md` 第八条第 3 条）重新解析：钉着的引用解析得出就用它（是池的照钉住、轮换挑成员，这时用不了的跳到下一个，施工 8-8）；解析不出的（没配、那一家没了、用不了）退回这一轮的 `models.chat`，退得回去的以后就钉在它上面（只在内存里）；都不行的当场报说完了，分类 `no_model`，原话照 `models.md`「出错」，没发出去，不报发出去了，记一行 `WARN no model why=…`。`request` 那一行写这个会话上一次解析出来的那一个。
4. key：这一家写了几个，照会话编号钉一个（`miyu_models::keys`：会话编号的 SHA-256 前 8 个字节、大端、对个数取余），取不到值的照写的先后取下一个；一个都取不到也是 `no_model`（`provider "<编号>" has no usable key`）。没写 key 的不带认证头。key 照这一轮的配置取（`{ secret }` 密钥文件、`{ env }` 核心的环境），只在内存里。
   - 出错换 key、换端点（施工 8-9，`models.md`「怎么走」第四条、第五条）：一个候选是一家、一个 key、一个模型，照先后排（出错换过去、成了的 key 在前，池照成员的先后）；取排在最前、没在冷却的，上一次主请求说到一半断了的还发给它，不止一个候选、全在冷却的当场报说完了，分类 `cooling`，没发出去。冷却表核心一份（`ModelData`）。
5. 一次请求派一个任务，带着会话的 span：HTTP 的几行写在会话编号后面（`log.md`）。任务里：
   1. 照驱动列的清单，在阻塞线程里从属主的 blob 取编码要的图片、文件。取不出来的（没有、坏了、读不了）不放进去。
   2. 编码（`drivers/openai-chat.md`，开关照档案）。缺了哪一个 blob：直接报说完了，分类 `other`，原话是 `编码要用的 blob <哈希> 取不出来`。没发出去，不报发出去了；重试也没用。
   3. 经 HTTP 执行器发出去、流式读回来，认证头照驱动（`http.md`）：发出去了，报发出去了（这一家的编号、模型名）；每一段增量，报增量。
   4. 说完、出错：先交给路由记（施工 8-9，`route/ended.rs`：出错的照分类记冷却，还有别的候选的说换了端点；成了的清零、钉住的池钉到它、会话的 key 换成它），再报说完了，带用量，出错的分类和原话，要等多久（供应商说的，换了端点的是别的候选都在冷却时要等多久），换了端点的带 `failover`。
   5. 被叫停：什么都不再报。

**9. 停下**

| 怎么停的 | 怎么走 |
|---|---|
| 有计划地停下（`Handle::stop`） | 先把这个会话在跑的后台命令记成报了、各写一条 `restarted`（`session/tools.md` 第 5 条第 4 款）；收件箱里已经到了的后台命令结束拿出来，别的回报不要了。依次送进「要重启了」、这几条结束（排在后面：内核这时只记下、不开轮，`kernel/session.md`「有计划的重启」）、那几条 `restarted`；都落了盘，这个会话的后台命令整组杀掉，记一行 `stopped`，回一声，actor 退出。再载入时被打断的那一轮接着干（施工 7-3） |
| 删之前停下（`Handle::delete`、`Handle::discard`，施工 3-8 三补） | `delete` 先问内核（`deletable()`）：删不了的交回原因，照常收下一封。删得了的、`discard` 不问的：这个会话的后台命令不再收新的，在跑的整组杀掉、不记回报（不像有计划地停下那样记 `restarted`：会话要删了，没人再看它的日志）；什么都不再写，关上日志的文件，记一行 `stopped for deletion`，回一声，actor 退出。日志关了才回：会话表一收到就挪会话目录（`protocol.md` 的 `session.delete`），Windows 上开着的文件挪不走。在跑的回合不收尾：路上的请求、在跑的工具随 actor 退出叫停、掐掉 |
| 拿着 `Handle` 的都放下了 | 记一行 `closed`，actor 退出 |
| 写不进去、写盘的线程 panic 了 | 第 5 条 |
| actor 自己 panic 了（内核的 bug、端口的 bug） | 看着它的任务记一行 `panicked, stopped`，别的会话照常 |

actor 退出以后：等着回应的命令、要订阅的、要停下的，都收到「会话停了」；订阅读完剩下的是 `Ended::Stopped`；路上的请求被叫停；在跑的工具被掐掉；这个会话还在任务表里的后台命令整组杀掉、不记（再载入时内核补 `aborted`，施工 7-3）；不再算在跑。协议端点照「会话停了」把它从表里拿掉，下次用到再从磁盘载入（`protocol.md`）。

**10. 时钟和会话编号**

1. 时钟：系统时间，到毫秒。系统时间往回拨了，照上一次的：一个会话里的时刻不往回走。1970 年以前的当 0；超过公元 9999 年最后一刻（`253402300799999` 毫秒）的，停在那一刻。命令到的时刻、执行器回报到的时刻，都照它。
2. 会话编号（`new_id`）：UUIDv7，小写的 8-4-4-4-12 写法。前 48 位是那一刻的毫秒；后面跟一个计数器，同一毫秒里造的一个比一个大，系统时间往回拨了照上一次的毫秒，后造的不会排到前面去；其余是系统给的随机数。计数器一个核心进程共用一份：一个数据根只有一个核心，数据根里的会话编号就都照造的先后。

### 运行日志

来源是 `session`，每一行都带会话编号（`log.md`）。阻塞线程里发的也带：在阻塞线程里做完的活（第 5 条的写盘、`session/tools.md` 存效果）都带着派活时的 span（施工 4-9 再补四上：原来 `effect content not stored` 不带）。

| 级别 | 这件事 | 键 | 什么时候 |
|---|---|---|---|
| INFO | `created` | `persona`、`venue`、`tools`（几件） | 造好会话，起 actor 之前 |
| INFO | `loaded` | `events`（几条） | 载入，起 actor 之前 |
| INFO | `request` | `seen`、`endpoint`、`model`、`changed`（变了的才有） | 第 7 条 |
| WARN | `no model` | `why`：`no_model` 的原话 | 路由挑不出端点，当场说完（第 8 条第 3 条，施工 8-6） |
| INFO | `failed` | `seen`、`took_ms`、`class` | 请求出错收场 |
| INFO | `ended` | `seen`、`took_ms`、`in`、`hit`、`write`、`out` | 请求说完 |
| INFO | `cancelled` | `seen`、`took_ms` | 不要这次请求了 |
| INFO | `recap request` | `seen`、`endpoint`、`model` | 回顾的请求交给端口之前（第 7 条第 7 款，施工 3-8 四补） |
| INFO | `recap failed` | `seen`、`took_ms`、`class` | 回顾的请求出错收场 |
| INFO | `recap ended` | `seen`、`took_ms`、`in`、`hit`、`write`、`out` | 回顾的请求说完 |
| INFO | `title request` | `seen`、`endpoint`、`model` | 起标题的请求交给端口之前（第 7 条，施工 3-8 五补） |
| INFO | `title failed` | `seen`、`took_ms`、`class` | 起标题的请求出错、没有正文收场；第二行是不再试的那一行 |
| INFO | `title ended` | `seen`、`took_ms`、`in`、`hit`、`write`、`out` | 起标题的请求说完 |
| WARN | `retrying` | `seen`、`attempt`、`limit`、`wait_ms`、`class` | 等着重试 |
| INFO | `compacted` | `seen`、`trigger`、`before`、`after`、`summary_in`、`summary_cached`、`summary_out`、`took_ms` | 压好了（`compaction.md` 第十三条）：摘要请求的输入、命中、输出、用时照它的 `model.called`，没有的不写 |
| INFO | `running` | `call`、`tool` | 开始跑一次调用（`session/tools.md`） |
| INFO | `ran` | `call`、`took_ms`、`error`（出错的才有，是 `true`） | 一次调用跑完 |
| INFO | `stopped` | `call`、`took_ms` | 叫停一次在跑的调用 |
| ERROR | `crashed` | `call`、`tool`、`took_ms` | 工具 panic 了 |
| WARN | `unavailable` | `call`、`tool` | 目录里没有这件工具 |
| WARN | `effect content not stored` | `error` | 效果里的内容存不成 blob |
| INFO | `subagent started` | `job`、`child` | 派出去一个子代理（施工 7-5，`session/tools.md`「派子代理」） |
| WARN | `subagent not created` | `job`、`error` | 会话表造不成子会话 |
| WARN | `subagent not given its task` | `job`、`child`、`error` | 交代没送进子会话 |
| INFO | `message sent` | `to` | 留言送到了：`to` 是 `parent` 或者任务编号（施工 7-7，`session/tools.md`「父子之间留言」） |
| WARN | `message not delivered` | `to`、`error` | 留言送不到：被拒的写 `refused: <原因码>` |
| INFO | `reported` | `job`、`parent` | 父会话接受了回报（施工 7-6，「向上回报」） |
| WARN | `report refused` | `job`、`parent`、`reason` | 父会话拒绝了回报 |
| WARN | `report not delivered` | `job`、`parent`、`error` | 回报交不到父会话 |
| WARN | `subagent without a job id` | `parent` | 有父会话、造它的命令编号读不出任务编号：回报交不出去 |
| WARN | `subagent not woken` | `child`、`error` | 父会话载入以后叫不起子会话 |
| WARN | `seen files not rebuilt` | `error` | 第 5 条第 4 点 |
| WARN | `session index not updated` | `error` | 落了盘，会话列表的索引更新失败（第 5 条第 7 点，施工 3-8 七补） |
| WARN | `write failed, stopped` | `kind` | 写不进去 |
| WARN | `read back failed, stopped` | `error` | 读回日志读不了（第 4 条，施工 6-9） |
| WARN | `abandoned session not removed` | `error` | 造会话那一条没落盘，收拾会话目录时删不掉（第 1 条第 6 点，施工 4-9 再补四下） |
| WARN | `job output not written` | `error` | 后台命令的输出写不进文件（`session/tools.md` 第 5 条，施工 7-3） |
| WARN | `job output not stored` | `error` | 后台命令的输出读不出来、存不成 blob |
| WARN | `job not waited` | `error` | 等不了后台命令结束 |
| DEBUG | `job output still open after the command ended` | | 后台命令退出了，还有东西拿着它的管道 |
| ERROR | `panicked, stopped` | | actor、写盘的线程 panic 了 |
| ERROR | `answer without a question` | `action`：`answer_tool` | 内核要把回答交给工具 |
| INFO | `stopped` | | 有计划地停好了 |
| INFO | `stopped for deletion` | | 删会话之前停好了（施工 3-8 三补） |
| INFO | `closed` | | 没人拿着了 |
| DEBUG | `input` | `kind` | 每一条输入送进内核之前；增量、执行中的输出记在 TRACE |
| DEBUG | `action` | `kind` | 每一个动作做之前；推送增量、推送执行中的输出记在 TRACE |

- 输入的种类：`command`、`stored`、`environment`、`turn_start_hooks_done`、`request_sent`、`model_delta`、`model_ended`、`woke`、`tool_done`、`tool_progress`、`tool_asks`、`restarting`、`restored`、`read_back`、`recalled`、`tool_guarded`、`job_ended`、`watched`（施工 7-9）、`aside_sent`、`aside_delta`（记在 TRACE）、`aside_ended`（施工 3-8 四补叫 `recap_*`，五补起回顾、起标题共用，改成这个名字）。
- 动作的种类：`append`、`reply`、`push`、`run_turn_start_hooks`、`call_model`、`push_transient`、`cancel_model`、`wake`、`run_turn_end_hooks`、`cancel_tool`、`guard_tool`、`answer_tool`、`run_tool`、`restore`、`read_back`、`recall`、`report`（施工 7-6）、`aside`（施工 3-8 四补叫 `recap`，五补改名）。
- 只写种类、编号、数，不写里面的字。

### 出错

说的话是英文，写进运行日志（施工 4-9 再补四中：原来是中文）；协议端点把它们换成原因码，回给头的话照握手时的语言（`protocol.md`）。

| 类型 | 哪一种 | 说的话 | 协议端点回 |
|---|---|---|---|
| `CreateError` | `Persona` | `persona not readable: <原因>` | 编号不合写法的 `bad_params`，读不了文件的 `unknown_persona` |
| | `Policy` | `policy not built: <原因>` | `internal_error` |
| | `Disk` | `session not created on disk: <原因>` | `internal_error` |
| | `Stopped` | `session.created not stored; the session stopped` | `internal_error` |
| `LoadError` | `Log` | `session log not opened: <原因>` | 没有这个会话的 `session_not_found`，别的 `session_broken` |
| | `NotCreated` | `the session log has no session.created` | `session_broken` |
| | `Blob` | `policy snapshot not fetched: <原因>` | `session_broken` |
| | `Snapshot` | `policy snapshot not understood: <原因>` | `session_broken` |
| | `Policy` | `policy not built from the snapshot: <原因>` | `session_broken` |
| | `Kernel` | `not loaded: <原因>` | `session_broken` |
| `Stopped` | | `the session stopped` | `session_stopped` |

回 `internal_error`、`session_broken` 的，协议端点把说的话记进运行日志：`create failed`、`load failed`（`log.md`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-session/tests/watch.rs`（施工 C-6） | 被等的名单：订进来时已经空着不当场发、等它下一次忙完才发（2026-10-01 改）、起算时刻不晚于上一次忙完的时刻的照样当场发、正忙时订了忙完才发（编号、`by`、带的那一行）、同一个会话只记一个、子代理没报完不发、报完被叫醒的那一轮做完了才发；等的这一边见 `session/tools.md`「订、计时、再订」。真核心见 `crates/miyu-endpoint/tests/watch.rs`（被重启打断的不算空：停的时候不发，再起来做完才发） |
| `crates/miyu-session/tests/delete.rs`（施工 3-8 三补） | 删之前停下：空闲的，后台命令回之前整组杀掉、不记回报，日志一条不多，回了以后连打断都收不到；有回合在进行的说删不了、会话照常、打断以后删得了；`discard` 停下停在请求上的会话，后台命令杀掉、不记，那一轮不收尾 |
| `crates/miyu-endpoint/tests/delete.rs`、`delete_children.rs`（施工 3-8 三补） | 真核心走一遍：目录挪得走；子会话不问忙不忙一起停（`protocol.md`「守着它的」） |
| `crates/miyu-session/tests/jobs.rs`（施工 7-3） | 有计划地停下先记 `restarted`、杀的时候已经落了盘；没人拿着了停下的，整组杀掉不记、再载入补 `aborted`（`session/tools.md`「守着它的」） |
| `crates/miyu-session/tests/actor.rs` | 造会话先存快照、第一条是 `session.created`；一轮先落盘、再推送、再回应，增量在回复落盘之前推过来；能重试的错到点才再请求、原样重发；打断叫停路上的请求；停下再载入接着干；同一个命令两次回两次、只生效一次；停在一轮中间的，落了盘、载入后接着干；载入的会话时刻不往回走；没人拿着了叫停路上的请求；换了工作目录下一轮才看到 |
| `crates/miyu-session/tests/session_fact.rs`（施工 1-13 再补） | 会话编号交给内核：第一轮注入的就是这个会话的编号，排在人那一句前面；停下再载入，编号一样、不重发；子会话注入它自己的编号，不是父会话的 |
| `crates/miyu-session/tests/limits.rs` | 造会话、载入以后先交限额，到线就压，没有窗口的不压（施工 6-3 上）；`Handle` 带着端口交的窗口和内核算的压缩线，载入的也一样，没报窗口的两格都没有（施工 6-3 补） |
| `crates/miyu-session/src/actor/tests.rs` | 写不进去就停下：等着的命令收到「会话停了」、记一行 `WARN`、不再算在跑、订阅不了、日志里没有对话的字 |
| `crates/miyu-session/tests/read_back_log.rs`（施工 6-9） | 撤掉压缩时日志读不回来：会话停下，撤销收到「会话停了」，记一行 `read back failed, stopped` |
| `crates/miyu-session/tests/undo_compaction.rs`（施工 6-9） | 真的会话：撤掉压缩所在的那一轮再恢复，不请求模型，检查点回来、重读的原文照 blob 取回；撤掉以后停了再载入，请求回到压缩前，那次压缩不算了 |
| `crates/miyu-session/tests/report_up.rs`（施工 7-6） | 子会话把回报交给父会话：命令编号照报的那一轮、`by` 是子会话、任务编号照造它的命令读回；父会话先拒两次 `unknown_job` 再收，同一份交了三次；停了再载入同一份再交一次；父会话载入以后叫起还没回报的子会话，交代不再送 |
| `crates/miyu-session/src/handle/tests.rs` | 掉过一次队就一直是掉队；会话停了读完剩下的；`try_next` 只拿已经到了的 |
| `crates/miyu-session/tests/backlog.rs`（施工 3-8 六补） | 补发：补到订阅那一刻落了盘的最后一条，订阅以后、读之前又落了盘的不读进来，从订阅推过来、从下一条起；`after` 是 0、中间、最后一条、比最后一条大的，补的是序号大于它的 |
| `crates/miyu-session/tests/watched.rs`（施工 7-9） | 有没有头看着：一次性会话有头订阅着，后台命令结束叫醒她；走了一个头还有一个照样叫醒；订阅都放下了只记下；造会话以后没人订阅过的当没人看着，后来有头订阅也不因为以前的开轮 |
| `crates/miyu-session/src/clock/tests.rs` | 时钟不往回走、1970 年以前当 0、出了范围停在最后一刻；会话编号是那一刻的 UUIDv7；同一毫秒里连造一千个照先后 |
| `crates/miyu-session/tests/http.rs` | 经路由请求假服务器回复（施工 8-6 起配置指到它）；限速照服务器说的等；打断断开连接；缺 blob 出错、不发；回复断了接着说（开关照档案）；卡住的回复照空闲超时；图片照字节发出去 |
| `crates/miyu-session/tests/route.rs`（施工 8-6） | key 照会话编号挑、重启（停了再载入、换一个路由）还是它；钉着的取不到照写的先后取下一个；没配 `models.chat`、key 一个都取不到、供应商没有、推不出驱动和地址的当场 `no_model`、不发；没写 key 的不带认证头；开着的会话钉着造它时的模型，`models.chat` 改了只影响新会话；钉着的那一家没了，退回这一轮的 `models.chat`、以后钉在它上面；造的时候没配的，配好以后下一轮用上；窗口照配置 |
| `crates/miyu-session/tests/route_failover.rs`（施工 8-9） | 换端点：429 换到别的 key 当场再来、成了以后一直用它；钉住的池换到下一个成员、成了才钉、推 `model.changed`、`Handle` 的限额跟着换；轮换的池跳过冷却中的成员、不推；说到一半断了还发给原来那一个；只有一个候选的照旧在它上面再来；全在冷却的不发、交 `cooling`、原话列出每个候选；换端点数进 5 次；请求本身有错的不换、不记冷却（`models.md`「守着它的」） |
| `crates/miyu-session/tests/failover_log.rs`（施工 8-9） | `endpoint cooling`、`failover` 两行带会话编号，key 只写第几个，值不在日志里 |
| `crates/miyu-session/tests/route_turn.rs`（施工 8-10） | 回合开始重新解析：换了模型的下一轮发给新的、推一条 `model.changed`（`why` 是 `turn`）、`Handle` 的限额和模型跟着换、没再变的不推；钉着的那一家没了退回这一轮的 `models.chat`、内核在那一轮里记下、以后钉在它上面；`models.chat` 也没有的不记、当场 `no_model`；只改了窗口的下一轮用上、也推；换成轮换的池推的没有端点、限额取小的；载入照换过的引用造路由 |
| `crates/miyu-session/tests/fallback_log.rs`（施工 8-10） | `model fallback` 一行带会话编号，写原来的和退回的 |
| `crates/miyu-session/tests/route_pools.rs`（施工 8-8） | 池：钉住的一个会话一直发给一个成员、新会话照指针分开、认不出的成员跳过；载入照日志认回钉着的、指针写进 `pools.json` 重启读回；轮换的一次一个；这时用不了的跳过、钉到下一个；池没了退回 `chat`；限额照钉着的、轮换的取小的；`session.created` 记下会话的引用（`models.md`「守着它的」） |
| `crates/miyu-session/tests/log.rs` | 会话造、请求、出错、重试、收场、停下、载入、没人拿着、端口 panic 的几行；手动压缩的 `compacted` 写 `trigger=manual`（施工 6-8）；撤销以后 `changed=message:0:user`；`DEBUG` 的输入和动作、增量在 `TRACE`；没有对话的字 |
| `crates/miyu-session/tests/recap_log.rs`（施工 3-8 四补） | 回顾的请求记 `recap request`、`recap ended`、`recap failed`，`seen` 是照到的那一条，格和主请求的一样；没有对话的字 |
| `crates/miyu-session/tests/title_log.rs`（施工 3-8 五补） | 起标题的请求记 `title request`、`title ended`、`title failed`，`seen` 是照到的那一条；两次都没起成，第三轮不再试；没有对话的字 |
| `crates/miyu-session/tests/http_log.rs` | HTTP 的两行带会话编号，key 不在日志里 |
| `crates/miyu-session/tests/index_log.rs`（施工 3-8 七补） | 每落一批，索引里那一行照到日志的末尾；表没了，更新失败只记一行带会话编号的 `session index not updated`，会话照常说完下一轮（`store/index.md`「守着它的」） |

### 出处

- `02-内核.md` 第四节「执行器怎么回动作」（每个动作怎么回、送回的输入带执行器的时钟）、第七节「会话 actor 怎么跑」。
- `07-存储.md` 第四节：先落盘后推送（S4）、写不进去就停下；第七节：会话按需载入。
- `04-核心协议.md` 第六节第 2 条（先见结果，后见回应）、第七节（慢了掉队、resync）。
- `05-内核接口.md` 第七节：驱动的规格、HTTP 执行器、编码要的 blob 取不出来。
- `models.md`「怎么走」第一条（供应商）、第四条（挑端点，8-6 那一半）。
- `03-事件模型.md` 第二节：会话编号是 UUIDv7、时刻的写法；第五节：瞬时事件不落盘。
- `28-运行日志.md` 第二节、第三节：会话的那几行、`request` 的 `changed`。

### 还没有的

- 会话空闲一段时间以后 actor 自己退出（`07-存储.md` 第七节）：现在只有没人拿着、停下、写不进去、panic 这几种退出。
- 回合开始、回合结束的挂接点真有模块：各模块照先后跑、等它们回来或者超时（`02-内核.md` 第四节，`05-内核接口.md` 第五节）。
- 执行前的链里除了权限策略的别的守卫、守卫超时按拒绝算（`05-内核接口.md` 第五节第 1、3 条）。
- 工具执行中问人、把回答交给工具（`02-内核.md` 第四节、第六节「提问怎么走」）：现在没有工具会问，这个动作只记一行 `ERROR`。
- 资源调度器：端点的并发上限、限速、优先级夹在请求模型的中间（`02-内核.md` 第七节）。
- 推送的队列紧张时，先合并同一条目的连续增量（`04-核心协议.md` 第七节）：现在攒满了就让读得慢的掉队。
- 子会话、后台命令，撤销时一起停下（`02-内核.md` 第七节，7-8）。
