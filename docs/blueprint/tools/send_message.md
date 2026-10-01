## `send_message`

### 是什么

发话（施工 7-7、C-5，`agents.md` 第六条、`cross-session.md` 第三条）：给自己派的、还没被停掉的子代理（`to` 写它的任务编号），给自己的父（`to: parent`），或者给不在这棵树上的别的会话（`to` 写它的会话编号）。话作为这个会话发来的话送过去，对方落了盘就返回，不等它回答：对方在跑，下一步看到；闲着，开一轮；是没人看着的一次性会话，只记下。对方的回应照留言、回报自己送回来。父子之间只在树上相邻的两层之间（2026-09-29 项目主人定）：孙代理不能越过子代理找父，兄弟之间不直接说，主会话没有父。发给别的会话要先认出是哪一个（`to` 写会话编号，和 `history` 的 `session` 同一个认法），受防刷屏管（限速、一模一样、对方没看的太多，`cross-session.md` 第五条），父子之间不受防刷屏管，只受长度上限管。

施工 C-5 从 `message_agent` 改名（照 Claude Code 的 `SendMessage`，2026-10-01 项目主人定）：以前的名字照样认得出（[`formerly`]）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/send_message.rs` | 参数、认 `to`（parent/子代理/会话编号）、长度上限、交给端口、结果和效果 |
| `crates/miyu-tool/src/messages.rs` | 发话的端口 `MessagePort`、发给谁 `Recipient`（多 `Session`）、送到了 `Delivered`（`Sent`/`Held`）、没送出去 `NotSent`，那件工具的名字 `SEND_MESSAGE`、以前的名字 `SEND_MESSAGE_FORMERLY`（`tools/interface.md`） |
| `crates/miyu-session/src/messages.rs` | 执行器这一头：照内核交的这个会话派出去的子代理认编号，经会话表的端口送过去；送到了再问一句对方是不是没人看着的一次性会话（`SessionPort::held`）；拒绝的原因码译成 `NotSent`（`session/tools.md`「父子之间留言」） |
| `crates/miyu-session/src/spawn.rs` | 会话表的端口多一个方法 `SessionPort::held`：对方这时是不是没人看着的一次性会话 |
| `crates/miyu-session/src/handle.rs`、`actor.rs`、`actor/mail.rs` | `Handle` 多 `oneshot()`、`watched()`：和 `busy()` 一样由 actor 的订阅者数算出来、共用一面旗 |
| `crates/miyu-endpoint/src/spawn.rs` | 会话表那一头 `held` 的实现：照会话表里的 `Handle` 看 |
| `resources/software/basesystem/tools/send_message.json` | 说明和参数格式 |
| `resources/software/basesystem/send_message/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句，`message_agent`、`send_message` 两个键都在 |

### 对外的样子

访问类别 `read`，和 `subagent` 一样：发话什么都不改，只读开着也发得出去；一步里给几个会话发话，连着的一起发。不用 `outbound`：那是出了 Miyu、发到通讯平台上的，要问人；这里发的都是她能看到的会话（自己的树、或者同一个属主的别的主会话）。说明照 `26-提示词.md` 附录的草稿，一字不差，「只发对方现在就得知道的」那一句留着（`agents.md` 第六条第 5 条）；不点名 `sessions`：子会话和主会话共用这一份说明，子会话里没有 `sessions`（`26-提示词.md` J4）。

样本 `resources/software/basesystem/tools/send_message.json`：

```json
{
  "description": "Send a message to a subagent you started, to your parent with `to: parent`, or to another of your sessions by its id. The other side reads it at its next step, or starts a new turn with it if idle. Send only what they need to know now, such as a question or a finding that changes their plan, since your final report goes up on its own.",
  "parameters": {"type":"object","properties":{"to":{"type":"string","description":"The job id of your subagent, such as j1, parent, or a session id."},"message":{"type":"string","description":"The message to send."}},"required":["to","message"]}
}
```

- 第一句比改名以前多了「or to another of your sessions by its id」，第二、三句一字不改。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `to` | 是 | `parent` 发给父会话；任务编号（`j1`、`j2.1` 这样）发给那个子代理；别的当会话编号认（第三条第 1 款） |
| `message` | 是 | 原样送过去，一块字，不加包装；超过 `peers.message_chars`（100000 个字）的拒，发出去之前查（`cross-session.md` 第五条第 4 款） |

- 别的参数不认，也不报错。
- 一条路径都不报：权限策略照访问类别判，读的放行。
- 本机（场所 `local`）的会话工具面里都有它，到了深度上限、没有 `subagent` 的子会话也有；场所会话（群）里没有：它派不了子代理，也没有父，也不能发给别的会话，给了只会被拒（`agents.md` 第一条第 6 条，`session/tools.md`「工具面」）。

### 怎么走

