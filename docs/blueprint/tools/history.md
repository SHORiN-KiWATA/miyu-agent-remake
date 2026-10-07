## `history`

### 是什么

翻这个会话自己的日志：按关键词找，按序号读，也能按时间、谁说的筛。压缩换出去的旧内容都还在日志里，她用它取回（`09-压缩.md` Z1）。只读这个会话自己的日志，不碰文件，不报效果。多一格 `session`（施工 C-4）：写了的读别的会话的日志，和读自己的日志走同一条路。认得出的只有 `sessions` 列得出的本机会话，场所会话读不到（施工 O-3）。

状态：6-4 做好了（2026-09-29）：关键词加筛选，读会话自己的整份日志，压缩换出去的找得回。2-7 补（2026-10-01）列出人切权限级别。C-4（2026-10-01）加 `session`：认出这个会话看得到的别的会话，只读地开它的日志；找不到、对得上不止一个、这个会话没有列会话的端口各拒一句。施工定下的技术细节写在各节里；向量一路、索引见「还没有的」。

### 在哪

施工时照这个放：

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/history.rs` | 参数、读日志、筛；`session` 认出来的不是她自己的，只读地开那个会话的日志再读（施工 C-4） |
| `crates/miyu-basesystem/src/history/entry.rs` | 哪些算一条、一条的原文怎么写、「谁」那一格（别的 harness 发来的写名字，施工 7-10；别的会话发来的写短编号，施工 C-2）；人切权限级别的那一条（施工 2-7 补） |
| `crates/miyu-basesystem/src/history/page.rs` | 找、读：一页怎么写、往下翻、整页上限、摘一段 |
| `crates/miyu-basesystem/src/history/time.rs` | `since`、`until` 的写法 |
| `crates/miyu-kernel/src/history.rs` | `History::whole()`：留着压缩替代掉的，撤销、恢复、撤回照有效历史的规矩算（`kernel/history.md`） |
| `crates/miyu-kernel/src/time.rs` | 照时区写到分钟、照时区的日期和钟点换回时刻 |
| `crates/miyu-tool/src/log.rs` | 这个会话日志的只读入口：`ReadLog`，`Call.log` 带着 |
| `crates/miyu-tool/src/sessions.rs` | `SessionsPort::open`（施工 C-4）：只读地开别的会话的日志，交回 `Log`；`find_session` 认 `session` 写的是哪一个（C-3 做好） |
| `crates/miyu-store/src/log/open.rs` | `read_segments`：只读地一段一段读 |
| `crates/miyu-session/src/tools.rs` | 执行器把这个会话日志的只读入口、会话的时区、列会话的端口交给这次调用（`sessions`、`history` 共用的 `Call.sessions`） |
| `crates/miyu-session/src/sessions.rs` | `Lister::open`（施工 C-4）：转给会话表的 `SessionPort::read_log` |
| `crates/miyu-session/src/spawn.rs`、`crates/miyu-endpoint/src/spawn.rs` | `SessionPort::read_log`（施工 C-4）：会话表这一头只算出会话的真实目录，不读盘，`Log` 包着它；`miyu-endpoint` 已经直接依赖 `miyu-store`（`peek` 先例），不必把 `miyu-session` 里自己那份 `LogDir` 公开出来 |
| `resources/software/basesystem/tools/history.json` | 说明和参数格式 |
| `resources/software/basesystem/history/*.txt` | 输出里给她看的几句；`agent.txt` 是别的 harness 发来的那一条的「谁」（施工 7-10）；`session.txt` 是别的会话发来的那一条的「谁」（施工 C-2）；`permission.txt` 是人切权限级别的那一条的原文（施工 2-7 补） |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

**交给工具的**（施工 6-4，C-4 多一格）：一次调用的 `Call` 多三格，别的工具不看（`sessions` 那格和 `sessions` 工具共用）。

- `log`：这个会话日志的只读入口（`ReadLog`）。一段一段交出事件，交给的函数交回「不读了」就停；最后一段末尾没写完的半行跳过，一个字节都不写：会话正在往里写。执行器照会话的目录造，底下是存储的 `read_segments`。测试里的假调用没有，`history` 照「读不了日志」出错。
- `offset`：会话的时区，执行器照会话现在的环境交（`kernel/facts` 的 `Environment`，头报上来换了跟着换）。没有环境的测试照 UTC。读别的会话时这一格不变：时刻照这个会话自己的时区写，和读的是哪一份日志无关（施工 C-4）。
- `sessions`（施工 C-3，C-4 多用一个方法）：列会话的端口（`SessionsPort`）。`session` 参数写了的，先经它的 `list` 列出这个会话看得到的主会话、和 `find_session` 一起认出是哪一个，再经它的 `open` 只读地开那个会话的日志。没有这个端口的（子会话、场所会话、测试里的假调用）写了 `session` 直接拒（`not-here.txt`）。

### 对外的样子

访问类别 `read`。不报要碰的路径：读的是会话自己的日志，不经过文件工具的边界，哪一级都能用，只读时也能用。

例子（说明和参数的原文施工时定，量 token、进登记簿，受工具面的预算管，`10-自带软件.md` 第九节）：

```json
{
  "description": "Search or read back earlier parts of this conversation, including what compaction moved out of context. Entries are numbered like the checkpoint says.",
  "parameters": {"type":"object","properties":{"query":{"type":"string","description":"Words to look for. Without it, entries are listed in order."},"from":{"type":"integer","description":"First entry number."},"to":{"type":"integer","description":"Last entry number."},"since":{"type":"string","description":"Earliest time, like 2026-09-29 14:00."},"until":{"type":"string","description":"Latest time."},"by":{"type":"string","enum":["user","assistant","tool"]},"limit":{"type":"integer","description":"Default 20."},"session":{"type":"string","description":"Another session's id, to read that session instead of this one."}}}
}
```

`session` 的原文施工 C-4 时定，待量 token（`cross-session.md`「样子」）。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `query` | 否 | 要找的词，空格隔开。有它是「找」，没有是「读」 |
| `from`、`to` | 否 | 序号的范围，两头都算。没给的是从头、到尾 |
| `since`、`until` | 否 | 时刻的范围，两头都算，照这个会话的时区：`2026-09-29 14:00`。只写日期的，`since` 是那一天的 0 点，`until` 是那一天的 24 点 |
| `by` | 否 | 谁说的：`user` 人说的话（人切权限级别也算，施工 2-7 补），`assistant` 她的回复，`tool` 工具结果 |
| `limit` | 否 | 这一页最多几条，默认 20 |
| `session` | 否 | 另一个会话的编号，读那个会话的日志，不是这次调用自己的（施工 C-4，`cross-session.md` 第二条）。写了自己的编号，照没写；找不到、对得上不止一个、这个会话不能读别的会话（没有列会话的端口）各拒一句 |

- 参数照两路检索定下来（2026-09-29 项目主人定）：M6 只有关键词一路；以后接上向量一路，`query` 还是它，只是排名变了（`17-记忆.md` 第四节）。混合召回以关键词匹配为主、向量为辅（2026-09-29 项目主人定）；向量那一路怎么做、开关放哪，做记忆、知识库时再定。
- 别的参数不认，也不报错。

**哪些算一条**：序号就是日志的序号，和检查点里「被替代的是第 1 到 N 条」说的是同一个数。

| 事件 | 算不算 | `by` |
|---|---|---|
| `message.user` | 算。`by` 是 `harness` 的（别的 harness 发来的话，施工 7-10），「谁」那一格照 `history/agent.txt` 写成 `agent "<名字>"`，名字照模板的规矩转义，和请求里那块标签是同一个名字（`kernel/request.md`「别的 harness 发来的话」）；筛的时候照样算 `user`。`by` 是别的会话的（既不是父会话、也不是这个会话派的子代理，照 `History::whole()` 记着的认，施工 C-2），「谁」那一格照 `history/session.txt` 写成 `session <短编号>`，和请求里那块标签是同一个编号（`kernel/request.md`「别的会话发来的话」）；筛的时候照样算 `user`。父会话的交代、留言，子代理的留言照旧写 `user` | `user` |
| `message.assistant` | 算：正文和工具调用；思考不给 | `assistant` |
| `tool.result` | 算 | `tool` |
| `context.compacted` | 算：以前的摘要，以前压缩掉的也找得到。清空的摘要是空的，一个字都没有，不算（施工 6-8 补）；清空以前的照样找得到 | `assistant` |
| 带 `permission` 的 `session.policy_changed`：人切了权限级别（施工 2-7 补） | 算。「谁」那一格写 `user`，原文照 `history/permission.txt` 写切成了哪一级（`Changed the permission level to <级别>`，级别的写法和事实里的一样：只读开着是 `read_only`，不认识的级别是 `read_only`，`kernel/request.md` 的 `effective_level`）。撤掉的回合里切的也算：撤销不改现在的权限（`kernel/session.md`「切权限级别」第 7 条，施工 2-7 补照内核的算法核对过：载入时照整份日志算，活着时撤销不动它）。只换了策略快照、没带 `permission` 的不算 | `user` |
| 撤掉的回合里的（里面的压缩也是，施工 6-9）、撤回的消息 | 不算：撤了就跟没说过一样。人切权限级别的除外（上一行） | |
| 她自己翻记录的那几步：`history` 的调用、结果 | 不算（施工 6-4）：找的时候会找到自己这一次调用（参数里就有要找的词），翻出来的旧结果又和原文重复。回复里别的正文、别的工具调用照算 | |
| 事实注入、`model.called`、回合和会话的事件（上面切权限的除外）、`files.restored` | 不算 | |

### 怎么走

1. 读参数，读不懂的：参数不对。时刻写法不对：时刻写得不对，带上正确的写法。
2. 认 `session`（施工 C-4，`cross-session.md` 第二条）：没写的读这次调用自己的日志。写了的，先照 `find_session` 在这个会话看得到的主会话（含她自己）里对：认成她自己的，照没写；找不到（`no-session.txt`）、对得上不止一个（`ambiguous.txt`）：各一句，出错，一条日志都不读。没有列会话的端口的（子会话、场所会话）：写了 `session` 直接拒（`not-here.txt`），不去找。认出来的，经会话表只读地开那个会话的日志（`ReadLog`），不载入它；在跑的也读得到。
3. 从头到尾一段一段读这份日志，照撤销、恢复、撤回算出哪些不算：交给内核的 `History::whole()`，和有效历史同一套规矩，只是压缩替代掉的留着，`context.compacted` 自己也算一条。人切权限级别的那几条不经它，照读来的原样挑出来，和别的照序号排在一起（施工 2-7 补）。读不了的：`Could not read the log: <原因>`，算出错——列会话、开日志失败的也照这一句（施工 C-4）。
4. 照 `from`、`to`、`since`、`until`、`by` 筛。
5. **找**（有 `query`）：
   1. 每个词都出现的算命中，不分大小写，照原样比，中文不分词。
   2. 新的在前。一条一行：`#<序号> <时刻> <谁>: <摘出来的一段>`（`<谁>` 是 `by` 的那三种写法，别的 harness 发来的是 `agent "<名字>"`，别的会话发来的是 `session <短编号>`），摘第一处命中前后，一共最多 200 个字；连着的空白（换行也算）换成一个空格，前后截掉了的写 `…`。比的是「读」时这一条下面的原文。
   3. 还有更早的：末尾接 `(Showing {n} of {total} results. Use to={next} to see older ones.)`，`next` 是这一页最早那一条的序号减一。
6. **读**（没有 `query`）：
   1. 照先后，从范围的第一条起。每条头一行 `#<序号> <时刻> <谁>`（`<谁>` 同第 5 条），下面是原文：工具调用写成 `→ <工具名> <参数原文>`；图片、文件写占位。
   2. 整页最多 30000 个字，和 `shell` 一样；一条就超过的，截到上限，写明这一条一共多少字。
   3. 还有：末尾接 `(Showing entries {first}-{last}. Use from={next} to continue.)`。
7. 什么都没有：`No entries found`，不算出错。
8. 叫停：读下一段日志之前看一眼；认 `session` 时列会话也照它（施工 C-4）。
9. 时刻照这个会话的时区写，到分钟：`2026-09-29 14:03`。`until` 写到分钟的，那一分钟里的都算。读别的会话时也是这个会话自己的时区，和目标会话无关（施工 C-4）。
10. `limit` 不是正整数的：参数不对。`from` 比 `to` 大、范围里一条都没有的：`No entries found`。

**一条的原文**：

- 人说的话、工具结果：正文照原样；图片写 `[image]`，文件写 `[file <名字>]`（`history/image.txt`、`history/file.txt`）。
- 她的回复：正文照原样，每个工具调用一行 `→ <工具名> <参数原文>`；思考不给。
- 以前的摘要：摘要正文。
- 人切了权限级别：`Changed the permission level to <级别>`（施工 2-7 补）。

- M6 不建索引，每次从头读：会话再长，日志也就几十兆，先量；慢了再建。全文索引随记忆、知识库那一套一起做（`07-存储.md` 第七节、`19-知识库.md`）。
- 日志里的原文是别人写的（工具输出、网页），当数据给她看，和当时她看到的是同一份。

### 样子

找：

```text
#230 2026-09-29 15:10 agent "claude-code": …迁移脚本写好了，按会话分区…
#212 2026-09-29 14:05 user: …数据库那张表改成按会话分区，别再用一张大表…
#87 2026-09-29 11:40 assistant: …按会话分区的话，迁移要…
(Showing 3 of 5 results. Use to=86 to see older ones.)
```

别的会话发来的一条（施工 C-2），「谁」写它的短编号：

```text
#231 2026-09-29 15:12 session 22334455: …迁移写完了，按会话分区…
```

读别的会话（`session` 写它的编号，施工 C-4）：

```text
#88 2026-10-01 14:10 session 22334455: …迁移写完了，按会话分区…
#61 2026-10-01 13:02 user: …先把 CI 修好再说…
```

找不到、对得上不止一个、这个会话不能读别的会话（施工 C-4）：

```text
No session has the id "9f03b21c".
```

```text
"9f03b21c" matches more than one session. Use the full id.
```

```text
This session cannot read other sessions.
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
| 读不了日志 | 读不了记录：<原因> | Could not read the history: <reason> |
| 找不到会话（施工 C-4） | 没有会话 <session> | No session <session> |
| 对得上不止一个（施工 C-4） | <session> 对得上不止一个会话 | <session> matches more than one session |
| 这个会话不能读别的会话（施工 C-4） | 这里不能读别的会话 | Cannot read other sessions here |

显示名那一格照别的工具：图标 `✱`，参数里挑 `query` 写在后面，没有 `query` 的只写名字。

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/src/history/tests.rs` | 哪些算一条；撤掉的、撤回的、她自己翻记录的不算；四种筛；找：每个词都要、不分大小写、新的在前、摘一段、往前翻；读：先后、工具调用的写法、整页上限、一条太长；时刻的写法和时区；参数不对；读不了日志；叫停 |
| `crates/miyu-kernel/src/history/tests.rs` | `History::whole()`：压缩替代掉的留着，摘要也是一条；撤销、恢复、撤回和有效历史一样；撤掉压缩所在的那一轮，摘要跟着不算（施工 6-9） |
| `crates/miyu-store/src/log/tests.rs` | `read_segments`：一段一段交、叫停就不读下去、半行跳过不截 |
| `crates/miyu-session/tests/history.rs` | 真的会话：压缩以后 `history` 找得到压缩以前的话，时刻照会话的时区 |
| `crates/miyu-basesystem/src/history/tests/permission.rs`（施工 2-7 补） | 人切权限级别：找、读都列出，「谁」是 `user`、原文写切成的那一级；筛 `user` 时在里面，筛 `assistant`、`tool` 时不在；撤掉的回合里切的照样列，只读开着写只读，只换了策略快照的不算。样本会话里的 52、63 两条跟着出现在 `tests.rs`、`read.rs` 的清单里 |
| `crates/miyu-basesystem/src/history/tests/harness.rs`（施工 7-10） | 别的 harness 发来的话：找、读的「谁」那一格写名字、名字照规矩转义；筛 `user` 时在里面，筛 `assistant`、`tool` 时不在 |
| `crates/miyu-basesystem/src/history/tests/peers.rs`（施工 C-2） | 别的会话发来的话：找、读的「谁」那一格写 `session <短编号>`，父会话的话照旧是 `user`；筛 `user` 时在里面，筛 `assistant`、`tool` 时不在 |
| `crates/miyu-basesystem/src/history/tests/other.rs`（施工 C-4） | 读别的会话：`session` 认出来的读的是那个会话的日志，不是这次调用自己的；认成她自己的照没写、不开任何别的日志；找不到、对得上不止一个、没有列会话的端口各拒一句，一条日志都不读；列会话、开日志失败都照「读不了日志」；时刻照这个会话自己的时区，和读的是哪一份日志无关 |
| `crates/miyu-session/tests/history_other.rs`（施工 C-4） | 真的会话 actor：读别的会话磁盘上真实的日志、不载入它、在跑的也读得到；子会话、场所会话（群）写了 `session` 都拒 |
| `crates/miyu-basesystem/tests/human_history.rs` | 每一种结果的说法，两种语言都换得出字，显示名也有；C-4 的三种拒绝也在内 |
| 真模型实测（M6 验收） | 压缩以后问她压缩前的细节，她会用 `history` 取回 |
| 真模型实测（C-7，跨会话验收） | 两个主会话，一个里说过一件事，另一个先 `sessions` 再 `history` 带 `session` 翻得到 |

### 出处

- `10-自带软件.md` 第三节、B10：独立成 `history`，不藏进 `read` 的路径前缀。
- `09-压缩.md` Z1：压缩是换出，不是删除。
- 2026-09-29 项目主人定：M6 先做关键词加筛选（时间、谁说的、序号范围），参数照两路检索定好，向量那一路随记忆做。
- `26-提示词.md` 附录：说明的草稿。

### 还没有的

- 人附的图片带名字（施工 3-9 四补）以后，这里的图片还写 `[image]`，不像文件那样写上名字：要另加一句带名字的，量 token、登记，以后顺手做。
- 向量一路、两路合并排名：随记忆（`17-记忆.md` 第四节）。
- 全文索引：同上。
- 群聊里按人名筛：随通讯平台。
