## OpenAI 的 Responses 接口

状态：图纸，2026-10-02 起草，2026-10-03 主会话审过（`response.failed` 交给出错分类的是哪一段、拼字的 `join` 挪进 `media.rs` 共用、拍板的那一题照 8-12 定了 A）。施工 8-13 做完了（2026-10-03，项目主人验收通过；真模型在项目主人给的中转站上实测过，加密思考的回传、官方的缓存命中待官方 key，见施工单）：这一页照做好的样子写，施工时定的记在「施工时定的」。

### 是什么

驱动家族 `openai-responses`：把统一的请求（`kernel/request.md`）编码成 OpenAI Responses 接口 `/responses` 的请求字节，把回来的 SSE 流解成内核的四种增量，出了错分成几类。OpenAI 官方、opencode Zen 上的 GPT（目录里 `provider.npm` 是 `@ai-sdk/openai` 的）说这一种。三样都是纯函数，同样的输入字节一样；真正发请求的是 HTTP 执行器（`http.md`）。

这一家不存对话：每次请求带全部历史、`"store":false`，不用 `previous_response_id`。对话的真相只在我们的日志里，前缀照我们自己的规矩稳（`08-上下文投影.md` 第一节）。缓存是自动的、按前缀，不用打点。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-drivers/src/openai_responses.rs` | 家族名、路径、顶层怎么写、要哪些 blob |
| `crates/miyu-drivers/src/driver.rs` | `OpenAiResponses` 和它的 `impl Driver`、`impl Decode` |
| `crates/miyu-drivers/src/openai_responses/input.rs` | 统一的请求里的消息写成 `input` 里的一项项 |
| `crates/miyu-drivers/src/openai_responses/effort.rs` | 思考强度换成 `reasoning`、`include` |
| `crates/miyu-drivers/src/openai_responses/wire.rs` | 线上的 JSON 结构、工具面 |
| `crates/miyu-drivers/src/openai_responses/decode.rs` | 解码：事件、每一项开一块、加密的思考、收尾 |
| `crates/miyu-drivers/src/openai_responses/usage.rs` | 用量归成四项 |
| `crates/miyu-drivers/src/media.rs` | 图片、文件发不了时换成的字、要哪些 blob，三个驱动共用（`drivers/anthropic.md`「在哪」）；8-13 起 user 的字照 openai-chat 拼的那一步（`join`）也挪进来，openai-chat 的样本一个字节不变 |
| `crates/miyu-drivers/src/sse.rs`、`classify.rs`、`texts.rs`、`text_file.rs`、`base64.rs` | 和 openai-chat 共用 |
| `crates/miyu-drivers/src/openai_chat/models.rs` | 列模型：OpenAI 的 `/models` 写法一样，共用 `parse_models` |

每个文件不超过 500 行。

### 对外的样子

**`OpenAiResponses`** 实现 `Driver`，和 `OpenAiChat` 一样由路由挑好端点以后造：

| 方法 | 做什么 |
|---|---|
| `family()` | `openai-responses` |
| `blobs_needed(请求, Call)` | 同 openai-chat 第 11 条 |
| `encode(请求, Call, blob)` | 编码，交回 `Encoded`：`path` 是 `/responses`；`messages` 是 `input` 里每一项的位置，照先后（system 写在 `instructions`，不算） |
| `decoder()` | 一次响应一个解码器 |
| `classify(Failure)` | 共用 `classify::classify` |
| `auth(key)` | `Authorization: Bearer <key>` |
| `models_path()` | `/models` |
| `parse_models(字节)` | 同 openai-chat（`openai_chat::parse_models`）：官方的列表不报窗口，窗口照目录 |

`OpenAiResponses::new(DriverTexts)`：没有开关。

**`Call`** 照旧。`max_output` 有才写，路由照旧不替它填。

常量：`FAMILY` = `openai-responses`，`PATH` = `/responses`。

### 怎么走：编码

1. **顶层**，照这个先后，别的字段一概不发：`model`、`instructions`（第 2 条）、`input`、`tools`（第 7 条）、`"store":false`、`"stream":true`、`max_output_tokens`（`Call.max_output` 有才写）、思考强度（「思考强度」：`reasoning`、`include`，有才写）。紧凑的 JSON，结构体照声明的先后写，参数格式原样照抄。
   - 不发：`previous_response_id`、`tool_choice`、`parallel_tool_calls`（默认就是能并行）、`truncation`（默认不截，超长报错，交给压缩）、`prompt_cache_key`、`user`、`metadata`、`text`、`service_tier`。
2. **system**：写进 `instructions`，一整段；空的不发这一格。
3. **user**：`{"role":"user","content":…}`。
   - 全是文字的，`content` 是一个字符串，照 openai-chat 的拼法（相邻两块之间补一个换行，前一块已经以换行结尾的不补）。
   - 有能发的图片、文件的，`content` 是几段：`{"type":"input_text","text":…}`、`{"type":"input_image","image_url":<data URL>}`、`{"type":"input_file","filename":…,"file_data":<data URL>}`；连着的字照上面拼成一段。图片、文件怎么挑、发不了的换成什么字、替它看的图、带名字的图片的标签、文本文件，全照 openai-chat 第 9 条。
   - 思考、工具调用、不认识的块不写。一个字都没有的，`content` 是空串。
4. **assistant**：照块的先后，写成几项，相邻的同种块合成一项：
   - 正文：连着的几块直接接上，写一项 `{"role":"assistant","content":<字>}`。空的不写。
   - 思考：私有数据是这个驱动的、里面有 `encrypted_content` 的，写 `{"type":"reasoning","id":<id>,"summary":[{"type":"summary_text","text":<字>}],"encrypted_content":<它>}`，字是空的写 `"summary":[]`。一块思考一项，不合。别家的、没有加密内容的不写：没有它，这一家认不出这一项（`store` 是假的，服务端没存）。
   - 工具调用：一次一项 `{"type":"function_call","call_id":…,"name":…,"arguments":…}`，不写 `id`（服务端没存，写了会去找）。`call_id`：私有数据是这个驱动的、里面有字符串 `call_id` 的用它，没有的用内核分的 `call_<序号>_<第几个>`。`arguments` 是一个字符串：参数原文是一个 JSON 对象的照原文，别的写 `{}`。
5. **tool**：`{"type":"function_call_output","call_id":…,"output":…}`。
   - `call_id` 和对应那次调用的一样。
   - 全是文字的，`output` 是一个字符串，照 user 的拼法；一个字都没有的写 `no-output.txt` 那一句。
   - 有能发的图片、PDF 的，`output` 是几段，和 user 的 `content` 一样写（要实测确认，见「要实测确认的」）：放在结果里面，不挪到后面，openai-chat 的那两句占位不用。
   - 统一的请求里的 `error` 不发：这一家的结果没有出错的标记，出错写在内容里。
6. **接着写**：不会。这一家没有前缀续写。带记号的照原样发，靠提示。
7. **工具面**：`[{"type":"function","name":…,"description":…,"parameters":<原样>,"strict":false}]`，照统一的请求的先后。`strict` 一定写假的：这一家的 function 默认是严格的，严格模式要求每个参数都必填、不许多出字段，我们的参数格式不是这样写的。工具面是空的、历史里也没有工具调用的，不发 `tools`；历史里有调用的，发 `[]`。
8. **`Encoded.messages`**：`input` 里每一项在 `body` 里的起止，照先后。

### 缓存

这一家照前缀自动缓存，不用打点：前缀至少 1024 token 才缓存，按 128 token 一段往上算（OpenAI 的说法）。前缀稳不稳全靠我们的字节：

1. 统一的请求只追加，编码是纯函数，字段先后固定，参数原样。
2. 工具调用的 `call_id`、加密的思考原样回传：同样的历史出同样的字节。
3. 不发会变的东西：时间、随机数、`user`、`metadata` 都不发；`store`、`stream` 每次一样。
4. `instructions` 在 `input` 前面：system 不变，它就一直在前缀里。
5. 断在哪：工具面、system 变了，换了模型，换了 key 而它在别的组织。历史里存着的加密思考不管这一次强度是什么都照样回传，所以换强度不改历史的字节；服务端会不会因为换了强度不认前缀，要实测确认。
6. 守着它：请求形状探针多一张 Responses 的脸（`docs/designs/samples/probe/terminal/openai-responses/`），每次请求是上一次的前缀延伸，照 openai-chat 那一套直接比（没有打点要去掉）。

`prompt_cache_key` 不发（「还没有的」）。

### 思考强度

照 `models.md`「驱动要守的约定」第 13 条和「怎么走」第十一条。

| `Call.effort` | 写法（接在请求最后） |
|---|---|
| 没有 | 什么都不加，照供应商的默认 |
| 档位（目录里的 `minimal`、`low`、`medium`、`high`、`xhigh`、`max`） | `"reasoning":{"effort":"<档位>","summary":"auto"}`，`"include":["reasoning.encrypted_content"]` |
| `off`（目录写 `none` 的那一档读成它） | `"reasoning":{"effort":"none"}` |
| `on` | 不会有：这一家没有开关，目录的 `toggle` 不算。万一来了，什么都不加 |

1. **`summary`**：写了档位的要思考的摘要，头上看得到她在想什么。
2. **加密的思考**：`store` 是假的，服务端不存思考；要回传就得在 `include` 里要加密的那一份，解码时存进私有数据，下一次原样带回去（「编码」第 4 条）。工具循环里回传思考，官方说答得更好。
3. **为什么只在有档位时写**：没写强度的时候不知道这个模型会不会思考，`reasoning` 发给不会思考的模型（`gpt-4.1` 这类）会报错。所以没写强度的，思考不回传、也看不到摘要（「起草时定的」第 12 条）。

### 怎么走：解码

SSE 分帧共用 `sse.rs`。这一家没有 `[DONE]`：说完是 `response.completed` 这类事件。一条事件照名字，没有名字的照 `data` 里的 `type`：

1. 已经出了错、或者见到了收尾的事件（`response.completed`、`response.incomplete`、`response.failed`），以后的都不理，`done()` 是真；`finished()` 也是这时候变真。
2. `data` 解成 JSON；解不开的照 openai-chat（`retryable` 或 `bad_stream`）。
3. 一项开一块，照项第一次出现的先后编号，从 0 数起；线上的 `output_index`、`item_id` 只拿来找是哪一块。不认的项（服务端工具这些）不占编号，它的增量不理。

| 事件 | 怎么办 |
|---|---|
| `response.output_item.added` | `item.type` 是 `message` 开正文块；`reasoning` 开思考块；`function_call` 开工具调用块 `ToolCall { name }`，私有数据 `{"driver":"openai-responses","data":{"call_id":<它>}}` |
| `response.output_text.delta`、`response.refusal.delta` | 正文块的 `Text`（拒答的字也是她说的话，照正文给人看） |
| `response.reasoning_summary_text.delta`、`response.reasoning_text.delta` | 思考块的 `Text`。一项里第二段摘要开始时（`summary_index` 变了），先交一个空行 `\n\n` 再接着 |
| `response.function_call_arguments.delta` | 攒着，不当场交（「施工时定的」）；`response.function_call_arguments.done` 的 `arguments` 来了换成它 |
| `response.output_item.done` | 思考项：有 `encrypted_content` 的交私有数据 `{"driver":"openai-responses","data":{"id":<id>,"encrypted_content":<它>}}`；摘要一个字都没流过来、`item.summary` 里有的，照它补上。正文项：一个字都没流过来的（有的网关只在这里给整段），照 `item` 里的补上。工具调用项：参数照 `item.arguments` 交一整段，没有的照攒着的；没等到这一项完了就说完的，收尾时照攒着的交 |
| `response.completed` | 用量记下，说完了 |
| `response.incomplete` | 用量记下；`response.incomplete_details.reason` 记下 |
| `response.failed` | 出错：照「出错分类」分，用的是 `Failure::stream(<data 里的 response 那个对象>)`：共用的读法先找 `error` 一格，`response` 对象里正好有（整段 `data` 交进去找不到，原话会是整段 JSON） |
| `error` | 出错，照「出错分类」分，用的是 `Failure::stream(data)` |
| 别的（`response.created`、`…in_progress`、`…content_part.*`、`…done` 这些） | 不理 |

4. 思考块没有字也没有加密内容的，内核不留（`accumulate.rs`）；有加密内容、没有字的留着，下一次照样回传。

**收尾**（`finish`）：

| 情形 | 结果 |
|---|---|
| 流里记下的错 | 照它 |
| `response.completed` | 正常说完 |
| `response.incomplete`，原因 `max_output_tokens` | 正常说完（和 openai-chat 的 `length` 一样） |
| `response.incomplete`，原因 `content_filter` | `content_policy`，原话 `incomplete: content_filter` |
| `response.incomplete`，别的原因 | `other`，原话 `incomplete: <它>` |
| 没见到收尾的事件 | `retryable`，「流断了：没等到 response.completed」 |

正常说完的，开始了的块照编号一块块 `End`；出了错的不收。

**用量**，取收尾那个事件的 `response.usage`：

| 项 | 取哪里 |
|---|---|
| 命中缓存 `cache_read` | `input_tokens_details.cached_tokens`，没有是 0 |
| 写进缓存 `cache_write` | 0：这一家不报写入，也不另收写入的钱 |
| 没命中 `uncached` | `input_tokens` 减去命中，最少 0 |
| 输出 `output` | `output_tokens`（含思考，`output_tokens_details.reasoning_tokens` 不另算） |

没有 `input_tokens` 的不算用量；只认非负整数。

### 怎么走：出错分类

共用 `classify::classify`，和 openai-chat 一样：错误体 `{"error":{"message":…,"type":…,"code":…}}`。流里的 `error` 事件、`response.failed` 的 `response.error` 是 `{"code":…,"message":…}`，共用的读法先看 `error` 再看顶层，也对得上。

| 状态、错误码 | 分类 |
|---|---|
| 400，`context_length_exceeded`，或者「Your input exceeds the context window」 | `context_too_long` |
| 400，`content_filter`、`content_policy_violation` 这些 | `content_policy` |
| 400 别的 | `other` |
| 401、403 | `auth` |
| 429 `insufficient_quota` | `auth`（额度用完） |
| 429 别的 | `rate_limited`，要等多久照头、照 `try again in …` |
| 500 起 | `retryable` |
| 流里的错、`response.failed`（没有状态），例如 `server_error`、`rate_limit_exceeded` | 大多 `retryable` |

### 样子

**线上的请求**，样本在 `docs/designs/samples/drivers/openai-responses/`，一种写法一个文件，是请求字节加一个换行：

| 样本 | 哪种写法 |
|---|---|
| `plain-text.json` | 只有文字，`instructions`，不发 `tools` |
| `tool-calls.json` | 自己家的 `call_id`、内核的编号、坏的参数写 `{}`、没有输出的占位、`strict:false` |
| `reasoning.json` | 加密的思考回传、空摘要写 `[]`、别家的和没有加密内容的不写、正文和调用的先后 |
| `media.json`、`media-omitted.json` | `input_image`、`input_file`；不能收的换成占位 |
| `tool-media.json` | 工具结果里的图、PDF 放在 `output` 里 |
| `text-files.json`、`image-names.json`、`image-descriptions.json` | 同 openai-chat 那几份 |
| `empty-tools.json` | 工具面是空的、历史里有调用：`"tools":[]` |
| `effort-level.json`、`effort-off.json` | 思考强度两种写法，接在最后 |
| `max-output.json` | `max_output_tokens` |

探针的每一次请求编码以后的样子：`docs/designs/samples/probe/terminal/openai-responses/`（模型 `gpt-5.4`，没有输出上限、没有思考强度）。

**流**，样本在同一目录的 `streams/` 下：`text`、`reasoning-summary`（两段摘要、加密内容）、`reasoning-encrypted-only`、`function-calls`（两次并行）、`done-only`（只在 `output_item.done` 给整段）、`refusal`、`incomplete-max-tokens`、`incomplete-content-filter`、`failed`、`error-event`、`unknown-item`、`cut-off`、`bad-json`、`no-completed`（没等到收尾的事件）、`arguments-dropped`（增量丢了字，整段是对的）。

**给模型看的几句**：没有新的，照 `DriverTexts`；`tool-attachments.txt`、`tool-attachments-only.txt` 这一家不用。

### 出错

| 什么时候 | 分类 | 怎么说 |
|---|---|---|
| 编码要的 blob 执行器没交进来 | `EncodeError::MissingBlob` | 同 openai-chat |
| 流里有一段解不开 | `bad_stream` | `流里有一段不是 JSON：<前 200 个字>` |
| 流完了才冲刷出来的一段解不开 | `retryable` | `流断在半段 JSON 上：<前 200 个字>` |
| 没等到收尾的事件 | `retryable` | `流断了：没等到 response.completed` |
| 说完了可是不完整 | 见收尾那张表 | `incomplete: <原因>` |
| HTTP 出错、流里报错 | 见出错分类 | `HTTP <状态>: <原话>`，或者原话 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-drivers/tests/openai_responses.rs` | 只有文字；顶层的先后；`instructions` 空的不发；工具面三种情形、`strict:false`；工具调用的编号、参数兜底、没有输出的占位；assistant 的几项照块的先后；每一项的位置 |
| `crates/miyu-drivers/tests/openai_responses_reasoning.rs` | 加密的思考回传、空摘要、别家的和没有加密内容的不写；思考强度两种写法接在最后，没写的一个字节不加 |
| `crates/miyu-drivers/tests/openai_responses_media.rs` | 图片、PDF、不能收的占位；工具结果里的图、PDF；文本文件、带名字的图片、替它看的图和 openai-chat 一样；缺 blob 报错 |
| `crates/miyu-drivers/tests/openai_responses_streams.rs` | 流的样本；从哪里切开喂都一样；解出来的编码回去：`call_id`、加密内容原样；驱动的接口走一遍；`finished()` 在收尾事件以后才说是 |
| `crates/miyu-assemble/tests/probe.rs`、`random_logs.rs`、`tests/support` | 加 Responses 的脸：每个探针、每段随机日志的每一次请求编码以后是上一次的前缀延伸；主会话（`terminal`）的存档多 `openai-responses/` |
| `crates/miyu-session/tests/route_responses.rs` | 路由照供应商的 `driver` 造驱动：发到 `/responses`、带 `Bearer`；输出上限不替它填、写了的照它；思考强度照这一家的写法；回来的流照这一家解 |
| `crates/miyu-models/src/provider/tests.rs`、`facts/tests.rs` | `openai-responses` 认得了、没有开关，目录有开关的模型也不多 `off` |
| `crates/miyu-core/src/models/tests.rs` | 出厂的档案有 `[providers.openai]` |

