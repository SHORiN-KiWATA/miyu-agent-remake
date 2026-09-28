## `miyu ask`

### 是什么

在 shell 里跟她说一句话：连上核心（没在跑就拉起来），开一个一次性会话或者接着说；她的回答边收边打，她做的每一步印成一行，问完印一行用量。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `ask`；换上帮助页；拉起核心用的命令是自己加上 `core` |
| `crates/miyu-cli/src/ask.rs` | 参数、退出码、找数据根、连核心、Ctrl+C |
| `crates/miyu-cli/src/ask/talk.rs` | 握手、找会话、订阅、发、跟着那一轮 |
| `crates/miyu-cli/src/ask/follow.rs` | 收推送：回答、思考、每一步、用量、结束 |
| `crates/miyu-cli/src/ask/steps.rs` | 每一步的标题、目录太宽那一句、沙盒用不了那一句（施工 5-4 下）、最后那一句 |
| `crates/miyu-cli/src/ask/steps/blocks.rs` | 执行命令、编辑那一块下面印什么（施工 4-11） |
| `crates/miyu-cli/src/ask/usage.rs` | 用量加起来 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、请求的编号、一行怎么上色、路径怎么写短；和 `miyu undo` 共用 |
| `crates/miyu-cli/src/language.rs` | 给人看的字 |
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

- 界面语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 里第一个设了、不是空的（`cli/main.md`），`zh` 开头说中文，别的说英文。帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`MIYU_RESOURCES`（资源目录，开发时用）、`DEEPSEEK_API_KEY`、`NO_COLOR`。

### 怎么走

0. **加进来的目录**（施工 5-10 上）：`--add-dir` 的每一个，相对的照敲命令时的目录接成绝对的；不存在的、不是目录的，照「参数写错时」说（`cli/main.md`），退出码 2。照写的先后，去掉重复的。
1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**：
   1. 设了 `DEEPSEEK_API_KEY`（去掉前后空白不是空的）：连；核心没在跑就拉起来。
   2. 没设：核心在跑的照样连，它可能有 key；没在跑的不拉起，说「没有可用的模型：设环境变量 DEEPSEEK_API_KEY」，退出码 5。
   3. 连不上：原因写在标准错误上，退出码 1。
3. **握手** `hello`：`protocol` 是 `[1, 1]`；`head` 是 `{"kind": "cli", "version": <版本>}`；`locale` 是 `zh-CN` 或 `en`；`caps.input` 是 `false`；带上本机令牌。
   - `caps.input` 是 `false`：`miyu ask` 里没有确认的界面，要确认的那一步，核心当场拒绝。
   - 回应里的 `sandbox` 说用不了：执行命令都要确认，这里确认不了。第一步之前、目录太宽那一句之前说一句，照原因和这台机器的系统写（下面「给人看的字」），一次（施工 5-4 下）。
4. **找会话**：
   1. 不写：`session.create`，带 `cwd`（敲命令时的目录；读不出来的写 `.`）、`dirs`（加进来的目录，没有的写空的）和 `oneshot: true`。回应里的 `cwd` 和敲命令时的目录不一样（目录太宽，退回账号的工作区），第一步之前说一句。
   2. `--continue`：`session.list`，带 `oneshot: true`、`limit: 1`，取第一个。一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
   3. `--session`：照写的。
5. **订阅** `subscribe`：`{"session": …, "stream": "events"}`。
6. **发** `session.send`：`{"session": …, "text": …, "cwd": …, "dirs": […]}`：`dirs` 每次都写，没有 `--add-dir` 就是空的，所以 `--continue` 时各次照各次的。请求的编号是 `ask-<16 位十六进制>-<序号>`：前缀每个进程随机一次（取不到随机数的，用进程号和此刻的纳秒，各写成十六进制接在一起），序号从 1 数起。被拒绝的：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。回应里有 `cwd`、和前面说过的不一样的，也说一句目录太宽，一次 `miyu ask` 至多说一次。
7. **跟着那一轮**：`turn.started` 的 `cause` 是自己发的那条命令的，就是它；之后只收这一轮的推送，照回合编号认。收到 `resync`（掉队了），重新订阅，不补看掉的那些。
8. **收尾**：`turn.ended` 来了，照下面「样子」印完，交回退出码。
9. **Ctrl+C**：第一次发 `session.interrupt`，带 `queued: "return"`，等这一轮收尾；第二次不等了，说「打断了」，退出码 3。
10. **核心断开**：说「核心断开了」，退出码 1。

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

