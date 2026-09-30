## 统一的请求和组装

### 是什么

每次请求模型之前，内核把有效历史组装成一份统一的请求：工具面、system、消息。它和供应商无关，驱动再把它编码成各家的格式（`drivers/openai-chat.md`）。同样的有效历史加同样的策略，出来的字节一样。

这一页还写和它一起用的几样零件：环境和状态的事实块、任务的两种回报（施工 7-2）、模板和转义、把模型的增量拼成内容块的累积器、和上一次请求比出的第一处不同。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/request.rs` | 统一的请求、规范的字节、哈希、指纹、第一处不同 |
| `crates/miyu-kernel/src/assemble.rs` | 组装的接口 `Assembler` |
| `crates/miyu-assemble/src/lib.rs` | 默认的组装器：稳定区、`stable`、接着写的记号 |
| `crates/miyu-assemble/src/render.rs` | 有效历史渲染成消息；人这一边的块合成一条 user |
| `crates/miyu-assemble/src/texts.rs` | 检查点的包装、回合没走完的五句、回报的写法 |
| `crates/miyu-assemble/src/jobs.rs` | 两种回报渲染成带标签的事实（施工 7-2）；子代理发来的留言包一层标签（施工 7-7） |
| `crates/miyu-kernel/src/facts.rs` | 三份事实模板、会话的环境、该不该注入 |
| `crates/miyu-kernel/src/session/turn.rs`、`permission.rs`、`retry.rs`、`tools.rs`、`call.rs` | 什么时候注入事实、什么时候组装、算第一处不同、记进 `model.called` |
| `crates/miyu-kernel/src/template.rs` | 模板的写法、换字段、转义 |
| `crates/miyu-kernel/src/accumulate.rs` | 增量、累积器 |
| `crates/miyu-kernel/src/time.rs` | 环境块里钟点和时区的写法 |
| `resources/core/` | 给模型看的字：事实的模板、检查点的包装、回合没走完的五句、回报的写法（`jobs/`，施工 7-2） |

### 对外的样子

**`Request`**，字段照这个先后写进字节：

| 字段 | 取值 | 是什么 |
|---|---|---|
| `tools` | `ToolSpec` 的列表 | 工具面 |
| `system` | 字符串 | 系统提示词，一整段 |
| `messages` | `Message` 的列表 | 示范对话、检查点、历史，照先后 |
| `stable` | 整数 | 稳定区有几条消息，就是示范对话的条数 |
| `continuation` | 布尔 | 接着写的记号（「组装」第 7 条）。是假的不写进字节 |

- 端点、模型、输出的上限不在请求里，发请求时才定（`drivers/openai-chat.md` 的 `Call`）。
- `ToolSpec`：`name`、`description`、`parameters`。`parameters` 是参数的 JSON Schema，原样的 JSON，空格、字段的先后都留着。
- `Message`，JSON 里第一格是 `role`：

| `role` | 其余的格 |
|---|---|
| `user` | `blocks`：内容块 |
| `assistant` | `blocks`：内容块，思考连同私有数据原样带着 |
| `tool` | `call_id`：哪一次调用；`error`：算不算出错；`blocks`：内容块 |

内容块的写法见 `kernel/blocks.md`。

**规范的字节**：`canonical_bytes()` 写成紧凑的 JSON，字段照结构体的先后，参数格式原样照抄。`hash()` 是这串字节的 SHA-256，写成 `sha256:` 加 64 位小写十六进制。

**指纹** `fingerprint()`：三样各算一个 SHA-256。工具面：`tools` 数组的 JSON；system：它写成的 JSON 字符串，带引号；每条消息：那一条的 JSON，另记它的角色。`stable`、`continuation` 不算进指纹。上一次的请求本身不留。

**第一处不同** `first_difference(上一次的指纹)`：交回 `Tools`、`System`、`Message { index, role }`（`index` 从 0 数起），或者没有。

**组装的接口** `Assembler`：同步的纯函数。一个会话一个（`Policy.assembler`），冻结的东西在造它的时候交进来。

| 方法 | 做什么 |
|---|---|
| `assemble(&History) -> Request` | 从有效历史组装请求 |
| `summarize(&History, upto, cut, instructions) -> Request` | 压缩的摘要请求：有效历史截到第 `upto` 条照平常组装，最后接摘要指令（施工 6-2 上，`compaction.md` 第三条第 3 条）。`cut` 是截短重试截到第几条（施工 6-6 中）：检查点后面第 `cut` 条及以前的不要，留下的第一条是助手的，前面补一条 user（`truncated.txt`）；没有是不截。`instructions` 是手动压缩时人附的要求，`None` 是没附（施工 6-8） |
| `summarize_isolated(&History, upto, cut, instructions) -> Request` | 隔离式的摘要请求（施工 6-6 下，`compaction.md` 第四条）：和 `summarize` 一样的消息，system 换成 `summarize-system.txt`，工具面空的；手动压缩附的要求照样夹在指令里（施工 6-8） |
| `summary(&[Block]) -> Option<String>` | 从摘要请求的回复里取出摘要；取不出来的是 `None`（`compaction.md` 第三条第 6 条）。指令和取法是一对，都归组装 |

**默认的组装器** `DefaultAssembler::new(Stable, Texts)`：

| 类型 | 格 | 是什么 |
|---|---|---|
| `Stable` | `tools` | 工具面 |
| | `system` | 拼好的 system（`policy.md`），组装时不再拆开 |
| | `demos` | 示范对话。照策略快照造的总是空的 |
| `Texts` | `checkpoint_open`、`checkpoint_close`、`checkpoint_end`、`restored_open`、`restored_close` | 检查点包装的开头、摘要的收尾、包装的结尾（施工 6-5 拆开），重读的文件那一块的头尾 |
| | `turn_ended` | `TurnEndedTexts`：`interrupted`、`error`、`step_limit`、`aborted`、`restarted` 五句 |
| | `summarize_task`、`summarize_instructions`、`summarize_end` | 摘要指令的正文、要求前面那一行、最后那一句（`core/compaction/summarize-task.txt` 施工 6-2 上；另两份 `summarize-instructions.txt`、`summarize-end.txt` 施工 6-8 拆出来。以前造的快照里没有这两份，是空的：那时的正文里本来就带着最后那一句） |
| | `jobs` | `JobTexts`：两种回报的写法，`core/jobs/` 下的十一份（施工 7-2，下面「回报」）；人停的那一句（施工 7-2 补，下面「回报」第 3 条，以前造的快照里没有，是空的）；子代理的留言的标签两份（施工 7-7，下面「子代理的留言」，以前造的快照里没有，是空的）。以前造的快照里没有 `jobs` 的，是没有：回报不渲染，那些会话也派不出任务 |

默认的 `summarize`：截到第 `upto` 条照平常组装；最后一条是 user 的，指令并进这一条做最后一块，不是的另起一条 user；`continuation` 是假。指令是一个文本块：`summarize_task`；有要求的接 `summarize_instructions` 和要求（原样，不转义，末尾没有换行的补一个）；最后是 `summarize_end`（施工 6-8，`compaction.md` 第七条第 3 条）。默认的 `summary`：只看正文块，有 `<summary>` 的取到 `</summary>` 或者末尾，没有的去掉 `<analysis>…</analysis>`，前后空白去掉，空的是 `None`（`crates/miyu-assemble/src/summary.rs`）。

**事实**：

- `FactTemplates::new(env, permission, reply_cut)`：三份模板的原文。
- `env(此刻, &Environment)`、`permission(&Permission)`、`reply_cut()`：各交回一块 `ContextInjected { kind, text }`，`kind` 是 `env`、`permission`、`reply_cut`。
- `Environment { offset, cwd }`：时区（`UtcOffset`，按分钟，−14:00 到 +14:00，东边是正的）；工作目录，头报上来的写法，例如 `~/src/miyu`，内核不改写。造会话、载入时交进来，执行器报「环境变了」就整个换掉。
- `changed(有效历史, by, 几块)`：这几块里该注入的，照原来的先后。

**模板** `Template`：`parse(原文)`、`render(字段)`、`fill(字段, 清理)`、`fields()`；另有 `escape(值)`。字段用 `BTreeMap<&str, &str>` 交进来。

**累积器** `Accumulator`：`apply(增量)`、`finish(回复的序号)`、`cut_off(回复的序号)`。增量 `Delta` 四种，都带 `index`（第几块）：

| 增量 | 另外带着 |
|---|---|
| `Start` | `kind`：`Text`、`Reasoning`、`ToolCall { name }` |
| `Text` | `text`：正文、思考、或者工具调用参数原文的一段 |
| `Private` | `private`：驱动私有数据 |
| `End` | 没有 |

### 怎么走

**组装**

1. 请求 = 工具面 + system + 示范对话 + 渲染出来的消息。工具面在造组装器时照名字的字节序排好，稳定排序；同名的两件，造策略时就拒了（`policy.md`）。`stable` 是示范对话的条数，现在总是 0。
2. 有检查点的（最近一次压缩），它是人这一边的第一块：`checkpoint_open`、摘要原文、`checkpoint_close`、代码写的几段（`notes`）、重读的文件（每个是 `restored_open`、原文、`restored_close`）、`checkpoint_end` 拼成一个文本块（施工 6-5）。摘要、重读的原文不转义：一个是模型写的多行正文，一个是文件本来的样子。重读的原文照 blob 从 `History` 取，取不到的那一份整块不写。清空的检查点（`trigger` 是 `clear`）什么都不出：她看到的上下文从这里起是空的，下一轮开头的环境、权限两块事实照常注入（施工 6-8 补，`compaction.md` 第十四条）。
3. 然后照有效历史排好的先后一条条渲染：以回复为界切段，每段先是那条回复，再是它的工具结果（按调用的先后），再是别的（照日志的先后）。细节见 `kernel/history.md`。

| 事件 | 渲染成 |
|---|---|
| `message.user` | 它的内容块，攒进人这一边。人附的图片、文件是文字后面的图片块、文件块（施工 3-9 三补，`protocol.md` 的 `session.send`），原样进请求，图片块、文件块都带着文件名（图片的施工 3-9 四补）；它们在线上是什么样（data URL、照字放进来的文本文件、占位，名字写在哪），是驱动的事（`drivers/openai-chat.md` 第 9 条）。`by` 是这个会话派的子代理的，包一层标签（下面「子代理的留言」，施工 7-7） |
| `context.injected` | 一个文本块，就是它的原文，攒进人这一边 |
| `turn.started` | 不出块。记下这个回合开始的地方、触发它的那一条；没有触发的（手动压缩、清空单开的那一轮，施工 6-8、6-8 补）不记 |
| `turn.ended` | 原因是 `interrupted`、`error`、`step_limit`、`aborted`、`restarted` 的，出一个文本块，就是那一句，攒进人这一边；`completed` 和不认识的原因不出；没有触发的那一轮的不出：她没看到过那一轮，写了她会当成是上一轮没走完（施工 6-8，`compaction.md` 第七条第 8 条） |
| `message.assistant` | 一条 assistant，内容块原样 |
| `tool.result` | 一条 tool：`call_id`；状态不是 `ok` 的（包括不认识的状态），`error` 是真；内容块 |
| `job.reported`、`child.reported` | 一个文本块，带标签的事实（下面「回报」），攒进人这一边；派它的那一轮撤掉了的、没派过的、快照里没有写法的，不出（施工 7-2） |
| `session.*`、`tool.approval_*`、`question.*`、`model.called`、`files.restored`、不认识的种类 | 不渲染 |

4. 内容块里不认识的种类，不进请求。`context.compacted`、`turn.reverted`、`turn.unreverted`、`message.withdrawn` 已经由有效历史用掉了，渲染时碰不到。
5. **人这一边合成一条 user**：碰到 assistant 或者 tool，攒着的块先合成一条 user，放在它前面；渲染完了，剩下的也合成一条；什么都没攒，不出消息。
6. 合的时候照攒进来的先后，只有一处例外：**每个回合开始的地方**，放这个回合开始时注入的事实和触发它的那一条，先事实、后触发（当前要回应的那句话离生成位置最近）。
   - 回合开始的地方：`turn.started` 那一刻已经攒了几块，就在那几块后面。
   - 开始时注入的事实：`turn.started` 以后、这个回合第一条回复或者 `turn.ended` 以前，带着这个回合编号的 `context.injected`，内核的、模块的都算。
   - 触发的那一条要在这一次合的块里，才挪过去；不在的（例如压缩掉了、没有认识的块、是模块的事件），事实照原来的先后。
   - 触发的不一定是人的消息：重启以后接着干的那一轮，那时没有排着队的消息的，由 `turn.ended` 触发，挪过去的是「被重启打断」那一句；有排着的，由排着的最后一条触发。
   - 早到的触发也挪：回合中途就来、下一轮才轮到的那一句，还有打断了这一轮的那一句，日志里都排在上一轮结束的那一句前面；挪到回合开始的地方，它才排在最后。
   - 挪的是回合开始的那个位置，日志里它不动：发过的请求里排好的先后，以后不变。
   - 「开始时注入的」到这一轮有了回复、结束，或者第一次记下 `model.called` 为止（施工 4-9 再补三上）：第一次请求什么都没收到就出了可以重试的错、等的时候又切了级别的，到点查出的事实照先后排在触发后面，下一次请求接着上一次往后长。
   - 排在检查点前面的 `model.called` 不算（施工 6-2 上）：那是被替代掉的那段的请求和摘要请求自己，压完的第一次请求前缀本来就从头来。回合开头压的，压完再注入的事实照样和触发的那句放在一起，这一轮第一次主请求的最后一块照旧是触发它的那句。
   - 回合中途注入的事实（第一条回复以后）照先后，排在那一步的工具结果后面。
7. **接着写的记号**：有效历史照排好的先后倒着看，跳过 `model.called`：最后一条是内核记的 `reply_cut` 事实，再往前一条是带 `interrupted` 的回复，`continuation` 就是真。这时最后一条 user 只有被打断的那一句，前面那条 assistant 是半截。那一句后面又来了别的（人的消息、别的事实），就是假。驱动怎么用它见 `drivers/openai-chat.md`。

**回报**（施工 7-2，`agents.md` 第九条）

1. 回报不带回合编号，照它在日志里的位置排：闲着时到的，就是开这一轮的那一条，照触发挪到回合开始的地方，排在开始时注入的事实后面（「组装」第 6 条）；回合中途到的，照请求看到的范围排在那一步的工具结果后面（`kernel/history.md`「照请求看到的范围排」）；只记下的，排在下一轮触发的那句前面。
2. 标签那一行带任务编号 `job`、标题 `title`（派它时给的，照有效历史记着的派出去过的任务取，派它的那一条压缩掉了也在）、结束的原因 `reason`（不认识的原样），三个字段照模板的规矩转义。里面一行一句，最后一行是收尾的标签；每一份以一个换行结尾。
3. 后台命令结束（`command-*.txt`）：人停的（`reason` 是 `stopped`、不带 `by_model`：人用 `job.stop` 停的）标签那一行后面先写一句 `stopped-by-user.txt`，和子代理的回报共用（施工 7-2 补）：人停的叫醒她，不写她会当成任务自己停了（2026-09-30 网页演示接真核心实测撞见）。她自己用 `jobs` 停的（`by_model`）不写：那种不叫醒她，停它的那次调用本来就在上下文里。以前造的快照里没有这一份的，不写。接着，有退出码的写退出码（带符号，照原样），被信号杀掉的写信号，有用时的写用时（毫秒），存下了输出的写它有多少字、用 `jobs` 的 `output` 看；不带输出本身（`agents.md` 第九条，照 Claude Code、dsh）。载入时补的 `aborted` 什么都没有，只有标签。
4. 子代理的回报（`subagent-*.txt`）：人停的（`stopped`、不带 `by_model`：人用 `job.stop` 停的、删掉这个子会话的）标签那一行后面先写 `stopped-by-user.txt` 那一句，规矩同第 3 条；人插过话的（`person`）注明一句，正文截过的（`truncated`）注明一句，接着是正文 `text`，原样放、不转义（和检查点里的摘要一样，是模型写的多行正文），末尾没有换行的补一个；一个字都没说的，正文换成它没说话就结束了那一句。
5. 派它的那一轮撤掉了的不渲染：派它的调用已经不在上下文里了（`agents.md` 第七条第 2 条）；这种回报内核也不叫醒她（`kernel/session.md`「回报」第 5 条）。恢复了撤销，照常渲染。

**子代理的留言**（施工 7-7，`agents.md` 第九条第 5 条）

1. `message.user` 的 `by` 是这个会话派的子代理的子会话（有效历史记着的派出去过的任务里有它，`history.md`「派出去过的任务」）：渲染成一块带标签的事实。标签那一行带任务编号 `job`、标题 `title`（派它时给的，照转义的规矩），接着是它的话，原样、不转义（和回报的正文一样，是模型写的多行正文），末尾没有换行的补一个，最后是收尾的标签。字以外的块接在这一块后面（`message_agent` 只送一块字）。
2. 它不带回合编号，照它在日志里的位置排，和回报一样：闲着时是开这一轮的那条，挪到回合开始的地方、排在事实后面；回合中途到的排在那一步的工具结果后面。
3. 派它的那一轮撤掉了的，一块都不出，和它的回报一样：派它的调用已经不在上下文里了。
4. 别人发来的原样：人、父会话发给子会话的交代和留言（子会话的场所说明已经说了交代来自父会话）。
5. 以前造的快照里没有标签的两份（施工 7-7 以前），标签是空的：只剩它的话。

**事实**

1. 三类，都是 `context.injected`，`by` 是内核，`cause` 是这一轮的：

| `kind` | 什么时候查 | 字段 |
|---|---|---|
| `env` | 回合开始；这一轮切过级别以后的边界 | `time`、`timezone`、`cwd` |
| `permission` | 同上 | `level` |
| `reply_cut` | 说到一半断了、要带着半截再请求 | 没有 |

2. **该不该注入**（`changed`）：`env`、`permission` 各和有效历史里同一个 `by`、同一个 `kind` 的最近一块比，原文逐字节相同就不注入。
   - 比最近那一块：先是 A，一个边界变成 B，下一个边界又回到 A，要注入 A。
   - 压缩替掉的、撤销掉的不在有效历史里，不算：压缩以后、撤销了带着它们的那一轮以后，下一个边界两块都重新注入。
   - 模块注入的同类块不算。
   - `reply_cut` 不比，每次都注入。
3. **回合开始**：照第 2 条查过，变了的和 `turn.started` 同一批追加，环境在前、权限在后。时刻取开这一轮那一刻（送进来的那条输入的时刻；载入以后接着干的，是载入的时刻）；时区、工作目录取会话现在的；权限取人最近一次切成的。放宽的级别这时生效。
4. **这一轮里切过级别**（切成了和原来不一样的）：到下面的边界，两块再查一遍，照第 2 条：
   - 回合开始的挂接点跑完了：排在模块注入的块后面；
   - 一步齐了、要请求下一次：走到步数上限的，不查，回合结束；
   - 等着重试，到点了；
   - 切的那一刻，回合正要请求（挂接点跑完、在等事件落盘）：当场查。

   查的时候，时刻取这个边界上那条输入的时刻，时区取会话现在的，工作目录取这一轮开始时的（派工具带的也是它）。查过就清掉「切过」，放宽的这时生效。执行器中途报的新工作目录，下一轮开头才写进去。
5. **回复被打断**：出了可以重试的错、收到的半截已经写成回复，紧跟着追加 `reply_cut`，再等着重试（`kernel/session.md`）。
6. **写成什么**：
   - `time`：此刻在那个时区落在哪一个小时，写成这个小时的起止：`Fri 2026-09-25 16:00–17:00`。星期三个字母（`Sun` 到 `Sat`），年-月-日，二十四小时制，分钟都写 `00`，中间是连接号 `–`；23 点写 `23:00–24:00`（施工 1-13 补，`kernel/ids.md`「当地钟点」）。
   - `timezone`：`UTC+09:00`、`UTC-05:30`，零时区写 `UTC+00:00`。
   - `cwd`：`Environment.cwd` 原样。
   - `level`：只读开着写 `read_only`；关着写常用的那一级，`workspace` 或 `full`；不认识的级别写 `read_only`。
   - 字段都照模板的规矩转义（下面）。模板文件行尾的换行也算，所以每块以换行结尾。
7. **造的时候就查**：`FactTemplates::new` 读三份模板，拿全部字段（值是空的）试换一次：`env` 给 `time`、`timezone`、`cwd`，`permission` 给 `level`，`reply_cut` 什么都不给。写法坏了、要了没给的字段，当场报错。模板可以只用其中几个字段。

**模板**

1. 原文照抄，`{名字}` 换成那个字段；`{{`、`}}` 写出 `{`、`}`。只有字段替换，没有条件、循环。
2. 名字：小写字母开头，只用小写字母、数字、`_`。
3. `render`：每个字段都先转义，可信的、不可信的一样。照 JSON 字符串的写法，再多转几个：

| 字 | 写成 |
|---|---|
| `\` | `\\` |
| 换行、回车、制表 | `\n`、`\r`、`\t` |
| 别的控制字符（U+0000 到 U+001F、U+007F 到 U+009F，包括退格、换页） | `\u` 加四位小写十六进制，例如 `\u001b` |
| `"`、`&`、`<`、`>` | `\u0022`、`\u0026`、`\u003c`、`\u003e` |
| U+2028、U+2029 | `\u2028`、`\u2029` |
| 别的，包括中文、表情 | 原样 |

   转出来只有一行，没有引号、尖括号、`&`：伪造不了标签、属性和一行一条的记录。前后加上引号，就是一段合法的 JSON 字符串，读回来和原文一样。
