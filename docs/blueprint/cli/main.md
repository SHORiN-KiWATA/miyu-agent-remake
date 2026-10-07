## 主程序 `miyu`

### 是什么

一个程序，像 busybox 那样按子命令分发：`ask`、`undo`（别名 `rewind`）、`restore`、`redo`、`compact`、`recap`、`rename`、`config`、`login`、`logout`、`sandbox` 是命令行的头，`core` 是核心进程。不认识的子命令就报错，绝不当成对话发给核心。给人看的话跟着界面语言。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令；换上帮助页；参数不对时交给 `misuse`；拉起核心用的命令 |
| `crates/miyu-cli/src/help.rs`、`help/{zh,en}/{miyu,ask,undo,restore,redo,compact,recap,rename,sandbox,config,login,logout,setup,web}.txt` | 帮助页：一种语言十四页，编进程序（施工 4-11；`web` 那一页施工 W-9，`setup` 那一页施工 8-11，`sandbox` 那一页施工 5-8，`compact` 那一页施工 6-8，`redo` 那一页施工 4-7 再补，`recap` 那一页施工 3-8 四补，`rename` 那一页施工 3-8 五补，`config` 那一页施工 8-2，`login`、`logout` 两页施工 8-5） |
| `crates/miyu-cli/src/misuse.rs` | 参数写错时说的那一句，不认识的子命令也在这里（施工 4-11）；少了子命令、嵌着的子命令写错、成对的选项少了一个（施工 5-8） |
| `crates/miyu-cli/src/lib.rs` | 命令行的头对外的几样：`Ask`、`ask`、`talk`、`Format`、`Plan`、`Screen`、`Target`、`exit`，`Undo`、`undo`、`undo_on`、`Direction`、`UndoPlan`，`Compact`、`compact`（施工 6-8），`Recap`、`recap`、`recap_on`、`RecapPlan`（施工 3-8 四补），`Rename`、`rename`、`rename_on`、`RenamePlan`（施工 3-8 五补），`Redo`、`redo`、`redo_on`、`RedoPlan`（施工 4-7 再补），`Setup`、`setup`、`setup_on`、`SetupPlan`、`HeadEnv`、`model_ready_on`（施工 8-11），`Sandbox`、`sandbox`，`Web`、`web`、`web_on`（施工 W-9），`help`、`misuse`、`language` |
| `crates/miyu-cli/src/language.rs` | 界面语言；这一页和 `miyu ask` 给人看的字 |
| `crates/miyu-cli/src/language/undo.rs` | `miyu undo`、`miyu restore` 给人看的字（`cli/undo.md`） |
| `crates/miyu-cli/src/sandbox.rs`、`sandbox/flow.rs`、`language/sandbox.rs` | `miyu sandbox setup`、`remove`（`sandbox/windows.md`，施工 5-8） |
| `crates/miyu-cli/src/ask.rs` 的 `exit` | 退出码 0、1、3、4、5；2 在 `main.rs` |

### 对外的样子

| 子命令 | 做什么 | 在哪一页 |
|---|---|---|
| `ask` | 说一句话，打印她的回答 | `cli/ask.md` |
| `undo`（别名 `rewind`） | 撤掉当前会话的最后一轮，把她改过的文件改回去 | `cli/undo.md` |
| `restore` | 发下一句之前，恢复最近一次撤销（原来叫 `redo`，施工 4-7 补改名，2026-09-29 项目主人定） | `cli/undo.md` |
| `redo` | 撤掉当前会话的最后一轮，把你那句话（或者换成写的话）再发一次，让她重新做（施工 4-7 再补，2026-09-30 项目主人定） | `cli/redo.md` |
| `compact` | 把当前会话的上下文压缩成摘要，可以附上要求（施工 6-8，命令名 2026-09-29 项目主人定） | `cli/compact.md` |
| `recap` | 一句话回顾当前会话：在做什么、做完了什么、卡在哪（施工 3-8 四补） | `cli/recap.md` |
| `rename` | 给当前会话起名（施工 3-8 五补） | `cli/rename.md` |
| `check` | 查手写的文件有没有写错：配置、密钥文件、人格（施工 8-30） | `cli/check.md` |
| `config` | `get`、`explain`、`path`：看配置（施工 8-2，`check` 施工 8-30 挪成 `miyu check`）；`set`、`unset`、`edit`、`trust`：改配置、信任项目配置（施工 8-3） | `cli/config.md` |
| `login`、`logout` | 存、列、删供应商的 key（施工 8-5） | `cli/login.md` |
| `setup` | 接上第一个模型：找现成的 key 和本机的服务，或者搜目录、贴 key，试通了写进系统配置（施工 8-11） | `cli/setup.md` |
| `sandbox` | `setup`、`remove`：Windows 上装好、撤掉沙盒用户，要管理员权限；别的平台上说一句不用装 | `sandbox/windows.md` |
| `web` | 打开网页界面：找主程序旁边的网页软件 `miyu-web`，参数交给它；没装的说怎么装（施工 W-9） | `web-ui.md` |
| `core` | 核心进程：由头拉起，平时不用人敲；不写进帮助 | `core.md` |
| `help` | clap 自带：印帮助，`miyu help <子命令>` 印那一条的 | |

