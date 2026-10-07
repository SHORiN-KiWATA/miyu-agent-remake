## `miyu ask`

### 是什么

在 shell 里跟她说一句话，可以附上文件：连上核心（没在跑就拉起来），开一个一次性会话或者接着说；她的回答边收边打，她做的每一步印成一行，问完印一行用量。她派了子代理、给子代理留了言的，等它们都报回来、被回报叫醒的几轮也印完才退出（施工 7-9，留了言的施工 7-9 补）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `ask`；换上帮助页；拉起核心用的命令是自己加上 `core` |
| `crates/miyu-cli/src/ask.rs` | 参数、退出码、找数据根、连核心、Ctrl+C；`--from`（施工 7-10） |
| `crates/miyu-cli/src/ask/talk.rs` | 握手、传附件（施工 3-9 三补）、找会话、订阅、发、跟着那一轮；等子代理时按 Ctrl+C 不等了、`--timeout`（施工 7-9） |
| `crates/miyu-cli/src/ask/follow.rs` | 收推送：回答、思考、每一步、用量 |
| `crates/miyu-cli/src/ask/follow/ending.rs` | 一轮结束、都结束了、不等了：收尾印的几行，退出码（施工 7-9 从 `follow.rs` 挪出来） |
| `crates/miyu-cli/src/ask/follow/waiting.rs` | 等子代理：跟被回报叫醒的几轮，等的那一行，报回来了那一行（施工 7-9） |
| `crates/miyu-cli/src/ask/follow/agents.rs` | 照事件流数还有几个子代理没报、回报叫醒的那一轮来不来（施工 7-9）；这一次以前派、这一次留了言的也数（施工 7-9 补） |
| `crates/miyu-cli/src/ask/follow/joining.rs` | 她正忙时，跟住听到这一句的那一轮（施工 7-10，「怎么走」第 8 条） |
| `crates/miyu-cli/src/ask/steps.rs` | 每一步的标题、目录太宽那一句、沙盒用不了那一句（施工 5-4 下）、最后那一句 |
| `crates/miyu-cli/src/ask/steps/blocks.rs` | 执行命令、编辑那一块下面印什么（施工 4-11） |
| `crates/miyu-cli/src/ask/usage.rs` | 用量加起来 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、请求的编号、一行怎么上色、路径怎么写短；和 `miyu undo` 共用。`rpc.rs` 放下时掐掉读的任务，连接当场关上（施工 7-9） |
| `crates/miyu-cli/src/language.rs`、`language/agents.rs`、`language/harness.rs`、`language/config.rs` | 给人看的字；等子代理时的那几句（施工 7-9）；`--from` 不等了的两句（施工 7-10）；配置有错那一句（施工 8-2）、项目配置没信任那一句（施工 8-3） |
| `crates/miyu-cli/src/ask/follow/opening.rs` | 开头那几行旁白：配置有错、项目配置没信任、沙盒用不了、目录太宽（施工 8-3 从 `follow.rs` 挪出来） |
| `crates/miyu-cli/src/help/{zh,en}/ask.txt` | 帮助页（`cli/main.md`「帮助页」） |
| `resources/software/basesystem/human/{zh,en}.json` | 每件工具的符号、显示名、下面印哪一块，结果那一句 |
| `resources/core/human/{zh,en}.json` | 内核记的那几句结果的说法（例如 `tool-results/unattended`） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `<words>...` | 要说的话，必填；几个词用一个空格连起来 |
| `-s`、`--session <编号>` | 接着这个会话说；和 `--continue` 不能一起写 |
| `-c`、`--continue` | 接着最新的那个一次性会话说 |
| `--format text\|json` | 默认 `text` |
| `--add-dir <目录>` | 多放行一个目录：和工作区一样能读能写；可以写好几次（施工 5-10 上） |
| `--file <文件>` | 附上一个文件：图片、PDF、文本……；可以写好几次，照写的先后（施工 3-9 三补） |
| `--from <名字>` | 别的 harness 发的（施工 7-10，`agents.md` 第十一条第 4 条）：写上它的名字，这一句记成它说的，不是本人。和 `-s`、`-c` 一起用是往那个会话里发；都不写开一个新会话。空的、只有空白的照「参数写错时」说（`cli/main.md`），退出码 2 |
| `--model <模型>` | 用哪个模型（施工 8-10，`models.md`「命令行」）：模型 `<供应商>/<模型>` 或 `@池`，原样交给核心解析（挡位 8-8 补去掉了，写挡位名的核心回 `unknown_model`）。不写 `-c`、`-s` 的，新开的会话照它造；接着说的，先换成它：永久换，以后都用它（`models.md`「定的」第 2 条） |
| `--no-memory` | 不开记忆（施工 R-3 下，`memory.md`「范围」）：新开的会话 `session.create` 带 `"memory": "off"`，不召回、不记，工具面上没有记忆的三件。接着说的会话范围开局时就定了：和 `-c`、`-s` 一起写的读参数时就拒（照「参数写错时」） |
| `--timeout <时长>` | 最多等多久：从发出算到全部了结，到了不再等，退出码 3（施工 7-9，「等子代理」第 8 条）。正整数，后面可以跟 `s`、`m`、`h`，不写是秒（`30`、`30s`、`10m`、`1h`），照 GNU `timeout` 的写法；0、负数、小数、别的单位照「参数写错时」说（`cli/main.md`），退出码 2。长到算不出那一刻的，当没写 |

- 界面语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 里第一个设了、不是空的（`cli/main.md`），`zh` 开头说中文，别的说英文。帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`MIYU_RESOURCES`（资源目录，开发时用）、`NO_COLOR`。模型、key 来自配置（施工 8-6，`models.md`），头不看 key。

### 怎么走

0. **加进来的目录**（施工 5-10 上）：`--add-dir` 的每一个，相对的照敲命令时的目录接成绝对的；不存在的、不是目录的，照「参数写错时」说（`cli/main.md`），退出码 2。照写的先后，去掉重复的。
1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**：
   1. 连；核心没在跑就拉起来（施工 8-6 起 key 来自配置，一律拉起；以前没设 `DEEPSEEK_API_KEY` 的不拉起）。
   2. 连不上：原因写在标准错误上，退出码 1。
   3. **先看有没有模型**（施工 8-11，`models.md` 第七条第 6 条）：没写 `--model` 的，在这条连接上握手、问 `config.get` 的 `models.chat`（`crates/miyu-cli/src/setup.rs` 的 `model_ready_on`）。有值的往下走。没有的：标准输入、标准错误都是终端的，说「还没有模型，先接上一个。」，接着走一遍 `miyu setup`（`cli/setup.md`，参数都不写），写好了往下走，没走完的照它的退出码退出；不是终端的，说没有模型那一句（下面「给人看的字」），退出码 5，不造会话。往下走的另连一次核心，从第 3 条握手起照常（这一条连接只管看模型、走 setup）。写了 `--model` 的不看：造会话、换模型时核心照它解析。
