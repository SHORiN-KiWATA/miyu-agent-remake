## 核心协议

### 是什么

头和核心之间说的话：一个连接上一行一条 JSON-RPC 2.0。连上先握手，之后能造会话、列出会话、说话、打断、撤销、恢复，订阅会话的事件流。连接从哪来不管：本机的套接字、命名管道（`ipc.md`），测试里的内存管道。

撤销、恢复的回应另写一页：`protocol/undo.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/lib.rs` | `Core`：核心的家底（数据根、资源目录、请求模型的端口、工具目录、系统的家目录、沙盒的助手（施工 5-4 上）、管理员、本机令牌、会话表）；数着几个连接；空不空闲；停下全部会话 |
| `crates/miyu-endpoint/src/listen.rs` | `run`：在监听器上一个个接连接 |
| `crates/miyu-endpoint/src/connection.rs` | `serve`：一个连接，读写分开；握手以前拦住；`subscribe`、`unsubscribe` |
| `crates/miyu-endpoint/src/wire.rs` | 读一行、认成请求、回应写成一行 |
| `crates/miyu-endpoint/src/hello.rs` | 握手 |
| `crates/miyu-endpoint/src/methods.rs` | 握手以后的方法 |
| `crates/miyu-endpoint/src/sessions.rs` | 会话表：造会话、找会话、载入；工作目录太宽的退回工作区 |
| `crates/miyu-endpoint/src/list.rs` | `session.list` |
| `crates/miyu-endpoint/src/subscriptions.rs` | 订阅：每个订阅一个转发任务，推 `event`、`resync` |
| `crates/miyu-endpoint/src/undo.rs` | 撤销、恢复的回应里给人看的几样（`protocol/undo.md`） |
| `crates/miyu-endpoint/src/refusal.rs` | 拒绝：错误码、原因码、中英文的话 |

### 对外的样子

#### 一行一条

- 每条消息是一行 JSON，以 `\n` 结尾。读的时候 `\r\n` 也认；连接关之前最后没带换行的那一行也认。
- 一行最长 1 MiB（1,048,576 字节，不算最后的 `\n`；`\r\n` 结尾的，`\r` 算在里面）。
- 核心写出的每一行是紧凑的 JSON，加 `\n`。回应里各层对象的格照名字的字母先后排（例如 `id`、`jsonrpc`、`result`）；推送的外层照 `jsonrpc`、`method`、`params` 的先后，`params` 里先写 `session`（下面「推送」）。

#### 请求

读到的一行照这个先后认，先对上的算：

| 这一行 | 结局 |
|---|---|
| 不是 JSON | `parse_error` |
| 是 JSON，不是对象 | `invalid_request` |
| `method` 没有，或者不是字符串 | `invalid_request` |
| `jsonrpc` 不是 `"2.0"` | `invalid_request` |
| `params` 写了，不是对象也不是数组 | `invalid_request` |
| 没有 `id` | 通知：不回应，不理 |
| `id` 不是字符串（数字、`null`……） | `invalid_request` |
| `id` 不合命令编号的写法：1 到 128 字节，不含控制字符 | `invalid_request` |
| 别的 | 一条请求：`id` 就是命令编号；没写 `params` 的当空对象 |

- 认不成请求的，回应里的 `id`：这一行的 `id` 是字符串或数字的照原样带回，别的（没有、`null`、对象……）写 `null`。
- 头发来的通知现在一种都不认。
- `params` 是数组的：`bad_params`（施工 4-9 再补三上）。方法都照名字取参数；照位置读的话，回应找不到该交给哪个订阅，破了「先见结果，后见回应」。
- 参数里不认识的格不理。标着「可以不写」的格，写 `null` 等于没写；标着「不写是……」的格写 `null`，是 `bad_params`。

#### 回应

接受的，`result` 里是这个方法的回应：

```json
{"id":"c7","jsonrpc":"2.0","result":{"cwd":"/home/me/src/miyu","events":[41]}}
```

拒绝的，`code` 是 JSON-RPC 的错误码，`message` 照握手时的语言写，`data.reason` 是给程序看的原因码（「出错」一节）：

```json
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"没有这个会话。"},"id":"c7","jsonrpc":"2.0"}
```

#### 握手 `hello`

