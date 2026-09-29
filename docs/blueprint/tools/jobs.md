## `jobs`

### 是什么

看她派出去的后台命令和子代理、读输出、停掉（施工 7-4，`agents.md` 第五条）。三个动作：`list` 列出这个会话派出去、还没结束的和最近结束的几个；`output` 读一条后台命令到这时的输出，或者一个子代理最近的回答和它这一步在做什么；`stop` 停掉一个，后台命令整组杀，子代理停掉它这一轮连它派的。做完了回报自己会送来，不用她来查（说明里写明）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/jobs.rs` | 参数、三个动作的结果那几句 |
| `crates/miyu-basesystem/src/jobs/page.rs` | 读输出的一页：照 `read` 分页，一页最多 30000 个字 |
| `crates/miyu-tool/src/jobs.rs` | 任务端口 `JobPort` 的 `list`、`output`、`stop`，`Listed`、`Output`、`JobError`（`tools/interface.md`） |
| `crates/miyu-session/src/jobs/roster.rs`、`query.rs`、`stop.rs`、`peek.rs` | 执行器这一头：派出去的任务照日志记着，读、停（`session/tools.md` 第 6 条） |
| `resources/software/basesystem/tools/jobs.json` | 说明和参数格式 |
| `resources/software/basesystem/jobs/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：列、读什么都不改；停掉的是她自己派出去的，不碰文件，只读开着的时候也停得了。说明照 `26-提示词.md` 附录的草稿，「做完会自己报、不用轮询」那一句留着。

样本 `resources/software/basesystem/tools/jobs.json`：

```json
{
  "description": "List your background commands and subagents, read a command's output, or stop one. Finished jobs report to you on their own, so there is no need to poll.",
  "parameters": {"type":"object","properties":{"action":{"type":"string","enum":["list","output","stop"]},"id":{"type":"string","description":"Job id, like j1."},"offset":{"type":"integer","description":"Line to start reading the output from."}},"required":["action"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `action` | 是 | `list`、`output`、`stop`；别的参数不对 |
| `id` | `output`、`stop` 要 | 任务编号，`j1` 这样；没写（空的、`"null"` 这类也算没写）参数不对；不合编号写法的照没有这个任务 |
| `offset` | 否 | 只有 `output` 看：从第几行读起，从 1 数；没写、写 0 是 1。照 `read` 的分页多了这一格：草稿只有 `action`、`id`，读不完的往下读要它（施工 7-4 定） |

- 别的参数不认，也不报错。
- 一条路径都不报：权限策略照访问类别判，读的放行。

### 怎么走

1. 读参数：读不成的（没写 `action`、不认识的动作、类型不对），交回参数不对的那一句，端口一次都不问。
2. `list`：问端口（`Call.jobs`）要列出来的，一个一行（`listed.txt`）：编号、种类（`command`、`agent`）、标题、状态、用时（毫秒）。状态是还在跑的写 `running`，结束了的写最后那条回报的 `reason`（`exited`、`stopped`、`undone`、`restarted`、`aborted`、`done`）。一个都没有、没有端口的（会话外面的调用）说一句（`none.txt`）。
   - 列哪些、用时怎么算由执行器照日志算（`session/tools.md` 第 6 条）：还在跑的全列，结束了的只列最近结束的 5 个，合起来照编号排；后台命令结束了的用时照回报里的，别的从派出去算到回报到的那一刻，还在跑的算到这一刻。
3. `output`：没写 `id` 的参数不对；不合写法、没有端口、端口说没有这个任务的，说没有（`unknown.txt`，出错）。端口交回读得到的字，照 `read` 分页：
   1. 从第 `offset` 行起一行一行接，一页最多 30000 个字（策略数据 `jobs.output_chars` 的出厂值，和 `shell` 一样），加上下一行就超过的停在前一行；第一行自己就超过的截在 30000 个字、补一个 `…`。「一行」照换行切，最后一段没有换行的也算一行；每一行以换行结尾。
   2. 后面还有的，接 `more.txt`（这一页是第几到第几行、一共几行、往下从哪接）。一共的行数边读边数，不把整份读进内存。
   3. 一行都没有的（没存下来、空的、子代理还没说过话）：`empty.txt`。`offset` 过了结尾：`past-end.txt`。
   4. 还在跑的，末尾再接一句：后台命令、子代理这一步没在跑工具的 `running.txt`；子代理这一步在跑工具的 `using.txt`，工具名照先后用 `, ` 接起来。
   - 后台命令读什么：结束了、输出存成了 blob 的读 blob；别的读会话目录下的 `jobs/<编号>.out`（跑着的读到这时的，载入时补 `aborted` 的读到崩的那一刻）。子代理读什么：执行器经会话表照它的日志看：最近的回答是最后一条回复里的字，这一轮完没完，这一步在跑的是最后那条回复里调了、还没有结果的工具（`session/tools.md` 第 6 条）。
4. `stop`：没写 `id`、没有这个任务同上。交给端口停：
   - 停了：`Stopped {job}.`（`stopped.txt`）。回报由执行器记下，带 `by_model`：后台命令 `job.reported`（`stopped`，`by` 是这次调用，`cause` 是这一轮的），子代理 `child.reported`（`stopped`）；都只记下、不叫醒她（`agents.md` 第三条第 3 条），下一次请求里照回报的写法渲染出来。
   - 已经结束了（回报到了，或者正好自己退出了：只认先到的那一个）：`ended.txt`，出错。子代理报过 `done` 也算结束了（父子留言叫醒它随 7-7）。
   - 不看叫停的旗：停一下子就交回。
5. 不报效果。

### 样子

`list`：

```text
j1 command "跑全部测试": running, 72134 ms
j2 agent "查 CI 为什么红": done, 125000 ms
j3 command "build": exited, 41000 ms
```

`output`，一条还在跑的后台命令，读了前三行、一共四行（每行一万个字，这一页用 `…` 省掉了）：

```text
字字…字
字字…字
字字…字
(Showing lines 1-3 of 4. Use offset=4 to continue.)
(j1 is still running.)
```

`output`，一个在跑的子代理：

```text
Reading the tests first.
(j2 is still running. It is using read, grep now.)
```

给她的字都在 `resources/software/basesystem/jobs/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 列出来的一行 | `listed.txt` | `{job} {what} "{title}": {status}, {ms} ms` |
| 一个都没有 | `none.txt` | `No jobs yet.` |
| 没有这个任务 | `unknown.txt` | `There is no job {id}.` |
| 停的时候已经结束了 | `ended.txt` | `{job} has already ended.` |
| 停了 | `stopped.txt` | `Stopped {job}.` |
| 读输出，后面还有 | `more.txt` | `(Showing lines {from}-{to} of {total}. Use offset={next} to continue.)` |
| 读输出，`offset` 过了结尾 | `past-end.txt` | `(The output has {total} lines; offset {offset} is past the end.)` |
| 读输出，一行都没有 | `empty.txt` | `No output.` |
| 读输出，还在跑 | `running.txt` | `({job} is still running.)` |
| 读输出，子代理这一步在跑工具 | `using.txt` | `({job} is still running. It is using {tools} now.)` |