3. **握手** `hello`：`protocol` 是 `[1, 1]`；`head` 是 `{"kind": "cli", "version": <版本>}`；`locale` 是 `zh-CN` 或 `en`；`caps.input` 是 `false`；带上本机令牌。
   - `caps.input` 是 `false`：`miyu ask` 里没有确认的界面，要确认的那一步，核心当场拒绝。
   - 回应里的 `sandbox` 说用不了：执行命令都要确认，这里确认不了。第一步之前、目录太宽那一句之前说一句，照原因和这台机器的系统写（下面「给人看的字」），一次（施工 5-4 下）。
   - 之后给人看的字照回应的 `language` 说（施工 8-2，`cli/main.md`「界面语言」）：`zh` 的中文，`en`、`ja` 的英文，没回的照握手以前的；换了的，给人看的字（`human/<语言>.json`）照新的那种重读一份。
   - 回应里有 `config_errors` 的：最先说一句配置里有几处错误，在沙盒用不了那一句前面，一次；`--format json` 的不说（施工 8-2，`config.md` 第十条第 10 条）。造会话、说话的回应里有 `untrusted_project` 的：接着说一句这里的项目配置还没信任、这次没用它，一次 `miyu ask` 只说一次；`--format json` 的不说（施工 8-3，`config.md` 第十条第 10 条）。新开的会话照造会话的回应说，接着说的会话照说话的回应说。
4. **传附件**（施工 3-9 三补）：`--file` 的每一个，读参数时相对的照敲命令时的目录接成绝对的（在不在、多大不查，由核心说），照写的先后发 `blob.put`，带 `{"path": <绝对路径>}`；写了几次附几次，不去重。
   - 被拒绝的：标准错误上印 `附不上 <绝对路径>：<核心照握手时的语言说的原因>`，退出码 1，不再往下：不造会话、不发话，也不留下空的会话。核心断开的，照第 11 条。
   - 在找会话之前传：`--continue`、`--session` 的，传不上也什么都不送进那个会话。
5. **找会话**：
   1. 不写：`session.create`，带 `cwd`（敲命令时的目录；读不出来的写 `.`）、`dirs`（加进来的目录，没有的写空的）和 `oneshot: true`；写了 `--model` 的再带 `model`（施工 8-10），写了 `--no-memory` 的再带 `memory: "off"`（施工 R-3 下）。回应里的 `cwd` 和敲命令时的目录不一样（目录太宽，退回账号的工作区），第一步之前说一句。解析不出的：核心照握手时的语言写的原因照原样印在标准错误上，退出码 1，什么都不造。
   2. `--continue`：`session.list`，带 `oneshot: true`、`limit: 1`，取第一个。一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
   3. `--session`：照写的。
   4. 接着说、写了 `--model` 的（施工 8-10）：找到会话以后、订阅之前发 `session.configure`，带 `session`、`model`。被拒绝的（解析不出、没有这个会话）：核心的原因照原样印在标准错误上，退出码 1，不订阅、不发话（`models.md`「施工时定的」8-10）。换成了的，这一句起就用它（下一个回合开始生效，这一句开的就是下一轮）。
6. **订阅** `subscribe`：`{"session": …, "stream": "events"}`。
7. **发** `session.send`（写了 `--timeout` 的，从这一刻算起）：`{"session": …, "text": …, "cwd": …, "dirs": […]}`：`dirs` 每次都写，没有 `--add-dir` 就是空的，所以 `--continue` 时各次照各次的。写了 `--from` 的（施工 7-10）：带上 `"from": <名字>`，照写的原样，去控制字符、截短由核心做（`protocol.md` 的 `session.send` 第 6 条）；不带 `cwd`，`dirs` 只在写了 `--add-dir` 时带（2026-09-30 主会话定：会话的工作目录、加进来的目录是人的，别的 harness 发一句不该把它们换成自己的）。开新会话时 `session.create` 照常带 `cwd`、`dirs`：那个会话是它开的。有附件的，再带 `attachments`：第 4 条的 `blob.put` 回应照先后原样放进去；没有附件的不写这一格。请求的编号是 `ask-<16 位十六进制>-<序号>`：前缀每个进程随机一次（取不到随机数的，用进程号和此刻的纳秒，各写成十六进制接在一起），序号从 1 数起。被拒绝的：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。回应里有 `cwd`、和前面说过的不一样的，也说一句目录太宽，一次 `miyu ask` 至多说一次。
8. **跟着那一轮**：`turn.started` 的 `cause` 是自己发的那条命令的，就是它；之后只收这一轮的推送，照回合编号认。收到 `resync`（掉队了），重新订阅，不补看掉的那些。
   - 她正忙（施工 7-10，2026-09-30 主会话定，平常的和 `--from` 一样）：这一句不另开一轮，由在进行的那一轮下一步听到。推过来的那条 `message.user`（`cause` 是自己的命令）带着回合编号的（人的话排进了那一轮），跟那一轮；不带的（`--from` 的话），它后面第一条带回合编号的推送是哪一轮的就跟哪一轮，闲着时由它开的那一轮照上面认。从接上的那一刻起印，之前的不补。
   - 跟住的那一轮是接上的、结束时还没听到这一句（这一轮每次请求的 `seen` 都比这一句的序号小：她在最后一步，或者被打断了），不收尾，也不算第一轮结束：内核同一批接着开下一轮（打断时排着的接着发的也是），跟那一轮。听到了的，照它怎么结束收尾。
9. **收尾**：`turn.ended` 来了，她派出去的、留过言的子代理还有没报的、回报叫醒她的那一轮要来的，接着等（下面「等子代理」）；别的照下面「样子」印完，交回退出码。
10. **Ctrl+C**：有回合在进行（还没认出第一轮的也算）：第一次发 `session.interrupt`，带 `queued: "return"`，等这一轮收尾；第二次不等了，说「打断了」，退出码 3。等子代理的时候按：不等了（「等子代理」第 7 条）。写了 `--from` 的（施工 7-10，2026-09-30 主会话定）：不打断，按一次就不等了，照第 13 条。
11. **核心断开**：说「核心断开了」，退出码 1。
12. **`--timeout` 到了**：有回合在进行的（还没认出第一轮的也算）发 `session.interrupt`，带 `queued: "return"`，不等它收尾；照「等子代理」第 8 条印完，退出码 3。写了 `--from` 的不打断，照第 13 条。
13. **别的 harness 不等了**（`--from`，施工 7-10，2026-09-30 主会话定）：按 Ctrl+C、到了 `--timeout`，都只是不等了，不发 `session.interrupt`：那一轮是她的，会话是人的，别的 harness 不该打断人的会话。有回合在进行的（还没认出第一轮的也算），照收尾印用量、最后那一句，再印一行灰字 `· 不等了，她那一轮还在接着跑`、`· 等到时间了，她那一轮还在接着跑`，退出码 3。在等子代理的时候，照「等子代理」第 7、8 条。平常的 `miyu ask` 照旧打断。

