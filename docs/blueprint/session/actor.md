## 会话 actor

### 是什么

把内核的会话状态机接上磁盘和外面的世界：一个会话一个异步任务。人的命令、执行器的回报进它的收件箱，一条条送进内核；内核交出的动作，它照表一个个做：写盘、回应、推送、请求模型、执行工具、改回文件。执行工具、效果、她看过的、改回文件另见 `session/tools.md`，执行前的权限策略见 `session/guard.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-session/src/open.rs` | 造会话、载入：备好磁盘上的，交给内核，起 actor |
| `crates/miyu-session/src/actor.rs` | actor 本身：收件箱、一批批送进内核、每个动作怎么回、写盘、停下 |
| `crates/miyu-session/src/actor/model.rs` | 请求模型：交给端口、叫停、说完了记一行 |
| `crates/miyu-session/src/handle.rs` | `Handle`：发命令、订阅、停下；推送和订阅 |
| `crates/miyu-session/src/port.rs` | 请求模型的端口：`Models`、`ModelPort`、`Reports`、`Cancel` |
| `crates/miyu-session/src/http.rs` | 端口的真实现：经驱动和 HTTP 执行器请求 |
| `crates/miyu-session/src/clock.rs` | 会话的时钟、新的会话编号 |
| `crates/miyu-session/src/store.rs` | 写盘的端口：平时是会话日志，测试里换成写不进去的 |
| `crates/miyu-session/src/kinds.rs`、`lines.rs` | 运行日志里的输入、动作种类名，和几种写法 |
| `crates/miyu-session/src/blocking.rs` | 在阻塞线程里做完磁盘上的事 |
| `crates/miyu-session/src/tools.rs`、`effects.rs`、`restore.rs` | 执行工具、效果、改回文件（`session/tools.md`） |
| `crates/miyu-session/src/guard.rs` | 权限策略（`session/guard.md`） |
| `crates/miyu-session/src/testkit.rs` | 测试用的、照剧本回的端口，`testkit` 开关打开才有 |

### 对外的样子

| 名字 | 做什么 |
|---|---|
| `create(Create)` | 造一个会话，`session.created` 落了盘才交回 `Handle` |
| `load(Load)` | 从磁盘载入一个会话，交回 `Handle` |
| `new_id(时刻)` | 一个新的会话编号 |
| `Handle` | 一个会话的收件箱，可以复制，几个头一起拿着 |
| `Pushed`、`Subscription`、`Ended`、`Stopped` | 推送、订阅、订阅断了、会话停了 |
| `Models`、`ForSession`、`ModelPort`、`Reports`、`Cancel` | 请求模型的端口 |
| `HttpModels`、`IDLE` | 端口的真实现；空闲超时 180 秒 |

`Create` 的格：数据根 `root`、资源目录 `resources`、会话编号 `id`、人格 `persona`、场所 `venue`、属主 `owner`、开始时的权限 `permission`、有没有人能确认 `attended`、一次性的 `oneshot`、环境 `environment`（时区、工作目录）、造会话的命令编号 `command`、谁发的 `by`、造端口的 `models`、工具目录 `tools`、系统的家目录 `home`（读不出来的是空的）、沙盒的助手 `sandbox`（这台机器上的沙盒能用才有，施工 5-4 上）、沙盒的缓存 `sandbox_cache`（`<缓存目录>/sandbox/<属主>`，核心算不出缓存目录的没有，施工 5-4 下）。`Load` 的格：`root`、`owner`、`id`、`environment`、`models`、`tools`、`home`、`sandbox`、`sandbox_cache`。

| `Handle` 的方法 | 做什么 |
|---|---|
| `id()` | 会话编号 |
| `busy()` | 有没有在跑的回合：核心看它决定能不能空闲退出（`core.md`）。停了的会话不算 |
| `command(编号, 谁, 命令)` | 发一个命令，等回应：接受的，它产生的事件落了盘才回；拒绝的当场回。编号由发的一方生成，同一个编号只生效一次（`kernel/session.md`） |
| `subscribe()` | 订阅：从这一刻起的推送 |
| `stop()` | 有计划地停下，停好了才回 |
| `environment(环境)` | 环境变了：工作目录、时区 |

`Pushed` 有两种：`Events`，落了盘的几条事件，照先后；`Transient`，一条瞬时事件，不落盘。`Subscription` 有 `next()`（等下一份）、`try_next()`（不等，没到的是空的）；断了的是 `Ended::Lagged`（掉了队）或 `Ended::Stopped`（会话停了）。

