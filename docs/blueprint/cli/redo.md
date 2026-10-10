## `miyu redo`

### 是什么

在 shell 里重做当前会话的最后一轮：撤掉它，把你那句话再发一次，让她重新做（别家叫 regenerate）；后面写了话的，换成这句话再发（施工 4-7 再补，2026-09-30 项目主人定：核心一个命令，只重做最后一轮）。连上核心（没在跑就拉起来，施工 8-6 起一律拉起），找会话，订阅，发 `session.redo`，先照 `miyu undo` 印撤掉了哪一轮，再照 `miyu ask` 跟着新的一轮边收边打。

`miyu restore` 是恢复撤销，不是这个；`redo` 这个名字 09-29 让给了它，这一步又收回来（`cli/main.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `redo`；换上帮助页；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/redo.rs` | 参数、找数据根、连核心、握手、找会话、订阅、发、跟着新的一轮、Ctrl+C、退出码 |
| `crates/miyu-cli/src/ask/follow.rs` | 收回应：撤掉了哪一轮那几行印成旁白，工作目录照回应的换上；收推送照 `miyu ask`（`cli/ask.md`） |
| `crates/miyu-cli/src/undo/print.rs` | 撤掉了哪一轮那几行：和 `miyu undo` 共用，第一行换成重做的那一句，不印怎么恢复 |
| `crates/miyu-cli/src/language/undo.rs` | 重做的第一行 |
| `crates/miyu-cli/src/link.rs`、`rpc.rs`、`shown.rs` | 握手、发请求等回应、找最新的一次性会话、请求的编号、上色；和 `miyu ask`、`miyu undo` 共用 |
| `crates/miyu-cli/src/help/{zh,en}/redo.txt` | 帮助页（`cli/main.md`「帮助页」） |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `[words]...` | 换成的话，可以不写：不写是原样重做；写了的几个词用一个空格连起来，换掉开这一轮的那一句（照 `miyu ask`、`miyu compact` 的位置参数） |
| `-s`、`--session <编号>` | 重做这个会话的最后一轮；不写的是上一次 `miyu ask` 开的那个，就是最新的那个一次性会话 |

- 没有 `--format json`、`--add-dir`：和 `miyu compact` 一样只有 `-s`。新的一轮照会话现在的环境（工作目录、加进来的目录），不报敲命令时的目录。
- 界面语言照 `cli/main.md`，帮助页也照它。
- 用到的环境变量：`MIYU_HOME`（数据根，不设是 `~/.miyu`）、`MIYU_RESOURCES`（资源目录，开发时用）、`NO_COLOR`；拉起的核心照它自己的一套（`core.md`）。

### 怎么走

1. **找数据根**，建骨架。出错：原因写在标准错误上，退出码 1。
2. **连核心**，照 `miyu ask` 第 2 步：没在跑就拉起来（施工 8-6 起一律拉起）。连不上、拉不起：原因写在标准错误上，退出码 1。
3. **握手** `hello`：和 `miyu ask` 一样（`cli/ask.md` 第 3 步），`caps.input` 是 `false`。回应里的 `sandbox` 说用不了的，照 `miyu ask` 最先说一句。
4. **找会话**，照 `miyu undo`：写了 `--session` 的照写的，头这边不查写法；没写的 `session.list`，带 `oneshot: true`、`limit: 1`，取第一个，一个都没有：说「还没有 miyu ask 开过的会话」，退出码 1。
5. **订阅** `subscribe`：`{"session": …, "stream": "events"}`。
6. **发** `session.redo`：`{"session": …, "text": …}`。`text` 是写的几个词用一个空格连起来；一个词都没写的，不写这一格。请求的编号是 `redo-<16 位十六进制>-<序号>`，写法照 `miyu ask`。
7. **被拒绝**（最后一轮不能重做的「无法重做。」、有回合在进行、没有这个会话……）：核心照握手时的语言写的原因，照原样印在标准错误上，退出码 1。
8. **被接受**：照回应印撤掉了哪一轮那几行（下面「样子」）；会话在哪个目录里干活照回应的 `cwd`，之后每一步的路径照它写短，不说目录太宽。回应到之前，撤销、重发、新的一轮的开头一定推过来了（`protocol.md`「先见结果，后见回应」）；核心写回应里给人看的几样要读日志，这时新的一轮可能已经开口、甚至说完（施工时真跑主程序撞到）。所以回应到之前推过来的先攒着，印完那几行再照先后收它们。
9. **跟着新的一轮**：`turn.started` 的 `cause` 是自己发的那条命令的，就是它；之后照 `miyu ask` 第 7 到 10 步：只收这一轮的推送，边收边打，`resync` 重新订阅，`turn.ended` 来了印完交回退出码，不等子代理（施工 7-9：只有 `miyu ask` 等，`cli/ask.md`「等子代理」）；Ctrl+C 第一次发 `session.interrupt`（`queued: "return"`），第二次不等了；核心断开说「核心断开了」。

### 样子

照 `miyu ask`（`cli/ask.md`「样子：`--format text`」）：她的回答在标准输出上，别的是旁白，在标准错误上，同一套写法、同一套颜色、同一套空行，由同一份代码印。最前面多几行旁白，说撤掉了哪一轮：

1. 第一行照 `miyu undo` 的第一行（`cli/undo.md`「样子」），后面接「，重新做」：`· 撤销「<said>」这一轮，重新做`；没有 `said` 的（那一句只有附件）`· 撤销最后一轮，重新做`。`said` 是撤掉的那一轮原来那一句，换了话的也是。
2. 接着照 `miyu undo` 印撤掉了压缩那一句、停掉了几个任务那一句（施工 7-8）、每个文件那一行和差异、执行过命令那一句（`cli/undo.md`「样子」）：重做撤掉的也是她做过的事，改回了哪些文件、哪些没动、命令改的撤不回，照样要说。
3. 不印 `miyu undo` 最后那一行「可以用 miyu restore 恢复」：重做以后恢复不了。
4. 这几行是连着的旁白，灰；没动、出错红，差异的 `-`、`+` 行照 `miyu undo` 上色。上不上色照标准错误。