### 样子：`--format text`

标准输出只有她的回答。别的都在标准错误上，叫旁白：思考、每一步、目录太宽那一句、用量、最后那一句、说为什么结束的那一句。

样本 `docs/designs/samples/cli/ask-text.txt`（标准输出和标准错误按先后交错，照终端里看到的）：

```text
· 目录太宽（~），这次在 ~/.miyu/home/admin/workspace 里干活

先读一下笔记。

→ 读取 notes.md · 3 行
→ 读取 missing.md · 出错：没有这个文件
→ 读取 ~/.gitconfig · 没做：要确认，这里没人能确认

$ ls
notes.md
todo.md

← 编辑 notes.md · 改了 1 处
-旧的一行
+新的一行

改好了。
· 输入 400 · 命中缓存 160（40%）· 输出 40
· 1 步没做：要你确认，miyu ask 里确认不了
```

| 行 | 是什么 | 在哪 | 颜色 |
|---|---|---|---|
| 1 | 目录太宽那一句：`· 目录太宽（<敲命令时的目录>）` + `，这次在 <实际的目录> 里干活`，两个目录都照家目录写成 `~/…`；只在目录太宽时有，第一步之前，一次 | 标准错误 | 灰 |
| 2、4、8、12 | 空行：一段思考、一块的前后 | 标准错误 | |
| 3 | 她的思考，边想边印，是一段 | 标准错误 | 灰 |
| 5–7 | 每一步，一步一行，做完才印，几件同时跑的照做完的先后 | 标准错误 | 见「每一步」 |
| 9–11 | 执行命令那一块：`$ 命令`，下面是她看到的输出 | 标准错误 | 原色；出错的，`$` 红 |
| 13–15 | 编辑那一块：标题，下面是改掉的、改成的 | 标准错误 | 标题见「每一步」；`-` 行红，`+` 行绿 |
| 16 | 空行：旁白和回答之间空一行，写在回答的第一段前面 | 标准错误 | |
| 17 | 她的回答，边收边打 | 标准输出 | 不上色 |
| 18 | 用量 | 标准错误 | 灰 |
| 19 | 有几步因为要确认没做：最后那一句 | 标准错误 | 灰；「没做」红 |

配置有错那一句（施工 8-2）：样本里没有它。只在握手的回应有 `config_errors` 时有，最先印，在沙盒用不了那一句前面，一次；标准错误，灰。

项目配置没信任那一句（施工 8-3）：样本里没有它。造会话、说话的回应里有 `untrusted_project` 时有，在配置有错那一句后面、沙盒用不了那一句前面（接着说的会话，说话的回应到了才说），一次；标准错误，灰。

沙盒用不了那一句（施工 5-4 下）：样本里没有它。只在握手的回应说沙盒用不了时有，在配置有错那一句后面、目录太宽那一句前面，一次；标准错误，灰，和目录太宽那一句连着、不空行。

压缩那一行（施工 6-3 下，照项目主人 2026-09-29 定的样子）：样本里没有它。内核压缩时推的 `compaction.progress`、`compaction.done` 和摘要请求的 `model.called` 印成一行旁白，标准错误，灰，前后和一步一样照「换行和空行」：

- 压缩中：`· 正在压缩上下文… 已写 3,120 字`；还没收到字的（摘要请求刚发出去，头一条进度是 0 字）只印 `· 正在压缩上下文…`，不写字数（施工 6-3 三补，和终端界面一样）。标准错误是终端的，每来一条进度回到行首、擦掉这一行重画（`\r` 加 `ESC[2K`）；不是终端的，压缩中不印。
- 压好了：`· 上下文已压缩：812.3k → 31k token`。终端里擦掉进度那一行换成它。token 数不到一千照写，一千以上写 `k`、一百万以上写 `M`，一位小数，整的不写小数；数是估算（`compaction.md` 第一条），和供应商下一次报的会差一点。
- 失败：`· 压缩失败：<原因>`，红。原因照摘要请求出错的分类说；取不出摘要的（`bad_summary`）分两种，原话说调了工具的说「摘要请求里调了工具」。被打断的不说：这一轮的收尾会说。
- 调了工具、改走隔离式（`bad_summary`，原话末尾是 `trying again without tools`，施工 6-6 下）：不是失败，灰色一行 `· 摘要请求里调了工具，改用不带工具的再压`（英文 `· The summary called a tool; compacting again without tools`），这次压缩接着来进度。
- 暂停了（`context.compaction_paused`，施工 6-6 上）：红，照 `reason` 印一行，写法见下表。不认识的 `reason`、缺了次数或序号的，印「自动压缩已暂停」那一行（见下表）。暂停着、这一次放不下的，照出错那一行印，分类说「自动压缩暂停着」。
- `--format json` 不印：脚本读事件流里的 `compaction.done`。

**上色**：标准错误是终端、`NO_COLOR` 没设或者设成空的才上色：no-color.org 的约定是设了、不是空的才不上色（施工 4-9 再补四上：原来设成空的也不上色）。灰是 `ESC[90m`，红是 `ESC[31m`，绿是 `ESC[32m`。一行分几段，换颜色时写新颜色，换回原色写 `ESC[0m`；上过色的行，行尾写 `ESC[0m`，中途退出也不会把终端留成灰的。思考一段一段写，每一段各自包在 `ESC[90m` 和 `ESC[0m` 里。标准输出从不上色。

**换行和空行**：

1. 连着的几行（每一步、目录太宽那一句）之间不空行。
2. 一段思考、一块（标题下面有东西的：执行命令的续行、输出，编辑的改动）前后各空一行：标准错误上还什么都没印过的、前面已经是空行的，前面不再空；后面的空行等下一样东西来了再写，两段挨着只空一行。标题下面没有东西的（例如命令被拒了、编辑没改成），照一行印（施工 4-11 验收时项目主人定：工具行和思考之间要空行，照 opencode）。
3. 旁白和回答之间空一行；一段思考、一块后面接着回答的，也只空这一行。
4. 回答那一行没完就来了一步、目录太宽那一句（她先说「我先看看」再调工具）：先在标准输出上换行，再印旁白。回答之后又来思考的（只有出错重试、带着半截接着说的时候），标准输出上不换行。
5. 思考那一行没完就来了别的：先换行。
6. 回答说完补一个换行；只想了没答的（例如被打断了），思考那一行也收个尾。

**每一步**：标题一行，`<符号> <显示名> <参数的值> · <结果那一句>`。执行命令、编辑这两种，标题下面还有一块，见后面两段。

