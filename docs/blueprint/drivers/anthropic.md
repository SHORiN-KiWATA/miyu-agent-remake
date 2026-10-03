## Anthropic 的消息接口

状态：2026-10-02 起草，2026-10-03 主会话审过（改了第 2 处打点的说法、思考强度第 1 条、加了「保留思考的校验」；拍板的两题都定了，见「起草时定的」第 17、18 条）。施工 8-12 做完了（2026-10-03，项目主人验收通过；真模型在 DeepSeek 的兼容接口上实测过，缓存打点命中、保留思考的校验待官方 key，见施工单）：这一页照做好的样子写，施工时定的记在「施工时定的」。

### 是什么

驱动家族 `anthropic`：把统一的请求（`kernel/request.md`）编码成 Anthropic 消息接口 `/messages` 的请求字节，带上显式的缓存打点，把回来的 SSE 流解成内核的四种增量，出了错分成几类。Anthropic 官方、opencode Zen 上的 Claude 和几个走这种写法的模型（目录里 `provider.npm` 是 `@ai-sdk/anthropic` 的）说这一种。和 `openai-chat` 一样，三样都是纯函数，同样的输入字节一样；真正发请求的是 HTTP 执行器（`http.md`）。

这一家的缓存是契约型（`08-上下文投影.md` 第六节）：写进缓存要另付钱，不打点就不缓存。所以这一页最要紧的是「缓存打点」那一节。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-drivers/src/anthropic.rs` | 家族名、路径、版本、顶层怎么写、输出上限、打点怎么接、要哪些 blob、`unmarked`（只在 `testkit`） |
| `crates/miyu-drivers/src/driver.rs` | `Anthropic` 和它的 `impl Driver`、`impl Decode` |
| `crates/miyu-drivers/src/anthropic/messages.rs` | 每条消息怎么写：块、合并相邻同角色的、工具调用和结果、思考块回传 |
| `crates/miyu-drivers/src/anthropic/marks.rs` | 缓存打点放在哪几块 |
| `crates/miyu-drivers/src/anthropic/effort.rs` | 思考强度换成 `thinking`、`output_config` |
| `crates/miyu-drivers/src/anthropic/wire.rs` | 线上的 JSON 结构 |
| `crates/miyu-drivers/src/anthropic/decode.rs` | 解码：事件、块、签名、`stop_reason`、流里的错 |
| `crates/miyu-drivers/src/anthropic/usage.rs` | 用量归成四项 |
| `crates/miyu-drivers/src/anthropic/models.rs` | 列模型：`GET /models?limit=1000` 读出模型名和窗口 |
| `crates/miyu-drivers/src/media.rs` | 图片、文件发不了时换成的字（占位、替它看的图、文本文件、带名字的图片的标签）、要哪些 blob：从 `openai_chat/messages.rs`、`openai_chat.rs` 挪出来，几个驱动共用，openai-chat 的样本一个字节不变 |
| `crates/miyu-drivers/src/sse.rs`、`classify.rs`、`texts.rs`、`text_file.rs`、`base64.rs` | 和 openai-chat 共用（`drivers/openai-chat.md`） |

每个文件不超过 500 行。

### 对外的样子

**`Anthropic`** 实现 `Driver`（`crates/miyu-drivers/src/driver.rs`），执行器照它调，和 `OpenAiChat` 一样由路由挑好端点以后造（`route/base.rs`）：

| 方法 | 做什么 |
|---|---|
| `family()` | `anthropic` |
| `blobs_needed(请求, Call)` | 同 openai-chat 第 11 条：user、tool 消息里的图片（能看图时）、每一个文件 |
| `encode(请求, Call, blob)` | 编码，交回 `Encoded`：`path` 是 `/messages`；`messages` 是 `messages` 数组里每条线上的消息的位置（system 不在里面，不算） |
| `decoder()` | 一次响应一个解码器，`feed`、`done`、`finished`、`finish` 的意思同 openai-chat |
| `classify(Failure)` | 出错分类，共用 `classify::classify`（见「出错分类」） |
| `auth(key)` | `x-api-key: <key>`、`anthropic-version: 2023-06-01`，照这个先后。没有 key 的不问它，版本头也不带 |
| `models_path()` | `/models?limit=1000` |
| `parse_models(字节)` | 读 `{"data":[{"id":…,"max_input_tokens":…},…],"has_more":…}`：`id` 是模型名，窗口是 `max_input_tokens`（不是正整数的当没报）；名字不是字、是空的跳过。`has_more` 不看：一页 1000 个，官方的模型远没有这么多。不是 JSON、没有 `data` 数组的报 `model list not readable: …` |

`Anthropic::new(DriverTexts)`：只有占位的几句，没有开关。这一家的写法只有一套，不像 openai-chat 要照供应商分。

**`Call`** 照旧（`drivers/openai-chat.md`）。`max_output` 这一家一定要写，路由替它填（「怎么走：编码」第 1 条）。

常量：`FAMILY` = `anthropic`，`PATH` = `/messages`，`VERSION` = `2023-06-01`，`FALLBACK_MAX_TOKENS` = 8192。

### 怎么走：编码

1. **顶层**，照这个先后，别的字段一概不发：`model`、`max_tokens`、`system`（第 2 条）、`tools`（第 8 条）、`messages`、`"stream":true`、思考强度（「思考强度」一节：`thinking`、`output_config`，有才写）。紧凑的 JSON，结构体照声明的先后写，参数格式原样照抄。
   - `max_tokens`：`Call.max_output`；没有的写 `FALLBACK_MAX_TOKENS`。路由替这一家填 `Call.max_output`：一次性入口写了的照它，没写的照真发的那个模型资料的最大输出（`models.md`「模型的资料」）；资料也没有的才轮到 8192。openai-chat 照旧不填。
   - 不发：`metadata`、`temperature`、`tool_choice`（默认就是 `auto`）、`stop_sequences`、顶层的 `cache_control`（自动缓存，见「缓存打点」第 6 条）、任何 `anthropic-beta` 头。
2. **system**：`[{"type":"text","text":<system>}]`，写成一块的数组，好打点。空的不发这一格。
3. **消息先合并**：统一的请求里的 user、tool 都写成线上的 `user`，assistant 写成 `assistant`。相邻两条线上的角色一样的，合成一条：后一条的块接在前一条后面。一串 tool 消息因此合成一条 user（这一家要一次调用的结果都在同一条里），tool 后面跟着的 user（上一次没等到回复，后来的话）也合进去，接在结果后面。一块都没有的消息不发（这一家不收空的 `content`），它前后两条要是同一个角色也照上面合。
4. **user** 的块，一块一块写，不拼：
   - 字：`{"type":"text","text":…}`，原样，不补换行（事实块自己带着换行）。空的一块不写：这一家不收空字。
   - 图片：`Call.inputs.images` 是真的、媒体类型是 `image/jpeg`、`image/png`、`image/gif`、`image/webp` 之一的，写 `{"type":"image","source":{"type":"base64","media_type":…,"data":…}}`；别的照 openai-chat 第 9 条换成字（占位、替它看的图的转述、带名字的占位），一段字写成一块 `text`。带名字的图片前后的标签各是一块 `text`。
   - 文件：照 openai-chat 第 9 条的先后。能读 PDF 的写 `{"type":"document","source":{"type":"base64","media_type":"application/pdf","data":…},"title":<文件名>}`；文本文件照字（开头、内容、收尾三段接成一块 `text`）；别的写占位。
   - 思考、工具调用、不认识的块不写。
5. **assistant** 的块，照原来的先后：
   - 正文：一块一块写 `text`，空的不写。
   - 思考：私有数据是这个驱动的，里面有 `signature` 的写 `{"type":"thinking","thinking":<字>,"signature":<签名>}`（字是空的也写：这一家默认不给思考的字，只给签名）；里面有 `redacted` 的写 `{"type":"redacted_thinking","data":<它>}`。别家的、没有私有数据的、只有字没有签名的（说到一半断了的）不写：发回去没签名的思考，这一家报 400。
   - 工具调用：`{"type":"tool_use","id":…,"name":…,"input":…}`。`id`：私有数据是这个驱动的、里面有字符串 `id` 的用它；没有的用内核分的 `call_<序号>_<第几个>`（别家的编号不用：可能带这一家不收的字）。`input`：参数原文是一个 JSON 对象的一个字节不改，别的写 `{}`。
   - 图片、文件、不认识的块不写。
6. **tool**：`{"type":"tool_result","tool_use_id":…,"content":[…],"is_error":true}`，`is_error` 只在统一的请求里 `error` 是真的时候写。
   - `tool_use_id` 和对应那次调用的 `id` 一样。
   - `content` 里一块一块照 user 的写法：字、图片、PDF 都放在结果里面，不挪到后面（这一家的结果收图；收 PDF 要实测确认，见「要实测确认的」）。所以 openai-chat 的 `tool-attachments.txt`、`tool-attachments-only.txt` 这一家用不上。
   - 一块都没有的，写一块 `no-output.txt` 那一句。
7. **接着写**：不会。这一家 4.6 起的模型不收结尾是 assistant 的请求（prefill），开着思考的也不收。带着 `continuation` 记号的请求照原样发，靠被打断的那一句提示（`05-内核接口.md` 第七节）。
8. **工具面**：`[{"name":…,"description":…,"input_schema":<参数格式原样>}]`，照统一的请求的先后。工具面是空的、历史里也没有工具调用的，不发 `tools`；历史里有调用的，发 `[]`（要实测确认这一家收不收）。
9. **替它看的图**：同 openai-chat 第 9 条。看不了图的，有转述、快照里有那三句标签的写成标签、转述、收尾一块 `text`；看得了图的不看 `described`。
10. **`Encoded.messages`**：合并以后每条线上的消息在 `body` 里的起止。

### 缓存打点

规则只有一条来源：前缀即契约。同一个会话里，相邻两次请求的 tools、system、已经发过的每一条消息逐字节一样，打点只告诉这一家「到这里存一份」。

1. **打几处，打在哪**，照这个先后算，同一块只打一次，最多 4 处：
   1. **工具和 system 的末尾**：system 那一块；system 是空的打在最后一件工具上；两样都没有的不打。
   2. **稳定区的末尾**：统一的请求 `stable` 大于 0 的，前 `stable` 条消息（示范对话）写出的最后一块。合并以后一条线上的消息可能装着几条统一的消息，空的消息不写块，所以照写出的块算，不照第几条线上的消息算。`stable` 是 0 的不打（和第 1 处是同一个位置）。
   3. **上一次请求的末尾**：稳定区以后最后一条 assistant 前面那一条线上消息的最后一块。稳定区以后没有 assistant 的（会话的第一次请求）不打。
   4. **这一次请求的末尾**：最后一条线上消息的最后一块。
2. **打在哪一块**：那一条消息里最后一个能打的块：`text`、`image`、`document`、`tool_use`、`tool_result`。思考块（`thinking`、`redacted_thinking`）不能打，往前找；整条都没有能打的，这一处不打。
3. **怎么写**：块的最后一格加 `"cache_control":{"type":"ephemeral"}`，5 分钟的。一份请求里打点的写法只有这一种。
4. **为什么是这四处**：第 1、2 处是会话里不变的前缀，策略不变就一直读得到；第 4 处让下一次请求从这里读；第 3 处是保险：这一家每个打点最多往回找 20 块，这一次新加的块超过 20 块（一长串事实、很多张图）时，第 4 处找不到上一次存的那一份，第 3 处正好落在上一次存的地方，照样读到。一串连着的 `tool_use`、一串连着的 `tool_result` 在这一家只算一块。
5. **打点会挪，不算改写**：第 3、4 处每次请求往后挪，上一次打过的块这一次可能没有了打点。缓存的键不含打点，前缀照样命中。查线上的前缀延伸时（请求形状探针、随机日志，`crates/miyu-assemble/tests/support`）先去掉打点再比：`anthropic::unmarked(&Encoded)` 去掉每一处 `,"cache_control":{"type":"ephemeral"}`、重算每条消息的位置，只在 `testkit` 开关打开时编进去。这一串字在 JSON 的字符串里出现不了（引号都转义了）。
6. **不用自动缓存**（顶层的 `cache_control`）：它只放一处，位置由服务端定；显式的四处我们自己定、样本里看得见。有的兼容接口不认顶层那一格。
7. **怎么让同一个会话相邻两次请求的前缀逐字节一样**：
   - 统一的请求本身只追加（内核不变量，`08-上下文投影.md` 第七节）；编码是纯函数，字段先后固定，紧凑 JSON，工具的参数格式、调用的参数原文一个字节不改。
   - 工具调用的编号用私有数据里存的原值，思考块连同签名原样回传：同样的历史出同样的字节。
   - 不发会变的东西：时间、随机数、请求编号都不进请求；`metadata.user_id` 不发。
   - 合并相邻同角色的只看统一的请求，同样的消息合出同样的字节。上一次的最后一条 user 这一次又接了几块的（上一次没等到回复），只在那一条后面接着长，前面的块不动。
   - 思考强度、`max_tokens` 照这一轮冻结的配置和模型资料，一轮里不变。`max_tokens` 不在缓存的键里，变了也不掉。
8. **前缀会在哪里断**，都是有原因的，不另记：
   - 工具面、system 变了（策略在回合开始时换，`02-内核.md` K3）：整个缓存重来。
   - 换了模型（缓存按模型分）、换了 key 而它在别的 workspace：整个重来。池钉住、key 钉在会话上（`models.md` 第一条第 6 条、第三条）就是为了少遇到它。
   - 思考强度变了（换档、开关）：对话部分重来，有的模型连工具、system 也重来。强度只在回合开始照配置变，是人自己改的，认这一次的钱（`models.md`「驱动要守的约定」第 13 条）。
   - 对话里第一次出现图片的那一次：照官方文档，图片「有没有」会让对话部分的缓存重来一次，以后照常。要实测确认。
   - Opus 4.5、Sonnet 4.6 以前的模型和 Haiku 4.5 开着思考时，新一轮人说话会让服务端剥掉上一轮的思考块，从那里往后不命中。这是服务端的行为，请求字节没变。
   - 压缩、撤销：统一的请求这一层就登记了的改写（`08-上下文投影.md` 第七节）。
   - 前缀不够长（随模型，512 到 4096 token）的不缓存，也不报错。
9. **守着它**：样本 `cache-marks*.json` 逐字节比；请求形状探针多一张 Anthropic 的脸（`docs/designs/samples/probe/terminal/anthropic/`），每一次请求去掉打点以后是上一次的前缀延伸；合并前真模型实测命中（「真模型实测」）。

### 思考强度

照 `models.md`「驱动要守的约定」第 13 条和「怎么走」第十一条。

| `Call.effort` | 写法（接在请求最后） |
|---|---|
| 没有 | 什么都不加，照供应商的默认：Opus 5 这一代默认会思考，4.7、4.8 默认不思考 |
| 档位（目录里的 `low`、`medium`、`high`、`xhigh`、`max`） | `"thinking":{"type":"adaptive","display":"summarized"}`，`"output_config":{"effort":"<档位>"}` |
| `off` | `"thinking":{"type":"disabled"}` |
| `on` | `"thinking":{"type":"adaptive","display":"summarized"}` |

1. **开关是接口自带的**：`thinking` 写 `disabled` 就是关。目录里标了开关的只有 Sonnet 5；Opus 5.5、Sonnet 5.5、Fable 收 `disabled` 报 400，它们目录里没有开关，不会多出 `off`（2026-10-03 审图时照官方文档查的）。所以这一家不用档案写 `compat.toggle`，目录有 `toggle` 的模型（例如 Sonnet 5）就多一档 `off`；只有开关、没有档位的是 `off`、`on`。这要改 `miyu_models` 认开关的地方：openai-chat 照档案的 `compat.toggle`，anthropic 自带，openai-responses 没有（「要跟着改的别的页」）。
2. **`display`**：Opus 4.7 起默认不给思考的字（`omitted`），只给签名。写了思考的就写 `summarized`，头上看得到思考的摘要。不多花钱：思考照样算钱，`display` 只管给不给看。
3. **思考预算**（`budget_tokens`）：不读、不写（`models.md`「还没有的」）。只有预算的模型（Haiku 4.5、Sonnet 4.5）目录里没有档位，请求里不带思考。
4. **思考块怎么回传**：见「编码」第 5 条。同一家的原样带签名回传，每一轮都带，不剥；别家的、没签名的丢掉。工具循环里最后那条 assistant 开着思考时一定以思考块开头，原样回传就满足。
5. **换了思考设置**：前面对话的缓存作废（「缓存打点」第 8 条）。

### 怎么走：解码

SSE 分帧共用 `sse.rs`（`drivers/openai-chat.md`「解码」）。一条事件照名字（`event`），没有名字的照 `data` 里的 `type`：

1. 已经出了错、或者见到了 `message_stop`，以后的都不理，`done()` 是真。
2. `data` 解成 JSON；解不开的照 openai-chat：流完了才冲刷出来的那一条算 `retryable`「流断在半段 JSON 上」，别的算 `bad_stream`「流里有一段不是 JSON」。
3. 照事件：

| 事件 | 怎么办 |
|---|---|
| `message_start` | `message.usage` 记下（「用量」） |
| `content_block_start` | 照 `content_block.type` 开一块，见下表；别的种类（服务端工具这些）不理，这一块以后的增量也不理 |
| `content_block_delta` | `text_delta` 的 `text`、`thinking_delta` 的 `thinking`、`input_json_delta` 的 `partial_json`：这一块的 `Text`。`signature_delta` 的 `signature`：这一块的私有数据。别的（`citations_delta`）不理 |
| `content_block_stop` | 不理：块在收尾时一起收 |
| `message_delta` | `delta.stop_reason` 不是空的记下，`finished()` 从此是真；`usage` 里有的项盖掉记下的 |
| `message_stop` | 说完了 |
| `ping` | 不理 |
| `error` | 出错，照「出错分类」分，用的是 `Failure::stream(data)` |
| 别的 | 不理 |

| `content_block.type` | 开的块 |
|---|---|
| `text` | 正文；`text` 不是空的，接着交一段 `Text` |
| `thinking` | 思考；`thinking` 不是空的接着交 `Text`；`signature` 不是空的，交私有数据 |
| `redacted_thinking` | 思考，没有字；私有数据 `{"driver":"anthropic","data":{"redacted":<data>}}` |
| `tool_use` | 工具调用 `ToolCall { name }`；私有数据 `{"driver":"anthropic","data":{"id":<id>}}`。`input` 在这里是空对象，不理，参数从 `input_json_delta` 来 |

4. 块照第一次出现的先后编号，从 0 数起，不认的块不占编号。线上的 `index` 只拿来找是哪一块。
5. 思考的私有数据是 `{"driver":"anthropic","data":{"signature":<签名>}}`，一块一份（内核的规矩，累积器一块只收一份）：签名来了才交，来两次的只认第一次（不会发生）。断在思考里的，这一块没有签名，编码时丢掉（「编码」第 5 条）。同一个线上 `index` 的 `content_block_start` 来第二次的不理。

**收尾**（`finish`）：

1. 流里记下的错优先；没有的照 `stop_reason` 判：

| 情形 | 结果 |
|---|---|
| `end_turn`、`tool_use`、`stop_sequence`、`max_tokens`、`model_context_window_exceeded` | 正常说完 |
| `refusal` | `content_policy`，原话 `stop_reason: refusal` |
| 别的（`pause_turn` 只在服务端工具时有，我们不用） | `other`，原话 `stop_reason: <它>` |
| 没有 `stop_reason`，见到了 `message_stop` | 正常说完 |
| 两样都没见到 | `retryable`，「流断了：没等到 stop_reason，也没等到 message_stop」 |

2. 正常说完的，开始了的块照编号一块块 `End`；出了错的不收。
3. 用量取记下的那一份。

**用量**，归成内核的四项（`message_start` 的 `message.usage` 先记，`message_delta` 的 `usage` 里有哪项盖哪项）：

| 项 | 取哪里 |
|---|---|
| 命中缓存 `cache_read` | `cache_read_input_tokens`，没有是 0 |
| 写进缓存 `cache_write` | `cache_creation_input_tokens`，没有是 0 |
| 没命中 `uncached` | `input_tokens`：这一家报的就是没命中的那一截，不用减 |
| 输出 `output` | `output_tokens`（含思考） |

没有 `input_tokens` 的不算用量；只认非负整数。

### 怎么走：出错分类

共用 `classify::classify`（`drivers/openai-chat.md`「出错分类」）：这一家的错误体 `{"type":"error","error":{"type":…,"message":…}}`，类型读 `error.type`，原话读 `error.message`，都对得上。超长的说法清单加一句 `exceed context limit`（老模型输入加 `max_tokens` 超了窗口时这样说），两家共用。

| 状态、类型 | 分类 | 来自共用表的哪一条 |
|---|---|---|
| 400 `invalid_request_error`，`prompt is too long: <M> tokens > <N> maximum` | `context_too_long`，交出超了多少、上限 N | 超长的说法；「超了多少」「报了的上限」已经认这种写法 |
| 400 `invalid_request_error`，`… exceed context limit: …` | `context_too_long`（超了多少、上限不解析） | 新加的那一句 |
| 400 `invalid_request_error`，`Output blocked by content filtering policy` | `content_policy` | 内容策略的说法 |
| 400 别的 | `other` | |
| 401 `authentication_error`、402 `billing_error`、403 `permission_error` | `auth` | 401、402、403 |
| 404 `not_found_error` | `other` | |
| 413 `request_too_large` | `context_too_long` | 413 |
| 429 `rate_limit_error` | `rate_limited`，要等多久照 `retry-after` | 429 |
| 500 `api_error`、529 `overloaded_error` | `retryable` | 500 起 |
| 流里的 `error` 事件（例如 `overloaded_error`） | 没有状态：大多 `retryable` | 没有状态 |

`x-should-retry` 头这一家也发，照共用表。

### 样子

**线上的请求**，样本在 `docs/designs/samples/drivers/anthropic/`，一种写法一个文件，是请求字节加一个换行：

| 样本 | 哪种写法 |
|---|---|
| `plain-text.json` | 只有文字，没有工具；`max_tokens` 照 `Call` |
| `max-tokens-fallback.json` | `Call.max_output` 没有，写 8192 |
| `cache-marks.json` | 工具、system、两条示范对话、一轮工具循环：四处打点各在哪 |
| `cache-marks-first.json` | 会话的第一次请求：没有第 3 处 |
| `cache-marks-no-system.json` | system 是空的，第 1 处打在最后一件工具上；`stable` 是 0，没有第 2 处 |
| `tool-calls.json` | 自己家的编号、内核的编号、别家的编号换成内核的、坏的参数写 `{}`、`is_error`、没有输出的占位 |
| `merged.json` | 一串 tool 合成一条 user；tool 后面跟着的 user 合进去；空消息不发 |
| `thinking.json` | 带签名的、只有签名没有字的、`redacted_thinking`、断了没签名的丢掉、别家的丢掉 |
| `media.json`、`media-omitted.json` | 图片、PDF（带 `title`）；不能收的、媒体类型不对的换成占位 |
| `tool-media.json` | 工具结果里的图、PDF 放在 `tool_result` 里面 |
| `text-files.json`、`image-names.json`、`image-descriptions.json` | 同 openai-chat 那几份，写成这一家的块 |
| `empty-tools.json` | 工具面是空的、历史里有调用：`"tools":[]` |
| `effort-level.json`、`effort-off.json`、`effort-on.json` | 思考强度三种写法，接在最后 |

探针的每一次请求编码以后的样子：`docs/designs/samples/probe/terminal/anthropic/`（模型 `claude-opus-5`，`max_tokens` 8192，没有思考强度）。

**流**，样本在同一目录的 `streams/` 下，`.sse` 进去、`.txt` 解出来：`text`、`thinking-signature`、`thinking-omitted`（字是空的、只有签名）、`redacted-thinking`、`tool-use-fragments`、`usage-in-delta`、`refusal`、`max-tokens`、`stream-error`、`ping-and-unknown`、`server-tool-ignored`、`cut-off`、`cut-in-thinking`、`bad-json`、`stop-without-message-stop`。

**给模型看的几句**：没有新的。占位、文本文件、带名字的图片、替它看的图那几句照 `DriverTexts`（`drivers/openai-chat.md`「样子」）；`tool-attachments.txt`、`tool-attachments-only.txt` 这一家不用。

### 出错

报错是中文，给查问题的人看。

| 什么时候 | 分类 | 怎么说 |
|---|---|---|
| 编码要的 blob 执行器没交进来 | `EncodeError::MissingBlob` | 同 openai-chat |
| 流里有一段解不开 | `bad_stream` | `流里有一段不是 JSON：<前 200 个字>` |
| 流完了才冲刷出来的一段解不开 | `retryable` | `流断在半段 JSON 上：<前 200 个字>` |
| 两样都没等到 | `retryable` | `流断了：没等到 stop_reason，也没等到 message_stop` |
| `stop_reason` 不对 | 见收尾那张表 | `stop_reason: <它>` |
| HTTP 出错、流里报错 | 见出错分类 | `HTTP <状态>: <原话>`，或者原话 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-drivers/tests/anthropic.rs` | 只有文字；顶层的先后；`max_tokens` 两种来源；system 写成一块；工具面三种情形；合并相邻同角色的、空消息不发；工具调用编号、参数兜底、`is_error`、没有输出的占位；每条消息的位置 |
| `crates/miyu-drivers/tests/anthropic_marks.rs` | 四处打点各在哪；第一次请求、system 空、`stable` 是 0 的；思考块不打、往前找；同一块只打一次、最多 4 处；`unmarked` 去掉以后，一段随机的会话每次请求都是上一次的前缀延伸 |
| `crates/miyu-drivers/tests/anthropic_thinking.rs` | 思考块回传：带签名、只有签名、`redacted`、断了没签名的丢掉、别家的丢掉；思考强度四种写法，接在最后，没写的一个字节不加 |
| `crates/miyu-drivers/tests/anthropic_media.rs` | 图片、PDF、媒体类型不对的占位；工具结果里的图、PDF 在 `tool_result` 里；文本文件、带名字的图片、替它看的图和 openai-chat 一样换成字；缺 blob 报错 |
| `crates/miyu-drivers/tests/anthropic_streams.rs` | 流的样本；从哪里切开喂都一样；解出来的编码回去：编号、签名原样；驱动的接口走一遍；`finished()` 在 `stop_reason` 到了以后才说是 |
| `crates/miyu-drivers/src/anthropic/models/tests.rs` | 列模型：名字和窗口、坏的跳过、回应坏了说是模型列表 |
| `crates/miyu-drivers/src/classify/tests.rs` | 加：`exceed context limit` 算超长；这一家的错误体几类各一个例子 |
| `crates/miyu-assemble/tests/probe.rs`、`random_logs.rs`、`tests/support` | 加 Anthropic 的脸：每个探针、每段随机日志的每一次请求编码以后去掉打点，是上一次的前缀延伸；主会话（`terminal`）的存档多 `anthropic/` |
| `crates/miyu-session/tests/route_anthropic.rs` | 路由照供应商的 `driver` 造驱动：发到 `/messages`、带 `x-api-key` 和版本头、不带 `Bearer`；输出上限照一次性入口写的、模型资料的、8192，openai-chat 照旧不写；思考强度照这一家的写法；回来的流照这一家解 |
| `crates/miyu-models/src/provider/tests.rs`、`facts/tests.rs` | `anthropic` 认得了；能不能关思考 openai-chat 照档案、anthropic 自带，目录有开关的模型多 `off`；要不要替它填输出上限 |
| `crates/miyu-core/src/models/tests.rs` | 出厂的档案有 `[providers.anthropic]` |

