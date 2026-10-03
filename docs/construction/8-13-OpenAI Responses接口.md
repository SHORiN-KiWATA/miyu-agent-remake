## 施工单 8-13：OpenAI Responses 接口

状态：施工中（2026-10-03 开工；图纸 `docs/blueprint/drivers/openai-responses.md` 10-02 起草，10-03 主会话审过）。

### 目的

多一个驱动家族 `openai-responses`：OpenAI 官方、opencode Zen 上的 GPT 能直接配上用。不存对话（`store:false`，每次带全部历史），工具、图片、PDF 照这一家的写法，配了思考强度的要摘要、要加密的思考并原样回传；缓存是自动的、按前缀，不打点。请求形状探针多一张 Responses 的脸。

### 蓝图改哪几节

1. `drivers/openai-responses.md`（主会话审，2026-10-03）：
   - 状态改成审过。
   - 「解码」`response.failed` 那一行：改前「`Failure::stream(<那一段>)`」；改后「`Failure::stream(<data 里的 response 那个对象>)`」。共用的分类先找 `error` 一格，整段交进去找不到，原话会是整段 JSON。
   - 「在哪」`media.rs`：user 的字照 openai-chat 拼的那一步（`join`）挪进来共用。
   - 「要项目主人拍板的」那一题（没配强度时看不看得见思考）照 8-12 项目主人定的 A，挪进「起草时定的」第 12 条：没配就什么都不加。
   - 做完照做好的样子改写，施工时定的另记一节。
2. `models.md`：认得的驱动多 `openai-responses`；思考强度第 1 条写上这一家没有开关、`off` 只从目录的 `none` 来；档案加 `[providers.openai]`；「驱动要守的约定」第 13 条写上这一家的写法。
3. `drivers/openai-chat.md`「还没有的」删掉 Responses；`kernel/request.md` 缓存标记那一行写上这一家不用；`05-内核接口.md` 第七节加两张表。

### 不做什么

- `prompt_cache_key`、WebSocket 预热、服务端工具、`previous_response_id`、图片的官方算法（图纸「还没有的」）。
- 8-14 的 `x-opencode-*` 头和占位工具。

### 验收

1. 测试（先写）：图纸「守着它的」那张表每一行；`models` 认 `openai-responses`、它没有开关、不替它填输出上限；路由发到 `/responses`、带 `Bearer`。
2. 请求形状探针：`MIYU_PROBE_WRITE=1` 重写，`openai-chat`、`anthropic` 的存档 `git diff` 是空的，新加 `terminal/openai-responses/`；每次请求是上一次的前缀延伸，随机日志也查。
3. 给模型看的字：没有新的。
4. 手写变异 15 个左右，全被逮住；`cargo xtask check` 八项全过；三台机器的 CI 全绿。
5. 真模型实测（主会话合并前）：项目主人给的中转站（OpenAI 兼容，`/responses` 走得通），带工具的循环、思考强度、附图、打断再接着说、写错 key；`function_call_output` 带图收不收、`"tools":[]` 收不收照实记。中转站给的思考没有加密内容，加密思考的回传、缓存命中记成「待 OpenAI 官方的 key 或 Zen 上的 GPT」。

### 风险

- 和 8-12 一样动 `media.rs`：openai-chat、anthropic 的样本和探针存档逐字节比着。
- 中转站不是 OpenAI 官方：认不认 `include`、`summary`、`strict` 和官方可能不一样，照实记，不为它改写法。