| 格 | 怎么写 | 颜色 |
|---|---|---|
| 符号 | 照 `human/{zh,en}.json` 里这件工具的 `icon`；没有这一格的、没有显示名的工具，写 `⚙` | 原色 |
| 显示名 | 照 `name`，见下表；`block` 是 `command` 的不写。没有显示名的，照她写的工具名，控制字符换掉，超过 40 个字的留前面 40 个，后面加 `…` | 原色 |
| 参数的值 | 跟哪个参数照下表。只取第一行，有第二行的加 `…`；控制字符换掉；最多 80 个字。没有这个参数、不是字符串、是空的：不写这一格。执行命令的另有写法，见「执行命令那一块」 | 原色 |
| 路径参数 | `file_path`、`path`：在会话实际干活的目录里的写相对的（目录本身写 `.`），在家目录里的写 `~/…`，别的照她写的原样；按一段段目录比，不按字比；太长的留后面 80 个字，前面加 `…` | 原色 |
| 别的参数 | 太长的留前面 80 个字，后面加 `…` | 原色 |
| 结果那一句 | 前面写 ` · `。结果里记的说法（`human`），照 `human/{zh,en}.json` 的 `said` 换成字，超过 120 个字的留前面 120 个，后面加 `…`。换不成字的（没有这一句、少了字段、写法不对）当没有说法 | 灰；「出错」「没做」红 |

| 工具 | 符号 | 显示名（中文） | 显示名（英文） | 跟哪个参数 | `block` |
|---|---|---|---|---|---|
| `read` | `→` | 读取 | Read | `file_path` | |
| `glob` | `✱` | 找文件 | Find files | `pattern` | |
| `grep` | `✱` | 搜内容 | Search | `pattern` | |
| `write` | `←` | 写入 | Write | `file_path` | |
| `edit` | `←` | 编辑 | Edit | `file_path` | `edits` |
| `trash` | `←` | 删除 | Delete | `file_path` | |
| `shell` | `$` | 执行命令 | Run | `command` | `command` |
| `subagent`（以前造的会话里是 `agent`，一样） | `↗` | 派子代理 | Subagent | `description` | |

| 结果的状态 | ` · ` 后面 |
|---|---|
| `ok` | 结果那一句；没有说法的，连 ` · ` 也不写 |
| `error` | 「出错」（红），有说法的跟 `：` 和说法 |
| `denied` | 「没做」（红），有说法的跟 `：` 和说法 |
| `cancelled` | 有说法的写说法；没有的写「打断了」 |
| `skipped` | 有说法的写说法；没有的写「跳过了」 |
| 别的 | 同 `ok` |

**执行命令那一块**（`block` 是 `command`）：

1. 标题是 `$ <命令的第一行>`，整行照印，不截；命令有好几行的，第二行起一行一行印在标题下面，前面写 `> `。控制字符换成 `�`，制表符照留。
2. 结果是工具自己写的（`tool.result` 外壳里 `by` 的 `kind` 是 `tool`）、不是放到后台的：标题后面不写结果那一句；下面印结果里的字，她看到的是什么就印什么，截断时中间那一句、末尾的退出码那一句都照印（`tools/shell.md` 第 8、9 条）。几块文字照先后接起来；ANSI 的 CSI、OSC 两种控制序列整段去掉，别的控制字符换成 `�`，制表符照留；末尾没有换行的补一个。开头、末尾的空行不印，行首的缩进照留：一块前后的空行由这边管（施工 4-11 实测：`cargo test` 末尾自带一个空行，接上一块后面的空行，就空了两行）。结果里没有字、只有空行的，下面不印。
3. 不是工具写的（被拒了、没跑就被打断了、跳过了……），或者放到后台了（效果里有 `job.started`，施工 7-9：结果里的字是给她看的英文回执，不印）：标题后面照上表写 ` · <结果那一句>`，例如 `$ cargo test · 放到后台了：j1`，下面不印。
4. 状态是 `error` 的，`$` 是红的。

**编辑那一块**（`block` 是 `edits`）：

1. 标题照「每一步」写。
2. 状态是 `ok` 的，下面照先后印参数 `edits` 里的每一处：`old_string` 的每一行前面写 `-`，红的；`new_string` 的每一行前面写 `+`，绿的；两处之间一行 `…`，灰的。字末尾的换行不多出一个空行，`\r\n` 当一个换行；控制字符换成 `�`，制表符照留。
3. 读不出 `edits`，或者某一处的 `old_string`、`new_string` 不是字符串：那一处不印。一处都印不出来的，下面不印。
4. 别的状态，下面不印。

对不上她调过的哪一次的结果（掉队重订以后，前面的推送没看到）：不印，不猜。给人看的字读不出来的（资源目录找不到、文件坏了）：当没有，每一步只照工具名和状态写，符号写 `⚙`，下面不印。

**用量那一行**：`· 输入 <输入> · 命中缓存 <命中>（<命中率>%）· 输出 <输出>`

- 这一轮每次请求的用量加起来：输入 = 没命中 + 命中 + 写进缓存；命中率 = 命中 ÷ 输入，四舍五入到整数。
- 数字三位一撇：`1830` 写成 `1,830`。
- 输入是 0 的，不写命中率那一格：`· 输入 0 · 命中缓存 0 · 输出 0`。
- 供应商一次都没报用量的，这一行不印。

**最后那一句**：这一轮里有几个结果的状态是 `denied`、说法是 `core/tool-results/unattended`（内核在没人能确认时记的那一句），就印 `· <几> 步没做：要你确认，miyu ask 里确认不了`，印在用量后面。只读时要写被拦的、碰到数据根被拦的，都不算。

**说为什么结束的那一句**：照结束的原因，印在最后。

| 原因 | 那一句 | 退出码 |
|---|---|---|
| `completed` | 不印 | 0；有几步因为要确认没做的，4 |
| `interrupted` | 打断了 | 3 |
| `error`，没发出去、分类是 `no_model`（施工 8-6；以前认的是没发出去的认证失败） | 没有可用的模型：还没配。运行 miyu setup。 | 5 |
| `error`，没发出去、分类是 `cooling`（施工 8-9：候选全在冷却，`models.md`「怎么走」第五条第 6 条） | 出错了：候选都在冷却：<原话> | 5 |
| `error`，别的 | 出错了：<分类>：<原话>；原话去掉前后空白是空的，只写分类；最后一次请求没出错、一次都没请求的，是「出错了：模型出错」 | 1 |
| 别的原因 | 这一轮没走完：<原因> | 1 |

出错时看的是这一轮最后一次请求（`model.called`）：前面出错、重试以后成了的，不算出错。

### 等子代理（施工 7-9）

`miyu ask` 等到这一轮结束、这一轮和后来被叫醒的几轮派出去的、留过言的子代理都报过、被回报叫醒的几轮也结束，才收尾（`agents.md` 第十一条，`22-命令行.md` 第三节）。这期间每一轮都跟着印。印的样子 2026-09-30 项目主人定：等的那一行原地刷新，每个回报先一行「报回来了」，用量最后印一行、几轮加起来；等子代理时按 Ctrl+C 不等了，退出码 3。这是命令行这个头的画法；终端界面、网页、以后的 shell hook 各画各的，只共用协议上的数据（2026-09-30 项目主人问起时说明）。

