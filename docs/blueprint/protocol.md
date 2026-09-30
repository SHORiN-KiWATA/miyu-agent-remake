## 核心协议

### 是什么

头和核心之间说的话：一个连接上一行一条 JSON-RPC 2.0。连上先握手，之后能造会话、列出会话、传附件、说话（可以带附件）、打断、撤销、恢复、重做、手动压缩、切权限级别、清空上下文、停掉派出去的任务、改标题、置顶、删除会话，订阅会话的事件流。连接从哪来不管：本机的套接字、命名管道（`ipc.md`），测试里的内存管道。

撤销、恢复、重做的回应另写一页：`protocol/undo.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/lib.rs` | `Core`：核心的家底（数据根、资源目录、请求模型的端口、工具目录、系统的家目录、沙盒的助手（施工 5-4 上）、管理员、本机令牌、会话表）；数着几个连接；空不空闲；停下全部会话 |
| `crates/miyu-endpoint/src/listen.rs` | `run`：在监听器上一个个接连接 |
| `crates/miyu-endpoint/src/connection.rs` | `serve`：一个连接，读写分开；握手以前拦住；`subscribe`（回应带会话的限额）、`unsubscribe` |
| `crates/miyu-endpoint/src/wire.rs` | 读一行、认成请求、回应写成一行 |
| `crates/miyu-endpoint/src/hello.rs` | 握手 |
| `crates/miyu-endpoint/src/methods.rs` | 握手以后的方法 |
| `crates/miyu-endpoint/src/meta.rs` | `session.set_meta` 的参数：标题去掉前后空白、量长短，`null` 是去掉标题（施工 3-8 三补） |
| `crates/miyu-endpoint/src/sessions.rs` | 会话表：造会话、找会话；工作目录太宽的退回工作区；造子会话（施工 7-5） |
| `crates/miyu-endpoint/src/sessions/found.rs` | 找会话、载入（施工 7-8 从 `sessions.rs` 挪出来：表的锁在调的一方手里） |
| `crates/miyu-endpoint/src/sessions/orphans.rs` | 载入时收掉派到一半的空子会话（施工 7-8，「会话表」第 8 条） |
| `crates/miyu-endpoint/src/sessions/delete.rs` | 会话表删会话：认出它派的子会话、停下、挪进回收处（施工 3-8 三补）；删子会话照人停掉它、父会话记回报，都在表的锁里（施工 7-8） |
| `crates/miyu-endpoint/src/spawn.rs` | 会话表交给会话的端口：造子会话、给会话发命令（施工 7-5，`session/tools.md`「派子代理」）；停下子会话、照日志看它（施工 7-4） |
| `crates/miyu-endpoint/src/list.rs` | `session.list`：标题、置顶照日志算（施工 3-8 三补） |
| `crates/miyu-endpoint/src/subscriptions.rs` | 订阅：每个订阅一个转发任务，推 `event`、`resync` |
| `crates/miyu-endpoint/src/undo.rs` | 撤销、恢复、重做的回应里给人看的几样（`protocol/undo.md`） |
| `crates/miyu-endpoint/src/attach.rs` | 附件（施工 3-9 三补）：`blob.put` 读、存；`session.send`、`session.redo` 的附件变成内容块 |
| `crates/miyu-endpoint/src/attach/kind.rs` | 认一个附件是什么：图片、PDF、别的文件，媒体类型 |
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
| `session.redo` | 重做最后一轮：撤掉它，把开它的话再发一次（施工 4-7 再补） |
| `session.compact` | 手动压缩：单开一轮只做压缩（施工 6-8） |
| `session.set_permission_level` | 切权限级别：开关只读，改常用的那一级（施工 3-8 再补） |
| `session.clear` | 清空上下文：单开一轮压成一个空的检查点，不请求模型（施工 6-8 补） |
| `job.stop` | 停掉一个后台命令或者子代理（施工 7-4） |
| `blob.put` | 传一个附件，存成 blob（施工 3-9 三补） |
| `session.set_meta` | 改标题、置顶（施工 3-8 三补） |
| `session.delete` | 删除会话：挪进回收处，留 7 天（施工 3-8 三补） |
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

回应：`{"sessions":[{"oneshot":<布尔>,"parent":<编号或 null>,"pinned":true,"session":"<编号>","title":"<标题>"}, …]}`。`parent` 是子会话的父会话，主会话写 `null`（施工 7-5，`agents.md`）。`title`、`pinned` 照日志里的 `session.meta_changed` 算（施工 3-8 三补）：有标题的才写 `title`，置顶的才写 `pinned`（写 `true`），没有的不写。

1. 只列管理员的会话，从新到旧：照编号倒着排，编号照造的先后。子会话也列，和主会话排在一起。删了的（挪进了回收处）不列。
2. 读每个会话日志的第一条，只读不写。跳过：目录名不合会话编号写法的、没有日志的（第一行还没写完的也算没有）、第一条读不出来的（记一条运行日志）、第一条不是 `session.created` 的。
3. 列进去的，再只读地把整份日志读一遍（`store.md` 第 7 条的 `read_segments`），`session.meta_changed` 一条条盖上去：写了 `title` 的换成它（空的是去掉），写了 `pinned` 的换成它；撤掉的回合里改的也算，改名不是对话的一部分。后面读不下去的（日志坏了）：记一条运行日志，照坏的那一段以前的算（一段查过了才交出来，只有一段的就当没有），照样列。现在每列一次都整份读，有了会话列表的索引再换（「还没有的」）。
4. `limit` 数的是列进去的。
5. 读不了放会话的目录：`internal_error`。

**`session.send`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 要说的话，照原样成一块文字；空的一块都没有 |
| `urgent` | 布尔，不写是 `false` | 急着插话 |
| `cwd` | 字符串，可以不写 | 头现在的工作目录 |
| `dirs` | 字符串的数组，可以不写 | 加进来的目录（施工 5-10 上）。不写的照旧；写了的，这一句以后开的回合照它，空的就是没有 |
| `attachments` | 数组，可以不写 | 附件（施工 3-9 三补）：`blob.put` 的回应，照先后。每一项要 `blob`、`name`、`media_type`，别的格不看 |

回应：`events` 是 `[<这一句 message.user 的序号>]`；`cwd` 是收下这一句的 `cwd` 以后，会话实际在哪个目录里干活。