4. `fill(字段, 清理)`：每个字段过交进来的清理，不转义。给人看的字用它，不进请求。
5. `fields()`：模板要的字段名，照出现的先后，重复的只算一次；`{{` 不算。
6. 用 `render` 的：三份事实、内核替工具写的几句（`kernel/tools.md`）、驱动的占位（`drivers/openai-chat.md`）、权限策略和执行器替工具写的几句（`policy.md`）、自带软件输出里的几句和 `shell` 的说明（`tools/`）。

**累积器**

1. 一次响应一个。块照开始的先后编号，从 0 数起，一块接一块地开始；几块的字可以交错着来。
2. `apply`：`Start` 的编号必须是下一块；`Text`、`Private`、`End` 要的块必须开始了、还没收全。`Text` 接在那一块后面；`Private` 只给思考和工具调用，一块最多一份；`End` 标成收全了。对不上的报错（见「出错」），累积器不动。
3. 收尾，拼成内容块，照开始的先后：

| 块 | `finish`（正常说完） | `cut_off`（被打断） |
|---|---|---|
| 正文 | 有字的留；空的不要 | 同左，没收全的也留 |
| 思考 | 有字或者有私有数据的留；两样都没有的不要 | 同左，没收全的也留 |
| 工具调用 | 都留，参数是收到的原文（可以是空的） | 只留收全了的 |

