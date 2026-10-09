## 核心协议

### 是什么

头和核心之间说的话：一个连接上一行一条 JSON-RPC 2.0。连上先握手，之后能造会话、列出会话、传附件、说话（可以带附件）、打断、撤销、恢复、重做、手动压缩、切权限级别、清空上下文、要一句回顾、停掉派出去的任务、读后台命令的输出、改标题、置顶、删除会话，订阅会话的事件流（能补发订阅以前的事件），查配置（施工 8-2）、改配置、信任项目配置（施工 8-3），叫一次模型、不进任何会话（`model.call`，施工 8-20），查用量和金额（`usage.query`，施工 8-15），要一个链接的卡片（`link.preview`，施工 W-7）。连接从哪来不管：本机的套接字、命名管道（`ipc.md`），测试里的内存管道。

撤销、恢复、重做的回应另写一页：`protocol/undo.md`。配置的五个方法写在 `config.md`「协议」，这一页只列进方法表、出错表。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/lib.rs` | `Core`：核心的家底（数据根、资源目录、请求模型的端口、工具目录、系统的家目录、沙盒的助手（施工 5-4 上）、管理员、本机令牌、会话表、配置（施工 8-2））；数着几个连接；空不空闲；停下全部会话 |
| `crates/miyu-endpoint/src/listen.rs` | `run`：在监听器上一个个接连接 |
| `crates/miyu-endpoint/src/connection.rs` | `serve`：一个连接，读写分开；握手以前拦住；登记成在后台答的查询交给这个连接的一组后台任务，连接断了一起停（施工 W-7）；`subscribe`（回应带会话的限额、接下来请求的模型（施工 8-10）；带 `after` 的先补发，施工 3-8 六补）、`unsubscribe`；订阅哪一个流、订阅会话的事件流在 `connection/streams.rs`（施工 9-4 补挪出来） |
| `crates/miyu-endpoint/src/wire.rs` | 读一行、认成请求、回应写成一行 |
| `crates/miyu-endpoint/src/hello.rs` | 握手 |
| `crates/miyu-endpoint/src/methods.rs` | 握手以后的方法 |
| `crates/miyu-endpoint/src/job_output.rs` | `job.output`：取最后几行、量上限（施工 7-4 补） |
| `crates/miyu-endpoint/src/meta.rs` | `session.set_meta` 的参数：标题去掉前后空白、量长短，`null` 是去掉标题（施工 3-8 三补） |
| `crates/miyu-endpoint/src/sessions.rs` | 会话表：造会话、找会话；工作目录太宽的退回工作区；造子会话（施工 7-5） |
| `crates/miyu-endpoint/src/sessions/found.rs` | 找会话、载入（施工 7-8 从 `sessions.rs` 挪出来：表的锁在调的一方手里）。会话认自己的属主（施工 O-4 上）：在跑的照把手（`Handle::owner`），没在跑的照哪个账号的家目录下有它（`DataRoot::owner_of`）；载入、读页、订阅补的日志、撤销、删会话（回收处、子会话、收空子会话）、派子代理读子会话的日志都照属主的家目录。造会话、连接的身份、列会话照旧是管理员 |
| `crates/miyu-endpoint/src/sessions/orphans.rs` | 载入时收掉派到一半的空子会话（施工 7-8，「会话表」第 8 条） |
| `crates/miyu-endpoint/src/sessions/delete.rs` | 会话表删会话：认出它派的子会话、停下、挪进回收处（施工 3-8 三补）；删子会话照人停掉它、父会话记回报，都在表的锁里（施工 7-8） |
| `crates/miyu-endpoint/src/from.rs` | `session.send` 的 `from`：去掉控制字符、截到 128 字节，记成 `harness`（施工 7-10） |
| `crates/miyu-endpoint/src/spawn.rs` | 会话表交给会话的端口：造子会话、给会话发命令（施工 7-5，`session/tools.md`「派子代理」）；停下子会话、照日志看它（施工 7-4）；列主会话（施工 C-3） |
| `crates/miyu-endpoint/src/list.rs` | `session.list`：标题、置顶照日志算（施工 3-8 三补）；工作目录、最近一次动静、忙不忙（施工 C-3）；读会话列表的索引、照日志补，起来时打开它，删会话删行（施工 3-8 七补，`store/index.md`）。她用 `sessions` 列会话也是这一个 `scan`（`tools/sessions.md`） |
| `crates/miyu-endpoint/src/subscriptions.rs` | 订阅：每个订阅一个转发任务，先写补发的（施工 3-8 六补），再推 `event`、`resync`；换掉一个订阅时等它写完 |
| `crates/miyu-endpoint/src/subscriptions/config.rs` | 配置的订阅（施工 8-4，`config.md`「协议」）：推 `config.changed`、掉队推 `resync`，`config.set` 的回应排在推送后面 |
| `crates/miyu-endpoint/src/undo.rs` | 撤销、恢复、重做的回应里给人看的几样（`protocol/undo.md`） |
| `crates/miyu-endpoint/src/attach.rs` | 附件（施工 3-9 三补）：`blob.put` 读、存；`session.send`、`session.redo` 的附件变成内容块；认是什么、文件名和媒体类型怎么查、存好了怎么拼回应，和分块上传共用（施工 W-5）；`model.call` 的图照哈希变成图片块（`images`，施工 8-20） |
| `crates/miyu-endpoint/src/attach/kind.rs` | 认一个附件是什么：图片、PDF、别的文件，媒体类型 |
| `crates/miyu-endpoint/src/uploads.rs` | `blob.open`、`blob.write`、`blob.close`：跟着连接走的上传表，60 秒不写、连接断了都作废（施工 W-5） |
| `crates/miyu-endpoint/src/refusal.rs`、`refusal/` | 拒绝：错误码、原因码、中英文的话；改人格、预设的两种在 `refusal/edits.rs`，人碰记忆的四种在 `refusal/memory.rs`（施工 R-3 补） |
| `crates/miyu-endpoint/src/settings.rs` | 端点的配置项：界面语言 `ui.language`，`auto` 照系统的语言算出 `zh`、`en`、`ja`（施工 8-1 声明，8-2 握手时用）；新会话开局只读 `permission.start_read_only`（施工 8-2） |
| `crates/miyu-endpoint/src/config.rs`、`config/` | 配置服务：起来时读的几份配置、最终值，照目录找项目配置、认信不信任；`config.schema`、`config.get`、`config.check`（施工 8-2，`config.md`）；`config.set`、`config.trust`，住在核心家底的一把锁里（施工 8-3）；监视配置文件、推 `config.changed`（施工 8-4）；密钥文件也住在这里（施工 8-5） |
| `crates/miyu-endpoint/src/secrets.rs`、`secrets/` | `secret.set`、`secret.delete`、`secret.list`：只能写、删、列名字，从不交出值；手改密钥文件被看到的、留痕（施工 8-5，`config.md` 第九条） |
| `crates/miyu-endpoint/src/models.rs`、`models/` | `model.list`：配好的供应商、模型、每一格资料的值和来源（施工 8-7，`models.md`「协议」），池、用途（施工 8-8；挡位 8-8 补去掉了），模型和 key 的冷却（施工 8-9），`facts.effort` 多 `key`（8-18（补））；`session.create`、`session.configure` 的 `model` 怎么解析（`record`，施工 8-8、8-10）；`session.configure` 的参数（`ConfigureParams`）、`subscribe` 回应的 `model`（施工 8-10）；`subscribe` 的 `model` 多 `effort`（施工 8-18，`from` 是配置的哪一层，8-18（补）起）；`model.call`（`models/call.rs`，施工 8-20）：读参数、认图、调一次性入口（`miyu_session::OneShot`）、出错写成拒绝；8-15 起一次性调用记在这个连接的账号上 |
| `crates/miyu-endpoint/src/usage.rs` | `usage.query`（施工 8-15，`models.md`「协议」）：读参数（`usage/tests.rs` 守着）、先补再查用量汇总、照 `usage.currency` 排金额、写成 `rows`；核心起来时开 `state/usage.db`（`open`） |
| `crates/miyu-endpoint/src/providers.rs`、`providers/trial.rs` | 第一次接入的 `provider.detect`、`provider.catalog`、`provider.test`（施工 8-11，`models.md`「协议」、「怎么走」第七条）；探本机、试一次在会话那一层（`miyu_session::find_local`、`probe`） |
| `crates/miyu-endpoint/src/queries.rs` | 可选软件包登记的查询：方法名到怎么答的一张表，`QueryError`；`mermaid.render` 经它接进来（施工 W-4，`mermaid.md`）；在后台答的一种登记 `register_background`，`link.preview` 照它接进来（施工 W-7，`net.md`） |

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

有的拒绝在 `data` 里多几格，和 `reason` 排在一起（施工 8-2 起：`unknown_config_key` 多 `problems`；施工 8-3：`config_invalid`、`config_file_broken` 多 `problems`，`config_conflict` 多 `current` 或 `version`）。

#### 握手 `hello`

| 参数 | 类型 | 说明 |
|---|---|---|
| `protocol` | 两个非负整数 `[最低, 最高]` | 头支持的主版本范围，必写 |
| `head` | `{"kind": 字符串, "version": 字符串}` | 头的种类和版本，必写，只记进运行日志 |
| `locale` | 字符串，可以不写 | 头所在系统的语言（BCP 47 或者 `LANG` 的写法都行）。`ui.language` 是 `auto` 时照它定这个连接的语言：`zh` 开头的（区分大小写）是 `zh`，`ja` 开头的是 `ja`，别的、没写的是 `en`（施工 8-2） |
| `caps` | 对象，不写是 `{}` | 现在只看 `input`（布尔，不写是 `false`）：这个头能不能让人输入。别的格不理 |
| `token` | 字符串，可以不写 | 本机令牌（`ipc.md`）；本机的头出示它 |
| `code` | 字符串，可以不写 | 一次性码：64 位小写十六进制（施工 W-8，`web-module.md`「怎么走」第一条） |
| `login` | 字符串，可以不写 | 登录令牌：64 位小写十六进制（施工 W-8） |
| `user`、`password` | 字符串，可以不写 | 网页登录用的用户名、密码，一起写（施工 W-8） |

回应：

| 格 | 值 |
|---|---|
| `protocol` | 选定的主版本：`1`，核心只支持这一个 |
| `core` | `{"version": <核心的版本号>}` |
| `account` | 你是谁：管理员的账号，核心里固定是 `admin`（`core.md`）；核心拉起的、清单声明了系统账号的包的扩展是那个系统账号，账号名是包的编号（施工 O-4 下，`packages.md`） |
| `host` | `{"home": <系统的家目录>, "platform": "linux" 或 "macos" 或 "windows", "workspace": <这个账号的工作区>}`，总有（施工 W-3，`web-module.md`「四、路径」）：`home` 是核心起来时拿到的系统的家目录，照原样，读不出来的是 `null`；`platform` 是核心所在的平台；`workspace` 是这个账号的工作区，换成真实的位置（管理员、系统账号的工作区核心起来时就建好了，`core.md`、施工 O-4 下；握手不另外建） |
| `language` | `zh`、`en`、`ja` 之一：这个连接给人看的字用哪种（施工 8-2，`config.md` 第二条第 8 条）。`ui.language` 的最终值（默认值、系统配置、个人设置）定了的就是它，`auto` 的照 `locale`。`ui.language` 改了，连接下一句就照新的说，不用再握手（施工 8-4）：回应里这一格只是握手那一刻的。核心拒绝时的话只有中文、英文，`ja` 的照英文；配置的名字、说明、报错的话有日文 |
| `config_errors` | 系统配置、个人设置、密钥文件（施工 8-5）里现在有几处错误（不算警告，施工 8-2）。没有的不写 |
| `config` | 只有核心拉起的扩展的连接有（施工 9-4 下下，`extensions.md`「配置」）：这个包自己的配置项的最终值，密钥是真值；之后变了推 `extension.config`（`{"keys": {键: 新值或 null}}`） |
| `setup` | `true`：用一次性码连上的，只能设用户名和密码（施工 W-8）。别的不写 |
| `login` | `{"expires": <时刻>, "token": <登录令牌>}`：用用户名、密码连上的才有（施工 W-8） |
| `sandbox` | 这台机器上的沙盒能不能用（核心起来时探的，`sandbox.md`）：`{"usable": true}`，或者 `{"usable": false, "reason": <原因>}`。原因是 `helper_missing`（主程序旁边没有助手）、`helper_failed`（助手跑不起来、超时、说的读不懂）、`no_mechanism`（探成了，这台机器上却没有能用的手段）之一（施工 5-4 下） |

1. 参数读不成（缺了必写的格、哪一格类型不对）：`bad_params`，连接不断。
2. `1` 不在 `[最低, 最高]` 里：`protocol_mismatch`，回完断开。
3. 凭据正好写一种（施工 W-8）：`token`，`code`，`login`，或者 `user` 加 `password`。写了不止一种、`user` 和 `password` 只写了一个：`bad_params`，回完断开。一种都没写、本机令牌不对：`bad_token`，回完断开；本机令牌长短要一样，每个字节都比，比到哪一个不一样都用一样长的时间。另外三种怎么验、被拒回什么（`bad_code`、`bad_login`、`bad_password`、`login_throttled`，都回完断开）照 `web-module.md`「怎么走」第一条。用一次性码连上的只能调 `hello`、`human.get`、`account.setup`，别的回 `setup_first`；用登录令牌、密码连上的，它靠的登录令牌作废了就断开。核心亲手拉起的扩展经标准输入输出连上，不看凭据，写了的也不看；它调 `extension.*` 回 `local_only`（施工 9-4 上，`extensions.md`）。
4. 过了：这个连接就是管理员。它发的命令都记成管理员发的（`by` 是 `{"kind":"person","account":"admin"}`），命令引起的事件，`cause` 是请求的 `id`（`kernel/events.md`）。
5. 握手以前：别的方法一律 `hello_first`，连接不断；读不懂的行照「请求」的表回；通知不理。
6. 握手以前的拒绝说英文；`hello` 本身被拒的，话照这一次报的 `locale` 说（读得出来的话，施工 4-9 再补三上）。过了的，话照回应的 `language` 说（施工 8-2）：`ui.language` 定成 `en` 的，报 `zh-CN` 的头也听英文。握手以后再发 `hello`：照样从第 1 条查起；过了，换成这一次算出的语言和能力；这一次被拒的，话照这一次报的语言说，没断开的还是上一次握手的语言和能力。
7. 握手的时限：连上 10 秒还没握手成的，断开（施工 4-9 再补三上）。不然一个连上不说话的本机进程，能让核心一直不空闲退出。

#### 方法

握手以后认这几个，别的方法回 `unknown_method`：

| 方法 | 做什么 |
|---|---|
| `session.create` | 造会话 |
| `venue.session` | 找回或者造一个通讯平台场所的主线会话（施工 O-3，`venues.md`） |
| `session.respond` | 照已经旁听记下的几条开一轮，带几块事实（施工 O-14 上，`venues.md`「照记下的几条开一轮」） |
| `session.note` | 只记几块事实，不开回合（施工 O-14 补，`venues.md`「记几块事实」） |
| `provide` | 核心拉起的扩展登记它提供的工具（施工 O-2 上，`providers.md`）；核心照登记反向调用 `tool.call`，超时、打断时发通知 `tool.cancel`（施工 O-2 下） |
| `venue.records` | 判官看的群聊记录：要判的那一条和它之前的几条，一行一条，和她看到的同一个写法（施工 O-24，`venues.md`「判官看的群聊记录」） |
| `events.append` | 往会话里记一条不带回合编号的事件：扩展自己的 `ext.*`、场所的 `venue.recalled`、`venue.delivered`（施工 O-13 上，`venues.md`） |
| `session.list` | 列出会话 |
| `session.send` | 说一句话 |
| `session.interrupt` | 打断在进行的回合 |
| `session.revert`、`session.unrevert` | 撤销、恢复最近一次撤销（`protocol/undo.md`） |
| `session.redo` | 重做最后一轮：撤掉它，把开它的话再发一次（施工 4-7 再补） |
| `session.compact` | 手动压缩：单开一轮只做压缩（施工 6-8） |
| `session.set_permission_level` | 切权限级别：开关只读，改常用的那一级（施工 3-8 再补） |
| `session.clear` | 清空上下文：单开一轮压成一个空的检查点，不请求模型（施工 6-8 补） |
| `session.recap` | 要一句回顾：这个会话在做什么、做完了什么、卡在哪（施工 3-8 四补） |
| `persona.list`、`persona.get` | 列出人格、读一个人格叠好的样子（施工 P-1 上，`personas.md`） |
| `preset.list`、`preset.get` | 列出预设、读一个预设叠好的样子（施工 P-2 上，`presets.md`） |
| `preset.set`、`preset.delete` | 新建、改一个预设（只写你家目录那一层），删掉你那一层（施工 P-3 中，`presets.md`「改」） |
| `persona.set`、`persona.read`、`persona.delete` | 新建、改一个人格（只写你家目录那一层），读一份提示词的原文和版本，删掉你那一层（挪进回收处）（施工 P-3 下，`personas.md`「改」） |
| `package.list` | 列出起来时读到的软件包清单（施工 9-1 上，`packages.md`）；装卸以后当场照新的，卸掉的出厂的带 `removed`（施工 F-5 上） |
| `package.install`、`package.remove` | 装、卸软件包，当场生效（施工 F-5 上，`packages.md`「装卸」） |
| `extension.status`、`extension.enable`、`extension.disable`、`extension.restart` | 核心拉起的扩展：列状态、开、关、重启（施工 9-4 上，`extensions.md`） |
| `check` | 查人手写的文件：配置、密钥文件、人格，照磁盘上现在的字（施工 8-30，`cli/check.md`） |
| `memory.list`、`memory.search`、`memory.remember`、`memory.update`、`memory.forget` | 人不经过她列、搜、记、改、忘和清空记忆（施工 R-3 补，`memory.md`「协议」） |
| `command.run` | 执行一条斜杠命令：头把人打的原文交过来，核心认、判谁能用、执行（施工 O-6） |
| `command.catalog` | 列核心认的斜杠命令，给头的命令菜单、`/help`；带会话的只列这个会话里真能用的（施工 O-6 补） |
| `session.answer` | 回答一次确认（允许这一次、本会话都允许、拒绝），或者一组题（施工 D-1） |
| `job.stop` | 停掉一个后台命令或者子代理（施工 7-4） |
| `job.output` | 读一条后台命令到这时为止的输出（施工 7-4 补） |
| `blob.put` | 传一个附件，存成 blob（施工 3-9 三补） |
| `account.setup_code` | 要一个一次性码：第一次给管理员设网页登录的用户名和密码、忘了密码重设；只给出示本机令牌的连接（施工 W-8） |
| `account.setup` | 用一次性码连上的设用户名和密码，换一个登录令牌（施工 W-8） |
| `account.logout` | 作废登录令牌：这一个，或者 `all` 全部（施工 W-8） |
| `blob.open`、`blob.write`、`blob.close` | 分块传一个附件，最后存成 blob，回应和 `blob.put` 一样；跟着连接走，60 秒不写、连接断了都作废（施工 W-5） |
| `blob.get` | 分块读这个账号的一个 blob：照属主给，不照会话（施工 W-6） |
| `fs.list` | 列一层目录：数据根只有账号自己的工作区能列（施工 W-2） |
| `fs.find` | 在一个目录里模糊找文件：数据根只有账号自己的工作区能找（施工 W-2） |
| `fs.realpath` | 一个路径换成真实的位置：不查边界，落在数据根里的照样换（施工 W-3） |
| `fs.read` | 分块读本机的一份文件：数据根只有账号自己的工作区能读（施工 W-6） |
| `session.set_meta` | 改标题、置顶（施工 3-8 三补） |
| `session.set_workspace` | 换会话在哪个目录干活（施工 9-7 上） |
| `session.configure` | 换模型，下一个回合开始生效（施工 8-10，`models.md`「协议」） |
| `session.delete` | 删除会话：挪进回收处，留 7 天（施工 3-8 三补） |
| `config.schema` | 配置清单，名字和说明照这个连接的语言（施工 8-2，`config.md`「协议」）；选项可以带 `note`：只有核心查得出的才有，头接在名字后面暗色写（施工 R-5 再补）；可以带 `available: false`：核心查得出用不了的才有，头画成灰的、选不了（施工 R-5 三补） |
| `config.get` | 最终值和来源，每一份文件在哪、版本，现在的全部问题；带 `cwd` 的算上那个目录的项目配置（施工 8-2）。`files` 多 `secrets`，只有 `file`；问题里有密钥文件的、引用取不到的（施工 8-5） |
| `config.check` | 把一段字当成一层的配置查，不生效（施工 8-2） |
| `config.set` | 在系统配置或个人设置里改一项或几项、恢复默认，或者整份换掉；只动那几项，落了盘、记了日志才回应（施工 8-3） |
| `config.trust` | 信任、不信任一份项目配置，带人看过的那一份的版本（施工 8-3） |
| `secret.set` | 写入或者换掉一个密钥（`name`、`value`），落了盘、记了日志才回应 `{"replaced"}`（施工 8-5，`config.md`「协议」） |
| `secret.delete` | 删掉一个密钥（`name`），回应 `{}`（施工 8-5） |
| `secret.list` | 密钥的名字、设没设、谁在用（`used_by`），从不交出值（施工 8-5） |
| `model.list` | 配好的供应商和模型，每一格资料的值和来源、状态，在用的目录（施工 8-7）；池、两种用途（施工 8-8：`pools`、`uses` 多 `vision`；8-8 补去掉 `tiers`，池多 `subagent`、`description`）；模型、key 的状态多 `cooling`，带 `until`、`class`（施工 8-9）；每个模型的 `facts` 多 `effort`（施工 8-18），多一格 `key`（8-18（补））。参数 `provider`（只看这一家）、`refresh`（先拉一遍供应商的模型列表）都可以不写；形状照 `models.md`「协议」`model.list` |
| `provider.detect` | 找现成的：核心的环境里设了的 key（不交值）、本机跑着的模型服务、找了哪些环境变量（施工 8-11）；形状照 `models.md`「协议」 |
| `provider.catalog` | 搜目录和档案里的供应商：`query`、`limit` 都可以不写；每一家能不能用、在不在本机（施工 8-11）；`featured` 只要常用的几家（施工 8-11 再补） |
| `provider.test` | 试一家：配好了的（`provider`）或者还没写进配置的（`candidate`），列模型、真发一句、收到第一段正文就停，交回成没成、哪一步、出错；会花一点额度（施工 8-11） |
| `model.call` | 经一次性入口叫一次模型或池，拿整段回答和用量；不进任何会话；会花额度（施工 8-20，`models.md`「协议」、「怎么走」第十二条）。8-15 起每发出去一次记一笔账：这个连接的账号的账号日志、用量汇总 |
| `usage.query` | 查用量和金额：照人、场所、模型、天、会话、用途分组，金额照币种各加各的；先补再查（施工 8-15，`models.md`「协议」、「怎么走」第九条） |
| `human.get` | 给人看的字：工具的样子、说法的模板原文，照这个连接的语言；不带 `config`（施工 W-1） |
| `mermaid.render` | mermaid 源码画成 SVG。编进了 `mermaid` 包才有，没编进来回 `unknown_method`（施工 W-4，`mermaid.md`） |
| `link.preview` | 一个链接的卡片：标题、简介、图（存成 blob）。编进了 `net` 包才有，没编进来回 `unknown_method`；在后台答（施工 W-7，`net.md`） |
| `subscribe`、`unsubscribe` | 订阅、取消订阅会话的事件流；配置、会话列表、扩展的状态的推送 |
| `view.page` | 历史按页读：从末尾一页页往前，页的边界落在回合之间（施工 9-6 下） |
| `view.detail` | 一次调用的完整差异：改了哪些文件、改前改后的统一格式差异（施工 9-6 三补） |

带 `session` 的，它要合会话编号的写法：UUID 的标准写法，小写十六进制，8-4-4-4-12；不合的 `bad_params`。找会话照下面「会话表」。

**`session.create`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串或 `null`，可以不写 | 照哪个人格造；写 `null` 的明着无人格，不写的照这时的 `persona.default`（施工 P-1 上，`personas.md`），没设的、指着没有的无人格（施工 P-4 上：预设不再管默认人格，出厂不设默认人格）。无人格的会话 system 里没有人设、记忆不生效 |
| `preset` | 字符串，可以不写 | 照哪个预设造（施工 P-2 上，`presets.md`）：不写、写 `null` 的照这时的 `preset.default`，都没写是出厂的 `full`。记进 `session.created` 的 `preset`，以后不改 |
| `cwd` | 字符串，必写 | 头的工作目录，人看到的那种写法，例如 `~/src/miyu` |
| `oneshot` | 布尔，不写是 `false` | 一次性的：`miyu ask` 开的写 `true`，记进 `session.created` |
| `chosen` | 布尔，不写是 `false` | `cwd` 是人明着选的（网页里选的、终端里明着给的路径）：太宽的照用、回应带 `wide`；不写的是头自己带上的（终端启动时的当前目录），太宽照旧退回（施工 9-7 补，「工作目录太宽」） |
| `dirs` | 字符串的数组，可以不写 | 加进来的目录：和工作区一样能读能写（「加进来的目录」（施工 5-10 上））。不写是没有 |
| `model` | 字符串，可以不写 | 用哪个模型：模型 `<供应商>/<模型>` 或池 `@<池>`（施工 8-8，`models.md`「两种写法」；挡位 8-8 补去掉了）。照这时的配置查过，记进 `session.created` 的 `model`；不写、写 `null` 的照这时的 `models.chat`，那也没配的不写 |
| `memory` | 字符串，可以不写 | 记忆的范围（施工 R-3 下，`memory.md`「范围」）：`persona`（跟着人格）、`session`（只在这个会话里）、`off`（不召回也不记）。记进策略快照，以后不改。不写、写 `null` 的照人格的 `persona.toml` 的 `[memory] scope`，那也没写的是 `persona` |

回应：`session` 新会话的编号；`events` 是 `[1]`，就是 `session.created` 那一条；`cwd` 是会话实际在哪个目录里干活（「工作目录太宽」）；`wide`：太宽、照人选的用着的才有，是 `true`（施工 9-7 补）；`untrusted_project`：这个目录找得到项目配置、又还没问过信不信任（`trust.toml` 里没有这个仓库，或者记的内容和现在的不一样），写它在哪，写法同配置来源的 `file`（家目录下的写成 `~/…`）。信任着的、选了不信任的、没有项目配置的不写（施工 8-2，`config.md` 第三条第 2 条）。

1. 编号是 UUIDv7：前 48 位是造的这一刻（毫秒），同一个核心造的照先后排。
2. 属主是管理员，场所是 `local`，权限从「工作区」开始，只读照 `permission.start_read_only` 的最终值：照实际干活的目录算，带上信任着的项目配置（施工 8-2，`config.md` 第二条第 9 条），没写的是不只读；有没有人能确认，照这个连接握手时的 `caps.input`；环境是核心所在机器此刻的时区偏移（到分钟）和实际干活的目录。
3. `session.created` 落了盘才回应。
4. 同一个命令编号再发：记着最近 1024 个造会话的编号，是其中之一的，交回上一次造的那一个，不再造；`cwd` 照这一次报的算。核心重启以后，第一次造会话时，从最新的 1024 个会话的 `session.created` 里把编号补回来（它的 `cause` 就是造会话的命令编号，施工 4-9 再补三上），在阻塞线程里读。
5. `memory` 不是这三种字符串之一（大小写也算）：`bad_params`，什么都不造（施工 R-3 下）。`model` 不是字符串：`bad_params`。解析不出（写法不对、没有这家供应商、没有这个池、池里一个成员都认不出）：`unknown_model`，什么都不造（施工 8-8，`models.md`「协议」）。先解析再找同一个命令编号造过的。
6. 人格的编号不合写法（小写英文字母开头，只有小写字母、数字、`-`、`_`，最长 64 字节）：`bad_params`。三层（出厂、系统区、管理员家目录，`personas.md`）都没有这个人格：`unknown_persona`，默认人格指着没有的也一样，不悄悄换。人格的文件写错了：`persona_invalid`，`data.problem` 写明哪一层、哪个文件第几行（施工 P-1 上）。预设照同样的规矩，先于人格找（施工 P-2 上，`presets.md`）：编号不合写法 `bad_params`，三层都没有（默认预设指着没有的也一样，Y12）`unknown_preset`，文件写错 `preset_invalid`（`data.problem` 写明哪一层、哪个文件第几行），都不造。人格的文件、随核心附带的 `core/` 下的字读不了（安装坏了）：`internal_error`（施工 4-9 再补三上）。别的造不成（策略造不出来、磁盘上建不成、`session.created` 没落盘）：`internal_error`，原因记进运行日志。

**`session.list`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `oneshot` | 布尔，不写是 `false` | `true` 只要一次性的 |
| `limit` | 非负整数，可以不写 | 最多几个；不写是全部，`0` 是一个都不要 |

回应：`{"sessions":[{"busy":true,"cwd":"<工作目录>","last_active":"<时刻>","oneshot":<布尔>,"parent":<编号或 null>,"pinned":true,"session":"<编号>","title":"<标题>"}, …]}`。没有标题、说过话的多一格 `preview`：第一句话的第一行，最多 50 个字，有标题就不带（施工 9-5，网页的侧边栏照它显示没起名的会话，`store/index.md`「一行记什么」）。`parent` 是子会话的父会话，主会话写 `null`（施工 7-5，`agents.md`）。`title`、`pinned` 照日志里的 `session.meta_changed` 算（施工 3-8 三补）：有标题的才写 `title`，置顶的才写 `pinned`（写 `true`），没有的不写。`cwd`、`last_active` 总有，`busy` 忙的才写（写 `true`）（施工 C-3，`cross-session.md`）。`persona` 是会话用哪个人格（施工 P-1 下，`session.created` 的那一格），`preset` 是用哪个预设（施工 P-2 上，同样照 `session.created`），以前的日志没有的不写；会话列表的推送 `sessions.changed` 的一项同样带它：

```json
{"busy":true,"cwd":"~/src/miyu","last_active":"2026-10-01T06:03:12.345Z","oneshot":false,"parent":null,"session":"0192f3a0-2222-7abc-8def-5566778899aa","title":"修 CI"}
```

1. 只列管理员的本机会话（通讯平台的场所会话不列，施工 O-3，`venues.md`），从新到旧：照编号倒着排，编号照造的先后。子会话也列，和主会话排在一起。删了的（挪进了回收处）不列。
2. 读每个会话日志的第一条，只读不写。跳过：目录名不合会话编号写法的、没有日志的（第一行还没写完的也算没有）、第一条读不出来的（记一条运行日志）、第一条不是 `session.created` 的。
3. 列进去的，再只读地把整份日志读一遍（`store.md` 第 7 条），`session.meta_changed` 一条条盖上去：写了 `title` 的换成它（空的是去掉），写了 `pinned` 的换成它；撤掉的回合里改的也算，改名不是对话的一部分。后面读不下去的（日志坏了）：记一条运行日志，照坏的那一段以前的算（一段查过了才交出来，只有一段的就当没有），照样列。
   - 第 2 到 4 条照日志算的，读会话列表的索引（施工 3-8 七补，`store/index.md`「怎么走」第 3 条）：索引里有这一行、照到的就是日志现在的末尾的，直接用，不读第一条、不整份读；日志比它长的只读多出来的那一截；没有这一行、对不上的（日志比记的短了、段对不上），这一个会话照上面整份读，读完写进索引。结果和整份读的一字不差。
4. 同一遍里再算两样（施工 C-3）：`cwd` 是日志里最后一条带 `cwd` 的 `turn.started` 的，没有就照 `session.created` 的，都没有（很早以前的日志）写 `~`，和「会话表」第 5 条同一个认法（同一个函数）；头报来的写法，不换成真实的位置。`last_active` 是日志最后一条事件的 `at`，哪种事件都算；读不下去的照坏的那一段以前的，只有一段、它坏了的是 `session.created` 的时刻。
5. `busy`（施工 C-3）：列之前拿着会话表的锁看一眼，在表里、这时有回合在进行的（和「空闲和停下」看的是同一样：内核说不空闲，结束了 `turn.ended` 还没落盘、在等人确认、等人回答、改回文件的都算）写 `true`。没载入的都是闲。
6. 她用 `sessions` 列会话（`tools/sessions.md`）是这同一个函数，只要主会话、不含她自己，排法不同：两边的字对得上。
7. `limit` 数的是列进去的。
8. 读不了放会话的目录：`internal_error`。

**`session.send`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 要说的话，照原样成一块文字；空的一块都没有 |
| `urgent` | 布尔，不写是 `false` | 急着插话 |
| `cwd`、`dirs` | 可以不写 | 施工 9-7 上起照收不理：工作区是会话的属性，换它走 `session.set_workspace`。一个连接第一次收到带它们的，记一行运行日志 `WARN session.send cwd ignored`，看得出谁还在发 |
| `attachments` | 数组，可以不写 | 附件（施工 3-9 三补）：`blob.put` 的回应，照先后。每一项要 `blob`、`name`、`media_type`；可以另写 `path`：这个附件原来在本机的哪儿，就是 `blob.put` 传的那个路径（施工 3-9 五补，剪贴板贴的、传 `data` 的不写），绝对路径（`/`、`~/`、Windows 的盘符或 `\\` 开头）、没有控制字符、最多 4096 字节，不对的 `bad_params`；核心只查写法、不碰磁盘，记进块里，模型看不了这个附件时占位那一句带上它。别的格不看 |
| `from` | 字符串，可以不写 | 别的 harness 报的自己的名字（施工 7-10，`agents.md` 第十一条第 4 条）：写了的，这一句是它说的，不是本人 |
| `as` | 对象，可以不写 | 代表通讯平台上的人（施工 O-3，`venues.md`）：`{"external": <平台身份>, "role": "manager"|"member"}`。只给场所会话，场所会话也只收带它的（不带的回 `venue_session`）；和 `from` 不能一起写 |
| `venue` | 对象，可以不写 | 通讯平台上的一条消息的那几格（施工 O-13 上，`venues.md`「场所的格」）：只跟着 `as` 来，原样记进 `message.user` 的 `venue`；`ambient` 的只记下，不开回合，回合进行中也不排进这一轮。不带 `as` 的、格写错的 `bad_params`，什么都不记 |

回应：`events` 是 `[<这一句 message.user 的序号>]`；`cwd` 是会话现在实际在哪个目录里干活；`untrusted_project` 照 `session.create` 的写法，照这时实际干活的目录找（施工 8-2）。

1. 没有回合在进行的，这一句开一轮；有的，排队，`urgent` 的插进下一步（`kernel/session.md`）。带 `from` 的照第 7 条。
2. 开的那一轮，`turn.started` 的 `cause` 是这一条的 `id`：头照它认出自己的那一轮。
3. `text` 是空的、又没有附件、也没带场所的东西（`venue.media`）：`empty_message`。先找会话，找不到的回的是找不到。只有附件、`text` 是空的，也是一句话；只有 `venue.media` 的场所消息也是（施工 O-13 补：群里只发一张图、一个表情）。
4. 附件变成内容块，照先后接在文字那一块后面（施工 3-9 三补）：核心照 blob 的内容照 `blob.put` 第 4 条再认一遍，同一份代码。图片是图片块，宽、高、媒体类型照这一次量的，头交回来的 `kind`、`width`、`height` 不算，`name` 照交回来的（施工 3-9 四补：一句话附了几张图，她分得清哪张是哪个文件）；文件是文件块，`name` 照交回来的，媒体类型照交回来的再过一遍第 4 条（内容是 PDF 的写 `application/pdf`，交回来写成 PDF、图片而内容不是的照内容认）。
5. 附件先查，再找会话：一项缺了格、格不合写法（`kernel/ids.md`）：`bad_params`；blob 不在管理员的 blob 里：`unknown_attachment`；读不出来（坏了、读不了）：`internal_error`，记一条运行日志；是超了上限的图（不是 `blob.put` 传的 blob 才会有）：`attachment_too_big`。拒了的，会话里什么都不送。
6. `from`（施工 7-10）：写了的，这条 `message.user` 的 `by` 记成 `{"kind":"harness","name":<名字>}`；不写的、写 `null` 的照旧记成本人。名字照短名字的规矩收（`kernel/ids.md`）：先去掉控制字符（Unicode 的 Cc 类），再截到 128 字节以内，不截断一个字；剩下是空的，`bad_params`。不是字符串的（数字、数组……）也是 `bad_params`。名字不核对，照它报的记；给模型看之前照不可信的文本转义（`kernel/request.md`「别的 harness 发来的话」）。它先查，查在附件前面：拒了的什么都不送。
7. 带 `from` 的这一句是别处来的，内核照「别的 harness 发来的话」收（`kernel/session.md`，和子代理的留言一样）：她闲着开一轮，`turn.started` 的 `cause` 是这一条的 `id`；正忙的，下一步看到；不带回合编号，打断时不撤回，不作废在等本人答的题。`urgent` 不看。附件照收，和本人附的一样。
8. 只有 `session.send` 收 `from`：`session.create`、`session.redo` 写了也不理（「请求」最后一条）。会话照旧是本人造的；重做的撤销记成发重做的人，重发的只有人说的话（`by` 照原来的），别的 harness 发来的话开的那一轮重做不了（`session.redo` 第 3 条）。

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
3. 一个最多 20 MiB（20,971,520 字节），多的 `attachment_too_big`；读到上限多一个字节就停，不整份读进来。`data` 放在一行 JSON 里，一行最长 1 MiB（「一行一条」），所以最多七百多 KiB，大的传 `path`；更大的（或者远程的头想一块一块传）用 `blob.open`、`blob.write`、`blob.close`（施工 W-5，下面）。
4. 认是什么，照内容，不看扩展名：
   1. 开头是四种图之一（PNG、JPEG、GIF、WebP）、量得出宽高的：图片，媒体类型照认出的，`media_type` 写了也不算。超过 5 MiB、哪一边超过 8000 像素：`attachment_too_big`，正好在线上的收。认法和上限和 `read` 读图片是同一份代码（`crates/miyu-tool/src/picture.rs`，`tools/read.md`「读图片」）：图跟着对话每次都发，被供应商拒掉的图会让这个会话以后的请求都失败。
   2. 别的都是文件。开头是 `%PDF-` 的，媒体类型是 `application/pdf`，`media_type` 写了也不算。
   3. 别的：`media_type` 写了的照写的，只是写成 `application/pdf`、`image/…` 的不算（驱动照它们把内容当 PDF、当图发，内容不是，供应商会拒）；没写、不算的，整份是 UTF-8、没有 NUL 字节的是 `text/plain`（和驱动认文本文件是同一条，`drivers/openai-chat.md` 第 9 条），别的 `application/octet-stream`。扩展名不认：头知道得更准的（例如浏览器给的类型）自己写 `media_type`（施工 3-9 三补定：扩展名的表是一份写死的名单，驱动给模型看的只有文件名和内容，用不上它）。
5. 存成管理员的 blob（`store.md` 第九条），落了盘才回应；同一份内容再传，还是那一个 blob。存不下来：`internal_error`，记一条运行日志。
6. 不碰会话，没有命令编号的去重：内容一样，存几次都是同一个。传了没发的留在 blob 里，随存储的回收那一步清。

**`blob.open`、`blob.write`、`blob.close`**（施工 W-5，`web-module.md`「六、分块上传」）：分块传一个附件，最后存成 blob，回应和 `blob.put` 一样。

| 方法 | 参数 | 回应 |
|---|---|---|
| `blob.open` | `name`（必写，照 `blob.put` 第 1 条）、`media_type`（可以不写，照 `blob.put` 第 1 条）、`size`（必写，非负整数：一共几个字节） | `{"upload": <上传编号>}` |
| `blob.write` | `upload`、`offset`（非负整数，从 0 数）、`data`（base64，一块最多 512 KiB） | `{"received": <一共收到几个字节>}` |
| `blob.close` | `upload` | 照 `blob.put` 的回应：`blob`、`name`、`media_type`、`kind`，图片另有 `width`、`height` |

1. `blob.open`：`name`、`media_type` 照 `blob.put` 第 1 条的写法查；`size` 超过 20 MiB（20,971,520 字节）当场 `attachment_too_big`。这个连接同时开着 4 个的：`too_many_uploads`。成了交回上传编号（16 位小写十六进制），在这个账号的 `blobs/tmp/` 里建暂存文件 `upload-<编号>`。
2. `blob.write`：`offset` 要正好等于已经收到的字节数，不对回 `upload_offset`，`data.received` 写已经收到几个，头从那里接着传。`data` 不是 base64、解出来超过 512 KiB、加上它超过 `size`：`bad_params`。写进暂存文件，边写边算 SHA-256。
3. `blob.close`：收到的不够 `size`：`upload_incomplete`（`data.received`）。够了：照 `blob.put` 第 4 条认是什么、查图片的上限，照第 5 条存：暂存文件改名进位置，同一份内容已经有了的，删掉暂存的，还是那一个 blob。
4. 上传跟着连接走：编号只认开它的那个连接，别的连接拿来用回 `upload_unknown`。连接断了，它开的上传全部作废、删掉暂存文件。60 秒没有 `blob.write` 的，也作废。`close` 以后编号作废。
5. 同一个连接上的请求本来就一条条办（「一个连接」第 1 条），一个上传不会同时写两块。一个附件拆成 40 块左右，一块一个来回。
6. 核心起来时清掉 `blobs/tmp/` 里的 `upload-*`：崩了、被杀留下的。
7. 不碰会话，不进会话的日志，和 `blob.put` 第 6 条一样。附件的大小上限和 `blob.put` 同一个数，一处定义（`crates/miyu-endpoint/src/attach.rs` 的 `LIMIT`）。

**`fs.list`**（施工 W-2，`web-module.md`「三、列文件、找文件」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 相对的路径照它接：绝对路径，或者 `~`、`~/…`，头报的那种写法 |
| `dir` | 字符串，不写是 `""` | 打的那一截目录：`~` 打头的照家目录，绝对的照原样，别的照 `cwd` |
| `prefix` | 字符串，不写是 `""` | 名字的开头 |

回应 `{"items": […], "partial": <布尔>}`：`items` 每一条 `{"dir": <布尔>, "full": <绝对路径>, "marks": [<第几个字>…], "path": <列表上写的>, "size": <字节数>}`，`size` 只有文件才有；`partial` 列没列全。

```json
{"building":false,"items":[{"dir":false,"full":"<家目录>/src/miyu/src/main.rs","marks":[4,5,6,7],"path":"src/main.rs","size":2048}],"partial":false}
```

1. `cwd` 照 `fs.md` 换成真实的位置（`~` 照家目录接），要是一个目录。换不成、不在、不是目录：`path_unreadable`。落在数据根里、又不在这个账号的工作区里：`path_forbidden`。
2. `dir` 照上面的参数表接好、换成真实的位置，只读那一层；同样要是一个目录、同样落进「谁都不能碰」那一片的 `path_forbidden`。
3. 名字照开头对 `prefix`，大小写不论。点开头的藏起来，`prefix` 以 `.` 开头才列。目录在前、文件在后，各照名字排（大小写不论）。目录的 `path` 后面带 `/`。最多 50 条，多了截掉、`partial` 是 `true`。`marks` 是 `path` 的前几个字，`prefix` 有几个字就几个。
4. 这一层里有东西落进了「谁都不能碰」那一片的（例如往上列到数据根的上级，列出来的一层恰好含着数据根自己），单单那一条不列，旁边的照样列。
5. `full` 照平台的写法（Windows 上是 `C:\…`）；`size` 照文件现在的大小，读不出来的不写。

**`fs.find`**（施工 W-2，`web-module.md`「三、列文件、找文件」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 在哪个目录里找，写法同上 |
| `query` | 字符串，不写是 `""` | 打的字 |
| `fresh` | 布尔，不写是 `false` | 头开列表时写 `true`：清单建好 10 秒以上的重建 |

回应同 `fs.list`，另有 `building`：清单还在建。

1. `cwd` 照 `fs.list` 第 1 条换、查边界。
2. 在 `cwd` 里建一份清单：`ignore` 库，和核心的 `glob`、`grep` 同一套，认 `.gitignore`（不要求是 git 仓库），跳过隐藏目录和出厂名单里的 `node_modules`、`target`，跳过数据根（工作区除外，工作区常常就在数据根里面），不跟链接；最深 8 层，最多 20000 个，收满就停、`partial` 是 `true`。`path` 是相对 `cwd` 的，用 `/` 连，目录后面带 `/`。
3. 清单在后台线程里建，不挡别的请求。还没建完，照已经建好的那一部分答，`building` 是 `true`；头隔 200 毫秒再问，直到 `false`。
4. 什么时候重建：这个目录还没有清单；`fresh` 是 `true`、清单建好 10 秒以上。核心最多记 4 个目录的清单，多了丢最久没用的。同一个目录同时来两次，第二次拿到第一次那一份。
5. 怎么排：打的字照先后都在 `path` 里（大小写不论）才列。先试整个落在文件名里，落不下再从路径开头找；每个字对上 1 分，落在文件名里多 3 分，在一段的开头（路径的头一个字，或者前面是 `/`、`-`、`_`、`.`、空格）多 8 分，和上一个字连着多 5 分；文件名去掉扩展名正好是打的字多 100 分。分高的在前，一样的路径短的在前，再一样的照字排。最多 50 条，对得上的比截出来的还多也算 `partial`。`query` 是空的都对得上、0 分。
6. 建清单时读不了一层目录的（没有权限这类），跳过它接着建，不算整份失败。

**`fs.realpath`**（施工 W-3，`web-module.md`「四、路径」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `path` | 字符串，必写 | 要换的路径：绝对的，或者 `~`、`~/…`；相对的要配 `cwd` |
| `cwd` | 字符串，可以不写 | `path` 是相对的才要：接在它前面，写法同 `fs.list` 的 `cwd` |

回应 `{"path": <真实的位置>}`。

1. `~` 照家目录接；绝对的照原样；相对的接在 `cwd` 上，`cwd` 自己也可能是 `~`、相对的写法，照同一条路换。都不是的（相对、又没给 `cwd`）：`bad_params`。
2. 从它自己往上找第一层在的，那一层换成真实的位置，后面几段原样接上：还没建出来的目录，建出来以后就是这个位置。一层都不在（Windows 上盘符都没有）：`path_unreadable`。
3. 不查边界，只说位置、不读内容：落在数据根里的照样换。

**`blob.get`**（施工 W-6，`web-module.md`「七、分块读」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `blob` | 字符串，必写 | 内容哈希 |
| `offset` | 非负整数，不写是 0 | 从第几个字节起读 |
| `length` | 非负整数，不写是 512 KiB（524288） | 最多读几个字节，最多 512 KiB；写 0 只问大小 |

回应 `{"data": <这一段的 base64>, "size": <一共几个字节>}`。

```json
{"id":"g1","jsonrpc":"2.0","result":{"data":"MzQ1Ng==","size":10}}
```

1. 这个账号的 blob，照属主给，不照会话：没有这个 blob，`unknown_blob`。
2. 从 `offset` 起读 `length` 个字节，读到结尾就停；`offset` 过了结尾的，`data` 是空的。`size` 是打开那一刻的大小。`length` 写 0 只回 `size`，`data` 是空的。`length` 超过 512 KiB：`bad_params`。
3. 不重新核对整份内容的哈希：核对在核心自己用整份内容的时候（`store.md` 第十条）。
4. 在阻塞线程里读。

**`fs.read`**（施工 W-6，`web-module.md`「七、分块读」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `path` | 字符串，必写 | 绝对路径，或者 `~`、`~/…`；相对的 `bad_params` |
| `offset` | 非负整数，不写是 0 | 同 `blob.get` |
| `length` | 非负整数，不写是 512 KiB（524288） | 同 `blob.get` |

回应同 `blob.get`。

1. `path` 照 `fs.md` 换成真实的位置，`~` 照家目录接；相对的（没有 `cwd` 可接）：`bad_params`。落在数据根里、又不在这个账号的工作区里：`path_forbidden`。
2. 照 `fs.md` 第四节安全地打开，路上一层链接都不跟；换不成真实的位置、没有、不是普通文件、没有权限：`path_unreadable`。
3. 读法同 `blob.get` 第 2、3、4 条。

**`session.interrupt`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `queued` | `"send"`、`"return"` 或 `"keep"`，必写 | 排着队的消息：打断以后马上发，退回来，还是留着（留在日志里、不撤回、不接着开新的一轮，施工 O-6） |

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
3. 最后一轮不是人说的话开的（回报叫醒的、别的 harness 发来的话开的（施工 7-10）、别的会话发来的话开的（施工 C-2）、空了的通知开的（施工 C-6）、手动压缩、清空、重启以后接着干的），或者一轮都没有：`not_redoable`，头把它那一句当一条提示通知显示。有回合在进行：`turn_running`。换过的那一句一块都不剩（原来只有字、`text` 是空的，原来只有附件、`attachments` 是空的）：`empty_message`。正在改回文件：`restoring`。附件照 `session.send` 第 5 条先查、再找会话：`bad_params`、`unknown_attachment`、`internal_error`、`attachment_too_big`，拒了的什么都不送。先找会话，找不到的回的是找不到。
4. 重做以后恢复不了：新的一轮开了（`session.unrevert` 回 `nothing_to_unrevert`）。
5. `text` 不是字符串（数字、数组……）、`attachments` 不是数组：`bad_params`；写 `null` 等于没写（「请求」最后一条）。不收 `cwd`、`dirs`：新的一轮照会话现在的环境，和 `session.revert` 一样（「会话表」第 5 条）。

**`session.compact`**（施工 6-8，`compaction.md` 第七条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `instructions` | 字符串，可以不写 | 人附的要求，例如「重点保留数据库设计的讨论」，原样交给内核；去掉前后空白是空的，当没写 |

回应：`events` 是 `[<那一轮 turn.started 的序号>]`，它落了盘就回，和 `session.send` 一样不等这一轮做完。压好了没有，看推送里的 `compaction.progress`、`compaction.done`、`context.compacted`、`turn.ended`。手里有一份提前压好的、没附要求的，直接换上：没有 `compaction.progress`，`compaction.done` 带 `prepared: true`（施工 6-11 上，`compaction.md` 第十五条）。

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

**`session.answer`**（施工 D-1，`kernel/asking.md`「回答」，`session/guard.md`「判一次调用」第 7 条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `call` | 字符串，必写 | 回答的是哪一次调用：`tool.approval_requested`、`question.asked` 里的 `call_id` |
| `decision` | `"once"`、`"session"` 或 `"deny"`，可以不写 | 回答确认：允许这一次、本会话都允许、拒绝 |
| `reason` | 字符串，可以不写 | 拒绝的理由，她看得到；只有拒绝能带 |
| `answers` | 数组，可以不写 | 回答提问：照题目的先后一道一条 `{"picked": [选项的标题], "text": 自己写的}`，两格都可以不写（这道没答） |

回应：`{"events": [序号]}`，这一次追加的事件（决定、拒绝时的工具结果，回答）都落了盘才回。

1. 先查参数，再找会话：`decision`、`answers` 两样都写、都不写；回答提问却带了 `reason`；`call` 不合调用编号的写法；`decision` 不是这三种（`workspace`、大写的也算）：`bad_params`。「这个工作区以后都允许」以后再加（2026-09-28 项目主人定），协议先不认：内核的 `unknown_decision` 因此从协议上碰不到。
2. 交给内核（`Command::Answer`），`by` 取自连接（现在都是管理员），`cause` 是这一条的 `id`。内核拒的：这个调用没在等回答（答过了、了结了、不是这一种），`not_asking`；请求没提放行规则却选了本会话都允许，`no_rule`；允许却带了理由，`unexpected_reason`；回答和题目对不上，`bad_answer`（`kernel/asking.md`）。空的理由当没写。
3. 允许的，决定落了盘这个调用才跑；本会话都允许的，这个会话以后落在同一个范围里的写入不再问（`session/guard.md`「判一次调用」第 7 条）。拒绝的，再记一条 `denied` 的结果，写了理由的带上，她接着干（`11-权限与沙盒.md` A13）。
4. 头收起抽屉照推送：这个调用有了结果（`tool.result`），或者回答落了盘（`tool.approval_decided`、`question.answered`）。

**`session.clear`**（施工 6-8 补，`compaction.md` 第十四条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |

回应照 `session.compact`：`events` 是 `[<那一轮 turn.started 的序号>]`。那一轮的开头、空的检查点、结束同一批，都落了盘才回：回应到的时候，推送里已经有这三条（`turn.started` 没有 `trigger`，`context.compacted` 的 `trigger` 是 `clear`、`summary` 是空的，`turn.ended` 是 `completed`），没有 `compaction.progress`、`compaction.done`。

1. 开的那一轮，三条的 `cause` 都是这一条的 `id`：头照它认出自己的那一轮。
2. 有回合在进行：`turn_running`。上下文本来就是空的：`nothing_to_clear`（`compaction.md` 第十四条第 2 条），头把它那一句当一条提示通知显示。正在改回文件：`restoring`。先找会话，找不到的回的是找不到。
3. 撤掉那一轮（`session.revert`）上下文回到清空以前，回应里 `clears` 数它一次、`compactions` 不算它、没有 `said`（`protocol/undo.md`）。

**`command.run`**（施工 O-6，`venues.md`「斜杠命令」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 人打的原文，例如 `/stop` |
| `as` | 对象，可以不写 | 代表通讯平台上的人，同 `session.send` 的 `as`：只给场所会话，场所会话也只收带它的 |
| `cwd` | 字符串，可以不写 | 头所在的目录，绝对的或者 `~` 开头的：只用来接 `/workspace` 后面相对的路径（施工 9-7 下） |

回应 `{"command": "clear"|"stop"|"workspace"|"remember", "events": [...], "said": "<回执>"}`：`command` 是正名（别名换成了正名），`events` 是这一次追加的全部事件的序号、最后一条是记下的 `command.ran`，`said` 是回执那一句，照这个连接的语言（`ui.language`），头原样发给人。

1. 认法：开头的空白不算，原文要以 `/` 开头（不是的回 `bad_params`），名字紧跟着 `/`、到空白为止，后面跟的字去掉前后空白，只有 `/workspace`、`/remember` 用。认得的：`clear`（别名 `reset`）、`stop`、`workspace`（施工 9-7 下）、`remember`（施工 R-3 补）。认不出的回 `unknown_command`，`/` 后面是空白的也是。`cwd` 不是绝对的、也不是 `~` 开头的回 `bad_params`。
2. 谁能用：本机的会话（本机的头就是管理员本人）；场所会话里主人对应表认出的本人（记成带 `via` 的本人）、对应表里有的外部身份（群里的主人，`account`）、`role` 是 `manager` 的。别人回 `command_not_allowed`。`/workspace` 动的是沙盒能写的地方，只有主人本人能用（本机的会话、对应表认出的本人、群里的主人），管理的人回 `owner_only`。先查参数、再找会话、再判身份。
3. `/clear` 同 `session.clear`：内核拒的照原因回（`turn_running`、`nothing_to_clear`、`restoring`）。
4. `/stop` 全停：打断这一轮，排着的照 `keep` 留着；没有回合在进行的照样往下走。再停掉这个会话派出去的后台命令和子代理（同 `job.stop`，停的人记成说命令的人）。
5. `/workspace <路径>` 同 `session.set_workspace` 只换工作目录，加进来的目录照旧（施工 9-7 下）：绝对的、`~` 开头的照原样；相对的照 `cwd` 接成真实的位置（`/workspace .` 就是头所在的目录），没带 `cwd` 的照会话现在的工作区接；路径里的空白照留，不认引号。写错的照那几种原因拒绝（`path_unreadable`、`not_a_directory`、`path_forbidden`，核心读不出家目录时的 `~` 也是读不了）；太宽的照人选的用，回执说一声范围大（「工作区换到了 ~（范围很大）。」，施工 9-7 补，原来退回账号的工作区）；和现在一样的不记换，照样记下命令。不带路径的什么都不换，回执说现在在哪。
6. 执行了的记一条 `command.ran`（`kernel/events-bodies.md`），`cause` 是 `<id>/ran`；被拒的什么都不记。它不进模型的请求。
7. 同一个 `id` 再发只算一次，核心重启以后也是：回应和头一次一样。
8. `/remember <话>`（施工 R-3 补，`memory.md`「协议」）：名字后面跟的字是那一条，类 `user`，记进这个会话那一间，`by` 是打命令的人，出处空，听众是这个人；回执带编号（`commands/remembered`）。不请求模型。场所会话、范围 `off` 的回 `memory_unavailable`，`data.why` 是照连接语言说的为什么（施工 O-6 再补：没有人格的「没有人格的会话记忆不生效」，预设没开记忆的带上预设的名字，字在 `core/human/<语言>.json` 的 `commands/unavailable/…`，别的照拒绝的那一句）；空的 `bad_params`，超过 120 字的 `memory_too_long`。先判身份，再查这个会话有没有记忆，再查字。同一个 `id` 再发只记一次（记忆事件的 `cause`，`memory.*` 第 5 条）。

**`command.catalog`**（施工 O-6 补，网页的会话要的，终端也用）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，可以不写 | 哪个会话：写了只列这个连接在这个会话里打了不会被拒的 |

回应 `{"commands": [{"name": "clear", "aliases": ["reset"], "summary": "清空上下文"}, {"name": "workspace", "aliases": [], "summary": "切换工作区", "argument": "<路径>"}, …]}`：照名字排；`summary`、`argument` 照这个连接的语言，字在 `core/human/<语言>.json` 的 `commands/summary/<名字>`、`commands/argument/<名字>`；名字后面要跟字的（`workspace`、`remember`）才有 `argument`。

1. 不写 `session`：列核心认的全部（头还没造会话、`/help` 列全部时用）。
2. 写了：只列这个连接在这个会话里打了不会被拒的，和 `command.run` 第 2、8 条同一份判法（谁能用；`/workspace` 只给主人本人；`/remember` 要这个会话开着记忆）。用不了的不列（2026-10-09 项目主人定）：人打了菜单里没有的命令，头照样交给 `command.run`，照拒绝的 `data.why` 说为什么，不自己说「没有这个命令」。
3. 不推送：头换会话、打开命令菜单时问一次。头自己的命令（`help`、`theme` 这些）、软件包登记的同名命令由头自己拼、自己去重；核心这份里没有只有头懂的命令。

**`check`**（施工 8-30，`cli/check.md`）

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，可以不写 | 头现在的工作目录：照它找项目配置，相对的 `file` 照它接 |
| `file` | 字符串，可以不写 | 只查这一份 |

回应 `{"problems": [...]}`，一处一格：`kind`（`config`、`secrets`、`persona`、`preset`（施工 P-2 上）、`package`（施工 9-1 上））、`file`（照 `config.get` 的写法：数据根里的相对数据根，项目配置 `~/…`，出厂的人格写真的路径）、`code`、`level`（`error`、`warning`）、`message`（照这个连接的语言，带改法）；有行列的带 `line`、`column`（人格的只有 `line`），配置的另带 `key`、`got`、`suggest`、`using`，和 `config.get` 的问题一样。

1. 不写 `file`：系统配置、管理员的个人设置、`cwd` 的项目配置（没写 `cwd` 的不查）照磁盘上现在的字查（还没有的跳过，读不了的报 `unreadable`、`too_big`、`not_utf8`）；密钥文件照核心手里的问题（字是密钥，不另读）；三层里每个人格的 `persona.toml`、`prompts/examples.md`、每一份预设（施工 P-2 上）各层各查各的，上面一层盖住了照样报；软件包清单（施工 9-1 上）；有 `[check]` 的包自己的检查（施工 9-2，`packages.md`「怎么走」第 6 条：核心照起来时读到的清单跑包里的程序，收它一行一个的问题；跑坏了、程序没找到、印了看不懂的行各报一条警告 `check_failed`、`check_unavailable`、`check_output`）。照这个先后：配置、密钥、人格（层、路径）、预设（层、路径）、清单、包自己的检查（包的编号）。
2. 写了 `file`：照它的真实位置认是哪一种、只查那一份；某个目录下的 `.miyu/config.toml` 当项目配置查（不在 `cwd` 下面也行）；人格目录里写了文件、某一层 `presets/` 下的 `<编号>.toml`（施工 P-2 上），文件还没有的报 `unreadable`。认不出的：`unknown_file`。
3. 人格的代码：`syntax`、`unknown_table`、`not_a_table`、`unknown_key`、`not_phrases`、`unknown_language`、`empty_phrase`、`first_line`、`take_turns`、`last_line`、`empty_line`；给人看的那一句在 `core/human/<语言>.json` 的 `persona-problems/<code>`，照 `detail`（表名、键、`persona.<格>.<语言>`，读不成 TOML 的是它的原话）填。
4. 预设的代码（施工 P-2 上）：`syntax`、`unknown_table`、`not_a_table`、`unknown_key`、`not_phrases`、`unknown_language`、`empty_phrase`、`bad_persona`、`bad_unlisted`、`bad_software`、`not_bool`、`bad_tool`、`not_false`；给人看的那一句在 `preset-problems/<code>`，照 `detail`（表名、`<表>.<键>`、`preset.<格>.<语言>`）填。

**`persona.list`**（施工 P-1 上，`personas.md`「怎么走」第 7 条）

没有参数。回应 `{"personas": [...]}`，照编号排，一个人格一格：`persona` 编号，`name`、`summary` 一句字（施工 P-3 补：写成一句的就是它；以前写成语言表的、出厂的几个照这个连接的语言挑，这种语言没写的照 `en`、`zh`、`ja` 的先后，都没写的是 `null`；说明写成空的字的也是 `null`，施工 P-3 再补）。来自哪几层不给（施工 P-3 补，2026-10-08 项目主人：人看的是名字）。文件写错的只有 `persona`、`problem` 和知道第几行的 `line`：`problem` 照这个连接的语言说哪里写错了，同 `persona.*` 里 `persona_invalid` 的 `data.message`、`data.line`（施工 P-3 再补：原来是给排查看的英文原话，项目主人看不懂）。

**`preset.list`**（施工 P-2 上，`presets.md`「协议」）

没有参数。回应 `{"presets": [...]}`，照编号排，一个预设一格：`preset` 编号，`name`、`summary` 一句字（挑法同 `persona.list`）。文件写错的只有 `preset`、`problem` 和知道第几行的 `line`，写法同 `persona.list`（施工 P-3 再补）。

**`preset.get`**（施工 P-2 上，`presets.md`「协议」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `preset` | 字符串，必写 | 预设的编号 |

回应 `{"preset", "name", "summary", "unlisted", "features", "remove"}`（施工 P-3 补：只给人要看的；`default_persona` 施工 P-4 上撤了；施工 F-3 下 `software`、`tools` 换成 `features`）：`name`、`summary` 一句字（挑法同 `persona.list`）；`unlisted` 是叠好以后的 `on`、`off`（几层都没写的是 `on`）；`remove` 是删了会怎样：`restore`（有你那一层、下面还有：删了回到出厂的样子）、`delete`（只有你那一层：删了就没了）、`null`（没有你那一层，没什么可删）。
- `features` 是一个个功能（施工 F-3 下，设计 `30-插件框架.md` 第三节、第四节）：装了的照清单读的先后（包照编号，包里照写的先后），每个 `{"id", "name", "summary", "on", "installed": true, "tools"}`：`name`、`summary` 照它的清单、照这个连接的语言挑，没说明的没有 `summary`；`on` 是叠好以后开不开；`tools` 是归它的、工具目录里现在有的工具，照名字排，每件 `{"name", "label", "on"}`，`label` 是给人看的显示名（没有的是工具名），功能关着的都是 `false`，开着的照 `[tools]` 关没关。预设的 `[features]`、`[software]` 里写了、没装的接在后面，照编号排：`{"id", "name", "on", "installed": false, "tools": []}`，名字照给人看的字 `software/<编号>`，没有的是编号；写的是装了的包的编号的不另列。`id` 是 `preset.set` 写 `features.<id>` 用的，`tools[].name` 是写 `tools.<name>` 用的，不往界面上露。头照它一个功能一个开关画，展开能逐件关。
- 编号不合写法的 `bad_params`，没有的 `unknown_preset`，写错的 `preset_invalid`（`data.message` 照这个连接的语言说一句、`data.line` 第几行，施工 P-3 补）。

**`preset.set`**（施工 P-3 中，`presets.md`「改」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `preset` | 字符串，可以不写 | 预设的编号；不写的是新建，编号由核心起 `preset-<n>`（几层里都还没有的最小的 n，施工 P-3 补，2026-10-08 项目主人定：新建不填编号） |
| `changes` | 数组，必写、不能是空的 | 每一项 `{"key", "value" \| "unset": true, "expect"?}`，照 `config.set` 的 `changes`：`key` 是文件里的键（`preset.name`、`preset.summary`、`preset.unlisted`、`software.<包>`、`tools.<工具>`；写 `preset.default_persona` 的 `bad_params`，施工 P-4 上撤了），`value` 是字、开关、数，`expect` 是你那一层里这一项现在应当是什么（`{"value": …}` 或者 `{}` 没写） |

回应同 `preset.get`：改完叠好的样子（带着编号）。只写管理员家目录那一层的 `<编号>.toml`：改出厂的、系统区的就是建同名覆盖，只写改了的项。名字、说明写成一句字（施工 P-3 补，2026-10-08 项目主人：不分语言），以前写成语言表的整格换成一句。说明能写空的字（`"value": ""`）：就是没有说明，盖住出厂的那句，`unset` 才回到出厂的（施工 P-3 再补）；名字照旧不收空的。人格同样。新建的什么开关都不写，就是全开（`unlisted` 没写是开）。

1. 一项项在原来的字上改，注释、顺序、别的字节照原样；你那一层本来就是这个值的、本来就没写又要删的不动。一项都没变的不写。
2. 改完的一份照预设的规矩读一遍、连同叠好以后再查：有错整条不收、什么都不写，`preset_invalid`，`data.problem` 是头一处（写法同 `preset.get`），`data.message`、`data.line` 照连接的语言说（施工 P-3 补）。
3. `expect` 对不上：`preset_conflict`，`data.current` 是你那一层里这一项现在的样子，什么都不写。写的那一瞬间有人手改、重来三次还不行：`preset_conflict`，不带 `data.current`。
4. 编号不合写法、`changes` 是空的、同一个键写了两次、一项里 `value` 和 `unset` 不是正好一个、`unset` 不是 `true`、`value` 不是字开关数、`expect` 不是那两种、新建的一项都不写（只删）：`bad_params`。
5. 新建：挑一个没用过的编号写一份新文件，写的那一瞬间别处占了这个编号的换下一个。
6. 写盘照配置文件的规矩：顺着链接写、先写临时文件再替换。开着的会话下一个回合照新的（P-2 下）。扩展进程调回 `local_only`。

**`preset.delete`**（施工 P-3 中，`presets.md`「改」）：`{"preset"}` → `{"remains": <下面几层还有没有>}`。删掉你家目录那一层的文件：下面还有出厂、系统区的回到它们的样子（`true`，界面写「恢复出厂」），没有了的这个预设就没了（`false`）。你那一层本来就没有的 `nothing_to_delete`；编号不合写法的 `bad_params`；扩展进程调回 `local_only`。会话钉着的、默认指着的也能删：开着的会话照旧用快照里的，默认指着没有的照旧拒开会话（Y12）。

**`persona.set`**（施工 P-3 下，`personas.md`「改」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串，可以不写 | 人格的编号；不写的是新建，编号由核心起 `persona-<n>`（同 `preset.set`，施工 P-3 补） |
| `changes` | 数组，可以不写 | 改 `persona.toml`，写法同 `preset.set` 的 `changes`；键是 `persona.name`、`persona.summary`、`memory.scope` |
| `prompts` | 对象，可以不写 | 提示词名（`persona`、`examples`、`reminders`）到 `{"text": "<整份>"}`、`{"unset": true}`，示范对话另可写 `{"pairs": [{"user", "assistant"}, …]}`（施工 P-3 补）；都可带 `"expect": "<版本>" \| null`（照 `persona.read` 给的） |

`changes`、`prompts` 至少写一样。回应同 `persona.get`：改完叠好的样子（带着编号）。只写管理员家目录那一层：改出厂的、系统区的就是建同名覆盖，提示词整份换你那一层的那一份，`unset` 删掉你那一层的、回到下面的；空的字（`{"text": ""}`）就是这一段是空的（空的人设不进 system，空的角色扮演提示等于没有）。

1. 几样一起查：`persona.toml` 改完的一份、示范对话、连同叠好以后；有错什么都不写，`persona_invalid`，`data.problem` 是头一处（写法同 `persona.get`），`data.message`、`data.line` 照连接的语言说（施工 P-3 补）。和你那一层现在一样的不写，一样都没变的什么都不写。
2. `pairs`（施工 P-3 补：界面里一对一对地编，格式留在核心）：核心写成 `user:` / `assistant:` 开头、对与对之间空一行的写法；一句里的空行去掉；空的 `pairs` 等于删掉。每一句去掉前后空白不能是空的（`bad_params`）；一句里有一行看起来像 `user:`、`assistant:` 开头、写了读不回原样的，`persona_invalid`（`data.message` 说是第几对）。
3. `changes` 的 `expect` 对不上、提示词的 `expect` 和你那一层这一份现在的版本对不上（`null` 是「还没有」）：`persona_conflict`，`data.current` 是 `{"value": …}`/`{}`（`changes` 的）或者现在的版本、`null`（提示词的），什么都不写。几份一份一份地写，只试一次：写到一半撞上有人手改的，前面写了的不撤，`persona_conflict`、不带 `data.current`，头重读再来。
4. 编号不合写法、两样都没写、提示词名不认识、一份里 `text`、`unset`、`pairs` 不是正好一个、`pairs` 写在示范对话以外、`unset` 不是 `true`、`expect` 不是字也不是 `null`、新建的一样都不写（只删），`changes` 同 `preset.set` 的那几种：`bad_params`。扩展进程调回 `local_only`。
5. 新建：挑一个没用过的编号、先建它的目录占住（别处同时建了同一个的换下一个），再照常写；写不成的把空目录删掉。

**`persona.read`**（施工 P-3 下）：`{"persona", "prompt": "persona" | "examples" | "reminders"}` → `{"text", "version"}`。`text` 是叠好的那一份的原文，`version` 是你家目录那一层这一份的版本（`sha256:…`，照 `config.get` 的写法）；没有的都是 `null`。示范对话另带 `pairs: [{"user", "assistant"}, …]`（读好的一对一对，施工 P-3 补）。来自哪一层不给（施工 P-3 补）。编辑器照它在原文上改，存的时候把 `version` 当 `expect` 带回去。编号、名字不对的 `bad_params`，没有这个人格的 `unknown_persona`，写错的 `persona_invalid`（带 `data.message`、`data.line`）。

**`persona.delete`**（施工 P-3 下）：`{"persona"}` → `{"remains"}`，同 `preset.delete`；你家目录里这个人格的整个目录挪进回收处 `home/<账号>/trash/personas/<编号>.<删的时刻，毫秒>/`，留 7 天（同会话）。

**`package.list`**（施工 9-1 上，`packages.md`「协议」）

不带参数。回应 `{"packages": [...]}`：核心手里这时的两层清单（出厂的、管理员家目录里的），照编号排。每一项的格子见 `packages.md` 的表：读成了的有 `kind`、`protocol`、`name`、`state`，写了的有 `version`、`summary`、`command`、`opens`、`pages_dir`、`process`、`check`；施工 F-1 起，必需的有 `required`，内置包、扩展包有 `features`（没写的照包算一个），写了的有 `connection`、`depends`、`recommends`、`worker`，`kind` 多 `builtin`、`worker`；写错的、撞了的、读不了的只有 `package`、`layer`、`code`、`problem`（照连接的语言）和有的话 `line`；协议版本对不上的照样带全，多 `code: "protocol_mismatch"` 和 `problem`。名字、说明照连接的语言挑。经 `package.install`、`package.remove` 装卸的当场换（施工 F-5）；手改了磁盘上的清单的要重启核心才认。

**`package.install`、`package.remove`**（施工 F-5 上，`packages.md`「装卸」）

- `package.install {"path"}`：`path` 是本机一份清单的绝对路径，文件名 `<编号>.toml`；旁边同名的目录一起拷。装进管理员家目录那一层，同一个编号已经有的换成新的。回应同 `package.list` 的一项。
- `package.install {"package"}`：把卸掉的出厂的包装回来。回应同 `package.list` 的一项。
- `package.remove {"package"}`：家目录那一层的删掉；出厂的在家目录记一笔。回应 `{"package", "removed": true}`。
- 拒绝：参数不对、两个都写、路径不是绝对的 `.toml` 的 `bad_params`；读不了的 `path_unreadable`；写错的、拷进去以后和别的包撞了的 `package_invalid`（`data.problem` 照连接的语言说一句，知道第几行的带 `data.line`，什么都不留）；和出厂的同编号的 `package_exists`；卸必需的 `package_required`；没装的、没卸过的 `unknown_package`。扩展自己调回 `local_only`。

**`extension.status`、`extension.enable`、`extension.disable`、`extension.restart`**（施工 9-4 上，`extensions.md`「对外的样子」）

`status` 不带参数，回应 `{"extensions": [...]}`，照编号排，只列读成了的 `process` 包；`disable`、`restart` 带 `{"package"}`，`enable` 带 `{"package", "approve"?}`（批的能力名，施工 9-4 下上），回应是那一个。一个的格子见 `extensions.md` 的表：`package`、`name`（照连接的语言挑）、`start`、`on`、`state`（`off`、`starting`、`running`、`waiting`、`stopped`）、`failures`，在跑的带 `pid`，在等的带 `retry_in`，停下的带 `reason` 和标准错误的最后几行 `stderr`；声明了能力的带 `capabilities`（`[{id, name, summary}]`），还有没批的带 `unapproved`（施工 9-4 下上）。`enable`、`disable` 先写开关再动进程；`restart` 关着的回 `extension_off`；要的能力还有没批的，`enable` 没盖住、`restart` 回 `needs_approval`（`data.capabilities`）。没有这个包、清单读不成的 `unknown_package`，界面包 `not_an_extension`。扩展自己调回 `local_only`。

**`persona.get`**（施工 P-1 上，`personas.md`「怎么走」第 8 条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串，必写 | 人格的编号 |

回应 `{"persona", "name", "summary", "prompts": {"persona", "reminders"}, "examples", "remove"}`（施工 P-3 补：只给人要看的）：`name`、`summary` 一句字（挑法同 `persona.list`）；`prompts` 里是人设、角色扮演提示有没有字（`true`、`false`）；`examples` 是示范对话几轮；`remove` 是删了会怎样（同 `preset.get`）。原文照 `persona.read` 给。编号不合写法的 `bad_params`，没有的 `unknown_persona`，写错的 `persona_invalid`（带 `data.message`、`data.line`）。

**`memory.*`**（施工 R-3 补，`memory.md`「协议」）

五个方法都收 `persona`（字符串）或 `session`（会话编号）指哪一间，最多写一个，都不写照默认人格（同 `session.create`），没设默认人格的 `memory_unavailable`（不带人格记忆不生效）；人格编号的写法、找不到的照 `session.create` 第 6 条，会话找不到的 `session_not_found`，会话那一间没有（范围 `off`、不带人格、场所会话）的 `memory_unavailable`。写了 `as` 的 `bad_params`。听众是这个连接的人（本机的是管理员）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.list` | `class`（字符串）、`from`（会话编号：出处在它里面的）、`forgotten`（布尔，不写是 `false`）、`limit`（1 到 500，不写 50），都可以不写 | `{"memories": [...]}`，新的在前 |
| `memory.search` | `query`（字符串，必写、不能是空白）、`forgotten`、`limit`（1 到 500，不写 10） | `{"memories": [...]}`，最相关的在前 |
| `memory.remember` | `class`（`user`、`feedback`、`episode`、`reference`，必写）、`text`（必写） | `{"id": "m<序号>"}` |
| `memory.update` | `id`、`text`，必写 | `{"id"}`：新记的那一条 |
| `memory.forget` | `id`（可以带 `why`，不写是空的）；或者 `clear`：`session`（这时 `session` 必写）、`me` | 作废的 `{}`，清空的 `{"cleared": <几条>}` |

