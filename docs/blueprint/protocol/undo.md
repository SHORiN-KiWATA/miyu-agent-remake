## 撤销、恢复的回应

### 是什么

`session.revert`（撤销）、`session.unrevert`（恢复最近一次撤销）被接受时，回应除了 `events`，还带给人看的几样：会话的工作目录、撤的是哪一轮、撤掉的几轮调过几次执行命令的工具、撤掉了几次压缩、每个文件怎样、之后又被改过的差异。这几样由核心算，头照着印（`cli/undo.md`）。

协议的其余部分见 `protocol.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/methods.rs` | 两个方法的参数；交给会话，等它的回应 |
| `crates/miyu-endpoint/src/undo.rs` | 照会话的日志写回应里给人看的几样 |
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
| `events` | 整数的数组 | 这一次产生的事件的序号，照先后：`turn.reverted`（恢复是 `turn.unreverted`）；有要改回的文件的，再加一条 `files.restored` |
| `cwd` | 字符串 | 会话的工作目录，换成了真实的位置 |
| `turns` | 整数 | 撤了（恢复了）几轮 |
| `said` | 字符串，可能没有 | 第一轮里人说的那句话的第一行不空的 |
| `commands` | 整数，只有撤销有 | 撤掉的几轮里真跑过几次执行命令的工具；可以是 `0` |
| `compactions` | 整数，只有撤销有，可能没有 | 撤掉的几轮里有几次压缩：撤掉了压缩，上下文回到了压缩前（`compaction.md` 第十一条，施工 6-9）；是 `0` 的不写这一格 |
| `files` | 数组 | 改回的每一步，照做的先后；没有要改回的是空的 |

`files` 的每一项：

| 格 | 是什么 |
|---|---|
| `path` | 改的是哪里：效果里记的真实位置 |
| `action` | 做了什么：`write` 写回一份内容；`trash` 移进回收站；`untrash` 从回收站移回原处 |
| `outcome` | 结局，见下表 |
| `error` | 出错（`failed`）时系统的原话；别的没有这一格 |
| `diff` | 之后又被改过的差异，几行字；没有的不写这一格 |
| `more` | 差异里没交出来的行数；是 0 的不写这一格 |

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

例子（撤销，第二个文件之后又被改过）：

```json
{"id":"undo-5c1e0a9b7d3f2468-3","jsonrpc":"2.0","result":{"commands":2,"cwd":"/home/me/proj","events":[14,15],"files":[{"action":"write","outcome":"restored","path":"/home/me/proj/src/a.rs"},{"action":"write","diff":["@@ -3 +3 @@","-fn main() {}","+fn main() { println!(\"hi\"); }"],"outcome":"changed","path":"/home/me/proj/src/b.rs"}],"said":"把 README 改成中文","turns":1}}
```

### 怎么走

1. 会话接受了命令、改完了文件、这一次的事件都落了盘，才有回应（`kernel/history.md`）。核心接着在阻塞线程里读这个会话的整份日志，写下面这几样。
2. **`turns`**：照 `events` 的第一条（`turn.reverted` 或 `turn.unreverted`）列的几轮，数有几轮。
3. **`said`**：那几轮里的第一轮。找到它的 `turn.started`，再找引起它的那一条（`trigger`）：是 `message.user`、`by` 是人的，照先后找第一块不空的文字块，取它第一行不空的（去掉前后空白以后），去掉前后空白。一行都不空的，没有这一格。不是人开的（内核、别的会话……）、找不到的，没有这一格。
4. **`commands`**：只有撤销有。数那几轮里每条 `message.assistant` 的工具调用，工具在核心的工具目录里访问类别是「执行命令」的才算，现在只有 `shell`（`tools/interface.md`）。只数跑过的：结果是 `ok`、`error` 的，和可能跑了一半的（跑到一半被打断的 `cancelled-running`、重启时没跑完的 `restarted`）；被拒的、没跑过的、跳过的不算。这一格是提醒「命令改的撤不回」，拿不准的宁可算上。
5. **`compactions`**：只有撤销有。数日志里 `context.compacted`，`turn` 在那几轮里的才算（施工 6-9）。是 0 的不写。
6. **`files`**：照 `events` 的第二条 `files.restored`，一步一项，照原来的先后。没有第二条的是空的。
7. **差异**：只有 `changed` 的才算。
   1. 要对照的内容：撤销时是她改完的样子（这一步照的那个 `file.changed` 效果的改后），恢复时是撤销以后的样子（改前）。恢复时改前是 `null` 的（她新建的），没有差异；这一步照的不是 `file.changed` 的（`file.trashed`），也没有。
   2. 对照的内容从账号的 blob 里取；现在的内容照 `path` 读。
   3. 两边任一边超过 1 MiB（1,048,576 字节）、不是 UTF-8、取不出来、读不了：没有差异。
   4. 统一格式的差异，上下文 3 行：每一段以 `@@ … @@` 那一行起头，接着是 ` `、`-`、`+` 开头的行。不带 `---`、`+++` 那两行；文件结尾没有换行的，也不加「没有换行」那一句。
   5. 交前 20 行，剩下的行数写进 `more`。
