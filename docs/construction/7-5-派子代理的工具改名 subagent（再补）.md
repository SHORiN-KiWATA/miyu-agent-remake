## 施工单 7-5（再补）：派子代理的工具改名 subagent

状态：已完成（2026-10-01）。

### 目的

派子代理的那件工具从 `agent` 改名 `subagent`（2026-10-01 项目主人定）。在 Miyu 里「agent」可能指她自己、子代理，以后跨会话还有别的会话，工具名叫 `subagent` 一看就知道是派子代理。留言工具 `message_agent` 随跨会话那一步改名 `send_message`，不在这一步。

### 蓝图改哪几节

- `resources/software/basesystem/tools/agent.json` 改成 `subagent.json`，说明、参数不变；`tools/agent.md` 改成 `tools/subagent.md`，所有引用跟着改（`agents.md`、`session/tools.md`、`kernel/…`、`cli/…`、`26-提示词.md` 第十节和附录、`10-自带软件.md`）。
- 工具面：新造的会话照新名字。以前造的会话快照里冻着 `agent` 这个名字（前缀不能变），它们发来的 `agent` 调用照样执行、照样认成派子代理；给人看的字 `tools` 里 `agent`、`subagent` 两个键都在，显示名一样。
- 工具名改了，这是一次计划内的缓存冷启动（只对新会话）；主会话量整张工具面的 token，登记簿、预算照新的写。
- 子代理那一段在子会话里的场所说明、别的给模型看的字里提到工具名的，照新名字改（量 token）。
- 头：`tool.call` 里派子代理的名字新会话是 `subagent`、旧会话照旧是 `agent`，合了告诉终端界面、网页两边。

### 验收

