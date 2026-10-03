## 施工单 8-14：opencode Zen

状态：待验收（2026-10-03 开工、施工完；图纸是 `drivers/openai-chat.md`「接 opencode Zen」和 `models.md`「八、opencode Zen」，10-02 起草，10-03 主会话审过、照实测收窄）。

### 目的

opencode 的 Zen（`/zen/v1`）和 Console Go（`/zen/go/v1`）只写 key 就能用：同一家里的模型各走各的驱动（GPT 走 Responses、Claude 和一部分别家的走 Anthropic、别的走 OpenAI 兼容），交错思考的模型照目录的 `interleaved` 把思考带回去，Go 的请求带它要的 `x-opencode-session`。

### 实测（2026-10-03 主会话，先摸底再收窄）

- Zen 的免费模型：带不带 `x-opencode-*` 头、补不补 `read`、`shell` 都回 403，原话 `OpenCode's free tier can only be used from within OpenCode`；User-Agent 写成 `opencode/1.3.0` 回 426，要求 1.18.0 以上。免费档现在照 User-Agent 认客户端，只给 OpenCode 自己用。
- Console Go：缺 `x-opencode-session` 回 400（`MissingSessionID`）。官方文档（`opencode.ai/docs/go`「Where can I use it」）写明 Go 给第三方编码 agent 用，要求用自己的 User-Agent、每段对话发一个稳定的 `x-opencode-session`。带上以后成了，User-Agent 照旧是 `miyu/<版本>`。
- Go 上的 `deepseek-v4.1-flash`：工具循环里 assistant 不带 `reasoning_content`、带空串、带字，都不报 400。

### 蓝图改哪几节

1. 两页图纸的状态改成审过，照实测收窄（2026-10-03 主会话；不冒充 OpenCode 的客户端，2026-10-03 项目主人同意先不做免费档）：
   - 「八、opencode Zen」：改前「免费档：头四个 + 占位工具」；改后「Go 要的 `x-opencode-session` 一个头」。`x-opencode-client`、`-project`、`-request` 和占位工具不做：它们只为过免费档的客户端检查，现在也过不了。实测原话记进去。
   - 头的值：只认 `{session_digest}`（种子的 SHA-256 写成十六进制的前 26 位）。种子是会话编号；一次性调用是用途；`provider.test` 是固定的 `provider.test`。
   - 「接 opencode Zen」第 4 条「为什么」：改前「不回传会 400（要实测确认）」；改后「Go 上实测不 400；回传是为了工具循环里接上思路」。
   - 模型的 `driver` 这一格：这一步只照目录（第 1、2 层对上的模型的 `provider.npm`），先后是「手写的供应商 `driver` > 目录对上的模型的 `npm` > 档案 > 目录里那一家的 `npm`」；手写的模型 `driver` 配置项不做，`model.list` 的 `facts` 不多这一格。能不能关思考、要不要替它填输出上限照模型的驱动算。
2. `models.md`：第一条第 2 条（驱动按模型）、第 4 条（另配的头只来自档案，值里只认 `{session_digest}`）、「模型的资料」`driver` 一行和多 `interleaved` 一行、第八条照做好的样子写；档案那一段加 `[providers.opencode-go]`。
3. `drivers/openai-chat.md`「接 opencode Zen」照做好的样子写；`http.md` 另配的头一条。

### 不做什么

- Zen 免费档：要冒充 OpenCode 的客户端，不做。
- 配置里手写的 `headers`、`compat`、模型的 `driver`：随用到它的那一步。
- 占位工具：不做（见上）。

### 验收

1. 测试（先写）：档案读 `headers`，值里不认识的字段读不进来；`{session_digest}` 怎么算、同一个种子同一个值；同一家的模型照目录走三种驱动、没有驱动的模型 `no_model`、手写的供应商驱动压过目录；`interleaved` 第 1、2 层取、第 3、4 层不取、档案写了 `reasoning` 的照档案、不走 openai-chat 的不管；路由发出去的请求带头、走对路径、照模型的驱动填输出上限。
2. 请求形状探针：别的存档 `git diff` 是空的（编码没变）。
3. 给模型看的字：没有新的。
4. 手写变异 15 个左右，全被逮住；`cargo xtask check` 八项全过；三台机器的 CI 全绿。
5. 真模型实测（主会话合并前，项目主人给的 Go key，模型 `deepseek-v4.1-flash`）：只写 key、走档案和目录，带工具的会话跑三轮，思考照 `reasoning_content` 回传；走 `anthropic`、`openai-responses` 的 Go 模型各问一句（各一两次，用的是项目主人的订阅额度）。

### 风险

- 改了路由造驱动的地方：会话、一次性入口、`provider.test` 都走一遍。
- key 是项目主人在对话里给的：只放在本机临时文件，不进仓库、日志、提交、施工单。

### 施工结果（2026-10-03 主会话）

1. 测试先写：`miyu-models` 九条（`headers/tests.rs` 三条；`profile`、`catalog`、`facts/tests/wire.rs` 各一条；`provider` 三条），`miyu-session/tests/route_zen.rs` 四条，`miyu-endpoint/tests/providers_test.rs` 一条（`provider.test` 照模型的驱动发、带头、没有驱动的是 `config`），`miyu-core` 出厂档案一条。裁出来的目录多三个真条目（Go 的 `minimax-m3`、`gpt-5.6-luna`，Zen 的 `gemini-3-pro`）。
2. 请求形状探针：存档没动，`git diff` 是空的。
3. 给模型看的字：没有新的。
4. 手写变异 21 个，全被逮住：摘要的位数、模板全放行、不换、读档案不查；目录丢包名、`interleaved` 写成 `true` 也认；第 3 层也取；能不能关思考照供应商；手写驱动不管用、一律当手写；换不出驱动退回供应商的；`always` 写假、档案写了也盖、别的驱动也回传、字段写反；挑候选不挂头、种子写死；路由照供应商造驱动、照供应商填上限；`provider.test` 照供应商的驱动发、不带头。
5. `cargo xtask check`：格式、clippy、文档、分层、纯逻辑、行数、许可证过；测试除容器里 root 跑的 `config_set` `a_write_that_fails_changes_nothing`（和这一步无关，CI 上过）全过（`--no-fail-fast` 跑全）。
6. 真模型实测（项目主人给的 Go key，只放在本机临时文件，照环境变量取；数据根是临时目录；配置只写 `[providers.opencode-go] keys`）：
   - `deepseek-v4.1-flash`（`openai-chat`，思考照 `reasoning_content` 回传）：带工具的会话三轮，读文件、接着算，六次请求全成，第二、三轮缓存命中 95%、93%（回传的思考没打乱前缀）。
   - `minimax-m3`（`anthropic`，`/messages`）、`gpt-5.6-luna`（`openai-responses`，`/responses`）各问一句，都答对。
   - 数据根、仓库里找不到 key；运行日志里没有头的值。
