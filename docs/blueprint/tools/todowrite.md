## `todowrite`

### 是什么

待办（施工 D-3，`10-自带软件.md` 第三节）：她做多步的活时维护一张待办清单，头照着显示（终端界面的侧边栏、窄屏输入框上面，网页的右侧面板）。每次整份换，照 Claude Code 的 TodoWrite、codex 的 `update_plan`、opencode 的 `todowrite`（2026-10-07 项目主人定，名字照它们，原来叫 `todo`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/todowrite.rs` | 参数、结果那一句、报效果 |
| `crates/miyu-tool/src/todos.rs` | 名字 `TODOWRITE`：执行器照它定工具面 |
| `crates/miyu-kernel/src/event/effect.rs` | 效果 `todo.written`（`TodoWritten`、`Todo`、`TodoStatus`） |
| `crates/miyu-kernel/src/history/todos.rs`、`session/todos.rs` | 当前的清单怎么算、变了推 `todos.changed`（`kernel/session.md`「待办」） |
| `resources/software/basesystem/tools/todowrite.json` | 说明和参数格式 |
| `resources/software/basesystem/todowrite/*.txt` | 结果里给她看的两句 |
| `resources/core/compaction/notes-todos.txt` | 检查点里待办那一段的头一行（`compaction.md` 第八条） |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：只改这个会话自己的清单，只读开着也能写。说明三句：是什么、整份换、怎么标状态。不要 Claude Code 的 `activeForm`：头照 `content` 显示就够，多一格每次都多写一遍。

样本 `resources/software/basesystem/tools/todowrite.json`：

```json
{
  "description": "Keep this session's task list for multi-step work, which the user sees. Each call replaces the whole list. Mark an item in_progress when you start it and completed as soon as it is done.",
  "parameters": {"type":"object","properties":{"todos":{"type":"array","items":{"type":"object","properties":{"content":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","completed"]}},"required":["content","status"]}}},"required":["todos"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `todos` | 是 | 整份清单，照先后 |
| `todos[].content` | 是 | 一句话：要做的事 |
| `todos[].status` | 是 | `pending`、`in_progress`、`completed`；别的是参数不对 |

- 不强制只有一项在做：说明里点一句，内核不拦。
- 一条路径都不报。
- **谁有**：本机的会话，主会话、子会话各管各的清单；场所会话（群）没有（`session/tools.md`「工具面」）。

**结果**（给她看的）：还有没做完的 `Todo list updated: <做完几项> of <一共几项> done.`（`updated.txt`）；全部做完（连同空的）清空，`All todos are done. The list is cleared.`（`cleared.txt`）。不把清单原样回给她。

**效果** `todo.written`：`{"todos": [{"content": …, "status": …}]}`，清空的是空列表（`kernel/events-bodies.md`）。因为全部做完而清空的多一格 `done`：她这一次交的那一份（每一项都做完了）；她交了空的的不写（施工 D-3 补）。头不翻效果，照 `subscribe` 的 `todos` 和推送的 `todos.changed`（`protocol.md`）。

### 怎么走

1. 读参数：读不成的（少了格、状态不认识、类型不对），交回参数不对的那一句，什么效果都不报。
2. 全部做完（连同空的）：效果是空列表，结果说清空了。
3. 不然：效果是这一整份，结果说做完几项、一共几项。
4. 内核照效果算当前的清单，变了推 `todos.changed`，压缩时写进检查点（`kernel/session.md`「待办」）。

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 显示名 `todowrite` | 待办 | Todos |
| `todowrite/updated` | 更新了待办：做完 {done}/{total} | Updated the todos, done: {done}/{total} |
| `todowrite/cleared` | 待办都做完了，清空 | All todos done, list cleared |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-basesystem/tests/todowrite.rs` | 结果那一句逐字节比、效果带着整份；全部做完和空的清空；参数不对的四种不报效果；访问类别读 |
| `crates/miyu-kernel/src/session/tests/todos.rs` | 落了盘才推、推一次，一样的不推，清空了也推；换待办的那一条还没落盘的不推；撤掉写它的那一轮退回更早的、恢复回来；载入以后照日志算回来、不推，接着说一轮也不重推 |
| `crates/miyu-kernel/src/history/todos/tests.rs` | 最近一份没撤掉的；压缩以后的有效历史照样记得 |
| `crates/miyu-kernel/src/session/tests/scenario/rebuild.rs` | 检查点里取回指路后面多一段待办，一项一行；模板没有的不写 |
| `crates/miyu-endpoint/tests/todos.rs` | 真核心、真工具：还没写过 `subscribe` 不带 `todos`；写了推 `todos.changed`；之后再订阅的回应带着 |
| `crates/miyu-kernel/tests/transient_sample.rs` | `todos.changed` 的样本一字不差 |

### 出处

- `10-自带软件.md` 第三节（`todowrite` 的行）、第九节（预算）；`26-提示词.md` 附录、第十节。
- `09-压缩.md` 第四节：待办清单原样带上。
- `13-终端界面.md` H2、`21-网页.md`：头怎么显示。

### 还没有的

- Claude Code 新的 Task 一族（编号、负责人、依赖）：多个代理分工时再加，到时候是「加」，这一件不推翻。