| 选项 | 做什么 |
|---|---|
| `-h`、`--help` | 印帮助 |
| `-V`、`--version` | 印 `miyu <版本>` |

**界面语言**：握手以前照系统的语言（`miyu-store` 的 `locale`）：`LC_ALL`、`LC_MESSAGES`、`LANG` 照这个先后，取第一个设了、不是空的（不是 UTF-8 的当没设），都没设的看系统设置（`sys-locale`：macOS 的首选语言、Windows 的界面语言，Linux 上它看 `LANGUAGE`，施工 8-2）；`zh` 开头的说中文，别的说英文，都没有的也是英文。握手以后，每个子命令都照回应的 `language` 说（施工 8-2，2026-10-01 主会话定）：核心照这个人的 `ui.language` 算，`auto` 的照握手时报的语言；`ja` 的照英文（命令行自己的字只有中文、英文）；老的核心没回的照握手以前的。换了语言的，`miyu ask`、`miyu redo` 给人看的字（`human/<语言>.json`）也照新的那种读一份。

| 语言 | 握手时报给核心的 `locale` | 给人看的字读哪一份 |
|---|---|---|
| 中文 | `zh-CN` | `human/zh.json` |
| 英文 | `en` | `human/en.json` |

### 怎么走

1. 先照界面语言给主程序和 `ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename`、`config`（连同它的八个子命令）、`login`、`logout`、`setup`、`sandbox`（连同它的 `setup`、`remove`）换上帮助页（clap 的 `override_help`；`rewind` 是 `undo` 的别名，用同一页），再解析参数。
2. 解析参数，不对的：
   1. 不认识的子命令：标准错误上说「没有 <名字> 这个子命令。想和她对话，用 miyu ask "…"」，退出码 2。不连核心，不拉起，什么都不发。
   2. `-h`、`--help`、`help`、`help <子命令>`：把那一页原样印在标准输出上，退出码 0。`-V`、`--version`：印 `miyu <版本>`，退出码 0。
   3. 别的：标准错误上说一句（下面「参数写错时」），退出码 2。
3. 没写子命令：标准错误上说「终端界面还没做好。想和她对话，用 miyu ask "…"」，退出码 2。
4. `ask`、`undo`（`rewind`）、`restore`、`redo`、`compact`、`recap`、`rename`、`config`、`login`、`logout`、`setup`：交给命令行的头（`cli/ask.md`、`cli/undo.md`、`cli/redo.md`、`cli/compact.md`、`cli/recap.md`、`cli/rename.md`、`cli/config.md`、`cli/login.md`、`cli/setup.md`），连同拉起核心用的命令。
5. 拉起核心用的命令：自己这个程序（`std::env::current_exe`，拿不到的用 `miyu`，照 `PATH` 找），加上 `core`。别的参数、环境变量不加；工作目录、标准输入输出、跟终端脱开，由拉起的那一边接（`ipc.md`）。
6. `core`：跑核心进程，`--idle-seconds <秒>` 是空闲多少秒退出，不写是 600（`core.md`）。
7. `sandbox setup`、`sandbox remove`：交给命令行的头（`sandbox/windows.md`）。

**帮助页**：自己写的，一种语言十三页（`miyu`、`ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename`、`sandbox`、`config`、`login`、`logout`、`setup`），编进程序，资源目录找不到也印得出；每页以一个换行结尾，最宽 80 列（中文字算两列）。`ask`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename` 的七页见 `cli/ask.md`、`cli/undo.md`、`cli/redo.md`、`cli/compact.md`、`cli/recap.md`、`cli/rename.md`，`sandbox` 那一页见 `sandbox/windows.md`，`login`、`logout` 两页见 `cli/login.md`；`miyu sandbox setup -h`、`miyu sandbox remove -h` 印的也是它。`help` 子命令、`core` 不列；`miyu core --help` 照样印得出，是 clap 照代码注释生成的。

样本 `crates/miyu-cli/src/help/zh/miyu.txt`（帮助页，中文）：

```text
用法：miyu <命令> [选项]