| 参数 | 类型 | 说明 |
|---|---|---|
| `protocol` | 两个非负整数 `[最低, 最高]` | 头支持的主版本范围，必写 |
| `head` | `{"kind": 字符串, "version": 字符串}` | 头的种类和版本，必写，只记进运行日志 |
| `locale` | 字符串，可以不写 | `zh` 开头的（区分大小写），给人看的话说中文；别的、没写的说英文 |
| `caps` | 对象，不写是 `{}` | 现在只看 `input`（布尔，不写是 `false`）：这个头能不能让人输入。别的格不理 |
| `token` | 字符串，可以不写 | 本机令牌（`ipc.md`）；不写的过不了第 3 条 |

回应：

| 格 | 值 |
|---|---|
| `protocol` | 选定的主版本：`1`，核心只支持这一个 |
| `core` | `{"version": <核心的版本号>}` |
| `account` | 你是谁：管理员的账号，核心里固定是 `admin`（`core.md`） |
| `sandbox` | 这台机器上的沙盒能不能用（核心起来时探的，`sandbox.md`）：`{"usable": true}`，或者 `{"usable": false, "reason": <原因>}`。原因是 `helper_missing`（主程序旁边没有助手）、`helper_failed`（助手跑不起来、超时、说的读不懂）、`no_mechanism`（探成了，这台机器上却没有能用的手段）之一（施工 5-4 下） |

1. 参数读不成（缺了必写的格、哪一格类型不对）：`bad_params`，连接不断。
2. `1` 不在 `[最低, 最高]` 里：`protocol_mismatch`，回完断开。
3. 令牌没带、不对：`bad_token`，回完断开。长短要一样，每个字节都比；比到哪一个不一样都用一样长的时间。
4. 过了：这个连接就是管理员。它发的命令都记成管理员发的（`by` 是 `{"kind":"person","account":"admin"}`），命令引起的事件，`cause` 是请求的 `id`（`kernel/events.md`）。
5. 握手以前：别的方法一律 `hello_first`，连接不断；读不懂的行照「请求」的表回；通知不理。
6. 握手以前的拒绝说英文；`hello` 本身被拒的，话照这一次报的 `locale` 说（读得出来的话，施工 4-9 再补三上）。握手以后再发 `hello`：照样从第 1 条查起；过了，换成这一次报的语言和能力；这一次被拒的，话照这一次报的语言说，没断开的还是上一次握手的语言和能力。
7. 握手的时限：连上 10 秒还没握手成的，断开（施工 4-9 再补三上）。不然一个连上不说话的本机进程，能让核心一直不空闲退出。

#### 方法

握手以后认这几个，别的方法回 `unknown_method`：

| 方法 | 做什么 |
|---|---|
| `session.create` | 造会话 |
| `session.list` | 列出会话 |
| `session.send` | 说一句话 |
| `session.interrupt` | 打断在进行的回合 |
| `session.revert`、`session.unrevert` | 撤销、恢复最近一次撤销（`protocol/undo.md`） |
| `subscribe`、`unsubscribe` | 订阅、取消订阅会话的事件流 |

带 `session` 的，它要合会话编号的写法：UUID 的标准写法，小写十六进制，8-4-4-4-12；不合的 `bad_params`。找会话照下面「会话表」。

**`session.create`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串，可以不写 | 照哪个人格造；不写是出厂的 `engineer` |
| `cwd` | 字符串，必写 | 头的工作目录，人看到的那种写法，例如 `~/src/miyu` |
| `oneshot` | 布尔，不写是 `false` | 一次性的：`miyu ask` 开的写 `true`，记进 `session.created` |
| `dirs` | 字符串的数组，可以不写 | 加进来的目录：和工作区一样能读能写（「加进来的目录」（施工 5-10 上））。不写是没有 |

回应：`session` 新会话的编号；`events` 是 `[1]`，就是 `session.created` 那一条；`cwd` 是会话实际在哪个目录里干活（「工作目录太宽」）。