### 真模型实测

合并前主会话做，结果记进施工单：

1. **要什么**：一个 OpenAI 官方的 key，或者 opencode Zen 的 key（Zen 上的 GPT 走这个驱动；它的免费模型里也有走这种写法的，免费名单常变，到时候照目录挑）。仓库里都没有，要项目主人给一个 key 或者端点。走 Zen 的要等 8-14 的头和占位工具，或者先在配置里手写固定的 `x-opencode-*` 头、在带 `shell`、`read` 的终端会话里测。
2. **怎么配**：`[providers.openai]` 写 `driver = "openai-responses"`、地址 `https://api.openai.com/v1`、`keys = [{ secret = "openai" }]`，`miyu login openai`；`models.chat` 指一个现役的会思考的模型。临时的 `MIYU_HOME`。
3. **缓存命中**：带工具的会话跑三轮（system 加工具面够 1024 token），照 `model.called` 的用量填表：后一次请求的命中约等于前一次的输入（按 128 取整）。命中掉了的，拿两次请求的字节比，找第一处不同。
4. **思考**：配一档（例如 `low`），跑一轮工具循环：不报 400（加密的思考原样回传了，没有「找不到这一项」的错），头上看得到摘要；配 `off`（目录写 `none` 的模型），确认不思考。
5. **附件**：人附一张图、一个 PDF；让她用 `read` 读一张图。
6. **打断、出错**：说到一半打断再接着说；故意写错 key，分类是 `auth`。