1. 没有回合在进行的，这一句开一轮；有的，排队，`urgent` 的插进下一步（`kernel/session.md`）。
2. 开的那一轮，`turn.started` 的 `cause` 是这一条的 `id`：头照它认出自己的那一轮。
3. `text` 是空的、又没有附件：`empty_message`。先找会话，找不到的回的是找不到。只有附件、`text` 是空的，也是一句话。
4. 附件变成内容块，照先后接在文字那一块后面（施工 3-9 三补）：核心照 blob 的内容照 `blob.put` 第 4 条再认一遍，同一份代码。图片是图片块，宽、高、媒体类型照这一次量的，头交回来的 `kind`、`width`、`height` 不算，`name` 照交回来的（施工 3-9 四补：一句话附了几张图，她分得清哪张是哪个文件）；文件是文件块，`name` 照交回来的，媒体类型照交回来的再过一遍第 4 条（内容是 PDF 的写 `application/pdf`，交回来写成 PDF、图片而内容不是的照内容认）。
5. 附件先查，再找会话：一项缺了格、格不合写法（`kernel/ids.md`）：`bad_params`；blob 不在管理员的 blob 里：`unknown_attachment`；读不出来（坏了、读不了）：`internal_error`，记一条运行日志；是超了上限的图（不是 `blob.put` 传的 blob 才会有）：`attachment_too_big`。拒了的，会话里什么都不送，`cwd`、`dirs` 也不送。

**`blob.put`**（施工 3-9 三补，`04-核心协议.md` 第九节）

| 参数 | 类型 | 说明 |
|---|---|---|
| `path` | 字符串，可以不写 | 本机的文件：绝对路径，或者 `~`、`~/…`；核心自己读 |
| `data` | 字符串，可以不写 | 文件的内容，base64（RFC 4648 的标准字母表，末尾补 `=`）：远程的头用 |
| `name` | 字符串，可以不写 | 文件名，只是名字（`kernel/ids.md` 的写法）。不写的取 `path` 的最后一段；传 `data` 的必写 |
| `media_type` | 字符串，可以不写 | 媒体类型（`kernel/ids.md` 的写法）。不写的照内容认 |

回应：`blob`（内容哈希）、`name`、`media_type`、`kind`（`image` 或 `file`）；图片另带 `width`、`height`（像素）。例子（格照名字的字母先后排）：

```json
{"id":"c3","jsonrpc":"2.0","result":{"blob":"sha256:…","height":600,"kind":"image","media_type":"image/png","name":"shot.png","width":800}}
```

1. `path`、`data` 正好写一个；两个都写、都不写（写 `null` 算没写）：`bad_params`。`path` 是相对的（没有工作目录可接，头自己接成绝对的）、`~别人/…`：`bad_params`。`data` 不是 base64、传 `data` 没写 `name`、`name` 不合文件名的写法、`media_type` 不合媒体类型的写法：`bad_params`。
2. 读 `path`，在阻塞线程里：照 `fs.md` 换成真实的位置（链接照指向的地方算），照边界表（管理员的工作区、数据根、这台机器的临时目录和系统目录，`fs.md` 第一节）落在谁都不能碰的那一片（数据根里、管理员的工作区以外）：`attachment_in_data_root`。别的地方都能读，和她读文件一样（`session/guard.md`：读哪儿都不问）。换不成真实的位置、打不开（没有、不是普通文件、没有权限）：`attachment_unreadable`。照 `fs.md` 第四节打开，路上一层链接都不跟。
3. 一个最多 20 MiB（20,971,520 字节），多的 `attachment_too_big`；读到上限多一个字节就停，不整份读进来。`data` 放在一行 JSON 里，一行最长 1 MiB（「一行一条」），所以最多七百多 KiB，大的传 `path`；分块上传以后再说（`04-核心协议.md` 第十一节）。
4. 认是什么，照内容，不看扩展名：
   1. 开头是四种图之一（PNG、JPEG、GIF、WebP）、量得出宽高的：图片，媒体类型照认出的，`media_type` 写了也不算。超过 5 MiB、哪一边超过 8000 像素：`attachment_too_big`，正好在线上的收。认法和上限和 `read` 读图片是同一份代码（`crates/miyu-tool/src/picture.rs`，`tools/read.md`「读图片」）：图跟着对话每次都发，被供应商拒掉的图会让这个会话以后的请求都失败。
   2. 别的都是文件。开头是 `%PDF-` 的，媒体类型是 `application/pdf`，`media_type` 写了也不算。
   3. 别的：`media_type` 写了的照写的，只是写成 `application/pdf`、`image/…` 的不算（驱动照它们把内容当 PDF、当图发，内容不是，供应商会拒）；没写、不算的，整份是 UTF-8、没有 NUL 字节的是 `text/plain`（和驱动认文本文件是同一条，`drivers/openai-chat.md` 第 9 条），别的 `application/octet-stream`。扩展名不认：头知道得更准的（例如浏览器给的类型）自己写 `media_type`（施工 3-9 三补定：扩展名的表是一份写死的名单，驱动给模型看的只有文件名和内容，用不上它）。
5. 存成管理员的 blob（`store.md` 第九条），落了盘才回应；同一份内容再传，还是那一个 blob。存不下来：`internal_error`，记一条运行日志。
6. 不碰会话，没有命令编号的去重：内容一样，存几次都是同一个。传了没发的留在 blob 里，随存储的回收那一步清。

**`session.interrupt`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `queued` | `"send"` 或 `"return"`，必写 | 排着队的消息：打断以后马上发，还是退回来 |

回应：`events`，这一次追加的全部事件的序号。没有回合在进行：`not_running`。

**`session.redo`**（施工 4-7 再补，`kernel/history.md`「重做」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，可以不写 | 开这一轮的那一句里的字换成这句话，写法照 `session.send` 的 `text`：照原样成一块文字，空的是不要字。不写的字照原来的 |
| `attachments` | 数组，可以不写 | 开这一轮的那一句里的附件换成这几个，写法照 `session.send` 的 `attachments`；空的是不要附件。不写的附件照原来的 |

