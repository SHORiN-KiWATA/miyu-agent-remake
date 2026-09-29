## `miyu compact`

### 是什么

在 shell 里叫她把当前会话的上下文压缩成一份摘要，最近的一段原样留着；后面写的话是给摘要的要求（命令名 2026-09-29 项目主人定，施工 6-8）。连上核心（没在跑、又设了 `DEEPSEEK_API_KEY` 的，拉起来），找会话，发 `session.compact`，跟着那一轮，照 `miyu ask` 压缩那一行的样子印进度和结果。

参数、印什么、退出码、帮助页 2026-09-29 项目主人定（施工单 `6-8-手动压缩.md`「拍板的」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `compact`；换上帮助页；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/compact.rs` | 参数、找数据根、连核心、握手、找会话、订阅、发、跟着那一轮、Ctrl+C、退出码 |
| `crates/miyu-cli/src/ask/talk.rs` 的 `follow_turn`，`ask/follow.rs`、`ask/follow/compacting.rs` | 跟着那一轮、Ctrl+C；收推送：压缩那一行、用量那一行、说为什么结束的那一句；和 `miyu ask` 共用 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、找最新的一次性会话、请求的编号、上色；和 `miyu ask`、`miyu undo` 共用 |
| `crates/miyu-cli/src/help/{zh,en}/compact.txt` | 帮助页（`cli/main.md`「帮助页」） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `[words]...` | 给摘要的要求，可以不写；几个词用一个空格连起来 |
| `-s`、`--session <编号>` | 压这个会话；不写的是上一次 `miyu ask` 开的那个，就是最新的那个一次性会话 |

- 没有 `--format json`：不是查询类的命令（`22-命令行.md` 第二节），和 `miyu undo` 一样。
- 界面语言照 `cli/main.md`，帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`DEEPSEEK_API_KEY`、`NO_COLOR`；拉起的核心照它自己的一套（`core.md`）。

### 怎么走

1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**，照 `miyu ask` 第 2 步：压缩要请求模型。设了 `DEEPSEEK_API_KEY`（去掉前后空白不是空的），没在跑就拉起来；没设的，核心在跑的照样连，没在跑的不拉起，说「没有可用的模型：设环境变量 DEEPSEEK_API_KEY」，退出码 5。连不上、拉不起：原因写在标准错误上，退出码 1。
3. **握手** `hello`：和 `miyu ask` 一样（`cli/ask.md` 第 3 步），`caps.input` 是 `false`。
4. **找会话**，照 `miyu undo`：写了 `--session` 的照写的，头这边不查写法；没写的 `session.list`，带 `oneshot: true`、`limit: 1`，取第一个，一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
5. **订阅** `subscribe`：`{"session": …, "stream": "events"}`。
6. **发** `session.compact`：`{"session": …, "instructions": …}`。`instructions` 是写的几个词用一个空格连起来；一个词都没写的，不写这一格。请求的编号是 `compact-<16 位十六进制>-<序号>`，写法照 `miyu ask`。
7. **被拒绝**（有回合在进行、没有能压的、没有这个会话……）：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。
8. **跟着那一轮**：`turn.started` 的 `cause` 是自己发的那条命令的，就是它；之后只收这一轮的推送，照 `miyu ask` 的规矩印（`cli/ask.md` 第 7 步，下面「样子」）。收到 `resync`，重新订阅，不补看掉的那些。
9. **收尾**：`turn.ended` 来了，照下面「样子」印完，交回退出码。
10. **Ctrl+C**：第一次发 `session.interrupt`，带 `queued: "send"`，等这一轮收尾；第二次不等了，说「打断了」，退出码 3。压缩期间别的头说的话排在这一轮后面（多半是另一个终端里的 `miyu ask -c`），打断压缩不退回它们：退回了，那边就一直等不到自己那一轮。
11. **核心断开**：说「核心断开了」，退出码 1。请求写不出去：系统的原话写在标准错误上，退出码 1。

### 样子

标准输出上什么都不印；全在标准错误上，是旁白，和 `miyu ask` 压缩那一行、用量那一行、说为什么结束的那一句同一套写法、同一套颜色（`cli/ask.md`「样子」），由同一份代码印。

样本 `docs/designs/samples/cli/compact-text.txt`（压好了，中文，管道里：没有进度那一行；测试照着造出来比）：

```text
· 上下文压缩好了：812.3k → 31k token
· 输入 812,457 · 命中缓存 810,112（100%）· 输出 2,412
```

| 行 | 是什么 | 颜色 |
|---|---|---|
| 1 | 压好了：压前、压后的用量（`compaction.done` 的 `before`、`after`）。标准错误是终端的，压缩中先印 `· 正在压缩上下文… 已写 <字数> 字`，每来一条进度回到行首重画，压好了换成这一行；不是终端的，压缩中不印 | 灰 |
| 2 | 这一轮的用量：摘要请求的，重试的加起来。供应商一次都没报的不印 | 灰 |

没压成的：

| 怎么了 | 印什么 | 退出码 |
|---|---|---|
| 压缩失败 | `· 压缩失败：<原因>`（红），用量那一行，最后一行 `出错了：<分类>：<原话>` | 1 |
| 没发出去就认证失败 | `· 压缩失败：认证失败`（红），最后一行 `没有可用的模型：设环境变量 DEEPSEEK_API_KEY` | 5 |
| 被打断 | 终端里进度那一行留着，下一行 `打断了` | 3 |
| 这一轮没走完（重启、崩了） | `这一轮没走完：<原因>` | 1 |

原因、分类的说法照 `cli/ask.md`「给人看的字」的出错的分类，调了工具的单说。摘要请求里调了工具、改走隔离式的（施工 6-6 下）不是失败：照 `miyu ask` 灰色印一行 `· 摘要请求里调了工具，改用不带工具的再压`，接着压。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 压好了 |
| 1 | 找不到数据根、建不了骨架；连不上、拉不起核心；被拒绝；压缩失败、没走完；核心断开；请求写不出去；一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |
| 3 | 按了两次 Ctrl+C；或者这一轮被打断了 |
| 5 | 没有可用的模型：没设 key、核心也没在跑；或者没发出去就认证失败 |

### 给人看的字

没有新的字：压缩那几行、用量、说为什么结束的那一句照 `cli/ask.md`；没有一次性会话、核心断开照 `cli/undo.md`；核心拒绝时说的话照核心写的原样印（`protocol.md`，`nothing_to_compact`、`turn_running` 两句施工 6-8 定）。

**帮助页**：`-h`、`--help`、`miyu help compact` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/compact.txt`（中文）：