8. **路径**：`path` 和 `cwd` 在 Windows 上去掉 `\\?\` 这个前缀；`\\?\UNC\` 开头的照原样。
9. **`cwd`**：会话表里这个会话现在实际干活的目录（`protocol.md`「会话表」），换成真实的位置（顺着链接找到本体，换不成的照原样）：效果里的路径是真实的位置，头照它写相对的路径才对得上。
10. **写不成的**：日志读不出来的（记一条运行日志）、`events` 的第一条不是撤销或恢复的，这几样照空的交：`turns` 是 `0`、`files` 是空的，没有 `said`、`commands`、`compactions`。撤销本身已经成了，照样是接受。写的时候崩了的，也照空的交，`cwd` 是空字符串。

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

运行日志（目标 `miyu::endpoint`）：`WARN undo report not written error=…`（日志读不出来），`ERROR undo report panicked error=…`（写的时候崩了）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/tests/undo.rs` | 不写回合编号的撤最后一轮：`events`、`cwd`、`turns`、`said`、`commands`、`files`，没撤掉压缩的不写 `compactions`；恢复时不带 `commands`；之后又被改过的附差异，最多 20 行、`more`；两轮的会话只算撤掉的那一轮、`said` 只取第一行去掉空白；恢复时对照改前的；上下文 3 行、不加「没有换行」；太大的、不是文本的不附差异；工作目录是链接的写真实的位置；被打断的一轮只算跑过的命令、排在后面没派的不算；`said` 跳过开头的空行，全是空白的没有这一格 |
| `crates/miyu-endpoint/src/undo/tests.rs` | 数跑过的命令：跑过的、可能跑了一半的算，没跑过的不算；数压缩：`turn` 在撤掉的几轮里的才算，一轮里压过两次的是 2（施工 6-9） |
| `crates/miyu-endpoint/tests/revert.rs` | 撤销、恢复的 `events`；`nothing_to_unrevert`、`unknown_turn`、`nothing_to_revert` 照头的语言；`turn` 写 0 |
| `crates/miyu-session/tests/restore.rs` | 改回文件的那一半（`kernel/history.md`） |

### 出处

- `04-核心协议.md` 第九节 `session.revert`、`session.unrevert` 那几条：参数、回应的 `events`、给人看的几样、原因码。
- `01-架构.md` D1：「该显示什么」写在核心里，头只负责画。
- `10-自带软件.md` 第七节：冲突不覆盖、把差异交给人；经过 `shell` 的改动撤不回，界面上写明；改回文件的细则。
- `02-内核.md` 第六节「撤销与恢复」。

### 还没有的

- 做视图投影（M8）时，这几样挪进视图，字段只加不改（`04-核心协议.md` 第九节）；完整的差异由头经 `view.detail` 按需取（第五节）。现在一个文件最多交 20 行，取不到其余的。
- 撤掉的那几轮派出去的子代理、放到后台的命令一起停下，撤销前列出它们已经做的改动（`10-自带软件.md` 第七节，M7）。