样本 `docs/designs/samples/cli/redo-text.txt`（标准输出和标准错误按先后交错，照终端里看到的）：

```text
· 撤销「把 README 的标题改成中文」这一轮，重新做
· 改回 README.md
· 这一轮执行过 1 条命令：命令改的文件撤不回
→ 读取 README.md · 3 行

改好了。
· 输入 400 · 命中缓存 160（40%）· 输出 40
```

| 行 | 是什么 | 在哪 | 颜色 |
|---|---|---|---|
| 1 | 撤掉了哪一轮，重新做 | 标准错误 | 灰 |
| 2 | 改回的文件：`miyu undo` 那一行 | 标准错误 | 灰 |
| 3 | 撤掉的那一轮执行过命令：`miyu undo` 那一句 | 标准错误 | 灰 |
| 4 | 新的一轮的每一步，路径照回应的 `cwd` 写短 | 标准错误 | 照 `miyu ask` |
| 5 | 空行：旁白和回答之间 | 标准错误 | |
| 6 | 她的回答 | 标准输出 | 不上色 |
| 7 | 用量 | 标准错误 | 灰 |

### 退出码

照 `miyu ask`：

| 码 | 什么时候 |
|---|---|
| 0 | 新的一轮照常结束 |
| 1 | 找不到数据根、建不了骨架；连不上、拉不起核心；被拒绝（无法重做、有回合在进行……）；核心断开；请求写不出去；模型出错、这一轮没走完；一个一次性会话都没有 |
| 2 | 参数不对（`cli/main.md`） |
| 3 | 按了两次 Ctrl+C；或者这一轮被打断了 |
| 4 | 这一轮照常结束，可有几步因为要确认没做 |
| 5 | 没有可用的模型：没发出去就是 `no_model`（施工 8-6） |

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 第一行 | `· 撤销「<said>」这一轮，重新做` | `· Undid the turn “<said>”, redoing it` |
| 第一行，没有 `said` | `· 撤销最后一轮，重新做` | `· Undid the last turn, redoing it` |

- 英文的引号是弯引号 `“` `”`。`said` 照 `miyu undo`：控制字符换成 `�`，超过 40 个字的留前面 40 个，后面加 `…`。
- 别的字照 `cli/undo.md`（撤掉了压缩、每个文件、执行过命令）、`cli/ask.md`（每一步、用量、说为什么结束、没有模型、打断了、核心断开）。
- 核心拒绝时说的话照核心写的原样印（`protocol.md`：`not_redoable` 是「无法重做。」「Cannot redo.」，2026-09-30 项目主人定）。

**帮助页**：`-h`、`--help`、`miyu help redo` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/redo.txt`（中文）：

```text
用法：miyu redo [选项] [话]

撤掉最后一轮，把你那句话再发一次，让她重新做。后面写了话的，换成这句再发。

选项：
  -s, --session <编号>  哪个会话；不写就是上一次 miyu ask 开的
  -h, --help            印帮助
```

样本 `crates/miyu-cli/src/help/en/redo.txt`（英文）：

```text
Usage: miyu redo [options] [words]

Undo the last turn and send what you said again, so she does it over.
Any words are sent instead of what you said.

Options:
  -s, --session <id>  Which session; default is the one the last miyu ask opened
  -h, --help          Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/redo/tests.rs` | 几个词用空格连起来、不写就没有 `text` 这一格 |
| `crates/miyu-cli/src/ask/follow/tests/redo.rs` | 回应到了先印撤掉了哪一轮：第一行、没有 `said` 的、英文、上色；回应到之前新的一轮已经说完的，先印那几行再印攒着的；被拒绝的照核心的话说；撤掉了压缩、文件、差异、执行过命令照 `miyu undo`，不印怎么恢复；工作目录照回应的换上、不说目录太宽；照样本的场景喂一轮，屏幕和 `docs/designs/samples/cli/redo-text.txt` 逐字节一样，蓝图里的样本块由门禁和同一份比 |
| `crates/miyu-cli/tests/redo.rs` | 真的核心：原样重做，她又答了一次，日志里撤掉了上一轮、原话又发了一次、退出码 0；换成写的话重做；最后一轮是清空的说「无法重做。」、退出码 1；一个会话都没有、退出码 1；`--session` 重做的是指定的那个 |
| `crates/miyu/tests/redo.rs` | 真跑主程序：`redo -h`、`--help`、`help redo` 印帮助页，跟着界面语言，不是恢复那一页；核心在跑、没配模型的，重做上一次 `miyu ask` 的那一轮，先说撤掉了哪一轮，新的一轮没有模型、退出码 5；`-s`、`--session` 重做写的那个 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |

### 出处

- `22-命令行.md` 第二节（输出的规矩、退出码）、第三节（`miyu ask`、`miyu undo`）。
- `04-核心协议.md` 第九节：`session.redo`。
- `02-内核.md` 第六节「撤销与恢复」第 7 条：重做。

### 还没有的

- 终端界面里的 `/redo`、网页里编辑上一句：M8（`13-终端界面.md`、`21-网页.md`），走同一个 `session.redo`。
- 重做更早的一轮、编辑更早的一句：不做（项目主人定只重做最后一轮）。
- 命令行上换附件（照 `miyu ask --file`）：协议收（`session.redo` 的 `attachments`，施工 4-7 再补），`miyu redo` 还不传，要用到再加。