4. 留下的工具调用照先后编号：`call_<回复的序号>_<第几个>`，从 1 数起，丢掉的不占号。回复的序号就是这条回复写进日志时的序号。
5. 会话这样用它（`kernel/session.md`）：正常说完照 `finish` 收；被打断、出了错照 `cut_off` 收，出了错的再去掉全部工具调用；正常说完却一块都没有的，算出错 `empty_reply`；增量对不上的，这次请求按 `bad_stream` 出错，原话就是那句报错。会话收尾用的是只在内核里用的 `numbered(回复的序号, 是不是正常说完)`：拼出来的和 `finish`、`cut_off` 一样，每一块多带着它在流里是第几块，会话照它对上每一块的起止（施工 2-3 补）。

**第一处不同**

1. 先比工具面，再比 system，再一条条比消息（角色和哈希）。第一处对不上的就是它，角色取这一次那一条的。
2. 前面都对得上、这一次更长或一样长的：没有不同，缓存照样命中。
3. 上一次有、这一次少了的：从少了的那一条算，角色取上一次那一条的。
4. 会话每次组装完算一次指纹，和这个会话上一次请求的比，再把这一次的留下。比出来的交给执行器（`CallModel.changed`，运行日志写成 `tools`、`system`、`message:<第几条>:<角色>`），也记进这次的 `model.called` 的 `first_difference`：`part` 是 `tools`、`system`、`message`，消息的另带 `index`、`role`（`kernel/events-bodies.md`）。指纹只在内存里：会话的第一次请求、载入以后的第一次请求，没有可比的，不写。