1. 一条：`{"at", "by", "class", "id", "retired", "sources", "text"}`（照名字排）：`by` 是 `person` 或 `tool`，`sources` 是 `[{"session", "turn"}]`，`retired` 是作废的原因、没作废的 `null`，`at` 是记下的时刻。
2. `text` 去掉前后空白再记；空的 `bad_params`，超过 120 字（数 Unicode 字符）的 `memory_too_long`，`data.chars`、`data.limit`。`class` 不是那四种、`id` 不合 `m<序号>` 的写法、`limit` 出了范围、`id` 和 `clear` 都写或都不写、`clear` 不是那两种：`bad_params`。
3. `id` 没有这一条、听众不合：`unknown_memory`；已经改掉、作废、清掉了：`memory_not_current`。
4. 记、改、作废、清空落了盘才回应；写不进记忆日志的 `internal_error`，原因记一行 `WARN memory failed`。
5. 记、改、作废、清空，同一个 `id` 再发只算一次，核心重启以后也是：回应和头一次一样（编号记在记忆事件的 `cause` 里，`memory.md`）。先认编号，再查参数。清空的 `cleared` 不算改掉的旧版本。

**`session.recap`**（施工 3-8 四补，`04-核心协议.md` 第九节，`kernel/session.md`「回顾」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |

