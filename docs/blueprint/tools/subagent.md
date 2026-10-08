## `subagent`

### 是什么

派一个子代理去做一件事：执行器照父会话抄好属主、场所、工作目录、权限、能不能确认，照她选的池定它用哪个模型（施工 8-8 补；8-8 是挡位），造一个子会话，把交代作为父会话发来的话送进去，开它的第一轮；子会话造好、交代送到就返回编号和标题，不等它做完。它在后台跑，做完怎么回报见 `agents.md` 第二条（施工 7-6）。以前叫 `agent`：以前造的会话照旧名字调，照样派得出去（「以前的名字」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/subagent.rs` | 参数（`pool` 照端口列着的查，施工 8-8 补）、交给端口、结果和效果；以前的名字（`formerly`） |
| `crates/miyu-tool/src/agents.rs` | 派子代理的端口 `AgentPort`（`tools/interface.md`）；名字 `SUBAGENT`、以前的名字 `SUBAGENT_FORMERLY`，`is_subagent` 两个都认 |
| `crates/miyu-tool/src/catalog.rs` | 工具目录照以前的名字也找得到这一件（`tools/interface.md`「登记」） |
| `crates/miyu-session/src/agents.rs` | 执行器这一头：照父会话填好子会话，经会话表的端口造出来、送交代（`session/tools.md`「派子代理」）；造会话时拼 `pool` 那一格、照快照读回列着的池（施工 8-8 补） |
| `crates/miyu-policy/src/tools/choice.rs` | 拼 `pool` 那一格（`ToolEntry::offer`）、照快照读回（`ToolEntry::offered`），照原样的 JSON 搬，别的字节不动（施工 8-8 补） |
| `resources/software/basesystem/tools/subagent.json` | 说明和参数格式 |
| `resources/software/basesystem/agent/*.txt` | 输出里给她看的两句（目录照以前的名字，「以前的名字」） |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名（`subagent`、`agent` 两个键）、结果那一句 |
| `crates/miyu-endpoint/src/sessions/orphans.rs` | 认派到一半的空子会话：日志里两个名字的调用都算（`agents.md` 第一条第 8 条） |

### 对外的样子

访问类别 `read`：派出去这一下什么都不改，子会话照父会话抄了权限，改不改由它自己的权限管。连着的只读调用一起派，所以一步里调几次，就同时派几个（`agents.md` 第一条第 4 条）；只读开着的时候也派得出去，子会话抄着只读。说明和参数的原文如下，说明照 `26-提示词.md` 附录的草稿，「它看不到这边的对话，交代要自己说得清」那一句留着。施工 8-8 加过 `tier`，施工 8-8 补换成 `pool`（`models.md`「工具」）：说明、另两格一字不改；资源里的 `pool` 没有 `enum`，会话开局时照配置拼（「会话开局时拼 `pool`」），token 见 `26-提示词.md` 第十节。

样本 `resources/software/basesystem/tools/subagent.json`：

```json
{
  "description": "Start a subagent in a new session to do one task in the background; its report arrives as a message when it finishes. It sees nothing of this conversation, so the prompt must stand on its own: background, what is already known, the goal and what to report.",
  "parameters": {"type":"object","properties":{"description":{"type":"string","description":"A short title for the task, 3 to 5 words."},"prompt":{"type":"string","description":"The task for the subagent to perform."},"pool":{"type":"string","description":"Model pool for the task. Default: your own model."},"persona":{"type":"string","description":"Persona for the subagent. Default: no persona."}},"required":["description","prompt"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `description` | 是 | 短标题：记进 `job.started` 的 `title`，头显示用，不交给子会话 |
| `prompt` | 是 | 整段交代：原样送进子会话，不加包装 |
| `persona` | 否 | 人格的编号（施工 P-2 补，走查 C5）：只能是这个会话列着的（端口的 `personas()`，同 `pool`：开会话时照这台机器上三层里的人格填进 `enum`，进快照）。交给端口，子会话照它造；不写、写 `null` 的是软件工程师 `engineer`。写错的同 `pool`，照参数不对列出能写的几个 |
| `pool` | 否 | 池的名字，不带 `@`：只能是这个会话列着的（施工 8-8 补，端口的 `pools()`）。交给端口，子会话记 `@<池>`；不写、写 `null` 的，子会话用父会话这时用的（`models.md`「怎么走」第三条第 4 条）。这个会话一个池都没列的，参数格式里没有它 |

- 别的参数不认，也不报错。以前的会话快照里冻着 `tier`（施工 8-8 造的），她照旧写的不报错、不理它，照不写办。没有 `preset`：子会话一律照父会话的预设，写了也不理（2026-10-08 项目主人定：子代理只看得到池和人格）。

**会话开局时拼 `pool`**（施工 8-8 补，`models.md`「工具」，2026-10-01 项目主人定、技术细节主会话定）：

1. 造会话时（`Agents::face`）照那时的配置列池：`subagent` 开着、至少有一个认得出的成员的，照名字的字节序排（`miyu_models::pools::offered`）。
2. 一个都没有：参数格式里拿掉 `pool`，和施工 8-8 以前的字节一样。有的：`pool` 在 `type` 后面插 `enum`，`description` 后面每个池接一行 `\n<名字>: <说明>`，没写说明的只接 `\n<名字>`（`ToolEntry::offer`，`crates/miyu-policy/src/tools/choice.rs`）。别的格、别的参数一个字节不动。
3. 拼好的进快照，整个会话照它发，载入不重拼。列着的池造会话、载入时照快照读回一次（`ToolEntry::offered`，存在 `Agents.pools`），端口的 `pools()` 交出它。

拼出来的样子（池 `fast` 带说明 `Small model for quick lookups.`，`flagship` 没带）：

```json
"pool":{"type":"string","enum":["fast","flagship"],"description":"Model pool for the task. Default: your own model.\nfast: Small model for quick lookups.\nflagship"}
```
- 一条路径都不报：权限策略照访问类别判，读的放行（`session/guard.md`）。
- 只有能派子代理的会话工具面里有它：在本机（场所 `local`）、还没到深度上限（`jobs.depth`，`agents.md`「对外的样子」）。场所会话、到了上限的会话造会话时就拿掉它，她调了照没有这件工具拒掉（`kernel/tools.md`）。

**以前的名字**：`agent`（施工 7-5 再补改名，2026-10-01 项目主人定：在 Miyu 里「agent」可能指她自己、子代理、别的会话，叫 `subagent` 一看就知道是派子代理）。

- 新造的会话工具面上只有 `subagent`，照名字排在 `shell` 和 `trash` 之间；说明、参数格式、访问类别和以前一字不差。她调 `agent` 照没有这件工具拒掉（`kernel/tools.md`）。
- 以前造的会话，快照里冻着的工具面上是 `agent`（排在最前）：工具面是请求的前缀，照快照发，一个字节不变。她照旧调 `agent`，工具目录照以前的名字找到这一件（`Tool::formerly`、`Catalog::get`，`tools/interface.md`），照样派得出去，结果、效果和调 `subagent` 一样。
- 日志里认派子代理的调用（派到一半的空子会话，`agents.md` 第一条第 8 条）两个名字都算。
- 不改名的：效果 `job.started` 的种类照旧是 `agent`（`kernel/events-bodies.md`）；输出那两句的目录 `agent/`、给人看的说法 `agent/started`、`agent/not-started` 照旧：说法记在日志里，头照它找字，改了以前的日志就换不出字。
- 给人看的显示名：`human/{zh,en,ja}.json` 的 `tools` 里 `subagent`、`agent` 两个键都在，一样的样子：以前造的会话里调的是 `agent`，头照调用的名字找。

### 怎么走

1. 读参数：读不成的（少了哪一个、不是字符串），交回参数不对的那一句，端口一次都不问。
2. 这一次调用没有派子代理的端口（`Call.agents` 是空的：测试里的假调用，没装会话表的核心）：交回派不了。
3. 写了 `pool`、不在端口的 `pools()` 里的（大小写不对、带 `@`、这个会话一个都没列的都算）：交回参数不对的那一句，原话照 `serde` 的 `unknown variant` 列出能写的几个（例如 ``unknown variant `x`, expected `fast` or `flagship` ``；一个都没列的是 ``unknown variant `x`, there are no variants ``），端口不派。
4. 交给端口 `spawn(description, prompt, pool)`：执行器领一个任务编号，子会话记 `@<池>`（没写的抄父会话的），造子会话，把交代送进去（`session/tools.md`「派子代理」）。端口说派不了的，交回派不了；原因执行器已经记进运行日志，不给她看。
5. 派出去了：交回派出去了那一句，效果报一条 `job.started`：`job` 是端口交回的编号，`what` 是 `agent`，`title` 是 `description` 原样，`session` 是子会话的编号（`kernel/events-bodies.md`）。
6. 不看叫停的旗：端口一下就返回。被掐掉的时候子会话可能已经造好了：它照样跑，父会话的日志里没有这一次的 `job.started`（撤销、停掉随 7-8）。

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

显示名：派子代理（Subagent），后面跟 `description` 的值；符号 `↗`。`subagent`、`agent` 两个键一样（「以前的名字」）。

| 说法 | 中文 | 英文 |
|---|---|---|
| `agent/started`（`job`、`title`） | `派出去了：{job}` | `Started {job}` |
| `agent/not-started` | 派不出去 | Could not start |
| `common/bad-args` | 见 `tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/subagent.rs` | 声明标题、交代和池（资源里的 `pool` 没有 `enum`，施工 8-8 补）、访问类别是读、说明里那一句在；列着的池交给端口、不写和 `null` 交没有、不在列表里的照参数不对端口不派、原话照 `serde` 列出能写的几个、`tier` 不理；交给端口的原样，交回的字和 `job.started`；没有端口、端口派不了的交回派不了、不报效果；少了参数的端口不问；两种说法两种语言都换得出字；`subagent`、`agent` 两个显示名三种语言里都在、一样 |
| `crates/miyu-session/tests/spawn.rs` | 执行器交给会话表的子会话抄对了每一样、交代记成父会话发的；一步里调两次派两个、各领各的编号；领了没派成的不回收、载入以后接着数；没有会话表的派不了；什么会话工具面里有 `subagent`、新会话里没有 `agent`（`session/tools.md`） |
| `crates/miyu-session/tests/spawn/pool.rs` | 子会话的模型：写了 `pool` 的记 `@池`，不写的、写 `tier` 的抄父会话的；工具面照造会话时的配置拼、配置改了这个会话不变（连同载入以后）、新会话变，老会话里写新会话才列的池照参数不对（施工 8-8 补，取代 8-8 的 `tier.rs`） |
| `crates/miyu-policy/src/tools/choice/tests.rs` | 拼 `pool`、读回列着的（施工 8-8 补） |
| `crates/miyu-session/tests/spawn/renamed.rs` | 以前的名字：新会话调 `agent` 照没有的工具拒掉；拿改名以前的目录造的会话换现在的核心载入，工具面一个字节不变、调 `agent` 照样派得出去；两张工具面只差名字和它带来的先后 |
| `crates/miyu-endpoint/tests/orphans.rs` | 改名以前造的父会话，派到一半的空子会话照样收掉 |
| `crates/miyu-tool/src/catalog/tests.rs` | 目录照以前的名字找得到、以前的名字不进工具面、撞名的登记不上 |
| `crates/miyu-endpoint/tests/spawn.rs` | 真核心走一遍：子会话的日志、快照、请求，`session.list` 的 `parent` |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `agents.md` 第一条（派子代理）、「对外的样子」（`jobs.depth`、效果 `job.started`）。
- `10-自带软件.md` 第三节、第九节（工具面的预算）、第十节（名字不照 Claude Code 的 `Agent`）。
- `26-提示词.md` 附录（说明的草稿）、第十节（登记簿）。

### 还没有的

- `persona` 施工 P-2 补加了；`preset` 不加（2026-10-08 项目主人定）。`tier` 施工 8-8 加了，施工 8-8 补换成 `pool`。
