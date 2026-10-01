## `miyu recap`

### 是什么

在 shell 里要一句当前会话的回顾：在做什么、做完了什么、卡在哪（施工 3-8 四补）。连上核心（没在跑就拉起来，施工 8-6 起一律拉起），找会话，发 `session.recap`，把那一句印在标准输出上。

什么时候要是头的事（2026-10-01 项目主人定）：终端界面以后可以做成 `/recap`，也可以离开几分钟以后自动要；这条命令是在 shell 里手动要的那一种。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `recap`；换上帮助页；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/recap.rs` | 参数、找数据根、连核心、握手、找会话、发、印、退出码 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、找最新的一次性会话、请求的编号；和 `miyu ask`、`miyu undo` 共用 |
| `crates/miyu-cli/src/help/{zh,en}/recap.txt` | 帮助页（`cli/main.md`「帮助页」） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `-s`、`--session <编号>` | 回顾这个会话；不写的是上一次 `miyu ask` 开的那个，就是最新的那个一次性会话 |

- 没有 `--format json`：印出来的只有那一句，本来就好接进管道。
- 界面语言照 `cli/main.md`，帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）；拉起的核心照它自己的一套（`core.md`）。

### 怎么走

1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**，照 `miyu compact` 第 2 步：没在跑就拉起来（施工 8-6 起一律拉起）。连不上、拉不起：原因写在标准错误上，退出码 1。
3. **握手** `hello`：和 `miyu ask` 一样（`cli/ask.md` 第 3 步），`caps.input` 是 `false`。
4. **找会话**，照 `miyu undo`：写了 `--session` 的照写的，头这边不查写法；没写的 `session.list`，带 `oneshot: true`、`limit: 1`，取第一个，一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
5. **发** `session.recap`：`{"session": …}`。请求的编号是 `recap-<16 位十六进制>-<序号>`，写法照 `miyu ask`。不订阅：回顾不开回合，回应里就是那一句。
6. **回好了**：把回应的 `text` 印在标准输出上，接一个换行，退出码 0。交回的上一句（`cached`）一样印，不另说。
7. **被拒绝**（没有能回顾的、回顾没写成、有这个会话……）：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。
8. **核心断开**：说「核心断开了」，退出码 1。请求写不出去：系统的原话写在标准错误上，退出码 1。
9. Ctrl+C 照系统的规矩结束这个程序：回顾不开回合，没有要打断的；核心那边照样写完那一句。

### 样子

标准输出上只有那一句：

```text
在查 CI 为什么在 macOS 上红：临时目录在链接下面，安全打开拒绝了。还没定是换成真实路径还是只改测试，等你回答。
```

标准错误上只有出错的那一句，照核心、`miyu undo` 的原话。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 回好了 |
| 1 | 找不到数据根、建不了骨架；连不上、拉不起核心；被拒绝；核心断开；请求写不出去；一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |
| 5 | 没有可用的模型（施工 8-6 起照回顾那一次请求的 `no_model`） |

### 给人看的字

没有新的字：没有一次性会话、核心断开照 `cli/undo.md`；没有可用的模型照 `cli/ask.md`；核心拒绝时说的话照核心写的原样印（`protocol.md`，`nothing_to_recap`、`recap_failed` 两句施工 3-8 四补定）。

**帮助页**：`-h`、`--help`、`miyu help recap` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/recap.txt`（中文）：

```text
用法：miyu recap [选项]

一句话回顾会话：在做什么、做完了什么、卡在哪。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/recap.txt`（英文）：

```text
Usage: miyu recap [options]

Recap the session in a few lines: the goal, what is done, what is blocked.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/tests/recap.rs` | 真的核心：上一次 `miyu ask` 的会话回顾出来，标准输出上只有那一句、退出码 0；没有新内容再要一次还是那一句、不再请求；`--session` 回顾的是指定的那个；一个会话都没有的说清楚、退出码 1；没写成的照核心的话说、退出码 1 |
| `crates/miyu/tests/recap.rs` | 真跑主程序：`recap -h`、`--help`、`help recap` 印帮助页，跟着界面语言；核心在跑的回顾上一次 `miyu ask` 开的（没配模型的核心那一轮没回复，照核心说没有可回顾的，退出码 1）；`-s` 和 `--session` 回顾的是写的那个 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）。
- `04-核心协议.md` 第九节：`session.recap`（2026-10-01 项目主人定：回顾是核心的协议，什么时候要是头的事）。

### 还没有的

- 终端界面里的 `/recap`、离开几分钟以后自动要：头的事，随 M9 的终端界面（`13-终端界面.md`）。
