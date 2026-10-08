## 提供者：扩展登记工具、核心反向调用

### 是什么

核心自带的工具编译在核心里（`tools/`）。通讯平台的桥要给她「发群消息」「撤回」这类只有它做得到的工具，以后头也会有（终端集成）。提供者经协议的 `provide` 把自己的工具登记进核心的工具目录；她调到这件工具时，核心反向调用 `tool.call` 交给那个连接去跑，等它回结果（`05-内核接口.md` 第四节「内核空间模块与外部扩展」、第六节「工具的规格」、第八节「启用、注册与目录快照」，`04-核心协议.md` 第九节「方法一览」的「提供者」「反向调用」）。

状态：图纸，施工 O-2 上起（2026-10-09 主会话写；O-2 由主会话拆成上、下）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-endpoint/src/wire.rs` | 读进来的一行多一种：对核心发出去的请求的回应 |
| `crates/miyu-endpoint/src/reverse.rs` | 反向调用：核心发给一个连接的请求（编号 `core-<n>`），等它的回应；连接断了，在等的都了结（施工 O-2 上） |
| `crates/miyu-endpoint/src/provide.rs` | `provide`：查规格、登记进目录；提供者表：哪个包现在由哪个连接提供（施工 O-2 上） |
| `crates/miyu-endpoint/src/provide/remote.rs` | 提供者的一件工具（`Tool` 的实现）：执行时经提供者表找到那个连接、反向调用 `tool.call`（施工 O-2 上） |
| `crates/miyu-tool/src/catalog.rs` | 目录多一样：换掉一个包的工具，交回新的目录（施工 O-2 上） |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展多一步 `serve`：登记以后答 `tool.call`（施工 O-2 上） |

### 对外的样子

**`provide {tools}`**（施工 O-2 上）：

1. 只收核心拉起的扩展的连接（`hello` 时认出是哪个包的）；别的连接回 `not_a_provider`。头扮演提供者随 O-2（下）。
2. `tools` 是这个包现在提供的全部工具，每件 `{name, description, input_schema, access, venues}`：
   - `name` 只用英文字母、数字、`_`、`-`，1 到 64 个字符；
   - `input_schema` 是 `{"type":"object",…}`；
   - `access` 是 `read`、`write`、`execute`、`network`、`outbound` 之一；
   - `venues` 是给哪种会话：`local`（本机的）、`private`（通讯平台的私聊）、`group`（群）里的一个或几个，不能是空的（05 第六节的 `venues`；2026-10-09 和通讯平台的会话定：「发给任意好友或群」只给本机，`skip_reply` 只给场所）。
3. 登记进工具目录，归这个包：换掉它上一次登记的那几件。名字撞上核心自带的、别的包的、写法不对的：整个不收，`bad_tool`，`data` 是 `{"tool": 名字, "problem": "duplicate" | "name" | "parameters" | "access" | "venues"}`。回应 `{"tools": 件数}`。
4. 记下这个包现在由这个连接提供。连接断了，工具照旧留在目录里（`05-内核接口.md` 第九节：工具从目录里消失会改变请求字节），被调到时回「暂时不可用」。

**造会话时的工具面**：照这时的目录，提供者的工具照它的 `venues` 挑这个会话是哪种（本机的、私聊、群）。已经造好的会话照它快照里的工具面，不跟着变（O-2 下：包的工具变了，开着的会话下一个回合换上，记 `session.policy_changed`，照换人格的办法）。提供者的工具归它的包，预设照包开关，和核心自带的一样。

**给模型看的字**：出厂的包（随 Miyu 装的，通讯平台的桥）的工具说明放在资源目录 `software/<包>/tools/<名字>.json`，和核心自带的一个写法，扩展启动时读它们再 `provide`；登记进 `26-提示词.md` 第十节，工具说明的总预算算进去（2026-10-09 和通讯平台的会话定）。第三方的扩展不在仓库里，不管。

**`tool.call`**（反向调用，核心发给提供者，施工 O-2 上）：`{"jsonrpc":"2.0","id":"core-<n>","method":"tool.call","params":{…}}`，`params`：

| 格 | 是什么 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数，修正过的 JSON 对象 |
| `cwd` | 这一轮的工作目录 |

回应 `{"result": {"blocks": [内容块…], "error": 布尔}}`：内容块照内核的写法（`text`、`image`），`error` 不写是假。回应是 JSON-RPC 的错误、写法不对的：算这次调用出错，原话写进结果。

### 怎么走

1. **读回应**：读进来的一行有 `result` 或 `error`、没有 `method` 的，是回应：照 `id` 找到核心发出去的那一条，交给等它的；找不到的不理。
2. **发反向调用**：每个连接一张「发出去还没回」的表；编号照连接从 `core-1` 数起。连接断了，表里在等的都了结成「连接断了」。
3. **执行提供者的工具**：执行器照目录找到这件（`RemoteTool`）；它照包查提供者表：没有连接的，交回「暂时不可用」（`core/tool-results/unavailable.txt`）；有的，发 `tool.call`、等回应，叫停时丢掉等待（O-2 下发 `tool.cancel`）。
4. **目录换代**：核心的目录可以换：`provide` 交回新的一份，以后造的会话、载入的会话照新的。

### 出错

| 什么时候 | 原因码 | 说明 |
|---|---|---|
| 不是核心拉起的扩展 | `not_a_provider` | 头扮演提供者随 O-2（下） |
| 规格不对、撞名 | `bad_tool` | `data.tool`、`data.problem` |
| 参数写错 | `bad_params` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-endpoint/src/wire/tests.rs` | 对核心发出去的请求的回应认得出来：`result`、`error` 原样交出，`id` 要是字符串、要是 2.0 |
| `crates/miyu-endpoint/src/reverse/tests.rs` | 反向调用发出去的一行带 `core-<n>`、方法、参数；对上编号的回应交给等它的，对不上的不理；连接断了，在等的和以后发的都了结 |
| `crates/miyu-endpoint/src/provide/tests.rs` | 访问类别、给哪种会话不认识的、空的拒；提供者表照包记，重新登记的换掉旧的；没有连接的、发不出去的暂时不可用，说法同执行器的 |
| `crates/miyu-tool/src/catalog/tests.rs` 的 `replacing_a_package_keeps_the_others_and_checks_the_new_ones` | 换掉一个包的工具：别的包的照留，原来那份不动，撞名、写法不对的整个不收 |
| `crates/miyu-session/src/agents/tests.rs` | 工具面照会话在哪挑提供者的工具：本机的、私聊、群 |
| `crates/miyu-endpoint/tests/provide.rs` | 契约：扩展登记、撞名的整个不收、头不是提供者；新造的会话工具面里有给本机的、没有只给群的；`tool.call` 带会话、调用编号、参数；结果、错误、写法不对的各自交回；扩展停了，暂时不可用 |

### 还没有的

- O-2（下）：包的工具变了，开着的会话下一个回合换上；每件工具的超时、到点发 `tool.cancel`、叫停也发；执行中的进度 `tool.progress`；`tool.call` 带上 `by`（这一轮触发的那一条的 `by` 原样）和 `owner`（核心认的是不是主人）；登记缓存在磁盘上，核心起来就有、新造的会话照记住的上一次登记带上；头扮演提供者；提供者登记命令、挂接点。
