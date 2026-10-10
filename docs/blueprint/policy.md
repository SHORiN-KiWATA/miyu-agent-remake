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
| `crates/miyu-policy/src/facts.rs` | 快照里事实的模板 `FactTexts`，造成内核的 `FactTemplates`（施工 2-7 补从 `snapshot.rs` 挪出来） |
| `crates/miyu-policy/src/drivers.rs` | 快照里驱动的几句占位 `DriverPlaceholders`，读成驱动的占位（施工 3-9 四补从 `snapshot.rs` 挪出来） |
| `crates/miyu-policy/src/tools.rs` | 工具面：排序、拆成两份；执行器替工具写的两句 |
| `crates/miyu-policy/src/tools/choice.rs` | 工具面上一件的一个参数照会话开局时的配置填上能选的几个（`ToolEntry::offer`：插 `enum`、说明后面一行一个，一个都没有的拿掉这个参数），照快照读回能选的（`ToolEntry::offered`）；照原样的 JSON 搬，别的字节不动（施工 8-8 补，`subagent` 的 `pool`） |
| `crates/miyu-policy/src/guard.rs` | 权限策略拒绝时写的三句 |
| `crates/miyu-policy/src/image_name.rs` | 带名字的图片的三句（施工 3-9 四补） |
| `crates/miyu-policy/src/attached_path.rs` | 看不了的附件带上原来的路径的两句（施工 3-9 五补） |
| `crates/miyu-policy/src/recap.rs` | 回顾的字 `RecapTexts`、数 `RecapNumbers` 和出厂的数 `RECAP`，交给组装器的样子（施工 3-8 四补） |
| `crates/miyu-policy/src/vision.rs` | 替看不了图的模型看图的字（施工 8-17）：转述请求的两份 `VisionTexts`、图的位置的三句标签 `ImageDescriptionTexts`，交给组装器、驱动的样子 |
| `crates/miyu-policy/src/title.rs` | 起标题的字 `TitleTexts`、数 `TitleNumbers` 和出厂的数 `TITLE`，交给组装器、内核的样子（施工 3-8 五补） |
| `crates/miyu-policy/src/jobs.rs` | 回报的写法、任务用的数；子会话回报的正文怎么截（`reports()`，施工 3-8 五补从 `snapshot.rs` 挪来，那边放不下了） |
| `crates/miyu-policy/src/memory.rs` | 记忆的范围 `MemoryScope`，快照的 `memory` 怎么写（`with_memory`）、怎么读（`memory_scope`，施工 R-3 下） |
| `crates/miyu-policy/src/peers.rs` | 别的会话发来的话：防刷屏的数 `PeerNumbers` 和出厂的数 `PEERS`，交给内核的样子；标签的两份 `PeerTexts`，交给组装器的样子（施工 C-2）。「空了告诉我」的两个数和通知的五份字 `PeerIdleTexts`，作废那一句照快照的小时数换好（施工 C-6） |
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
| `compaction` | 对象 | 压缩用的数：`reserve_cap` 输出预留的上限、`margin` 余量、`image`、`file` 估算时一张图、一个文件各算多少 token、`tail` 尾巴的上限，现在是 20000、13000、2000、2000、16000（施工 6-2）。以前造的快照里没有，读成没有；没有的不写。6-2（上）造的没有 `tail`，读成 16000。`rebuild` 压后重建的数（施工 6-5）。`pause` 熔断的数：`failures` 连续失败几次、`turns` 几个回合内又到线算快、`refills` 连着快几次，现在都是 3（施工 6-6 上）；以前造的没有，读成没有：不熔断。`shorten` 截短重试的数：`tries` 最多再试几次、`percent` 没说超多少时截百分之几，现在是 3、20（施工 6-6 中）；以前造的没有，或者只有数、没有字的，不截短。`line_percent`、`margin_percent` 压缩线、余量各至多窗口的百分之几，现在是 85、5，排在 `margin` 后面（施工 6-11 再补，`compaction.md` 第二条第 2 条）；以前造的没有，读成 85、5：老会话也照它 |
| `jobs` | 对象 | 任务用的数（施工 7-6）：`report_chars` 子会话回报的正文最多几个字，现在是 30000（`agents.md`「对外的样子」）。以前造的快照里没有，读成没有、不写：照出厂的 30000 截 |
| `recap` | 对象 | 回顾用的数（施工 3-8 四补，`kernel/request.md`「回顾的请求」）：`turns` 最多喂几轮她答过的、`tokens` 整份最多约多少 token，现在是 8、8192（2026-10-01 项目主人定，照 codex）。以前造的快照里没有，读成没有、不写：不做回顾 |
| `title` | 对象 | 起标题用的数（施工 3-8 五补，`kernel/request.md`「起标题的请求」、`kernel/session.md`「起标题」）：`tokens` 整份请求最多约多少 token、`chars` 标题最多几个字、`tries` 一个会话最多试几次，现在是 1024、50、2（1024 照回顾的截法施工时定；50 是 2026-10-01 项目主人定；2 是施工单定的）。以前造的快照里没有，读成没有、不写：不起标题 |
| `peers` | 对象 | 别的会话发来的话怎么防刷屏（施工 C-2，`cross-session.md`「对外的样子」）：`burst` 同一个发话方一个窗口里最多几句、`window` 窗口多少秒、`unread` 没听到的最多几句，现在是 5、600、50（数是估的，待 C-7 实测）。排在 `title` 后面。以前造的快照里没有，读成没有、不写：照出厂的数，防刷屏不能因为会话旧就不管。施工 C-6 加两格，排在后面：`watch_hours` 订了多久没等到就作废（小时，出厂 12）、`status_chars` 通知那一行最多几个字（出厂 200）；C-2 时造的快照里没有这两格，读成没有、不写，照出厂的数 |
| `memory` | 字符串 | 记忆的范围（施工 R-3 下，`memory.md`「范围」）：`persona`、`session`、`off`，造会话时定（`with_memory`），新造的都写明；排在 `peers` 后面、最后。以前造的快照里没有，读成没有、不写，照 `persona`；认不出的照 `off` |