回应 `{"cached": <布尔>, "text": "<那一句>", "upto": <序号>}`：

| 格 | 是什么 |
|---|---|
| `text` | 那一句：她写的，去掉了前后空白 |
| `upto` | 照到第几条：喂进回顾请求的最新那一条消息的序号 |
| `cached` | 是交回的上一句：上一次回顾以后没有新内容，没有请求 |

1. 回顾照 codex 的做法单独发一次辅助请求，只喂最近几轮的对话正文，不接主对话的前缀、不带工具面，用会话自己的模型（`kernel/request.md`「回顾的请求」）。什么时候要、要来了怎么显示是头的事：头可以做成 `/recap` 命令，也可以离开几分钟以后自动要（2026-10-01 项目主人定）。
2. 有回合在进行时照收：照这一刻落了盘的有效历史。
3. 写成了的，推送里先有这两条，再是回应（先见结果，后见回应）：`model.called`（`purpose` 是 `recap`）、`session.recapped`（`text`、`upto`），都不带 `turn`，`cause` 是这一条的 `id`（在路上又来的几条并进去，照第一条的）。订阅着这个会话的头都收到，别的头看得到别人要的回顾。`session.recapped` 不进她的上下文。
4. 上一次回顾以后没有新内容（有效历史里最近一条 `session.recapped` 的 `upto` 就是这一次照到的）：交回它，`cached` 是真，不请求，什么都不推。改标题、工具结果这些不算新内容：照到的是最新那一条消息。
5. 一次只有一个在路上：路上又来的并进去，一起回，只请求一次。
6. 她一个带正文的回复都还没有（没什么可回顾的）：`nothing_to_recap`，什么都不写。请求出了错、回复里没有正文：`recap_failed`，推送里有那一条出错的 `model.called`，不再来：头要再要一次就是。正在改回文件：`restoring`。先找会话，找不到的回的是找不到。
7. 回顾不记编号：同一个 `id` 再来就是再要一次，没有新内容的照样交回上一句（`kernel/session.md`「命令和回应」第 4 条）。

