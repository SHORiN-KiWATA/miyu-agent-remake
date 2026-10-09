## 撤销、恢复的回应

### 是什么

`session.revert`（撤销）、`session.unrevert`（恢复最近一次撤销）、`session.redo`（重做，施工 4-7 再补：撤销的那一半照撤销写）被接受时，回应除了 `events`，还带给人看的几样：会话的工作目录、撤的是哪一轮、撤掉的几轮调过几次执行命令的工具、撤掉了几次压缩、几次清空、停掉了哪几个任务（施工 7-8）、每个文件怎样、之后又被改过的差异。这几样由核心算，头照着印（`cli/undo.md`）。

协议的其余部分见 `protocol.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/methods.rs` | 三个方法的参数；交给会话，等它的回应 |
| `crates/miyu-endpoint/src/undo.rs` | 照会话的日志写回应里给人看的几样 |
| `crates/miyu-endpoint/src/undo/jobs.rs` | 停掉了哪几个任务（施工 7-8） |
| `crates/miyu-kernel/src/session/revert.rs` | 能不能撤、撤哪几轮、能不能恢复（`kernel/history.md`） |

### 对外的样子

**参数**

| 方法 | 参数 | 类型 | 说明 |
|---|---|---|---|
| `session.revert` | `session` | 字符串，必写 | 哪个会话 |
| | `turn` | 正整数，可以不写 | 从哪一轮起撤：那一轮 `turn.started` 的序号。不写的撤还在有效历史里的最后一轮；写 `0` 是 `bad_params` |
| `session.unrevert` | `session` | 字符串，必写 | 哪个会话 |

**回应**

| 格 | 类型 | 是什么 |
|---|---|---|
| `events` | 整数的数组 | 这一次产生的事件的序号，照先后：`turn.reverted`（恢复是 `turn.unreverted`）；有要改回的文件的，再加一条 `files.restored`；重做的，最后是重发的每一句 `message.user` |
| `cwd` | 字符串 | 会话的工作目录，换成了真实的位置 |
| `turns` | 整数 | 撤了（恢复了）几轮 |
| `said` | 字符串，可能没有 | 第一轮里人说的那句话的第一行不空的 |
| `commands` | 整数，只有撤销（重做）有 | 撤掉的几轮里真跑过几次执行命令的工具；可以是 `0` |
| `compactions` | 整数，只有撤销（重做）有，可能没有 | 撤掉的几轮里有几次压缩：撤掉了压缩，上下文回到了压缩前（`compaction.md` 第十一条，施工 6-9）；清空不算在里面；是 `0` 的不写这一格 |
| `clears` | 整数，只有撤销（重做）有，可能没有 | 撤掉的几轮里有几次清空：上下文回到了清空以前（`compaction.md` 第十四条，施工 6-8 补）；是 `0` 的不写这一格 |
| `jobs` | 数组，只有撤销（重做）有，可能没有 | 停掉的任务（施工 7-8）：撤掉的几轮派出去、撤销那一刻还在跑的，照编号；一项是 `job`（编号，`j1`）、`what`（`command` 或 `agent`，新版本才有的照原样）、`title`（标题），照 `job.started` 写；一个都没有的不写这一格 |
| `files` | 数组 | 改回的每一步，照做的先后；没有要改回的是空的 |

`files` 的每一项：

| 格 | 是什么 |
|---|---|
| `path` | 改的是哪里：效果里记的真实位置 |
| `action` | 做了什么：`write` 写回一份内容；`trash` 移进回收站；`untrash` 从回收站移回原处 |
| `outcome` | 结局，见下表 |
| `error` | 出错（`failed`）时系统的原话；别的没有这一格 |
| `diff` | 几行字：`changed` 的是之后又被改过的差异；`restored`、`action` 是 `write` 的是改回以前对改回以后的差异（施工 4-7 再补）。两边一样（`restored` 里本来就一样、没写的）没有差异，没有这一格；`trash`、`untrash` 也没有 |
| `more` | 差异里没交出来的行数；是 0 的不写这一格 |
| `added` | 差异里新增了几行，照整份差异数，不照截断以后的；有 `diff` 的才有（施工 4-7 再补） |
| `removed` | 差异里删掉了几行，照整份差异数，不照截断以后的；有 `diff` 的才有（施工 4-7 再补） |

| `outcome` | 意思 |
|---|---|
| `restored` | 改回了，或者现在已经是要改成的样子 |
| `changed` | 内容被改过了，不是她留下的样子：没动 |
| `missing` | 东西没了：没动 |
| `occupied` | 原处被占了：没动 |
| `gone` | 回收站里已经没有了：没动 |
| `unsaved` | 要写回的内容当时没存下来：没动 |
| `unavailable` | 回收站收不了：没动 |
| `failed` | 出错了 |

`action`、`outcome` 是新版本才有的取值的，照原样交出去。怎么核对、怎么改回见 `kernel/history.md`、`fs.md`。