1. 编号是 UUIDv7：前 48 位是造的这一刻（毫秒），同一个核心造的照先后排。
2. 属主是管理员，场所是 `local`，权限从「工作区，不只读」开始；有没有人能确认，照这个连接握手时的 `caps.input`；环境是核心所在机器此刻的时区偏移（到分钟）和实际干活的目录。
3. `session.created` 落了盘才回应。
4. 同一个命令编号再发：记着最近 1024 个造会话的编号，是其中之一的，交回上一次造的那一个，不再造；`cwd` 照这一次报的算。核心重启以后，第一次造会话时，从最新的 1024 个会话的 `session.created` 里把编号补回来（它的 `cause` 就是造会话的命令编号，施工 4-9 再补三上），在阻塞线程里读。
5. 人格的编号不合写法（小写英文字母开头，只有小写字母、数字、`-`、`_`，最长 64 字节）：`bad_params`。人格的目录 `personas/<编号>/` 不存在：`unknown_persona`。资源目录里别的读不了（人格目录里的文件、随核心附带的 `core/` 下的字，安装坏了）：`internal_error`（施工 4-9 再补三上）。别的造不成（策略造不出来、磁盘上建不成、`session.created` 没落盘）：`internal_error`，原因记进运行日志。

**`session.list`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `oneshot` | 布尔，不写是 `false` | `true` 只要一次性的 |
| `limit` | 非负整数，可以不写 | 最多几个；不写是全部，`0` 是一个都不要 |

回应：`{"sessions":[{"oneshot":<布尔>,"session":"<编号>"}, …]}`。

1. 只列管理员的会话，从新到旧：照编号倒着排，编号照造的先后。
2. 读每个会话日志的第一条，只读不写。跳过：目录名不合会话编号写法的、没有日志的（第一行还没写完的也算没有）、第一条读不出来的（记一条运行日志）、第一条不是 `session.created` 的。
3. `limit` 数的是列进去的。
4. 读不了放会话的目录：`internal_error`。

**`session.send`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 要说的话，照原样成一块文字；空的一块都没有 |
| `urgent` | 布尔，不写是 `false` | 急着插话 |
| `cwd` | 字符串，可以不写 | 头现在的工作目录 |
| `dirs` | 字符串的数组，可以不写 | 加进来的目录（施工 5-10 上）。不写的照旧；写了的，这一句以后开的回合照它，空的就是没有 |

回应：`events` 是 `[<这一句 message.user 的序号>]`；`cwd` 是收下这一句的 `cwd` 以后，会话实际在哪个目录里干活。

1. 没有回合在进行的，这一句开一轮；有的，排队，`urgent` 的插进下一步（`kernel/session.md`）。
2. 开的那一轮，`turn.started` 的 `cause` 是这一条的 `id`：头照它认出自己的那一轮。
3. `text` 是空的：`empty_message`。先找会话，找不到的回的是找不到。

**`session.interrupt`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `queued` | `"send"` 或 `"return"`，必写 | 排着队的消息：打断以后马上发，还是退回来 |

回应：`events`，这一次追加的全部事件的序号。没有回合在进行：`not_running`。

**`subscribe`、`unsubscribe`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `stream` | 字符串，必写 | 现在只有 `events`，别的 `bad_params` |

回应是空对象 `{}`。

1. 订阅：从这一刻起的推送都转给这个连接，以前的不补。没在跑的会话先载入。
2. 这个连接已经订阅着这个会话、还在推的，还是那一个；停了推的（掉了队、会话停了），换一个新的。
3. 取消订阅：停掉转发。不载入会话，没订阅过的也回 `{}`。
4. 订阅的回应不排在推送后面：会话正忙的，回应之前可能已经有这个会话的推送。

#### 推送

| 方法 | `params` | 什么时候 |
|---|---|---|
| `event` | `{"session": <编号>, "event": <事件>}` | 订阅着的会话的每一条事件，一条一个 |
| `resync` | `{"session": <编号>, "stream": "events"}` | 读得太慢，掉了队：这个订阅停了 |

`event` 里的事件照原样嵌进去：落了盘的照 `kernel/events.md` 的写法，样本在 `docs/designs/samples/events/`；瞬时的 `model.delta`、`tool.progress`、`status` 没有 `seq`，样本在 `docs/designs/samples/transient/`。

### 怎么走

**一个连接**

