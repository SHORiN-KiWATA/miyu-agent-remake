## 场所会话与外部身份（核心那一半）

### 是什么

通讯平台的桥（`miyu-onebot`）把平台上的人接进会话。核心不认识 QQ，只给几样通用的东西（`18-通讯平台.md` 第十六节 Q27）：

- **主人对应表** `external.bindings`：系统配置里一张「平台上的身份 → 本机账号」的表。是不是主人，核心查它认，不信桥的一面之词。
- **场所会话**：一个场所（一个群、一个私聊）一个主线会话，桥用 `venue.session` 找回或者造。
- **代表外部的人说话**：`session.send` 多一格 `as`，桥说明这一句是平台上的谁说的、他在场所里是什么身份。代表别人只能降权：认主人是核心自己查表。
- **不在本机的头上**：场所会话不进 `session.list`、`sessions.changed`，跨会话的工具也不列它们（`18-通讯平台.md` 第十一节）。

状态：图纸，施工 O-3 做了（2026-10-07 主会话；形状给通讯平台的会话对过，对应表的写法、换了属主怎么找回由项目主人同一天定）。由施工 O-3 做，斜杠命令施工 O-6 做了（2026-10-07 主会话；命令名、`/stop` 停到哪由项目主人同一天定），属主是系统账号 `onebot` 的场所会话施工 O-4 下做了（2026-10-09 主会话；回应带 `account` 是通讯平台的会话同一天要的）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-config/src/key.rs` | 键的占位多一种 `<external>`：平台身份，照短名字的写法 |
| `crates/miyu-endpoint/src/settings.rs` | `ExternalSettings`：`external.bindings.<external>`，只能写在系统配置 |
| `crates/miyu-kernel/src/origin.rs` | `Person` 多 `via`（私聊里经哪个平台身份认出来的本人）；`External` 多 `account`（对应表里对着的本机账号）、`role`（桥报的场所里的身份） |
| `crates/miyu-store/src/index.rs`、`index/row.rs` | 索引一行多 `venue`，版本 3 |
| `crates/miyu-endpoint/src/venues.rs` | `venue.session`：照场所加属主找回或者造；`as` 怎么认、记成谁（照会话的属主比，施工 O-4 下）；`venue.binding`：问一个平台身份在对应表里对着谁（施工 O-31 前） |
| `crates/miyu-endpoint/src/venues/message.rs`、`crates/miyu-kernel/src/event/venue.rs` | `session.send` 的 `venue` 怎么查、记成什么（施工 O-13 上） |
| `crates/miyu-endpoint/src/appending.rs` | `events.append`：种类、大小、格怎么查，记成谁（施工 O-13 上） |
| `crates/miyu-endpoint/tests/venue_binding.rs`（施工 O-31 前） | `venue.binding`：系统账号的扩展问对着管理员的、没写的、对着不存在的账号的；写错的、多写格的、一次问几个的 `bad_params`；本机的头 `no_system_account`；只读 |
| `crates/miyu-endpoint/src/venues/records.rs` | `venue.records`：照会话日志和快照里的时区渲染判官看的记录（施工 O-24） |
| `crates/miyu-endpoint/tests/venue_judge.rs` | `venue.records` 的写法、写错的、没有的会话（施工 O-24） |
| `crates/miyu-endpoint/src/responding.rs` | `session.respond`：参数怎么查，交给内核的 `Respond`（施工 O-14 上）；`session.note`：交给内核的 `Note`，事实的查法两边共用（施工 O-14 补） |
| `crates/miyu-endpoint/tests/respond.rs` | 照旁听的几条开一轮、`triggers` 排好去重、事实接在后面；写错的什么都不记；`not_ambient`、`already_answered` 带上是哪几条；同一个编号再发只算一次（施工 O-14 上） |
| `crates/miyu-endpoint/tests/note.rs` | 空闲时记下、不带回合编号、不开回合，下一轮的请求里排在触发前面；正在跑一轮时带回合编号；空的、太多的、写错的什么都不记；同一个编号再发只算一次（施工 O-14 补） |
| `crates/miyu-endpoint/tests/venue_records.rs` | `venue` 原样记下、旁听的不开回合、写错的什么都不记；`events.append` 收的三类、回应带序号、不带回合编号，拒的几种；扩展只能写自己的包那一段（`system_account.rs`）（施工 O-13 上） |
| `crates/miyu-endpoint/src/system_accounts.rs` | 系统账号（施工 O-4 下，`packages.md`「`[process]`」）：这次起来认的有哪些、连接是谁、记忆照谁算；起来时建它们的家目录 |
| `crates/miyu-endpoint/src/list.rs` | 列会话、推会话列表时跳过场所会话 |

### 对外的样子

**主人对应表**（系统配置，2026-10-07 项目主人定按编号的表）：

```toml
[external.bindings]
"qq:10001" = "admin"
"qq:10002" = "admin"   # 小号
```

| 键 | 类型 | 默认 | 层 | 项目配置 | 生效时机 |
|---|---|---|---|---|---|
| `external.bindings.<external>` | 名字（本机账号） | 没有 | 系统 | 不能写 | `now`：下一句照新的认 |

- 键是平台身份，照短名字的写法（1 到 128 字节，没有控制字符），裸着写不下的照 TOML 加引号。一个号只能对一个账号（TOML 的键本来就不能重），一个账号可以对几个号。
- 值是本机账号。写了没有的账号（现在只有管理员）：配置照常读进来，认的时候当没写，记一行运行日志 `binding to an unknown account ignored`。

**问对应表**（施工 O-31 前，2026-10-10 核心定；通讯平台的桥挡在场所里做的事要的，`providers.md`「在场所里做的事」）：`venue.binding {"id": <平台身份>}`，回应 `{"account": <本机账号>}`，不在对应表里的、对着不存在的账号的（同上，当没写）是 `{"account": null}`。

1. 一次只问一个号；只读，对应表照旧。照这时的配置答（对应表改了，下一次问照新的）。
2. 只给系统账号的连接（核心拉起的、清单声明了系统账号的包的扩展，同 `venue.session` 第 1 条）；别的连接回 `no_system_account`。
3. `id` 写错的（不合短名字的写法，例如空的）、多写格的、少写的回 `bad_params`。

**`venue.session`**（命令）：找回或者造一个场所的主线会话。

| 参数 | 类型 | 说明 |
|---|---|---|
| `venue` | 字符串，必写 | 场所编号，短名字的写法，例如 `qq:private:10001`。核心不解读 |
| `kind` | 字符串，必写 | `private` 或 `group` |
| `peer` | 字符串 | 私聊的对方，平台身份，`private` 必写，`group` 写了是参数不对 |
| `cwd` | 字符串，可以不写 | 造的时候用；找回的不看 |
| `persona` | 字符串或 `null`，可以不写 | 造的时候用哪个人格（施工 P-1 上，`personas.md`）：桥照场所规则算好交来，核心不读场所规则；写 `null` 的明着无人格，不写的照默认人格，没设的无人格（施工 P-4 上：预设不再管默认人格）。找回的不看。明着写了没有的人格回 `unknown_persona`，什么都不造 |
| `preset` | 字符串，可以不写 | 造的时候用哪个预设（施工 P-2 上，`presets.md`）：同 `persona`，桥照场所规则算好交来；不写的照 `preset.default`。找回的不看。指着没有的回 `unknown_preset`、写错的回 `preset_invalid`，什么都不造 |

回应：`{"session": <编号>, "created": <布尔>, "account": <属主>}`（`account` 施工 O-4 下加：桥照它认陌生人的私聊，属主是桥自己的系统账号的就是陌生人）。

1. 先定属主：`private`、`peer` 在对应表里的，是那个本机账号；别的归系统账号：连接是系统账号的（核心拉起的、清单声明了系统账号的包的扩展，`packages.md`、`extensions.md`）归它，别的连接回 `no_system_account`（施工 O-4 下）。
2. 照「场所加属主」找：属主的会话里，`session.created` 的场所是它的，最新的那一个；删了的不算。对应表改了、属主变了的，找不到就另造：旧的留在原来的属主名下（2026-10-07 项目主人定）。
3. 找不到就造：场所照写，没人能确认（`attended` 是假：要确认的一律拒绝，工具面没有 `ask_user`），工作目录照 `cwd`，没写的、太宽的是属主的默认工作区（系统账号的是 `home/<它>/workspace/`）。2026-10-08 定的「多收可选的 `workspace`」就是这一格，不另加。
   - 归系统账号的会话：目录在它的家目录下，会话列表的索引、用量记在它名下（`usage.query` 照 `account` 分开看），本机的头的会话列表照旧不列；记忆归管理员（`personas.md`「怎么走」第 5 条）；带附件发话的，附件拷一份进它的 blob（连接上传的存在管理员名下）。
4. 同一个命令编号再发：交回上一次的那一个（和 `session.create` 一样）。

**`session.send` 的 `as`**：

```json
{"session": "…", "text": "在吗", "as": {"external": "qq:10001", "role": "member"}}
```

| 格 | 说明 |
|---|---|
| `external` | 平台身份，必写 |
| `role` | 桥报的这个人在场所里的身份：`manager`、`member`，不写的是 `member`。斜杠命令照它判谁能用（O-6） |

- 主人不靠 `role` 认：照对应表认，记在 `account`、`via` 里。`role` 没有 `owner` 这一种，以后也不加：桥报的身份只能降权。

1. 只给场所会话：本机的会话写了 `as` 是参数不对。反过来，场所会话只收带 `as` 的：不带的回 `venue_session`（「这是通讯平台的场所会话，本机的头不能直接说话」）。主人定了群会话在终端里看不到、也不能打字（`18-通讯平台.md` 第十一节、Q16），只在列表里不列，拿着编号照样发话就是一条后门。桥发的每一句都带 `as`（主人的私聊也带，`external` 是主人的号），不挡桥；订阅只读，不拦。
2. 记成谁（`message.user` 的 `by`）：
   - 私聊、这个号在对应表里、对应的账号就是会话的属主：本人，`{"kind":"person","account":"admin","via":"qq:10001"}`。
   - 别的：外部身份，`{"kind":"external","venue":<会话的场所>,"id":"qq:10001","role":"member"}`；这个号在对应表里的（群里的主人），另记 `account`：写的时候就记下，以后对应表改了，以前谁说的不跟着变（记忆照它认主人，`17-记忆.md` L16）。
3. 谁能写 `as`：O-3 里本机连接都能（本人本来什么都能做，代表外部的人只会更低）；9-4 以后扩展照清单里批准的 `act_for_external`。
4. 同一个命令编号再发只生效一次，核心重启以后照样（`04-核心协议.md` 第六节第 1 条）：桥照「平台、登录的账号、消息编号、平台给的时刻」拼编号（`chat.md` 第七条第 1 条），断线重发、平台重发都靠它。

**场所的格**（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2 条，2026-10-09 和通讯平台的会话又对过）：`session.send` 多一格可选的 `venue`，只跟着 `as` 来，原样记进 `message.user` 的 `venue`：

| 格 | 写法 | 是什么 |
|---|---|---|
| `msg` | 字，必写，1 到 128 个字符 | 平台的消息编号 |
| `reply_to` | 字，同上，可以不写 | 引用的那一条的平台编号 |
| `name` | 字，最多 64 个字符，可以不写 | 发的人此刻在这个场所里叫什么（群名片，没有的用昵称）；名字会变，每条各记各的 |
| `mentions` | 平台身份的列表，可以不写 | @ 了谁；@ 了谁的名字桥写在正文里 |
| `mentions_me`、`mentions_all` | 布尔，不写是假 | @ 了她；@ 了全体成员（不算 @ 她） |
| `media` | 列表，可以不写 | 带的东西，每项 `{kind, id, name?}`：`kind` 是 `image`、`file`、`voice`、`video`、`sticker`，`id` 平台的编号（懒下载，不进内容块），`name` 文件名、表情的字（最多 200 个字符） |
| `ambient` | 布尔，不写是假 | 旁听：只记下，不开回合，回合进行中也不排进这一轮 |
| `asleep` | 布尔，不写是假 | 睡着时收到的（桥照样带 `ambient`）；群聊近况不收它（O-13 下） |
| `show_ids` | 布尔，不写是假 | 渲染这一条时写不写发的人的平台身份（施工 O-13 中）：桥照这时的场所规则每条带上，规则改了从下一条起照新的 |

写错的、不带 `as` 的：`bad_params`，什么都不记。

**判官看的群聊记录**（施工 O-24，chat.md 第六条 `Ask.records`、`Ask.current`，形状 2026-10-09 和通讯平台的会话对过）：`venue.records {session, msg, count}`，回应 `{"records": "…", "current": "…"}`。

1. `msg` 是要判的那一条，必须是这个会话里带 `venue` 的 `message.user`；`count` 1 到 100。写错的、`msg` 不对的 `bad_params`；没有这个会话的 `session_not_found`。
2. `current` 是那一条的一行；`records` 是它之前的群里的话，一行一条（每行以换行结尾，没有的是空的），照日志的先后，从新往旧取 `count` 条：场所的 `message.user`（旁听的、开过回合的都收，睡着时收到的不收），`venue.delivered` 写成 `[you]` 行（判官看不到她的回复，主线的也收）；`msg` 以前记下的撤回照样标。
3. 写法和她看到的一行同一个函数（`miyu_assemble::group::records`），照会话快照里钉下的时区和字；快照里没有的（私聊）照核心这时的时区和出厂的字。照会话日志读（同 `view.page`，不载入会话），撤掉的、撤回的不算，压缩换出去的照样算。
4. 判官带人格（2026-10-09 项目主人定，出厂开，能按场所关）：桥照订阅回应里的 `persona` 用 `persona.read {persona, prompt: "persona"}` 读现在文件里的那一份，开关是群聊内核的出厂参数 `judge.persona`（通讯平台的会话 O-23 补）；`venue.records` 不管人格，形状不变。

**照记下的几条开一轮**（施工 O-14 上，chat.md 第七条第 3 条第 1 项，形状 2026-10-09 和通讯平台的会话对过）：`session.respond {session, to, facts}`，回应 `{"events": [...]}`（`turn.started` 和事实的序号，同 `session.send`）。

1. `to`：序号的列表，1 到 64 条，核心照序号排好、去重；`facts`：可以不写，每块 `{kind, text}`，`kind` 照事实类别的写法，`text` 最多 4 KiB，原样记成 `context.injected`，排在触发前面。写错的 `bad_params`，什么都不记。
2. 记成谁同下面的 `events.append`。同一个命令编号再发只算一次，回应和头一次一样。
3. 正在跑一轮的并进这一轮（施工 O-14 下）：事实和一条 `turn.joined {triggers}`（带回合编号）当场记下，下一步就听到，这一轮没再请求就结束的接着开一轮，打断时不退回、不接着开。`to` 里有不是这个会话里旁听的 `message.user` 的 `not_ambient`，有已经当过触发的 `already_answered`，`data.messages` 是不合的那几条。
4. 开的那一轮 `turn.started` 带 `triggers`，那几条在回合开始的地方渲染（群会话里一行一条），以后的群聊近况不再收它们。

**记几块事实**（施工 O-14 补，2026-10-09 和通讯平台的会话对过，它的退信要的：她的话发不出去，下一步知道）：`session.note {session, facts}`，回应 `{"events": [...]}`（事实的序号）。

1. `facts` 1 到 16 块，写法同上面的 `facts`，原样记成 `context.injected`。写错的 `bad_params`，什么都不记。记成谁同下面的 `events.append`；同一个命令编号再发只算一次。哪个会话都收。
2. 不开回合、不打断、不叫醒。正在跑一轮的带这一轮的回合编号，下一次请求就看到，撤销这一轮跟着撤；这一轮没再请求就结束的，下一轮开头看到。空闲的不带回合编号，下一轮开头看到，排在那一轮的触发前面。
3. 不并进 `events.append`：那条路记的一律不带回合编号，在一轮里来的退信该跟着这一轮。

**桥记的事件**（施工 O-13 上，chat.md 第七条第 3 条第 2 项）：`events.append {session, kind, body}`，回应 `{"seq": n}`。记成不带回合编号的事件，任何时候都收，不开回合、不打断；记成谁：核心拉起的扩展是那个包（模块），本机的头是管理员。

1. `ext.<包>.<名字>`：`<名字>` 一段或几段、点隔开，每段小写字母开头，只有小写字母、数字、`_`、`-`；整个种类最多 128 个字符；`body` 是 JSON 对象，序列化以后最多 16 KiB。核心拉起的扩展的连接，`<包>` 必须是它自己的包编号；本机的头都收。哪个会话都收。不渲染，撤销、压缩都不动它。
2. `venue.recalled`：`{msg, by}`，被撤的平台编号、谁撤的平台身份。
3. `venue.delivered`：`{line, turn, to, msg, text, images?}`，哪条线的会话编号、哪一轮的回合编号、回的人（平台身份的列表）、平台编号、正文、图的内容哈希。
4. 第 2、3 条只收场所会话的；格多了、少了、写法不对的，和第 1 条不合的，都 `bad_params`，什么都不记。O-13 下照它们渲染撤回的标记和 `[you]` 行。

**斜杠命令**（施工 O-6，`protocol.md` 的 `command.run`）：桥把人打的原文连同 `as` 交过来，核心认、判谁能用、执行，回执那一句也由核心照语言写好，桥原样发出去。

1. 头一批两个：`/clear`（别名 `/reset`）清空她看到的上下文；`/stop` 全停：打断这一轮，排着的话留在聊天记录里、不回、不撤回，再停掉她派出去的后台命令和子代理。终端、网页、通讯平台同一套名字。施工 9-7 下加 `/workspace <路径>`：换会话在哪个目录干活（`protocol.md` 的 `command.run` 第 5 条）。
2. 谁能用：私聊里对应表认出的本人、群里对应表里有的外部身份（带 `account` 的）、`role` 是 `manager` 的；别人 `command_not_allowed`。本机的会话本来就是本人。`/workspace` 动的是沙盒能写的地方，只有本人（私聊里认出的、群里带 `account` 的）能用，管理的人 `owner_only`。
3. 执行了的记一条 `command.ran`（原文、正名），聊天记录看得到谁在什么时候清过、停过；被拒的什么都不记。命令编号照 `session.send` 的拼法，同一个只生效一次。

**不在本机的头上**：`session.list`、`subscribe` 的 `sessions`、推送 `sessions.changed`、她的 `sessions` 工具、`history` 读别的会话，都跳过场所不是本机的会话（场所是 `local` 的才算本机的）。直接拿编号订阅、发话照常：桥就是这么用的。

### 起草时定的

- 对应表按编号的表，不是表的列表：配置系统改得少，一个号只能对一个账号（2026-10-07 项目主人定）。
- 换了属主另造：旧会话里的话是在外部的信任下说的，整个转给新属主，等于让它继承主人的权限。
- 核心不解析场所编号：是不是私聊、对方是谁，桥照实报；核心只在私聊时拿对方查对应表，群不查。`kind` 所以由桥报。
- 场所会话不收不带 `as` 的话：回专门的原因码 `venue_session`，不混在参数不对里，头看得出是哪一种拒绝（2026-10-07 通讯平台的会话提、主会话定）。
- `role` 记进 `by`：由桥担保，和平台身份同一个担保。
- 斜杠命令单开一个方法 `command.run`，不混在 `session.send` 里：头不用自己认命令，认不出的核心回 `unknown_command`，桥照原话回人（2026-10-07 项目主人定单开一步）。清空的命令叫 `/clear`、别名 `/reset`；`/stop` 是全停，排着的留在历史里不回（同一天项目主人定）。
- 群、陌生人私聊的会话归系统账号（施工 O-4 下，2026-10-09 主会话）：连接是谁照核心拉起的是哪个包认，不看头报的；没有系统账号的连接照旧 `no_system_account`。`as` 认本人照会话的属主比（原来照管理员比）：群归了系统账号以后，主人在群里说的是外部身份带 `account`，不是本人。回应带 `account`：桥照它认陌生人，不用自己再查对应表（通讯平台的会话要的）。