1. **等谁**：这一次 `miyu ask` 跟着的几轮里派出去的子代理，和这几轮里留过言的子代理（效果 `job.messaged`），不管是不是这一次派的（施工 7-9 补，M7 验收自测撞见：她把人的答复留言转给上一次派的子代理，`miyu ask` 当场就退出了，那个子代理改好报回来时没人看着，只记下、没叫醒她）。后台命令不等（它可能是一直开着的服务，`agents.md` 第四条）。这一次以前派出去、这一次没留言的（例如上次按 Ctrl+C 不等了的）不等：这边看不到它们派出去的那一条。「空了告诉我」的通知不等（施工 C-6，`cross-session.md`「定的」第 4 条）：别的会话可能要做几个小时，和后台命令一样；通知记下，`miyu ask -c` 接着说时她看到。
2. **怎么数**：协议不另给，头照事件流自己数（施工 7-9 照最简单、不加协议定，`protocol.md`「`subscribe`」第 8 条）：
   - 跟着的那几轮里，工具结果的效果 `job.started`（`what` 是 `agent`）记下一个，欠一份回报；记着它的编号和标题（`title`）。
   - `child.reported` 到了（不带回合编号，哪一轮里到的都认），它就不欠了：哪一种原因都算，停掉、崩了补报的也算报过。还没记下的（这一次以前派、还没留言的），只记着它最近一份回报的序号，给它留言时用（下一款，施工 7-9 补）。
   - 效果 `job.messaged`（她给它留了言，施工 7-7）：它又欠一份。这边没见过它的 `job.started` 的（这一次以前派的），也记下一个，欠一份回报，没有标题（施工 7-9 补）。留言那次调用所在的那条回复（调用编号 `call_<序号>_<第几个>` 里的序号，`kernel/ids.md`）以后已经报过的不算：它手快，做完先报了，和内核的账本一样算回了这句留言（`kernel/history.md`）。
3. **叫醒的那一轮来不来**：回报和它叫醒的那一轮在内核里是同一批落盘、一起推送（`kernel/session.md`「回报」第 5 条），头先看到回报，再看到 `turn.started`，所以要先认出那一轮会来：
   - 没有跟着的回合在进行时到的回报，会叫醒她的（照内核的规矩：原因是 `undone`、`aborted` 的，`stopped` 又带 `by_model` 的不叫醒，别的都叫醒，不认识的原因也叫醒），等那一轮。
   - 一轮里到的、会叫醒她的回报，序号比这一轮最后一次请求的 `seen`（`model.called`，取最大的）大的，是她没听到的：这一轮不是打断结束的，内核同一批接着开下一轮（`kernel/session.md`「排队的消息」第 2 条），也等。
   - 这一次以前派出去、还没留言的，它的回报不认，也不等它叫醒的那一轮。留了言以后的认，也等它叫醒的那一轮（施工 7-9 补）。留言发出以后、结果记下以前先到的那一份（第 2 条最后一款），到的时候还不认：不印，只算回了这句留言；它是这一轮里到的，结果记下以后她下一次请求就看得到，不用另等一轮。
4. **什么时候了结**：没有跟着的回合在进行，等的子代理都不欠回报，也没有要来的一轮。了结的那一刻就收尾，之后到的推送不再看。
5. **跟哪几轮**：第一轮照「怎么走」第 8 条认；第一轮结束以后，等着的时候这个会话开的每一轮都跟（被回报叫醒的；这期间别处开的也跟，照样印）。被打断结束的一轮（这一次按的 Ctrl+C，或者别处打断的），不再等，照它收尾。
6. **印什么**（`--format text`）：
   1. 每一轮照上面「样子」印：思考、每一步、回答；用量那一行、最后那一句、说为什么结束的那一句，只在收尾时印一次。
   2. 一轮结束了还有没报的，也没有要来的一轮：标准错误上印一行灰字 `· 等 <N> 个子代理回报…（按 Ctrl+C 不等了）`，`N` 是还欠着回报的几个。标准错误是终端的，原地刷新：不换行，回到行首、擦掉这一行重画（`\r` 加 `ESC[2K`），数目变了重画；别的东西要印之前先擦掉，屏幕照画它之前的样子接着印。不是终端的，一次 `miyu ask` 只印一次，照一行旁白。要来的一轮马上开的，不印。
   3. 等的子代理报回来了：印一行灰字 `· <编号>「<标题>」报回来了`；没有标题的（这一次以前派、这一次留了言的，第 2 条）写 `· <编号> 报回来了`（施工 7-9 补）。照一行旁白。闲着时到的，这一行就在它叫醒的那一轮前面；一轮里到的，印在那一步前后。编号、标题照每一步参数的值写成一行（只取第一行，控制字符换掉，最多 80 个字）。
   4. 收尾：用量那一行是跟过的每一轮每次请求加起来的；最后那一句的几步也是几轮加起来的；说为什么结束的那一句、退出码照最后结束的那一轮（上面「说为什么结束的那一句」）。
7. **等子代理的时候按 Ctrl+C**（第一轮结束了、没有跟着的回合在进行）：不等了。终端里擦掉等的那一行，照收尾印用量、最后那一句，再印一行灰字 `· 不等了，子代理还在后台跑，下次 miyu ask -c 时她会看到结果`，退出码 3。回合在进行时按照旧（「怎么走」第 10 条）。
8. **`--timeout` 到了**：有回合在进行的叫它打断，不等它收尾；照第 7 条收好屏幕、印用量和最后那一句，再印一行灰字：还有子代理没报的 `· 等到时间了，没回报的子代理还在后台跑`，没有的 `· 等到时间了`；退出码 3。
9. **`--format json`**：等的那一行、报回来了那一行不印；`turns` 照先后放这期间结束了的每一轮（下面「样子：`--format json`」）。不等了、到时间了的，照样印那一行 JSON，列出已经结束了的几轮，再在标准错误上说为什么不等了（不上色）。
10. **头走了以后**：`miyu ask` 退出，连接断了，核心那边就没人看着这个一次性会话了，后来的回报只记下、不叫醒她，下次 `miyu ask -c` 时她一起看到（`agents.md` 第三条第 3 条；有没有头看着由会话 actor 照订阅数，`session/actor.md` 第 3 条）。
11. **等不到的**：别处在这期间撤销了派它的那一轮、或者还能恢复撤销时，回报只记下、不开轮（`kernel/session.md`「回报」第 5、6 条），这边等的那一轮不会来；掉队重订以后漏看了回报的，也数不准。按 Ctrl+C 或者用 `--timeout`。

样本 `docs/designs/samples/cli/ask-agents-text.txt`（标准错误不是终端时，标准输出和标准错误按先后交错；终端里等的那一行在「报回来了」那一行出来时擦掉了）：

```text
↗ 派子代理 查 A · 派出去了：j1
↗ 派子代理 查 B · 派出去了：j2

派出去了，等它们回来。
· 等 2 个子代理回报…（按 Ctrl+C 不等了）
· j1「查 A」报回来了

A 查完了。
· j2「查 B」报回来了

B 也查完了，都齐了。
· 输入 500 · 命中缓存 200（40%）· 输出 50
```

