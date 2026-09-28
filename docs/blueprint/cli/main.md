## 主程序 `miyu`

### 是什么

一个程序，像 busybox 那样按子命令分发：`ask`、`undo`、`redo` 是命令行的头，`core` 是核心进程。不认识的子命令就报错，绝不当成对话发给核心。给人看的话跟着界面语言。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令；帮助换成界面语言的；参数不对怎么说；拉起核心用的命令 |
| `crates/miyu-cli/src/lib.rs` | 命令行的头对外的几样：`Ask`、`ask`、`localize`、`talk`、`Format`、`Plan`、`Screen`、`Target`、`exit`，`Undo`、`undo`、`undo_on`、`localize_undo`、`Direction`、`UndoPlan`，`language` |
| `crates/miyu-cli/src/language.rs` | 界面语言；这一页和 `miyu ask` 给人看的字 |
| `crates/miyu-cli/src/language/undo.rs` | `miyu undo`、`miyu redo` 给人看的字（`cli/undo.md`） |
| `crates/miyu-cli/src/ask.rs` 的 `exit` | 退出码 0、1、3、4、5；2 在 `main.rs` |

### 对外的样子

| 子命令 | 做什么 | 在哪一页 |
|---|---|---|
| `ask` | 说一句话，打印她的回答 | `cli/ask.md` |
| `undo` | 撤掉当前会话的最后一轮，把她改过的文件改回去 | `cli/undo.md` |
| `redo` | 发下一句之前，恢复最近一次撤销 | `cli/undo.md` |
| `core` | 核心进程：由头拉起，平时不用人敲；不写进帮助 | `core.md` |
| `help` | clap 自带：印帮助，`miyu help <子命令>` 印那一条的 | |

| 选项 | 做什么 |
|---|---|
| `-h`、`--help` | 印帮助 |
| `-V`、`--version` | 印 `miyu <版本>` |

**界面语言**：`LC_ALL`、`LC_MESSAGES`、`LANG` 照这个先后，取第一个设了、不是空的（不是 UTF-8 的当没设）；`zh` 开头的说中文，别的说英文，都没设的也是英文。

| 语言 | 握手时报给核心的 `locale` | 给人看的字读哪一份 |
|---|---|---|
| 中文 | `zh-CN` | `human/zh.json` |
| 英文 | `en` | `human/en.json` |

### 怎么走

1. 先照界面语言换掉 `ask`、`undo`、`redo` 的说明，再解析参数：`miyu --help` 的子命令列表、各自的 `--help` 里，都是换过的。
2. 解析参数，不对的：
   1. 不认识的子命令：标准错误上说「没有 <名字> 这个子命令。想和她对话，用 miyu ask "…"」，退出码 2。不连核心，不拉起，什么都不发。
   2. `-h`、`--help`、`help`、`-V`、`--version`：照 clap 印在标准输出上，退出码 0。
   3. 别的（缺了必写的、不认识的选项、`--session` 和 `--continue` 一起写、`--format` 的值不认识……）：照 clap 的写法印在标准错误上，退出码 2。
3. 没写子命令：标准错误上说「终端界面还没做好。想和她对话，用 miyu ask "…"」，退出码 2。
4. `ask`、`undo`、`redo`：交给命令行的头（`cli/ask.md`、`cli/undo.md`），连同拉起核心用的命令。
5. 拉起核心用的命令：自己这个程序（`std::env::current_exe`，拿不到的用 `miyu`，照 `PATH` 找），加上 `core`。别的参数、环境变量不加；工作目录、标准输入输出、跟终端脱开，由拉起的那一边接（`ipc.md`）。
6. `core`：跑核心进程，`--idle-seconds <秒>` 是空闲多少秒退出，不写是 600（`core.md`）。

**帮助里的字**

