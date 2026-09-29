## `agent`

### 是什么

派一个子代理去做一件事：执行器照父会话抄好属主、场所、工作目录、权限、能不能确认，造一个子会话，把交代作为父会话发来的话送进去，开它的第一轮；子会话造好、交代送到就返回编号和标题，不等它做完。它在后台跑，做完怎么回报见 `agents.md` 第二条（施工 7-6）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/agent.rs` | 参数、交给端口、结果和效果 |
| `crates/miyu-tool/src/agents.rs` | 派子代理的端口 `AgentPort`（`tools/interface.md`） |
| `crates/miyu-session/src/agents.rs` | 执行器这一头：照父会话填好子会话，经会话表的端口造出来、送交代（`session/tools.md`「派子代理」） |
| `resources/software/basesystem/tools/agent.json` | 说明和参数格式 |
| `resources/software/basesystem/agent/*.txt` | 输出里给她看的两句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：派出去这一下什么都不改，子会话照父会话抄了权限，改不改由它自己的权限管。连着的只读调用一起派，所以一步里调几次，就同时派几个（`agents.md` 第一条第 4 条）；只读开着的时候也派得出去，子会话抄着只读。说明和参数的原文如下，说明照 `26-提示词.md` 附录的草稿，「它看不到这边的对话，交代要自己说得清」那一句留着。

样本 `resources/software/basesystem/tools/agent.json`：

```json
{
  "description": "Start a subagent in a new session to do one task in the background; its report arrives as a message when it finishes. It sees nothing of this conversation, so the prompt must stand on its own: background, what is already known, the goal and what to report.",
  "parameters": {"type":"object","properties":{"description":{"type":"string","description":"A short title for the task, 3 to 5 words."},"prompt":{"type":"string","description":"The task for the subagent to perform."}},"required":["description","prompt"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `description` | 是 | 短标题：记进 `job.started` 的 `title`，头显示用，不交给子会话 |
| `prompt` | 是 | 整段交代：原样送进子会话，不加包装 |

- 别的参数不认，也不报错。挡位、人格、预设三个参数随配置和预设那一步（「还没有的」）。
- 一条路径都不报：权限策略照访问类别判，读的放行（`session/guard.md`）。
- 只有能派子代理的会话工具面里有它：在本机（场所 `local`）、还没到深度上限（`jobs.depth`，`agents.md`「对外的样子」）。场所会话、到了上限的会话造会话时就拿掉它，她调了照没有这件工具拒掉（`kernel/tools.md`）。

### 怎么走

1. 读参数：读不成的（少了哪一个、不是字符串），交回参数不对的那一句，端口一次都不问。
2. 这一次调用没有派子代理的端口（`Call.agents` 是空的：测试里的假调用，没装会话表的核心）：交回派不了。
3. 交给端口 `spawn(description, prompt)`：执行器领一个任务编号，造子会话，把交代送进去（`session/tools.md`「派子代理」）。端口说派不了的，交回派不了；原因执行器已经记进运行日志，不给她看。
4. 派出去了：交回派出去了那一句，效果报一条 `job.started`：`job` 是端口交回的编号，`what` 是 `agent`，`title` 是 `description` 原样，`session` 是子会话的编号（`kernel/events-bodies.md`）。
5. 不看叫停的旗：端口一下就返回。被掐掉的时候子会话可能已经造好了：它照样跑，父会话的日志里没有这一次的 `job.started`（撤销、停掉随 7-8）。

### 样子

```text
Started subagent j1: "查导出".
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 派出去了 | `agent/started.txt` | `Started subagent {job}: "{title}".` |
| 派不了 | `agent/not-started.txt` | `The subagent could not be started.` |

- 换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）：标题里的引号、换行写不进这一行。

### 出错

出错的结果都标成出错，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 没有端口、端口说派不了 | `agent/not-started.txt` | `agent/not-started` |

### 给人看的字

显示名：派子代理（Subagent），后面跟 `description` 的值；符号 `↗`。

| 说法 | 中文 | 英文 |
|---|---|---|
| `agent/started`（`job`、`title`） | `派出去了：{job}` | `Started {job}` |
| `agent/not-started` | 派不出去 | Could not start |
| `common/bad-args` | 见 `tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/agent.rs` | 只声明标题和交代、访问类别是读、说明里那一句在；交给端口的原样，交回的字和 `job.started`；没有端口、端口派不了的交回派不了、不报效果；少了参数的端口不问；两种说法两种语言都换得出字 |
| `crates/miyu-session/tests/spawn.rs` | 执行器交给会话表的子会话抄对了每一样、交代记成父会话发的；一步里调两次派两个、各领各的编号；领了没派成的不回收、载入以后接着数；没有会话表的派不了；什么会话工具面里有 `agent`（`session/tools.md`） |
| `crates/miyu-endpoint/tests/spawn.rs` | 真核心走一遍：子会话的日志、快照、请求，`session.list` 的 `parent` |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `agents.md` 第一条（派子代理）、「对外的样子」（`jobs.depth`、效果 `job.started`）。
- `10-自带软件.md` 第三节、第九节（工具面的预算）。
- `26-提示词.md` 附录（说明的草稿）、第十节（登记簿）。

### 还没有的

- `tier`、`persona`、`preset` 三个参数：随配置和预设那一步，加的时候工具面变一次（`15-模型与供应商.md`、`16-人格与预设.md`）。