命令：
  ask <要说的话>        说一句话，打印她的回答
  undo、rewind          撤掉最后一轮，把她改过的文件改回去
  restore               发下一句之前，恢复最近一次撤销
  redo [话]             撤掉最后一轮，重新做；写了话的换成这句
  compact [要求]        把上下文压缩成摘要，可以附上要求
  recap                 一句话回顾：在做什么、做完了什么、卡在哪
  rename <标题>         给会话起名
  config <命令>         看配置、改配置、信任项目配置
  check [文件]          查手写的文件有没有写错：配置、密钥、人格
  login [名字]          存一个供应商的 key；--list 列出哪几家设了
  logout [名字]         删掉一个供应商的 key
  setup                 接上第一个模型：找现成的 key，试通了写进配置
  web                   打开网页界面
  sandbox setup|remove  装好、撤掉沙盒用户（Windows，要管理员权限）

ask 的选项：
  -c, --continue          接着上一次 miyu ask 开的会话说
  -s, --session <编号>    接着这个会话说
      --format text|json  text 给人看（默认），json 给脚本
      --add-dir <目录>    多放行一个目录，她能读能写，可以写好几次
      --file <文件>       附上一个文件，图片、PDF、文本都行，可以写好几次
      --timeout <时长>    最多等多久，到了就不等了：30s、10m、1h
      --from <名字>       别的 harness 用：写上它的名字，例如 claude-code
      --model <模型>      用这个模型说；接着的会话以后都用它
      --no-memory         不召回、也不记；只管新开的会话

undo、restore、redo、compact、recap、rename 的选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的

每次 miyu ask 都新开一个会话；-c、undo、restore、redo、compact、
recap、rename 管的是上一次开的那个。

例子：
  miyu ask "这个项目是做什么的"
  miyu ask -c "那测试怎么跑"
  miyu undo
  miyu compact 重点保留数据库设计的讨论

  -h, --help     印帮助
  -V, --version  印版本
```

样本 `crates/miyu-cli/src/help/en/miyu.txt`（帮助页，英文）：

```text
Usage: miyu <command> [options]

Commands:
  ask <words>           Say something and print her answer
  undo, rewind          Undo the last turn and restore the files she changed
  restore               Bring back what the latest undo took
  redo [words]          Redo the last turn, or redo it with new words
  compact [words]       Compact the context into a summary
  recap                 Recap the session: goal, progress, blockers
  rename <title>        Give the session a title
  config <command>      See and change settings, trust a project config
  check [file]          Look for mistakes in config, secrets and personas
  login [name]          Save a provider's key; --list shows which are set
  logout [name]         Delete a provider's key
  setup                 Connect the first model: find a key, try it, save it
  web                   Open the web UI
  sandbox setup|remove  Set up or remove the sandbox user (Windows, needs admin)

ask options:
  -c, --continue          Go on in the session the last miyu ask opened
  -s, --session <id>      Go on in this session
      --format text|json  text for people (default), json for scripts
      --add-dir <dir>     Let her read and write this directory too; repeatable
      --file <file>       Attach a file: image, PDF, text…; repeatable
      --timeout <time>    Stop waiting after this long: 30s, 10m, 1h
      --from <name>       For another harness: its name, e.g. claude-code
      --model <model>     Use this model; a continued session keeps it
      --no-memory         Don't recall or remember; new sessions only

undo, restore, redo, compact, recap, rename options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened

Each miyu ask opens a new session; -c, undo, restore, redo, compact,
recap and rename use the last one.