1. 请求一条条办：上一条的回应放进了写队列（订阅着的会话的，交给了它的转发任务，下面「先见结果，后见回应」），才读下一行。要等的（`session.send` 等落盘，撤销等改完文件，载入会话）等着的时候，这个连接上的下一条也等着。
2. 读和写分开：写的一头从写队列里一行行写出去，队列最多攒 256 行。写不出去就停；读的一头往写队列放回应、放不进去时发现，断开（交给转发任务的那一条放得进，要到再下一条）。
3. 断开：一行太长的，回 `parse_error`（`id` 是 `null`）；`protocol_mismatch`、`bad_token` 回完；对方关了、读出错。
4. 断开以后，这个连接的订阅都停了。会话照常跑：头走了，在跑的那一轮照样跑完。
5. 连接从接上起就算连着，握没握手都算，断开才不算；没握手的最多连 10 秒（握手第 7 条）。

**先见结果，后见回应**

1. 方法的回应，`params.session` 这个会话在这个连接上有订阅的，交给这个订阅的转发任务：它先把已经到了的推送都放进写队列，再放回应。会话先推送、后回应（`session/actor.md`），回应到的时候，这条命令产生的推送一定已经到了。
2. 别的回应直接放进写队列：没订阅的会话的；`params` 是数组的；`hello`、`subscribe`、`unsubscribe` 的；握手以前的拒绝；读不懂的行的。
3. 订阅停了推（掉了队、会话停了），转发任务接着替这个会话转回应，直到这个订阅被取消、被新的换掉，或者连接断了。

**慢和掉队**

1. 头读得慢：写队列满了，转发任务等着，不再从会话那里拿。会话给每个订阅最多攒 1024 份没读走的推送，再多就掉了队。核心和会话都不等这个头。
2. 掉了队：推一条 `resync`，这个订阅停了，之后不再推；回应照样到。头重新 `subscribe`，从那一刻起再推，掉了的不补。
3. 会话停了：这个订阅也停了，推一条 `resync`（施工 4-9 再补三上）：头重新订阅，会话照「会话表」重新载入。

**同一个命令编号**

1. 会话接受过的编号再来（每个会话记最近 1024 个）：照上一次回应，不再产生事件；那几条事件还没落盘的，落了盘再回。载入时从日志里认回来：`cause` 是这个编号的那几条就是回应的 `events`，和当时的回应不一定一样。
2. 被拒绝的编号不记：再发，重新判。
3. `session.create` 另记，见上。

**会话表**

1. 在跑的会话直接用；没在跑的从磁盘载入。载入时整张表锁着：两个连接同时说给同一个没在跑的会话，只载入一次、只起一个。
2. 没有这个会话的日志：`session_not_found`。别的载入不了（日志、策略快照坏了、读不了）：`session_broken`，原因记进运行日志。
3. 发命令、订阅时会话已经停了（写不进去、出了 bug）：从表里拿掉，回 `session_stopped`；下一次用到再载入。
4. `session.send` 带着 `cwd`、`dirs`，和这个会话上一次报的不一样：照「工作目录太宽」重新定实际干活的目录，送进会话，到下一个边界才注入（`kernel/request.md`）；会话这时停了的，回 `session_stopped`。不带的、一样的，照旧。
5. 载入时没有报来的 `cwd`（`session.interrupt`、`session.revert`、`session.unrevert`、`subscribe` 载入的）：照日志里最后一条带 `cwd` 的 `turn.started`（加进来的目录照最后一条 `turn.started` 的 `dirs`，没有就是没有（施工 5-10 上）），没有就照 `session.created` 的，都没有（之前的日志）才当报来的是 `~`，退回管理员的工作区（施工 4-9 再补三上）。核心重启以后撤销，路径照会话真正的目录写短。
6. 会话一直留在表里，直到核心退出、停下全部会话，或者用到时发现它停了。

**工作目录太宽**

`session.create`、`session.send` 报来的 `cwd`，照这几条定会话实际在哪个目录里干活：

1. 去掉前后空白是 `~` 的：太宽。
2. 换成真实的位置（`fs.md`）：`~`、`~/…` 接在系统的家目录上（家目录先换成真实的位置），相对的接在 `/` 上，链接顺着找到本体。换不成的：照原样用，说不清它宽不宽，用到时工具自己报错。
3. 真实的位置是系统的家目录、是根目录、包含数据根，或者落在数据根里却不在管理员的工作区里：太宽。
4. 太宽的，退回管理员的工作区 `<数据根>/home/admin/workspace`；它还没有的，当场建（建不成的记一条运行日志，照样用它）。
5. 不太宽的，照头报的原样用，不换成真实的位置。

