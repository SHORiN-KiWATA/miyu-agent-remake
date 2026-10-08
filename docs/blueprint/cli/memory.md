## `miyu memory`

### 是什么

在 shell 里看她记了你什么，搜一条、记一条、改错的、忘掉一条、清空（施工 R-3 再补，`memory.md`「协议」）。连上核心（没在跑就拉起来），走 `memory.*`，不另起一套。给人看的只放人要的：那句话、哪天记的、作废了没有，编号留着好改、好忘；出处、谁记的、类、放在哪一间都不印（2026-10-08 项目主人定的设计原则）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `memory`；换上帮助页 |
| `crates/miyu-cli/src/memory.rs` | 参数、找数据根、连核心、握手、照子命令发请求、退出码 |
| `crates/miyu-cli/src/memory/shown.rs` | 一条记忆印成一行，几种情形说的话照界面语言（纯的，测试里照它算） |
| `crates/miyu-cli/src/language/memory.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/memory.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出来，新的在前 | `memory.list`，带 `forgotten`、`class` |
| `search <词>` | 搜，最相关的在前 | `memory.search`，带 `query`、`forgotten` |
| `add <话>` | 记一条 | `memory.remember`，`class` 照 `--class`，不写是 `user` |
| `edit <编号> <话>` | 改一条 | `memory.update` |
| `forget <编号>` | 忘掉一条 | `memory.forget`，带 `--why` 的原因 |
| `clear session [<会话>]` | 清掉这个会话记下的 | `memory.forget`，`clear: "session"`；不写会话的是上一次 `miyu ask` 开的 |
| `clear me` | 清掉她关于你的全部记忆 | `memory.forget`，`clear: "me"` |

| 选项 | 做什么 |
|---|---|
| `--persona <编号>` | 哪个人格的记忆；不写照核心的默认人格 |
| `-s`、`--session <编号>` | 这个会话用的那一份记忆；和 `--persona` 只能写一个 |
| `--forgotten` | `list`、`search`：连作废的一起 |
| `--class user\|feedback\|episode\|reference` | `list`：只要这一类；`add`：记成这一类，不写是 `user`；别的值参数不对 |
| `--why <原因>` | `forget`：为什么忘 |
| `--format text\|json` | `list`、`search`：`json` 原样印核心回的那一串，给脚本 |

- 几个词用一个空格连起来（照 `miyu rename`）；以 `-` 开头的写在 `--` 后面。
- 清空不问（2026-10-07 项目主人定）：清空只是追加一条事件，日志里原文还在。
- 界面语言照 `cli/main.md`。用到的环境变量：`MIYU_HOME`。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu rename`（`cli/rename.md` 第 1 到 3 步）。请求的编号是 `memory-<16 位十六进制>-<序号>`。
2. **找哪一间**：写了 `--persona` 的带 `persona`，写了 `-s` 的带 `session`，都不写的都不带（核心照默认人格；没设默认人格的回 `memory_unavailable`，照原话说）。`clear session` 的会话：写了的照写的；没写的照 `-s`；都没写的照 `miyu rename` 找上一次 `miyu ask` 开的那个，一个都没有说「还没有 miyu ask 开过的会话」，退出码 1。
3. **发**：照上面的表。
4. **印**（标准输出）：
   - `list`、`search`：一条一行，`<编号>  <日期>  <正文>`，两个空格隔开；日期是记下的时刻照这台机器此刻的时区换成的那一天（`YYYY-MM-DD`）；作废的后面接 `（已作废：<原因>）`，原因是空的只接 `（已作废）`。一条都没有的：`list` 说「还没有记忆。」，`search` 说「没找到。」。
   - `add`：「记下了：<编号>」；`edit`：「改好了：<新的编号>」；`forget`：什么都不印；清空：「清掉了 <几> 条。」。
5. **被拒绝**：核心的原话照原样印在标准错误上，退出码 1（`protocol.md` 的 `memory.*`）。太长的（`memory_too_long`）例外：核心的原话是给程序看的（说的是去 `data` 里看），这里照 `data.chars`、`data.limit` 说「太长了：这一条 121 个字，一条最多 120 个字。」。核心断开、请求写不出去照 `miyu rename`。