### 样子：给模型看的字

原文都在 `resources/core/` 下，行尾的换行也算；每份登记在 `26-提示词.md` 第十节的登记簿里，token 数在那里。造会话时读进策略快照（`policy.md`），以后照快照发。

| 文件 | 原文 | 进到哪 |
|---|---|---|
| `facts/env.txt` | `<env time="{time}" timezone="{timezone}" cwd="{cwd}"/>` | 事实 `env` |
| `facts/permission.txt` | `<permission level="{level}"/>` | 事实 `permission` |
| `facts/reply-cut.txt` | `<reply-cut>The reply above was cut off before it was finished. The user has already seen it. Continue from exactly where it stopped, without repeating it.</reply-cut>` | 事实 `reply_cut` |
| `turn-ended/interrupted.txt` | `<turn-ended reason="interrupted">The user interrupted this turn.</turn-ended>` | 人这一边 |
| `turn-ended/error.txt` | `<turn-ended reason="error">This turn stopped on an error.</turn-ended>` | 人这一边 |
| `turn-ended/step_limit.txt` | `<turn-ended reason="step_limit">This turn stopped at the step limit.</turn-ended>` | 人这一边 |
| `turn-ended/aborted.txt` | `<turn-ended reason="aborted">Miyu stopped unexpectedly and this turn did not finish.</turn-ended>` | 人这一边 |
| `turn-ended/restarted.txt` | `<turn-ended reason="restarted">A planned restart of Miyu stopped this turn.</turn-ended>` | 人这一边 |