端口：`Models::port(ForSession)` 给一个会话造端口，`ForSession` 带这个会话的驱动占位（取自策略快照）和属主的 blob。`ModelPort::model()` 交回端点的编号和模型名；`ModelPort::call(seen, 请求, Reports, Cancel)` 马上返回，在别的任务里发。`Reports` 有 `sent(模型, 请求字节的哈希)`、`delta(增量)`、`ended(用量, 出错, 要等多久)`；`Cancel::wait()` 等到被叫停。

### 怎么走

**1. 造会话**（`create`）

1. 在阻塞线程里依次做，哪一步不成就交回那一种错，actor 不起；已经存下的快照留着：
   1. 读出这个人格要用的原文（`store/resources.md`）。
   2. 拼策略快照：人格、有没有人能确认、工具目录里每件工具的名字、说明、参数格式、访问类别，照名字排（`policy.md`）。
   3. 照快照造内核的策略、驱动的占位、替工具写的两句（`session/tools.md`）、权限策略拒绝时的三句（`session/guard.md`）。
   4. 快照存成属主的 blob：先落 blob，再写引用它的事件。
   5. 建会话目录和空的第一段（`store.md`）。
2. 造请求模型的端口。时钟从现在起。
3. `session.created` 写属主、场所、快照的哈希、开始时的权限，`oneshot` 照交进来的；交给内核造会话，`cause` 是造会话的命令。
   马上交给内核这个模型的限额（`Input::Limits`，端口的 `limits()`：窗口、最大输出、一张图怎么算，施工 6-3 上），在别的输入之前；什么动作都不出。
4. 造权限策略、执行工具的端口（她看过的是空的）、actor；记下造会话的命令在等回应。
5. 在会话的 span 里记一行 `created`，起 actor。
6. 等回应：`session.created` 落了盘，内核回应这个命令，交回 `Handle`。actor 在那之前停了的，交回 `CreateError::Stopped`（「出错」一节），在阻塞线程里删掉这个会话的目录：只剩一段空的第一段时才删，别的不动（施工 4-9 再补四下：原来留在磁盘上）。已经存下的快照留着：按内容存，别的会话可能也在用，回收随 blob 回收那一步。

**2. 载入**（`load`）

1. 在阻塞线程里依次做，哪一步不成就交回那一种错：
   1. 打开会话日志：自检，截掉最后一段末尾那半行（`store.md`）。
   2. 第一条要是 `session.created`；日志是空的、第一条不是它的，报错。
   3. 照它记的哈希从属主的 blob 里取快照，读懂，造策略、驱动的占位、两句、三句。
2. 造请求模型的端口。
3. 时钟从日志里最后一条的时刻起：系统时间比它还早（往回拨过），照它。
4. 从日志里的效果重建她看过的（`session/tools.md`）。
5. 交给内核载入：交回会话，和一串要回的动作。有计划的重启打断了的一轮接着干，崩了的那一轮标成没走完（`kernel/session.md`）。
   马上交给内核这个模型的限额，同上：接着干的那一轮，发主请求之前就知道限额（施工 6-3 上）。最近一个检查点重读过文件的，接着交 `Input::Recalled`：照它的 `restored` 从 blob 读出原文，读不出来的不交（施工 6-5）。
6. 造权限策略、执行工具的端口、actor；记一行 `loaded`；起 actor，先回那一串动作。
7. 马上交回 `Handle`，不等那一串动作做完。

**3. 收件箱**

1. 一个会话一个 tokio 任务，带着会话的 span：`error_span!`，目标 `miyu::session`，名字 `session`，一格 `session` 是会话编号。开在 `ERROR` 级，调到 `WARN` 也筛不掉，底下的行都带着会话编号（`log.md`）。外面再套一个看着它的任务。
2. 两条通道，都不设上限：
   - 人的：`Handle` 发来的命令、订阅、停下、环境变了。拿着 `Handle` 的都放下了，它就关了。
   - 执行器的回报：请求的回报、到点了、工具的回报。actor 自己也拿着一头，它不会自己关。
3. 两条都有的时候，先收执行器的回报：读流不断。
4. 人的一封：

   | 来的 | 怎么办 |
   |---|---|
   | 命令 | 记下等它回应的那一头，照 actor 的时钟记下到的时刻，送进内核 |
   | 订阅 | 当场交回一个订阅，不进内核 |
   | 环境变了 | 送进内核：不当场注入，到下一个边界再查（`kernel/session.md`） |
   | 停下 | 第 9 条 |