**`ToolEntry`**：`name`、`description`、`parameters`（参数的 JSON Schema，原样的 JSON）、`access`（`read`、`write`、`execute`、`network`、`outbound`、`venue`（施工 O-31 前），不认识的原样留着），照这个先后。

**`CoreTexts`**，每一格是 `resources/core/` 下一份文件的原文：

| 格 | 里面的格 | 文件 |
|---|---|---|
| `checkpoint_open`、`checkpoint_close`、`checkpoint_end` | | `checkpoint-open.txt`、`checkpoint-close.txt`、`checkpoint-end.txt`（施工 6-5 从 close 里拆出来。以前造的快照里没有 end，读成空的：close 里原本就带着那一句，拼出来一字不差） |
| `turn_ended` | `interrupted`、`error`、`step_limit`、`aborted`、`restarted` | `turn-ended/<同名>.txt` |
| `facts` | `env`、`permission`、`reply_cut`、`session`、`permission_changed` | `facts/env.txt`、`facts/permission.txt`、`facts/reply-cut.txt`、`facts/session.txt`（施工 1-13 再补，会话编号。以前造的快照里没有，读成没有、不写：那些会话不注入这一块）、`facts/permission-changed.txt`（施工 2-7 补，切了级别以后的权限那一块，读法照 `session`：以前造的快照里没有，读成没有、不写，那些会话切了照旧写平常那一份，快照的字节、请求的前缀都一字不变） |
| `tool_results` | `unknown`、`not_an_object`、`cancelled_before`、`cancelled_running`、`skipped`、`read_only`、`denied`、`denied_with_reason`、`unattended`、`question_interrupted`、`question_voided`、`question_unattended`、`restarted`、`unavailable`、`crashed` | `tool-results/` 下，下划线换成 `-` 的同名文件 |
| `drivers` | `image_omitted`、`file_omitted`、`no_output`、`tool_attachments`、`tool_attachments_only`；`text_file` 里的 `file_open`、`file_cut`、`file_close`；`image_name` 里的 `image_open`、`image_close`、`image_omitted_named` | `drivers/` 下，下划线换成 `-` 的同名文件。`text_file`（施工 3-9 三补，文本文件照字放进消息，`drivers/openai-chat.md` 第 9 条）以前造的快照里没有，读成没有、不写：文本文件照别的文件写占位。`image_name`（施工 3-9 四补，带名字的图片，同一条）也是：以前造的快照里没有，读成没有、不写，带名字的图片照不带名字的写。`attached_path` 里的 `image_omitted_path`、`file_omitted_path`（施工 3-9 五补，看不了的附件带上原来的路径）同样：以前造的快照里没有，读成没有、不写，照旧不带路径。`image_description` 里的 `image_description_open`、`image_description_open_named`、`image_description_close`（施工 8-17，替它看的图的标签，同一条）也是：以前造的快照里没有，读成没有、不写，看不了图的照旧写占位 |
| `permissions` | `forbidden`、`unresolvable` | `permissions/forbidden.txt`、`permissions/unresolvable.txt` |
| `compaction` | `summarize_task`、`summarize_instructions`、`summarize_end`、`notes_files`、`notes_files_more`、`notes_retrieve`、`notes_too_large`、`restored_open`、`restored_close`、`truncated`、`notes_uncovered`、`summarize_system` | `compaction/` 下，下划线换成 `-` 的同名文件（摘要指令施工 6-2 上，截短重试的两份施工 6-6 中，隔离式那一句施工 6-6 下，别的施工 6-5；`summarize_instructions`、`summarize_end` 施工 6-8 从摘要指令里拆出来）。以前造的快照里没有，读成没有；没有的不写：没有 `notes_*` 的不写那一段，没有 `restored_*` 的不重读，没有截短重试的两份的不截短，没有 `summarize_system` 的不改走隔离式；有 `summarize_task`、没有 `summarize_instructions`、`summarize_end` 的，那两份读成空的：那时的 `summarize_task` 里本来就带着最后那一句，拼出来一字不差 |
| `jobs` | `command_open`、`command_exit`、`command_signal`、`command_duration`、`command_output`、`command_close`、`subagent_open`、`subagent_person`、`subagent_truncated`、`subagent_silent`、`subagent_close`、`subagent_omitted`、`stopped_by_user`、`subagent_message_open`、`subagent_message_close` | `jobs/` 下，下划线换成 `-` 的同名文件（施工 7-2，两种回报的写法，`kernel/request.md`「回报」）。以前造的快照里没有，读成没有、不写：回报不渲染，那些会话也派不出任务。`subagent_omitted` 是子会话回报的正文截在中间的那一行，内核截的时候用、不交给组装器（施工 7-6）；7-2 到 7-5 造的没有，读成空的、不写：头尾之间只换一行。`stopped_by_user` 是人停的那一句，两种回报共用（施工 7-2 补，`kernel/request.md`「回报」第 3 条）；以前造的没有，读成空的、不写：人停的照原来的写。`subagent_message_open`、`subagent_message_close` 是子代理发来的留言的标签（施工 7-7，`kernel/request.md`「子代理的留言」）；以前造的没有，读成空的、不写：留言只剩它的话 |
| `harness` | `message_open`、`message_close` | `harness/` 下，下划线换成 `-` 的同名文件（施工 7-10，别的 harness 发来的话的标签，`kernel/request.md`「别的 harness 发来的话」）。以前造的快照里没有，读成没有、不写：那种话照人的话原样渲染 |
| `peers` | `message_open`、`message_close`；`idle_open`、`idle_silent`、`idle_expired`、`idle_gone`、`idle_close` | `peers/` 下，下划线换成 `-` 的同名文件（施工 C-2，别的会话发来的话的标签，`kernel/request.md`「别的会话发来的话」；施工 C-6，空了的通知，「空了的通知」）。排在 `harness` 后面。以前造的快照里没有，读成没有、不写：那种话照人的话原样渲染。通知的五份是一组、和标签平铺在同一层，C-2 时造的快照里没有，读成没有、不写：通知不出 |
| `recap` | `instruction`、`user`、`assistant`、`omitted`、`excerpted` | `recap/` 下的同名文件（施工 3-8 四补，回顾的请求，`kernel/request.md`「回顾的请求」）。以前造的快照里没有，读成没有、不写：不做回顾 |
| `title` | `instruction` | `title/instruction.txt`（施工 3-8 五补，起标题的请求，`kernel/request.md`「起标题的请求」）；标签、截断的记号借 `recap` 的。以前造的快照里没有，读成没有、不写：不起标题 |
| `vision` | `instruction`、`question` | `vision/` 下的同名文件（施工 8-17，转述一张图的请求，`kernel/request.md`「替它看的图」）。以前造的快照里没有，读成没有、不写：不转述 |