### 真模型实测

合并前主会话做，结果记进施工单：

1. **要什么**：一个 Anthropic 官方的 key（或者 opencode Zen 的 key，走 Zen 上的 Claude，收费）。这两样仓库里都没有，要项目主人给一个 key 或者端点。没有的时候能先做的：DeepSeek 的 Anthropic 兼容接口（地址写 `https://api.deepseek.com/anthropic/v1`，用现成的 DeepSeek key）测通编码、解码、工具、思考；它的缓存是自动的，不认打点，测不了第 3 条。
2. **怎么配**：`[providers.anthropic]` 写 `driver = "anthropic"`、地址 `https://api.anthropic.com/v1`、`keys = [{ secret = "anthropic" }]`，`miyu login anthropic` 存 key；`models.chat` 指一个现役的模型。临时的 `MIYU_HOME`，不碰真实数据。
3. **缓存命中**：终端里一个带工具的会话跑三轮，每轮让她读一两个文件。照 `model.called` 的用量填一张表：每次请求的没命中、命中、写入。要看到：同一轮工具循环里，后一次的命中约等于前一次的整个输入；第二、三轮的第一次请求命中约等于上一轮最后一次的整个输入；写入只是新加的那一截。命中掉成 0 的，拿两次请求的字节去掉打点比，找第一处不同。
4. **思考**：给模型配一档（例如 `low`），跑一轮工具循环：不报 400（签名原样回传了），头上看得到思考的摘要。有开关的模型（Sonnet 5）配 `off`，确认不思考。
5. **附件**：人附一张图、一个 PDF；让她用 `read` 读一张图（工具结果里的图）。
6. **打断**：回复说到一半（思考里、正文里各一次）打断，再接着说：不报 400。
7. **出错**：故意写错 key，分类是 `auth`。