**`job.stop`**（施工 7-4，`agents.md` 第五条第 5 条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话派出去的 |
| `job` | 字符串，必写 | 任务编号，`j1` 这样 |

回应：空对象 `{}`，这个任务的回报落了盘才回（先见结果，后见回应）。

1. 后台命令：整组杀掉，记 `job.reported`（`stopped`，`by` 是管理员，`cause` 是这一条的 `id`）；子代理：停掉它这一轮连它派的，父会话记 `child.reported`（`stopped`，`by` 是子会话）。两种都不带 `by_model`：人停的叫醒她（`agents.md` 第三条第 4 条）。
2. 没有这个任务、已经结束了（回报到了，子代理报过 `done` 也算；正好自己退出了的只认先到的）：`unknown_job`，什么都没写。先找会话，找不到的回的是找不到。
3. `job` 不合任务编号的写法（`j` 加一段或几段不带前导零的正整数，段之间用 `.`，`kernel/ids.md`）、不是字符串：`bad_params`。

**`job.output`**（施工 7-4 补，`tools/jobs.md` 第 3 条，`session/tools.md` 第 6 条第 3 款）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话派出去的 |
| `job` | 字符串，必写 | 任务编号，`j1` 这样 |
| `tail` | 整数，不写是 200 | 要最后几行：1 到 2000 |

回应：`lines` 一共几行、`output` 交的字、`running` 还在不在跑、`truncated` 前面还有没有没交的。查询，不改会话：什么都不写，不推送。例子（格照名字的字母先后排）：

```json
{"id":"o1","jsonrpc":"2.0","result":{"lines":2,"output":"building\nhalf\n","running":true,"truncated":false}}
```

1. 读的和她用 `jobs` 读的是同一份，执行器同一个函数交出来（`Handle::job_output`，`session/tools.md` 第 6 条第 3 款）：这条命令到这时为止的输出，标准输出、标准错误照写进去的先后合在一起；结束了、输出存成了 blob 的读 blob，别的读会话目录下的 `jobs/<编号>.out`（跑着的读到这时的，载入时补 `aborted` 的读到崩的那一刻的）；开不了的当是空的。`running` 是它的回报（`job.reported`）还没落盘。
2. `output` 只交最后 `tail` 行，照原样接起来：每一行带着它的换行，最后一段没有换行的照样没有；解不开的字节换成 `�`，和 `jobs` 一样。`lines` 照 `jobs` 的数法数一共几行：照换行切，最后一段没有换行的也算一行，只有 `\r` 的不切。一行都没有的，`output` 是空字符串、`lines` 是 0。
3. 一次最多交 128 KiB（131,072 字节，照 UTF-8 算）：一行最长 1 MiB（「一行一条」），字写进 JSON 最坏一个字节变六个（控制字符写成 `\u001b` 这样），回应的其余部分不到 1 KiB，6 × 128 KiB + 1 KiB 放得进一行。最后 `tail` 行超了的，从前面按整行去掉；最后一行自己就超了的，只交它的末尾：不过 128 KiB，从一个字的开头起，前面的行都不交（接着别的行，看着就像它从那里开头）。
4. `truncated` 是前面还有没交的：去掉了前面的行，或者只交了最后一行的末尾。
5. 在阻塞线程里读，整份边读边数，不整份读进内存：一行再长，留在内存里的也只有它的末尾。每读一次都从头数：头照 1 秒读一次（网页演示这样接），推增量随 M8（「还没有的」）。
6. `tail` 先查：写了 0、负数、超过 2000、不是整数的（小数、字符串、`null`……）：`bad_params`，不找会话。`job` 照 `job.stop` 第 3 条，不合写法的 `bad_params`。先找会话，找不到的回的是找不到；没在跑的照「会话表」载入。
7. 这个会话没派过这个任务（不认识的种类也算）：`unknown_job`，和 `job.stop` 同一个原因码。是子代理的：`not_a_command`，它说了什么，头订阅它的子会话看（`agents.md`）。
8. 谁能读照 `job.stop`：现在连上来的只有管理员（「还没有的」）。

**`human.get`**（施工 W-1，`web-module.md`「二、给人看的字」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `language` | 字符串，可以不写 | 2 到 8 个小写字母，例如 `zh`。不写照这个连接的语言（握手回应的 `language`） |

回应：`{"language": <语言>, "said": {<说法的编号>: <模板>}, "tools": {<工具名>: <样子>}}`。查询，不改会话：不推送。例子（格照名字的字母先后排）：

```json
{"id":"h1","jsonrpc":"2.0","result":{"language":"zh","said":{"core/tool-results/unattended":"要确认，这里没人能确认"},"tools":{"read":{"icon":"→","name":"读取","subject":"file_path"}}}}
```

1. 每次现读资源目录，照 `store/resources.md`「怎么走」第 3 条的读法：先读内核的 `core/human/<语言>.json`，再照名字的先后读 `software/` 下每个软件包的 `human/<语言>.json`，哪一份没有这种语言照英文。开发时改了资源，下一次调就是新的，不用重启核心。在阻塞线程里读。
2. `tools` 合成一张，软件包盖掉内核的同名工具；`said` 的编号前面加上它在资源目录里的位置（`core/…`、`software/<包>/…`），模板原样给、一个字不换：换字段是头的事，照 `store/resources.md`「怎么走」第 4 条，控制字符换成 `�`。
3. 不给 `config` 那一格：配置的名字、说明在 `config.schema` 里。
4. `language` 不合写法：`bad_params`。读得到却读不懂：`internal_error`，记一行 `WARN human not read error=…`，写明是哪一份。
5. 回应的 `language` 是要的那一种；哪一份退回了英文，回应里不分，和 `miyu ask` 读到的一样。

**`mermaid.render`**（施工 W-4，`mermaid.md`）

| 参数 | 类型 | 说明 |
|---|---|---|
| `source` | 字符串，必写 | mermaid 源码 |

回应 `{"marks": {"label": <色>, "line": <色>, "text": <色>}, "svg": <SVG 的字>}`：SVG 里字、线、连线标签垫底用的三种记号色，头照它们换成自己的颜色。详细的怎么走、出错、缓存、懒初始化都在 `mermaid.md`，这一页只列进方法表、出错表（照 `config.md` 的先例）。

1. 编进了 `mermaid` 包才有（核心起来时经 `crates/miyu-core/src/packages.rs` 往查询表 `crates/miyu-endpoint/src/queries.rs` 里登记）；没编进来的回 `unknown_method`，和没有这个方法一样。
2. 查询，不改会话：不推送。

**`link.preview`**（施工 W-7，`net.md`）

| 参数 | 类型 | 说明 |
|---|---|---|
| `url` | 字符串，必写 | 网址 |

回应二选一：`{"card": {"author", "description", "duration", "icon", "image", "kind", "site", "title", "url"}}`，`icon`、`image` 是 `{"blob": <内容哈希>, "media_type": <类型>}` 或者 `null`，`kind` 是 `video`、`article`、`page` 之一，`duration`（秒）、`author` 没有的不写（W-7 再补）；`{"card": null, "why": <原因>}`，`why` 是 `not_a_url`、`unsupported_scheme`、`no_preview`、`unreachable` 之一。做不出卡片不是拒绝。地址闸、代理、记多久都在 `net.md`，这一页只列进方法表、出错表（照 `mermaid.render` 的先例）。

1. 编进了 `net` 包才有，登记的路子同 `mermaid.render`；没编进来的回 `unknown_method`。
2. 在后台答：这个连接上后面的请求不等它，回应照 `id` 对上（「一个连接」第 1 条的例外）。不碰会话，不推送。
3. 卡片的图是管理员的 blob，头照 `blob.get` 读。

**`model.call`**（施工 8-20，`models.md`「协议」、「怎么走」第十二条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `model` | 字符串，可以不写 | 模型 `<供应商>/<模型>` 或池 `@<池>`；不写、写 `null` 的照这一刻的 `models.chat` |
| `purpose` | 字符串，必写 | 用途：1 到 32 个字符，只有小写字母、数字、`-` |
| `messages` | 数组，必写 | `{"role": "system" 或 "user" 或 "assistant", "text": <字>, "images": [<blob 的哈希>, …]}` |
| `max_tokens` | 正整数，可以不写 | 最多输出多少 token |

回应 `{"text": <正文>, "provider": <供应商编号>, "model": <模型名>, "usage": <四项> 或 null}`。消息的样子、出错、用量的形状都在 `models.md`「协议」`model.call`，这一页只列进方法表、出错表（照 `mermaid.render` 的先例）。

1. 命令：经一次性入口发一次（`miyu_session::OneShot`），和会话的路由共用冷却表、池的指针。连上来的头都能调；有了扩展以后前面加一道能力的检查。
2. 不进任何会话的日志，不推送；在后台答（施工 8-20 补）：同一个连接后面的请求不等它说完，回应照 `id` 对上；连接断了，没答完的停下（下面「一个连接」第 1 条）。

**`usage.query`**（施工 8-15，`models.md`「协议」`usage.query`、「怎么走」第九条）

| 参数 | 类型 | 说明 |
|---|---|---|
| `from`、`until` | 时刻，可以不写 | 只算这一段，左闭右开 |
| `group` | 字符串的数组，可以不写 | `account`、`venue`、`model`、`day`、`session`、`purpose` 里的几样；不写是一行总计 |
| `session` | 字符串，可以不写 | 只算这个会话 |
| `tree` | 布尔，不写是 `false` | 写了 `session` 的，连它派的子会话一起算 |
| `offset` | 字符串，可以不写 | 分天照哪个时区，`+09:00` 这样写，`±14:00` 以内；不写是核心所在机器此刻的 |

回应 `{"rows": [...]}`，一组一行，照分组的几格排（`null` 在前）：分组的几格（没有的是 `null`）、`requests`、`usage` 四项、`amounts`（`[{"currency", "amount"}]`，`usage.currency` 排最前）、`unpriced`。行的写法、例子都在 `models.md`「协议」`usage.query`，这一页只列进方法表、出错表。

1. 参数不对的 `bad_params`；查不到的会话回空的 `rows`。
2. 先补再查：管理员的账号日志、会话、回收处里的会话（`models.md` 第九条第 4 条）；读汇总出错的删掉重建再补一次，还不行回 `internal`。M8 只有管理员，谁能查别人的随多用户。