两种回报的写法（`core/jobs/`，施工 7-2，「回报」），标签里的三个字段是 `job`、`title`、`reason`：

| 文件 | 原文 | 什么时候 |
|---|---|---|
| `jobs/command-open.txt` | `<command-ended job="{job}" title="{title}" reason="{reason}">` | 后台命令结束，开头 |
| `jobs/command-exit.txt` | `Exit code {code}.` | 有退出码 |
| `jobs/command-signal.txt` | `Killed by signal {signal}.` | 被信号杀掉 |
| `jobs/command-duration.txt` | `Ran for {ms} ms.` | 有用时 |
| `jobs/command-output.txt` | `The output has {chars} characters. Read it with jobs output.` | 存下了输出 |
| `jobs/command-close.txt` | `</command-ended>` | 收尾 |
| `jobs/subagent-open.txt` | `<subagent-report job="{job}" title="{title}" reason="{reason}">` | 子代理的回报，开头 |
| `jobs/subagent-person.txt` | `The user also talked to this subagent during the task.` | 人插过话 |
| `jobs/subagent-truncated.txt` | `The middle of this report was cut. Read all of it with jobs output.` | 正文截过 |
| `jobs/subagent-silent.txt` | `The subagent ended without saying anything.` | 一个字都没说，代替正文 |
| `jobs/subagent-close.txt` | `</subagent-report>` | 收尾 |
| `jobs/stopped-by-user.txt` | `The user stopped this.` | 人停的（`stopped`、不带 `by_model`），两种回报共用，紧跟标签那一行（施工 7-2 补） |