1. 读参数：读不成的（少了哪一个、不是字符串），交回参数不对的那一句，端口一次都不问。
2. 认 `to`，照这个先后（`cross-session.md` 第三条第 1 款）：
   1. `parent`：发给父会话。
   2. 合任务编号写法的（`j1`、`j2.1`）：发给自己派的子代理。
   3. 别的当会话编号认：这个会话不能发给别的会话的（子会话、场所会话，没有列会话的端口），交回「这里不能给别的会话发话」，不去找。有端口的，在这个会话看得到的主会话（加她自己）里找：一个都没有，交回「没有这个会话」；不止一个，交回「写长一点」；是她自己，交回「就是这个会话」。
3. 长度：`message` 超过 `peers.message_chars` 的，发出去之前拒，不去送（父子之间的留言也照它）。
4. 这一次调用没有发话的端口（`Call.messages` 是空的：测试里的假调用、没装会话表的核心）：交回送不到。
5. 交给端口 `send(to, message)`（执行器这一头见 `session/tools.md`「父子之间留言」）：
   - 发给父，这个会话是主会话：没有父。
   - 发给子代理，编号不在这个会话派出去的子代理里（没派过、派的是后台命令、派它的那一轮撤掉了）：不是她派的。兄弟、孙代理的编号都落在这一种：她只认得自己派的。
   - 是她派的、被停掉了（以 `stopped`、`undone` 报过）：被停掉了。做完了报过的不算停：它的会话还在，留言开它的下一轮，结束时照样回报。
   - 发给别的会话，对方因为防刷屏拒绝（限速、一模一样、对方没看的太多，只在发给别的会话时才会碰到，`cross-session.md` 第五条第 7 款）：照拒绝的原因各一句，一模一样的不算出错。
   - 会话表送不到（对方拒收、对方的会话停了、核心正在停）：送不到，原因执行器记进运行日志。
6. 送到了，再看一眼对方这时是不是没人看着的一次性会话（会话表照它的 `Handle` 看）：是，交回「存下了」；不是，交回「送到了」。两种都不算出错。发给子代理的报一样效果 `job.messaged`（`job` 是它的编号，`kernel/events-bodies.md`）：它欠一份回报，这个会话照它等，它报了才把自己的活向上报（`agents.md` 第二条第 2 条）。发给父会话、别的会话的不报：对方不欠她什么，要回话由对方自己发。
7. 不看叫停的旗：端口很快就返回。被掐掉的时候留言可能已经送到了：对方照样看到。
8. 运行日志：送到了记 `INFO message sent`，`to` 写短编号（父子之间同一行）。送不到记 `WARN message not delivered`。话的字不进运行日志。

### 样子