**`session.set_workspace`**（施工 9-7 上，`kernel/session.md`「换工作区」；2026-10-07 项目主人定方向，形状 2026-10-08 和终端界面、网页的会话对过）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `cwd` | 字符串，可以不写 | 新的工作目录：绝对路径或者 `~` 开头的。不写的照旧（只换加进来的目录） |
| `dirs` | 字符串的数组，可以不写 | 新的加进来的目录，整份换掉；空的是去掉全部；不写的照旧 |

回应 `{"cwd", "dirs"}`：实际用的工作目录、现在加进来的目录。

1. 工作区是会话的属性：新会话开在哪就记哪（`session.create` 的 `cwd`、`dirs`），之后哪个头打开都照它，只有人明确换（这个方法、`/workspace`）才变。说话（`session.send`）带的目录照收不理。
2. `cwd`、`dirs` 都没写、写了别的格：`bad_params`。工作目录换不成真实位置、读不了：`path_unreadable`；是文件：`not_a_directory`；落在数据根里、又不是账号自己的工作区：`path_forbidden`；太宽的（系统的家目录、根目录、包含数据根的）不拒，照人选的用，回应多 `wide: true`（施工 9-7 补，2026-10-09 项目主人定；原来退回账号的工作区）；核心读不出家目录时的 `~` 照不了，`path_unreadable`。加进来的目录照造会话的规矩查（`dir_too_wide`）。拒了的什么都不记。
3. 和现在一样的：接受，什么都不记。不一样的：会话记一条 `session.workspace_changed`（`kernel/events-bodies.md`），订阅着的头照推送跟着换；会话列表那一项的 `cwd` 跟着变（`sessions.changed`）。这一轮里照旧，下一轮开始照新的。
4. `subscribe`（events）的回应带 `workspace: {"cwd", "dirs"}`，接进来就知道她在哪干活。

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
6. **内核自己起的标题**（施工 3-8 五补，`kernel/session.md`「起标题」）：没起名的会话一轮答完，核心单独发一次起标题的请求，起好了也记成 `session.meta_changed`（只写 `title`），推给订阅着这个会话的头，前面紧跟着那一次请求的 `model.called`（`purpose` 是 `title`）；两条都是 `by` 内核，没有 `cause`、不带 `turn`。和人改的一样出现在 `session.list` 里。人起过名、去掉过的会话不起，子会话、一次性的会话不起。头分得出是谁起的：看 `by`。协议的形状没变，只是 `session.meta_changed` 多了由内核起的这一种。

**`session.configure`**（施工 8-10，`models.md`「协议」「怎么走」第六条，`kernel/session.md`「换模型」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `model` | 字符串，必写 | 换成的模型 `<供应商>/<模型>` 或 `@池`（`models.md`「两种写法」） |

施工 8-18 一度在这里多收过一格 `effort`（会话给一个模型记的思考强度，`model` 那时变成可以不写）；8-18（补）去掉了这一层，思考强度改在配置里（`models.md`「怎么走」第十一条）。`effort` 写了（不是 `null`）的回 `bad_params`：这是「同一个主版本只做加法，双方都忽略不认识的字段」（`04-核心协议.md` 第八节）的一处例外——这个字段以前收过，照样收但当场拒，免得旧头以为还能这样换、静悄悄没生效。

回应：`{}`。换了没有，看推送里的 `session.policy_changed`。

1. `model` 没写、不是字符串、是空字（`""`）的、`effort` 写了的：`bad_params`，不找会话；`session` 不合写法的也是。
2. 先找会话，找不到的回的是找不到；没在跑的照「会话表」载入。再照这时不算项目配置的最终值解析（和 `session.create` 的 `model` 一样）：解析不出的 `unknown_model`，什么都不记，原话记一行 `DEBUG unknown model`。
3. 和会话现在记着的一样的：什么都不记，照样回 `{}`。别的记一条 `session.policy_changed`，只写 `model`，`by` 取自连接，`cause` 是这一条的 `id`，回合进行中换的带上这个回合；落了盘才回应。
4. 下一个回合开始时生效，一轮里前后一致；撤掉的回合里换的也算。生效的那一刻，订阅着的头收到 `model.changed`（`why` 是 `turn`），之后的 `subscribe` 带新的 `model`、限额。
5. 以后别的临时开关（`04-核心协议.md` 第九节）加进来也是这个方法，参数至少写一个。

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

**`view.page`**（施工 9-6 下，2026-10-08 形状和终端界面、网页的会话对过）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 会话编号 |
| `before` | 正整数，可以不写 | 只要序号小于它的；不写的是最新一页。往前翻拿上一页的 `first` |
| `turns` | 整数 1 到 50，可以不写 | 最多几轮，不写是 20 |

回应 `{"events", "more"}`，有事件的再带 `first`、`last`，因为字节少给了轮数的带 `capped: true`，这一页里报完了、在更早派出去的任务带 `jobs`。`events` 是原始事件，写法同补发（`event` 推送里的那一个），照序号；核心不做视图投影。

1. 页的边界落在回合之间：从 `before` 往前数 `turn.started`，每一轮从它的 `turn.started` 起，到下一轮的 `turn.started` 之前；不在回合里的事件（改标题、回顾、斜杠命令、撤销）跟着它们所在的位置走。数到第一轮的，连它前面的（`session.created` 这些）一起给，`more` 是假。
2. 数够 `turns` 轮，或者再加一轮就超过 1 MiB（照事件写成一行的字节数），停；至少给一整轮，一轮自己超了也整轮给。因为字节停的带 `capped: true`。
3. 这一页里每一轮的触发消息（`turn.started` 的 `trigger`）在切点前的，也带上，排在最前面：可能和更早的一页重复，头照序号去重。`first` 照切点算，不算带进来的这几条。
4. 这一页里报完了的任务（`job.reported`、`child.reported`，带进来的触发消息也算：后台命令跑完常引起下一轮），派它的 `job.started` 在切点前的，带在 `jobs` 里：照派出的先后，每一个照 `job.started` 的写法（`job`、`what`、`title`，子代理带 `session`），没有的不写这一格。头照它写「后台命令跑完了」那一行，不用为找标题往前翻（施工 9-6 再补）。
5. `last` 是这一页最后一条：最新一页的 `last` 就是读的那一刻落了盘的最后一条，头接着 `subscribe {"after": last}`，只接新的。压缩、撤销照原样在页里，头照有效历史自己画：先拿到的总是更新的页，撤销总比被撤的那几轮先到。
6. 只读地读会话目录里的日志，不为翻历史载入会话；读的时候会话照常跑，正在写的那半行不算。
7. 会话编号不合写法、`before` 是 0 或不是正整数、`turns` 不在 1 到 50、写了别的格：`bad_params`。没有这个会话（删了的也是）：`session_not_found`。日志读不了：`session_broken`，记一行 `WARN log not read`。


**`view.detail`**（施工 9-6 三补，2026-10-09 形状和终端界面、网页的会话对过；`04-核心协议.md` 第九节「条目只带摘要，完整的 diff 头展开时用 `view.detail` 按需取」）

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 会话编号 |
| `call` | 字符串，必写 | 那一次工具调用的编号（`tool.result` 的 `call_id`） |

回应 `{"files": [...]}`，照那次调用的工具结果里效果的先后，一个文件一项：

| 格 | 说明 |
|---|---|
| `path` | 换成真实位置以后的绝对路径 |
| `action` | `write`（`file.changed`：新建、覆盖、编辑）或 `trash`（`file.trashed`，没有差异） |
| `diff` | 整份差异，几行字：和撤销回应的同一套（`protocol/undo.md` 第 8 条第 5 款）——统一格式、上下文 3 行，每段 `@@ -旧起始,行数 +新起始,行数 @@` 起头，不带 `---`、`+++`，文件末尾没有换行的不写那一句；不截断。两边一样的是空的 |
| `added`、`removed` | 新增、删掉了几行，照整份差异数；有 `diff` 的才有 |
| `skipped` | 算不出差异的原因，有它的没有 `diff`、`added`、`removed`：`too_big`（一边超过 1 MiB）、`binary`（不是 UTF-8）、`missing`（改前改后的 blob 取不出来） |

```json
{"id":"d1","jsonrpc":"2.0","result":{"files":[{"action":"write","added":1,"diff":["@@ -1,3 +1,3 @@"," a","-b","+B"," c"],"path":"/home/me/proj/a.txt","removed":1}]}}
```

1. 只读地读会话的日志（和 `view.page` 一样，不为它载入会话），找这次调用的工具结果；改前、改后都从属主的 blob 取，不读磁盘：之后又被改过的照样是那一次的。改前是 `null`（新建的文件）的当空的算：`diff` 只有 `+` 行，头一行 `@@ -0,0 +1,N @@`。
2. 读文件、派任务这些效果不算；这次调用没改文件的，`files` 是空的。同一次调用的结果不会变，头照 `call` 记着、问一次就够。
3. 会话编号、调用编号写错、写了别的格：`bad_params`。没有这个会话：`session_not_found`。日志里没有这次调用的结果（编号对不上、还没回）：`unknown_call`。日志读不了：`session_broken`，记一行 `WARN log not read`。
4. 以后做视图投影，`view.detail` 还会交长输出这些，字段只加不改。

**`subscribe`、`unsubscribe`**

| 参数 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串 | 哪个会话：`events` 必写，`config`、`sessions` 不写（写了 `bad_params`） |
| `stream` | 字符串，必写 | `events` 会话的事件流；`config` 配置的推送（施工 8-4，`config.md`「订阅配置的推送」）；`sessions` 会话列表的推送（施工 9-5，下面「会话列表的推送」）；`extensions` 扩展的状态的推送（施工 9-4 补，`extensions.md`「推送」）。别的 `bad_params` |
| `after` | 非负整数，可以不写 | 只有 `subscribe` 的 `events` 认（施工 3-8 六补，`config`、`sessions`、`extensions` 写了 `bad_params`）：先补发日志里序号大于它、落了盘的事件，`0` 是从头。见下面「补发」 |

回应：`config` 的都是 `{}`；`sessions` 的 `subscribe` 是 `{"sessions": [<一项>, …]}`，`unsubscribe` 是 `{}`（施工 9-5）；`extensions` 的 `subscribe` 是 `{"extensions": [<一个>, …]}`，`unsubscribe` 是 `{}`（施工 9-4 补）。`subscribe` 的是 `{"limits": <限额>, "model": <模型>}`，写了 `after` 的多一格 `upto`（补到哪一条）：`{"limits": <限额>, "model": <模型>, "upto": <序号>}`。当前的待办不空的多一格 `todos`（施工 D-3，照 `todo.written` 的写法）；会话用哪个人格写在 `persona`（施工 P-1 下）、哪个预设写在 `preset`（施工 P-2 上），都照日志第一条 `session.created` 读，以前的日志没有的不写；之后变了照推送的瞬时事件 `todos.changed`，头只认这两样，不自己翻效果。施工 9-6 上起再多三格「当前的」，和订阅在会话 actor 的同一步里拿，头之后照推过来的事件往上加、不重不漏：`usage` 这个会话（不带子会话）累计的，写法、口径同 `usage.query {"session": <它>}` 那一行（`requests`、`usage`、`amounts`、`unpriced`），另加 `main`（只算主请求的四项用量：`purpose` 是空的，压缩的摘要请求也算，回顾、起标题这些辅助请求不算；头照它算命中率、上下文，施工 9-6 上补）、`compactions`（压缩的检查点有几个）、`cache_breaks`（意外断了缓存的主请求有几次：带 `first_difference`、`purpose` 是空的；压缩的摘要请求（带 `compaction`，或者看到的比之前的主请求少）不算；压缩、撤销以后的头一个主请求本来就会断，也不算；口径同终端，施工 9-6 再补）；`permission` 人这一刻设的权限 `{"level", "read_only"}`；`jobs` 还在跑的后台命令和子代理，照编号，每一个照 `job.started` 的写法（`job`、`what`、`title`，子代理带 `session`）。施工 9-7 上起再多 `workspace`：`{"cwd", "dirs"}`，会话在哪个目录干活，之后照推过来的 `session.workspace_changed` 换。已经订阅着、再订阅一次不带 `after` 的（「还是那一个」），这几格另要一份这一刻的。`unsubscribe` 的是空对象 `{}`。

**模型** `model`（施工 8-10，`models.md`「协议」）：会话接下来请求的。`ref` 是会话的引用（模型或 `@池`），`endpoint`、`model` 是接下来发给哪一家的哪个模型；轮换的池（每次都换）、解析不出的没有 `endpoint`、`model`，没配 `models.chat` 的会话没有 `ref`。一个都没有的不写这一格。回合开始重新解析过的、出错换了成员的是换了以后的。施工 8-18 多一格 `effort`：`{"level": <一档>, "from": "system" 或 "personal"}`，接下来那个模型真用的思考强度和从配置的哪一层来（8-18（补）起不再有 `session`）；请求里什么都不带的、轮换的池不写。

**限额** `limits`：会话实际用的模型给这个会话多少地方（施工 6-3 补）。两格都是 token 数，没有的不写，从不写 `null`；两格都没有就是 `{}`。

| 格 | 类型 | 说明 |
|---|---|---|
| `window` | 非负整数，可以没有 | 上下文窗口。模型的资料没报的没有（`compaction.md`「对外的样子」模型的资料） |
| `compaction_line` | 非负整数，可以没有 | 压缩线：用量过了它，发下一次请求之前自动压（`compaction.md` 第二条第 2 条）。没有窗口的、窗口太小算不出正数的没有 |

例子：会话照 `models.chat` 记下 `deepseek/deepseek-v4`，核心照 DeepSeek 的资料，窗口 1000000、最大输出 393216，压缩线 = 1000000 − min(393216, 20000) − 13000；没设默认人格，会话无人格，没有 `persona` 这一格（施工 P-4 上）（键照字母先后排）：

```json
{"id":"c2","jsonrpc":"2.0","result":{"jobs":[],"limits":{"compaction_line":967000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"permission":{"level":"workspace","read_only":false},"preset":"full","usage":{"amounts":[],"cache_breaks":0,"compactions":0,"main":{"cache_read":0,"cache_write":0,"output":0,"uncached":0},"requests":0,"unpriced":0,"usage":{"cache_read":0,"cache_write":0,"output":0,"uncached":0}},"workspace":{"cwd":"<工作区>","dirs":[]}}}
```

（`<工作区>` 是在 `~` 里造的会话退回的账号的工作区，照真实的那个写，施工 9-7 上。）

模型的资料没报窗口的：`{"id":"c2","jsonrpc":"2.0","result":{"limits":{},"model":{…}}}`。

1. 订阅：从这一刻起的推送都转给这个连接。不写 `after` 的，以前的不补。没在跑的会话先载入。
2. 不写 `after` 的：这个连接已经订阅着这个会话、还在推的，还是那一个，回应照样带限额；停了推的（掉了队、会话停了），换一个新的。写了 `after` 的总是换一个新的（「补发」第 6 条）。
3. 取消订阅：停掉转发。不载入会话，没订阅过的也回 `{}`。
4. 不写 `after` 的，订阅的回应不排在推送后面：会话正忙的，回应之前可能已经有这个会话的推送。写了 `after` 的排在补发的后面（「补发」第 2 条）。
5. 限额、模型是造会话、载入时定的：会话 actor 把模型的限额交给内核以后，向内核要一份（`kernel/session.md` 的 `context_limits()`），连同端口的引用和接下来发给谁，`Handle` 带着它（`session/actor.md`）。会话中途会变：钉住的池出错换了成员、成了以后（施工 8-9），回合开始重新解析换了模型、钉着的没了、配置改了窗口的（施工 8-10），`Handle` 跟着换，之后的 `subscribe` 交新的，订阅着的头收到一条 `model.changed`（`models.md`「瞬时事件」，`why` 是 `failover` 或 `turn`）。
6. 压缩线由内核算好，和它自己判到线用的是同一条；头照 `window` 画「用量 / 窗口」、照 `compaction_line` 算离压缩还有多少，不照公式自己算（公式里的输出预留、余量在策略里）。
7. 限额不进日志：头每次接进来（造完会话、中途接进一个在跑的会话、掉了队重新订阅、核心重启以后）都经 `subscribe`，从回应里拿（为什么见 `04-核心协议.md` 第九节「先做的几样怎么写」）；接着的时候变了的，照推过来的瞬时 `model.changed` 换（施工 8-9）。
8. 订阅着就算这个头在看着这个会话（施工 7-9）：一次性的会话没有头订阅着，回报只记下、不叫醒她（`agents.md` 第三条第 3 条）。取消订阅、连接断了，就不算了；会话 actor 数着拿着订阅的头（`session/actor.md` 第 3 条），不进日志，协议上不另说。头要知道还有几个子代理没报、叫醒的那一轮会不会来，照推过来的事件自己数（`job.started`、`child.reported`、`job.messaged`，`cli/ask.md`「等子代理」），协议不另给：施工 7-9 照最简单、不加协议定。

**补发**（施工 3-8 六补，`04-核心协议.md` 第六节第 9 条、第七节）

头切进一个已有的会话、掉了队、断了线重连，都带上 `after` 订阅，拿回以前的事件：头不读核心的存储（`01-架构.md` 第五节第 1 条）。

1. 写了 `after` 的：日志里序号大于 `after`、订阅那一刻落了盘的事件，照原样一条条推过来（`event`，和实时推的一样），再接着推之后的，中间不丢不重。`upto` 是订阅那一刻落了盘的最后一条：补的是 `after + 1` 到 `upto`，之后推的从 `upto + 1` 起。
   例子：日志里有 42 条，头看到第 30 条掉了线，重连以后 `{"session": …, "stream": "events", "after": 30}`：先推第 31 到 42 条，再是回应 `{"id":"c2","jsonrpc":"2.0","result":{"limits":{},"upto":42}}`，之后从第 43 条接着推。
2. 补的都在回应之前到（「先见结果，后见回应」）：回应交给新订阅的转发任务，排在补的后面。补完到回应之间，可能已经有新推的。
3. `after` 不比 `upto` 小的，什么都不补，照常推新的；`upto` 照样是日志里最后一条，比 `after` 小的，是头记的比日志还多。
4. 只补落了盘的：瞬时事件（`model.delta`、`tool.progress`、`status`、`model.changed` 这些）没有序号，不补。撤销、压缩、清空、撤回的事件照原样补，头照有效历史自己算怎么画（视图投影随 M9）。
5. 补的那一截和新的订阅在会话 actor 的同一步里拿（`session/actor.md` 第 6 条）：那一步之前落了盘的都推过了，之后的都还没推。读日志不占 actor：端点在这个连接上读完这一截（阻塞线程里一次读完，不分批：载入会话本来就整份读进内存，照最简单的做），交给转发任务先写；读的时候会话照常跑，新推的攒在这个订阅里。这个连接上的下一条请求，等读完才办（「一个连接」第 1 条）。
6. 写了 `after` 的总是换一个新的订阅。这个连接原来订阅着这个会话的，先不再交回应给它，等它把手里的推送、回应都放进写队列，补的才开始写：两段不交错。新的订阅拿到了才放下旧的，这个头一直算看着（`subscribe` 第 8 条）。旧的推过、补的又补了的，序号是重的：头照序号认，补的是回应之前、从 `after + 1` 起连着到 `upto` 的那一段。
7. 补的走同一个订阅，受同一个限：会话给每个订阅最多攒 1024 份（「慢和掉队」）。一次补得多、头读得慢、会话同时推得多的，照样掉队、推 `resync`，头带上最后看到的序号再来。
8. `after` 不是非负整数的（负数、小数（`1.0` 也算）、字符串、布尔、数组、对象、超过 2^64 − 1 的）：`bad_params`，先查参数、不找会话，什么都没订阅。写 `null` 等于没写。`unsubscribe` 不认 `after`，写了不理。
9. 读不了日志的（坏了、读的时候会话被删了）：`session_broken`，记一行运行日志 `replay not read`，什么都没订阅（原来订阅着的也停了）。没有要补的（第 3 条）不读日志。
10. 会话停了、没在跑的，照「会话表」：停了的回 `session_stopped`，下一次再载入；没在跑的（核心重启过、停下过全部会话）先载入，再照样补。

**会话列表的推送**（施工 9-5，2026-10-07 项目主人批准 M9 头几步时定、提到 O 线前面：网页的侧边栏要对每个会话订阅整份日志才画得出标题、在跑，会话一多就慢；形状先给终端界面、网页看过）

1. `subscribe` 的 `sessions`：回应 `{"sessions": […]}`，每一项和 `session.list` 的一样（`busy`、`cwd`、`last_active`、`oneshot`、`parent`、`pinned`、`session`、`title`，有的才写照那边），从新到旧，全部：一次性的、子会话都在。一个连接至多一个；再订阅换一个新的（交回一份新的列表，旧的停掉）。`unsubscribe` 停掉它，没订阅过的也回 `{}`。
2. 之后推 `sessions.changed`：`{"session": <编号>, "entry": <一项>}`，头照编号整项替换；删掉的是 `{"session": <编号>, "removed": true}`。
3. 什么时候推：造了会话（主会话、子会话、一次性的）；改名、置顶（人改的、内核起的标题都算）；一轮开始（`busy`、`cwd`、`last_active` 跟着变）；一轮结束、会话空下来（`busy` 去掉，`last_active` 跟着变）；删了。`last_active` 只在这几个时刻跟着走，不是每条事件都推。「未读」由头自己拿 `last_active` 和自己看过的比。
4. 先后：一项一项照核心里一个排队的任务算，一个接一个；订阅的回应里的列表也由它算。所以回应里的列表包含回应之前的每一次变化，之后推的每一条都比它新，旧的一项盖不住新的。推送和别的命令（例如 `session.create`）的回应之间不保证先后：推的是整项，晚到的照样对。
5. 读得太慢、掉了队：推 `resync`（`{"stream": "sessions"}`），这个订阅停了，头重新订阅。
6. 场所会话以后也不进这个流（`18-通讯平台.md` 第十一节）。
7. 怎么知道变了（`session/actor.md`「推送和订阅」第 7 条）：会话每送完一批，照这一批推过的事件（`session.created`、`session.meta_changed`、`turn.started`、`turn.ended`）和忙不忙变没变，经会话表的端口报一声「这个会话的那一项变了」（`SessionPort::listing`）；端点照会话列表的索引只算这一个会话的一项（和 `session.list` 同一个函数）。造会话、删会话由会话表自己报。不靠订阅每个会话的事件流：订阅着就算有头在看着它（`subscribe` 第 8 条）。

#### 推送