**加进来的目录**（施工 5-10 上）

`session.create`、`session.send` 报来的 `dirs`，每一条照「工作目录太宽」的第 1 到 3 条判：是 `~`，或者换成真实的位置以后是系统的家目录、根目录、包含数据根、落在数据根里的，太宽。有一条太宽，整条命令都不收，回 `dir_too_wide`，什么都没写：说的话照拒绝的规矩是固定的一句，不带是哪一条，头知道自己报了哪几条。不太宽的照报的原样用；换不成真实位置的也照原样，边界表里那一片不算（`session/guard.md`）。

**空闲和停下**

- `Core::idle`：没有连接，会话表里也没有哪个会话忙着：在跑回合、回合结束了 `turn.ended` 还没落盘、在改回文件，都算忙；停了的会话不算。核心照它空闲退出（`core.md`）。
- `Core::stop_sessions`：有计划地停下表里全部的会话，跑到一半的回合记成「重启了」，下次载入接着干（`kernel/session.md`）；会话表清空。
- 接不了连接（例如打开的文件太多）：歇 100 毫秒再接，不空转。

### 出错

JSON-RPC 自己的几种照它的标准码；Miyu 的一律 `-32010`，原因写在 `data.reason`。

| 原因码 | `code` | 什么时候 |
|---|---|---|
| `parse_error` | -32700 | 不是 JSON；一行太长（之后断开） |
| `invalid_request` | -32600 | 是 JSON，不是请求（「请求」的表） |
| `unknown_method` | -32601 | 握手以后，没有这个方法 |
| `bad_params` | -32602 | 参数读不成、类型不对；会话编号、人格编号不合写法；`turn` 写了 0；`stream` 不是 `events` |
| `internal_error` | -32603 | 造会话时装坏了、磁盘上建不成、`session.created` 没落盘；列会话时读不了放会话的目录、崩了 |
| `hello_first` | -32010 | 握手以前发了别的方法 |
| `protocol_mismatch` | -32010 | 头支持的主版本里没有 1（之后断开） |
| `bad_token` | -32010 | 本机令牌没带、不对（之后断开） |
| `unknown_persona` | -32010 | 造会话时人格的目录不存在 |
| `session_not_found` | -32010 | 没有这个会话 |
| `session_stopped` | -32010 | 会话停了：写不进去、出了 bug |
| `session_broken` | -32010 | 会话载入不了：日志、策略快照坏了、读不了 |
| `empty_message` | -32010 | `session.send` 的 `text` 是空的 |
| `dir_too_wide` | -32010 | 加进来的目录太宽（「加进来的目录」（施工 5-10 上）） |
| `not_running` | -32010 | 打断时没有回合在进行 |
| `turn_running` | -32010 | 撤销时有回合在进行 |
| `unknown_turn` | -32010 | 要撤的那一轮不在有效历史里：没有，或者已经撤掉了 |
| `compacted` | -32010 | 要撤的那一轮已经压缩进摘要了 |
| `nothing_to_unrevert` | -32010 | 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过 |
| `nothing_to_revert` | -32010 | 不写 `turn` 的撤销，一轮都没有 |
| `restoring` | -32010 | 正在改回文件时来的命令。兜底：会话改完文件才接下一个命令，照常碰不到 |

- 从 `empty_message` 起的八个是内核拒命令时给的原因码（`kernel/session.md`）。
- 内核还有六个原因码，现在没有方法碰得到：`unknown_level`、`not_asking`、`unknown_decision`、`no_rule`、`unexpected_reason`、`bad_answer`。它们没有配话，说的是最后那一句「被拒绝了」。

运行日志（目标 `miyu::endpoint`，`log.md`）：