```text
Message sent to j1.
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 送到了 | `send_message/sent.txt` | `Message sent to {to}.` |
| 送到了，对方没人看着 | `send_message/held.txt` | `Message saved for {to}. Nobody is watching that one-shot session, so it reads this only when someone continues it.` |
| 主会话写了 `to: parent` | `send_message/no-parent.txt` | `This session has no parent.` |
| 不是她派的子代理 | `send_message/not-yours.txt` | `"{to}" is not a subagent you started. Message only your own subagents or your parent.` |
| 子代理被停掉了 | `send_message/stopped.txt` | `Subagent {to} was stopped and takes no more messages.` |
| 送不到 | `send_message/not-sent.txt` | `The message could not be delivered.` |
| 找不到会话 | `send_message/no-session.txt` | `No session has the id "{to}".` |
| 对得上不止一个会话 | `send_message/ambiguous.txt` | `"{to}" matches more than one session. Use the full id.` |
| 是她自己 | `send_message/self.txt` | `"{to}" is this session.` |
| 这个会话不能发给别的会话 | `send_message/not-here.txt` | `This session cannot message or watch other sessions.` |
| 太长 | `send_message/too-long.txt` | `The message has {chars} characters, over the limit of {limit}. Send a shorter one.` |
| 限速 | `send_message/too-many.txt` | `Too many messages to {to} just now. Put the rest into one message and send it later.` |
| 一模一样的 | `send_message/duplicate.txt` | `{to} already has this exact message.` |
| 对方没看的太多 | `send_message/inbox-full.txt` | `{to} has too many unread messages. Send again after it has read them.` |

- `{to}` 是她写的原样，照模板的规矩转义（`tools/read.md` 第 8 条）。
- 对方那一头怎么看到：子代理看到的是父会话发来的一句话，原样，不加标签（它的场所说明已经说了交代来自父会话）；父会话看到的是一块带标签的事实，注明是哪个子代理（`kernel/request.md`「子代理的留言」）；别的会话看到的是一块带标签的事实，注明是哪个会话（`cross-session.md` 第八条，`<session-message from="短编号">`）。

### 出错

出错的结果都标成出错，不报效果。`duplicate.txt`、`held.txt` 不是出错。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 没有父 | `send_message/no-parent.txt` | `send_message/no-parent` |
| 不是她派的 | `send_message/not-yours.txt` | `send_message/not-yours`，字段 `to` |
| 被停掉了 | `send_message/stopped.txt` | `send_message/stopped`，字段 `to` |
| 没有端口、送不到 | `send_message/not-sent.txt` | `send_message/not-sent` |
| 找不到、对得上不止一个、是她自己 | `send_message/no-session.txt`、`ambiguous.txt`、`self.txt` | `send_message/no-session`、`ambiguous`、`self`，字段 `to` |
| 这个会话不能发给别的会话 | `send_message/not-here.txt` | `send_message/not-here` |
| 太长 | `send_message/too-long.txt` | `send_message/too-long`，字段 `chars`、`limit` |
| 限速、对方没看的太多 | `send_message/too-many.txt`、`inbox-full.txt` | `send_message/too-many`、`inbox-full`，字段 `to` |

### 给人看的字

显示名：留言（Message），后面跟 `to` 的值；符号 `↗`；`message_agent`、`send_message` 两个键都在，显示名一样。

| 说法 | 中文 | 英文 |
|---|---|---|
| `send_message/sent`（`to`） | `送到了：{to}` | `Sent to {to}` |
| `send_message/held`（`to`） | `存下了：{to} 是没人看着的一次性会话` | `Saved: {to} is a one-shot session nobody is watching` |
| `send_message/no-parent` | 没有父会话 | No parent session |
| `send_message/not-yours`（`to`） | `{to} 不是它派的子代理` | `{to} is not its subagent` |
| `send_message/stopped`（`to`） | `{to} 已经停了` | `{to} was stopped` |
| `send_message/not-sent` | 没送到 | Not delivered |
| `send_message/no-session`（`to`） | `没有会话 {to}` | `No session {to}` |
| `send_message/ambiguous`（`to`） | `{to} 对得上不止一个会话` | `{to} matches more than one session` |
| `send_message/self`（`to`） | `{to} 就是这个会话` | `{to} is this session` |
| `send_message/not-here` | 这里不能给别的会话发话 | Cannot message other sessions here |
| `send_message/too-long`（`chars`、`limit`） | `太长了：{chars} 个字，最多 {limit}` | `Too long: {chars} characters, at most {limit}` |
| `send_message/too-many`（`to`） | `刚给 {to} 发得太多了` | `Too many messages to {to} just now` |
| `send_message/duplicate`（`to`） | `{to} 刚收到过一样的话` | `{to} already got the same message` |
| `send_message/inbox-full`（`to`） | `{to} 没看的话太多了` | `{to} has too many unread messages` |
| `common/bad-args` | 见 `tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/send_message.rs` | 只声明 `to`、`message`、访问类别是读、说明里那两句在、以前的名字认得出来；发给谁照 `to` 认、话原样交出去，几段的编号照样认（施工 7-1 补），发给子代理的报 `job.messaged`、发给父的不报；每种拒绝各一句；`held`、一模一样的不算出错；超长的发出去之前拒、刚好到上限不算超；写法不对的端口不问；没有端口的送不到；少了参数的端口不问；每种说法两种语言都换得出字 |
| `crates/miyu-basesystem/tests/send_message/by_session_id.rs` | `to` 当会话编号认：整个编号、短编号都找得到同一个会话；没有列会话的端口的不去找、直接拒；找不到、对得上不止一个、是她自己：各一句，不去送 |
| `crates/miyu-session/tests/messages.rs` | 执行器：送到对的会话、`by` 是这个会话、命令编号照调用、原话一块字、效果记进日志；到了深度上限的发给父；主会话没有父、没派过的、派它的那一轮撤掉了的、被停掉的、对方拒收、没有会话表；什么会话工具面里有它 |
| `crates/miyu-session/tests/messages_peer.rs` | 执行器发给别的会话：命令编号、`by` 照这个会话；没人看着的一次性会话交回 `held`；限速、一模一样、对方没看的太多三种拒绝译成对应的说法 |
| `crates/miyu-session/tests/messages_log.rs` | 运行日志：送到、送不到各一行，留言的字不进日志 |
| `crates/miyu-endpoint/tests/messages.rs` | 真核心走一遍三层：孙代理问、中间一层答、孙代理做完、中间一层把整件活报上去，只报一次 |
| `crates/miyu-endpoint/tests/peers.rs` | 真核心两个主会话：A 列出、发话给 B，B 被叫醒、回话，A 又被叫醒，来回几句；连续发到第 6 句被限速挡住，链自然断掉 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `agents.md` 第六条（父子之间留言）、第二条第 2 条（欠着回报的先不向上报）、第九条第 5 条（渲染）。
- `cross-session.md` 第三条（发话）、第五条（防刷屏）、第八条（上下文渲染）、第九条（谁能用）。
- `10-自带软件.md` 第三节（13 件里的 `send_message`）、第九节（工具面的预算）。
- `26-提示词.md` 附录（说明的草稿）、第十节（登记簿）。

### 还没有的

- 「空了告诉我」（`notify_when_idle`）、`message` 改成可以不写：施工 C-6。