| 方法 | `params` | 什么时候 |
|---|---|---|
| `event` | `{"session": <编号>, "event": <事件>}` | 订阅着的会话的每一条事件，一条一个；补发的也是它（施工 3-8 六补） |
| `resync` | `{"session": <编号>, "stream": "events"}`；配置的是 `{"stream": "config"}`（施工 8-4），会话列表的是 `{"stream": "sessions"}`（施工 9-5） | 读得太慢，掉了队：这个订阅停了 |
| `sessions.changed` | `{"session": <编号>, "entry": <一项>}`，删了的 `{"session": <编号>, "removed": true}`（施工 9-5） | 订阅着会话列表的：上面「会话列表的推送」第 3 条那几种时刻 |
| `config.changed` | 见 `config.md`「推送 `config.changed`」（施工 8-4） | 订阅着配置的：系统配置、个人设置每变一次 |

推送的事件不都由这个连接的命令引起：内核自己起的标题（`session.set_meta` 第 6 条，施工 3-8 五补）一轮答完以后自己来，没有 `cause`。

`event` 里的事件照原样嵌进去：落了盘的照 `kernel/events.md` 的写法，样本在 `docs/designs/samples/events/`；瞬时的 `model.delta`、`tool.progress`、`status`（施工 8-9 起换了端点的多 `failover`）、`model.changed`（施工 8-9，会话接下来请求的模型、限额变了，`models.md`「瞬时事件」）这些没有 `seq`，样本在 `docs/designs/samples/transient/`。

### 怎么走

**一个连接**

1. 请求一条条办：上一条的回应放进了写队列（订阅着的会话的，交给了它的转发任务，下面「先见结果，后见回应」），才读下一行。要等的（`session.send` 等落盘，撤销等改完文件，载入会话）等着的时候，这个连接上的下一条也等着。例外是在后台答的：登记成在后台答的查询（现在只有 `link.preview`，施工 W-7），和自带的 `model.call`（要等模型说完，施工 8-20 补，`methods.rs` 的 `answered_later`）：交给这个连接的一个后台任务，接着读下一行；办完了回应照 `id` 对上，直接放进写队列，不经会话的订阅；连接断了，这些任务一起停。
2. 读和写分开：写的一头从写队列里一行行写出去，队列最多攒 256 行。写不出去就停；读的一头往写队列放回应、放不进去时发现，断开（交给转发任务的那一条放得进，要到再下一条）。
3. 断开：一行太长的，回 `parse_error`（`id` 是 `null`）；`protocol_mismatch`、`bad_token` 回完；对方关了、读出错。
4. 断开以后，这个连接的订阅都停了。会话照常跑：头走了，在跑的那一轮照样跑完。
5. 连接从接上起就算连着，握没握手都算，断开才不算；没握手的最多连 10 秒（握手第 7 条）。

**先见结果，后见回应**

1. 方法的回应，`params.session` 这个会话在这个连接上有订阅的，交给这个订阅的转发任务；`config.set` 的回应，这个连接订阅着配置的，交给配置的转发任务（施工 8-4）：它先把已经到了的推送都放进写队列，再放回应。会话先推送、后回应（`session/actor.md`），回应到的时候，这条命令产生的推送一定已经到了。
2. 别的回应直接放进写队列：没订阅的会话的；`params` 是数组的；`hello`、不写 `after` 的和被拒的 `subscribe`、`unsubscribe` 的；握手以前的拒绝；读不懂的行的。写了 `after`、订阅上了的 `subscribe`，回应交给新订阅的转发任务，排在补发的后面（「补发」第 2 条，施工 3-8 六补）。
3. 订阅停了推（掉了队、会话停了），转发任务接着替这个会话转回应，直到这个订阅被取消、被新的换掉，或者连接断了。

**慢和掉队**

1. 头读得慢：写队列满了，转发任务等着，不再从会话那里拿。会话给每个订阅最多攒 1024 份没读走的推送，再多就掉了队。配置的推送最多攒 16 条（施工 8-4）。核心和会话都不等这个头。
2. 掉了队：推一条 `resync`，这个订阅停了，之后不再推；回应照样到。头重新 `subscribe`：不写 `after` 的从那一刻起再推，掉了的不补；带上最后看到的序号（`after`）的，掉的那一截补回来（「补发」，施工 3-8 六补）。
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
5. 载入时没有报来的 `cwd`（`session.interrupt`、`session.revert`、`session.unrevert`、`session.redo`、`session.compact`、`session.set_permission_level`、`session.clear`、`session.recap`、`session.set_meta`、`subscribe` 载入的）：照日志里最后一条带 `cwd` 的 `turn.started`（加进来的目录照最后一条 `turn.started` 的 `dirs`，没有就是没有（施工 5-10 上）），没有就照 `session.created` 的，都没有（之前的日志）才当报来的是 `~`，退回管理员的工作区（施工 4-9 再补三上）。核心重启以后撤销，路径照会话真正的目录写短。
6. 会话一直留在表里，直到核心退出、停下全部会话、删了它（`session.delete`），或者用到时发现它停了。
7. 造会话、载入时，交给会话一份造子会话的端口（施工 7-5，`session/tools.md`「派子代理」）：会话里派出去的子会话由会话表造，放进表里，和头造的一样照编号找得到、只起一个；子会话也算进「有没有会话忙着」，停下全部会话时一起停。父会话已经不在表里的（删了、停了）不再造，派不了（施工 3-8 三补：不留下没有父会话的子会话）。
8. 载入一个会话以后、放进表之前，收掉它派到一半的空子会话（施工 7-8，`agents.md` 第一条第 8 条）：它的日志里有没派成的 `subagent` 调用（以前造的会话里叫 `agent`，也算；结果里没有 `job.started`，或者还没有结果）才去认，认的是放会话的目录里 `session.created` 的 `parent` 是它、它的日志里又没有这个子会话的 `job.started` 的，连同它们派的；在跑的停下（`Handle::discard`），目录挪进回收处，最深的在前，照 `session.delete` 第 7 条。一个记一行 `INFO orphan subagent removed`；挪不走的记一行 `WARN`，不耽误载入。这时表拿着锁，它不在表里，也就派不出新的，认不错。

**工作目录太宽**

`session.create`、`session.send` 报来的 `cwd`，照这几条定会话实际在哪个目录里干活：

1. 去掉前后空白是 `~` 的：太宽。
2. 换成真实的位置（`fs.md`）：`~`、`~/…` 接在系统的家目录上（家目录先换成真实的位置），相对的接在 `/` 上，链接顺着找到本体。换不成的：照原样用，说不清它宽不宽，用到时工具自己报错。
3. 真实的位置是系统的家目录、是根目录、包含数据根，或者落在数据根里却不在管理员的工作区里：太宽。
4. 落在数据根里、却不在会话属主的工作区里的，退回属主的工作区 `<数据根>/home/<账号>/workspace`；它还没有的，当场建（建不成的记一条运行日志，照样用它）。
5. 别的太宽的：人明着选的（`session.create` 带 `chosen: true`、`session.set_workspace`、`/workspace`）照用，回应多 `wide: true`；头自己带上的（终端启动时的当前目录）照第 4 条退回（施工 9-7 补，2026-10-09 项目主人：「当然是我手动选择的，或者手动运行命令切换的，就别自动切工作区了」；原来一律退回）。数据根不管工作区设在哪都挡着（`session/guard.md`），设成 `~` 也写不进数据根。
6. 不太宽的，照原样用，不换成真实的位置。
7. 会话载入时（核心重启以后）照日志里最后记下的那一个，不再判太宽：记下的就是当时挑好的；落在数据根里、不在属主工作区的照第 4 条退回；什么都没记下的照头自己带上的判。

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
| `bad_params` | -32602 | 参数读不成、类型不对；会话编号、人格编号不合写法；`turn` 写了 0；`stream` 不是 `events`、`config`，`config` 带了 `session`、`after`（施工 8-4）；切权限级别两格都不写、`level` 不是 `workspace`、`full`；`blob.put` 第 1 条那几种；`session.send`、`session.redo` 的附件缺了格、格不合写法；`session.send` 的 `from` 不是字符串、去掉控制字符以后是空的（施工 7-10）；改标题两格都不写，标题去掉空白以后是空的、超过 200 个字；`job.stop`、`job.output` 的任务编号不合写法（施工 7-4），`job.output` 的 `tail` 不是 1 到 2000 的整数（施工 7-4 补）；`human.get` 的 `language` 不合写法（施工 W-1）；`fs.realpath` 的 `path` 是相对的、没给 `cwd`（施工 W-3）；`mermaid.render` 的源码是空的（去掉前后空白以后，施工 W-4）；`blob.write` 的 `data` 不是 base64、解出来超过 512 KiB、加上它超过 `size`（施工 W-5）；`model.call` 的 `purpose` 不合写法、`messages` 不是那个样子、`max_tokens` 是 0 或者太大、`model` 是空字、图的 blob 不是图（施工 8-20）；`fs.read` 的 `path` 是相对的；`blob.get`、`fs.read` 的 `length` 超过 512 KiB（施工 W-6）；`link.preview` 的 `url` 没写、不是字符串（施工 W-7）；`usage.query` 的分组不认识、时刻和时区写法不对、会话编号不合写法、类型不对（施工 8-15）；`session.answer` 的 `decision`、`answers` 两样都写或都不写、`decision` 不是那三种、回答提问带了 `reason`、`call` 不合写法（施工 D-1）；`command.run` 的 `text` 不以 `/` 开头、本机的会话带了 `as`（施工 O-6）；`view.page` 第 7 条那几种（施工 9-6 下） |
| `internal_error` | -32603 | 造会话时装坏了、磁盘上建不成、`session.created` 没落盘；列会话时读不了放会话的目录、崩了；附件存不下来、读不出来；删会话时读不了放会话的目录、挪不进回收处、崩了；读后台命令的输出时崩了（施工 7-4 补）；给人看的字读不懂（施工 W-1）；画图的库初始化不了：`style.json` 读不懂，或者这台机器上一种字体都读不到（施工 W-4）；分块上传的暂存文件建不了、写不进（施工 W-5）；`link_preview.json` 读不懂（施工 W-7） |
| `hello_first` | -32010 | 握手以前发了别的方法 |
| `protocol_mismatch` | -32010 | 头支持的主版本里没有 1（之后断开） |
| `bad_token` | -32010 | 凭据一种都没写、本机令牌不对（之后断开） |
| `bad_code`、`bad_login`、`bad_password`、`login_throttled`、`setup_first`、`local_only` | -32010 | 网页登录的几种（施工 W-8，`web-module.md`「出错」）；扩展调 `extension.*` 也回 `local_only`（施工 9-4 上），调 `preset.set`、`preset.delete` 也是（施工 P-3 中），`persona.set`、`persona.delete` 也是（施工 P-3 下） |
| `unknown_persona` | -32010 | 造会话、`persona.get` 时三层都没有这个人格（施工 P-1 上起三层，`personas.md`） |
| `persona_invalid` | -32010 | 人格的文件写错了；`data.problem` 写明哪一层、哪个文件第几行（施工 P-1 上）；`persona.*` 里另带 `data.message`（照连接的语言）、`data.line`（施工 P-3 补） |
| `unknown_preset` | -32010 | 造会话、`preset.get` 时三层都没有这个预设，默认预设指着没有的也一样（施工 P-2 上，`presets.md`） |
| `preset_invalid` | -32010 | 预设的文件写错了；`data.problem` 写明哪一层、哪个文件第几行（施工 P-2 上）；`preset.set` 改完的一份写错（施工 P-3 中）；`preset.*` 里另带 `data.message`、`data.line`（施工 P-3 补） |
| `preset_conflict` | -32010 | `preset.set` 的 `expect` 对不上（`data.current`），写的那一瞬间有人手改、重来三次都不行（施工 P-3 中） |
| `persona_conflict` | -32010 | `persona.set` 的 `expect` 对不上（`data.current`），写到一半撞上有人手改（施工 P-3 下） |
| `nothing_to_delete` | -32010 | `preset.delete`、`persona.delete` 删的在你家目录那一层本来就没有（施工 P-3 中、下） |
| `unknown_file` | -32010 | `check` 写的文件不是 Miyu 读的那几种（施工 8-30） |
| `not_a_directory` | -32010 | `session.set_workspace` 换到的是文件（施工 9-7 上）；`/workspace` 也一样（施工 9-7 下） |
| `unknown_package`、`not_an_extension`、`extension_off` | -32010 | `extension.*`：没有这个包、清单读不成；是界面包；重启一个关着的（施工 9-4 上，`extensions.md`）。`package.remove`、`package.install {"package"}` 也回 `unknown_package`（施工 F-5 上） |
| `package_invalid`、`package_exists`、`package_required` | -32010 | `package.install`、`package.remove`：清单写错了、和别的包撞了；和出厂的同编号；卸必需的（施工 F-5 上，`packages.md`「装卸」） |
| `needs_approval` | -32010 | `extension.enable`、`extension.restart`：要的能力还有没批的，`data.capabilities` 是那几个（施工 9-4 下上，`extensions.md`「能力」） |
| `session_not_found` | -32010 | 没有这个会话，删了的也是 |
| `unknown_call` | -32010 | `view.detail` 的会话日志里没有这次调用的结果：编号对不上，或者还没回（施工 9-6 三补） |
| `no_system_account` | -32010 | 场所会话的属主该是系统账号，这个连接不是（施工 O-3；O-4 下起核心拉起的、清单声明了系统账号的包的扩展是，别的连接还回它） |
| `venue_session` | -32010 | 场所会话只收代表外部的人说的话：不带 `as` 的 `session.send`（施工 O-3）、`command.run`（施工 O-6）；`command.catalog` 只收本机的会话（施工 O-6 补） |
| `unknown_command` | -32010 | `command.run` 认不出这个命令（施工 O-6） |
| `command_not_allowed` | -32010 | `command.run`：场所里既不是主人、也不是管理的人（施工 O-6） |
| `owner_only` | -32010 | `command.run`：这个命令只有主人本人能用，管理的人也不行（`/workspace`，施工 9-7 下） |
| `session_stopped` | -32010 | 会话停了：写不进去、出了 bug |
| `session_broken` | -32010 | 会话载入不了：日志、策略快照坏了、读不了 |
| `empty_message` | -32010 | `session.send` 的 `text` 是空的、又没有附件、也没带场所的东西（施工 O-13 补）；`session.redo` 换过的那一句一块都不剩 |
| `dir_too_wide` | -32010 | 加进来的目录太宽（「加进来的目录」（施工 5-10 上）） |
| `attachment_unreadable` | -32010 | `blob.put` 读不了 `path`：换不成真实的位置、没有、不是普通文件、没有权限（施工 3-9 三补） |
| `attachment_too_big` | -32010 | 附件超过 20 MiB；图片超过 5 MiB，或者哪一边超过 8000 像素（施工 3-9 三补）；`blob.open` 的 `size` 超过 20 MiB（施工 W-5） |
| `attachment_in_data_root` | -32010 | `blob.put` 的 `path` 在数据根里、管理员的工作区以外（施工 3-9 三补） |
| `unknown_attachment` | -32010 | `session.send`、`session.redo` 附的 blob 这个核心里没有（施工 3-9 三补）；`model.call` 的图这个账号的 blob 里没有（施工 8-20） |
| `path_unreadable` | -32010 | `fs.list`、`fs.find` 换不成真实的位置、不在、该是目录的不是目录、没有权限（施工 W-2）；`fs.realpath` 换不成真实的位置——一层都不在、路上的链接指向不存在的地方、没有家目录（施工 W-3）；`fs.read` 换不成真实的位置、没有、不是普通文件、没有权限（施工 W-6）；`session.set_workspace`、`/workspace` 换不成真实的位置、不在、读不了（施工 9-7 上、下） |
| `path_forbidden` | -32010 | `fs.list`、`fs.find` 的目录落在数据根里、又不在这个账号的工作区里（施工 W-2）；`fs.read` 的路径也一样（施工 W-6）；`session.set_workspace`、`/workspace` 换去的目录也一样，账号自己的工作区可以（施工 9-7 上、下） |
| `mermaid_too_long` | -32010 | `mermaid.render` 的源码超过 64 KiB（施工 W-4） |
| `mermaid_failed` | -32010 | `mermaid.render` 画不出；`data.detail` 是画图的库的原话（施工 W-4） |
| `too_many_uploads` | -32010 | 这个连接同时开着 4 个分块上传（施工 W-5） |
| `upload_unknown` | -32010 | 没有这个上传：编号不对、作废了、不是这个连接开的（施工 W-5） |
| `upload_offset` | -32010 | `blob.write` 的 `offset` 和已经收到的对不上；`data.received` 是实际收到的几个（施工 W-5） |
| `upload_incomplete` | -32010 | `blob.close` 时没收齐；`data.received` 是实际收到的几个（施工 W-5） |
| `unknown_blob` | -32010 | `blob.get` 的 blob 这个账号没有（施工 W-6） |
| `not_running` | -32010 | 打断时没有回合在进行 |
| `turn_running` | -32010 | 撤销、重做、手动压缩、清空、删会话时有回合在进行 |
| `unknown_turn` | -32010 | 要撤的那一轮不在有效历史里：没有，或者已经撤掉了 |
| `nothing_to_unrevert` | -32010 | 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过 |
| `nothing_to_revert` | -32010 | 不写 `turn` 的撤销，一轮都没有 |
| `nothing_to_compact` | -32010 | 手动压缩时没有能压的：上一次压缩以后没有新的消息、回复、工具结果，或者全在尾巴里（施工 6-8） |
| `nothing_to_clear` | -32010 | 清空时上下文本来就是空的：没有摘要，最近的检查点后面也没有人的消息、回复、工具结果、回报（施工 6-8 补） |
| `unknown_job` | -32010 | `job.stop` 时没有这个任务，或者它已经结束了（施工 7-4）；`job.output` 时没有这个任务（施工 7-4 补） |
| `not_a_command` | -32010 | `job.output` 读的是子代理，不是后台命令（施工 7-4 补） |
| `not_redoable` | -32010 | 重做时最后一轮不是人说的话开的，或者一轮都没有（施工 4-7 再补） |
| `nothing_to_recap` | -32010 | 回顾时她一个带正文的回复都还没有；以前造的快照里没有回顾的字（施工 3-8 四补） |
| `not_asking` | -32010 | `session.answer` 回答的调用没在等回答：答过了、了结了、等的不是这一种（施工 D-1） |
| `not_a_provider` | -32010 | `provide` 不是核心拉起的扩展发的（施工 O-2 上） |
| `bad_tool` | -32010 | `provide` 的一件工具规格不对、撞名；`data.tool` 是哪一件，`data.problem` 是 `duplicate`、`name`、`parameters`、`access`、`venues`、`timeout`（施工 O-2 下）之一（施工 O-2 上） |
| `not_ambient` | -32010 | `session.respond` 的 `to` 里有不是这个会话里旁听的 `message.user` 的；`data.messages` 是那几条（施工 O-14 上） |
| `already_answered` | -32010 | `session.respond` 的 `to` 里有已经当过触发的；`data.messages` 是那几条（施工 O-14 上） |
| `no_rule` | -32010 | `session.answer` 选了本会话都允许，请求却没提放行规则（施工 D-1） |
| `unexpected_reason` | -32010 | `session.answer` 允许却带了理由（施工 D-1） |
| `bad_answer` | -32010 | `session.answer` 的回答和题目对不上：条数不对、选了题目里没有的、单选的选了几项、选重了（施工 D-1） |
| `recap_failed` | -32010 | 回顾没写成：请求出了错，或者回复里没有正文（施工 3-8 四补） |
| `unknown_config_key` | -32010 | `config.schema`、`config.get` 的 `keys`、`config.set` 的 `changes` 里有清单里没有的键：`data.problems` 里每个不认识的一条，`code` 是 `unknown_key`、`level` 是 `error`，带最近的键名 `suggest`（施工 8-2） |
| `config_invalid` | -32010 | `config.set` 的值不对、不能写在这一层，整份换的字里有错误：整条不收，`data.problems` 里是每一处（施工 8-3） |
| `config_conflict` | -32010 | `config.set` 的 `expect` 对不上（`data.current`），整份换的、`config.trust` 的 `version` 对不上，写的那一瞬间有人手改、重来三次都不行（`data.version`）（施工 8-3） |
| `config_file_broken` | -32010 | `config.set` 改几项时文件读不进来，或者这一项放不进去：`data.problems` 是这份文件现在的问题（施工 8-3）；`secret.set`、`secret.delete` 时密钥文件读不进来、名字写成了一张表（施工 8-5） |
| `no_project_config` | -32010 | `config.trust` 时这个目录找不到项目配置（施工 8-3） |
| `unknown_secret` | -32010 | `secret.delete` 删的密钥没有（施工 8-5） |
| `unknown_provider` | -32010 | `model.list`、`provider.test` 的 `provider` 不是配好了的（施工 8-7、8-11） |
| `unknown_model` | -32010 | `session.create`、`session.configure` 的 `model` 解析不出：写法不对（连同以前的挡位名）、没有这家供应商、没有这个池、池是空的（施工 8-8、8-8 补）；`model.call` 的 `model` 解析不出（施工 8-20） |
| `no_model` | -32010 | `model.call` 没有能用的模型：没写 `model`、`models.chat` 也没配，那一家用不了、key 取不到；`data.message` 是原话（施工 8-20） |
| `cooling` | -32010 | `model.call` 的候选全在冷却，没发；`data.message` 是原话，`data.wait_ms` 是最早恢复的那一个还要多久（施工 8-20） |
| `model_failed` | -32010 | `model.call` 发了、出错了：`data.class`、`data.status`（有状态码的才写）、`data.message`，和 `model.called` 的 `error` 一样（施工 8-20） |
| `memory_unavailable` | -32010 | 这里没有记忆：不带人格（没写人格、又没设默认人格的也是）、范围 `off`、场所会话（施工 R-3 补，`memory.*`、`/remember`；施工 R-3 再补那句话也说没有人格） |
| `unknown_memory` | -32010 | 没有这一条记忆，或者听众不合（施工 R-3 补） |
| `memory_not_current` | -32010 | 那一条记忆已经改掉、作废、清掉了（施工 R-3 补） |
| `memory_too_long` | -32010 | 一条记忆超过 120 字；`data.chars`、`data.limit`（施工 R-3 补） |
| `restoring` | -32010 | 撤销、恢复还没做完（正在读回更早的日志、正在改回文件）时来的命令、删会话。兜底：会话做完才接下一个命令，照常碰不到 |

