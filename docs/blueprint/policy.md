## 策略快照

### 是什么

一个会话发请求要用的全部字和几样开关：人格、拼好的 system、工具面、随核心附带的字、步数上限、有没有人能确认、重启以后接着干几次。造会话时拼一份，写成规范的字节，按内容哈希存成 blob，`session.created` 记着它的哈希。以后载入，照哈希取回来重建策略：核心升级改了出厂的字，老会话发出的请求照样和当时逐字节一样。

模型、供应商、密钥不在里面：同一份快照可以交给不同的端点，发请求时才定。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-policy/src/lib.rs` | 对外的几样 |
| `crates/miyu-policy/src/compose.rs` | 拼快照：system 怎么拼、重启以后接着干几次 |
| `crates/miyu-policy/src/snapshot.rs` | 快照的类型、字节、哈希、读回来、造会话的那一条、造策略、驱动的占位 |
| `crates/miyu-policy/src/tools.rs` | 工具面：排序、拆成两份；执行器替工具写的两句 |
| `crates/miyu-policy/src/guard.rs` | 权限策略拒绝时写的三句 |
| `crates/miyu-store/src/resources.rs` | 从资源目录读原文（`store/resources.md`） |
| `crates/miyu-store/src/blob.rs` | 存 blob、取 blob（`store.md`） |
| `crates/miyu-session/src/open.rs` | 造会话时存、载入时取 |

### 对外的样子

**`Snapshot`**，字段的先后就是字节里的先后：

| 字段 | 取值 | 是什么 |
|---|---|---|
| `persona` | 字符串 | 人格的编号，就是资源目录 `personas/` 下那一层目录的名字 |
| `system` | 字符串 | 拼好的 system |
| `tools` | `ToolEntry` 的列表 | 工具面，照名字排好。一件都没有的不写这一格 |
| `core` | `CoreTexts` | 随核心附带的字 |
| `step_limit` | 整数或 `null` | 一个回合最多请求几次模型；`null` 是不限，现在总是 `null` |
| `attended` | 布尔 | 有没有人能确认 |
| `resumes` | 整数 | 有计划的重启打断了一轮，再起来时连着接着干几次，现在是 3 |
| `compaction` | 对象 | 压缩用的数：`reserve_cap` 输出预留的上限、`margin` 余量、`image`、`file` 估算时一张图、一个文件各算多少 token、`tail` 尾巴的上限，现在是 20000、13000、2000、2000、16000（施工 6-2）。以前造的快照里没有，读成没有；没有的不写。6-2（上）造的没有 `tail`，读成 16000。`rebuild` 压后重建的数（施工 6-5）。`pause` 熔断的数：`failures` 连续失败几次、`turns` 几个回合内又到线算快、`refills` 连着快几次，现在都是 3（施工 6-6 上）；以前造的没有，读成没有：不熔断。`shorten` 截短重试的数：`tries` 最多再试几次、`percent` 没说超多少时截百分之几，现在是 3、20（施工 6-6 中）；以前造的没有，或者只有数、没有字的，不截短 |

**`ToolEntry`**：`name`、`description`、`parameters`（参数的 JSON Schema，原样的 JSON）、`access`（`read`、`write`、`execute`、`network`、`outbound`，不认识的原样留着），照这个先后。

**`CoreTexts`**，每一格是 `resources/core/` 下一份文件的原文：

| 格 | 里面的格 | 文件 |
|---|---|---|
| `checkpoint_open`、`checkpoint_close`、`checkpoint_end` | | `checkpoint-open.txt`、`checkpoint-close.txt`、`checkpoint-end.txt`（施工 6-5 从 close 里拆出来。以前造的快照里没有 end，读成空的：close 里原本就带着那一句，拼出来一字不差） |
| `turn_ended` | `interrupted`、`error`、`step_limit`、`aborted`、`restarted` | `turn-ended/<同名>.txt` |
| `facts` | `env`、`permission`、`reply_cut` | `facts/env.txt`、`facts/permission.txt`、`facts/reply-cut.txt` |
| `tool_results` | `unknown`、`not_an_object`、`cancelled_before`、`cancelled_running`、`skipped`、`read_only`、`denied`、`denied_with_reason`、`unattended`、`question_interrupted`、`question_voided`、`question_unattended`、`restarted`、`unavailable`、`crashed` | `tool-results/` 下，下划线换成 `-` 的同名文件 |
| `drivers` | `image_omitted`、`file_omitted`、`no_output`、`tool_attachments`、`tool_attachments_only` | `drivers/` 下，下划线换成 `-` 的同名文件 |
| `permissions` | `forbidden`、`unresolvable` | `permissions/forbidden.txt`、`permissions/unresolvable.txt` |
| `compaction` | `summarize_task`、`summarize_instructions`、`summarize_end`、`notes_files`、`notes_files_more`、`notes_retrieve`、`notes_too_large`、`restored_open`、`restored_close`、`truncated`、`notes_uncovered`、`summarize_system` | `compaction/` 下，下划线换成 `-` 的同名文件（摘要指令施工 6-2 上，截短重试的两份施工 6-6 中，隔离式那一句施工 6-6 下，别的施工 6-5；`summarize_instructions`、`summarize_end` 施工 6-8 从摘要指令里拆出来）。以前造的快照里没有，读成没有；没有的不写：没有 `notes_*` 的不写那一段，没有 `restored_*` 的不重读，没有截短重试的两份的不截短，没有 `summarize_system` 的不改走隔离式；有 `summarize_task`、没有 `summarize_instructions`、`summarize_end` 的，那两份读成空的：那时的 `summarize_task` 里本来就带着最后那一句，拼出来一字不差 |

**函数**：

| 名字 | 做什么 |
|---|---|
| `compose(人格, Sources, attended)` | 拼一份快照。`Sources` 是读好的原文：`core`（`CoreTexts`）、`persona`（`PersonaTexts { persona }`，人设的原文） |
| `Snapshot::with_tools(工具)` | 带上工具面 |
| `to_bytes()`、`hash()`、`from_bytes(字节)` | 规范的字节、内容哈希、读回来 |
| `session_created(属主, 场所, 权限)` | 造会话那一条的 `body`：`owner`、`venue`、`policy`（这份快照的哈希）、`permission`、`oneshot: false` |
| `policy()` | 照快照造出内核的 `Policy` |
| `driver_texts()` | 驱动的五句占位（`drivers/openai-chat.md`） |
| `run_texts()` | 执行器替工具写的两句：`unavailable(工具名)`、`crashed(工具名)` |
| `guard_texts()` | 权限策略拒绝时的三句：`forbidden(路径)`、`unresolvable(路径, 原因)`、`read_only()` |

### 怎么走

**读原文**（`ResourceRoot::sources`，资源目录怎么找见 `store/resources.md`）

1. 人格的编号要合写法：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。它是一层目录的名字，不许带路径。
2. 读 `CoreTexts` 表里的每一份，再读 `personas/<编号>/prompts/persona.md`。原文照抄，行尾的换行也算。
3. `permission-rule.txt` 不读：它现在不进请求。

**拼**（`compose`）

1. `system` 照 `26-提示词.md` 第四节的先后拼：每一块去掉末尾的空白，空的块不要，块和块之间空一行（`\n\n`）。开头的空白是人格自己写的，照留。现在只有人设这一块，所以软件工程师的 system 就是 `You are a helpful software engineer.`。
2. `tools` 先是空的；`with_tools` 带上工具面，照名字的字节序排，稳定排序：交进来的先后不影响字节。
3. `step_limit` 是 `null`，`resumes` 是 3，`attended` 照交进来的，`compaction` 是出厂的四个数。

**字节和哈希**

1. 规范的字节：紧凑的 JSON，字段照结构体的先后，同样的内容字节一定一样。参数格式原样照抄。
2. `tools` 是空的不写这一格：带上一张空的工具面，字节、哈希不变。
3. 哈希：规范字节的 SHA-256，写成 `sha256:` 加 64 位小写十六进制。存成 blob 用的、`session.created` 记的，都是它。
4. 读回来：不认识的字段不理。缺了 `tools`、`core.permissions`、`core.tool_results.unavailable`、`core.tool_results.crashed` 的，读成空的：没有工具的会话用不到它们。缺了 `step_limit` 的读成不限。缺了别的，读不回来。格式改了不背兼容。

**造会话**（`crates/miyu-session/src/open.rs` 的 `create`，在阻塞线程里做）

1. 照人格读原文；拼快照，带上核心工具目录里每一件的规格（名字、说明、参数格式、访问类别）。人格是 `session.create` 写的，不写是 `engineer`；`attended` 是握手时头报的能不能输入（`protocol.md`）。
2. 先造一遍策略、驱动的占位、执行器的两句、权限策略的三句：哪一样造不出来，会话造不成，什么都不存。
3. 快照存成属主家目录里的 blob。先落 blob，再写引用它的事件。
4. 建会话目录和日志，内核记第 1 条 `session.created`：`owner`、`venue`、`policy`（快照的哈希）、`permission`，一次性的再带 `"oneshot":true`。
5. 以后这个会话的工具面一直照快照发，核心的目录变了也不变。

**载入**（`open.rs` 的 `load`）

1. 打开日志；第 1 条不是 `session.created` 的，载入不了。
2. 照它的 `policy` 从属主的 blob 里取字节。取的时候核对内容哈希：没有、对不上、读不了，都载入不了。
3. 读回快照，造策略、驱动的占位、执行器的两句、权限策略的三句，交给内核从日志重建。
4. 工具面照快照；执行时照名字在核心的工具目录里找：快照里有、目录里没有的，结果写 `unavailable` 那一句（`session/tools.md`）。

**造策略**（`policy()`），照这个先后查，先错的先报：

1. 检查点的包装、回合没走完的五句、摘要指令（没有的是空的），交给组装器（`kernel/request.md`）。
2. 工具面拆成两份，照快照里的先后：组装器的工具面（名字、说明、参数格式），内核的工具规则（名字 → 访问类别、参数格式）。两件同名的，造不出。
3. 稳定区：工具面、`system`，示范对话是空的。
4. 三份事实模板，造的时候试换（`kernel/request.md`）。
5. 内核替工具写的十三句（`kernel/tools.md`）。
6. `Policy` 的几格：`assembler`、`facts`、`tools`、`step_limit`、`tool_texts`、`attended`、`resumes`，照快照的带；`compaction`：快照里压缩的数和摘要指令都有的，照数带上，缺一样就是没有，不主动压。

**几句模板**，造的时候拿空的字段试换一次，要了不该要的字段就报错：

| 哪几句 | 字段 | 给人看的说法 |
|---|---|---|
| `run_texts` 的 `unavailable`、`crashed` | `name` | `core/tool-results/unavailable`、`core/tool-results/crashed`，带 `name` |
| `guard_texts` 的 `forbidden` | `path` | `core/permissions/forbidden`，带 `path` |
| `guard_texts` 的 `unresolvable` | `path`、`reason` | `core/permissions/unresolvable`，带 `path`、`reason` |
| `guard_texts` 的 `read_only` | 没有，用的是 `tool_results.read_only` | `core/tool-results/read-only` |

给模型看的那一句，字段照模板的规矩转义；给人看的说法里，字段原样（`kernel/tools.md`）。

**为什么能逐字节重现**

1. 请求里的字只有两个来处：日志里记下的（人说的、她说的、工具的结果、注入的事实），和快照里的（system、工具面、检查点的包装、回合没走完的五句、驱动的占位）。事实、内核和执行器和权限策略替工具写的几句，照快照里的模板写成字，记进日志。
2. 快照的字节就是存下来的那一份，取的时候核对过哈希；参数格式原样。
3. 组装、事实、驱动都是纯函数：同样的快照、同样的日志，出同样的请求。

### 样子

规范的字节，一行紧凑的 JSON（软件工程师，带两件工具，中间省略）：

```text
{"persona":"engineer","system":"You are a helpful software engineer.","tools":[{"name":"edit","description":"…","parameters":{"type":"object"},"access":"write"},{"name":"read",…}],"core":{"checkpoint_open":"<conversation-checkpoint>\n…","checkpoint_close":…,"turn_ended":{…},"facts":{…},"tool_results":{…},"drivers":{…},"permissions":{…},"compaction":{"summarize_task":"Respond with text only. …"}},"step_limit":null,"attended":true,"resumes":3,"compaction":{"reserve_cap":20000,"margin":13000,"image":2000,"file":2000,"tail":16000}}
```

- blob 的位置：`home/<属主>/blobs/<哈希的前两位>/<64 位十六进制>`（`store.md`）。
- `session.created` 的样本：`docs/designs/samples/events/session.created.jsonl`。
- 给模型看的原文见 `kernel/request.md`、`kernel/tools.md`、`drivers/openai-chat.md` 的「样子」，登记在 `26-提示词.md` 第十节。

### 出错

报错是英文，给查问题的人看，写进运行日志，不进请求（施工 4-9 再补四中：原来是中文）。

| 什么时候 | 怎么说 |
|---|---|
| 字节读不回来 | `policy snapshot not readable: <serde 的原因>` |
| 模板坏了 | `bundled <哪一类> not usable: bad template: …`，哪一类是 `fact templates`、`kernel's tool result texts`、`driver placeholders`、`executor's tool result texts`、`permission denial texts` |
| 工具面上两件同名 | `two tools named "<名字>"` |

造会话、载入时的说法（`open.rs`）：

| 什么时候 | 怎么说 |
|---|---|
| 人格读不出来 | `persona not readable: persona id "<编号>" is not valid: …`，或者 `persona not readable: cannot read <路径>: <原因>` |
| 造不出策略 | `policy not built: <上表>` |
| 存不下快照、建不了目录和日志 | `session not created on disk: <原因>` |
| 造会话那一条没落盘 | `session.created not stored; the session stopped` |
| 日志打不开（没有这个会话、日志坏了） | `session log not opened: <原因>` |
| 日志里第 1 条不是造会话 | `the session log has no session.created` |
| 取不出快照 | `policy snapshot not fetched: no blob <哈希>`，或者 `…: blob <哈希> does not match its name; left as it is`，或者读的错 |
| 读不懂快照 | `policy snapshot not understood: policy snapshot not readable: …` |
| 快照造不出策略 | `policy not built from the snapshot: <上表>` |
| 内核载入不了（日志过不了账本） | `not loaded: <原因>` |

协议上怎么回（人格编号不合写法、没有这个人格），见 `protocol.md`。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-policy/src/snapshot/tests.rs` | 软件工程师的 system 就是那一句、`step_limit`、`resumes`；同样的原文同样的字节和哈希，读得回来，开头结尾的样子，改一个字哈希就变；坏字节读不回来；造得出策略，坏模板说是哪一类；`session.created` 带着哈希；开关照给的带；五句占位各是各的 |
| `crates/miyu-policy/src/compose.rs`（内嵌的测试） | system 每块去掉末尾空白、空的不要、空一行 |
| `crates/miyu-policy/src/tools/tests.rs` | 工具面照名字排、读回来一样、交进来的先后不影响字节；没有工具的不写 `tools`，带上空的字节不变；造策略时拆成两份；同名的造不出（读回来的也造不出）；执行器的两句带名字、转义、说法；坏的说是哪一类；缺了这两格的快照读成空的 |
| `crates/miyu-policy/src/guard/tests.rs` | 三句带路径和原因、转义；说法；坏的说是哪一类；缺了 `permissions` 的快照读成空的 |
| `crates/miyu-store/tests/snapshot.rs` | 从源码树的资源拼出快照，存成 blob，哈希就是快照的哈希；取回来一样；两份策略跑同一个剧本，每一次请求逐字节一样 |
| `crates/miyu-store/src/resources/tests.rs` | 读出软件工程师的一句和随核心附带的字；没有的人格说是哪个文件，坏编号被拒 |
| `crates/miyu-session/tests/actor.rs` | 造会话先存快照：`session.created` 记的哈希取得出快照 |
| `crates/miyu-endpoint/tests/tools.rs` | 协议上造的会话，工具面照核心的目录存进快照；换一份核心以后载入，照新核心的目录执行 |
| `crates/miyu-endpoint/tests/endpoint.rs` | 不能输入的头造的会话，快照里没人能确认 |