回应照撤销的（`protocol/undo.md`）：`events` 照先后是 `turn.reverted`、`files.restored`（有要改回的文件的）、重发的每一句 `message.user` 的序号；`cwd`、`turns`、`said`、`commands`、`compactions`、`clears`、`files` 照撤销写，`said` 是撤掉的那一轮原来那一句的第一行。重发的都落了盘才回，和 `session.send` 一样不等新的一轮做完。

1. 只重做最后一轮：撤掉它，把开它的那几句人的话（排着接过来的几句，和开这一轮的那一句）照先后再发一次，开新的一轮；附件照带。`text`、`attachments` 两样都不写的原样重发；写了的只换开这一轮的那一句：写了 `text` 的换字、写了 `attachments` 的换附件，没写的那一样照原来的，字在前、附件在后（2026-09-30 项目主人定：网页里编辑上一句也走这里；换附件主会话定）。
2. 新的一轮，`turn.started` 的 `cause` 是这一条的 `id`：头照它认出自己的那一轮，和 `session.send` 一样。重发的几句 `by` 照原来的，`cause` 也是这一条的 `id`。
3. 最后一轮不是人说的话开的（回报叫醒的、手动压缩、清空、重启以后接着干的），或者一轮都没有：`not_redoable`，头把它那一句当一条提示通知显示。有回合在进行：`turn_running`。换过的那一句一块都不剩（原来只有字、`text` 是空的，原来只有附件、`attachments` 是空的）：`empty_message`。正在改回文件：`restoring`。附件照 `session.send` 第 5 条先查、再找会话：`bad_params`、`unknown_attachment`、`internal_error`、`attachment_too_big`，拒了的什么都不送。先找会话，找不到的回的是找不到。
4. 重做以后恢复不了：新的一轮开了（`session.unrevert` 回 `nothing_to_unrevert`）。
5. `text` 不是字符串（数字、数组……）、`attachments` 不是数组：`bad_params`；写 `null` 等于没写（「请求」最后一条）。不收 `cwd`、`dirs`：新的一轮照会话现在的环境，和 `session.revert` 一样（「会话表」第 5 条）。

**`session.compact`**（施工 6-8，`compaction.md` 第七条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `instructions` | 字符串，可以不写 | 人附的要求，例如「重点保留数据库设计的讨论」，原样交给内核；去掉前后空白是空的，当没写 |

回应：`events` 是 `[<那一轮 turn.started 的序号>]`，它落了盘就回，和 `session.send` 一样不等这一轮做完。压好了没有，看推送里的 `compaction.progress`、`compaction.done`、`context.compacted`、`turn.ended`。

1. 开的那一轮，`turn.started` 的 `cause` 是这一条的 `id`，没有 `trigger`：头照 `cause` 认出自己的那一轮。
2. 有回合在进行：`turn_running`。没有能压的：`nothing_to_compact`（`compaction.md` 第七条第 2 条）。正在改回文件：`restoring`。先找会话，找不到的回的是找不到。
3. `instructions` 不是字符串（数字、数组……）：`bad_params`。

**`session.set_permission_level`**（施工 3-8 再补，`kernel/session.md`「切权限级别」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `level` | `"workspace"` 或 `"full"`，可以不写 | 常用的那一级：工作区、完全放开 |
| `read_only` | 布尔，可以不写 | 只读开关 |

回应：`{}`。切了没有、切成了什么，看推送里的 `session.policy_changed`。

1. 改哪样写哪样，没写的那一格照旧。两格都不写（写 `null` 也算没写）：`bad_params`，不找会话。
2. `level` 只认这两种，别的（`read_only`、大写的、不是字符串的）：`bad_params`，和 `queued`、`stream` 一样。内核的 `unknown_level` 因此从协议上碰不到。
3. 和现在一样的：什么都不记，照样回 `{}`。
4. 不一样的：记一条 `session.policy_changed`，`permission` 两格都写，`by` 取自连接（现在都是管理员），`cause` 是这一条的 `id`，回合进行中切的带上这个回合；落了盘才回应。收紧的当场生效：收紧成只读的，这一步里还没跑的写入当场补 `denied`，和它同一批落盘、推送；放宽的下一次请求才生效（`kernel/session.md`「切权限级别」第 4、5 条）。
5. 改成完全放开之前请人确认一次，是头的事（`04-核心协议.md` 第九节），核心不问。
6. 先找会话，找不到的回的是找不到。

**`session.clear`**（施工 6-8 补，`compaction.md` 第十四条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |

回应照 `session.compact`：`events` 是 `[<那一轮 turn.started 的序号>]`。那一轮的开头、空的检查点、结束同一批，都落了盘才回：回应到的时候，推送里已经有这三条（`turn.started` 没有 `trigger`，`context.compacted` 的 `trigger` 是 `clear`、`summary` 是空的，`turn.ended` 是 `completed`），没有 `compaction.progress`、`compaction.done`。

1. 开的那一轮，三条的 `cause` 都是这一条的 `id`：头照它认出自己的那一轮。
2. 有回合在进行：`turn_running`。上下文本来就是空的：`nothing_to_clear`（`compaction.md` 第十四条第 2 条），头把它那一句当一条提示通知显示。正在改回文件：`restoring`。先找会话，找不到的回的是找不到。
3. 撤掉那一轮（`session.revert`）上下文回到清空以前，回应里 `clears` 数它一次、`compactions` 不算它、没有 `said`（`protocol/undo.md`）。

**`job.stop`**（施工 7-4，`agents.md` 第五条第 5 条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话派出去的 |
| `job` | 字符串，必写 | 任务编号，`j1` 这样 |

回应：空对象 `{}`，这个任务的回报落了盘才回（先见结果，后见回应）。

1. 后台命令：整组杀掉，记 `job.reported`（`stopped`，`by` 是管理员，`cause` 是这一条的 `id`）；子代理：停掉它这一轮连它派的，父会话记 `child.reported`（`stopped`，`by` 是子会话）。两种都不带 `by_model`：人停的叫醒她（`agents.md` 第三条第 4 条）。
2. 没有这个任务、已经结束了（回报到了，子代理报过 `done` 也算；正好自己退出了的只认先到的）：`unknown_job`，什么都没写。先找会话，找不到的回的是找不到。
3. `job` 不合任务编号的写法（`j` 加一段或几段不带前导零的正整数，段之间用 `.`，`kernel/ids.md`）、不是字符串：`bad_params`。