每种原因（停掉的分她停的、人停的，施工 7-2 补）、截过的、人插过话的、没说话的各一份样本，在 `docs/designs/samples/reports/`，由出厂的字渲染出来逐字节比（`crates/miyu-assemble/src/jobs/tests.rs`）。

子代理发来的留言的标签（`core/jobs/`，施工 7-7，「子代理的留言」），标签里的两个字段是 `job`、`title`：

| 文件 | 原文 | 什么时候 |
|---|---|---|
| `jobs/subagent-message-open.txt` | `<subagent-message job="{job}" title="{title}">` | 子代理发来的留言，开头 |
| `jobs/subagent-message-close.txt` | `</subagent-message>` | 收尾 |

样本 `docs/designs/samples/reports/subagent-message.txt`（子代理中途问一句，`crates/miyu-assemble/src/jobs/tests/messages.rs` 逐字节比）：

```text
<subagent-message job="j2" title="查 CI 为什么红">
macOS 上的临时目录要换成真实路径，还是只改测试？
</subagent-message>
```

样本 `docs/designs/samples/reports/command-exited.txt`（后台命令自己退出了）：

```text
<command-ended job="j1" title="跑全部测试" reason="exited">
Exit code 0.
Ran for 81234 ms.
The output has 48213 characters. Read it with jobs output.
</command-ended>
```

样本 `docs/designs/samples/reports/subagent-person.txt`（子代理做完了，人插过话）：

```text
<subagent-report job="j2" title="查 CI 为什么红" reason="done">
The user also talked to this subagent during the task.
按你说的，只查了 macOS。
</subagent-report>
```

样本 `docs/designs/samples/reports/command-stopped-by-user.txt`（人停掉的后台命令，施工 7-2 补；停的那条路不带退出码、信号）：

