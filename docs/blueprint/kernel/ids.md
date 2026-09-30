## 编号、时间和「谁」

### 是什么

事件里出现的每一种编号、名字、时刻，各是一种类型，写法定死；再加上事件的 `by`：这件事由谁引起。读和写一样严：写出去是什么样，读进来就只认什么样，对不上的报错。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/id.rs` | 十五种用字符串写的编号和名字；序号 `Seq`、回合编号 `TurnId`；内容哈希怎么算（`ContentHash::of`、`Hasher`） |
| `crates/miyu-kernel/src/id/call.rs` | 调用编号 `CallId` |
| `crates/miyu-kernel/src/id/job.rs` | 任务编号 `JobId`（施工 7-1；带上父会话的编号，施工 7-1 补） |
| `crates/miyu-kernel/src/time.rs` | 时刻 `Timestamp`；时区 `UtcOffset`；给模型看的当地钟点 `local_hour` |
| `crates/miyu-kernel/src/origin.rs` | `by`：八种，加上不认识的 |
| `crates/miyu-kernel/src/format_error.rs` | 写法不对时报的 `FormatError`（它的样子见 `kernel/events.md`「出错」） |
| `crates/miyu-kernel/src/raw.rs` | `by` 照 `kind` 分派的读法（`kernel/blocks.md`） |

这些东西不在内核里造：

| 什么 | 谁造 |
|---|---|
| 会话编号 | `crates/miyu-session/src/clock.rs` 的 `new_id`：UUIDv7，一个核心进程共用一个计数器（`session/actor.md`） |
| 时刻 | 同一个文件里的会话时钟：系统时间，到毫秒，不往回走 |
| 序号 | 账本 `Ledger::next_seq`，追加一条给一个（`kernel/history.md`） |
| 调用编号 | 流式累积器，照回复的序号一个个分（`kernel/request.md`） |
| 命令编号 | 发命令的一方：协议里每个请求的 `id` 就是命令编号，例如 `miyu ask` 的 `ask-<16 位十六进制>-<序号>`（`protocol.md`、`cli/ask.md`） |
| 任务编号 | 执行器的 `JobIds`：一个会话一份，照日志里用过的往下数，子会话带上自己在父会话里的编号（`session/tools.md`「派子代理」第 1 条） |

内核不读时钟：纯逻辑门禁拦 `SystemTime`、`Instant`。

### 对外的样子

**用字符串写的十五种**：JSON 里是字符串。每一种都有 `parse`（照规则查，不合的报 `FormatError`）、`as_str`（原样的文字）；`Display` 原样写；读 JSON 时照样查；比较、排序照字符串。

| 类型 | 是什么 | 规则 | 报错里叫它 | 例子 |
|---|---|---|---|---|
| `SessionId` | 会话编号 | 会话编号 | `session id` | `0192f3a0-1111-7abc-8def-001122334455` |
| `CommandId` | 命令编号，写进每条事件的 `cause` | 短名字 | `command id` | `cmd-7f3a` |
| `AccountId` | 账号，出现在路径 `home/<账号>/` 里 | 路径里的名字 | `account` | `alice` |
| `ContentHash` | 内容哈希：blob、策略快照、请求字节都用它 | 内容哈希 | `content hash` | `sha256:e3b0c442…b855` |
| `ModuleId` | 模块，清单里的 `id`，出现在路径 `home/<账号>/modules/<模块>/` 里 | 路径里的名字 | `module` | `memory` |
| `DriverFamily` | 驱动家族，驱动认领私有数据用 | 路径里的名字 | `driver family` | `openai-chat` |
| `FactKind` | 事实块的类别 | 路径里的名字 | `fact category` | `env` |
| `VenueId` | 场所：一个群、一个私聊、桌面语音 | 短名字 | `venue` | `qq:group:123456` |
| `ExternalId` | 外部身份：通讯平台上说话的人 | 短名字 | `external identity` | `qq:10086` |
| `ProviderId` | 供应商：配置里 `[providers.<名字>]` 的名字 | 短名字 | `provider` | `deepseek` |
| `ModelName` | 模型，照供应商的叫法原样记 | 短名字 | `model` | `deepseek-v4`、`qwen/qwen3-235b-a22b@2026-07` |
| `MediaType` | 媒体类型 | 媒体类型 | `media type` | `image/png` |
| `FileName` | 文件名，给人看的名字，不是路径 | 文件名 | `file name` | `报告.pdf` |
| `EventKind` | 事件种类 | 事件种类 | `event kind` | `message.user`、`ext.memory.recalled` |
| `HarnessName` | 别的 harness 报的名字（施工 7-1） | 短名字 | `harness name` | `claude-code` |

**数字的两种**：

| 类型 | 是什么 | JSON | 规则 |
|---|---|---|---|
| `Seq` | 序号：会话里的第几条事件 | 数字 | 大于等于 1 的整数，最大到 `u64` 的上限 |
| `TurnId` | 回合编号：这个回合 `turn.started` 的序号 | 数字 | 和序号一样。代码里是另一种类型，传错了编译不过 |

- `Seq::FIRST` 是 1；`Seq::new(n)` 给 0 得到空的；`get()` 是那个数；`next()` 是下一个。
- `TurnId::new(started)` 由 `turn.started` 的序号得到；`started()` 取回那个序号。

**调用编号** `CallId`：`call_<助手消息的序号>_<这条消息里的第几个调用>`，例如 `call_44_1`。

- 第几个从 1 数起，最大 4294967295（`u32`）；`CallId::new(message, index)` 给 index 0 得到空的。
- `message()` 是那条助手消息的序号，`index()` 是第几个。
- JSON 里是字符串。排序先照序号，再照第几个。
- 带着序号，一个会话里不会重复；供应商自己的编号放在驱动私有数据里（`kernel/blocks.md`）。

**任务编号** `JobId`（施工 7-1，`agents.md`）：`j` 加一段或几段从 1 起的整数，段之间用 `.` 连，例如 `j1`、`j12`、`j2.1`、`j2.1.1`（施工 7-1 补）。一个会话里后台命令和子代理共用一串，数的是最后一段；前面的几段是这个会话在父会话里的编号，主会话没有：主会话派的是 `j2`，`j2` 派的是 `j2.1`，`j2.1` 放到后台的命令是 `j2.1.1`，一棵树上不重名（`agents.md`「对外的样子」）。编号不回收，撤掉的回合里用过的也不再用（账本查，`kernel/history.md`）。

- `JobId::new(n)` 是只有一段的 `j<n>`，给 0 得到空的；`under(n)` 在这个编号后面接一段：`j2` 的 `under(1)` 是 `j2.1`，给 0 得到空的；`last()` 是最后一段的数。每一段最大到 `u64` 的上限，段数不限（深度上限管着，`agents.md`「上限」的 `jobs.depth`）。
- JSON 里是字符串。排序一段一段照数比，前面的段都一样的，段少的在前：`j2` 在 `j10` 前面，`j2` 在 `j2.1` 前面，`j2.9` 在 `j10` 前面。
- 短，她写得对；头要找子会话，看 `job.started` 里的会话编号。

**时刻** `Timestamp`：UTC，到毫秒。

- 存的是从 `1970-01-01T00:00:00.000Z` 起的毫秒数，1970 年以前是负的（`unix_millis()`）。
- 只收 0000 年到 9999 年：`from_unix_millis` 收 −62167219200000（`0000-01-01T00:00:00.000Z`）到 253402300799999（`9999-12-31T23:59:59.999Z`），超出的得到空的。所以写出去永远是 24 个字符。
- JSON 里是字符串：`2026-09-25T07:04:05.123Z`。写成数字的读不进来。
- 公历，照 Howard Hinnant 的换算，不用日期库。闰年：能被 4 整除、不能被 100 整除的，或者能被 400 整除的；0000 年也照这个算，是闰年。
- 先后照毫秒数比。

**时区** `UtcOffset`：比 UTC 早多少分钟，东边是正的。

- `from_minutes(m)` 只收 −840 到 840（−14:00 到 +14:00），超出的得到空的；`minutes()` 取回分钟数。
- 写成 `UTC+09:00`、`UTC-05:30`；零时区写 `UTC+00:00`，字数固定。
- 不进 JSON。内核不读本机的时区设置：时区由执行器送进来，夏令时一换，送进来的就跟着变。

**当地钟点** `Timestamp::local_hour(offset)`：这个时刻在那个时区落在哪一个小时，写成这个小时的起止，例如 `Fri 2026-09-25 16:00–17:00`（施工 1-13 补）。

- 星期写三个字母：`Sun`、`Mon`、`Tue`、`Wed`、`Thu`、`Fri`、`Sat`（1970-01-01 是星期四）。
- 日期写成 `年-月-日`，年补足四位，月、日两位；二十四小时制，起止的小时都是两位、分钟都写 `00`，中间是连接号 `–`（U+2013）：同一个小时里字节不变。
- 23 点写 `23:00–24:00`，日期还是这一天。
- 加上时区跨出 0000 年到 9999 年的（0000 年初在西边的时区、9999 年末在东边的时区），年写不成四位：`-001-12-31`、`10000-01-01`（没有测试证实）。
- 环境那一块事实的 `time`、`timezone` 两格用它和 `UtcOffset`（`kernel/request.md`）。

**「谁」** `By`：JSON 里用 `kind` 分开八种。

| `kind` | 是谁 | 其余几格 | 例子 |
|---|---|---|---|
| `person` | 有账号的人 | `account`：账号 | `{"kind":"person","account":"alice"}` |
| `external` | 通讯平台上的人，没有账号，由桥担保 | `venue`：在哪个场所说的；`id`：平台上的身份 | `{"kind":"external","venue":"qq:group:123456","id":"qq:10086"}` |
| `model` | 模型：它的回复，连同里面的工具调用 | `endpoint`：经哪个供应商；`model`：模型 | `{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}` |
| `tool` | 一次工具调用：执行时引起的 | `call_id`：调用编号 | `{"kind":"tool","call_id":"call_44_1"}` |
| `module` | 模块，包括扩展 | `id`：模块 | `{"kind":"module","id":"memory"}` |
| `session` | 另一个会话 | `id`：会话编号 | `{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}` |
| `harness` | 别的 harness：经 `miyu ask --from` 发来的话（`agents.md` 第十一条，施工 7-1） | `name`：它自己报的名字 | `{"kind":"harness","name":"claude-code"}` |
| `kernel` | 内核自己 | 没有 | `{"kind":"kernel"}` |

几格的写法照上面的类型：`account` 是 `AccountId`，`venue` 是 `VenueId`，`id` 依次是 `ExternalId`、`ModuleId`、`SessionId`，`endpoint` 是 `ProviderId`，`model` 是 `ModelName`，`call_id` 是 `CallId`，`name` 是 `HarnessName`。

`harness` 的 `name` 是对方自己报的，不可信，照 `external` 的做法只管写法：短名字的规则，1 到 128 字节、没有控制字符，读的时候不合的报错。收的那一边（协议的 `from`，施工 7-10，`protocol.md` 的 `session.send` 第 6 条）先去掉控制字符、截到 128 字节以内（不切断一个字），截完是空的不收；给模型看之前照模板的规矩转义（`kernel/request.md`「别的 harness 发来的话」）。

### 怎么走

**用字符串写的**：照下面的先后查，第一条不合的报出来，报的话写在引号里。

1. **短名字**（`CommandId`、`VenueId`、`ExternalId`、`ProviderId`、`ModelName`、`HarnessName`）：内核不解读，冒号、斜杠、中文都行。
   1. 空的：「不能是空的」。
   2. 超过 128 个字节：「at most 128 bytes」。
   3. 有控制字符（Unicode 的 Cc 类，例如换行、`\u0007`）：「no control characters」。
2. **路径里的名字**（`AccountId`、`ModuleId`、`DriverFamily`、`FactKind`）：三个平台都能当目录名。
   1. 空的：「must not be empty」。
   2. 第一个字符不是 `a` 到 `z`（大写、数字、中文都不行）：「must start with a lowercase letter」。
   3. 有 `a-z`、`0-9`、`-`、`_` 以外的字符（空格、大写、点都不行）：「only lowercase letters, digits, - and _」。
   4. 超过 32 个字符：「at most 32 characters」。
   5. 正好是 Windows 的保留名：`con`、`nul`、`aux`、`prn`、`com1` 到 `com9`、`lpt1` 到 `lpt9`，一共 22 个：「a reserved name on Windows」。
3. **会话编号**：UUID 的标准写法，只查写法，不查是不是第 7 版。
   1. 不是正好 36 个字节：「must be 36 characters」。
   2. 从左往右，第 9、14、19、24 个字符要是 `-`：「characters 9, 14, 19 and 24 must be -」；别的要是 `0-9a-f`：「only lowercase hex digits」。先碰到哪个不合的字符，报哪一句。
4. **内容哈希**：
   1. 不以 `sha256:` 开头（`SHA256:` 也不行）：「must start with sha256:」。
   2. 后面不是正好 64 个字节：「needs 64 digits after sha256:」。
   3. 后面有 `0-9a-f` 以外的：「only lowercase hex digits」。
5. **媒体类型**：
   1. 没有 `/`：「write it as type/subtype」。
   2. 在第一个 `/` 处切成两截，两截都不能空，只用小写字母、数字和 `!#$&^_.+-`；第二个 `/` 算在子类型里，所以 `image/png/x` 也不合：「only lowercase letters, digits and !#$&^_.+-」。
   3. 超过 127 个字符：「at most 127 characters」。