她先后派出去两个子代理，第一轮说完就等；j1 报回来叫醒她说了一轮，j2 再报回来叫醒她说了一轮。第一轮三次请求，叫醒的两轮各一次，用量五次加起来。

样本 `docs/designs/samples/cli/ask-agents-messaged-text.txt`（留言叫醒以前派的子代理，施工 7-9 补；标准错误不是终端时）：

```text
↗ 留言 j1 · 送到了：j1
↗ 派子代理 查 C · 派出去了：j3

转给 A 了，C 也派出去了。
· 等 2 个子代理回报…（按 Ctrl+C 不等了）
· j1 报回来了

A 改好了。
· j3「查 C」报回来了

C 也查完了。
· 输入 500 · 命中缓存 200（40%）· 输出 50
```

上一次 `miyu ask` 她派了 j1「查 A」、j2「查 B」，等的时候按了 Ctrl+C；j1 在回报里问「port 改成多少？」，那时没人看着，只记下。这一次 `miyu ask -c "port 改成 8080，再派个人查 C。"`：她留言把答复转给 j1，又派了 j3「查 C」。j1、j3 都等，j1 报回来了那一行没有标题；j2 没留言，不等。这一轮三次请求，叫醒的两轮各一次，用量五次加起来。

### 样子：`--format json`

标准输出上只有一行 JSON，这一轮结束时印；格照名字的字母先后排：

```json
{"session":"s1","turns":[{"text":"你好。","usage":{"cache_read":40,"cache_write":0,"input":100,"output":10}}]}
```

- `turns` 照先后放这期间结束了的每一轮：第一轮，等子代理时被回报叫醒的几轮（施工 7-9，「等子代理」）；没派子代理的只有一轮。
- `text` 是那一轮她说的全部回答；中间隔着步骤的两段，前一段没换行的补一个换行；没隔着步骤的照原样接上。
- `usage` 是那一轮的，`input` 是加起来的输入（没命中 + 命中 + 写进缓存）。供应商一次都没报用量的，四格都是 0。
- 最后结束的那一轮最后一次请求出错的，多一格 `"error": {"class": …, "message": …}`，排在最前面。
- 不印思考、每一步、沙盒用不了那一句、目录太宽那一句、用量那一行、最后那一句；标准错误上只印出错（被拒绝、核心断开……）和说为什么结束的那一句。
- 退出码和 `text` 一样，有几步因为要确认没做的也是 4。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 这一轮照常结束 |
| 1 | 出错：找不到数据根、连不上、被拒绝、核心断开、模型出错、这一轮没走完、一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |
| 3 | 按了两次 Ctrl+C；或者这一轮被打断了；等子代理的时候按了 Ctrl+C、到了 `--timeout`（施工 7-9）；`--from` 按了一次 Ctrl+C、到了 `--timeout`（施工 7-10） |
| 4 | 这一轮照常结束，可有几步因为要确认没做 |
| 5 | 没有可用的模型：没配，或者候选全在冷却、没发出去（施工 8-9） |

几样同时有的，照最后结束的那一轮怎么结束的算：4 只在照常结束时才有，几步没做照跟过的几轮加起来。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 配置有错（施工 8-2） | `· 配置里有 <n> 处错误：miyu config check 看是哪里` | `· <n> errors in the config: run miyu config check to see them`（1 处：`· 1 error in the config: run miyu config check to see it`） |
| 项目配置没信任（施工 8-3） | `· 这里的项目配置 <文件> 还没信任，这次没用它：miyu config trust 看一眼再定` | `· The project config at <file> is not trusted yet, so it was not used: run miyu config trust to review it` |
| 目录太宽 | `· 目录太宽（<目录>），这次在 <目录> 里干活` | `· Working directory too wide (<dir>), using <dir> this time` |
| 沙盒用不了（一行：括号里的原因，接着那半句后果） | `· 沙盒用不了（<原因>）：执行命令要你确认，miyu ask 里确认不了` | `· Sandbox unavailable (<reason>): commands need your approval, which cannot be given in miyu ask` |
| 原因：Linux 上没有手段 | 内核没有能用的 Landlock：要 Linux 5.13 起，启动参数的 lsm= 里开着 | the kernel has no usable Landlock: Linux 5.13 or later, enabled in the lsm= boot parameter |
| 原因：macOS 上没有手段 | 装不上 Seatbelt 配置，Miyu 可能跑在别的沙盒里 | the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox |
| 原因：Windows 上没有手段 | 这一版在 Windows 上还不能把命令关进沙盒 | this version cannot sandbox commands on Windows yet |
| 原因：别的系统上没有手段 | 这个系统上没有能用的沙盒 | no sandbox is available on this system |
| 原因：找不到助手 | 主程序旁边没有 miyu-sandbox：重装一次 Miyu | miyu-sandbox is missing beside the main program: reinstall Miyu |
| 原因：助手跑不起来 | miyu-sandbox 跑不起来：重装一次 Miyu | miyu-sandbox does not run: reinstall Miyu |
| 用量 | `· 输入 … · 命中缓存 …（…%）· 输出 …` | `· input … · cache hit … (…%) · output …` |
| 压缩中 | `· 正在压缩上下文… 已写 <字数> 字`；0 字时 `· 正在压缩上下文…` | `· Compacting the context… <n> characters written`；0 字时 `· Compacting the context…` |
| 压好了 | `· 上下文已压缩：<压前> → <压后> token` | `· Context compacted: <before> → <after> tokens` |
| 压缩失败 | `· 压缩失败：<原因>` | `· Compaction failed: <reason>` |
| 暂停：连续失败 | `· 自动压缩连续失败 <n> 次，已暂停：可以手动压缩、换一个模型，或者开新会话` | `· Automatic compaction failed <n> times and is paused: compact manually, switch models, or start a new session` |
| 暂停：内容太大 | `· 第 <序号> 条内容太大，压完很快又满了，自动压缩已暂停` | `· Entry <seq> is too large and keeps filling the context; automatic compaction is paused` |
| 暂停：别的 | `· 自动压缩已暂停：可以手动压缩、换一个模型，或者开新会话` | `· Automatic compaction is paused: compact manually, switch models, or start a new session` |
| 原因：摘要请求里调了工具 | 摘要请求里调了工具 | the summary called a tool |
| 最后那一句，一步 | `· 1 步没做：要你确认，miyu ask 里确认不了` | `· 1 step not done: it needs your approval, which cannot be given in miyu ask` |
| 最后那一句，几步 | `· 2 步没做：要你确认，miyu ask 里确认不了` | `· 2 steps not done: they need your approval, which cannot be given in miyu ask` |
| 一步没做成的词 | 出错、没做、打断了、跳过了 | failed、not done、interrupted、skipped |
| 没有模型（施工 8-6；施工 8-11 换成指向 `miyu setup` 的这一句） | 没有可用的模型：还没配。运行 miyu setup。 | No model is available: none is set up. Run miyu setup. |
| 没有模型、在终端里，先走 setup（施工 8-11） | 还没有模型，先接上一个。 | No model is set up yet. Let's connect one first. |
| 没有一次性会话 | 还没有 miyu ask 开过的会话 | No session opened by miyu ask yet |
| 附件传不上（施工 3-9 三补） | 附不上 <文件>：<核心说的原因> | Cannot attach <file>: <reason> |
| 打断了 | 打断了 | Interrupted |
| 核心断开 | 核心断开了 | The core went away |
| `--from` 后面是空的、只有空白（施工 7-10） | `--from 后面要写别的 harness 的名字` | `--from needs the name of the other harness` |
| 等子代理，一个（施工 7-9） | `· 等 1 个子代理回报…（按 Ctrl+C 不等了）` | `· Waiting for 1 subagent to report… (Ctrl+C stops waiting)` |
| 等子代理，几个 | `· 等 2 个子代理回报…（按 Ctrl+C 不等了）` | `· Waiting for 2 subagents to report… (Ctrl+C stops waiting)` |
| 报回来了 | `· j1「查 A」报回来了` | `· j1 “查 A” reported back` |
| 报回来了，没有标题（施工 7-9 补） | `· j5 报回来了` | `· j5 reported back` |
| 等的时候按了 Ctrl+C | `· 不等了，子代理还在后台跑，下次 miyu ask -c 时她会看到结果` | `· Stopped waiting; the subagents keep running, and she will see their results at the next miyu ask -c` |
| 到了 `--timeout`，还有子代理没报 | `· 等到时间了，没回报的子代理还在后台跑` | `· Time is up; the subagents that have not reported keep running` |
| 到了 `--timeout`，没有 | `· 等到时间了` | `· Time is up` |
| `--from` 按了 Ctrl+C，她那一轮还在进行（施工 7-10） | `· 不等了，她那一轮还在接着跑` | `· Stopped waiting; her turn keeps going` |
| `--from` 到了 `--timeout`，她那一轮还在进行 | `· 等到时间了，她那一轮还在接着跑` | `· Time is up; her turn keeps going` |
| 没走完 | 这一轮没走完：<原因> | The turn did not finish: <reason> |
| 出错 | 出错了：<分类>：<原话> | Error: <kind>: <message> |