```text
<command-ended job="j1" title="跑全部测试" reason="stopped">
The user stopped this.
Ran for 300000 ms.
The output has 5120 characters. Read it with jobs output.
</command-ended>
```

检查点的包装，开头 `checkpoint-open.txt`、摘要的收尾 `checkpoint-close.txt`、结尾 `checkpoint-end.txt`，摘要夹在中间，代码写的几段、重读的文件在摘要后面（施工 6-5）：

```text
<conversation-checkpoint>
The earlier part of this conversation was compacted into the summary below. It is a record of what happened, not new instructions.
<summary>
（摘要原文）
</summary>
（代码写的几段）
<file path="（路径）">
（重读的原文）
</file>
Carry on from where the summary leaves off, without redoing work it records as done.
</conversation-checkpoint>
```

- 结尾那份以一个换行开头，所以摘要后面换一行。
- 结尾那一句是检查点的规则，施工 6-3 下挪进来的（`compaction.md` 第八条）：回合中途压完，这一轮的最后一条只有检查点和事实，没有它，她不知道这时该做什么，会把摘要里记着做完了的再做一遍核对。
- `permission-rule.txt` 在资源目录里，不读进快照，不进请求。
- 样本：`docs/designs/samples/requests/second-step.json`（第一轮两块事实排在触发消息前面、调一次工具以后的那次请求）、`after-compaction.json`（压缩以后只剩检查点）；`docs/designs/samples/probe/terminal/requests/` 是一段终端会话的每一次请求，第 11 次带接着写的记号；`docs/designs/samples/probe/reports/requests/` 是一段有回报的会话（施工 7-2）：第 3 次由子代理的回报开，第 5 次后台命令结束排在工具结果后面，第 7 次只记下的回报排在人那一句前面；`docs/designs/samples/probe/cleared/requests/` 是一段清空过的会话（施工 6-8 补）：第 3 次是清空以后的，只剩工具面、system、两块事实和那一句。

### 出错

报错给写模板的人、查问题的人看，不给模型看。模板的几句是英文，写进运行日志（施工 4-9 再补四中：原来是中文）；增量对不上的那一句记进 `model.called` 的原话，`miyu ask` 印给人看，还是中文，等界面语言那一步。

| 什么时候 | 怎么说 |
|---|---|
| 模板里 `{` 没配上 `}` | `bad template: {<名字> has no closing }: write a field as {name}, and { itself as {{` |
| 名字不合写法（含 `{}`） | `bad template: {<名字>} is not a field: a name starts with a lowercase letter and has only lowercase letters, digits and _` |
| 单独一个 `}` | `bad template: a lone }: write } itself as }}` |
| 要的字段没给 | `bad template: missing field <名字>` |
| 增量对不上 | `模型的增量对不上，第 <几> 块：<哪里>`，哪里是：`这一块已经开始过了`、`跳过了编号，块要一块接一块地开始`、`这一块还没开始`、`这一块已经收全了`、`正文块没有私有数据`、`私有数据来了两次` |