沙盒用不了那一句（施工 5-4 下）：样本里没有它。只在握手的回应说沙盒用不了时有，最先印，在目录太宽那一句前面，一次；标准错误，灰，和目录太宽那一句连着、不空行。

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
2. 结果是工具自己写的（`tool.result` 外壳里 `by` 的 `kind` 是 `tool`）：标题后面不写结果那一句；下面印结果里的字，她看到的是什么就印什么，截断时中间那一句、末尾的退出码那一句都照印（`tools/shell.md` 第 8、9 条）。几块文字照先后接起来；ANSI 的 CSI、OSC 两种控制序列整段去掉，别的控制字符换成 `�`，制表符照留；末尾没有换行的补一个。开头、末尾的空行不印，行首的缩进照留：一块前后的空行由这边管（施工 4-11 实测：`cargo test` 末尾自带一个空行，接上一块后面的空行，就空了两行）。结果里没有字、只有空行的，下面不印。
3. 不是工具写的（被拒了、没跑就被打断了、跳过了……）：标题后面照上表写 ` · <结果那一句>`，下面不印。
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
| `error`，没发出去、分类是认证失败 | 没有可用的模型：设环境变量 DEEPSEEK_API_KEY | 5 |
| `error`，别的 | 出错了：<分类>：<原话>；原话去掉前后空白是空的，只写分类；最后一次请求没出错、一次都没请求的，是「出错了：模型出错」 | 1 |
| 别的原因 | 这一轮没走完：<原因> | 1 |

出错时看的是这一轮最后一次请求（`model.called`）：前面出错、重试以后成了的，不算出错。

### 样子：`--format json`

标准输出上只有一行 JSON，这一轮结束时印；格照名字的字母先后排：

```json
{"session":"s1","turns":[{"text":"你好。","usage":{"cache_read":40,"cache_write":0,"input":100,"output":10}}]}
```

- `text` 是这一轮她说的全部回答；中间隔着步骤的两段，前一段没换行的补一个换行；没隔着步骤的照原样接上。
- `usage` 的 `input` 是加起来的输入（没命中 + 命中 + 写进缓存）。供应商一次都没报用量的，四格都是 0。
- 这一轮最后一次请求出错的，多一格 `"error": {"class": …, "message": …}`，排在最前面。
- 不印思考、每一步、沙盒用不了那一句、目录太宽那一句、用量那一行、最后那一句；标准错误上只印出错（被拒绝、核心断开……）和说为什么结束的那一句。
- 退出码和 `text` 一样，有几步因为要确认没做的也是 4。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 这一轮照常结束 |
| 1 | 出错：找不到数据根、连不上、被拒绝、核心断开、模型出错、这一轮没走完、一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |
| 3 | 按了两次 Ctrl+C；或者这一轮被打断了 |
| 4 | 这一轮照常结束，可有几步因为要确认没做 |
| 5 | 没有可用的模型：没设 key、核心也没在跑；或者没发出去就认证失败 |

几样同时有的，照这一轮怎么结束的算：4 只在照常结束时才有。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 目录太宽 | `· 目录太宽（<目录>），这次在 <目录> 里干活` | `· Working directory too wide (<dir>), using <dir> this time` |
| 沙盒用不了（一行：括号里的原因，接着那半句后果） | `· 沙盒用不了（<原因>）：执行命令要你确认，miyu ask 里确认不了` | `· Sandbox unavailable (<reason>): commands need your approval, which cannot be given in miyu ask` |
| 原因：Linux 上没有手段 | 内核没有能用的 Landlock：要 Linux 5.13 起，启动参数的 lsm= 里开着 | the kernel has no usable Landlock: Linux 5.13 or later, enabled in the lsm= boot parameter |
| 原因：macOS 上没有手段 | 装不上 Seatbelt 配置，Miyu 可能跑在别的沙盒里 | the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox |
| 原因：Windows 上没有手段 | 这一版在 Windows 上还不能把命令关进沙盒 | this version cannot sandbox commands on Windows yet |
| 原因：别的系统上没有手段 | 这个系统上没有能用的沙盒 | no sandbox is available on this system |
| 原因：找不到助手 | 主程序旁边没有 miyu-sandbox：重装一次 Miyu | miyu-sandbox is missing beside the main program: reinstall Miyu |
| 原因：助手跑不起来 | miyu-sandbox 跑不起来：重装一次 Miyu | miyu-sandbox does not run: reinstall Miyu |
| 用量 | `· 输入 … · 命中缓存 …（…%）· 输出 …` | `· input … · cache hit … (…%) · output …` |
| 最后那一句，一步 | `· 1 步没做：要你确认，miyu ask 里确认不了` | `· 1 step not done: it needs your approval, which cannot be given in miyu ask` |
| 最后那一句，几步 | `· 2 步没做：要你确认，miyu ask 里确认不了` | `· 2 steps not done: they need your approval, which cannot be given in miyu ask` |
| 一步没做成的词 | 出错、没做、打断了、跳过了 | failed、not done、interrupted、skipped |
| 没有模型 | 没有可用的模型：设环境变量 DEEPSEEK_API_KEY | No model available: set DEEPSEEK_API_KEY |
| 没有一次性会话 | 还没有 miyu ask 开过的会话 | No session opened by miyu ask yet |
| 打断了 | 打断了 | Interrupted |
| 核心断开 | 核心断开了 | The core went away |
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
| 别的 | 模型出错 | model error |