5. 执行器的一封，照 actor 的时钟记下到的时刻：

   | 来的 | 送进内核的 |
   |---|---|
   | 请求发出去了 | 发给了哪个模型、请求字节的哈希 |
   | 一段增量 | 增量 |
   | 请求说完了 | 先记一行收场（第 7 条），再送用量、出错（分类和原话）、供应商说要等多久 |
   | 到点了 | 为哪一次请求等的 |
   | 工具的回报 | 见 `session/tools.md`；已经叫停了的不理 |

6. 一封送进内核，内核交回一串动作，照第 4 条一个个做。当场就能回的输入（落盘了、挂接点跑完了、链判完了、改回了、工具不在目录里的结果）不回收件箱，排进本地的队列，接着送，队列空了才收下一封：它们先于收件箱里的任何一封。
7. 每送完一批，照内核说的空不空闲，写一次「有没有在跑的回合」。

**4. 每个动作怎么做**

| 动作 | 做什么 | 当场送回 |
|---|---|---|
| 追加事件 | 在阻塞线程里写一批、同步（第 5 条） | 落盘了，到这一批最后一条为止 |
| 回应命令 | 交给等这个编号的最早那一头；它不等了，丢掉；没人在等的，不理 | |
| 推送事件 | 推给订阅了的；没有订阅的，丢掉 | |
| 推送瞬时事件 | 同上；是 `status`（现在只有等着重试这一种）的，先记一行 `retrying`；是 `compaction.done` 的，先记一行 `compacted`（施工 6-3 下） | |
| 跑回合开始的挂接点 | 现在没有模块挂它 | 挂接点跑完了，没有注入 |
| 请求模型 | 交给端口（第 7 条） | |
| 到点叫醒 | 起一个定时的任务，到那一刻送回「到点了」；那一刻已经过了的，马上送 | |
| 不要这次请求了 | 叫端口停下（第 7 条） | |
| 跑回合结束的挂接点 | 现在没有模块挂它，什么都不做 | |
| 过执行前的链 | 权限策略在阻塞线程里判，等它判完（`session/guard.md`） | 链判完了 |
| 执行工具 | 交给执行工具的端口（`session/tools.md`） | 目录里没有这件工具的：一条出错的结果 |
| 停下工具 | 掐掉跑它的任务（`session/tools.md`） | |
| 改回文件 | 在阻塞线程里一步步做完，这期间不收收件箱（`session/tools.md`） | 改回了，一步一项结局 |
| 压完重读（`Reread`，施工 6-5） | 在阻塞线程里一个一个读：照安全打开（`fs.md`），超过上限的不读完，不是普通文件、读不了、不是 UTF-8 的算读不到；读到的存进这个会话的 blob。这期间不收收件箱 | 一个一项：读到了（`blob`、原文）、太大、读不到（`compaction.md` 第九条） |
| 把回答交给工具 | 现在没有工具会问：记一行 `ERROR` | |

**5. 落盘、推送、回应**

1. 追加的一批在阻塞线程里写进会话日志、同步到磁盘（`store.md`）。写完才往下走。空的一批：什么都不做，也不送「落盘了」。
2. 写完送「落盘了」进内核。内核这才先推送这些事件，再回应事件都落了盘的命令，再跑结束了的回合的挂接点，然后回合往下走（`kernel/session.md`）。所以头见过的事件，崩了以后一定还在；回应到的时候，它产生的事件已经在推送里了。
3. 拒绝的命令没有事件，内核当场回应。
4. 这一批里有 `turn.reverted`、`turn.unreverted` 的：写完，在同一个阻塞线程里只读地读一遍整份日志，重算她看过的（`session/tools.md`）。读不了的记一行 `seen files not rebuilt`，照旧用原来的那一份。
5. 写不进去（磁盘满了、没有权限这类）：记一行 `write failed, stopped`，`kind` 写出错的种类，会话停下（第 9 条）。没落盘的不算发生：没回应过，也没推送过，下次载入照磁盘上的来。不在原地重试：内存里的会话已经往前走了，和磁盘对不上。
6. 写盘的线程 panic 了：记一行 `panicked, stopped`，会话停下。

**6. 推送和订阅**