1. 测试（先写，退回改之前的代码要红）：新会话的工具面里是 `subagent`、没有 `agent`；旧快照（带 `agent`）载入以后工具面不变、`agent` 调用照样派得出子代理；给人看的两个键都换得出；请求形状探针新会话那几张脸只变了工具名（和它带来的字节），别的一字不差。
2. 真模型：新会话里让她派一个子代理，调的是 `subagent`、派得出、报得回来。
3. 手写的变异全被逮住；`CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑全绿。

### 验收结果（2026-10-01）

1. **测试**（先写，退回改之前的代码全红：`miyu-session` 的 `spawn` 9 个、`miyu-basesystem` 的 `subagent` 6 个、`miyu-core` 的 `tools` 1 个、`miyu-endpoint` 的 `orphans` 4 个）：
   - 新会话（`miyu-session/tests/spawn.rs`、`spawn/renamed.rs`）：主会话、第 1 层的工具面里是 `subagent`、没有 `agent`，第 2 层、群里两个都没有；新会话调 `agent` 照没有的工具拒掉（`There is no tool named "agent".`），一个都不派。
   - 旧会话（`spawn/renamed.rs`）：拿改名以前的目录（测试工具 `Renamed` 把 `subagent` 换回叫 `agent`，说明、参数、访问类别不动）造会话、调 `agent` 派一个，停下；换现在的目录载入，第二轮调 `agent` 照样派得出去（`Started subagent j2`），载入前后请求里的工具面逐字节一样、只有 `agent`。和同一个核心新造的会话比：旧的工具面把 `agent` 改名 `subagent`、照名字重排，和新的逐字节一样。
   - 派到一半的空子会话（`miyu-endpoint/tests/orphans.rs`）：改名以前的核心造的父会话调 `agent` 派出去、没记下 `job.started` 就崩了，换现在的核心载入，子会话照样挪进回收处。
   - 工具目录（`miyu-tool/src/catalog/tests.rs`）：照以前的名字找得到、以前的名字不进 `specs()`；以前的名字撞上前面的现在的名字、前面的以前的名字，后面现在的名字撞上前面的以前的名字，都登记不上、报撞的那个名字。
   - 给人看的字（`miyu-basesystem/tests/subagent.rs`）：`subagent`、`agent` 两个显示名中文、英文、日文里都在、一样；两种说法两种语言换得出字。
   - 请求形状探针：五张脸用的是固定的两件假工具，工具名改了不碰它；`MIYU_PROBE_WRITE=1` 重写存档，零 diff。
   - 别的测试里派子代理的调用都改成 `subagent`；`spawn_log.rs` 挑运行日志时只挑以 `subagent` 开头的行（`running` 那一行的工具名现在也是 `subagent`）。全工作区 2071 个测试全过。
2. **给模型看的字**：只有工具面变了，派子代理那件的名字 `agent` → `subagent`，说明、参数一字不改（`subagent.json` 就是改了名的 `agent.json`，指纹照旧 `67a06362`）；照名字排，它从第一件挪到 `shell` 和 `trash` 之间，tools 数组 6906 → 6909 字节。场所说明、回报、结果那两句里都没写工具名，一字不动。主会话 2026-10-01 照开发端点、`deepseek-v4.1-flash` 量（带工具减不带，边际份量是十一件减去它）：整个 tools 数组 1967 → 1968，这一件的边际份量 140 → 141，多 1 个 token；登记簿那一行照新的写。工具面的预算：说明的字节没变（6496，预算 7200 字节），十一件合计 1757 个 token，还在 1931 里，不改（`budget.rs`、`10-自带软件.md` 第九节、`tools/interface.md`）。
3. **变异**：手写 12 个，每个跑 `--workspace --lib --test spawn --test subagent --test orphans --test tools`，全被逮住：
   - `Catalog::get` 不认以前的名字；`subagent` 不报以前的名字（旧会话那一条红）。
   - 以前的名字登记成一件工具、进了工具面（核心的目录那一条、协议端点几条红）。
   - 以前的名字不查重名；查重名只看现在的名字（撞名那一条红）。
   - 工具面拿掉的写成旧名字（深度上限那一条红）。
   - 认空子会话只认新名字；`is_subagent` 只认新名字（旧父会话的空子会话那一条红）。
   - 中文少了 `agent` 的显示名；日文两个显示名不一样；英文少了 `subagent` 的显示名（显示名那一条、说法那一条、`miyu ask` 等子代理的几条红）。
   - 名字还是 `agent`（基础系统、核心的目录几条红）。
4. `CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑见提交说明。
5. 真模型（2026-10-01，主会话，开发端点的 `deepseek-v4.1-flash`，`miyu ask`）：新会话里让她派一个子代理数工作目录里的 .txt 文件，她调的是 `subagent`（日志里没有 `agent`），子代理报回 5 个，她照回报答了，`miyu ask` 等到回报才退出。

**施工中定的、和施工单对不上的**（按推荐定）：
- 旧名字怎么认：工具接口多一个 `formerly()`（以前的名字，默认没有），工具目录登记时跟着现在的名字记下、查重名，`get` 照以前的名字也找得到，`specs()` 不列它。别的办法都要在执行器、权限的两处各写一次「`agent` 就是 `subagent`」。蓝图 `tools/interface.md` 跟着补（施工单没列这一页）。
- 从日志里认派子代理的调用（派到一半的空子会话，`orphans.rs`）两个名字都算：`miyu-tool` 的 `is_subagent`。施工单没写到这一处。
- 不改名的：输出那两句的目录 `software/basesystem/agent/`、给人看的说法 `agent/started`、`agent/not-started`、效果 `job.started` 的种类 `agent`。说法和种类记在日志里，改了以前的日志就换不出字、对不上；那两句的目录和说法同名，一起不动。
- 代码的文件跟着改名：`miyu-basesystem` 的 `src/agent.rs`、`tests/agent.rs` 改成 `subagent.rs`；派子代理的端口 `AgentPort` 不改（它不是工具名）。
- 顺手：蓝图 `core.md` 列工具目录那一条写的是十件、漏了 `message_agent`，改成十一件。
- `miyu-cli` 的 `ask/follow/tests/waiting.rs` 里假的事件流调的还是 `agent`，没改：它测的是等回报，和工具名无关，正好当旧会话。
