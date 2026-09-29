## `history`

### 是什么

翻这个会话自己的日志：按关键词找，按序号读，也能按时间、谁说的筛。压缩换出去的旧内容都还在日志里，她用它取回（`09-压缩.md` Z1）。只读这个会话自己的日志，不碰文件，不报效果。

状态：图纸（2026-09-29 定），M6 照它施工（6-4）。

### 在哪

施工时照这个放：

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/history.rs` | 参数、哪些算一条、筛、找 |
| `crates/miyu-basesystem/src/history/render.rs` | 一条怎么写、分页、给人看的说法 |
| `crates/miyu-tool/` | 一次调用交给工具的 `Call` 多两格：这个会话日志的只读入口、会话的时区 |
| `crates/miyu-session/src/tools.rs` | 执行器把这个会话日志的只读入口交给这次调用 |
| `resources/software/basesystem/tools/history.json` | 说明和参数格式 |
| `resources/software/basesystem/history/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`。不报要碰的路径：读的是会话自己的日志，不经过文件工具的边界，哪一级都能用，只读时也能用。

例子（说明和参数的原文施工时定，量 token、进登记簿，受工具面的预算管，`10-自带软件.md` 第九节）：

```json
{
  "description": "Search or read back earlier parts of this conversation, including what compaction moved out of context. Entries are numbered like the checkpoint says.",
  "parameters": {"type":"object","properties":{"query":{"type":"string","description":"Words to look for. Without it, entries are listed in order."},"from":{"type":"integer","description":"First entry number."},"to":{"type":"integer","description":"Last entry number."},"since":{"type":"string","description":"Earliest time, like 2026-09-29 14:00."},"until":{"type":"string","description":"Latest time."},"by":{"type":"string","enum":["user","assistant","tool"]},"limit":{"type":"integer","description":"Default 20."}}}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `query` | 否 | 要找的词，空格隔开。有它是「找」，没有是「读」 |
| `from`、`to` | 否 | 序号的范围，两头都算。没给的是从头、到尾 |
| `since`、`until` | 否 | 时刻的范围，两头都算，照这个会话的时区：`2026-09-29 14:00`。只写日期的，`since` 是那一天的 0 点，`until` 是那一天的 24 点 |
| `by` | 否 | 谁说的：`user` 人说的话，`assistant` 她的回复，`tool` 工具结果 |
| `limit` | 否 | 这一页最多几条，默认 20 |

- 参数照两路检索定下来（2026-09-29 项目主人定）：M6 只有关键词一路；以后接上向量一路，`query` 还是它，只是排名变了（`17-记忆.md` 第四节）。混合召回以关键词匹配为主、向量为辅（2026-09-29 项目主人定）；向量那一路怎么做、开关放哪，做记忆、知识库时再定。
- 别的参数不认，也不报错。

**哪些算一条**：序号就是日志的序号，和检查点里「被替代的是第 1 到 N 条」说的是同一个数。

| 事件 | 算不算 | `by` |
|---|---|---|
| `message.user` | 算 | `user` |
| `message.assistant` | 算：正文和工具调用；思考不给 | `assistant` |
| `tool.result` | 算 | `tool` |
| `context.compacted` | 算：以前的摘要，以前压缩掉的也找得到 | `assistant` |
| 撤掉的回合里的、撤回的消息 | 不算：撤了就跟没说过一样 | |
| 事实注入、`model.called`、回合和会话的事件、`files.restored` | 不算 | |

### 怎么走

1. 读参数，读不懂的：参数不对。时刻写法不对：时刻写得不对，带上正确的写法。
2. 从头到尾一段一段读这个会话的日志，照撤销、恢复、撤回算出哪些不算。
3. 照 `from`、`to`、`since`、`until`、`by` 筛。
4. **找**（有 `query`）：
   1. 每个词都出现的算命中，不分大小写，照原样比，中文不分词。
   2. 新的在前。一条一行：`#<序号> <时刻> <谁>: <摘出来的一段>`，摘第一处命中前后，一共最多 200 个字。
   3. 还有更早的：末尾接 `(Showing {n} of {total} results. Use to={next} to see older ones.)`，`next` 是这一页最早那一条的序号减一。
5. **读**（没有 `query`）：
   1. 照先后，从范围的第一条起。每条头一行 `#<序号> <时刻> <谁>`，下面是原文：工具调用写成 `→ <工具名> <参数原文>`；图片、文件写占位。
   2. 整页最多 30000 个字，和 `shell` 一样；一条就超过的，截到上限，写明这一条一共多少字。
   3. 还有：末尾接 `(Showing entries {first}-{last}. Use from={next} to continue.)`。
6. 什么都没有：`No entries found`，不算出错。
7. 叫停：读下一段日志之前看一眼。
8. 时刻照这个会话的时区写，到分钟：`2026-09-29 14:03`。

- M6 不建索引，每次从头读：会话再长，日志也就几十兆，先量；慢了再建。全文索引随记忆、知识库那一套一起做（`07-存储.md` 第七节、`19-知识库.md`）。
- 日志里的原文是别人写的（工具输出、网页），当数据给她看，和当时她看到的是同一份。

### 样子

找：

```text
#212 2026-09-29 14:05 user: …数据库那张表改成按会话分区，别再用一张大表…
#87 2026-09-29 11:40 assistant: …按会话分区的话，迁移要…
(Showing 2 of 5 results. Use to=86 to see older ones.)
```

读：

```text
#211 2026-09-29 14:04 assistant
→ read {"file_path":"src/db.rs"}

#212 2026-09-29 14:05 user
数据库那张表改成按会话分区，别再用一张大表
(Showing entries 211-212. Use from=213 to continue.)
```

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 显示名 | 翻记录 | History |
| 找到了 | 找到 <n> 条 | Found <n> entries |
| 读了 | 读了第 <a>–<b> 条 | Read entries <a>–<b> |
| 什么都没有 | 没有找到 | Nothing found |

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/src/history/tests.rs` | 哪些算一条；撤掉的、撤回的不算；四种筛；找：每个词都要、不分大小写、新的在前、摘一段、往前翻；读：先后、工具调用的写法、整页上限、一条太长；时刻的写法和时区；参数不对 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| 真模型实测（M6 验收） | 压缩以后问她压缩前的细节，她会用 `history` 取回 |

### 出处

- `10-自带软件.md` 第三节、B10：独立成 `history`，不藏进 `read` 的路径前缀。
- `09-压缩.md` Z1：压缩是换出，不是删除。
- 2026-09-29 项目主人定：M6 先做关键词加筛选（时间、谁说的、序号范围），参数照两路检索定好，向量那一路随记忆做。
- `26-提示词.md` 附录：说明的草稿。

### 还没有的

- 向量一路、两路合并排名：随记忆（`17-记忆.md` 第四节）。
- 全文索引：同上。
- 群聊里按人名筛：随通讯平台。
