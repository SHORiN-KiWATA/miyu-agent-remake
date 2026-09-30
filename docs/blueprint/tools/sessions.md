## `sessions`

### 是什么

列出你别的主会话（施工 C-3，`cross-session.md` 第一条）：第一行是她自己，下面一个会话一行，最近有动静的在前。每一行是短编号、标题、工作目录、忙不忙、最近一次动静。只有本机的主会话有这件工具，子会话、场所会话（群）没有（`cross-session.md` 第九条）。头的 `session.list` 每一项多出的三格和这里是同一个函数算的（`protocol.md`「`session.list`」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/sessions.rs` | 参数、排序、一行一个、分页；短编号撞了放长 |
| `crates/miyu-tool/src/sessions.rs` | 列会话的端口 `SessionsPort`、列出来的一个 `MainSession`、那件工具的名字 `SESSIONS`；认会话编号的 `find_session`（C-4、C-5 用）（`tools/interface.md`） |
| `crates/miyu-session/src/sessions.rs` | 执行器这一头：本机的主会话才造端口，经会话表的端口要这个会话的属主的主会话，拿掉她自己（`session/tools.md`「1d. 列会话」） |
| `crates/miyu-session/src/agents.rs` | 工具面：`Agents::lists_sessions` 说哪些会话有 `sessions` |
| `crates/miyu-session/src/spawn.rs` | 会话表的端口 `SessionPort::sessions` |
| `crates/miyu-endpoint/src/spawn.rs`、`list.rs` | 会话表那一头：和 `session.list` 同一个 `scan` 读，只要主会话 |
| `resources/software/basesystem/tools/sessions.json` | 说明和参数格式 |
| `resources/software/basesystem/sessions/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：只读别的会话的日志，什么都不改，只读开着也列得出来。一条路径都不报，权限策略照访问类别判，读的放行。不报效果。

样本 `resources/software/basesystem/tools/sessions.json`：

```json
{
  "description": "List your other sessions, most recently active first. Each row gives the session id, the title, working directory, whether it is busy and when it was last active.",
  "parameters": {"type":"object","properties":{"limit":{"type":"integer","description":"Default 20."},"offset":{"type":"integer","description":"How many sessions to skip."}}}
}
```

- 说明第二句不点名 `send_message`、`history`：C-3 合进来时还没有 `send_message`（C-5 改名），`history` 也还没有 `session`（C-4），点了名就是一件不存在的工具（`26-提示词.md` J4）。C-5 改名时补成「Each row gives the id to use with send_message and history, …」，和那一次冷启动放在一起（2026-10-01 主会话定）。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `limit` | 否 | 最多几个，正整数；不写（`null` 也算）是 20 |
| `offset` | 否 | 跳过几个，从 0 数，不能是负数；不写（`null` 也算）是 0 |

- 两格都照 `integer` 认：整数、写成字符串的整数都收（施工 4-9 再补二的做法）。
- 别的参数不认，也不报错。

### 怎么走

1. 读参数：读不成的（类型不对、`limit` 不是正整数、`offset` 是负数），交回参数不对的那一句，端口一次都不问。
2. 没有端口（`Call.sessions` 是空的：测试里的假调用，核心没装会话表的）：只交 `none.txt`，不写第一行，说法 `sessions/none`。
3. 问端口要：执行器经会话表要这个会话的属主的主会话（`session.created` 不带 `parent`）、没删的，拿掉她自己（`session/tools.md`「1d. 列会话」）。会话表那一头和 `session.list` 同一个函数算，每个会话读一遍日志，读下一个之前看一眼叫停的旗：
   - 工作目录：日志里最后一条带 `cwd` 的 `turn.started` 的，没有就照 `session.created` 的，都没有（很早以前的日志）写 `~`。和会话表载入时同一个认法（`protocol.md`「会话表」第 5 条）。
   - 最近一次动静：日志最后一条事件的 `at`。日志坏了的，照坏的那一段以前的；只有一段、它坏了的，是 `session.created` 的时刻。
   - 忙：它在会话表里，这时有回合在进行（和 `Core::idle` 看的是同一样：内核说不空闲，结束了 `turn.ended` 还没落盘的、在等人确认、等人回答的、改回文件的都算）。没载入的都是闲。
4. 列不出来（放会话的目录读不了、核心正在停）：`failed.txt`，出错，说法 `sessions/failed`（字段 `error`，英文的一句）。执行器记一行 `WARN sessions not listed`。不当成「没有别的会话」答（2026-10-01 主会话定）。
5. 叫停的旗举起来了：交回「停下了」（`Done::stopped`），内核照「已取消，跑到一半」记。
6. 排：最近一次动静新的在前，一样的照编号倒着排。
7. 写：
   1. 第一行 `you.txt`，写她自己的短编号。
   2. 一个别的会话都没有：接 `none.txt`，说法 `sessions/none`。
   3. `offset` 不小于一共几个：接 `past-end.txt`，说法 `sessions/past-end`（字段 `total`）。
   4. 别的：从第 `offset` 个起最多 `limit` 个，一个一行，有标题的 `listed.txt`、没有的 `listed-untitled.txt`；`state` 是 `busy` 或者 `idle`；时刻照这个会话的时区写到分钟（`Call.offset`，和 `history` 一样）。后面还有的接 `more.txt`（这一页是第几到第几个，从 1 数，一共几个，下一次的 `offset`）。说法 `sessions/listed`（字段 `count`：这一页列了几个）。