**函数**：

| 名字 | 做什么 |
|---|---|
| `compose(人格, Sources, attended)` | 拼一份快照。`Sources` 是读好的原文：`core`（`CoreTexts`）、`persona`（`PersonaTexts { persona, examples, reminders }`：人设、示范对话、角色扮演提示的原文）、`reminder`（角色扮演提示的包装 `Wrap { open, close }`，施工 P-1 补） |
| `Snapshot::with_tools(工具)` | 带上工具面 |
| `Snapshot::with_venue(说明)` | 带上场所说明：system 的第二块，接在人设后面（施工 7-5）。子会话有 |
| `Snapshot::with_group(说明, GroupChat)` | 带上群会话（施工 O-13 中）：格式说明照 `with_venue` 接在人设后面（`core/venues/group.txt`），快照记下 `group` |
| `Snapshot::with_core_lines(&CoreLines)` | 带上核心的几行（施工 2-7 补）：system 的第三块，所以在 `with_tools`、`with_venue` 以后调。`CoreLines` 有三格：`permission`（`permission-rule.txt`）、`local_paths`（`local-paths-rule.txt`）、`style_lock`（`style-lock.txt`，施工 P-1 补，交给 `with_style_lock`）。一行一句，先权限、后路径；工具面是空的不带权限那一句 |
| `Snapshot::swappable(新的)` | 能不能换成 `新的`（施工 P-1 再补）：除了 `system`、`demos`、`tools`、`reminder`、`persona_digest`、`preset`（后两格施工 P-2 下：换预设时工具面重新筛过，留着的那几件是原样），别的格都一样 |
| `Snapshot::with_preset(预设, 那一行)` | 带上预设（施工 P-2 中）：记进快照的 `preset`（编号、装了没开的软件）；没开角色扮演的去掉 `reminder`；装了没开的软件（角色扮演除外）写成一行接在 system 后面（`26-提示词.md` 第四节第 5 块，`core/preset-off.txt`）。在 `with_core_lines` 以后、`with_style_lock` 以前调；没有预设、都开着的 system 一字不变 |
| `Snapshot::with_style_lock(风格锁)` | 带上风格锁（施工 P-1 补）：system 的最后一块（`26-提示词.md` 第四节第 7 块），在 `with_core_lines` 以后调。快照有 `reminder` 的才带，别的 system 一字不变 |
| `REPORT_CHARS` | 策略数据 `jobs.report_chars` 的出厂值 30000（施工 7-6）：拼快照时写进 `jobs` |
| `JOB_DEPTH` | 策略数据 `jobs.depth` 的出厂值 2（`agents.md`「对外的样子」，施工 7-5）：造会话定工具面时用，不进快照 |
| `RECAP` | 回顾用的数的出厂值：8 轮、8192 个 token（施工 3-8 四补）：拼快照时写进 `recap` |
| `TITLE` | 起标题用的数的出厂值：1024 个 token、50 个字、试 2 次（施工 3-8 五补）：拼快照时写进 `title` |
| `PEERS` | 防刷屏的数的出厂值：5 句、600 秒、50 句（施工 C-2），12 小时、200 个字（施工 C-6）：拼快照时写进 `peers`，以前造的快照没有的照它 |
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
3. 核心的几行 `core/permission-rule.txt`、`core/local-paths-rule.txt` 另读（`ResourceRoot::core_lines`，施工 2-7 补），交给 `with_core_lines`；它们只拼进 system，不另存进快照的 `core`。风格锁 `core/style-lock.txt` 和它们一起读（施工 P-1 补），装了没开的那一行 `core/preset-off.txt` 也是（施工 P-2 中）。
4. 角色扮演提示的包装 `core/facts/reminder-open.txt`、`reminder-close.txt` 读进 `Sources.reminder`（施工 P-1 补）：拼进快照的 `reminder`，也不另存进 `core`，没有角色扮演提示的快照字节不变。