6. **文件名**：
   1. 空的：「must not be empty」。
   2. 超过 255 个字节（`报` 占三个字节，86 个就超了）：「at most 255 bytes」。
   3. 有控制字符、`/` 或 `\`：「no control characters, / or \」。
   4. 正好是 `.` 或 `..`：「must not be . or ..」。
7. **事件种类**：
   1. 空的：「must not be empty」。
   2. 超过 128 个字节：「at most 128 bytes」。
   3. 照 `.` 切成段，一段一段查：第一个字符要是 `a` 到 `z`，空的段也不行（`.user`、`message..user`、`message.9user`）：「every part must start with a lowercase letter」；后面只用 `a-z`、`0-9`、`_`、`-`：「only lowercase letters, digits, _ and -」。
   4. 一个 `.` 都没有：「at least two parts separated by dots」。
   5. 模块自己的种类写成 `ext.<模块>.<种类>`，这是约定，这里不查。

**序号和回合编号**：

8. 读 JSON 时要是整数。0 报「bad seq: starts at 1 (got "0")」，回合编号也报这一句。负数、小数（`1.0`）、字符串（`"1"`）照 serde_json 的原话报错。

**调用编号**（`CallId::parse`，JSON 里读的时候也走它）：只认内核自己写出去的样子。

9. 不以 `call_` 开头：「must start with call_」。
10. 在后面的第一个 `_` 处切不成两截：「write it as call_<seq>_<index>」。
11. 序号那一截要是十进制的正整数，全是数字，不带正负号，不以 `0` 开头，不超出 `u64`：不合的「seq must be a decimal number from 1」。`call_044_1`、`call_0_1`、`call_+4_1` 都是这一句。
12. 第几个那一截同样查，还不能超出 `u32`：不合的「index must be a decimal number from 1」。`call_44_01`、`call_44_0`、`call_44_4294967296` 是这一句；`call_44_1_2` 的第几个是 `1_2`，也是这一句。

**时刻**（`Timestamp::parse`，JSON 里读的时候也走它）：只认一种写法 `YYYY-MM-DDTHH:MM:SS.mmmZ`。

13. 不是正好 24 个字节：「must be 24 characters, like 2026-09-25T07:04:05.123Z」。不带毫秒的、写成 `+08:00` 的都是这一句。
14. 第 5、8 个字符不是 `-`，第 11 个不是 `T`，第 14、17 个不是 `:`，第 20 个不是 `.`，第 24 个不是 `Z`（小写 `z`、空格都不行）：「write it like 2026-09-25T07:04:05.123Z」。
15. 年、月、日、时、分、秒、毫秒照这个先后查，有数字以外的（例如 `+026`）：「date and time must be digits」。
16. 月不在 1 到 12，日是 0 或者超过那个月的天数（2 月 29 日只在闰年有）：「no such day」。
17. 时超过 23、分超过 59、秒超过 59（没有闰秒）：「no such time」。

**当地钟点**：

18. 先把时刻加上时区的分钟数，再照 UTC 的办法算日期、小时、星期：跨日、跨月、跨年、闰日、1970 年以前、负的时区、差半小时的时区都照这一条。例如 `2026-12-31T20:30:00.000Z` 在 +09:00 是 `Fri 2027-01-01 05:00–06:00`，`2026-09-25T00:30:00.000Z` 在 −03:30 是 `Thu 2026-09-24 21:00–22:00`，`2026-09-25T14:59:59.999Z` 在 +09:00 是 `Fri 2026-09-25 23:00–24:00`。

**「谁」**：

19. 读：先把整块原样读下来，看 `kind`。
    1. 不是 JSON 对象：serde_json 的原话（`expected a map`）。
    2. 没有 `kind`：「missing field `kind`」。
    3. `kind` 不是字符串：serde_json 的原话（`invalid type`）。
    4. 认识的七种照那一种读其余几格，缺了、写法不对的照那一格报，例如「missing field `account`」「bad account: …」「bad harness name: …」。`kernel` 不看别的格。
    5. 认识的种类多出来的格不管：读进内存时丢掉，写出去不再有（日志里的原文留着，`kernel/events.md`）。
    6. 不认识的种类：整块原样留着（`By::Unknown`），写出去一字不差，空格、数字的写法都不变。
20. 写：`kind` 在最前，其余几格照上表的先后；不认识的照原文写。
21. 这一格取自连接，不取自正文：内核照执行器交进来的记。现在连上核心的只有本机，本机连上来的一律是管理员，`{"kind":"person","account":"admin"}`（`crates/miyu-endpoint/src/sessions.rs` 的 `admin`，`protocol.md`）。

**用得上 `by` 的地方**：

| 哪里 | 看什么 | 见 |
|---|---|---|
| 撤销 | 跟着撤掉的几轮一起去掉的消息，只算 `person` 发来的 `message.user` | `kernel/history.md` |
| 事实注入 | 和有效历史里同一个 `by`、同一个类别的最近一块比，一样的不再注入 | `kernel/request.md` |
| 接着写被打断的回复 | 最后一块 `reply_cut` 事实要是 `kernel` 记的 | `kernel/request.md` |
| `miyu undo` 的回应 | 引起那一轮的是 `person` 发来的 `message.user`，才写出那句话的第一行 | `protocol.md` |
| 子代理的回报 | `child.reported` 的 `by` 要是那个子会话（`session`） | `kernel/history.md`（施工 7-1） |

**内容哈希怎么算**：

22. `ContentHash::of(内容)`：SHA-256，写成 `sha256:` 加 64 位小写十六进制。空的内容是 `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。
23. `hex()`：去掉 `sha256:` 的那 64 位。blob 的文件名用它：Windows 的文件名里不许有冒号。
24. `Hasher`：一段段喂（`update`），喂完（`finish`）和 `ContentHash::of` 整份一次算的一样，用不着把整份内容放进内存；什么都没喂的，是空内容的哈希。读文件的工具边读边用它。