**`session.set_meta`**（施工 3-8 三补，`kernel/session.md`「改标题、置顶」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `title` | 字符串或 `null`，可以不写 | 新的标题：去掉前后空白以后 1 到 200 个字（Unicode 字符）。写 `null` 是去掉标题 |
| `pinned` | 布尔，可以不写 | `true` 置顶，`false` 取消置顶 |

回应：`{}`。改了什么，看推送里的 `session.meta_changed`。

1. 改哪样写哪样，没写的那一格照旧。`title` 和别的「可以不写」的格不一样：写 `null` 有意思，是去掉标题；`pinned` 写 `null` 照旧等于没写。两格都不写（`pinned` 写 `null` 也算没写）：`bad_params`，不找会话。
2. `title` 去掉前后空白（Unicode 的空白）再量：空的、超过 200 个字的、不是字符串的：`bad_params`。中间的空白、换行照原样。`pinned` 不是布尔的：`bad_params`。
3. 和现在一样的格去掉：标题照去掉空白以后的比，没有标题时去掉标题、没置顶时取消置顶都算一样。一格都不剩的，什么都不记，照样回 `{}`。
4. 还剩的记一条 `session.meta_changed`，只写还剩的那几格；去掉标题写成 `"title":""`（`kernel/events-bodies.md`）。`by` 取自连接，`cause` 是这一条的 `id`，回合进行中改的带上这个回合；落了盘才回应。改名不影响回合：标题、置顶不进请求。
5. 先找会话，找不到的回的是找不到；没在跑的照「会话表」载入。

**`session.delete`**（施工 3-8 三补，`store.md` 第 12 条，`agents.md` 第七条第 5 条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |

回应：`{}`，目录都挪好了才回。

1. 会话表拿着锁办完一整件：这期间别的连接载入不了会话、给会话发不了命令，会话里也造不了子会话。
2. 先认出它派出去的子会话：放会话的目录里每个会话 `session.created` 的 `parent`，一层层往下（子、孙……）。第一条读不出来的会话认不出父会话，不算。读不了放会话的目录：`internal_error`，什么都没动。
3. 删的是一个子会话、它的父会话还在的，等于人先停掉它再删（2026-09-30 主会话定：父会话不会白等）。任务编号照子会话 `session.created` 的 `cause`（`<父会话>/<编号>`）读回；父会话照「会话表」找，没在跑的载入。它在跑的，不问忙不忙就停下（`Handle::discard`，它派的、它的后台命令照第 6 条）：正忙的子会话因此也删得了。第 6 条停完子会话以后，父会话照人停它的样子记一条 `child.reported`（`stopped`，`by` 是子会话，不带 `by_model`，正文照它的日志看它这一轮最后说的，截法同 `job.stop`），叫醒父会话：回报在父会话的 actor 里当场记下，不经会话表（`Handle::stopped_child`）。父会话照名册认它：已经报过、被停过的不再记；父会话已经不在的（删了、连带删的）、载入不了的，不送，照样往下删。施工 7-8 把这一步挪进锁里：原来经会话表的端口来回、在拿锁之前，父会话记下它停了以后、这一头拿到锁之前，有人给它发一句，它就又开了一轮，删的时候说有回合在进行，父会话却以为它停了。
4. 别的，它在跑的：交给它的 actor，内核说删不删得了（`kernel/session.md` 的 `deletable()`）。有回合在进行（结束了 `turn.ended` 还没落盘的也算）：`turn_running`；正在读回日志、改回文件：`restoring`。被拒的什么都没动，会话照常：头先打断，或者等它做完。删得了的，actor 把它在跑的后台命令整组杀掉、不记回报，关上日志，退出（`session/actor.md` 第 9 条）。它自己已经停了的（写不进去、出了 bug），照样往下删。
5. 它没在跑的：不载入。载入会收尾崩了的那一轮、接着干被重启打断的那一轮、叫起子会话，删之前都不该做。磁盘上没有它的日志（第一行还没写完的也算）：`session_not_found`。
6. 在跑的子会话一层层往下停：不问忙不忙，路上的请求叫停、在跑的工具掐掉、后台命令整组杀掉，都不记（`Handle::discard`）；连带删的子会话不向它们的父会话送回报：父会话也在删。
7. 目录挪进回收处 `home/<账号>/trash/sessions/<会话编号>/`，一个会话一个，先写 `deleted_at`（`store.md` 第 12 条）。从最深的子会话挪起，它自己最后：半路崩了、挪不了的，它还在原处、列得出来，再删一次接着挪完。挪不了：记一条运行日志，`internal_error`。
8. 删了的：不在 `session.list` 里；再对它发命令、订阅、改标题、再删，都是 `session_not_found`；订阅着它的头收到 `resync`（它停了），重新订阅时是没有这个会话。重发的造会话不再交回它。
9. 子会话向上交的回报、派子代理，碰上删了的会话：父会话没了的回报丢掉（`agents.md` 第二条第 5 条）；父会话已经不在会话表里的，不再造子会话（「会话表」第 7 条）。

**`subscribe`、`unsubscribe`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `stream` | 字符串，必写 | 现在只有 `events`，别的 `bad_params` |

回应：`subscribe` 的是 `{"limits": <限额>}`，`unsubscribe` 的是空对象 `{}`。

**限额** `limits`：会话实际用的模型给这个会话多少地方（施工 6-3 补）。两格都是 token 数，没有的不写，从不写 `null`；两格都没有就是 `{}`。

| 格 | 类型 | 说明 |
|---|---|---|
| `window` | 非负整数，可以没有 | 上下文窗口。模型的资料没报的没有（`compaction.md`「对外的样子」模型的资料） |
| `compaction_line` | 非负整数，可以没有 | 压缩线：用量过了它，发下一次请求之前自动压（`compaction.md` 第二条第 2 条）。没有窗口的、窗口太小算不出正数的没有 |

例子：核心照 DeepSeek 的资料，窗口 1000000、最大输出 393216，压缩线 = 1000000 − min(393216, 20000) − 13000：

```json
{"id":"c2","jsonrpc":"2.0","result":{"limits":{"compaction_line":967000,"window":1000000}}}
```

模型的资料没报窗口的：`{"id":"c2","jsonrpc":"2.0","result":{"limits":{}}}`。