- `miyu --help`：没有说明那一行，第一行就是用法 `Usage: miyu [COMMAND]`（施工 4-9 再补四上：原来印代码注释「`miyu`。」）。主程序的一句说明是产品的话，等做终端界面那一步一起定。子命令列表里 `ask`、`undo`、`redo` 的说明跟着界面语言；`core` 不列。
- clap 自己的字是英文：`Usage`、`Commands`、`Options`，`help` 子命令、`-h`、`-V` 的说明，参数不对时的报错。
- `miyu core --help` 照样印得出，说明是代码里的中文注释，`--idle-seconds` 不列。

### 退出码

各条命令共用（`22-命令行.md` 第二节）：

| 码 | 什么时候 |
|---|---|
| 0 | 成功；`--help`、`--version` |
| 1 | 出错了：核心、模型、工具出了问题 |
| 2 | 用法不对：不认识的子命令、参数不对、只敲了 `miyu` |
| 3 | 被打断了（`miyu ask`） |
| 4 | 有几步要人确认，这里确认不了，没做（`miyu ask`） |
| 5 | 没有可用的模型（`miyu ask`） |

`miyu core` 的另见 `core.md`。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 不认识的子命令 | 没有 <名字> 这个子命令。想和她对话，用 miyu ask "…" | There is no <name> command. To talk to her, use miyu ask "…" |
| 只敲了 `miyu` | 终端界面还没做好。想和她对话，用 miyu ask "…" | The terminal interface is not ready yet. To talk to her, use miyu ask "…" |
| `ask` 的说明 | 说一句话，打印她的回答 | Say something and print her answer |
| `undo` 的说明 | 撤掉当前会话的最后一轮，把她改过的文件改回去 | Undo the last turn of the current session and restore the files she changed |
| `redo` 的说明 | 发下一句之前，恢复最近一次撤销 | Redo the latest undo, until you say something else |

- 引号是半角的 `"`，中间是省略号 `…`。
- `ask`、`undo`、`redo` 的参数的说明见 `cli/ask.md`、`cli/undo.md`。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu/tests/commands.rs` | 不认识的子命令：中文、英文的那一句，退出码 2，不拉起核心、核心没起来过；只敲 `miyu`：退出码 2、说用 `miyu ask`；`--help`、`--version` 退出码 0，版本印 `miyu ` 开头，`--help` 第一行是用法 |
| `crates/miyu/tests/ask.rs` | 参数不对退出码 2（什么都不写、`--session` 和 `--continue` 一起写）；`miyu ask --help` 跟着界面语言；没有 key、核心没在跑的不拉起 |
| `crates/miyu/tests/undo.rs` | `miyu undo --help`、`miyu redo --help` 跟着界面语言；`undo`、`redo` 各接各的 |
| `crates/miyu/tests/core.rs` | 拉起的是真的 `miyu core`（`core.md`、`ipc.md`） |

### 出处

- `12-进程形态与分发.md` 第三节、R2：一个主程序，按子命令分发；R4：对话必须显式，不认识的子命令直接报错。
- `22-命令行.md` 第二节：命令行的规矩、退出码、「没有 hello 这个子命令」那一句；第五节：命令的全表。
- `00-设计理念.md` 第六节：文字也是数据，住在代码之外（界面的字现在还写在代码里，见「还没有的」）。

### 还没有的

- 只敲 `miyu` 打开终端界面（`22-命令行.md` 第五节、`13-终端界面.md`）：现在只说还没做好。
- 第五节表里的其余命令：`stdio`、`web`、`setup`、`status`、`doctor`、`logs`、`session`、`config`、`persona`、`preset`、`memory`、`kb`、`venue`、`listen`、`stt`、`pkg`、`tools`、`account`、`service`、`sandbox`、`upgrade`、`shell-init`、`completions`。
- 界面语言是配置里跟着人走的一项（`14-配置.md` 第一节、`16-人格与预设.md` 第五节）；界面的字放在代码之外（`00-设计理念.md` 第六节）：现在照环境变量，字写在代码里。