- 事实模板坏了，造策略时报，会话造不成、载入不了（`policy.md`）。
- 组装、事实、累积器收尾不会出错：请求里没有写不成 JSON 的东西，模板造的时候试换过，调用编号从 1 数起。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/request/tests.rs` | 同样的请求字节、哈希一样；参数格式一个字节不改；消息以角色开头；第一处不同的四种情形 |
| `crates/miyu-kernel/tests/request_sample.rs` | 样本 `second-step.json` 就是规范的字节；哈希是它的 SHA-256 |
| `crates/miyu-assemble/src/tests.rs` | 工具面照名字排；示范对话在前、算进 `stable`；接着写的记号什么时候真、什么时候假 |
| `crates/miyu-assemble/src/render/tests.rs` | 每种事件渲染成什么；回合开始的事实和触发放到回合开始的地方；等重试时切了级别，事实排在触发后面；早到的触发；重启以后接着干；检查点在最前、摘要不转义；回合没走完的五句；没有触发的那一轮出错、打断、崩了、重启都不出那一句（施工 6-8）；清空的检查点不出字，压缩过再清空的摘要也跟着没了（`render/tests/clear.rs`，施工 6-8 补）；不认识的块和不进上下文的种类 |
| `crates/miyu-assemble/tests/sample_session.rs` | 样本会话组装出两份样本请求；撤回的、确认和提问的事件不进请求；样本里的两种回报渲染成带标签的事实（施工 7-2） |
| `crates/miyu-assemble/tests/probe.rs` | 一段八轮的终端会话由真内核跑出来，每次请求和存档（`requests/`、`openai-chat/`）逐字节一样；五条性质；什么都没收到的再来一字不差。有回报的会话（施工 7-2）一样和存档比、查五条性质；回报开的那一轮最后一块是那条回报，回合中途到的单独一条 user 排在工具结果后面，只记下的在人那一句前面。清空过的会话（施工 6-8 补）一样和存档比、查五条性质；清空以后的那一次算改写过，只剩工具面、system 和一条 user：两块事实、那一句 |
| `crates/miyu-assemble/src/jobs/tests.rs` | 两种回报（施工 7-2）：出厂的字渲染出来和样本逐字节一样（每种原因、截过的、人插过话的、没说话的；停掉的分她停的、人停的，施工 7-2 补）；人停的那一句紧跟标签那一行；旧快照没有那一句的，人停的照原来的写；负的退出码照原样、没存下输出的不写字数；标题照规矩转义；开这一轮的那条挪到回合开始的地方、事实在前；回合中途到的排在那一步的工具结果后面；派它的那一轮撤掉了的不渲染；派它的那一条压缩掉了照样有标题；旧快照没有写法的不渲染 |
| `crates/miyu-assemble/src/jobs/tests/messages.rs`（施工 7-7） | 子代理的留言：出厂的字渲染出来和样本一字不差、末尾有换行的不再补；人、别的会话发来的原样；开这一轮的挪到回合开始的地方、事实在前；回合中途到的排在那一步的工具结果后面；派它的那一轮撤掉了的不渲染；旧快照没有标签的只剩它的话 |
| `crates/miyu-assemble/src/summary/tests.rs` | 摘要指令怎么拼：没附要求的和原来的整份一字不差；附了的夹在中间、原样、补换行；旧快照没有那两份的（施工 6-8）；取摘要的每一种 |
| `crates/miyu-assemble/tests/random_logs.rs` | 五百份随机会话（有手动压缩单开的那一轮，施工 6-8），每次请求查五条性质：同样的日志同样的字节、前缀延伸（统一的请求和线上的字节两层；中间撤销、恢复、压缩过的那一次不查）、调用和结果成对、没有连着的 user、回合第一次请求的最后一块是触发；CI 长跑两万份；重试的回合里，一半在等着重试时切一下只读 |
| `crates/miyu-kernel/src/facts/tests.rs` | 模板造的时候查；两块的写法、目录转义、实际生效的级别；该不该注入的八种情形 |
| `crates/miyu-kernel/tests/sample_facts.rs` | 用出厂模板，样本会话每个边界该注入的几块 |
| `crates/miyu-kernel/src/session/tests/turn.rs`、`permission.rs`、`reply.rs`、`scenario/retrying.rs` | 回合开始注入、切级别以后在哪个边界注入、第二轮只注入变了的、断了以后追加 `reply_cut` |
| `crates/miyu-kernel/src/session/tests/difference.rs` | 第一处不同交给执行器、记进 `model.called` |
| `crates/miyu-kernel/src/time/tests.rs` | 钟点到小时、星期、时区的写法和范围 |
| `crates/miyu-kernel/src/template/tests.rs` | 换字段、双写的大括号、每种要转的字、转出来是一行合法的 JSON 字符串、伪造属性和记录和标签都失效、`fields()`、`fill`、坏模板、少了字段 |
| `crates/miyu-kernel/src/accumulate/tests.rs` | 三种块拼对、调用编号、空块、交错、被打断、对不上的增量、随机切片拼出来一样；每一块带着它在流里是第几块（施工 2-3 补） |

### 出处

- `08-上下文投影.md` 第二节「统一的请求怎么写」：字段、规范的字节、指纹。
- `08-上下文投影.md` 第四节「默认的组装怎么写」：稳定区、检查点、渲染、人这一边合成一条（C2）。
- `08-上下文投影.md` 第五节「模板与转义怎么写」「环境和状态的事实怎么写」，C7、C10。
- `08-上下文投影.md` 第七节：第一处不同、测试门禁、接着写的那次登记在案的改写。
- `03-事件模型.md` 第五节「增量和累积器怎么写」；第六节「照每次请求看到的范围排」。
- `05-内核接口.md` 第五节：组装请求是独占的挂接点，模板只做字段替换。
- `26-提示词.md` 第四节（system 的排法）、第八节（东西放在哪）、第十节（登记簿）。

### 还没有的

- 示范对话：快照里还没有这一格，`stable` 总是 0（`26-提示词.md` 第四节，`16-人格与预设.md`）。
- 缓存标记：`stable` 算好了，还没有驱动用它（`08-上下文投影.md` 第六节）。
- 压缩：检查点里由代码补上的部分、压后重建、压缩以后算一个边界（M6，`09-压缩.md` 第四节）。
- 群里的发送者标签和群聊近况、模块用模板声明的事件（`08-上下文投影.md` 第四节第 6 条）。别的 harness 发来的话注明来处（`agents.md` 第九条第 4 条，7-10）。
- 角色扮演提示，排在触发之后（C2 的例外，`26-提示词.md` J10）。
- 事实：到分钟的时间、没有工作目录的场所、场所的强制策略、工作区的文件清单、关掉的补一条「已关」（`08-上下文投影.md` 第五节）。
- 按段记哈希、前缀改写的登记簿（`08-上下文投影.md` 第七节）。
- 中途连上的头要的「到目前为止的内容」（`03-事件模型.md` 第五节，M8）。