- 从 `empty_message` 起，除了 `dir_too_wide`、附件的四个和 `not_a_command`，十三个是内核拒命令时给的原因码（`kernel/session.md`）；施工 D-1 起加上 `session.answer` 的四个（`not_asking`、`no_rule`、`unexpected_reason`、`bad_answer`），十七个。
- 内核还有两个原因码，现在没有方法碰得到：`unknown_level`（协议上的级别只认两种，别的先是 `bad_params`）、`unknown_decision`（协议上的决定只认三种，施工 D-1）。它们没有配话，说的是最后那一句「被拒绝了」。

运行日志（目标 `miyu::endpoint`，`log.md`）：

| 级别 | 这件事 | 什么时候 |
|---|---|---|
| `DEBUG` | `connected` | 接上一个连接 |
| `INFO` | `connected head=… version=… protocol=1 via=…` | 握手过了；`via` 是 `token`、`code`、`login`、`password`（施工 W-8） |
| `WARN` | `protocol mismatch head=… low=… high=…`、`bad token head=…`、`bad code head=…`、`bad login head=…`、`bad password head=…`、`login throttled head=…` | 握手被拒、断开 |
| `INFO` | `login revoked, closed` | 这个连接靠的登录令牌作废了，断开（施工 W-8） |
| `DEBUG` | `request method=…` | 每一条请求 |
| `WARN` | `line too long, closed` | 一行太长 |
| `INFO` | `disconnected` | 握过手的连接断了 |
| `WARN` | `accept failed error=…` | 接不了连接 |
| `WARN` | `memory failed error=…` | `memory.*`、`/remember` 写不进、读不了记忆日志（施工 R-3 补） |
| `ERROR` | `connection task failed error=…` | 一个连接的任务崩了 |
| `WARN` | `create failed error=…`、`load failed session=… error=…` | 造不成、载入不了 |
| `WARN` | `lagged, resync session=…` | 掉了队 |
| `WARN` | `lagged, resync stream=config` | 配置的订阅掉了队（施工 8-4） |
| `WARN` | `replay not read session=… error=…` | 带 `after` 订阅，补发的那一截读不了（施工 3-8 六补） |
| `INFO` | `session stopped, resync session=…` | 订阅着的会话停了（施工 4-9 再补三上） |
| `WARN` | `sessions not listed error=…`、`first event not read session=… error=…` | 列会话读不了 |
| `ERROR` | `list panicked error=…` | 列会话崩了 |
| `WARN` | `meta not read session=… error=…` | 列会话时后面的日志读不下去，标题、置顶照坏的那一段以前的算（施工 3-8 三补） |
| `INFO` | `session index created` | 起来时会话列表的索引没有，新建了一份（施工 3-8 七补，`store/index.md`） |
| `WARN` | `session index rebuilt reason=…` | 起来时索引读不了、坏了、版本不对，删掉换了一份空的 |
| `WARN` | `package invalid package=… file=… error=…` | 起来时读到一份写错的、撞了的软件包清单（施工 9-1 上，`packages.md`）；照样起来 |
| `WARN` | `package unreadable package=… file=… error=…` | 起来时一份软件包清单读不了（施工 9-1 上） |
| `WARN` | `session index unusable error=…` | 删了重建也打不开：这一回每次列会话都整份读 |
| `WARN` | `session index not read error=…` | 列会话时读出索引坏了：删掉重建，这一次整份读 |
| `WARN` | `session index not updated session=… error=…` | 列会话时补好的一行写不回去 |
| `WARN` | `session index row not removed session=… error=…` | 删会话时删不掉索引里那一行 |
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
| `ERROR` | `job output panicked error=…` | 读后台命令的输出时崩了（施工 7-4 补） |
| `WARN` | `human not read error=…` | `human.get` 给人看的字读不懂（施工 W-1） |
| `ERROR` | `human panicked error=…` | 读给人看的字时崩了（施工 W-1，照别的 `spawn_blocking` 同一个写法） |
| `WARN` | `files index failed dir=… error=…` | `fs.find` 建清单时读不了一层目录，跳过它接着建（施工 W-2） |
| `ERROR` | `files panicked error=…` | `fs.list`、`fs.find` 在阻塞线程里崩了（施工 W-2） |
| `WARN` | `upload not stored error=…` | 分块上传暂存、改名进位置没成（施工 W-5） |
| `ERROR` | `upload panicked error=…`、`upload cleanup panicked` | 分块上传在阻塞线程里崩了；连接断了、作废时删暂存那一步崩了（施工 W-5） |

`mermaid.render` 的日志目标是 `miyu::mermaid`，不是 `miyu::endpoint`：`WARN not ready error=…`，画图的库初始化不了。见 `mermaid.md`「出错」。

`link.preview` 的日志目标是 `miyu::net`：`WARN link preview failed host=… why=…`、`WARN link image not stored error=…`、`WARN not ready error=…`，见 `net.md`「出错」。在后台答的任务崩了（它的回应永远不会来了）记 `ERROR background request panicked error=…`，目标 `miyu::endpoint`（施工 W-7）。