1. 订阅：从这一刻起的推送都转给这个连接，以前的不补。没在跑的会话先载入。
2. 这个连接已经订阅着这个会话、还在推的，还是那一个，回应照样带限额；停了推的（掉了队、会话停了），换一个新的。
3. 取消订阅：停掉转发。不载入会话，没订阅过的也回 `{}`。
4. 订阅的回应不排在推送后面：会话正忙的，回应之前可能已经有这个会话的推送。
5. 限额是造会话、载入时定的：会话 actor 把模型的限额交给内核以后，向内核要一份（`kernel/session.md` 的 `context_limits()`），`Handle` 带着它（`session/actor.md`）。会话里不变：一个核心只有一个模型，策略冻结在会话上。
6. 压缩线由内核算好，和它自己判到线用的是同一条；头照 `window` 画「用量 / 窗口」、照 `compaction_line` 算离压缩还有多少，不照公式自己算（公式里的输出预留、余量在策略里）。
7. 限额不进日志，也不推瞬时事件：头每次接进来（造完会话、中途接进一个在跑的会话、掉了队重新订阅、核心重启以后）都经 `subscribe`，从回应里拿（为什么见 `04-核心协议.md` 第九节「先做的几样怎么写」）。
8. 订阅着就算这个头在看着这个会话（施工 7-9）：一次性的会话没有头订阅着，回报只记下、不叫醒她（`agents.md` 第三条第 3 条）。取消订阅、连接断了，就不算了；会话 actor 数着拿着订阅的头（`session/actor.md` 第 3 条），不进日志，协议上不另说。头要知道还有几个子代理没报、叫醒的那一轮会不会来，照推过来的事件自己数（`job.started`、`child.reported`、`job.messaged`，`cli/ask.md`「等子代理」），协议不另给：施工 7-9 照最简单、不加协议定。

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
5. 载入时没有报来的 `cwd`（`session.interrupt`、`session.revert`、`session.unrevert`、`session.redo`、`session.compact`、`session.set_permission_level`、`session.clear`、`session.set_meta`、`subscribe` 载入的）：照日志里最后一条带 `cwd` 的 `turn.started`（加进来的目录照最后一条 `turn.started` 的 `dirs`，没有就是没有（施工 5-10 上）），没有就照 `session.created` 的，都没有（之前的日志）才当报来的是 `~`，退回管理员的工作区（施工 4-9 再补三上）。核心重启以后撤销，路径照会话真正的目录写短。
6. 会话一直留在表里，直到核心退出、停下全部会话、删了它（`session.delete`），或者用到时发现它停了。
7. 造会话、载入时，交给会话一份造子会话的端口（施工 7-5，`session/tools.md`「派子代理」）：会话里派出去的子会话由会话表造，放进表里，和头造的一样照编号找得到、只起一个；子会话也算进「有没有会话忙着」，停下全部会话时一起停。父会话已经不在表里的（删了、停了）不再造，派不了（施工 3-8 三补：不留下没有父会话的子会话）。
8. 载入一个会话以后、放进表之前，收掉它派到一半的空子会话（施工 7-8，`agents.md` 第一条第 8 条）：它的日志里有没派成的 `agent` 调用（结果里没有 `job.started`，或者还没有结果）才去认，认的是放会话的目录里 `session.created` 的 `parent` 是它、它的日志里又没有这个子会话的 `job.started` 的，连同它们派的；在跑的停下（`Handle::discard`），目录挪进回收处，最深的在前，照 `session.delete` 第 7 条。一个记一行 `INFO orphan subagent removed`；挪不走的记一行 `WARN`，不耽误载入。这时表拿着锁，它不在表里，也就派不出新的，认不错。

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

- `Core::idle`：没有连接，执行器的任务表里没有在跑的后台命令（结束了、记录还没落盘的也算，施工 7-3），会话表里也没有哪个会话忙着：在跑回合、回合结束了 `turn.ended` 还没落盘、在改回文件，都算忙；停了的会话不算。核心照它空闲退出（`core.md`）。
- `Core::stop_sessions`：有计划地停下表里全部的会话，跑到一半的回合记成「重启了」，下次载入接着干（`kernel/session.md`）；各会话在跑的后台命令先记 `restarted`、落了盘再整组杀（`session/actor.md` 第 9 条，施工 7-3）；会话表清空。
- 任务表（`miyu_session::Jobs`）是核心的家底里的一张，造会话、载入时交给会话（施工 7-3）。
- 接不了连接（例如打开的文件太多）：歇 100 毫秒再接，不空转。

### 出错

JSON-RPC 自己的几种照它的标准码；Miyu 的一律 `-32010`，原因写在 `data.reason`。