出错的分类：

| 分类 | 中文 | 英文 |
|---|---|---|
| `retryable` | 暂时出错 | temporary error |
| `rate_limited` | 被限速了 | rate limited |
| `context_too_long` | 上下文太长 | context too long |
| `auth` | 认证失败 | authentication failed |
| `content_policy` | 被内容策略拦下了 | blocked by content policy |
| `bad_stream` | 回复的流不对 | bad stream |
| `empty_reply` | 回复是空的 | empty reply |
| `bad_summary` | 取不出摘要 | no summary in the reply |
| 别的 | 模型出错 | model error |

- 核心拒绝时说的话，照核心写的原样印（它照握手时的语言写，`protocol.md`）。
- 连上核心之前的出错（找不到数据根、拉不起核心……），照出错的原话印，不跟界面语言（`ipc.md`、`store.md`）。
- 执行命令那一块下面印的，是她看到的原样，不跟界面语言（例如末尾的 `Exit code 101`）。

**帮助页**：`-h`、`--help`、`miyu help ask` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/ask.txt`（中文）：

```text
用法：miyu ask [选项] <要说的话>

说一句话，打印她的回答。不写 -c、-s 的，每次新开一个会话。
她派了子代理的，等它们都报回来、她说完再退出。

选项：
  -c, --continue          接着上一次 miyu ask 开的会话说
  -s, --session <编号>    接着这个会话说
      --format text|json  text 给人看（默认），json 给脚本
      --add-dir <目录>    多放行一个目录，她能读能写，可以写好几次
      --file <文件>       附上一个文件，图片、PDF、文本都行，可以写好几次
      --timeout <时长>    最多等多久，到了就不等了：30s、10m、1h
      --from <名字>       别的 harness 用：写上它的名字，例如 claude-code
      --model <模型>      用这个模型说；接着的会话以后都用它
      --no-memory         不召回、也不记；只管新开的会话
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/ask.txt`（英文）：

```text
Usage: miyu ask [options] <words>

Say something and print her answer, in a new session unless -c or -s.
If she starts subagents, it waits for their reports and her answers.