`model.call` 的 `model` 解析不出记 `DEBUG unknown model why=…`（目标 `miyu::endpoint`）；调一次记一行 `INFO model call …` 或 `INFO model call failed …`，目标 `miyu::session`，不带会话编号（施工 8-20，`models.md`「出错」）。

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
| `bad_code` 到 `local_only` | 照 `web-module.md`「给人看的字」（施工 W-8） | |
| `unknown_persona` | 没有这个人格。 | There is no such persona. |
| `persona_invalid` | 这个人格的文件写错了，详情在 data.problem 里。 | This persona's files have a mistake; data.problem says where. |
| `unknown_preset` | 没有这个预设。 | There is no such preset. |
| `preset_invalid` | 这个预设的文件写错了，详情在 data.problem 里。 | This preset's file has a mistake; data.problem says where. |
| `preset_conflict` | 这个预设刚被别处改过，重新读一遍再改。 | This preset was just changed elsewhere; read it again and retry. |
| `persona_conflict` | 这个人格刚被别处改过，重新读一遍再改。 | This persona was just changed elsewhere; read it again and retry. |
| `nothing_to_delete` | 你这一层本来就没有，没有可删的。 | There is nothing of yours to delete here. |
| `unknown_file` | Miyu 不读这个文件：能查的是配置、密钥文件、人格目录里的 persona.toml 和 prompts/examples.md、预设、软件包清单。 | Miyu does not read this file: it checks the config, the secrets file, persona.toml and prompts/examples.md in persona directories, presets and package manifests. |
| `not_a_directory` | 这不是一个目录。 | This is not a directory. |
| `unknown_package` | 没有这个软件包。 | There is no such package. |
| `package_exists` | 出厂的软件包里已经有这个编号。 | A shipped package already has this id. |
| `package_required` | 这个软件包是必需的，不能卸。 | This package is required and cannot be removed. |
| `package_invalid` | 这份清单装不上，详情在 data.problem 里。 | This manifest cannot be installed; data.problem says why. |
| `not_an_extension` | 这个软件包是界面，不由核心拉起。 | This package is an interface; the core does not start it. |
| `extension_off` | 这个扩展关着，先打开它。 | This extension is off; turn it on first. |
| `needs_approval` | 这个扩展要的能力还没批准。 | This extension's capabilities are not approved yet. |
| `session_not_found` | 没有这个会话。 | There is no such session. |
| `unknown_call` | 没有这次调用。 | There is no such tool call. |
| `no_system_account` | 这个场所的会话要归系统账号，只有带系统账号的扩展能开。 | This venue's session belongs to a system account; only an extension with one can open it. |
| `venue_session` | 这是通讯平台的场所会话，本机的头不能直接说话。 | This is a chat platform venue session; local heads cannot talk in it directly. |
| `unknown_command` | 没有这个命令。 | There is no such command. |
| `command_not_allowed` | 只有主人和管理的人能用命令。 | Only the owner and managers can use commands. |
| `owner_only` | 只有主人能用这个命令。 | Only the owner can use this command. |
| `session_stopped` | 这个会话停了，详情在运行日志里；再发一次会重新载入。 | This session has stopped; the runtime log has the details. Sending again reloads it. |
| `session_broken` | 这个会话载入不了：它的日志或者策略快照坏了。 | This session cannot be loaded: its log or policy snapshot is broken. |
| `empty_message` | 消息是空的。 | The message is empty. |
| `dir_too_wide` | 加进来的目录太宽：家目录、根目录、Miyu 的数据根不能整个放行。 | An added directory is too wide: the home directory, the root and Miyu's data root cannot be opened up whole. |
| `attachment_unreadable` | 读不了这个文件：没有、不是普通文件，或者没有权限。 | This file cannot be read: it is missing, not a regular file, or not permitted. |
| `attachment_too_big` | 附件太大：一个最多 20 MiB，图片最多 5 MiB、每边最多 8000 像素。 | The attachment is too big: at most 20 MiB, and an image at most 5 MiB and 8000 pixels a side. |
| `attachment_in_data_root` | Miyu 的数据根里的文件不能当附件。 | Files in Miyu's data root cannot be attached. |
| `unknown_attachment` | 附件不在核心里：先用 blob.put 传上来。 | The attachment is not in the core; upload it with blob.put first. |
| `path_unreadable` | 读不了这个路径。 | This path cannot be read. |
| `path_forbidden` | 这是 Miyu 自己的数据，不给看。 | This is Miyu's own data and is not shown. |
| `mermaid_too_long` | 这张图的源码太长了。 | The diagram source is too long. |
| `mermaid_failed` | 这张图画不出来。 | The diagram could not be drawn. |
| `too_many_uploads` | 同时传的文件太多了，等前面的传完。 | Too many uploads at once; wait for the others to finish. |
| `upload_unknown` | 没有这个上传，可能等太久作废了，重新传一次。 | No such upload; it may have expired. Upload the file again. |
| `upload_offset` | 上传接不上，从核心说的地方接着传。 | The upload is out of step; continue from where the core says. |
| `upload_incomplete` | 文件还没传完。 | The file is not fully uploaded yet. |
| `unknown_blob` | 找不到这份内容。 | This content cannot be found. |
| `not_running` | 没有正在进行的回合，打断不了。 | No turn is running, so there is nothing to interrupt. |
| `turn_running` | 有回合在进行：先打断，或者等它做完。 | A turn is running; interrupt it or wait for it to finish. |
| `unknown_turn` | 没有这一轮，或者它已经撤掉了。 | There is no such turn, or it has already been undone. |
| `nothing_to_unrevert` | 没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。 | There is nothing to restore: nothing was undone, or a turn or compaction came since. |
| `restoring` | 正在撤销、恢复，等它做完再来。 | An undo or restore is still in progress; try again when it is done. |
| `nothing_to_revert` | 没有能撤销的回合。 | There is no turn to undo. |
| `nothing_to_compact` | 没有能压的：还没压过的内容都在原样留着的最近一段里。 | Not enough to compact: everything not yet compacted is in the recent part that stays as it is. |
| `nothing_to_clear` | 上下文为空 | The context is empty. |
| `unknown_job` | 没有这个任务，或者它已经结束了。 | There is no such job, or it has already ended. |
| `not_a_command` | 这是子代理，不是后台命令：去看它的会话。 | This is a subagent, not a background command; open its session instead. |
| `not_redoable` | 无法重做 | Cannot redo. |
| `nothing_to_recap` | 还没有可回顾的内容 | There is nothing to recap yet. |
| `not_asking` | 它没在等回答：已经答过，或者已经了结了。 | It is not waiting for an answer: it was answered or settled already. |
| `not_ambient` | 不是旁听记下的消息 | Not an overheard message. |
| `not_a_provider` | 只有扩展能提供工具 | Only extensions can provide tools. |
| `bad_tool` | 工具规格不对 | Bad tool spec. |
| `already_answered` | 已经回过 | Already answered. |
| `no_rule` | 这一次只能允许这一次，或者拒绝。 | This one can only be allowed once or denied. |
| `unexpected_reason` | 只有拒绝能带理由。 | Only a denial can carry a reason. |
| `bad_answer` | 回答和题目对不上：几道题几条，只能选题目里的选项。 | The answers do not fit the questions: one per question, picking only their options. |
| `unknown_config_key` | 没有这一项配置。 | There is no such setting. |
| `config_invalid` | 配置有几处不对，没有改。 | Some settings are not right. Nothing was changed. |
| `config_conflict` | 这一项刚被别处改过，没有改：先看看现在的值。 | This was just changed elsewhere. Nothing was changed. Look at the current value first. |
| `config_file_broken` | 配置文件现在读不进来，没法只改一项：先把它改好，比如用 miyu config edit。 | The config file cannot be read right now, so a single setting cannot be changed. Fix the file first, e.g. with miyu config edit. |
| `no_project_config` | 这个目录找不到项目配置。 | There is no project config for this directory. |
| `unknown_secret` | 没有这个密钥。 | There is no such secret. |
| `unknown_provider` | 没有这个供应商。 | There is no such provider. |
| `unknown_model` | 配置里没有这个模型或者池。 | There is no such model or pool in the configuration. |
| `no_model` | 没有可用的模型。 | No model is available. |
| `cooling` | 模型都在冷却，稍后再试。 | All models are cooling down; try again later. |
| `model_failed` | 请求模型出错了。 | The model request failed. |
| `recap_failed` | 回顾没写成：请求模型出错了。 | The recap could not be written: the model request failed. |
| `memory_unavailable` | 这里没有记忆：没有人格、记忆关着，或者是通讯平台的会话。 | No memory here: no persona, memory is off, or this is a platform session. |
| `unknown_memory` | 没有这一条记忆。 | There is no such memory. |
| `memory_not_current` | 这一条已经改掉、作废或者清掉了。 | That memory was already replaced, forgotten or cleared. |
| `memory_too_long` | 一条记忆太长了，字数和上限在 data 里。 | The memory is too long; data has its length and the limit. |
| 别的 | 被拒绝了。 | Refused. |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 认请求的每一条、回应是一行、去掉行尾、太长的 |
| `crates/miyu-endpoint/tests/endpoint.rs` | 握手先行、令牌（长短也比）、版本对不上，被拒的断开；握手的回应照核心探到的报沙盒，四种（施工 5-4 下）；造会话、说话；同一个造会话只造一个；没有的会话；载入上一次运行的会话；两个连接只载入一次；照头的语言拒绝；JSON-RPC 的错误码、通知不回应；太长的断开；工作目录跟着头；不能输入的头造的会话没人确认；空消息；停了的会话下次再载入；打断时排着的接着发 |
| `crates/miyu-endpoint/tests/limits.rs` | 订阅的回应带限额（施工 6-3 补）：窗口、压缩线照核心的模型算；没报窗口的是 `{}`，窗口太小的只有 `window`；已经订阅着的再订阅也带；核心重启以后载入的照样带；`unsubscribe` 还是 `{}`；回应带 `model`，照蓝图的例子一字不差（施工 8-10） |
| `crates/miyu-endpoint/tests/subscribe.rs` | 先见结果后见回应；两个会话不串；取消订阅以后不推；掉队推 `resync`、回应一条不丢、重新订阅照常推；积压时回应排在推送后面；取消订阅时已经交给转发任务的回应照样到；会话停了推 `resync`；订阅要握手、要有这个会话、只认 `events` |
| `crates/miyu-endpoint/tests/replay.rs`（施工 3-8 六补） | 补发：`after` 是 0 补整份日志、一字不差、都在回应前面、回应带 `upto`、没有瞬时的，补完接着推下一条；中间的序号补之后的；最后一条、比最后一条大的什么都不补、照常推、一条不重；不写、写 `null` 的照旧、回应没有 `upto`；写错的八种 `bad_params`、先查参数不找会话、一个都没订阅上；核心重启以后要载入的照样补；订阅着、推送堵着时带 `after` 再订阅，旧的手里的回应照样到、补的中间不夹旧的、之后只有新的在推；日志坏了的 `session_broken`、没订阅上，没有要补的不读日志 |
| `crates/miyu-endpoint/tests/replay_race.rs`（施工 3-8 六补） | 掉了队的头带上最后看到的序号重新订阅，看到的和补的合起来就是日志；真核心：另一个头一句接一句地说、会话一直在追加，中途几个头先后从头订阅，每个头补的和推的合起来都和日志一字不差 |
| `crates/miyu-endpoint/tests/restart.rs` | 核心重启以后：不带 `cwd` 载入的会话照最后一轮的工作目录、没开过回合的照造会话时的；重发的造会话交回原来那一个 |
| `crates/miyu-endpoint/tests/check.rs`（施工 8-30） | `check`：不写文件的照磁盘上现在的字查配置、照核心手里的查密钥、人格每一层各查各的，先后、代码、级别、行对，给人看的那一句照连接的语言；写了文件的照位置认、只查那一份，项目配置照 `.miyu/config.toml` 认、相对的照 `cwd` 接，还没有的人格文件读不了，认不出的 `unknown_file`，多写格的参数不对 |
| `crates/miyu-endpoint/tests/personas.rs`（施工 P-1 上） | 家目录里的人格进 system、示范对话排在前面；不写人格照默认、个人设置压着系统配置、都没设的无人格；没有的、编号不对的、写错的拒绝，默认人格指着没有的当没设（施工 P-4 上）；`null` 明着无人格、不写进会话列表和订阅回应；`venue.session` 带人格造、找回时不看；`persona.list`、`persona.get` |
| `crates/miyu-endpoint/tests/persona_set.rs`（施工 P-3 下、补） | 不写编号新建、核心起 `persona-1`、`persona-2`，一次带名字、人设、一对一对的示范对话、角色扮演提示，写成文件的写法、空行去掉，开会话用得上，一样的字不再写，只删不写的新建参数不对；读原文和版本、不给来处，出厂的没改过 `remove` 是空的、改过是 `restore`，旧版本存撞上什么都不写，空的字是这一段空的；删提示词回到下面的、删没有的不出错；写错的示范对话、`memory.scope`、写不回原样的 `pairs` 什么都不写、带照连接语言的 `message` 和行；参数不对的十种、读的两种、没有的人格；删了挪进回收处、整个目录、读不到了，盖在出厂上的回到出厂、本来没有的、编号不对的 |
| `crates/miyu-endpoint/tests/preset_set.rs`（施工 P-3 中、补） | 不写编号新建、核心起 `preset-1`、`preset-2`，别的照旧全开，`remove` 是 `delete`，开会话用得上，只删不写的新建参数不对；改出厂的只写改了的项、`remove` 是 `restore`，没改过的是空的；注释、顺序照原样，以前的语言表换成一句字；删一项回到下面的；`expect` 对不上的两种、对得上的照写；写错的值、不认识的键、空的名字、默认人格写法不对的整条不收、什么都不写、带照连接语言的 `message`；参数不对的九种；删你那一层回到下面的、只有你那一层的就没了、本来没有的、编号不对的；一样的值不写 |
| `crates/miyu-endpoint/tests/presets.rs`（施工 P-2 上） | 不写预设照默认、个人设置压着系统配置、指定的压着默认；人格照「指定、预设的默认人格、`persona.default`」；没有的、编号不对的、写错的拒绝、什么都不造，默认预设指着没有的也拒；会话列表、`subscribe` 写 `preset`，以前的日志不写；`venue.session` 带预设造、找回时不看；`preset.list`、`preset.get`；`check` 查预设 |
| `crates/miyu-endpoint/tests/edges.rs` | 不握手的到时断开、握手了的不受管；数组的 `params` 参数不对；握手被拒照它报的语言说；人格目录不存在是 `unknown_persona`、目录在而读不了是 `internal_error` |
| `crates/miyu-endpoint/tests/list.rs` | 从新到旧、只要一次性的、`limit`、参数不对、空的；每一项带 `cwd`、合写法的 `last_active`，闲着的不写 `busy`（施工 C-3） |
| `crates/miyu-endpoint/src/list/tests/indexed.rs`、`tests/index.rs`、`tests/index_log.rs`（施工 3-8 七补） | 读索引的和整份读的一字不差；补上、重读、重建；索引那一行跟着会话走、删会话删行；几行运行日志（`store/index.md`「守着它的」） |
| `crates/miyu-endpoint/tests/sessions.rs`、`src/list/tests.rs`（施工 C-3） | 工作目录跟着头报的换、忙着的写 `busy`（子会话也算）、最近一次动静是日志最后一条；工作目录照最后一条带 `cwd` 的、不带的不盖、一条都没记的写 `~`，哪种事件都算动静，叫停的旗举了一个都不读；她用 `sessions` 列的和它是同一份（`tools/sessions.md`） |
| `crates/miyu-endpoint/tests/meta.rs` | 改标题、置顶（施工 3-8 三补）：改名去掉空白、只写改了的那一格、推送在回应前面、`by`、`cause`；置顶、取消、两样一起；`null` 去掉标题记成空的；和现在一样的六种什么都不记；200 个字收、201 个字和空白的不收；两格都不写（含 `pinned` 写 `null`、会话没有的）、类型不对、会话编号不对是参数不对，没有的会话找不到；回合进行中改的带上回合；`session.list` 带标题、置顶，取消了、去掉了的不写，核心重启以后照样，载入以后照日志接着比；日志坏了的照样列出来，工作目录、最近一次动静照第一条（施工 C-3） |
| `crates/miyu-endpoint/tests/sessions_changed.rs`、`src/listing/tests.rs`、`src/subscriptions/sessions/tests.rs`（施工 9-5） | 订阅的回应是整张列表、没标题的带 `preview`；造会话、一轮开始、空下来、改名、置顶、删会话各推一项，整项和 `session.list` 的一样，有标题就不带预览；内核起的标题也推；取消以后不推；`sessions` 带 `session`、`after` 是参数不对；另一个连接一直改名时订阅，回应以后推来的不比回应里的旧；列表记的号盖住它前面的变化；转发任务先写回应、丢掉号不大于它的、掉队推 `resync` 停下 |
| `crates/miyu-endpoint/tests/delete.rs` | 删除会话（施工 3-8 三补）：空闲的整个目录挪进回收处、日志不变、`deleted_at` 是删的时刻，列不出来，再发命令、订阅、改名、打断、再删都是没有这个会话，重发造它的那一条另造一个；回合进行中的拒绝、什么都没动，打断以后删得掉；核心重启以后没在跑的不载入就删（被重启打断的那一轮不接着干）；参数不对、没有的会话 |
| `crates/miyu-endpoint/src/sessions/delete/tests.rs`（施工 7-8） | 删子会话在表的锁里停它、父会话记回报：父会话一记下它停了就去叫醒它，拿到表的锁时它已经删掉了，删得掉（挪进锁以前，这时它又开了一轮，删的时候说有回合在进行） |
| `crates/miyu-endpoint/tests/orphans.rs`（施工 7-8） | 真核心：父会话派出去一个子代理，没来得及记下 `job.started` 就崩了（日志截在派它的那条回复后面），再载入父会话时子会话挪进回收处；一次派两个、只记下一个的只收那一个；记下了的照留 |
| `crates/miyu-endpoint/tests/delete_children.rs` | 删会话连子会话（施工 3-8 三补）：主会话派的子代理、子代理派的孙代理一起停下、各自挪进回收处，两条后台命令各杀一次、谁都不记回报；删一个正忙的子会话，主会话记一条 `stopped` 的回报（`by` 是子会话、不带 `by_model`）、被叫醒，子会话挪走、主会话还在；报过 `done` 又被留了言、主会话又在等它的，删它也记一条 `stopped`、叫醒主会话（施工 7-7）；主会话已经进了回收处的，删子会话照样删、不送；子代理派的编号带着它自己的 `j1`（`j1.1`、`j1.2`），删孙会话时子会话记的 `stopped` 回报是 `j1.1`（施工 7-1 补） |
| `crates/miyu-endpoint/tests/spawn.rs` | 会话里派子代理，会话表造出子会话、交代送进去、替身模型在子会话里答话；`session.list` 里子会话写着父会话、主会话写 `null`（施工 7-5） |
| `crates/miyu-endpoint/tests/revert.rs` | 协议上撤销、恢复；三种拒绝的中文；`turn` 写 0 |
| `crates/miyu-endpoint/tests/permission.rs` | 协议上切权限级别（施工 3-8 再补）：切到完全放开、开只读、两样一起换，各记一条、推给订阅着的头、回应 `{}`；和现在一样的四种什么都不记不推；两格都不写（含写 `null`、会话没有的）、级别和只读的值不对、会话编号不对、没写会话是参数不对；没有的会话找不到，停了的会话是停了；回合进行中收紧成只读，真核心走一遍：等着的写入当场补 `denied`、和切权限同一批、推送在回应前面，放行以后请求之前注入只读那一块（切换那一份，带上一级 `workspace`，施工 2-7 补），写的一次没跑 |
| `crates/miyu-endpoint/tests/answer.rs`（施工 D-1） | `session.answer` 真核心走一遍：允许这一次照常跑、回应列出事件；本会话都允许以后同一个目录不再问；拒绝的理由她看得到、这一轮接着走；参数不对的七种 `bad_params`、什么都没记；`unexpected_reason`、答过了 `not_asking`，话照握手的语言 |
| `crates/miyu-endpoint/tests/job_stop.rs` | 协议上停子代理（施工 7-4）：回应 `{}`、回应之前父会话记下了回报、子会话那一轮被父会话打断；停过的、没有的 `unknown_job`，中文、英文；编号不合写法、不是字符串的参数不对；没有这个会话 |
| `crates/miyu-endpoint/tests/job_output.rs` | 协议上读后台命令的输出（施工 7-4 补），后台命令用假的：跑着的读到这时为止的、`running` 是真，和她用 `jobs` 读到的一样；结束了的读 blob（拿掉输出文件照样读得到）、`running` 是假，和 `jobs` 读到的一字不差；`tail` 截尾、`truncated`、`lines`，最后一段没有换行的照样，不写 `tail` 交最后 200 行；超了上限从前面按整行去掉；开不了的输出文件当是空的；空的；没有这个任务、编号不合写法、`tail` 不对的七种（先查、不找会话）、没有这个会话、子代理的拒绝，中文、英文；拒绝的什么都不写 |
| `crates/miyu-endpoint/src/job_output/tests.rs` | 取尾巴（施工 7-4 补）：照 `jobs` 数行、只照换行切；最后几行；上限正好 131,072 字节的一行整行给、多一个字节只留末尾；超了从前面按整行去掉；最后一行太长只交末尾、前面的不接上（前面那一行正好放得下也不接），读的时候就去掉过前面的也一样；中间太长的一行、截过的一行后面又来了行，整行去掉；截处从一个字的开头起，剩半个字的跳过；解不开的字节换成 `�`；读不下去的读到多少算多少；最坏的回应（每个字节都转义成六个、编号全是引号）放得进一行；回应的格 |
| `crates/miyu-endpoint/tests/human.rs`（施工 W-1） | 协议上 `human.get`：和 `Human::load` 读到的一样，工具的样子一样、说法的编号带位置（`core/…`、`software/<包>/…`）、模板是原文一个字不换；软件包盖掉内核的同名工具；没有这种语言照英文；不写 `language` 照握手的语言；`language` 不合写法参数不对；回应里没有 `config` 那一格；读不懂 `internal_error`；改了资源，核心不重启下一次调就是新的；真核心照源码树的资源，`zh`、`en`、`ja` 三种都交得出 `tools`、`said` |
| `crates/miyu-endpoint/src/queries/tests.rs`、`crates/miyu-core/tests/packages.rs`（施工 W-4） | 查询表：没登记的方法交回 `None`、照协议是 `unknown_method`；登记过的名字正好对上才找得到。`mermaid.render`：真核心走一遍，一张流程图、一张时序图都出 SVG，回应的 `marks` 和 SVG 里用的三种记号色对得上，同一份源码两次拿到一样的回应；空的 `bad_params`、超过 64 KiB `mermaid_too_long`、画不出 `mermaid_failed`（`data.detail` 不是空的）。细节见 `mermaid.md`「守着它的」 |
| `crates/miyu-endpoint/src/queries.rs` 里的测试、`crates/miyu-core/tests/packages.rs`、`crates/miyu/tests/link_preview.rs`（施工 W-7） | 在后台答的只有照 `register_background` 登记的；同一个名字两种登记也 panic。`link.preview`：没登记回 `unknown_method`；登记了的读不成地址、不是 http 不碰网络就答；`url` 没写、不是字符串、`params` 是数组 `bad_params`；在后台答的不挡后面的请求、回应照 `id` 对上、连接断了它跟着停；真的核心照环境变量里的代理做出卡片、图用 `blob.get` 读得回来。细节见 `net.md`「守着它的」 |
| `crates/miyu-endpoint/tests/redo.rs` | 协议上重做（施工 4-7 再补）：回应带撤销的几样和重发的那一句、推送里是一批撤销、原话、新的一轮，新的一轮的请求和撤掉的那一轮的一字不差；换了话的推送里是新的话、`said` 是原来的；改过文件的先改回、回应带 `files`；重做以后恢复不了；最后一轮是清空、没说过话的，有回合在进行、换成空的拒绝，中文、英文；`text` 不是字符串、会话编号不对的参数不对，写 `null` 当没写；附件照带、换掉、不要，没有的 blob `unknown_attachment` 什么都不写 |
| `crates/miyu-endpoint/tests/compact.rs` | 协议上手动压缩（施工 6-8）：回应是那一轮的开头、推送里压好了；要求原样到了摘要请求里；撤掉那一轮的回应里没有 `said`；有回合在进行、没有能压的两种拒绝，中文、英文；`instructions` 不是字符串的参数不对 |
| `crates/miyu-endpoint/tests/recap.rs`（施工 3-8 四补） | 协议上要回顾：回应是那一句、照到的、不是交回的；推送里先有回顾的 `model.called`、`session.recapped`，都不带回合编号、`cause` 是这一条，再是回应，别的头也收到；请求是一条 user、没有 system 和工具面；没有新内容再要一次交回上一句、不请求；有回合在进行时照收、照到的是这一轮那句话；没有能回顾的、没写成的两种拒绝，中文、英文，没写成的不再来；会话编号不对、没写、不是字符串的参数不对，没有的会话找不到 |
| `crates/miyu-endpoint/tests/title.rs`（施工 3-8 五补） | 自动起标题：第一轮答完，订阅着的头收到起标题的 `model.called`（`purpose: "title"`）和 `session.meta_changed`，`by` 是内核、不带回合编号和 `cause`；请求是一条 user、没有 system 和工具面，只喂第一轮；`session.list` 带上标题，核心重启以后照样；第二轮不再起；人先起过名的不起 |
| `crates/miyu-endpoint/tests/commands.rs`（施工 O-6） | `command.run`：`/clear`、别名 `/reset`（开头空白、后面跟的字照认）清空并记 `command.ran`，回执是中文那一句；`/stop` 打断、排着的留在日志里不接着开、子代理停了，没有回合也照样记；认不出的、`/` 后面是空白的 `unknown_command`，不以 `/` 开头的、多写格的参数不对，都什么都不写；内核拒的照原因回；同一个编号再发、重启以后再发回应一样、只记一次；场所里主人、管理的人能用，别人 `command_not_allowed`，不带 `as` 的 `venue_session`，本机的会话带 `as` 参数不对；`session.interrupt` 收 `keep` |
| `crates/miyu-endpoint/tests/workspace_command.rs`（施工 9-7 下） | `/workspace <路径>` 同 `set_workspace` 只换工作目录、加进来的目录照旧，回执是中文那一句，一样的不记换；相对的照 `cwd` 接、没带的照会话的接，路径里的空白照留，`cwd` 不是绝对的参数不对；不带路径的只说现在在哪；不在的、文件、数据根里的照原因拒绝、什么都不记；`~` 太宽退回账号的工作区、回执说一声；场所里管理的人 `owner_only`，主人能换 |
| `crates/miyu-endpoint/tests/clear.rs` | 协议上清空（施工 6-8 补）：回应是那一轮的开头、订阅的推送里是那一批三条、不请求模型；下一次请求里没有清空以前的；撤掉那一轮回应里撤掉了一次压缩、没有 `said`，再问看得到了；有回合在进行、本来就空的两种拒绝，中文、英文；会话编号不对、没写的参数不对 |
| `crates/miyu-endpoint/src/sessions/tests.rs` | 父会话不在会话表里的不再造子会话、什么都没建（施工 3-8 三补） |
| `crates/miyu-endpoint/tests/workspace.rs` | 太宽的五种（`~`、家目录、根目录、数据根、数据根里面）和读不出家目录时的 `~`；项目目录、账号的工作区照旧；回应里的 `cwd`、重发的造会话 |
| `crates/miyu-endpoint/tests/dirs.rs` | 加进来的目录（施工 5-10 上）：造会话、说话时报的记进这一轮，不写的照旧、写空的就没有；太宽的五种整条命令都不收、什么都没写；核心重启以后照最后一轮的 |
| `crates/miyu-endpoint/tests/idle.rs` | 连着连接、跑着回合不空闲；停下全部会话，跑到一半的记成重启了 |
| `crates/miyu-endpoint/tests/attach.rs` | `blob.put`（施工 3-9 三补）：传路径、传内容；照内容认图片（扩展名不算）、PDF、文本、别的文件，量宽高，回应的格照字母排、存成管理员的 blob；写了的媒体类型什么时候算、改名、写 `null` 等于没写；太大（20 MiB、图片的宽高和 5 MiB，正好在线上的收）；数据根里的不给、管理员的工作区给、指到数据根里的链接不给；读不了（没有、目录、没有家目录时的 `~`）；参数不对的十二种、一个都没存；四种拒绝的中英文 |
| `crates/miyu-endpoint/tests/uploads.rs`、`crates/miyu-core/tests/packages.rs`（施工 W-5） | `blob.open`、`blob.write`、`blob.close`：分块传完和 `blob.put` 同一个回应、同一个 blob；接不上回 `upload_offset`（`data.received` 对）；没收齐 `close` 回 `upload_incomplete`，还能接着写完；别的连接拿编号用不了；连接断了、60 秒不写都作废并删暂存；`size` 超过 20 MiB 当场 `attachment_too_big`；同时开到第 5 个 `too_many_uploads`，关掉一个腾出位置；`data` 不是 base64、一块超过 512 KiB、加起来超过 `size` 都是 `bad_params`；图片照 `blob.put` 的规矩认、查上限。`packages.rs` 另测 `packages::clear_uploads`：崩了留下的 `upload-*` 清掉、真的 blob 不碰、账号还没存过东西时不出错 |
| `crates/miyu-endpoint/tests/attach_send.rs` | `session.send` 带附件（施工 3-9 三补）：照先后接在文字后面，宽高、种类照核心量的，图片块带着 `blob.put` 的名字（施工 3-9 四补），她收到的请求里就是这几块；只有附件也是一句话，`null` 是没有；blob 不在的拒绝、什么都没写、换的工作目录也没送进会话；附件的格不对的七种 |
| `crates/miyu-endpoint/tests/files.rs` | `fs.list`、`fs.find`（施工 W-2）：真核心上数据根不列不找、账号的工作区照样列；开头对、大小写不论、点开头的打了点才列、目录在前、50 条截断、`marks`；模糊找有 `marks`、子目录的 `path` 用 `/`；清单没建完先给一部分、`building`；`fresh` 隔一段时间才重建、不是 `true` 不重建；最多记 4 份、多了丢最久没用的；换不成真实的位置、不是目录的 `path_unreadable`；没写 `cwd` 的 `bad_params` |
| `crates/miyu-endpoint/tests/hello.rs`（施工 W-3） | 握手的 `host`：三格总有、`platform` 是这台机器的、`workspace` 换成真实的位置（链接也换成指的地方）、没有系统的家目录 `home` 是 `null`、没人建过工作区就回原样的路径（不替连上来的头造目录）。`fs.realpath`：`~` 照家目录接、`cwd` 可以不写也可以本身是 `~`；相对的没给 `cwd` 的 `bad_params`；往上找最近在的一层、后面几段原样接上；路中间的链接换成指的地方；落在数据根里的照样换，不查边界；一层都不在（没有家目录）`path_unreadable` |
| `crates/miyu-endpoint/tests/login.rs`、`login_log.rs`（施工 W-8） | 握手的四种凭据、`account.setup_code`、`account.setup`、`account.logout`、作废了断开、运行日志里没有码、密码、令牌（`web-module.md`「守着它的」） |
| `crates/miyu-endpoint/tests/reads.rs`（施工 W-6） | `blob.get`：读一段、读到结尾就停、`offset` 过了结尾是空的、不写 `offset`、`length` 的默认值、`length` 写 0 只问大小；没有这个 blob `unknown_blob`。`fs.read`：数据根拒、工作区能读、相对的 `bad_params`、`~` 接系统的家目录；没有、目录、（Unix）套接字 `path_unreadable`。两个方法 `length` 超过 512 KiB 都是 `bad_params`；拒绝的中英文 |
| `crates/miyu-endpoint/tests/from.rs`、`src/from/tests.rs` | `session.send` 带 `from`（施工 7-10）：记成 `harness`、带着名字，不带的、`null` 照旧记成本人；闲着开一轮、`cause` 是这一条，正忙排进这一轮；附件照收；控制字符去掉、截到 128 字节不截断一个字；空的、只有控制字符的、不是字符串的参数不对，什么都没写；`session.create`、`session.redo` 写了不理 |
| `crates/miyu-endpoint/src/attach/kind/tests.rs` | 认附件：量得出的图是图片、头写的不算，量不出的当文件；图片的上限和线上的；PDF 照开头认；别的文件照头写的，写成 PDF、图片的照内容认，文本、空的、二进制、不是 UTF-8 的 |
| `crates/miyu-endpoint/tests/tools.rs` | 造会话、载入时用核心的工具目录；核心的沙盒造会话、载入时都交给会话，沙盒用不了的核心上执行命令没人能确认就拒（施工 5-4 上） |
| `crates/miyu-endpoint/tests/socket.rs` | 真的套接字（Windows 上是命名管道）上握手、造会话、说话，第二个头也连得上 |
| `crates/miyu-endpoint/tests/config.rs`、`config_trust.rs`（施工 8-2） | 握手的 `language`、`config_errors`；`config.schema`、`config.get`、`config.check`；`unknown_config_key` 带 `problems`；开局只读照配置、照信任着的项目配置；造会话、说话的回应带 `untrusted_project`（`config.md`「守着它的」）。`config.trust` 的回答、拒绝、日志（施工 8-3） |
| `crates/miyu-endpoint/tests/models.rs`（施工 8-7） | `model.list` 的形状、来源、状态；`provider` 只看一家、`unknown_provider`、参数不对；`refresh` 拉完再答、不写的在后台拉；冷却（施工 8-9）：模型照能用的 key 里最好的那个，都在冷却的带最早恢复的 `until`、`class`，认证失败停了整个 key 的那个 key 也是 `cooling`，取不到值的 key 不算（`models.md`「守着它的」） |
| `crates/miyu-endpoint/tests/models_pools.rs`（施工 8-8） | `model.list` 的 `pools`（8-8 补多 `subagent`、`description`，没有 `tiers`）、`uses`；`session.create` 的 `model` 记下解析出的、`unknown_model` 什么都不造、不是字符串的 `bad_params`；`session.configure` 照这时的配置解析好记一条、先推再回应、一样的不记，参数不对的几种 `bad_params`、先找会话、解析不出的 `unknown_model`、都什么都不记；`subscribe` 的 `model` 照真路由解析出的写，轮换的池只有 `ref`，一个都没有的不写（施工 8-10，`models.md`「守着它的」） |
| `crates/miyu-endpoint/tests/models_effort.rs`（施工 8-18；8-18（补）去掉会话那一层） | `session.configure` 写了 `effort` 回 `bad_params`、不写 `model` 回 `bad_params`；`subscribe` 的 `model` 多 `effort`，`from` 是配置的哪一层；`model.list` 的 `facts.effort`、多一格 `key`；配置里写错的 `unknown_effort`（`models.md`「守着它的」） |
| `crates/miyu-endpoint/tests/providers.rs`、`providers_test.rs`、`providers_log.rs`（施工 8-11） | `provider.detect`、`provider.catalog`、`provider.test` 的形状、参数不对、`unknown_provider`，`{value}` 的 key 不进回应和运行日志（`models.md`「守着它的」） |
| `crates/miyu-endpoint/tests/model_call.rs`、`model_call_log.rs`（施工 8-20） | `model.call` 的回应形状、参数校验、blob 的账号、几种出错的 `data`、不进会话日志、运行日志那两行（`models.md`「守着它的」） |
| `crates/miyu-endpoint/tests/secrets.rs`、`secrets_log.rs`（施工 8-5） | `secret.*` 的回应、拒绝、日志；值不进回应、拒绝、系统日志、运行日志（`config.md`「守着它的」） |
| `crates/miyu-endpoint/tests/config_set.rs`（施工 8-3） | `config.set` 的回应、每一种拒绝、`expect`、版本、手改重读、全收或者全不收、写不成什么都没变、日志（`config.md`「守着它的」） |
| `crates/miyu-endpoint/tests/config_watch.rs`、`config_watch_log.rs`（施工 8-4） | 订阅配置、取消、参数不对；手改推 `config.changed`；`config.set` 先见推送后见回应；掉队推 `resync`；改了语言下一句照新的（`config.md`「守着它的」） |

### 出处

- `04-核心协议.md` 第二节（JSON-RPC、分帧、拒绝的写法）、第三节（一次连接的全过程：订阅时先拿会话状态）、第四节（连接即身份、本机令牌）、第五节（事件流；会话状态由核心算）、第六节第 1、2、4、9 条、第七节（慢、`resync`、重连时补发）、第八节（版本）、第九节「先做的几样怎么写」；P1、P2。
- `09-压缩.md` 第二节：压缩线。
- `04-核心协议.md` 第九节 `session.recap`：回顾（2026-10-01 项目主人定），照 codex 的做法单独发一次辅助请求。
- `04-核心协议.md` 第九节 `session.set_permission_level`、`02-内核.md` 第六节「权限级别怎么切」、`11-权限与沙盒.md` 第二节：切权限级别。
- `04-核心协议.md` 第九节 `blob.put`、`22-命令行.md` 第三节 `--file`、`03-事件模型.md` 第四节（量不出尺寸的不当图片）、E4：附件（施工 3-9 三补）。
- `04-核心协议.md` 第九节 `session.set_meta`、`session.delete`，`03-事件模型.md` 第三节 `session.meta_changed`：改标题、置顶、删除会话；删了的进回收处、留 7 天（2026-09-30 项目主人定）。
- `04-核心协议.md` 第九节 `mermaid.render`：mermaid 源码画成 SVG，细节见 `mermaid.md`（施工 W-4）。
- `04-核心协议.md` 第九节、`10-自带软件.md` 第四节 `net`：`link.preview`，细节见 `net.md`（施工 W-7）。
- `04-核心协议.md` 第十节「后续再定」（分块上传）：`blob.open`、`blob.write`、`blob.close`，细节见 `web-module.md`「六、分块上传」（施工 W-5）。
- `blob.get`、`fs.read`：分块读，细节见 `web-module.md`「七、分块读」（施工 W-6）。
- `02-内核.md` 第四节（拒绝附原因码）、不变量 9（同一个编号只生效一次）。
- `06-多用户与身份.md` 第二节、U13：本机连上来的是管理员 `admin`。
- `07-存储.md` 第七节：会话按需载入。
- `11-权限与沙盒.md` 第四节：当前目录太宽。

### 还没有的

设计里有、还没做的：

- 第九节表里的其余方法：`session.fork`、`command.run`、查询、账号……（`04-核心协议.md` 第九节）。
- blob 的一般回收（删会话以后没人引用的、`blob.put` 自己崩溃留下的那种临时文件）：`store.md`「还没有的」。
- 视图流，`view.*` 这些推送；核心决定「显示什么」（第五节、P3）。会话列表流 `sessions.changed` 施工 9-5 做了。
- 找回删了的会话、自动起标题（照第一句话生成，要请求模型）：以后（施工 3-8 三补）。回收处里的文件留着，找回时挪回去。
- 队列紧张时先合并同一条目的连续增量（第七节）。
- 头发现核心比自己旧，请求它空闲时重启（第八节，`kernel.restart_when_idle`）。
- 远程连接、登录令牌、WebSocket 和它的 Origin 检查；扩展、桥当提供者，反向调用（第二节、第四节）。
- 事件流只对会话的属主和有 `events.read` 能力的扩展开放（第五节）：现在连上来的只有管理员。
- `job.output` 照会话的读权限：现在连上来的只有管理员，不拦，多用户那一步再拦（施工 7-4 补）。
- 后台命令的输出推给头（施工 7-4 补定不做）：现在头照 1 秒读一次 `job.output`，每次从头数；推增量随 M8 的视图流再说。
- 成员只能切到只读和工作区（`11-权限与沙盒.md`）：现在连上来的只有管理员，`session.set_permission_level` 不拦，多用户那一步再拦。
- 消息结构只在 Rust 类型里定义一次，生成 JSON Schema 和 TypeScript 类型（第二节）。
- 会话空闲一段时间后 actor 退出（`07-存储.md` 第七节）。
- 视图流的会话状态（第五节，M8）里也带限额，字段只加不改。