| 原因码 | `code` | 什么时候 |
|---|---|---|
| `parse_error` | -32700 | 不是 JSON；一行太长（之后断开） |
| `invalid_request` | -32600 | 是 JSON，不是请求（「请求」的表） |
| `unknown_method` | -32601 | 握手以后，没有这个方法 |
| `bad_params` | -32602 | 参数读不成、类型不对；会话编号、人格编号不合写法；`turn` 写了 0；`stream` 不是 `events`；切权限级别两格都不写、`level` 不是 `workspace`、`full`；`blob.put` 第 1 条那几种；`session.send`、`session.redo` 的附件缺了格、格不合写法；改标题两格都不写，标题去掉空白以后是空的、超过 200 个字 |
| `internal_error` | -32603 | 造会话时装坏了、磁盘上建不成、`session.created` 没落盘；列会话时读不了放会话的目录、崩了；附件存不下来、读不出来；删会话时读不了放会话的目录、挪不进回收处、崩了 |
| `hello_first` | -32010 | 握手以前发了别的方法 |
| `protocol_mismatch` | -32010 | 头支持的主版本里没有 1（之后断开） |
| `bad_token` | -32010 | 本机令牌没带、不对（之后断开） |
| `unknown_persona` | -32010 | 造会话时人格的目录不存在 |
| `session_not_found` | -32010 | 没有这个会话，删了的也是 |
| `session_stopped` | -32010 | 会话停了：写不进去、出了 bug |
| `session_broken` | -32010 | 会话载入不了：日志、策略快照坏了、读不了 |
| `empty_message` | -32010 | `session.send` 的 `text` 是空的、又没有附件；`session.redo` 换过的那一句一块都不剩 |
| `dir_too_wide` | -32010 | 加进来的目录太宽（「加进来的目录」（施工 5-10 上）） |
| `attachment_unreadable` | -32010 | `blob.put` 读不了 `path`：换不成真实的位置、没有、不是普通文件、没有权限（施工 3-9 三补） |
| `attachment_too_big` | -32010 | 附件超过 20 MiB；图片超过 5 MiB，或者哪一边超过 8000 像素（施工 3-9 三补） |
| `attachment_in_data_root` | -32010 | `blob.put` 的 `path` 在数据根里、管理员的工作区以外（施工 3-9 三补） |
| `unknown_attachment` | -32010 | `session.send`、`session.redo` 附的 blob 这个核心里没有（施工 3-9 三补） |
| `not_running` | -32010 | 打断时没有回合在进行 |
| `turn_running` | -32010 | 撤销、重做、手动压缩、清空、删会话时有回合在进行 |
| `unknown_turn` | -32010 | 要撤的那一轮不在有效历史里：没有，或者已经撤掉了 |
| `nothing_to_unrevert` | -32010 | 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过 |
| `nothing_to_revert` | -32010 | 不写 `turn` 的撤销，一轮都没有 |
| `nothing_to_compact` | -32010 | 手动压缩时没有能压的：上一次压缩以后没有新的消息、回复、工具结果，或者全在尾巴里（施工 6-8） |
| `nothing_to_clear` | -32010 | 清空时上下文本来就是空的：没有摘要，最近的检查点后面也没有人的消息、回复、工具结果、回报（施工 6-8 补） |
| `unknown_job` | -32010 | `job.stop` 时没有这个任务，或者它已经结束了（施工 7-4） |
| `not_redoable` | -32010 | 重做时最后一轮不是人说的话开的，或者一轮都没有（施工 4-7 再补） |
| `restoring` | -32010 | 撤销、恢复还没做完（正在读回更早的日志、正在改回文件）时来的命令、删会话。兜底：会话做完才接下一个命令，照常碰不到 |

