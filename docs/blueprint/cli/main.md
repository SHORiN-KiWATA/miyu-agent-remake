## 主程序 `miyu`

### 是什么

一个程序，像 busybox 那样按子命令分发：`ask`、`undo`、`redo`、`sandbox` 是命令行的头，`core` 是核心进程。不认识的子命令就报错，绝不当成对话发给核心。给人看的话跟着界面语言。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令；换上帮助页；参数不对时交给 `misuse`；拉起核心用的命令 |
| `crates/miyu-cli/src/help.rs`、`help/{zh,en}/{miyu,ask,undo,redo,sandbox}.txt` | 帮助页：一种语言五页，编进程序（施工 4-11；`sandbox` 那一页施工 5-8） |
| `crates/miyu-cli/src/misuse.rs` | 参数写错时说的那一句，不认识的子命令也在这里（施工 4-11）；少了子命令、嵌着的子命令写错、成对的选项少了一个（施工 5-8） |
| `crates/miyu-cli/src/lib.rs` | 命令行的头对外的几样：`Ask`、`ask`、`talk`、`Format`、`Plan`、`Screen`、`Target`、`exit`，`Undo`、`undo`、`undo_on`、`Direction`、`UndoPlan`，`Sandbox`、`sandbox`，`help`、`misuse`、`language` |
| `crates/miyu-cli/src/language.rs` | 界面语言；这一页和 `miyu ask` 给人看的字 |
| `crates/miyu-cli/src/language/undo.rs` | `miyu undo`、`miyu redo` 给人看的字（`cli/undo.md`） |
| `crates/miyu-cli/src/sandbox.rs`、`sandbox/flow.rs`、`language/sandbox.rs` | `miyu sandbox setup`、`remove`（`sandbox/windows.md`，施工 5-8） |
| `crates/miyu-cli/src/ask.rs` 的 `exit` | 退出码 0、1、3、4、5；2 在 `main.rs` |

### 对外的样子

| 子命令 | 做什么 | 在哪一页 |
|---|---|---|
| `ask` | 说一句话，打印她的回答 | `cli/ask.md` |
| `undo` | 撤掉当前会话的最后一轮，把她改过的文件改回去 | `cli/undo.md` |
| `redo` | 发下一句之前，恢复最近一次撤销 | `cli/undo.md` |
| `sandbox` | `setup`、`remove`：Windows 上装好、撤掉沙盒用户，要管理员权限；别的平台上说一句不用装 | `sandbox/windows.md` |
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

1. 先照界面语言给主程序和 `ask`、`undo`、`redo`、`sandbox`（连同它的 `setup`、`remove`）换上帮助页（clap 的 `override_help`），再解析参数。
2. 解析参数，不对的：
   1. 不认识的子命令：标准错误上说「没有 <名字> 这个子命令。想和她对话，用 miyu ask "…"」，退出码 2。不连核心，不拉起，什么都不发。
   2. `-h`、`--help`、`help`、`help <子命令>`：把那一页原样印在标准输出上，退出码 0。`-V`、`--version`：印 `miyu <版本>`，退出码 0。
   3. 别的：标准错误上说一句（下面「参数写错时」），退出码 2。
3. 没写子命令：标准错误上说「终端界面还没做好。想和她对话，用 miyu ask "…"」，退出码 2。
4. `ask`、`undo`、`redo`：交给命令行的头（`cli/ask.md`、`cli/undo.md`），连同拉起核心用的命令。
5. 拉起核心用的命令：自己这个程序（`std::env::current_exe`，拿不到的用 `miyu`，照 `PATH` 找），加上 `core`。别的参数、环境变量不加；工作目录、标准输入输出、跟终端脱开，由拉起的那一边接（`ipc.md`）。
6. `core`：跑核心进程，`--idle-seconds <秒>` 是空闲多少秒退出，不写是 600（`core.md`）。
7. `sandbox setup`、`sandbox remove`：交给命令行的头（`sandbox/windows.md`）。