**任务编号**（`JobId::parse`，JSON 里读的时候也走它，施工 7-1）：只认内核自己写出去的样子，报错里叫它 `job id`。

25. 不以 `j` 开头（`J1` 也不行）：「must start with j」。
26. 后面照 `.` 切成段，每一段要是十进制的正整数，全是数字，不带正负号，不以 `0` 开头，不超出 `u64`：有一段不合的「needs decimal numbers from 1 after j, joined by dots」。`j`、`j0`、`j01`、`j+1`、`j1x`、`j18446744073709551616`、`j1.0`、`j1.`、`j.1`、`j1..2`、`j1.01`、`j1.x` 都是这一句（施工 7-1 补）。

### 出错

写法不对的，`parse` 交回 `FormatError`：读的是什么（上面「报错里叫它」那一格，序号是 `seq`，调用编号是 `call id`，时刻是 `time`）、错在哪（上面引号里的话）、读到的原文（超过 80 个字符的只留前 80 个，后面加 `…`；写的时候带着双引号，照 Rust 的调试写法转义，见 `kernel/events.md`「出错」）。写成一句：

```text
bad session id: must be 36 characters (got "x")
```

从 JSON 里读的，这一句成了 serde_json 的报错，后面带着第几行第几列（行列没有测试证实）。报错是英文（施工 4-9 再补四中：原来是中文）：给查问题的人看，写进运行日志，不给模型（`kernel/events.md`「出错」）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/id/tests.rs` | 图纸上的例子读写一字不差（`samples_from_the_drawing_round_trip`、`names_from_the_drawing_round_trip`）；会话编号（`session_id_must_be_lowercase_uuid_text`）；短名字（`command_id_is_short_printable_text`、`short_names_are_opaque_but_bounded`）；路径里的名字（`account_is_like_a_linux_login_name`、`module_driver_and_fact_names_follow_the_account_rule`）；内容哈希的写法和算法（`content_hash_is_sha256_in_lowercase_hex`、`content_hash_of_known_contents`、`hashing_piece_by_piece_is_the_same_as_all_at_once`）；媒体类型、文件名、事件种类各自的规则；序号和回合编号（`seq_starts_at_one`、`turn_id_reads_like_a_seq`）；调用编号第 9 到 12 条（`call_id_accepts_only_what_the_kernel_writes`）；任务编号第 25、26 条和排序（`job_id_accepts_only_what_the_kernel_writes`、`job_ids_sort_by_number`，施工 7-1；几段的、`under`、`last`，施工 7-1 补）；报错的样子和 80 个字符（`error_says_what_why_and_what_was_read`、`long_text_in_errors_is_cut`） |
| `crates/miyu-kernel/src/time/tests.rs` | 图纸上的例子；几个标准时刻和两头的界（`well_known_moments`、`years_outside_0000_to_9999_are_refused`）；闰年；1600 年到 2400 年一天一天数过去和换算对得上；第 13 到 17 条每种坏写法；时区的写法和范围；第 18 条当地钟点（`the_local_hour_is_the_wall_clock_to_the_hour`；23 点到 24 点 `the_last_hour_of_a_day_ends_at_24`，施工 1-13 补）；七天的写法 |
| `crates/miyu-kernel/src/origin/tests.rs` | 八种读写一字不差、各读成自己那一种；不认识的一字不差；认识的种类多出来的格不管；第 19 条的几种坏写法；`harness` 的名字照短名字的规则（施工 7-1） |
| `xtask/src/purity.rs`（门禁「纯逻辑」） | 内核的 `src/` 里没有 `SystemTime`、`Instant`：不读时钟 |

### 出处

- `03-事件模型.md` 第二节：公共字段、「`by` 的写法」、「编号和时间的写法」、读和写一样严。
- `02-内核.md` 第四节：纯逻辑内核，不读时钟；第九节不变量 1：序号连续递增。
- `01-架构.md` 第六节：身份取自连接，正文里自称的不作数。
- `06-多用户与身份.md` 第二节（账号、外部身份）、第五节（权限按这一步是谁要求的判）。
- `07-存储.md` 第五节：blob 的文件名是去掉 `sha256:` 的 64 位。
- `08-上下文投影.md` 第五节「环境和状态的事实怎么写」：时间到小时、时区写成 `UTC+09:00`。
- `agents.md`「对外的样子」：任务编号 `j1`、`j2`，`by` 多一种 `harness`（施工 7-1）；子会话派的带上它在父会话里的编号（施工 7-1 补）。

### 还没有的

- 到分钟的当地时间：通讯平台、桌面语音这类场所用（`08-上下文投影.md` 第五节「环境和状态的事实怎么写」的「还没有的」）。
- `external` 这种 `by` 读得懂，还没有哪里造：通讯平台（`18-通讯平台.md`）做到时才有。`session` 由子会话造（施工 7-5 起），`harness` 由 `session.send` 的 `from` 造（施工 7-10）。
- 按 `by` 判权限：现在只有本机的管理员，执行前的链不看 `by`（`06-多用户与身份.md` 第五节）。
- 远程连接、成员账号、扩展身份（`06-多用户与身份.md` 第二节）。