### 样子

```text
m12  2026-10-08  用户住在上海
m8   2026-10-07  回答要短，先说结论
m3   2026-10-07  用户养了两只猫（已作废：试一下）
```

编号照最长的那个对齐，后面空两格。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了，列、搜一条都没有的也是 |
| 1 | 找不到数据根、连不上核心；被拒绝；核心断开；`clear session` 一个会话都没有 |
| 2 | 参数不对：子命令写错、`--persona` 和 `-s` 一起写、`--class` 写错、少了编号或话（`cli/main.md`） |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 一条都没有（`list`） | 还没有记忆。 | No memories yet. |
| 没找到（`search`） | 没找到。 | Nothing found. |
| 记下了 | 记下了：{id} | Remembered as {id} |
| 改好了 | 改好了：{id} | Changed; now {id} |
| 清掉了 | 清掉了 {n} 条。 | Cleared {n}. |
| 作废了 | （已作废：{why}）、（已作废） | (forgotten: {why}), (forgotten) |
| 太长了 | 太长了：这一条 {chars} 个字，一条最多 {limit} 个字。 | Too long: this one has {chars} characters, a memory takes at most {limit}. |

核心拒绝时说的话照核心写的原样印；没有一次性会话、核心断开照 `cli/undo.md`。

**帮助页**：`-h`、`--help`、`miyu help memory`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/memory.txt`（中文）：

```text
用法：miyu memory [命令] [选项]

看她记了你什么，搜、记、改、忘、清空。不写命令就是 list。

命令：
  list                    列出记下的，新的在前
  search <词>             搜记下的
  add <话>                记一条
  edit <编号> <话>        改一条，印出新的编号
  forget <编号>           忘掉一条
  clear session [<会话>]  清掉这个会话记下的；不写是上一次 miyu ask 开的
  clear me                清掉她关于你的全部记忆

选项：
      --persona <编号>    哪个人格的记忆；不写是默认人格
  -s, --session <编号>    这个会话用的那一份记忆
      --forgotten         list、search：连忘掉的一起
      --class user|feedback|episode|reference
                          list：只要这一类；add：记成这一类，不写是 user
      --why <原因>        forget：为什么忘
      --format text|json  list、search：json 给脚本
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/memory.txt`（英文）：

```text
Usage: miyu memory [command] [options]

See what she remembers about you; search, add, edit, forget, clear.
Without a command it is list.

Commands:
  list                    List memories, newest first
  search <words>          Search memories
  add <text>              Remember one thing
  edit <id> <text>        Change one; prints the new id
  forget <id>             Forget one
  clear session [<id>]    Clear what this session remembered; default is
                          the session the last miyu ask opened
  clear me                Clear everything she remembers about you

Options:
      --persona <id>      Whose memory; default is the default persona
  -s, --session <id>      The memory this session uses
      --forgotten         list, search: include forgotten ones
      --class user|feedback|episode|reference
                          list: only this class; add: save as it (default user)
      --why <reason>      forget: why
      --format text|json  list, search: json for scripts
  -h, --help              Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/memory/tests.rs` | 一行的写法：编号对齐、日期照时区换天、作废的带原因、没原因的只写作废；一条都没有的两句 |
| `crates/miyu-cli/tests/memory.rs` | 真的核心：每个子命令发什么、印什么、退出码；`--persona`、`-s` 找哪一间；`clear session` 不写会话的照上一次 `miyu ask`；没设默认人格、核心拒的照原话、退出码 1 |
| `crates/miyu/tests/memory.rs` | 真跑主程序：`memory -h`、`--help`、`help memory`、几个子命令的 `-h` 印的都是这一页，跟着界面语言；参数写错的八种退出码 2 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |

### 出处

- `17-记忆.md` 第八节（你能做什么：命令行用 `miyu memory`）、L17。
- 施工单 `R-3-命令行（再补）.md`；全套子命令、清空不问（2026-10-07 项目主人定）；只放人要的（2026-10-08 项目主人定）。

### 还没有的

- 记忆页（终端界面、网页）：M9。
- 一般知识那一层：随 O 线。