| 级别 | 这件事 | 什么时候 |
|---|---|---|
| `DEBUG` | `connected` | 接上一个连接 |
| `INFO` | `connected head=… version=… protocol=1` | 握手过了 |
| `WARN` | `protocol mismatch head=… low=… high=…`、`bad token head=…` | 握手被拒、断开 |
| `DEBUG` | `request method=…` | 每一条请求 |
| `WARN` | `line too long, closed` | 一行太长 |
| `INFO` | `disconnected` | 握过手的连接断了 |
| `WARN` | `accept failed error=…` | 接不了连接 |
| `ERROR` | `connection task failed error=…` | 一个连接的任务崩了 |
| `WARN` | `create failed error=…`、`load failed session=… error=…` | 造不成、载入不了 |
| `WARN` | `lagged, resync session=…` | 掉了队 |
| `INFO` | `session stopped, resync session=…` | 订阅着的会话停了（施工 4-9 再补三上） |
| `WARN` | `sessions not listed error=…`、`first event not read session=… error=…` | 列会话读不了 |
| `ERROR` | `list panicked error=…` | 列会话崩了 |
| `WARN` | `workspace not prepared kind=…` | 退回的工作区建不成 |
| `DEBUG` | `already stopped session=…` | 停下全部会话时，这一个已经停了 |

撤销、恢复的回应写不成的两行见 `protocol/undo.md`。

### 给人看的字

`message` 照握手时的语言，中文、英文各一句：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `parse_error` | 读不懂这条消息。 | The message could not be read. |
| `invalid_request` | 这不是一条请求。 | This is not a request. |
| `unknown_method` | 没有这个方法。 | There is no such method. |
| `bad_params` | 参数不对。 | The parameters are not right. |
| `internal_error` | 核心出了问题，详情在运行日志里。 | The core ran into a problem; the runtime log has the details. |
| `hello_first` | 连上以后要先打招呼（hello）。 | Say hello first after connecting. |
| `protocol_mismatch` | 头和核心的协议版本对不上，请把它们升级到同一个版本。 | The head and the core speak different protocol versions; upgrade them to the same release. |
| `bad_token` | 本机令牌不对。 | The local token is wrong. |
| `unknown_persona` | 没有这个人格。 | There is no such persona. |
| `session_not_found` | 没有这个会话。 | There is no such session. |
| `session_stopped` | 这个会话停了，详情在运行日志里；再发一次会重新载入。 | This session has stopped; the runtime log has the details. Sending again reloads it. |
| `session_broken` | 这个会话载入不了：它的日志或者策略快照坏了。 | This session cannot be loaded: its log or policy snapshot is broken. |
| `empty_message` | 消息是空的。 | The message is empty. |
| `dir_too_wide` | 加进来的目录太宽：家目录、根目录、Miyu 的数据根不能整个放行。 | An added directory is too wide: the home directory, the root and Miyu's data root cannot be opened up whole. |
| `not_running` | 没有正在进行的回合，打断不了。 | No turn is running, so there is nothing to interrupt. |
| `turn_running` | 有回合在进行，撤销不了：先打断再撤。 | A turn is running; interrupt it before undoing. |
| `unknown_turn` | 没有这一轮，或者它已经撤掉了。 | There is no such turn, or it has already been undone. |
| `compacted` | 这一轮已经压缩进摘要了，撤不回来。 | That turn is already compacted into the summary and cannot be undone. |
| `nothing_to_unrevert` | 没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。 | There is nothing to redo: nothing was undone, or a turn or compaction came since. |
| `restoring` | 正在改回文件，等它做完再来。 | Files are being restored; try again when that is done. |
| `nothing_to_revert` | 没有能撤销的回合。 | There is no turn to undo. |
| 别的 | 被拒绝了。 | Refused. |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 认请求的每一条、回应是一行、去掉行尾、太长的 |
| `crates/miyu-endpoint/tests/endpoint.rs` | 握手先行、令牌（长短也比）、版本对不上，被拒的断开；握手的回应照核心探到的报沙盒，四种（施工 5-4 下）；造会话、说话；同一个造会话只造一个；没有的会话；载入上一次运行的会话；两个连接只载入一次；照头的语言拒绝；JSON-RPC 的错误码、通知不回应；太长的断开；工作目录跟着头；不能输入的头造的会话没人确认；空消息；停了的会话下次再载入；打断时排着的接着发 |
| `crates/miyu-endpoint/tests/subscribe.rs` | 先见结果后见回应；两个会话不串；取消订阅以后不推；掉队推 `resync`、回应一条不丢、重新订阅照常推；积压时回应排在推送后面；取消订阅时已经交给转发任务的回应照样到；会话停了推 `resync`；订阅要握手、要有这个会话、只认 `events` |
| `crates/miyu-endpoint/tests/restart.rs` | 核心重启以后：不带 `cwd` 载入的会话照最后一轮的工作目录、没开过回合的照造会话时的；重发的造会话交回原来那一个 |
| `crates/miyu-endpoint/tests/edges.rs` | 不握手的到时断开、握手了的不受管；数组的 `params` 参数不对；握手被拒照它报的语言说；人格目录不存在是 `unknown_persona`、目录在而读不了是 `internal_error` |
| `crates/miyu-endpoint/tests/list.rs` | 从新到旧、只要一次性的、`limit`、参数不对、空的 |
| `crates/miyu-endpoint/tests/revert.rs` | 协议上撤销、恢复；三种拒绝的中文；`turn` 写 0 |
| `crates/miyu-endpoint/tests/workspace.rs` | 太宽的五种（`~`、家目录、根目录、数据根、数据根里面）和读不出家目录时的 `~`；项目目录、账号的工作区照旧；回应里的 `cwd`、重发的造会话 |
| `crates/miyu-endpoint/tests/dirs.rs` | 加进来的目录（施工 5-10 上）：造会话、说话时报的记进这一轮，不写的照旧、写空的就没有；太宽的五种整条命令都不收、什么都没写；核心重启以后照最后一轮的 |
| `crates/miyu-endpoint/tests/idle.rs` | 连着连接、跑着回合不空闲；停下全部会话，跑到一半的记成重启了 |
| `crates/miyu-endpoint/tests/tools.rs` | 造会话、载入时用核心的工具目录；核心的沙盒造会话、载入时都交给会话，沙盒用不了的核心上执行命令没人能确认就拒（施工 5-4 上） |
| `crates/miyu-endpoint/tests/socket.rs` | 真的套接字（Windows 上是命名管道）上握手、造会话、说话，第二个头也连得上 |