**拼**（`compose`）

1. `system` 照 `26-提示词.md` 第四节的先后拼：每一块去掉末尾的空白，空的块不要，块和块之间空一行（`\n\n`）。开头的空白是人格自己写的，照留。现在只有人设这一块，所以软件工程师的 system 就是 `You are a helpful software engineer.`；子会话多一块场所说明（`core/jobs/subagent-venue.txt`），`with_venue` 照同样的规矩接在人设后面（施工 7-5，`agents.md` 第九条第 3 条）。造会话时最后接上核心的几行（`with_core_lines`，施工 2-7 补）：`You are a helpful software engineer.`、空一行、权限那一句、换行、路径那一句。以前造的快照 system 已经拼好存着，载入照它发，前缀一字不变。
2. `tools` 先是空的；`with_tools` 带上工具面，照名字的字节序排，稳定排序：交进来的先后不影响字节。
3. `step_limit` 是 `null`，`resumes` 是 3，`attended` 照交进来的，`compaction` 是出厂的四个数，`recap` 是出厂的两个数（施工 3-8 四补），`title` 是出厂的三个数（施工 3-8 五补），`peers` 是出厂的五个数（施工 C-2、C-6），`memory` 不写（造会话时 `with_memory` 写上，施工 R-3 下）。
4. `reminder`（施工 P-1 补）：人格的角色扮演提示去掉末尾空白，是空的不写；不空的拼成一块：包装的开头、原文、换行、包装的收尾。原文不转义：是人格的作者写的。`policy()` 把它交给内核的事实模板（`FactTemplates::with_reminder`），回合开始时隔几轮注入（`kernel/request.md`「事实」）。造会话时最后带上风格锁（`with_style_lock`）：有 `reminder` 的 system 末尾空一行接上 `core/style-lock.txt`。
6. `preset`（施工 P-2 中）：会话的预设，`{"id", "off", "digest"}`，`off` 是造会话时装了、这个预设没开的软件，照编号排，都开着的不写；`digest` 是叠好的预设文件的指纹（`PresetFile::digest`，施工 P-2 下），回合开始时执行器照它认出预设改了，P-2（中）造的没有、不换。工具面、记忆的范围造会话时已经照预设筛过；换人格重拼时照它（`with_preset`）去掉角色扮演提示、写那一行，所以人格的指纹照旧算人格原来的字，不会每轮都当成改过。以前造的快照没有：全开。
7. `group`（施工 O-13 中）：群会话钉下的 `{"offset", "no_text", "recent", "voice"}`（`recent` 施工 O-13 下：群聊近况的块头、缺口提示的原文和预算 `budget`，出厂 `RECENT_BUDGET` 80000；O-13 中造的没有、不写，不出近况；缺口提示的字段只能是 `count`，不合的造不出策略。`voice` 施工 O-33：语音那一句 `core/venues/voice.txt` 的原文，交给组装器时去掉行尾空白；有它的带的东西标第几个，O-33 以前造的没有、不写，照以前写，`kernel/request.md`「群里的一行」）：造会话时这台机器的时区（比 UTC 早多少分钟），正文、带的东西都没有的那一条写什么（`core/venues/no-text.txt` 的原文）。`policy()` 交给组装器（时区换成内核的类型、空的那一句去掉行尾空白），群里的人说的照它渲染成一行一条；时区超出 ±14:00 的造不出策略（`BuildError::Offset`）。私聊、本机的会话没有、不写，字节和以前一样。换人格、换预设重拼时照旧快照的时区。
8. `foreground`（施工 T-1 上）：后台运行关着的会话是真（预设关了 `basesystem` 的 `background`，设计 30 第三节第 7 条）：执行器不给任务端口，工具面上 `shell` 没有放到后台那一项。开着的、以前造的不写，字节和以前一样。
5. `persona_digest`（施工 P-1 再补）：人格三份字（人设、示范对话、角色扮演提示，叠好的）的 SHA-256（`PersonaTexts::digest`，三样写成一个 JSON 数组再算）。回合开始时执行器照它认出人格的文件改了（`session/actor.md`「换快照」）。以前造的快照没有，读成没有、不写：那些会话不换。