Options:
  -c, --continue          Go on in the session the last miyu ask opened
  -s, --session <id>      Go on in this session
      --format text|json  text for people (default), json for scripts
      --add-dir <dir>     Let her read and write this directory too; repeatable
      --file <file>       Attach a file: image, PDF, text…; repeatable
      --timeout <time>    Stop waiting after this long: 30s, 10m, 1h
      --from <name>       For another harness: its name, e.g. claude-code
      --model <model>     Use this model; a continued session keeps it
      --no-memory         Don't recall or remember; new sessions only
  -h, --help              Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/ask/follow/tests.rs` | 思考和回答分两条通道、上色、只跟自己那一轮、`--format json`、出错和退出码、重试成了不算出错 |
| `crates/miyu-cli/src/ask/follow/tests/blocks.rs` | 一块前后的空行：最前面的不空、两块挨着只空一行、后面接回答、接思考、接用量；下面没有东西的照一行印（施工 4-11） |
| `crates/miyu-cli/src/ask/follow/tests/asides.rs` | 换行和空行；给脚本的两段回答隔开、没隔着步骤的照原样接上；目录太宽那一句只说一次；路径照会话实际干活的目录写短；沙盒用不了那一句：每种原因、最先印、只说一次、`--format json` 不印（施工 5-4 下） |
| `crates/miyu-cli/src/ask/follow/tests/compaction.rs` | 压缩那一行：终端里进度原地刷新、换成结果，管道里只有结果，失败的红、调了工具的单说，英文，`--format json` 不印（施工 6-3 下）；`follow/compacting/tests.rs` 守 token 数的写法 |
| `crates/miyu-cli/src/ask/follow/tests/unattended.rs` | 最后那一句、退出码 4、只算内核那一句 |
| `crates/miyu-cli/src/ask/follow/tests/sample.rs` | 照样本的场景喂一轮，整块屏幕和 `docs/designs/samples/cli/ask-text.txt` 逐字节一样；蓝图里的样本块由门禁和同一份比（施工 4-9 三补） |
| `crates/miyu-cli/src/ask/steps/tests.rs` | 每一步的标题：符号、显示名、参数的值、结果那一句、颜色；没有显示名的写 `⚙` |
| `crates/miyu-cli/src/ask/steps/blocks/tests.rs` | 执行命令那一块：几行的命令、工具自己写的才印、控制序列去掉、红的 `$`；编辑那一块：`-`、`+`、两处之间的 `…`、读不出的那一处不印、不是 `ok` 的不印 |
| `crates/miyu-cli/src/ask/usage/tests.rs` | 用量加法、命中率、三位一撇 |
| `crates/miyu-cli/src/ask/follow/tests/waiting.rs`（施工 7-9） | 等子代理：终端里等的那一行原地刷新、数目变了重画、别的来了先擦掉；不是终端的只印一次；报回来了那一行；用量、几步没做几轮加起来，只印一次；回报没被听到的等下一轮、不印等的那一行；不是这次派的不印不数；不等子代理的头跟完一轮就走；打断结束的不再等；不等了、到时间了的两种说法；什么时候算在等；`--format json` 列出每一轮、等的时候什么都不印、不等了也印那一行 JSON；英文 |
| `crates/miyu-cli/src/ask/follow/tests/waiting/messaged.rs`（施工 7-9 补） | 这一次以前派、这一次留了言的也等：报回来了那一行没有标题，两种语言；没留言的以前派的不印不数；那一行的编号、标题只取第一行、控制字符换掉 |
| `crates/miyu-cli/src/ask/follow/agents/tests.rs`（施工 7-9） | 数还有几个没报：后台命令不数、哪种原因的回报都算报过、不是这次派的不认；留言以后又欠、留言发出以后先到的回报算回了；闲着时到的会不会叫醒她（每种原因）；一轮里没听到的回报等下一轮、打断结束的不等、她自己停的不排进回报队；这一次以前派的，留了言才数，留言以前到的回报不算回了、留言发出以后先到的算回了（施工 7-9 补） |
| `crates/miyu-cli/tests/agents.rs`（施工 7-9） | 真的核心、出厂的工具、替身模型在主会话和子会话里各答各的：派两个子代理、先后回报，屏幕和 `docs/designs/samples/cli/ask-agents-text.txt` 逐字节一样；`--format json` 的 `turns`；报过又被留言的再等它报；后台命令不等、那一步印「放到后台了」、头走了以后它结束只记下；等的时候按 Ctrl+C 退出码 3、头走了以后的回报只记下；`--timeout` 在等的时候到了、在一轮里到了（打断那一轮），退出码 3；上一次派的这一次被留言：等它报回来、那一行没有标题、跟着印叫醒的那一轮、用量加起来，这一次派的照旧有标题，没留言的以前派的不等，屏幕和 `docs/designs/samples/cli/ask-agents-messaged-text.txt` 逐字节一样（施工 7-9 补） |
| `crates/miyu-cli/tests/from.rs`（施工 7-10） | 真的核心：`--from` 和 `-s`、`-c` 一起用往那个会话里发、都不写开新会话，记成 `harness`、带着名字，她收到的请求里是带标签的那一块；带 `from`、不带 `cwd`，写了 `--add-dir` 才带 `dirs`，会话的工作目录不变；往正忙的会话里发，平常的和 `--from` 都跟住听到它的那一轮、印它的回答、退出码 0；`--from` 按 Ctrl+C、到了 `--timeout` 退出码 3、说那一轮还在接着跑，她那一轮没被打断、照常答完 |
| `crates/miyu-cli/src/ask/follow/tests/joining.rs`（施工 7-10） | 接上正忙的那一轮：人的话带着回合编号的当场接上，`--from` 的话接下一条带回合编号的推送；接上以前的推送不印；接上的那一轮没听到就结束的（被打断的也是）接着跟下一轮、不收尾，听到了的照它怎么结束收尾；闲着时由它开的那一轮照旧认；`--from` 在她那一轮还在进行时不等了的两种说法，两种语言，在等子代理时照旧 |
| `crates/miyu-cli/src/ask/tests.rs` | 几个词用空格连起来；给人看的字照界面语言读，读不出来的当没有；加进来的目录照写的先后、去掉重复的；相对的接成绝对的，不是目录的读不成（施工 5-10 上）；`--file` 相对的接成绝对的、不查在不在，照写的先后、不去重（施工 3-9 三补）；`--timeout` 的写法，读不成的几种（施工 7-9）；`--from` 空的、只有空白的读不成，别的照原样（施工 7-10）；`--no-memory` 和 `-c`、`-s` 一起写的读不成（施工 R-3 下） |
| `crates/miyu-cli/tests/attach.rs` | 真的核心走一遍 `--file`（施工 3-9 三补）：文字在前、附件照写的先后变成块，图片的宽高、文本的媒体类型；没有的、太大的说是哪个文件、为什么，中文、英文，退出码 1，不造会话 |
| `crates/miyu-cli/tests/ask.rs` | 真的核心：开一次性会话、`--continue`、没有会话可接、被拒绝、没有模型、Ctrl+C 一次和两次；加进来的目录跟着每一次 `miyu ask`：`--continue` 不写的那一轮就没有，太宽的造会话时就被拒、不留空会话（施工 5-10 上）；`--model` 新开的照它造、接着的先换（`@池`）再说、换不成的（连同以前的挡位名）退出码 1 不发话（施工 8-10、8-8 补）；`--no-memory` 新开的会话快照里是 `off`、不写的不是（施工 R-3 下） |
| `crates/miyu-cli/tests/steps.rs` | 真的核心、真的工具走一遍：每一步、执行命令和编辑那两块、目录太宽、给脚本的只看退出码。要确认的一步是写到工作区外面（施工 5-4 上起读哪儿都不问）；执行命令经 cargo 编出来的助手在沙盒里跑 |
| `crates/miyu-cli/src/shown/tests.rs` | 原色的段不带控制序列，上过色的行尾回到原色 |
| `crates/miyu/tests/ask.rs` | 真跑主程序：没配模型的退出码 5、不造会话（施工 8-11）；核心在跑的照样连；参数不对退出码 2；`-h` 印帮助页，跟着界面语言 |
| `crates/miyu-cli/tests/setup.rs`（施工 8-11） | 说话之前先看有没有模型：不是终端的退出码 5、不造会话，终端里先走一遍 `miyu setup`、写好了往下走，有了的一句不问（`cli/setup.md`「守着它的」） |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）、第三节（`miyu ask`，`--file`）、O1 到 O3。
- `04-核心协议.md` 第九节：`session.create`、`session.send` 的回应带 `cwd`。
- `11-权限与沙盒.md` 第四节（目录太宽退回账号的工作区）、A11（没有确认界面的场所）。
- `10-自带软件.md` 第十节：路径怎么写。

### 还没有的

设计里有、还没做的（`22-命令行.md` 第三节）：

- `--persona`、`--preset`、`--tools`、`--no-memory`，`--format stream-json`。
- 管道进来的内容当附件。
- 还没配模型时，先问首次引导的那两件事。
- 这一句一时没有哪一轮听到它的，一直等到有一轮听到它：接上的那一轮被打断、这一句被退回了（`return`），或者 `--from` 的话被打断以后只记下、那个会话还能恢复撤销时只记下。按 Ctrl+C 或者用 `--timeout`（施工 7-10）。