1. 一份推送所有订阅者共用（tokio 的 broadcast），一个会话最多攒 1024 份还没被读走的（`PUSH_QUEUE`）。
2. 订阅从 actor 收到它的那一刻起，之前的不补。在发命令之前订阅的，这个命令产生的事件一定先于它的回应到。
3. 读得慢、被挤掉了的：这个订阅掉了队，`Ended::Lagged`，以后一直是掉队，要重新订阅（协议里的 `resync`，`protocol.md`）。
4. 会话停了：读完已经到了的，再读是 `Ended::Stopped`。
5. `try_next` 不等：已经到了的交回，没到的交回空。协议端点收到回应时，先把到了的推送都写出去，再写回应（`protocol.md`）。

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

**8. 经驱动和 HTTP 请求**（`HttpModels`）

1. 一个核心一份：HTTP 客户端（连接跨请求复用）、端点的编号、地址、key 和另配的头、各家供应商不一样的几处、这一次调用要定的（模型名、输出的上限、模型能收哪些输入）、空闲超时。给每个会话造一个端口，驱动的占位用这个会话快照里的。
2. 一次请求派一个任务，带着会话的 span：HTTP 的几行写在会话编号后面（`log.md`）。
3. 任务里：
   1. 照驱动列的清单，在阻塞线程里从属主的 blob 取编码要的图片、文件。取不出来的（没有、坏了、读不了）不放进去。
   2. 编码（`drivers/openai-chat.md`）。缺了哪一个 blob：直接报说完了，分类 `other`，原话是 `编码要用的 blob <哈希> 取不出来`。没发出去，不报发出去了；重试也没用。
   3. 经 HTTP 执行器发出去、流式读回来（`http.md`）：发出去了，报发出去了；每一段增量，报增量。
   4. 说完、出错：报说完了，带用量，出错的分类和原话，供应商说的要等多久。
   5. 被叫停：什么都不再报。
4. 核心起来时没有 key 的，用的是另一个端口：每次都当场报认证失败（`core.md`）。

**9. 停下**

| 怎么停的 | 怎么走 |
|---|---|
| 有计划地停下（`Handle::stop`） | 送进「要重启了」；它产生的事件落了盘，记一行 `stopped`，回一声，actor 退出。再载入时被打断的那一轮接着干 |
| 拿着 `Handle` 的都放下了 | 记一行 `closed`，actor 退出 |
| 写不进去、写盘的线程 panic 了 | 第 5 条 |
| actor 自己 panic 了（内核的 bug、端口的 bug） | 看着它的任务记一行 `panicked, stopped`，别的会话照常 |

actor 退出以后：等着回应的命令、要订阅的、要停下的，都收到「会话停了」；订阅读完剩下的是 `Ended::Stopped`；路上的请求被叫停；在跑的工具被掐掉；不再算在跑。协议端点照「会话停了」把它从表里拿掉，下次用到再从磁盘载入（`protocol.md`）。

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
| INFO | `failed` | `seen`、`took_ms`、`class` | 请求出错收场 |
| INFO | `ended` | `seen`、`took_ms`、`in`、`hit`、`write`、`out` | 请求说完 |
| INFO | `cancelled` | `seen`、`took_ms` | 不要这次请求了 |
| WARN | `retrying` | `seen`、`attempt`、`limit`、`wait_ms`、`class` | 等着重试 |
| INFO | `compacted` | `seen`、`trigger`、`before`、`after`、`summary_in`、`summary_cached`、`summary_out`、`took_ms` | 压好了（`compaction.md` 第十三条）：摘要请求的输入、命中、输出、用时照它的 `model.called`，没有的不写 |
| INFO | `running` | `call`、`tool` | 开始跑一次调用（`session/tools.md`） |
| INFO | `ran` | `call`、`took_ms`、`error`（出错的才有，是 `true`） | 一次调用跑完 |
| INFO | `stopped` | `call`、`took_ms` | 叫停一次在跑的调用 |
| ERROR | `crashed` | `call`、`tool`、`took_ms` | 工具 panic 了 |
| WARN | `unavailable` | `call`、`tool` | 目录里没有这件工具 |
| WARN | `effect content not stored` | `error` | 效果里的内容存不成 blob |
| WARN | `seen files not rebuilt` | `error` | 第 5 条第 4 点 |
| WARN | `write failed, stopped` | `kind` | 写不进去 |
| WARN | `abandoned session not removed` | `error` | 造会话那一条没落盘，收拾会话目录时删不掉（第 1 条第 6 点，施工 4-9 再补四下） |
| ERROR | `panicked, stopped` | | actor、写盘的线程 panic 了 |
| ERROR | `answer without a question` | `action`：`answer_tool` | 内核要把回答交给工具 |
| INFO | `stopped` | | 有计划地停好了 |
| INFO | `closed` | | 没人拿着了 |
| DEBUG | `input` | `kind` | 每一条输入送进内核之前；增量、执行中的输出记在 TRACE |
| DEBUG | `action` | `kind` | 每一个动作做之前；推送增量、推送执行中的输出记在 TRACE |

