## `message_agent`

### 是什么

父子之间留言（施工 7-7，`agents.md` 第六条）：给自己派的、还没被停掉的子代理（`to` 写它的任务编号），或者给自己的父（`to: parent`）。留言作为这个会话发来的话送过去，对方落了盘就返回，不等它回答：对方在跑，下一步看到；闲着，开一轮。对方的回应照留言、回报自己送回来。只在树上相邻的两层之间（2026-09-29 项目主人定）：孙代理不能越过子代理找父，兄弟之间不直接说，主会话没有父。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/message_agent.rs` | 参数、认 `to`、交给端口、结果和效果 |
| `crates/miyu-tool/src/messages.rs` | 留言的端口 `MessagePort`、发给谁 `Recipient`、没送出去 `NotSent`，那件工具的名字 `MESSAGE_AGENT`（`tools/interface.md`） |
| `crates/miyu-session/src/messages.rs` | 执行器这一头：照内核交的这个会话派出去的子代理认编号，经会话表的端口送过去（`session/tools.md`「父子之间留言」） |
| `resources/software/basesystem/tools/message_agent.json` | 说明和参数格式 |
| `resources/software/basesystem/message_agent/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`，和 `agent` 一样：留言什么都不改，只读开着也发得出去；一步里给几个子代理留言，连着的一起发。不用 `outbound`：那是出了 Miyu、发到通讯平台上的，要问人；留言在她自己的这棵树里。说明照 `26-提示词.md` 附录的草稿，一字不差，「只发对方现在就得知道的」那一句留着（`agents.md` 第六条第 5 条）。

样本 `resources/software/basesystem/tools/message_agent.json`：

```json
{
  "description": "Send a message to a subagent you started, or to your parent with `to: parent`. The other side reads it at its next step, or starts a new turn with it if idle. Send only what they need to know now, such as a question or a finding that changes their plan, since your final report goes up on its own.",
  "parameters": {"type":"object","properties":{"to":{"type":"string","description":"The job id of your subagent, such as j1, or parent."},"message":{"type":"string","description":"The message to send."}},"required":["to","message"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `to` | 是 | `parent` 发给父会话；任务编号（`j1`、`j2.1` 这样）发给那个子代理。别的写法（大写、带空格、会话编号、`j0`）照「不是她派的」拒，端口不问 |
| `message` | 是 | 原样送过去，一块字，不加包装 |

- 别的参数不认，也不报错。
- 一条路径都不报：权限策略照访问类别判，读的放行。
- 本机（场所 `local`）的会话工具面里都有它，到了深度上限、没有 `agent` 的子会话也有，只能发给父；场所会话（群）里没有：它派不了子代理，也没有父，给了只会被拒（`agents.md` 第一条第 6 条，`session/tools.md`「工具面」）。

### 怎么走

1. 读参数：读不成的（少了哪一个、不是字符串），交回参数不对的那一句，端口一次都不问。
2. 认 `to`：`parent` 是发给父会话；读得成任务编号的是发给那个子代理；别的交回「不是她派的」，端口不问。
3. 这一次调用没有留言的端口（`Call.messages` 是空的：测试里的假调用、没装会话表的核心）：交回送不到。
4. 交给端口 `send(to, message)`（执行器这一头见 `session/tools.md`「父子之间留言」）：
   - 发给父，这个会话是主会话：没有父。
   - 发给子代理，编号不在这个会话派出去的子代理里（没派过、派的是后台命令、派它的那一轮撤掉了）：不是她派的。兄弟、孙代理的编号都落在这一种：她只认得自己派的。
   - 是她派的、被停掉了（以 `stopped`、`undone` 报过）：被停掉了。做完了报过的不算停：它的会话还在，留言开它的下一轮，结束时照样回报。
   - 会话表送不到（对方拒收、对方的会话停了、核心正在停）：送不到，原因执行器记进运行日志。
5. 送到了：交回送到了那一句。发给子代理的报一样效果 `job.messaged`（`job` 是它的编号，`kernel/events-bodies.md`）：它欠一份回报，这个会话照它等，它报了才把自己的活向上报（`agents.md` 第二条第 2 条）。发给父的不报：父会话不欠谁的。
6. 不看叫停的旗：端口很快就返回。被掐掉的时候留言可能已经送到了：对方照样看到。

### 样子

```text
Message sent to j1.
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 送到了 | `message_agent/sent.txt` | `Message sent to {to}.` |
| 主会话写了 `to: parent` | `message_agent/no-parent.txt` | `This session has no parent.` |
| 不是她派的子代理 | `message_agent/not-yours.txt` | `"{to}" is not a subagent you started. Message only your own subagents or your parent.` |
| 子代理被停掉了 | `message_agent/stopped.txt` | `Subagent {to} was stopped and takes no more messages.` |
| 送不到 | `message_agent/not-sent.txt` | `The message could not be delivered.` |

- `{to}` 是她写的原样，照模板的规矩转义（`tools/read.md` 第 8 条）。
- 对方那一头怎么看到：子代理看到的是父会话发来的一句话，原样，不加标签（它的场所说明已经说了交代来自父会话）；父会话看到的是一块带标签的事实，注明是哪个子代理（`kernel/request.md`「子代理的留言」）。

### 出错

出错的结果都标成出错，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 没有父 | `message_agent/no-parent.txt` | `message_agent/no-parent` |
| 不是她派的 | `message_agent/not-yours.txt` | `message_agent/not-yours`，字段 `to` |
| 被停掉了 | `message_agent/stopped.txt` | `message_agent/stopped`，字段 `to` |
| 没有端口、送不到 | `message_agent/not-sent.txt` | `message_agent/not-sent` |

### 给人看的字

显示名：留言（Message），后面跟 `to` 的值；符号 `↗`。

| 说法 | 中文 | 英文 |
|---|---|---|
| `message_agent/sent`（`to`） | `送到了：{to}` | `Sent to {to}` |
| `message_agent/no-parent` | 没有父会话 | No parent session |
| `message_agent/not-yours`（`to`） | `{to} 不是它派的子代理` | `{to} is not its subagent` |
| `message_agent/stopped`（`to`） | `{to} 已经停了` | `{to} was stopped` |
| `message_agent/not-sent` | 没送到 | Not delivered |
| `common/bad-args` | 见 `tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/message_agent.rs` | 只声明 `to`、`message`、访问类别是读、说明里那一句在；发给谁照 `to` 认、话原样交出去，几段的编号照样认（施工 7-1 补），发给子代理的报 `job.messaged`、发给父的不报；拒的每一种各一句；写法不对的端口不问；没有端口的送不到；少了参数的端口不问；五种说法两种语言都换得出字 |
| `crates/miyu-session/tests/messages.rs` | 执行器：送到对的会话、`by` 是这个会话、命令编号照调用、原话一块字、效果记进日志；到了深度上限的发给父；主会话没有父、没派过的、派它的那一轮撤掉了的、被停掉的、对方拒收、没有会话表；什么会话工具面里有它 |
| `crates/miyu-session/tests/messages_log.rs` | 运行日志：送到、送不到各一行，留言的字不进日志 |
| `crates/miyu-endpoint/tests/messages.rs` | 真核心走一遍三层：孙代理问、中间一层答、孙代理做完、中间一层把整件活报上去，只报一次 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `agents.md` 第六条（父子之间留言）、第二条第 2 条（欠着回报的先不向上报）、第九条第 5 条（渲染）。
- `10-自带软件.md` 第三节（13 件里的 `message_agent`）、第九节（工具面的预算）。
- `26-提示词.md` 附录（说明的草稿）、第十节（登记簿）。

### 还没有的

- 跨会话发消息（不在她这棵树上的）：2026-09-30 项目主人定要做，M7 做完以后单独画图纸、拆步（施工方案第三节 M7 下面）。