- 核心拒绝时说的话，照核心写的原样印（它照握手时的语言写，`protocol.md`）。
- 连上核心之前的出错（找不到数据根、拉不起核心……），照出错的原话印，不跟界面语言（`ipc.md`、`store.md`）。
- 执行命令那一块下面印的，是她看到的原样，不跟界面语言（例如末尾的 `Exit code 101`）。

**帮助页**：`-h`、`--help`、`miyu help ask` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/ask.txt`（中文）：

```text
用法：miyu ask [选项] <要说的话>

说一句话，打印她的回答。不写 -c、-s 的，每次新开一个会话。

选项：
  -c, --continue          接着上一次 miyu ask 开的会话说
  -s, --session <编号>    接着这个会话说
      --format text|json  text 给人看（默认），json 给脚本
      --add-dir <目录>    多放行一个目录，她能读能写，可以写好几次
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/ask.txt`（英文）：

```text
Usage: miyu ask [options] <words>

Say something and print her answer, in a new session unless -c or -s.

Options:
  -c, --continue          Go on in the session the last miyu ask opened
  -s, --session <id>      Go on in this session
      --format text|json  text for people (default), json for scripts
      --add-dir <dir>     Let her read and write this directory too; repeatable
  -h, --help              Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/ask/follow/tests.rs` | 思考和回答分两条通道、上色、只跟自己那一轮、`--format json`、出错和退出码、重试成了不算出错 |
| `crates/miyu-cli/src/ask/follow/tests/blocks.rs` | 一块前后的空行：最前面的不空、两块挨着只空一行、后面接回答、接思考、接用量；下面没有东西的照一行印（施工 4-11） |
| `crates/miyu-cli/src/ask/follow/tests/asides.rs` | 换行和空行；给脚本的两段回答隔开、没隔着步骤的照原样接上；目录太宽那一句只说一次；路径照会话实际干活的目录写短；沙盒用不了那一句：每种原因、最先印、只说一次、`--format json` 不印（施工 5-4 下） |
| `crates/miyu-cli/src/ask/follow/tests/unattended.rs` | 最后那一句、退出码 4、只算内核那一句 |
| `crates/miyu-cli/src/ask/follow/tests/sample.rs` | 照样本的场景喂一轮，整块屏幕和 `docs/designs/samples/cli/ask-text.txt` 逐字节一样；蓝图里的样本块由门禁和同一份比（施工 4-9 三补） |
| `crates/miyu-cli/src/ask/steps/tests.rs` | 每一步的标题：符号、显示名、参数的值、结果那一句、颜色；没有显示名的写 `⚙` |
| `crates/miyu-cli/src/ask/steps/blocks/tests.rs` | 执行命令那一块：几行的命令、工具自己写的才印、控制序列去掉、红的 `$`；编辑那一块：`-`、`+`、两处之间的 `…`、读不出的那一处不印、不是 `ok` 的不印 |
| `crates/miyu-cli/src/ask/usage/tests.rs` | 用量加法、命中率、三位一撇 |
| `crates/miyu-cli/src/ask/tests.rs` | 几个词用空格连起来；给人看的字照界面语言读，读不出来的当没有；加进来的目录照写的先后、去掉重复的；相对的接成绝对的，不是目录的读不成（施工 5-10 上） |
| `crates/miyu-cli/tests/ask.rs` | 真的核心：开一次性会话、`--continue`、没有会话可接、被拒绝、没有模型、Ctrl+C 一次和两次；加进来的目录跟着每一次 `miyu ask`：`--continue` 不写的那一轮就没有，太宽的造会话时就被拒、不留空会话（施工 5-10 上） |
| `crates/miyu-cli/tests/steps.rs` | 真的核心、真的工具走一遍：每一步、执行命令和编辑那两块、目录太宽、给脚本的只看退出码。要确认的一步是写到工作区外面（施工 5-4 上起读哪儿都不问）；执行命令经 cargo 编出来的助手在沙盒里跑 |
| `crates/miyu-cli/src/shown/tests.rs` | 原色的段不带控制序列，上过色的行尾回到原色 |
| `crates/miyu/tests/ask.rs` | 真跑主程序：没有 key、核心没在跑的不拉起，退出码 5；核心在跑的照样连；参数不对退出码 2；`-h` 印帮助页，跟着界面语言 |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）、第三节（`miyu ask`）、O1 到 O3。
- `04-核心协议.md` 第九节：`session.create`、`session.send` 的回应带 `cwd`。
- `11-权限与沙盒.md` 第四节（目录太宽退回账号的工作区）、A11（没有确认界面的场所）。
- `10-自带软件.md` 第十节：路径怎么写。

### 还没有的

设计里有、还没做的（`22-命令行.md` 第三节）：

- `--persona`、`--preset`、`--model`、`--file`、`--tools`、`--no-memory`、`--timeout`，`--format stream-json`。
- 管道进来的内容当附件。
- 还没配模型时，先问首次引导的那两件事。
- 等子代理回报完才退出（M7）。
