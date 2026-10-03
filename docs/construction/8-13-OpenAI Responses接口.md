## 施工单 8-13：OpenAI Responses 接口

状态：已完成（2026-10-03 施工，项目主人验收通过；图纸 `docs/blueprint/drivers/openai-responses.md` 10-02 起草，10-03 主会话审过）。

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

### 验收结果

- 测试（先写；新的类型、函数改之前编译不过）：
  - `miyu-drivers`：`tests/openai_responses.rs`（8 个）、`openai_responses_reasoning.rs`（4 个）、`openai_responses_media.rs`（5 个）、`openai_responses_streams.rs`（8 个，15 份流的样本、从每个字节切开喂都一样）。样本 13 份在 `docs/designs/samples/drivers/openai-responses/`。
  - `miyu-models`：`provider/tests.rs`、`facts/tests.rs` 加了 `openai-responses`（认得、没有开关、目录有开关的模型也不多 `off`）；`miyu-session`：`tests/route_responses.rs`（2 个）；`miyu-core`：出厂档案那一条多 `openai`。
  - 三种驱动都有了，拿 `openai-responses` 当「还没有的驱动」的几处测试改了：用不了的供应商写成不带驱动和地址（`UNUSABLE_MODEL`），还没有的驱动写在档案里（`google`）。
- 请求形状探针：`MIYU_PROBE_WRITE=1` 重写，`openai-chat`、`anthropic` 的存档 `git diff` 是空的；新加 `terminal/openai-responses/`（22 份）。每个探针、随机日志的每一次请求编码成 Responses 都是上一次的前缀延伸。
- 实测抓到一个问题，当场修了：项目主人给的中转站流过来的工具参数增量丢了开头的 `{"`，`output_item.done` 的整段是对的，她连调了十几次 `read` 都参数不对。改成参数攒到这一项完了照整段交（图纸「施工时定的」），样本 `arguments-dropped` 复现它，修之前是红的。
- 手写变异 19 个，全逮住（两个第一轮没逮住，补了测试再逮住）：不写 `store`；system 写进 `input`；`strict` 写真；正文不合并；别家的思考也回传（补：别家的数据长得和自己家一样）；空摘要也写一段；用别家的编号（补：别家的私有数据写法一样）；没输出不写占位；档位不要加密内容；`off` 不写；摘要不隔空行；参数照增量交；不认加密内容；`max_output_tokens` 不完整当出错；用量不减命中；`response.failed` 交整段；只在 `done` 给的不补；responses 能关思考；结果里的图挪走。
- `cargo xtask check`：格式、clippy、文档、分层、纯逻辑、行数、许可证都过；测试只有 `config_set` 的 `a_write_that_fails_changes_nothing` 不过（容器里是 root），和这一步无关。
- 新依赖：没有。给模型看的字：没有新的。

**主会话合并前真模型实测**（2026-10-03，项目主人给的中转站，`/responses`，模型 `glm-5.3-flash`，临时数据根，驱动 `openai-responses`，地址和 key 都照环境变量取）：

| 项 | 结果 |
|---|---|
| 带工具的循环 | 修了参数以后，读两个文件、加起来，两轮接着说都成 |
| 思考强度 | `high`：收，带着 `include` 不报错；`reasoning: {effort: none}`：收，不思考（直接发请求试的） |
| 思考摘要 | 中转站给摘要的字，没有加密内容：头上看得到，不回传 |
| 附图 | 人附的 PNG、`read` 读出来的 PNG（在 `function_call_output` 里）都看得到 |
| 打断 | 说到一半打断再接着说，不报错 |
| 写错 key | HTTP 401，`auth` |
| 历史里有调用、`"tools":[]` | 收（直接发请求试的） |
| 缓存 | 中转站自己按前缀缓存，同一轮里命中九成以上；新一轮第一次请求有时掉成 0，探针证明我们的字节是前缀延伸，是中转站那边的 |
| 加密思考的回传、PDF、官方的缓存命中、换强度后缓存掉不掉 | 测不了：**待 OpenAI 官方的 key，或者 Zen 上的 GPT** |

key 只在环境变量里：数据根、日志、仓库里查过都没有。