- 从 `empty_message` 起，除了 `dir_too_wide` 和附件的四个，十一个是内核拒命令时给的原因码（`kernel/session.md`）。
- 内核还有六个原因码，现在没有方法碰得到：`unknown_level`（协议上的级别只认两种，别的先是 `bad_params`）、`not_asking`、`unknown_decision`、`no_rule`、`unexpected_reason`、`bad_answer`。它们没有配话，说的是最后那一句「被拒绝了」。

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
| `WARN` | `meta not read session=… error=…` | 列会话时后面的日志读不下去，标题、置顶照坏的那一段以前的算（施工 3-8 三补） |
| `WARN` | `session not deleted session=… error=…` | 删会话时这一个挪不进回收处 |
| `ERROR` | `delete panicked error=…` | 删会话崩了 |
| `WARN` | `workspace not prepared kind=…` | 退回的工作区建不成 |
| `DEBUG` | `already stopped session=…` | 停下全部会话、删会话时，这一个已经停了 |
| `DEBUG` | `stopped before deletion session=… job=…` | 删一个子会话时照人停掉了它，父会话（`session`）记了回报（施工 3-8 三补；施工 7-8 起在表的锁里） |
| `INFO` | `orphan subagent removed session=… parent=…` | 载入父会话时收掉了一个派到一半的空子会话（施工 7-8，「会话表」第 8 条） |
| `WARN` | `orphan subagent not removed session=… error=…` | 这一个挪不进回收处 |
| `ERROR` | `orphan sweep panicked error=…` | 认、挪空子会话时崩了 |
| `WARN` | `attachment not stored error=…` | `blob.put` 存不下来（施工 3-9 三补） |
| `WARN` | `attachment not read blob=… error=…` | `session.send` 的附件读不出来：坏了、读不了 |
| `ERROR` | `attachment panicked error=…` | 读、存附件时崩了 |

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
| `attachment_unreadable` | 读不了这个文件：没有、不是普通文件，或者没有权限。 | This file cannot be read: it is missing, not a regular file, or not permitted. |
| `attachment_too_big` | 附件太大：一个最多 20 MiB，图片最多 5 MiB、每边最多 8000 像素。 | The attachment is too big: at most 20 MiB, and an image at most 5 MiB and 8000 pixels a side. |
| `attachment_in_data_root` | Miyu 的数据根里的文件不能当附件。 | Files in Miyu's data root cannot be attached. |
| `unknown_attachment` | 附件不在核心里：先用 blob.put 传上来。 | The attachment is not in the core; upload it with blob.put first. |
| `not_running` | 没有正在进行的回合，打断不了。 | No turn is running, so there is nothing to interrupt. |
| `turn_running` | 有回合在进行：先打断，或者等它做完。 | A turn is running; interrupt it or wait for it to finish. |
| `unknown_turn` | 没有这一轮，或者它已经撤掉了。 | There is no such turn, or it has already been undone. |
| `nothing_to_unrevert` | 没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。 | There is nothing to restore: nothing was undone, or a turn or compaction came since. |
| `restoring` | 正在撤销、恢复，等它做完再来。 | An undo or restore is still in progress; try again when it is done. |
| `nothing_to_revert` | 没有能撤销的回合。 | There is no turn to undo. |
| `nothing_to_compact` | 没有能压的：还没压过的内容都在原样留着的最近一段里。 | Not enough to compact: everything not yet compacted is in the recent part that stays as it is. |
| `nothing_to_clear` | 上下文为空 | The context is empty. |
| `unknown_job` | 没有这个任务，或者它已经结束了。 | There is no such job, or it has already ended. |
| `not_redoable` | 无法重做 | Cannot redo. |
| 别的 | 被拒绝了。 | Refused. |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 认请求的每一条、回应是一行、去掉行尾、太长的 |
| `crates/miyu-endpoint/tests/endpoint.rs` | 握手先行、令牌（长短也比）、版本对不上，被拒的断开；握手的回应照核心探到的报沙盒，四种（施工 5-4 下）；造会话、说话；同一个造会话只造一个；没有的会话；载入上一次运行的会话；两个连接只载入一次；照头的语言拒绝；JSON-RPC 的错误码、通知不回应；太长的断开；工作目录跟着头；不能输入的头造的会话没人确认；空消息；停了的会话下次再载入；打断时排着的接着发 |
| `crates/miyu-endpoint/tests/limits.rs` | 订阅的回应带限额（施工 6-3 补）：窗口、压缩线照核心的模型算；没报窗口的是 `{}`，窗口太小的只有 `window`；已经订阅着的再订阅也带；核心重启以后载入的照样带；`unsubscribe` 还是 `{}` |
| `crates/miyu-endpoint/tests/subscribe.rs` | 先见结果后见回应；两个会话不串；取消订阅以后不推；掉队推 `resync`、回应一条不丢、重新订阅照常推；积压时回应排在推送后面；取消订阅时已经交给转发任务的回应照样到；会话停了推 `resync`；订阅要握手、要有这个会话、只认 `events` |
| `crates/miyu-endpoint/tests/restart.rs` | 核心重启以后：不带 `cwd` 载入的会话照最后一轮的工作目录、没开过回合的照造会话时的；重发的造会话交回原来那一个 |
| `crates/miyu-endpoint/tests/edges.rs` | 不握手的到时断开、握手了的不受管；数组的 `params` 参数不对；握手被拒照它报的语言说；人格目录不存在是 `unknown_persona`、目录在而读不了是 `internal_error` |
| `crates/miyu-endpoint/tests/list.rs` | 从新到旧、只要一次性的、`limit`、参数不对、空的 |
| `crates/miyu-endpoint/tests/meta.rs` | 改标题、置顶（施工 3-8 三补）：改名去掉空白、只写改了的那一格、推送在回应前面、`by`、`cause`；置顶、取消、两样一起；`null` 去掉标题记成空的；和现在一样的六种什么都不记；200 个字收、201 个字和空白的不收；两格都不写（含 `pinned` 写 `null`、会话没有的）、类型不对、会话编号不对是参数不对，没有的会话找不到；回合进行中改的带上回合；`session.list` 带标题、置顶，取消了、去掉了的不写，核心重启以后照样，载入以后照日志接着比；日志坏了的照样列出来 |
| `crates/miyu-endpoint/tests/delete.rs` | 删除会话（施工 3-8 三补）：空闲的整个目录挪进回收处、日志不变、`deleted_at` 是删的时刻，列不出来，再发命令、订阅、改名、打断、再删都是没有这个会话，重发造它的那一条另造一个；回合进行中的拒绝、什么都没动，打断以后删得掉；核心重启以后没在跑的不载入就删（被重启打断的那一轮不接着干）；参数不对、没有的会话 |
| `crates/miyu-endpoint/src/sessions/delete/tests.rs`（施工 7-8） | 删子会话在表的锁里停它、父会话记回报：父会话一记下它停了就去叫醒它，拿到表的锁时它已经删掉了，删得掉（挪进锁以前，这时它又开了一轮，删的时候说有回合在进行） |
| `crates/miyu-endpoint/tests/orphans.rs`（施工 7-8） | 真核心：父会话派出去一个子代理，没来得及记下 `job.started` 就崩了（日志截在派它的那条回复后面），再载入父会话时子会话挪进回收处；一次派两个、只记下一个的只收那一个；记下了的照留 |
| `crates/miyu-endpoint/tests/delete_children.rs` | 删会话连子会话（施工 3-8 三补）：主会话派的子代理、子代理派的孙代理一起停下、各自挪进回收处，两条后台命令各杀一次、谁都不记回报；删一个正忙的子会话，主会话记一条 `stopped` 的回报（`by` 是子会话、不带 `by_model`）、被叫醒，子会话挪走、主会话还在；报过 `done` 又被留了言、主会话又在等它的，删它也记一条 `stopped`、叫醒主会话（施工 7-7）；主会话已经进了回收处的，删子会话照样删、不送；子代理派的编号带着它自己的 `j1`（`j1.1`、`j1.2`），删孙会话时子会话记的 `stopped` 回报是 `j1.1`（施工 7-1 补） |
| `crates/miyu-endpoint/tests/spawn.rs` | 会话里派子代理，会话表造出子会话、交代送进去、替身模型在子会话里答话；`session.list` 里子会话写着父会话、主会话写 `null`（施工 7-5） |
| `crates/miyu-endpoint/tests/revert.rs` | 协议上撤销、恢复；三种拒绝的中文；`turn` 写 0 |
| `crates/miyu-endpoint/tests/permission.rs` | 协议上切权限级别（施工 3-8 再补）：切到完全放开、开只读、两样一起换，各记一条、推给订阅着的头、回应 `{}`；和现在一样的四种什么都不记不推；两格都不写（含写 `null`、会话没有的）、级别和只读的值不对、会话编号不对、没写会话是参数不对；没有的会话找不到，停了的会话是停了；回合进行中收紧成只读，真核心走一遍：等着的写入当场补 `denied`、和切权限同一批、推送在回应前面，放行以后请求之前注入只读那一块，写的一次没跑 |
| `crates/miyu-endpoint/tests/job_stop.rs` | 协议上停子代理（施工 7-4）：回应 `{}`、回应之前父会话记下了回报、子会话那一轮被父会话打断；停过的、没有的 `unknown_job`，中文、英文；编号不合写法、不是字符串的参数不对；没有这个会话 |
| `crates/miyu-endpoint/tests/redo.rs` | 协议上重做（施工 4-7 再补）：回应带撤销的几样和重发的那一句、推送里是一批撤销、原话、新的一轮，新的一轮的请求和撤掉的那一轮的一字不差；换了话的推送里是新的话、`said` 是原来的；改过文件的先改回、回应带 `files`；重做以后恢复不了；最后一轮是清空、没说过话的，有回合在进行、换成空的拒绝，中文、英文；`text` 不是字符串、会话编号不对的参数不对，写 `null` 当没写；附件照带、换掉、不要，没有的 blob `unknown_attachment` 什么都不写 |
| `crates/miyu-endpoint/tests/compact.rs` | 协议上手动压缩（施工 6-8）：回应是那一轮的开头、推送里压好了；要求原样到了摘要请求里；撤掉那一轮的回应里没有 `said`；有回合在进行、没有能压的两种拒绝，中文、英文；`instructions` 不是字符串的参数不对 |
| `crates/miyu-endpoint/tests/clear.rs` | 协议上清空（施工 6-8 补）：回应是那一轮的开头、订阅的推送里是那一批三条、不请求模型；下一次请求里没有清空以前的；撤掉那一轮回应里撤掉了一次压缩、没有 `said`，再问看得到了；有回合在进行、本来就空的两种拒绝，中文、英文；会话编号不对、没写的参数不对 |
| `crates/miyu-endpoint/src/sessions/tests.rs` | 父会话不在会话表里的不再造子会话、什么都没建（施工 3-8 三补） |
| `crates/miyu-endpoint/tests/workspace.rs` | 太宽的五种（`~`、家目录、根目录、数据根、数据根里面）和读不出家目录时的 `~`；项目目录、账号的工作区照旧；回应里的 `cwd`、重发的造会话 |
| `crates/miyu-endpoint/tests/dirs.rs` | 加进来的目录（施工 5-10 上）：造会话、说话时报的记进这一轮，不写的照旧、写空的就没有；太宽的五种整条命令都不收、什么都没写；核心重启以后照最后一轮的 |
| `crates/miyu-endpoint/tests/idle.rs` | 连着连接、跑着回合不空闲；停下全部会话，跑到一半的记成重启了 |
| `crates/miyu-endpoint/tests/attach.rs` | `blob.put`（施工 3-9 三补）：传路径、传内容；照内容认图片（扩展名不算）、PDF、文本、别的文件，量宽高，回应的格照字母排、存成管理员的 blob；写了的媒体类型什么时候算、改名、写 `null` 等于没写；太大（20 MiB、图片的宽高和 5 MiB，正好在线上的收）；数据根里的不给、管理员的工作区给、指到数据根里的链接不给；读不了（没有、目录、没有家目录时的 `~`）；参数不对的十二种、一个都没存；四种拒绝的中英文 |
| `crates/miyu-endpoint/tests/attach_send.rs` | `session.send` 带附件（施工 3-9 三补）：照先后接在文字后面，宽高、种类照核心量的，图片块带着 `blob.put` 的名字（施工 3-9 四补），她收到的请求里就是这几块；只有附件也是一句话，`null` 是没有；blob 不在的拒绝、什么都没写、换的工作目录也没送进会话；附件的格不对的七种 |
| `crates/miyu-endpoint/src/attach/kind/tests.rs` | 认附件：量得出的图是图片、头写的不算，量不出的当文件；图片的上限和线上的；PDF 照开头认；别的文件照头写的，写成 PDF、图片的照内容认，文本、空的、二进制、不是 UTF-8 的 |
| `crates/miyu-endpoint/tests/tools.rs` | 造会话、载入时用核心的工具目录；核心的沙盒造会话、载入时都交给会话，沙盒用不了的核心上执行命令没人能确认就拒（施工 5-4 上） |
| `crates/miyu-endpoint/tests/socket.rs` | 真的套接字（Windows 上是命名管道）上握手、造会话、说话，第二个头也连得上 |