### 要实测确认的

- PDF 放在 `tool_result` 里收不收（图片是收的）。不收的话改成接在同一条 user 的结果后面，改图纸再改代码。
- 工具面是空的、历史里有调用时，`"tools":[]` 收不收。
- 关着思考（`off`、或者没写强度的老模型）时，历史里带着的思考块收不收（预期是服务端不理）。
- Opus 4.5 收不收 `adaptive`（预期不收，报 400）：它的目录写着档位，配了档位就会撞上；撞上的照「起草时定的」第 7 条的退路。
- 对话里第一次出现图片的那一次，对话部分的缓存是不是重来。
- 走这种写法的别家（Zen 上的 MiniMax、Qwen，DeepSeek 的兼容接口）认不认 `display`、`adaptive`；思考块带不带签名。
- Zen 上的 Claude 认 `x-api-key` 还是 `Authorization`。
- **保留思考的校验**：Fable 5.1、Opus 5.5、Sonnet 5.5 的思考块签名绑着产生它时的前缀（system、工具面、它前面的每条消息），2026-08-31 以后建的账号默认查：发回去的思考块前面被改过的，整个请求 400 `invalid_request_error`（原话带 `bound to a different conversation`）。我们会改前缀的地方：压缩留尾巴（尾巴里的思考块绑着压缩前的历史）、回合开始换了工具面或 system。撤销只删后面的，不碰；去掉打点不算改。要官方 key、新建的账号才测得出（2026-10-03 主会话审图时查的官方文档）。