8. 短编号撞了放长（`cross-session.md`「对外的样子」会话的短编号）：照这一张列表（她自己加上全部别的会话，不只是这一页）比，后 8 位和别的撞了的写后 12 位（整个最后一段），后 12 位还撞的写整个编号。每一个各看各的：不撞的照写 8 位。
9. 现在每列一次都把每个会话的日志整份读一遍，和 `session.list` 一样；慢了随会话列表的索引一起换（「还没有的」）。

### 样子

```text
You are session 22334455.
9f03b21c "修 CI" in ~/src/miyu: busy, last active 2026-10-01 14:03
0c5d77aa (untitled) in ~/notes: idle, last active 2026-09-30 22:41
(Showing 1-2 of 5. Use offset=2 to see more.)
```

给她的字都在 `resources/software/basesystem/sessions/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 第一行 | `you.txt` | `You are session {id}.` |
| 有标题的一行 | `listed.txt` | `{id} "{title}" in {cwd}: {state}, last active {time}` |
| 没标题的一行 | `listed-untitled.txt` | `{id} (untitled) in {cwd}: {state}, last active {time}` |
| 后面还有 | `more.txt` | `(Showing {from}-{to} of {total}. Use offset={next} to see more.)` |
| 没有别的 | `none.txt` | `You have no other sessions.` |
| `offset` 过了结尾 | `past-end.txt` | `(You have {total} other sessions. Offset {offset} is past the end.)` |
| 列不出来 | `failed.txt` | `Could not list the sessions: {error}` |

- 换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）：标题、工作目录里的引号、反斜杠、换行写不进这一行。

### 出错

出错的结果都标成出错，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 列不出来 | `failed.txt` | `sessions/failed`，字段 `error` |

### 给人看的字

显示名：列会话（Sessions，セッション一覧），不跟参数；符号 `≡`。

| 说法 | 中文 | 英文 | 日文 |
|---|---|---|---|
| `sessions/listed`（`count`） | 列出 {count} 个会话 | Listed {count} sessions | {count} 件のセッション |
| `sessions/none` | 没有别的会话 | No other sessions | ほかのセッションはありません |
| `sessions/past-end`（`total`） | 一共 {total} 个别的会话，已经列完了 | All {total} other sessions are listed | ほかのセッション {total} 件はすべて表示済みです |
| `sessions/failed`（`error`） | 没列出来：{error} | Could not list sessions: {error} | 一覧を取得できませんでした：{error} |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/sessions.rs` | 访问类别是读、两格都可以不写；输出一字不差、第一行是自己、新的在前、一样的照编号、没标题的、字段转义、时刻照会话的时区；分页、写成字符串的整数、不写是 20、到了结尾不接那一句；过了结尾、没有别的（过了结尾也照没有别的说）；短编号撞了放长到 12 位、整个编号；参数不对的七种不问端口，`null` 和不认识的参数照收；没有端口不写第一行；列不出来；叫停；四种说法中文、英文、日文都换得出字、显示名不跟参数 |
| `crates/miyu-tool/src/sessions/tests.rs` | 认会话编号：整个编号、8 位和 12 位的后缀；大写、带空格、不到 8 位、带 `-` 的后缀、编号的前缀都对不上；后缀撞了是不止一个，写长了是一个 |
| `crates/miyu-session/tests/sessions.rs` | 执行器：照这个会话的属主要、拿掉她自己；没有会话表的照没有别的；载入的主会话照样列；工具面本机主会话有 `sessions`、子会话和群没有、别的工具一件不少；子会话调它照没有的工具拒、端口不问 |
| `crates/miyu-endpoint/tests/sessions.rs` | 真核心：`session.list` 的工作目录跟着头报的换、忙着的写 `busy`、闲着的不写、最近一次动静是日志最后一条；她列出来的只有同一个属主的别的主会话，不列自己、子会话、删了的，和 `session.list` 那一份对得上 |
| `crates/miyu-endpoint/src/list/tests.rs` | 工作目录照最后一条带 `cwd` 的、不带的不盖，一条都没记的写 `~`；最近一次动静是最后一条的时刻，哪种事件都算；忙不忙照会话表交来的；叫停的旗举了一个都不读 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `cross-session.md` 第一条（列会话）、第九条（谁能用）、「对外的样子」（短编号、`session.list` 的三格）、「起草时定的」第 1、2、4、14、15、20 条，施工时定的第 40 到 48 条。
- `docs/designs/29-跨会话.md` 第一节第 1 条。
- `10-自带软件.md` 第三节（基础系统 14 件）、第九节（工具面的预算）。
- `26-提示词.md` J4（说明里点名的要在）、第十节（登记簿）。

### 还没有的

- 会话列表的索引：现在每列一次都整份读每个会话的日志（`protocol.md`「还没有的」）。
- 列表里分出「在等人回答」：现在算忙。
- 说明里点名 `send_message`、`history`：C-5 改名时补。
- 多用户：只列同一个人的，随多用户那一步。现在只有管理员。