### 出处

- `03-事件模型.md` E5：策略按内容哈希存档，会话里记引用；写法。
- `26-提示词.md` 第四节「怎么拼」；第八节（出厂的字放在哪）。
- `02-内核.md` K3（策略冻结在会话上）、第六节「载入、崩溃、重启」（接着干的次数）。
- `07-存储.md` 第四节（先落 blob，再写引用它的事件）、第五节（blob）、第九节（密钥不进快照）。
- `08-上下文投影.md` 第二节：模型和供应商不在请求里，也就不在快照里。

### 还没有的

- 示范对话：快照里还没有这一格（`03-事件模型.md` E5，`16-人格与预设.md`）。
- system 只有人设：场所说明、核心和软件包的几行、技能与知识库的列表、没开的软件、子代理能选的人格、风格锁（`26-提示词.md` 第四节）。
- 没人盯着的场所（例如群聊）的步数上限，随预设定；出厂不设（`02-内核.md` 第六节「工具怎么调、下一步怎么走」第 6 条）。
- 预设、人格的覆盖链：自己的家目录、系统区、出厂的（`16-人格与预设.md` 第四节）；现在只读资源目录。
- 会话中途换快照：`session.policy_changed` 带新的 `policy`，下一个回合开始时换（`02-内核.md` K3，`03-事件模型.md` 第三节）。