例子（撤销，第一个文件改回了内容、第二个文件之后又被改过，施工 4-7 再补带了 `diff`、`added`、`removed`）：

```json
{"id":"undo-5c1e0a9b7d3f2468-3","jsonrpc":"2.0","result":{"commands":2,"cwd":"/home/me/proj","events":[14,15],"files":[{"action":"write","added":1,"diff":["@@ -1 +1 @@","-let x = 2;","+let x = 1;"],"outcome":"restored","path":"/home/me/proj/src/a.rs","removed":1},{"action":"write","added":1,"diff":["@@ -3 +3 @@","-fn main() {}","+fn main() { println!(\"hi\"); }"],"outcome":"changed","path":"/home/me/proj/src/b.rs","removed":1}],"said":"把 README 改成中文","turns":1}}
```

### 怎么走

1. 会话接受了命令、改完了文件、这一次的事件都落了盘，才有回应（`kernel/history.md`）。核心接着在阻塞线程里读这个会话的整份日志，写下面这几样。
2. **`turns`**：照 `events` 的第一条（`turn.reverted` 或 `turn.unreverted`）列的几轮，数有几轮。
3. **`said`**：那几轮里的第一轮。找到它的 `turn.started`，再找引起它的那一条（`trigger`）：是 `message.user`、`by` 是人的，照先后找第一块不空的文字块，取它第一行不空的（去掉前后空白以后），去掉前后空白。一行都不空的，没有这一格。不是人开的（内核、别的会话……）、找不到的，没有这一格。
4. **`commands`**：只有撤销有。数那几轮里每条 `message.assistant` 的工具调用，工具在核心的工具目录里访问类别是「执行命令」的才算，现在只有 `shell`（`tools/interface.md`）。只数跑过的：结果是 `ok`、`error` 的，和可能跑了一半的（跑到一半被打断的 `cancelled-running`、重启时没跑完的 `restarted`）；被拒的、没跑过的、跳过的不算。这一格是提醒「命令改的撤不回」，拿不准的宁可算上。
5. **`compactions`**、**`clears`**：只有撤销有。数日志里 `context.compacted`，`turn` 在那几轮里的才算（施工 6-9）；`trigger` 是 `clear` 的数进 `clears`，别的数进 `compactions`（施工 6-8 补，2026-09-30 项目主人定撤掉清空单说一句）。是 0 的不写。
6. **`jobs`**：只有撤销有（施工 7-8，`agents.md` 第七条第 1 条）。回应不等停掉的回报落盘，所以照日志算撤销那一刻的：`turn.reverted` 以前的日志过一遍账本，还在跑的（`running_jobs`）里挑派它的 `job.started` 在撤掉的那几轮里的，照编号；和内核交给执行器停的是同一批。撤销以前的日志过不了账本的，照空的交，记一行 `WARN undo report jobs not read`。一个都没有的不写。恢复的不写：停掉的不会再起来。
7. **`files`**：照 `events` 的第二条 `files.restored`，一步一项，照原来的先后。没有第二条的、第二条不是它的（重做没改回文件，第二条是重发的那一句）是空的。
8. **差异**：`changed` 的，和 `restored`、`action` 是 `write` 的才算（施工 4-7 再补）；`trash`、`untrash` 不算：只是挪位置，内容没变。
   1. 要对照的两边。`changed`：这一步照的那个 `file.changed` 效果（撤销时是改后、恢复时是改前）对现在磁盘上 `path` 的内容。`restored`（`action` 是 `write`）：改回以前对改回以后，两边都是那个效果的改前、改后（撤销时前者是改后、后者是改前；恢复时反过来），都从账号的 blob 取，不读磁盘——这时磁盘上已经是改回以后的样子了。改前是 `null`（新建的文件）的一边，当空的算，不是没有差异；这一步照的不是 `file.changed` 的，没有差异。
   2. 对照的内容从账号的 blob 里取；`changed` 的「现在」照 `path` 读磁盘。
   3. 两边任一边超过 1 MiB（1,048,576 字节）、不是 UTF-8、取不出来、读不了：没有差异。
   4. 两边一样：没有差异，不写 `diff`（`restored` 里「现在已经是要改成的样子」、没写的，就是这一种）。
   5. 统一格式的差异，上下文 3 行：每一段以 `@@ … @@` 那一行起头，接着是 ` `、`-`、`+` 开头的行。不带 `---`、`+++` 那两行；文件结尾没有换行的，也不加「没有换行」那一句。
   6. 交前 20 行，剩下的行数写进 `more`；`added`、`removed` 数整份差异里 `+`、`-` 开头的行，不照截断以后的。