### 要实测确认的

- `function_call_output` 的 `output` 写成几段（带图、PDF）收不收。不收的话改成照 openai-chat 挪到后面的一条 user，改图纸再改代码。
- 回传的 `reasoning` 项要不要 `id`、带了 `id` 在 `store:false` 时会不会去找（我们定的是带 `id` 和加密内容）；`function_call` 不带 `id` 行不行。
- assistant 的字写成一个字符串收不收（预期收）。
- 没通过组织验证的账号要 `summary` 会不会报错。会的话 `summary` 做成档案里的一格，改图纸。
- 工具面是空的、历史里有调用时 `"tools":[]` 收不收。
- 这一家的流真的没有 `[DONE]`；Zen 上的 Responses 认不认 `include`、`store:false`。
- 换了思考强度，服务端的前缀缓存掉不掉（「缓存」第 5 条）。

### 起草时定的

技术细节照推荐定的（2026-10-02 起草）：

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 不存对话：`store:false`、每次带全部历史、不用 `previous_response_id` | 对话的真相只在我们的日志里；撤销、压缩、换端点都照我们自己的；前缀照我们的规矩稳 | 用服务端存的对话：一换端点、一撤销就对不上 |
| 2 | system 写进 `instructions` | 这一家的标准写法，永远在最前面 | 写成第一项 `developer` 消息 |
| 3 | `strict` 一定写假的 | 这一家默认严格，我们的参数格式有可选参数，严格了要么报错、要么逼她每个参数都填 | 不写：靠默认，默认会变 |
| 4 | 工具结果里的图、PDF 放在 `output` 里 | 放在原处，她知道是哪次调用的 | 照 openai-chat 挪到后面 |
| 5 | 思考只在配了档位时要摘要、要加密内容、回传 | 不知道会不会思考的模型发 `reasoning` 会报错；`Call` 不多一格 | `Call` 加「会思考」一格，没配也回传（第 12 条） |
| 6 | 回传的思考带 `id` 和加密内容，工具调用不带 `id` | 思考项的 `id` 在接口的格式里是必写的，有加密内容就不用服务端存；调用的 `id` 可写可不写，写了服务端会去找 | 都不带 `id`、都带 `id` |
| 7 | 一项开一块；一项里几段摘要用空行接成一块 | 和 openai-chat 一样一块一段字；回传时一项一块对得上 | 一段摘要一块：回传时拆不回一项 |
| 8 | 工具调用的编号只用自己家私有数据里的 `call_id` | 同 Anthropic：别家的编号写法不一定收 | 原样用别家的 |
| 9 | 出错分类共用一份 | 错误体和 openai-chat 一样 | 另写 |
| 10 | `prompt_cache_key` 不发 | 一台机器一个人用，按前缀分流就够；它要会话编号，`Call` 里没有 | 照会话编号发：要 `Call` 多一格 |
| 11 | 一张图算多少 token 照策略的固定数 | 官方公式随模型变，先不做 | 照官方公式 |
| 12 | 没配思考强度的什么都不加：不要摘要、不要加密内容、不回传（2026-10-03 主会话照 8-12 项目主人定的 A 定，原「要项目主人拍板的」第 1 题） | 和 Anthropic 一个规矩，`models.md` 定的第 12 条；想看思考的在头上选一档 | 会思考的模型没配也要摘要、回传：`Call` 多一格，三个驱动都改 |