**字节和哈希**

1. 规范的字节：紧凑的 JSON，字段照结构体的先后，同样的内容字节一定一样。参数格式原样照抄。
2. `tools` 是空的不写这一格：带上一张空的工具面，字节、哈希不变。
3. 哈希：规范字节的 SHA-256，写成 `sha256:` 加 64 位小写十六进制。存成 blob 用的、`session.created` 记的，都是它。
4. 读回来：不认识的字段不理。缺了 `tools`、`core.permissions`、`core.tool_results.unavailable`、`core.tool_results.crashed` 的，读成空的：没有工具的会话用不到它们。缺了 `step_limit` 的读成不限。缺了别的，读不回来。格式改了不背兼容。

**造会话**（`crates/miyu-session/src/open.rs` 的 `create`，在阻塞线程里做）

1. 照人格读原文；拼快照，带上核心工具目录里每一件的规格（名字、说明、参数格式、访问类别）。人格是 `session.create` 写的，不写是 `engineer`；`attended` 是握手时头报的能不能输入（`protocol.md`）。不能派子代理的会话（不在本机、到了深度上限）不带 `subagent`；子会话带上场所说明（施工 7-5，`session/tools.md`「工具面」）。快照的格式不变：施工 8-8 给 `subagent` 加了参数 `tier`，施工 8-8 补换成 `pool`：`pool` 那一格照造会话时的配置拼好（列哪几个池、每个一行说明，一个都没有的拿掉，`session/tools.md`「工具面」、`tools/subagent.md`「会话开局时拼 `pool`」），快照里存的是拼好的。以前造的照旧，前缀一个字节不变；同一个核心上开着的几个会话，这一格可以不一样。施工 8-15 目录里多了 `session_usage`：新造的本机会话工具面多这一件，群里的没有；以前造的快照没有它，照快照发，前缀一个字节不变。
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