9. **路径**：`path` 和 `cwd` 在 Windows 上去掉 `\\?\` 这个前缀；`\\?\UNC\` 开头的照原样。
10. **`cwd`**：会话表里这个会话现在实际干活的目录（`protocol.md`「会话表」），换成真实的位置（顺着链接找到本体，换不成的照原样）：效果里的路径是真实的位置，头照它写相对的路径才对得上。
11. **写不成的**：日志读不出来的（记一条运行日志）、`events` 的第一条不是撤销或恢复的，这几样照空的交：`turns` 是 `0`、`files` 是空的，没有 `said`、`commands`、`compactions`、`clears`、`jobs`。撤销本身已经成了，照样是接受。写的时候崩了的，也照空的交，`cwd` 是空字符串。

### 出错

拒绝照 `protocol.md` 的写法，原因码和它们的话也在那里：

| 原因码 | 什么时候 |
|---|---|
| `bad_params` | 参数读不成；会话编号不合写法；`turn` 写了 0 |
| `session_not_found`、`session_broken`、`session_stopped` | 找会话时（`protocol.md`「会话表」） |
| `turn_running` | 撤销时有回合在进行：先打断再撤 |
| `nothing_to_revert` | 不写 `turn` 的撤销，一轮都没有：没说过话、都撤掉了 |
| `unknown_turn` | 要撤的那一轮不在有效历史里 |
| `nothing_to_unrevert` | 恢复时没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过 |
| `restoring` | 撤销、恢复还没做完：正在读回更早的日志、正在改回文件（兜底，照常碰不到） |

有文件没动、改回时出错的，不是拒绝：照上面交在 `files` 里。

运行日志（目标 `miyu::endpoint`）：`WARN undo report not written error=…`（日志读不出来），`WARN undo report jobs not read seq=…`（撤销以前的日志过不了账本，施工 7-8），`ERROR undo report panicked error=…`（写的时候崩了）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/tests/undo.rs` | 不写回合编号的撤最后一轮：`events`、`cwd`、`turns`、`said`、`commands`、`files`，没撤掉压缩的不写 `compactions`；恢复时不带 `commands`；改回了内容的、之后又被改过的都附差异，连同 `added`、`removed`（施工 4-7 再补）；最多 20 行、`more`；两轮的会话只算撤掉的那一轮、`said` 只取第一行去掉空白；恢复时对照改前的；上下文 3 行、不加「没有换行」；太大的、不是文本的不附差异；工作目录是链接的写真实的位置；被打断的一轮只算跑过的命令、排在后面没派的不算；`said` 跳过开头的空行，全是空白的没有这一格 |
| `crates/miyu-endpoint/src/undo/tests.rs` | 数跑过的命令：跑过的、可能跑了一半的算，没跑过的不算；数压缩：`turn` 在撤掉的几轮里的才算，一轮里压过两次的是 2（施工 6-9）；清空另数，数压缩的不算它（施工 6-8 补）；改回了内容的附差异、`added`、`removed`，恢复时反过来；新建的文件改回以前当空的算；`trash`、`untrash`、两边一样的不附差异；太大的不附；长差异 `added`、`removed` 照整份算（施工 4-7 再补） |
| `crates/miyu-endpoint/tests/redo.rs` | 重做的回应：撤销那几样照撤销写，`events` 最后是重发的那一句（施工 4-7 再补，`protocol.md`「守着它的」） |
| `crates/miyu-endpoint/tests/undo_jobs.rs`（施工 7-8） | 撤销、重做的回应列出停掉的任务（编号、种类、标题），回应之前后台命令已经杀了、随后记 `undone`；恢复的不带、停过的再撤销不列 |
| `crates/miyu-endpoint/tests/revert.rs` | 撤销、恢复的 `events`；`nothing_to_unrevert`、`unknown_turn`、`nothing_to_revert` 照头的语言；`turn` 写 0 |
| `crates/miyu-session/tests/restore.rs` | 改回文件的那一半（`kernel/history.md`） |

### 出处

- `04-核心协议.md` 第九节 `session.revert`、`session.unrevert` 那几条：参数、回应的 `events`、给人看的几样、原因码。
- `01-架构.md` D1：「该显示什么」写在核心里，头只负责画。
- `10-自带软件.md` 第七节：冲突不覆盖、把差异交给人；经过 `shell` 的改动撤不回，界面上写明；改回文件的细则。
- `02-内核.md` 第六节「撤销与恢复」。

### 还没有的

- 做视图投影（M8）时，这几样挪进视图，字段只加不改（`04-核心协议.md` 第九节）；完整的差异由头经 `view.detail` 按需取（第五节）：施工 9-6 三补起照调用编号取得到一次调用的整份差异（`protocol.md`「`view.detail`」，两边同一套差异的代码，`diffs.rs`）；撤销回应里照旧一个文件最多 20 行。
- 撤销前列出撤掉的那几轮派出去的任务已经做的改动（`10-自带软件.md` 第七节）：施工 7-8 只停下它们、列出停了哪几个。