### 出处

- `04-核心协议.md` 第二节（JSON-RPC、分帧、拒绝的写法）、第三节（一次连接的全过程）、第四节（连接即身份、本机令牌）、第五节（事件流）、第六节第 1、2、4 条、第七节（慢、`resync`）、第八节（版本）、第九节「先做的几样怎么写」；P1、P2。
- `02-内核.md` 第四节（拒绝附原因码）、不变量 9（同一个编号只生效一次）。
- `06-多用户与身份.md` 第二节、U13：本机连上来的是管理员 `admin`。
- `07-存储.md` 第七节：会话按需载入。
- `11-权限与沙盒.md` 第四节：当前目录太宽。

### 还没有的

设计里有、还没做的：

- 第九节表里的其余方法：`session.fork`、`session.configure`、`session.set_permission_level`、`session.answer`、`session.compact`、`command.run`、`blob.put`、查询、账号、配置……（`04-核心协议.md` 第九节）。
- 视图流、会话列表流，`view.*`、`sessions.changed`、`config.changed` 这些推送；核心决定「显示什么」（第五节、P3）。
- 事件流重连时报出最后看到的序号、补发之后的（第七节）；队列紧张时先合并同一条目的连续增量（第七节）。
- 头发现核心比自己旧，请求它空闲时重启（第八节，`kernel.restart_when_idle`）。
- 远程连接、登录令牌、WebSocket 和它的 Origin 检查；扩展、桥当提供者，反向调用（第二节、第四节）。
- 事件流只对会话的属主和有 `events.read` 能力的扩展开放（第五节）：现在连上来的只有管理员。
- 消息结构只在 Rust 类型里定义一次，生成 JSON Schema 和 TypeScript 类型（第二节）。
- 会话空闲一段时间后 actor 退出（`07-存储.md` 第七节）。
- 头拿不到会话实际用的模型限额（窗口、最大输出）：它们只在内核内存里（`Input::Limits`，施工 6-3 上），终端界面的侧边栏因此画不出「用量 / 窗口」（2026-09-29 终端演示那边报的）。打算照会话状态的一格或者一条瞬时的「限额」事件推，造会话、载入、换模型时各一次；不进 `model.called`：窗口是配置，不是日志。
