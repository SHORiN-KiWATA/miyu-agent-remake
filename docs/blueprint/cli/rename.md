## `miyu rename`

### 是什么

在 shell 里给当前会话起名，会话列表里一眼认得出（施工 3-8 五补）。连上核心（没在跑、又设了 `DEEPSEEK_API_KEY` 的，拉起来），找会话，发 `session.set_meta`。没起名的会话核心会自己起一个（`kernel/session.md`「起标题」）；人起过名的，核心不再动。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `rename`；换上帮助页；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/rename.rs` | 参数、找数据根、连核心、握手、找会话、发、退出码 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、找最新的一次性会话、请求的编号；和 `miyu ask`、`miyu undo` 共用 |
| `crates/miyu-cli/src/help/{zh,en}/rename.txt` | 帮助页（`cli/main.md`「帮助页」） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `<标题>` | 新的标题，必写；写好几个词的，用一个空格连起来（照 `miyu compact` 的要求）。以 `-` 开头的写在 `--` 后面 |
| `-s`、`--session <编号>` | 给这个会话起名；不写的是上一次 `miyu ask` 开的那个，就是最新的那个一次性会话 |

- 只起名，不去掉标题、不置顶：那些随终端界面、会话管理的命令（`cli/main.md`「还没有的」）。
- 界面语言照 `cli/main.md`，帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`DEEPSEEK_API_KEY`；拉起的核心照它自己的一套（`core.md`）。

### 怎么走

1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**，照 `miyu undo`（`cli/undo.md`）：起名用不着模型，可设了 `DEEPSEEK_API_KEY`（去掉前后空白不是空的）的，没在跑就拉起来；没设的，核心在跑的照样连，没在跑的不拉起（拉起一个没有 key 的核心，之后设了 key 的 `miyu ask` 连上它也用不了），说 `miyu undo` 的那一句，退出码 5。连不上、拉不起：原因写在标准错误上，退出码 1。
3. **握手** `hello`：和 `miyu ask` 一样（`cli/ask.md` 第 3 步），`caps.input` 是 `false`。
4. **找会话**，照 `miyu undo`：写了 `--session` 的照写的，头这边不查写法；没写的 `session.list`，带 `oneshot: true`、`limit: 1`，取第一个，一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
5. **发** `session.set_meta`：`{"session": …, "title": <几个词用空格连起来>}`。去掉前后空白、量长短是核心的事（`protocol.md` 的 `session.set_meta`）。请求的编号是 `rename-<16 位十六进制>-<序号>`，写法照 `miyu ask`。不订阅。
6. **起好了**：什么都不印，退出码 0（照 `mv`，施工时定）。和现在的一样的也是。
7. **被拒绝**（全是空白的、超过 200 个字的、没有这个会话……）：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。
8. **核心断开**：说「核心断开了」，退出码 1。请求写不出去：系统的原话写在标准错误上，退出码 1。

### 样子

起好了什么都不印。标准错误上只有出错的那一句，照核心、`miyu undo` 的原话。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 起好了 |
| 1 | 找不到数据根、建不了骨架；连不上、拉不起核心；被拒绝；核心断开；请求写不出去；一个一次性会话都没有 |
| 2 | 参数不对，没写标题也是（`cli/main.md`） |
| 5 | 没设 key、核心也没在跑：不拉起 |

### 给人看的字

没有新的字：没有一次性会话、核心断开、没设 key 又没在跑照 `cli/undo.md`；核心拒绝时说的话照核心写的原样印（`protocol.md`）。

**帮助页**：`-h`、`--help`、`miyu help rename` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/rename.txt`（中文）：

```text
用法：miyu rename [选项] <标题>

给会话起名，会话列表里一眼认得出。标题的几个词用空格连起来。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/rename.txt`（英文）：

```text
Usage: miyu rename [options] <title>

Give the session a title, so it is easy to spot in the session list.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/tests/rename.rs` | 真的核心：给上一次 `miyu ask` 的会话起名，什么都不印、退出码 0，日志里一条人改的 `session.meta_changed`；`--session` 起名的是指定的那个，最新的那个没动；一个会话都没有的说清楚、退出码 1；全是空白的照核心的话说、退出码 1、什么都没记 |
| `crates/miyu/tests/rename.rs` | 真跑主程序：`rename -h`、`--help`、`help rename` 印帮助页，跟着界面语言；没写标题退出码 2；核心在跑的给上一次 `miyu ask` 开的起名，几个词用空格连起来、记进日志，什么都不印；`-s` 和 `--session` 起名的是写的那个（不在的照核心说没有这个会话）；没有 key、核心也没在跑的，不拉起、退出码 5、`miyu undo` 那一句 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）。
- `04-核心协议.md` 第九节：`session.set_meta`。施工单 3-8 五补：`miyu rename [-s <会话>] <标题>`，走 `session.set_meta`（2026-10-01 项目主人定，照 Claude Code 的 `/rename`）。

### 还没有的

- 去掉标题、置顶的命令：随终端界面、会话管理（`cli/main.md`「还没有的」）。