- 输入的种类：`command`、`stored`、`environment`、`turn_start_hooks_done`、`request_sent`、`model_delta`、`model_ended`、`woke`、`tool_done`、`tool_progress`、`tool_asks`、`restarting`、`restored`、`tool_guarded`。
- 动作的种类：`append`、`reply`、`push`、`run_turn_start_hooks`、`call_model`、`push_transient`、`cancel_model`、`wake`、`run_turn_end_hooks`、`cancel_tool`、`guard_tool`、`answer_tool`、`run_tool`、`restore`。
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
| `crates/miyu-session/tests/actor.rs` | 造会话先存快照、第一条是 `session.created`；一轮先落盘、再推送、再回应，增量在回复落盘之前推过来；能重试的错到点才再请求、原样重发；打断叫停路上的请求；停下再载入接着干；同一个命令两次回两次、只生效一次；停在一轮中间的，落了盘、载入后接着干；载入的会话时刻不往回走；没人拿着了叫停路上的请求；换了工作目录下一轮才看到 |
| `crates/miyu-session/src/actor/tests.rs` | 写不进去就停下：等着的命令收到「会话停了」、记一行 `WARN`、不再算在跑、订阅不了、日志里没有对话的字 |
| `crates/miyu-session/src/handle/tests.rs` | 掉过一次队就一直是掉队；会话停了读完剩下的；`try_next` 只拿已经到了的 |
| `crates/miyu-session/src/clock/tests.rs` | 时钟不往回走、1970 年以前当 0、出了范围停在最后一刻；会话编号是那一刻的 UUIDv7；同一毫秒里连造一千个照先后 |
| `crates/miyu-session/tests/http.rs` | 经假服务器回复；限速照服务器说的等；打断断开连接；缺 blob 出错、不发；回复断了接着说；卡住的回复照空闲超时；图片照字节发出去 |
| `crates/miyu-session/tests/log.rs` | 会话造、请求、出错、重试、收场、停下、载入、没人拿着、端口 panic 的几行；撤销以后 `changed=message:0:user`；`DEBUG` 的输入和动作、增量在 `TRACE`；没有对话的字 |
| `crates/miyu-session/tests/http_log.rs` | HTTP 的两行带会话编号，key 不在日志里 |

### 出处

- `02-内核.md` 第四节「执行器怎么回动作」（每个动作怎么回、送回的输入带执行器的时钟）、第七节「会话 actor 怎么跑」。
- `07-存储.md` 第四节：先落盘后推送（S4）、写不进去就停下；第七节：会话按需载入。
- `04-核心协议.md` 第六节第 2 条（先见结果，后见回应）、第七节（慢了掉队、resync）。
- `05-内核接口.md` 第七节：驱动的规格、HTTP 执行器、编码要的 blob 取不出来。
- `03-事件模型.md` 第二节：会话编号是 UUIDv7、时刻的写法；第五节：瞬时事件不落盘。
- `28-运行日志.md` 第二节、第三节：会话的那几行、`request` 的 `changed`。

### 还没有的

- 会话空闲一段时间以后 actor 自己退出（`07-存储.md` 第七节）：现在只有没人拿着、停下、写不进去、panic 这几种退出。
- 回合开始、回合结束的挂接点真有模块：各模块照先后跑、等它们回来或者超时（`02-内核.md` 第四节，`05-内核接口.md` 第五节）。
- 执行前的链里除了权限策略的别的守卫、守卫超时按拒绝算（`05-内核接口.md` 第五节第 1、3 条）。
- 工具执行中问人、把回答交给工具（`02-内核.md` 第四节、第六节「提问怎么走」）：现在没有工具会问，这个动作只记一行 `ERROR`。
- 资源调度器：端点的并发上限、限速、优先级夹在请求模型的中间（`02-内核.md` 第七节）。
- 推送的队列紧张时，先合并同一条目的连续增量（`04-核心协议.md` 第七节）：现在攒满了就让读得慢的掉队。
- 子会话、后台命令，撤销时一起停下（`02-内核.md` 第七节）。