1. 检查点的包装、回合没走完的五句、摘要指令（没有的是空的）、回顾的字和数（两样都有的才有，缺一样就是没有、不做回顾，施工 3-8 四补）、起标题的字和数（两样都有的才有，施工 3-8 五补；组装时还要有回顾的标签）、转述一张图的两份（有的才有，施工 8-17）、回报的写法（没有的是没有；有的，带字段的七份读成模板，拿各自的字段试换一次：标签的两份 `job`、`title`、`reason`，另外四份各一个 `code`、`signal`、`ms`、`chars`，写坏了、要了别的字段的造不出，说是 `job report texts`），交给组装器（`kernel/request.md`）。
2. 工具面拆成两份，照快照里的先后：组装器的工具面（名字、说明、参数格式），内核的工具规则（名字 → 访问类别、参数格式）。两件同名的，造不出。
3. 稳定区：工具面、`system`，示范对话是空的。
4. 事实模板，造的时候试换（`kernel/request.md`）：三份，加上会话编号、切换那两份（有的话）。
5. 内核替工具写的十三句（`kernel/tools.md`）。
6. `Policy` 的几格：`assembler`、`facts`、`tools`、`step_limit`、`tool_texts`、`attended`、`resumes`，照快照的带；`compaction`：快照里压缩的数和摘要指令都有的，照数带上，缺一样就是没有，不主动压；`titles`：快照里起标题的字和数都有的，带上 `tries`、`chars`，缺一样就是没有，不起标题（施工 3-8 五补）。

