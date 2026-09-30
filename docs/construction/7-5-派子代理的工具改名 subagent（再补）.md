## 施工单 7-5（再补）：派子代理的工具改名 subagent

状态：施工中。

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