```text
用法：miyu compact [选项] [要求]

把上下文压缩成一份摘要，最近的一段原样留着。后面写的话是给摘要的要求。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/compact.txt`（英文）：

```text
Usage: miyu compact [options] [words]

Compact the context into a summary; the most recent part stays as it is.
Any words are instructions for the summary, such as what to keep.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/compact/tests.rs` | 几个词用空格连起来、不写就没有这一格；印出来的和样本 `compact-text.txt` 逐字节一样；改走隔离式的灰色那一句；失败、打断、没走完的几行和退出码；英文 |
| `crates/miyu-cli/tests/compact.rs` | 真的核心：压好了、退出码 0；刚压过再压说「没有能压的」、退出码 1；一个会话都没有、退出码 1；`--session` 压的是指定的那个 |
| `crates/miyu/tests/compact.rs` | 真跑主程序：`compact -h`、`--help`、`help compact` 印帮助页，跟着界面语言；核心在跑的压上一次 `miyu ask` 开的（说得短的没有能压的，退出码 1）；`-s` 和 `--session` 压的是写的那个；没有 key、核心也没在跑的，不拉起、退出码 5、两种语言的那一句 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）、第三节（`miyu compact`）。
- `09-压缩.md` 第二节「手动压缩」；蓝图 `compaction.md` 第七条。
- `04-核心协议.md` 第九节：`session.compact`。

### 还没有的

- 终端界面里的 `/compact`、局部压缩（选中一条消息压到那里）：M8（`13-终端界面.md`）。