**几句模板**，造的时候拿空的字段试换一次，要了不该要的字段就报错：

| 哪几句 | 字段 | 给人看的说法 |
|---|---|---|
| `run_texts` 的 `unavailable`、`crashed` | `name` | `core/tool-results/unavailable`、`core/tool-results/crashed`，带 `name` |
| `guard_texts` 的 `forbidden` | `path` | `core/permissions/forbidden`，带 `path` |
| `guard_texts` 的 `unresolvable` | `path`、`reason` | `core/permissions/unresolvable`，带 `path`、`reason` |
| `guard_texts` 的 `read_only` | 没有，用的是 `tool_results.read_only` | `core/tool-results/read-only` |

给模型看的那一句，字段照模板的规矩转义；给人看的说法里，字段原样（`kernel/tools.md`）。

**为什么能逐字节重现**

1. 请求里的字只有两个来处：日志里记下的（人说的、她说的、工具的结果、注入的事实、图的转述），和快照里的（system、工具面、检查点的包装、回合没走完的五句、驱动的占位和标签）。事实、内核和执行器和权限策略替工具写的几句，照快照里的模板写成字，记进日志。
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
| `crates/miyu-policy/src/snapshot/tests.rs`、`snapshot/tests/facts.rs` | 软件工程师的 system 就是那一句、`step_limit`、`resumes`；同样的原文同样的字节和哈希，读得回来，开头结尾的样子，改一个字哈希就变；坏字节读不回来；造得出策略，坏模板说是哪一类；`session.created` 带着哈希；开关照给的带；五句占位各是各的，带名字的图片的三句也是，以前造的快照没有这三句的读回来一字不差、照不带名字的写（施工 3-9 四补）；会话编号的模板进快照、造的策略写得出那一块、坏了说是事实的模板，以前造的快照没有这一格的读进来再写出去一字不差、没有那一块（`facts.rs`，施工 1-13 再补）；切了级别以后的那一份一样：进快照、写得出那一块、坏了说是事实的模板，以前造的没有这一格的读回来一字不差、没有那一块（`facts.rs`，施工 2-7 补） |
| `crates/miyu-policy/src/compose.rs`（内嵌的测试） | system 每块去掉末尾空白、空的不要、空一行；场所说明接在人设后面、空的不留空行（施工 7-5）；核心的几行排在人设、场所说明后面，一行一句，工具面是空的不带权限那一句，没接它的 system 和原来一样、空的几行不留空行（施工 2-7 补） |
| `crates/miyu-policy/src/tools/tests.rs` | 工具面照名字排、读回来一样、交进来的先后不影响字节；没有工具的不写 `tools`，带上空的字节不变；造策略时拆成两份；同名的造不出（读回来的也造不出）；执行器的两句带名字、转义、说法；坏的说是哪一类；缺了这两格的快照读成空的 |
| `crates/miyu-policy/src/tools/choice/tests.rs` | 拼：一个都没有的拿掉这个参数、和资源去掉它一字不差；有的插 `enum`、说明一行一个、没写说明的只写名字；别的字节不动；读回的和拼进去的一样，没有这一格、写坏的是空的（施工 8-8 补） |
| `crates/miyu-policy/src/snapshot/tests/recap.rs`（施工 3-8 四补） | 回顾进快照：出厂的快照带着五份字和两个数，造出的组装器回顾得出来；字、数少一样都不回顾；快照里的数照快照的；以前造的快照没有这两格，读进来再写出去一字不差，不回顾 |
| `crates/miyu-policy/src/snapshot/tests/vision.rs`（施工 8-17） | 替看图进快照：出厂的快照带着两份字和三句标签，造出的组装器交得出转述的请求、驱动写得出带标签的转述；以前造的快照没有这两格，读进来再写出去一字不差，不转述、照旧写占位 |
| `crates/miyu-policy/src/snapshot/tests/title.rs`（施工 3-8 五补） | 起标题进快照：出厂的快照带着指令和三个数，造出的组装器起得出标题、内核拿到 `tries`、`chars`；字、数少一样都不起，没有回顾的标签的组装器也不起；快照里的数照快照的；以前造的快照没有这两格，读进来再写出去一字不差，不起 |
| `crates/miyu-policy/src/snapshot/tests/jobs.rs`（施工 3-8 五补从 `tests.rs` 挪出来） | 子会话回报的正文怎么截：出厂的 30000 和截在中间的那一行；以前造的快照没有，读成出厂的数、空的那一行，读写一字不差；快照里的数照快照的 |
| `crates/miyu-policy/src/peers/tests.rs`（施工 C-2、C-6） | 空了的通知（C-6）：出厂的快照多两个数、通知的五份字（平铺在 `core.peers` 里），作废那一句照快照的小时数换；通知的字坏了照名字报；C-2 时造的快照没有这几格，数照出厂的、通知不出、读回写出一字不差。别的会话发来的话进快照：出厂的快照带着三个数（排在最后）和两份标签，造出的策略照出厂的数、组装器渲染出带短编号的标签；快照里的数照快照的；标签坏了照名字报；以前造的快照没有这两格，读进来再写出去一字不差，照出厂的数防刷屏，那种话和人的话一字不差 |
| `crates/miyu-policy/src/memory/tests.rs`（施工 R-3 下） | 记忆的范围进快照：三种写进去读回一字不差、排在最后；以前造的快照没有这一格，字节不变、照 `persona`；认不出的照 `off` |
| `crates/miyu-policy/src/jobs/tests.rs` | 人停的那一句：出厂的快照带着、交给组装器；以前造的快照没有，读成空的，读回来一字不差（施工 7-2 补） |
| `crates/miyu-policy/src/guard/tests.rs` | 三句带路径和原因、转义；说法；坏的说是哪一类；缺了 `permissions` 的快照读成空的 |
| `crates/miyu-store/tests/snapshot.rs` | 从源码树的资源拼出快照，存成 blob，哈希就是快照的哈希；取回来一样；两份策略跑同一个剧本，每一次请求逐字节一样 |
| `crates/miyu-store/src/resources/tests.rs` | 读出软件工程师的一句和随核心附带的字（会话编号的模板是它那份文件，施工 1-13 再补；回顾的五份各是各的文件，施工 3-8 四补；切了级别以后的权限那一份也是，施工 2-7 补；起标题的指令，施工 3-8 五补）；没有的人格说是哪个文件，坏编号被拒 |
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
- system 还没有的几块：别的场所的说明、软件包的几行、技能与知识库的列表、子代理能选的人格（`26-提示词.md` 第四节）。
- 没人盯着的场所（例如群聊）的步数上限，随预设定；出厂不设（`02-内核.md` 第六节「工具怎么调、下一步怎么走」第 6 条）。
- 预设、人格的覆盖链：自己的家目录、系统区、出厂的（`16-人格与预设.md` 第四节）；现在只读资源目录。
- 会话中途换快照：`session.policy_changed` 带新的 `policy`，下一个回合开始时换（`02-内核.md` K3，`03-事件模型.md` 第三节）。