### 起草时定的

技术细节照推荐定的（2026-10-02 起草）：

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 打点四处：工具和 system 的末尾、稳定区的末尾、上一次请求的末尾、这一次的末尾，都是 5 分钟的 | 前缀契约分三段：会话里不变的、只追加的历史、这一次新加的。第 3 处保住「往回找 20 块」够不着的情形。四处正好是上限 | 只打最后一处：一次加了 20 块以上就整段重写。1 小时的：写入贵一倍，空闲续命随保温那一步（`08-上下文投影.md` 第六节） |
| 2 | 不用顶层的自动缓存 | 位置自己定、样本里看得见；它只放一处；有的兼容接口不认 | 用它：少写一个文件，换来位置看不见、兼容接口上报错 |
| 3 | 查线上前缀时去掉打点再比，`unmarked` 只在 `testkit` 里 | 打点每次往后挪，缓存的键不含它 | 不比线上前缀：等于不查这一家 |
| 4 | 相邻同角色的合成一条 | 一定合法，不靠服务端替我们合；一串工具结果在一条里也是这一家要的写法 | 照原样发，靠服务端合 |
| 5 | 工具结果里的图、PDF 放在 `tool_result` 里 | 这一家的结果收图；放在原处，她知道是哪次调用的；不用两句占位 | 照 openai-chat 挪到后面 |
| 6 | 别家的思考、没签名的思考丢掉 | 没签名的发回去报 400；写成字，她会当成自己说过的话 | 写成一块字 |
| 7 | 档位写 `adaptive` 加 `effort` 加 `display: summarized`；`on` 写 `adaptive` | 4.6 起的写法，现役模型都收；看得见思考。Opus 4.5 不收 `adaptive` 的，手写这个模型的 `reasoning` 为空就不带（`models.md`「模型的资料」：手写的盖过目录） | 照模型名分写法：代码里就有了模型表（`00-设计理念.md` 第六节） |
| 8 | 开关是这一家接口自带的，资料照它多一档 `off`，不用档案写 `toggle` | `thinking: disabled` 是这一家的标准写法，哪个供应商都一样 | 档案里照 openai-chat 写一份：每加一家走这种写法的都要写 |
| 9 | `max_tokens`：一次性入口写了的、再是模型资料的最大输出、再是 8192 | 这一家必写；资料几乎都有；8192 是现役模型都收的保守值 | 照压缩的输出预留（最多 20000）：思考就吃掉大半，回答被截 |
| 10 | 工具调用的编号只用自己家私有数据里的 | 别家的编号可能带 `.`、`:`，这一家不收 | 原样用别家的 |
| 11 | 出错分类共用一份，说法清单加一句 | 这一家的错误体和共用的读法对得上，同一件事不写两遍 | 这一家另写一份 |
| 12 | 列模型 `?limit=1000`，不翻页 | 接口不用改；官方的模型远不到 1000 个 | 给 `Driver` 加翻页 |
| 13 | user 的字一块一块发，不拼成一段 | 结构原样，打点落在块上，事实块自己带着换行 | 照 openai-chat 拼成一段 |
| 14 | 媒体类型不是那四种的图换成占位 | 发过去也报 400 | 照发 |
| 15 | 一张图算多少 token 照策略的固定数 | 官方公式随模型变（新模型分辨率更高），先不做 | 照官方公式另写一份（「还没有的」） |
| 16 | 接着写不开 | 4.6 起不收结尾是 assistant 的请求，开着思考也不收 | 给老模型开：只有不思考的老模型用得上 |
| 17 | 没配思考强度的什么都不加，照供应商的默认（2026-10-03 定，原「要项目主人拍板的」第 1 题的 A，项目主人没反对） | 照 `models.md` 定的第 12 条；想看思考的在头上选一档 | 会思考的模型没配也写 `adaptive`、`summarized`：4.7、4.8 因此默认会思考，多花钱 |
| 18 | 保留思考的校验撞上以后怎么办（2026-10-03 项目主人定）：这一步照原样回传，不加 beta 头；有官方 key 实测以后另开一步（8-12 补），这一种 400 去掉全部思考块重发一次 | 只影响 2026-08-31 以后建的账号上的三个模型；修法要动执行器，要实测出真的报错原话才写得准 | 只回传这一轮的思考块：每一轮第一次请求都要重写上一轮那段对话的缓存，所有人都付 |