### 出处

- `04-核心协议.md` 第二节（JSON-RPC、分帧、拒绝的写法）、第三节（一次连接的全过程：订阅时先拿会话状态）、第四节（连接即身份、本机令牌）、第五节（事件流；会话状态由核心算）、第六节第 1、2、4 条、第七节（慢、`resync`）、第八节（版本）、第九节「先做的几样怎么写」；P1、P2。
- `09-压缩.md` 第二节：压缩线。
- `04-核心协议.md` 第九节 `session.set_permission_level`、`02-内核.md` 第六节「权限级别怎么切」、`11-权限与沙盒.md` 第二节：切权限级别。
- `04-核心协议.md` 第九节 `blob.put`、`22-命令行.md` 第三节 `--file`、`03-事件模型.md` 第四节（量不出尺寸的不当图片）、E4：附件（施工 3-9 三补）。
- `04-核心协议.md` 第九节 `session.set_meta`、`session.delete`，`03-事件模型.md` 第三节 `session.meta_changed`：改标题、置顶、删除会话；删了的进回收处、留 7 天（2026-09-30 项目主人定）。
- `02-内核.md` 第四节（拒绝附原因码）、不变量 9（同一个编号只生效一次）。
- `06-多用户与身份.md` 第二节、U13：本机连上来的是管理员 `admin`。
- `07-存储.md` 第七节：会话按需载入。
- `11-权限与沙盒.md` 第四节：当前目录太宽。

### 还没有的

设计里有、还没做的：

- 第九节表里的其余方法：`session.fork`、`session.configure`、`session.answer`（随 M8 的抽屉）、`command.run`、查询、账号、配置……（`04-核心协议.md` 第九节）。
- 附件分块上传、远程的头传大文件（`04-核心协议.md` 第十一节）；blob 的回收（`store.md`「还没有的」）。
- 视图流、会话列表流，`view.*`、`sessions.changed`、`config.changed` 这些推送；核心决定「显示什么」（第五节、P3）。改名、置顶、删除现在只推给订阅着那个会话的头（删除是 `resync`），别的头要重新列。
- 会话列表的索引（`07-存储.md` 第六节）：现在 `session.list` 每列一次，列进去的会话日志都整份读一遍。
- 找回删了的会话、自动起标题（照第一句话生成，要请求模型）：以后（施工 3-8 三补）。回收处里的文件留着，找回时挪回去。
- 事件流重连时报出最后看到的序号、补发之后的（第七节）；队列紧张时先合并同一条目的连续增量（第七节）。
- 头发现核心比自己旧，请求它空闲时重启（第八节，`kernel.restart_when_idle`）。
- 远程连接、登录令牌、WebSocket 和它的 Origin 检查；扩展、桥当提供者，反向调用（第二节、第四节）。
- 事件流只对会话的属主和有 `events.read` 能力的扩展开放（第五节）：现在连上来的只有管理员。
- 成员只能切到只读和工作区（`11-权限与沙盒.md`）：现在连上来的只有管理员，`session.set_permission_level` 不拦，多用户那一步再拦。
- 消息结构只在 Rust 类型里定义一次，生成 JSON Schema 和 TypeScript 类型（第二节）。
- 会话空闲一段时间后 actor 退出（`07-存储.md` 第七节）。
- 换模型（`session.configure`，随配置和多供应商那一步）：那时限额会在会话中途变，推一条瞬时事件带新的限额和模型，头照最新的画。现在造会话、载入以后就不变，只在 `subscribe` 的回应里。
- 视图流的会话状态（第五节，M8）里也带限额，字段只加不改。