### 还没有的

- `prompt_cache_key`、连接预热（Responses 的 WebSocket，`15-模型与供应商.md` 第五节）、服务端工具、`previous_response_id`。
- 一张图的官方公式（`ImagePrice`）：现在照策略的固定数。
- 列模型的窗口：官方的列表不报，照目录。

### 施工时定的

8-13 施工时照推荐定的（2026-10-03 主会话施工时定，写进了正文）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| `response.output_item.done` 来了、`added` 没来过的那一项，照 `done` 里的 `item` 开一块再补 | 只在 `done` 给整段的网关可能连 `added` 都不发；照样开块不丢话 | 没开的不理：那一项的话就丢了 |
| 思考的第二段摘要先接一个空行，只在前面已经流过字时接 | 第一段的 `summary_index` 不一定是 0；一项开头不多一个空行 | 照 `summary_index` 大于 0 就接 |
| `response.incomplete` 没写原因的，原话 `incomplete: unknown`，分类 `other` | 不该发生；照不认的原因算 | 当正常说完 |
| `response.failed` 交给出错分类的是 `response` 那个对象（审图时改的） | 共用的分类先找 `error` 一格 | 整段 `data`：原话是整段 JSON |
| 流里的错没有状态，限速的说法也照可重试算、要等多久照原话（共用的分类） | 和 openai-chat 流里的错一样 | 这一家另算 |
| 工具调用的参数不照增量交，攒到这一项完了照整段交（`output_item.done` 的 `arguments`，再是 `function_call_arguments.done` 的，再是攒的增量） | 2026-10-03 实测项目主人给的中转站：增量丢了开头的 `{"`，整段是对的，她连调了十几次都参数不对；没收全的调用本来就执行不了，晚一点交不丢东西（样本 `arguments-dropped`） | 照增量交：中转站坏了她就一直调不成 |
| 中转站给的思考摘要没有加密内容：留着字给人看，不回传 | 没有加密内容，`store` 是假的，这一家认不出这一项 | 写成一块字回传：她会当成自己说过的话 |

### 要跟着改的别的页

8-13 都改了：`models.md`（「施工时定的」8-13 那张表、正文几处）、`drivers/openai-chat.md`、`kernel/request.md`（`stable` 那一行）、`05-内核接口.md` 第七节、探针；施工图合进 main 时补。下表是起草时列的：


| 页 | 改什么 |
|---|---|
| `models.md` | 认得的驱动多 `openai-responses`（`miyu_models::provider::Driver`、`parse`）；「十一、思考强度」第 1 条：这一家没有开关，`off` 只从目录的 `none` 来；档案加 `[providers.openai]`（驱动、地址）；「驱动要守的约定」第 13 条写上这一家的写法 |
| `drivers/openai-chat.md` | 「还没有的」删掉 Responses |
| `kernel/request.md` | 缓存标记两处这一家不用，指过来 |
| `05-内核接口.md` 第七节 | 加「Responses 接口怎么编码、解码」两张表，照这一页 |
| `crates/miyu-assemble/tests/support` | 探针、随机日志多一张 Responses 的脸 |
| `docs/construction/施工图.html` | 8-13 那一块 |