### 还没有的

- 1 小时的缓存、空闲续命（保温）：随投放层那一步（`08-上下文投影.md` 第六节）。
- 一张图的官方公式（`ImagePrice`）：现在照策略的固定数。图太大（5 MB、8000 像素以上）先缩：现在照发，报 400 算 `other`。
- 1M 上下文的 beta 头：要头才给 1M 的模型（Sonnet 4.5 这类）不发头，撞一次超长以后照「用出来的」窗口算（`models.md` 第二条第 9 条）。
- 工具参数边生成边流（`eager_input_streaming`）、服务端工具、引用（citations）、文件接口（`file_id`）、思考预算。
- 接着写：见「起草时定的」第 16 条。
- 保留思考的校验撞上以后怎么办（「要实测确认的」）：这一步照原样回传；撞上的这个会话每次请求都 400，直到尾巴被下一次压缩压掉。有官方 key 实测以后另开一步：这一种 400 去掉全部思考块重发一次（官方文档给的恢复办法）。

### 施工时定的

8-12 施工时照推荐定的（2026-10-03 主会话施工时定，写进了正文）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 打点不进线上的结构体：一块照紧凑的 JSON 写好，打了点的去掉最后的 `}`，接上 `,"cache_control":{"type":"ephemeral"}}` | 每一块都是 JSON 对象；打点只有一种写法，`unmarked` 照同一串字节去掉 | 每种块多一格 `cache_control`：七种块各加一格，`unmarked` 要重新编码 |
| 第 2 处照「前 `stable` 条消息写出的最后一块」算（审图时改的）：写到第 `stable` 条时记下线上的位置 | 合并以后一条线上的消息装着几条统一的消息，前缀的边界在块上 | 照线上第几条消息的最后一块：后面合进来的块会把打点带走 |
| 第 3 处的「稳定区以后的 assistant」照线上这一条从统一的请求第几条开始算 | 示范对话里的 assistant 不算上一次请求的回复 | 照线上的位置算：合并以后数不准 |
| 能看图、媒体类型这一家不收的图，照「不能看图」写：有转述的写转述，没有的写占位 | 和 `media.rs` 一套写法；转述只有看不了图的端点才有，一般是占位 | 另写一句「这种图发不了」：要新加给模型看的字 |
| 调用的参数原文前后的空白不算：嵌进请求的是那个对象本身（`serde_json` 读原样 JSON 的读法），中间一个字节不改 | `input` 是嵌进去的 JSON，不是字符串；同样的历史出同样的字节就够了 | 原文整段照抄：前后有空白的嵌不进去 |
| 签名来两次只认第一次（图纸原写「照最后一次」） | 累积器一块只收一份私有数据，交第二次这次响应就算出错；不会发生 | 等 `content_block_stop` 再交：打断在中间的丢了签名 |
| `blobs_needed` 挪进 `media.rs` 共用，openai-chat 的那个转过去 | 两家一样：图片（能看图时）、每一个文件 | 各写一份 |