Examples:
  miyu ask "what is this project"
  miyu ask -c "how do I run the tests"
  miyu undo
  miyu compact keep the database design discussion

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
| `--add-dir` 后面不是一个已经有的目录（施工 5-10 上） | `--add-dir 后面要写一个已经有的目录：<值>` | `--add-dir needs an existing directory: <值>` |
| `--timeout` 后面不是一个时长（施工 7-9，写法见 `cli/ask.md`） | `--timeout 后面要写一个时长，例如 30s、10m、1h：<值>` | `--timeout needs a duration such as 30s, 10m or 1h: <值>` |
| `--from` 后面是空的、只有空白（施工 7-10，`cli/ask.md`） | `--from 后面要写别的 harness 的名字` | `--from needs the name of the other harness` |
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
| 3 | 被打断了（`miyu ask`、`miyu redo`、`miyu compact`）；`miyu ask` 等子代理的时候不等了、到了 `--timeout`（施工 7-9） |
| 4 | 有几步要人确认，这里确认不了，没做（`miyu ask`、`miyu redo`） |
| 5 | 没有可用的模型（`miyu ask`、`miyu redo`、`miyu compact`） |

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
| `crates/miyu-cli/src/help/tests.rs` | 每一页列的选项和程序真有的一一对得上（长短写法、值名），最宽 80 列，以一个换行结尾；值名照表换成页里的写法（`<编号>`、`<目录>`）（施工 5-10 上）、`<时长>`（施工 7-9） |
| `crates/miyu-cli/src/misuse/tests.rs` | 七种错各说哪一句、两种语言；值的连法；控制字符换掉；全部子命令里必写的只有 `ask` 的要说的话；成对的少了一个、少了子命令、嵌着的子命令写错（施工 5-8）；`--add-dir` 后面不是已经有的目录（施工 5-10 上）；`--timeout` 后面不是时长（施工 7-9）；`--from` 后面是空的、只有空白（施工 7-10） |
| `crates/miyu/tests/ask.rs` | 参数不对退出码 2（什么都不写、`--session` 和 `--continue` 一起写）；`miyu ask --help` 跟着界面语言；没有 key、核心没在跑的不拉起 |
| `crates/miyu/tests/undo.rs` | `miyu undo --help`、`miyu rewind -h`、`miyu restore --help` 跟着界面语言；`undo`、`restore` 各接各的 |
| `crates/miyu/tests/redo.rs` | `miyu redo --help`、`miyu help redo` 跟着界面语言，`redo` 印的是重做那一页、不是恢复那一页；没有 key、核心没在跑的不拉起（施工 4-7 再补） |
| `crates/miyu/tests/compact.rs` | `miyu compact --help` 跟着界面语言；没有 key、核心没在跑的不拉起（施工 6-8） |
| `crates/miyu/tests/recap.rs` | `miyu recap --help` 跟着界面语言；没有 key、核心没在跑的不拉起（施工 3-8 四补） |
| `crates/miyu/tests/rename.rs` | `miyu rename --help` 跟着界面语言；没写标题是参数不对；没有 key、核心没在跑的不拉起（施工 3-8 五补） |
| `crates/miyu/tests/config.rs` | `miyu config -h`、`miyu config get --help`、`miyu help config` 跟着界面语言；参数不对退出码 2；没有 key、核心没在跑的不拉起；握手以后照 `ui.language` 说，`miyu ask` 也是（施工 8-2，`cli/config.md`） |
| `crates/miyu/tests/login.rs` | `miyu login -h`、`miyu logout --help` 跟着界面语言；名字写错、不写名字又不在终端里、`--format` 不带 `--list` 退出码 2；没有 key、核心没在跑的不拉起（施工 8-5，`cli/login.md`） |
| `crates/miyu-cli/src/link/tests.rs` | 握手以后照回应的 `language`，`ja` 的照英文、没回的照旧；握手以前照系统的语言挑（施工 8-2） |
| `crates/miyu/tests/core.rs` | 拉起的是真的 `miyu core`（`core.md`、`ipc.md`） |

### 出处

- `12-进程形态与分发.md` 第三节、R2：一个主程序，按子命令分发；R4：对话必须显式，不认识的子命令直接报错。
- `22-命令行.md` 第二节：命令行的规矩、退出码、「没有 hello 这个子命令」那一句；第五节：命令的全表。
- `00-设计理念.md` 第六节：文字也是数据，住在代码之外（界面的字现在还写在代码里，见「还没有的」）。

### 还没有的

- 只敲 `miyu` 打开终端界面（`22-命令行.md` 第五节、`13-终端界面.md`）：现在只说还没做好。
- 会话怎么接：现在每次 `miyu ask` 开一个一次性会话，`--continue`、`undo`、`restore`、`redo`、`compact`、`recap`、`rename` 管的都是上一次 `miyu ask` 开的那个，容易让人迷惑。做头的时候和终端里的会话一起重定（2026-09-28 项目主人定）。
- 第五节表里的其余命令：`stdio`、`web`、`status`、`doctor`、`logs`、`session`、`config`、`persona`、`preset`、`memory`、`kb`、`venue`、`listen`、`stt`、`pkg`、`tools`、`account`、`service`、`upgrade`、`shell-init`、`completions`。
- 界面语言是配置里跟着人走的一项（`14-配置.md` 第一节、`16-人格与预设.md` 第五节）；界面的字放在代码之外（`00-设计理念.md` 第六节）：现在照环境变量，字写在代码里。