**帮助页**：自己写的，一种语言五页（`miyu`、`ask`、`undo`、`redo`、`sandbox`），编进程序，资源目录找不到也印得出；每页以一个换行结尾，最宽 80 列（中文字算两列）。`ask`、`undo`、`redo` 的三页见 `cli/ask.md`、`cli/undo.md`，`sandbox` 那一页见 `sandbox/windows.md`；`miyu sandbox setup -h`、`miyu sandbox remove -h` 印的也是它。`help` 子命令、`core` 不列；`miyu core --help` 照样印得出，是 clap 照代码注释生成的。

样本 `crates/miyu-cli/src/help/zh/miyu.txt`（帮助页，中文）：

```text
用法：miyu <命令> [选项]

命令：
  ask <要说的话>        说一句话，打印她的回答
  undo                  撤掉最后一轮，把她改过的文件改回去
  redo                  发下一句之前，恢复最近一次撤销
  sandbox setup|remove  装好、撤掉沙盒用户（Windows，要管理员权限）

ask 的选项：
  -c, --continue          接着上一次 miyu ask 开的会话说
  -s, --session <编号>    接着这个会话说
      --format text|json  text 给人看（默认），json 给脚本

undo、redo 的选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的

每次 miyu ask 都新开一个会话；-c、undo、redo 管的是上一次开的那个。

例子：
  miyu ask "这个项目是做什么的"
  miyu ask -c "那测试怎么跑"
  miyu undo

  -h, --help     印帮助
  -V, --version  印版本
```

样本 `crates/miyu-cli/src/help/en/miyu.txt`（帮助页，英文）：

```text
Usage: miyu <command> [options]

Commands:
  ask <words>           Say something and print her answer
  undo                  Undo the last turn and restore the files she changed
  redo                  Redo the latest undo, until you say something else
  sandbox setup|remove  Set up or remove the sandbox user (Windows, needs admin)

ask options:
  -c, --continue          Go on in the session the last miyu ask opened
  -s, --session <id>      Go on in this session
      --format text|json  text for people (default), json for scripts

undo, redo options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened

Each miyu ask opens a new session; -c, undo and redo use the last one.

Examples:
  miyu ask "what is this project"
  miyu ask -c "how do I run the tests"
  miyu undo

  -h, --help     Print help
  -V, --version  Print version
```

**参数写错时**：标准错误上只说一句，退出码 2。照 clap 报的错分：

| clap 报的 | 中文 | 英文 |
|---|---|---|
| 缺了必写的：只有 `ask` 的要说的话是必写的 | `少了要说的话：miyu ask "…"` | `Missing what to say: miyu ask "…"` |
| 成对的选项少了一个：只给提升过的自己用的 `--owner-home`、`--owner-sid`（`sandbox/windows.md`） | `少了 <选项>` | `Missing <选项>` |
| 少了子命令：`miyu sandbox` 后面没写 | `<命令> 后面要写：<子命令> 或 <子命令>` | `<命令> needs one of: <子命令> or <子命令>` |
| 嵌着的子命令写错：`miyu sandbox frob` | `<命令> 没有 <名字> 这个子命令` | `<命令> has no <名字> command` |
| 不认识的参数，`-` 开头 | `没有 <参数> 这个选项` | `No such option: <参数>` |
| 不认识的参数，别的 | `多了参数：<参数>` | `Unexpected argument: <参数>` |
| 两个选项不能一起写 | `<选项> 和 <选项> 只能写一个` | `<选项> and <选项> can't be used together` |
| 选项后面没写值 | `<选项> 后面少了值` | `<选项> needs a value` |
| 值不认识 | `<选项> 只能是 <值> 或 <值>` | `<选项> must be <值> or <值>` |
| 别的 | `参数不对：<clap 的原话>` | `Bad arguments: <clap 的原话>` |