### 要跟着改的别的页

8-12 都改了：`models.md`（「施工时定的」8-12 那张表、正文几处）、`drivers/openai-chat.md`、`http.md`、`kernel/request.md`（`stable` 那一行、「还没有的」删掉缓存标记）、`05-内核接口.md` 第七节、探针；施工图合进 main 时补。下表是起草时列的：

| 页 | 改什么 |
|---|---|
| `models.md` | 认得的驱动多 `anthropic`（`miyu_models::provider::Driver`、`parse`）；「十一、思考强度」第 1 条和「模型的资料」的档位名一条：开关 openai-chat 照档案、anthropic 自带；路由给 anthropic 填 `Call.max_output`（「十二、模型调用口」第 1 条的底子）；档案加 `[providers.anthropic]`（驱动、地址）；「驱动要守的约定」第 11、13 条写上这一家的取法 |
| `drivers/openai-chat.md` | 「在哪」：换成字的那几步挪到 `media.rs`；「出错分类」第 4 条超长的说法加 `exceed context limit`（23 句）；「还没有的」删掉 Anthropic |
| `http.md` | 认证头一条写上 Anthropic 的两个头 |
| `kernel/request.md` | 缓存标记两处照这一页落成四处打点，指过来 |
| `05-内核接口.md` 第七节 | 加「Anthropic 的消息接口怎么编码、解码」两张表，照这一页 |
| `crates/miyu-assemble/tests/support` | 探针、随机日志多一张 Anthropic 的脸，去掉打点比前缀 |
| `docs/construction/施工图.html` | 8-12 那一块 |