- 换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）：标题、她给的编号里的引号、换行写不进这一行。输出本身不转义。
- `more.txt` 和 `read/more.txt` 一字不差，各登记各的：两件工具各管各的字。

### 出错

出错的结果都标成出错。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对、读、停没写 `id` | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 没有这个任务（不合写法、没有端口、端口说没有） | `unknown.txt` | `jobs/unknown`，字段 `id` |
| 停的时候已经结束了 | `ended.txt` | `jobs/ended`，字段 `job` |

### 给人看的字

显示名：任务（Jobs），后面跟 `action` 的值；符号 `≡`。

| 说法 | 中文 | 英文 |
|---|---|---|
| `jobs/listed`（`count`） | `列出 {count} 个任务` | `Listed {count} jobs` |
| `jobs/none` | 没有任务 | No jobs |
| `jobs/output`（`job`） | `读了 {job} 的输出` | `Read the output of {job}` |
| `jobs/stopped`（`job`） | `停掉了 {job}` | `Stopped {job}` |
| `jobs/unknown`（`id`） | `没有任务 {id}` | `No job {id}` |
| `jobs/ended`（`job`） | `{job} 已经结束了` | `{job} has already ended` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/jobs.rs` | 声明三个动作、访问类别是读、「不用轮询」那一句在；`list` 一个一行、标题里的引号转义、一个都没有和没有端口的一样；`output` 分页、从 `offset` 读起、上限停在整行上、`offset` 过了结尾、没存下来和空的一样、还在跑的、子代理在跑的工具；`stop` 照编号交给端口，停了、已经结束了、没有；不合写法的编号照没有、没有端口的照没有、没写 `id` 参数不对、不认识的动作；六种说法两种语言都换得出字 |
| `crates/miyu-basesystem/src/jobs/page/tests.rs` | 最后一段没有换行也算一行、空行照算；停在整行上、停了照样数到结尾、多一个字就停在前一行；一行就超过的截断补 `…`、正好到上限的一整行照给；`offset` 过了结尾 |
| `crates/miyu-session/tests/jobs_stop.rs`、`jobs_stop/agents.rs` | 会话里真的 `jobs`：读到这时的输出、停掉（`by` 是那次调用、`cause` 是那一轮的、`by_model`、不叫醒她）、列出来是停掉的；子代理最近的回答和在跑的工具、停掉它（经会话表停、回报记成它交来的、`by_model`、不叫醒） |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `agents.md` 第五条（`jobs`）、第三条（她停的不叫醒）、「对外的样子」（`jobs.output_chars`）。
- `10-自带软件.md` 第三节、第九节（工具面的预算）。
- `26-提示词.md` 附录（说明的草稿、「不用轮询」来自旧版实测）、第十节（登记簿）。

### 还没有的

- 父子留言叫醒报过 `done` 的子代理以后，它又在跑：`list` 照旧写 `done`、`stop` 照已经结束了（7-7 再改）。
- `jobs.output_chars` 在配置里改：随配置那一步，现在是代码里的出厂值。