- `<参数>` 照敲的原样；`<选项>` 照 clap 报的，是长的写法，去掉后面的值名（`--session <SESSION>` 写成 `--session`）。
- 少了子命令时，`<命令>` 是 clap 报的那一层的全名（`miyu sandbox`），`<子命令>` 是它报的能写的几个，不列 `help`（`sandbox` 关掉了 `help` 子命令）。
- 子命令写错时，看 clap 报的用法那一行（`Usage: miyu sandbox <COMMAND>`）：第一个 `<`、`[` 之前的几个词就是 `<命令>`。只有 `miyu` 一个词的，是最外面那一层写错，照「怎么走」第 2 条说。
- 能写的值两个的用「或」（`or`）连，三个以上的前面用顿号（逗号）隔开，最后一个前面用「或」（`or`）。
- clap 的原话取它报错的第一行，去掉开头的 `error: `。
- 控制字符换成 `�`：敲的参数可能混着终端的控制序列。
- 说的话、clap 报的用法里，主程序的名字一律是 `miyu`：clap 的 `bin_name` 定死了，不照可执行文件的名字（Windows 上是 `miyu.exe`，施工 5-8 查出来的）。
- 必写的不止一个了，第一种要跟着改：测试查全部子命令里必写的只有这一个。

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

- 引号是半角的 `"`，中间是省略号 `…`。
- 帮助页、参数写错时的那一句，见上面「怎么走」。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu/tests/commands.rs` | 不认识的子命令：中文、英文的那一句，退出码 2，不拉起核心、核心没起来过；只敲 `miyu`：退出码 2、说用 `miyu ask`；`-h`、`--help`、`help` 印那一页，中文、英文各和样本一样，退出码 0；`--version` 印 `miyu ` 开头；参数写错的几种，中文、英文各说那一句，退出码 2；主程序换了文件名，说的还是 `miyu`（施工 5-8） |
| `crates/miyu-cli/src/help/tests.rs` | 每一页列的选项和程序真有的一一对得上（长短写法、值名），最宽 80 列，以一个换行结尾 |
| `crates/miyu-cli/src/misuse/tests.rs` | 七种错各说哪一句、两种语言；值的连法；控制字符换掉；全部子命令里必写的只有 `ask` 的要说的话；成对的少了一个、少了子命令、嵌着的子命令写错（施工 5-8） |
| `crates/miyu/tests/ask.rs` | 参数不对退出码 2（什么都不写、`--session` 和 `--continue` 一起写）；`miyu ask --help` 跟着界面语言；没有 key、核心没在跑的不拉起 |
| `crates/miyu/tests/undo.rs` | `miyu undo --help`、`miyu redo --help` 跟着界面语言；`undo`、`redo` 各接各的 |
| `crates/miyu/tests/core.rs` | 拉起的是真的 `miyu core`（`core.md`、`ipc.md`） |

### 出处

- `12-进程形态与分发.md` 第三节、R2：一个主程序，按子命令分发；R4：对话必须显式，不认识的子命令直接报错。
- `22-命令行.md` 第二节：命令行的规矩、退出码、「没有 hello 这个子命令」那一句；第五节：命令的全表。
- `00-设计理念.md` 第六节：文字也是数据，住在代码之外（界面的字现在还写在代码里，见「还没有的」）。

### 还没有的

- 只敲 `miyu` 打开终端界面（`22-命令行.md` 第五节、`13-终端界面.md`）：现在只说还没做好。
- 会话怎么接：现在每次 `miyu ask` 开一个一次性会话，`--continue`、`undo`、`redo` 管的都是上一次 `miyu ask` 开的那个，容易让人迷惑。做头的时候和终端里的会话一起重定（2026-09-28 项目主人定）。
- 第五节表里的其余命令：`stdio`、`web`、`setup`、`status`、`doctor`、`logs`、`session`、`config`、`persona`、`preset`、`memory`、`kb`、`venue`、`listen`、`stt`、`pkg`、`tools`、`account`、`service`、`upgrade`、`shell-init`、`completions`。
- 界面语言是配置里跟着人走的一项（`14-配置.md` 第一节、`16-人格与预设.md` 第五节）；界面的字放在代码之外（`00-设计理念.md` 第六节）：现在照环境变量，字写在代码里。
