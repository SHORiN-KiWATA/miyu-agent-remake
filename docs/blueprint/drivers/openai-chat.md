## OpenAI 兼容的对话接口

### 是什么

驱动家族 `openai-chat`：把统一的请求（`kernel/request.md`）编码成 OpenAI 兼容接口 `/chat/completions` 的请求字节，把回来的 SSE 流解成内核的四种增量，出了错分成几类。DeepSeek、智谱、OpenRouter、opencode Zen、本机的 Ollama、LM Studio 都说这一种。三样都是纯函数，同样的输入字节一样；真正发请求的是 HTTP 执行器（`http.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-drivers/src/lib.rs` | `Call`、`Inputs`、`BlobBytes`、`Encoded`、`EncodeError`、`Ending` |
| `crates/miyu-drivers/src/driver.rs` | 驱动的接口 `Driver`、`Decode`；`OpenAiChat`（`Anthropic` 也在这里，`drivers/anthropic.md`） |
| `crates/miyu-drivers/src/openai_chat.rs` | 家族名、路径、`Compat` 的开关、顶层怎么写、要哪些 blob |
| `crates/miyu-drivers/src/openai_chat/messages.rs` | 每条消息怎么写、附件挪到后面、接着写 |
| `crates/miyu-drivers/src/media.rs` | 图片、文件发不了时换成的字（占位、替它看的图、文本文件、带名字的图片的标签）和要哪些 blob：施工 8-12 从 `openai_chat/messages.rs`、`openai_chat.rs` 挪出来，和 `anthropic` 共用；施工 8-13 把拼字（`join`）也挪进来，和 `openai-responses` 共用。这一页的样本一个字节没变 |
| `crates/miyu-drivers/src/openai_chat/wire.rs` | 线上的 JSON 结构、工具面 |
| `crates/miyu-drivers/src/openai_chat/decode.rs` | 解码：块、工具调用、`finish_reason`、流里的错 |
| `crates/miyu-drivers/src/openai_chat/usage.rs` | 各家的用量归成四项 |
| `crates/miyu-drivers/src/openai_chat/models.rs` | 列模型：`GET /models` 的回应读出模型名和报了的窗口（施工 8-7） |
| `crates/miyu-drivers/src/sse.rs` | SSE 分帧 |
| `crates/miyu-drivers/src/classify.rs` | 出错分类、要等多久、原话 |
| `crates/miyu-drivers/src/texts.rs` | 给模型看的几句：五句占位，文本文件的三句（施工 3-9 三补），带名字的图片的三句（施工 3-9 四补），替它看的图的三句标签（施工 8-17） |
| `crates/miyu-drivers/src/text_file.rs` | 什么算文本文件、最多给多少（施工 3-9 三补） |
| `crates/miyu-drivers/src/base64.rs` | data URL 用的 base64 |
| `resources/core/drivers/` | 那几句的原文 |

### 对外的样子

**`Driver`**，执行器照它调，一个会话造一个，能跨线程：

| 方法 | 做什么 |
|---|---|
| `family()` | `openai-chat`：私有数据里写的是它的，才归它用 |
| `blobs_needed(请求, Call)` | 编码要用的 blob 清单，执行器照着先取 |
| `encode(请求, Call, blob)` | 编码，交回 `Encoded` |
| `decoder()` | 一次响应一个解码器 `Decode`：`feed(字节) -> 增量`、`done()`、`finished()`（`finish_reason` 到了没有，施工 4-9 再补三下）、`finish() -> Ending` |
| `classify(Failure)` | 出错分类，交回 `Classified` |
| `auth(key)` | 带 key 的请求要带的认证头（施工 8-6，`models.md`「驱动要守的约定」第 2 条）：`Authorization: Bearer <key>`。HTTP 执行器照它写，不自己写；没有 key 的请求不问它 |
| `models_path()` | 列模型发到地址后面的哪一截（施工 8-7，「驱动要守的约定」第 3 条）：`/models` |
| `parse_models(字节)` | 读列模型的回应 `{"data":[{"id":…},…]}`，交回 `Listed { id, window }` 的列表：名字不是字、是空的跳过；窗口照 `context_window`、`context_length`、`max_context_length` 先有的算，不是正整数的当没报。不是 JSON、没有 `data` 数组的报 `model list not readable: …`。OpenAI 的这一条不分页 |

`OpenAiChat::new(Compat, DriverTexts)`：开关和占位造的时候交进来，会话里不变。

**一次调用要定的** `Call`：`model`（模型名，照供应商的叫法）、`max_output`（输出上限，没有就不写）、`inputs`：`Inputs { images, pdf }`，能看图、能读 PDF，默认都是不能；`effort`：这一次的思考强度（施工 8-18，规整过的名字 `off`、`on`（常量 `EFFORT_OFF`、`EFFORT_ON`）或者目录里的档位名，没有就什么都不加）。

**编码的结果** `Encoded`：`body`（请求字节，发出去的就是它）、`messages`（每条线上的消息在字节里的位置，照先后；system 和挪出来的那条 user 也各算一条）、`path`（发到地址后面的哪一截）。

**说完了** `Ending`：`deltas`（流完了才冲刷出来的那一条解出的增量；正常说完的，再加上收块的 `End`）、`usage`（用量，没报的没有）、`error`（出错的分类和原话，正常说完的没有）、`retry_after_ms`（流里报的错，供应商说要等多久；施工 4-9 再补三下）。

**开关** `Compat`，跟着供应商定：来自供应商的档案（`resources/models/profiles.toml` 的 `compat`，施工 8-6，`models.md`「怎么走」第一条第 3 条），档案没有的用默认。`Continuation::Prefix` 的 `path` 是字（档案里写的），`Compat` 不再是 `Copy`。DeepSeek 那一套以前写在代码里（`Compat::deepseek()`），8-6 挪进档案；代码里那一份只在 `testkit` 开关打开时编进去，给请求形状探针和别的测试用，核心的测试守着它和档案一样。

| 格 | 取值 | 默认 | DeepSeek（档案的 `[providers.deepseek]`） |
|---|---|---|---|
| `output_limit` | `MaxTokens` 写 `max_tokens`；`MaxCompletionTokens` 写 `max_completion_tokens` | `MaxTokens` | 同默认 |
| `reasoning` | `Drop` 不回传；`Replay { field, always }`：`field` 是 `ReasoningContent`（`reasoning_content`）或 `Reasoning`（`reasoning`），`always` 是没有思考时也写空串 | `Drop` | `Replay { ReasoningContent, always: true }` |
| `stream_usage` | 发不发 `stream_options.include_usage` | 发 | 同默认 |
| `continuation` | `None` 不会接着写；`Prefix { field, path }`：`field` 是 `Prefix`（`prefix`）或 `Partial`（`partial`） | `None` | `Prefix { Prefix, "/beta/chat/completions" }` |
| `toggle` | 开关思考的字段（施工 8-18）：`Toggle { field, on, off }`，`on`、`off` 照原样的 JSON 发；没有的是没有。装在 `Box` 里 | 没有 | `thinking`：`{"type":"enabled"}`、`{"type":"disabled"}` |

常量：`FAMILY` = `openai-chat`，`PATH` = `/chat/completions`，`MESSAGE_LIMIT` = 2000（原话最多几个字节，在 `classify.rs`）。

**出错的输入** `Failure { status, headers, body }`：HTTP 状态（没有的是连不上、流里报的）、响应头（名字不分大小写）、响应体。`Failure::stream(body)` 是流里报的错，没有状态和头。**分好的类** `Classified { error, retry_after_ms, excess, limit }`：分类、原话和 HTTP 状态码，供应商说了要等多久（毫秒），超长的超了多少 token（施工 6-6 中），超长的报了的上限（施工 8-7）。分类是内核的 `ErrorClass`：`context_too_long`、`content_policy`、`auth`、`rate_limited`、`retryable`、`other`，解码时还会出 `bad_stream`。

### 怎么走：编码

1. **顶层**，照这个先后，别的字段一概不发：`model`、`messages`、`tools`（见第 8 条）、`"stream":true`、`"stream_options":{"include_usage":true}`（开关开着才有）、输出上限（`Call.max_output` 有才写，字段名照开关）、思考强度（`Call.effort` 有才写，施工 8-18，`openai_chat/effort.rs`：档位写 `"reasoning_effort":"<档位>"`；`off` 有开关的写 `"<field>":<off>`，没有的写 `"reasoning_effort":"none"`；`on` 有开关的写 `"<field>":<on>`，没有的不写）。紧凑的 JSON，结构体照声明的先后写，参数格式原样照抄。
2. **system**：第一条 `{"role":"system","content":…}`；空的不发。
3. **user**：
   - 全是文字的，`content` 是一个字符串：相邻两块之间补一个换行，前一块已经以换行结尾的不补；空的一块什么都不接。
   - 有能发的图片、文件的（第 9 条），`content` 是几段：`{"type":"text","text":…}`、`{"type":"image_url","image_url":{"url":…}}`、`{"type":"file","file":{"filename":…,"file_data":…}}`；连着的文字照上面拼成一段，带名字的图片前后的标签也算文字。发不了的图片、文件换成的字（占位、替它看的图的转述、文本文件的内容）照文字拼。
   - 思考、工具调用、不认识的块不写。一个字都没有的，`content` 是空串。
4. **assistant**：
   - 正文各块直接接上，不补换行，写进 `content`。没有正文、有工具调用的，`content` 写 `null`；两样都没有的，写空串。
   - 思考各块直接接上。开关是 `Drop` 的不写；`Replay` 的写进 `reasoning_content` 或 `reasoning`，没有思考的，`always` 才写空串。思考的私有数据（签名这类）不发。
   - 工具调用写进 `tool_calls`，见第 5 条；没有的不写这一格。图片、文件、不认识的块不写。
   - 一格的先后：`role`、`content`、`reasoning_content`、`reasoning`、`tool_calls`、`prefix`、`partial`。
5. **工具调用**：`{"id":…,"type":"function","function":{"name":…,"arguments":…}}`。
   - `id`：私有数据是这个驱动的、里面有字符串 `id` 的，用它；没有的用内核分的 `call_<序号>_<第几个>`。
   - `arguments`：参数原文是一个 JSON 对象的，一个字节不改；空的、坏的、不是对象的，写 `{}`。
6. **tool**：`{"role":"tool","tool_call_id":…,"content":…}`。
   - `tool_call_id` 和对应那次调用的 `id` 一样。
   - 文字照 user 的拼法，发不了的图片、文件换成的字也照文字拼；一个字都没有的，写 `no-output.txt` 那一句；只有能发的图片、文件、没有字的，写 `tool-attachments-only.txt` 那一句。
   - 统一的请求里的 `error` 不发：出错写在内容里。
7. **挪出来的附件**：tool 消息里能发的图片、PDF 挪走，一串 tool 消息完了（下一条不是 tool、或者到了最后），插一条 user：第一段是 `tool-attachments.txt` 那一句，后面照先后放它们。带名字的图片连同前后的标签一起挪，标签照 user 的拼法和挨着的文字拼成一段（`read` 读出来的图不带名字，现在不会有，施工 3-9 四补）。没有要挪的不插。
8. **工具面**：`[{"type":"function","function":{"name":…,"description":…,"parameters":…}}]`，照统一的请求的先后，参数格式原样。工具面是空的、历史里也没有工具调用的，不发 `tools`；历史里有调用的，发 `[]`（有的网关要）。
9. **图片、文件**：
   - 图片：`Call.inputs.images` 是真的，写成 data URL；不是的，换成占位那一句，照文字接上。
   - 替它看的图（施工 8-17，`models.md`「怎么走」第十三条）：`Call.inputs.images` 不是真的、统一的请求的 `described` 里有这张图（照 `blob`）的转述、快照里有那三句标签的，不写占位，写 `image-description-open.txt`（带名字的图用 `image-description-open-named.txt`，写上名字）、转述原文（不转义：模型写的多行正文，和检查点里的摘要一样；末尾没有换行的补一个）、`image-description-close.txt`，照文字接上。工具结果里的就地换，和占位一样。能看图的不看 `described`：请求字节和没有转述时一个不差。快照里没有那三句的（以前造的）照旧写占位。
   - 带名字的图片（人附的，`kernel/blocks.md` 第 14 条，施工 3-9 四补）：能看图的，前后各一段文字，`image-open.txt`（带名字）、图片、`image-close.txt`；不能看图的，占位写 `image-omitted-named.txt`（带名字）。不带名字的照旧：图片前后什么都不加，占位写 `image-omitted.txt`。快照里没有这三句的（以前造的），带名字的也照不带名字的写。
   - 为什么图片带名字（施工 3-9 四补，2026-09-30 网页演示接真核心实测撞见）：一句话附了一张图、一个 PDF、一个文本文件，问哪个是图片、只答文件名，她答不出，因为发给她的图没有名字。标签的写法照文本文件的 `<file name=…>`。
   - 文件，照这个先后，先对上的算：
     1. `Call.inputs.pdf` 是真的、媒体类型正好是 `application/pdf` 的：写成 data URL 放进 `file`，`filename` 是文件名。
     2. 内容是文本文件（整份是合法的 UTF-8，又没有 NUL 字节；媒体类型、扩展名不看，`crates/miyu-drivers/src/text_file.rs`），快照里有文本文件的三句的（施工 3-9 三补）：照文字接上，`file-open.txt`（带文件名）、内容、`file-close.txt`。内容原样，不转义（和检查点里重读的文件一样，`kernel/request.md`）；末尾没有换行的补一个，空的只有开头收尾。最多给 64 KiB（65,536 字节），多的截在字的边界上，开头那一行后面先写 `file-cut.txt`：给了多少、一共多少字节。原文整份留在 blob 里。
     3. 别的：换成 `file-omitted.txt` 那一句，带文件名、媒体类型、大小（字节数），照文字接上。
   - 为什么文本文件照字给（施工 3-9 三补，2026-09-30 项目主人定附件现在就排）：哪个模型都读得了字，不用另有本事；64 KiB 的上限是一个附件不占掉大半个上下文，它每次请求都跟着。以前造的快照里没有那三句，文本文件照第 3 小条写占位（`file-omitted.txt` 那时也没有大小）。
   - data URL：`data:<媒体类型>;base64,<内容>`，base64 用 RFC 4648 的标准字母表，末尾补 `=`。字节由执行器照 `blobs_needed` 先取出来交进来，驱动不碰文件。
10. **接着写**：开关是 `Prefix`、统一的请求带着 `continuation` 的：
    - 最后一条是 user 的（只有被打断的那一句），不发；
    - 写好以后最后一条是 assistant（半截那条）的，加上 `"prefix":true` 或 `"partial":true`；
    - `path` 是开关里的路径：只看开关和记号，最后几条的样子不对也发到那里。
    - 别的情形（开关是 `None`，或者没带记号）照原样发，`path` 是 `/chat/completions`。
11. **要哪些 blob**（`blobs_needed`）：user 和 tool 消息里的图片（能看图时）、每一个文件（施工 3-9 三补：发不了的也要认是不是文本、要写有多大）。assistant 里的不算。
12. **`Encoded.messages`**：每条线上的消息 JSON 在 `body` 里的起止，照先后。

### 怎么走：解码

**SSE 分帧**（WHATWG 的规矩）：

1. `\n`、`\r\n`、`\r` 都算换行；`\r\n` 被切在两片之间，还是一个换行。
2. 一行收全了才按 UTF-8 解，坏的字节换成 U+FFFD：一个汉字被切在两片之间也不坏。
3. `:` 开头的是注释。别的行是「字段名:值」，值前面的一个空格去掉；只有字段名、没有冒号的，值是空的。`data` 的值几行用换行接起来；`event` 记下名字；`id`、`retry` 和不认识的字段不理。
4. 空行结束一条：有 `data` 的交出来，名字跟着清掉。
5. 流完了，没收全的最后一行、最后一条照样交出来。
6. 分片从哪里切开都一样。

**一条事件**：

1. 见到 `[DONE]`，或者已经出了错，以后的都不理，`done()` 是真，执行器不再读。
2. `data` 去掉前后空白；是 `[DONE]` 的，说完了；是空的，不理。
3. 名字是 `error` 的：出错，照「出错分类」分，用的是 `Failure::stream(data)`。
4. 解成 JSON，取 `choices`、`usage`、`error` 三格，别的不理。格的类型对不上的（例如 `content`、`finish_reason` 写成了数字；`null` 不算）也算解不开。解不开：流完了才冲刷出来的那一条，算 `retryable`，「流断在半段 JSON 上」；别的算 `bad_stream`，「流里有一段不是 JSON」。
5. `error` 有内容的：出错，照「出错分类」分，这一段别的都不看。`null`、`""`、`{}`、`false`、`0`、`[]` 是网关的噪声，不理（施工 4-9 再补三下）；别的都算有内容。
6. 顶层的 `usage`，有 `prompt_tokens` 的记下（见下表），以后来的盖掉以前的。
7. 只看 `choices[0]`，没有就完了。它里面的 `usage`（Moonshot）照第 6 条。
8. `delta` 里照这个先后：
   - 思考：`reasoning_content`、`reasoning`、`reasoning_text` 里第一个不是空串的；
   - 正文：`content`，不是空串的；
   - 工具调用：`tool_calls` 里的每一片，见下。
9. 思考、正文一次响应各最多一块：第一次来时 `Start`，以后都是这一块的 `Text`。块照第一次出现的先后编号，从 0 数起，和工具调用共用一套编号。
10. `finish_reason` 不是空串的记下，以后来的盖掉以前的。

**工具调用的一片**：

1. 认是哪一次：有 `index` 的按 `index` 找；没有的按 `id` 找；都没有的，是最近的那一次。找不到的，是新的一次。
2. 第一个不是空串的 `id`、第一个不是空串的 `name` 记下；`arguments` 先攒着。
3. 工具名到了，这一块才开始：`Start`，种类 `ToolCall { name }`。
4. 开始了的：有 `id`、还没写过的，写一次私有数据 `Private`：`{"driver":"openai-chat","data":{"id":"…"}}`；攒着的参数交出去（`Text`）。
5. 工具名一直没来的调用，参数和编号都丢掉，不报错。

**收尾**（`finish`）：

1. 流里记下的错优先；没有的照 `finish_reason` 判：

| 情形 | 结果 |
|---|---|
| `stop`、`tool_calls`、`function_call`、`length`、`end` | 正常说完 |
| `content_filter` | `content_policy`，原话 `finish_reason: content_filter` |
| `network_error` | `retryable`，原话 `finish_reason: network_error` |
| 别的 | `other`，原话 `finish_reason: <它>` |
| 没有 `finish_reason`，见到了 `[DONE]` | 正常说完 |
| 两样都没见到 | `retryable`，「流断了：没等到 finish_reason，也没等到 [DONE]」 |

2. 正常说完的：开始了的块（正文、思考、有名字的调用）照编号一块块 `End`。出了错的不收，交给内核照出错处理。
3. 用量取最后记下的那一份。

**用量**，归成内核的四项：

| 项 | 取哪里（相对 `usage` 那一格） |
|---|---|
| 命中缓存 `cache_read` | `prompt_tokens_details.cached_tokens`；没有取 `prompt_cache_hit_tokens`（DeepSeek）；再没有取 `cached_tokens`；都没有是 0 |
| 写进缓存 `cache_write` | `prompt_tokens_details.cache_write_tokens`；没有是 0 |
| 没命中 `uncached` | `prompt_tokens` 减去上面两项，最少 0 |
| 输出 `output` | `completion_tokens`（含思考）；没有是 0 |

没有 `prompt_tokens` 的不算用量；只认非负整数。

### 怎么走：出错分类

1. 响应体按 UTF-8 读（坏字节换掉），试着解成 JSON。错误码、类型、原话先看 `error` 里的，没有再看顶层：
   - 错误码 `code`、类型 `type`：字符串照写，数字写成字；
   - 流里报的错没有状态：`code` 是 400 到 599 的整数，就当状态用；
   - 原话：`message`、`error` 本身是字符串的、`detail`，第一个不是空串的；都没有，用整个响应体去掉前后空白。
2. 找说法的字：原话、错误码、类型、整个响应体，接在一起、全部小写，找子串。「4xx」指状态在 400 到 499，或者没有状态。
3. 照这个先后，先对上的算：

| 类 | 条件 |
|---|---|
| `context_too_long` | 413；或者 4xx、没有限速的说法，又是超长的错误码或者有超长的说法 |
| `content_policy` | 4xx，又是内容策略的错误码或者有内容策略的说法 |
| `auth` | 401、402、403；或者有额度的说法（额度用完也算这一类） |
| `rate_limited` | 429；或者有状态、是 4xx、有限速的说法 |
| `retryable` / `other` | 有 `x-should-retry` 头的，值是 `true`（不分大小写）就 `retryable`，别的 `other` |
| `retryable` | 没有状态；408、409；500 起 |
| `other` | 别的 |

4. 清单，都照小写比。「错误码是」指 `code` 或 `type` 正好是其中一个；「说法」指找说法的字里有这一截：
   - 超长的错误码：`context_length_exceeded`、`model_context_window_exceeded`、`request_too_large`。
   - 超长的说法（23 句）：`prompt is too long`、`prompt too long`、`input is too long`、`too large for model`、`exceeds the context window`、`context window exceeds`、`maximum context length`、`context length exceeded`、`context_length_exceeded`、`context length is only`、`greater than the context length`、`longer than the model`、`exceeds the available context size`、`the configured context size`、`exceeded model token limit`、`token limit exceeded`、`too many tokens`、`tokens in request more than max tokens allowed`、`reduce the length of the messages`、`maximum prompt length is`、`maximum allowed input length`、`range of input length should be`、`exceed context limit`（Anthropic 的老模型输入加 `max_tokens` 超了窗口时这样说，施工 8-12 加，两家共用）。
   - 限速的说法：`rate limit`、`rate_limit`、`too many requests`、`throttling`、`service unavailable`。
   - 内容策略的错误码：`content_filter`、`responsibleaipolicyviolation`、`content_policy_violation`、`image_content_policy_violation`、`refusal`、`cyber_policy`、`bio_policy`、`misalignment_policy_violation`。
   - 内容策略的说法：`violating our usage policy`、`blocked by content filtering policy`、`content policy`、`content-policy`、`content_policy`、`contentpolicy`、`rejected as a result of our safety system`。
   - 额度的说法（也找错误码、类型，因为它们在找说法的字里）：`insufficient_quota`、`insufficient quota`、`insufficient balance`、`insufficient_balance`、`exceeded your current quota`、`quota exceeded`、`billing_hard_limit_reached`、`credit balance is too low`、`usagelimiterror`。
5. **原话**：有 HTTP 状态的写成 `HTTP <状态>: <原话>`，流里报的只写原话；最长 2000 字节，截在字的边界上。原话给查问题的人看，不进上下文。HTTP 状态码另写进 `status`，原话开头的 `HTTP <状态>: ` 照留；连不上的、流里报的没有，流里的 `code` 只拿来分类（施工 3-5 三补）。
6. **要等多久**，先有的算：头 `retry-after-ms`（毫秒）；头 `retry-after`（秒，可以带小数；写成日期的不认）；找说法的字里的 `try again in <数>`，单位 `ms` 是毫秒、`s` 开头的是秒（`s`、`seconds`）。非负的数才算，四舍五入到毫秒。都没有就不写，由内核退避。头的名字不分大小写，值去掉前后空白。流里报的错，解码器连同要等多久一起留下，收尾时交给 HTTP 执行器（施工 4-9 再补三下）。
7. **超了多少**（施工 6-6 中，`compaction.md` 第三条第 10 条）：分成 `context_too_long` 的，从找说法的字里解析，先对上的算：`maximum context length is <N>` 后面跟着 `resulted in <M>` 或者 `requested <M>`（OpenAI、DeepSeek 的写法）；`prompt is too long: <M> tokens > <N>`（Anthropic 的写法）。数可以带千分位的逗号。M 比 N 大才算，`excess` 是 M 减 N；别的分类、解析不出来的没有。
8. **报了的上限**（施工 8-7，`models.md`「驱动要守的约定」第 7 条）：分成 `context_too_long` 的，同样的两种写法里说得出 N 的交出 `limit`，就是 N，后半段说不出、没超的也交；N 是 0 的、别的分类没有。执行器照它记下用出来的窗口（`models.md`「怎么走」第二条第 9 条）。

### 现在接的是哪一家

照配置（施工 8-6，`models.md`）：`[providers.<编号>]` 写了 `driver = "openai-chat"`，或者档案推得出是它的，或者它对上的目录里那一家的 `npm` 照档案的 `[npm]` 换成它的（施工 8-7），都走这个驱动。开关照档案（写了 `catalog` 的照它指的那一家的档案）；档案里只有 DeepSeek 官方一家：上表那一套开关，一张图照官方计算器的算法。能收哪些输入照模型资料（施工 8-7：手写的，再是目录；都没有的只收字），目录里 DeepSeek 官方的 `deepseek-flash` 收图、不收 PDF。不写输出上限。档案里没有的供应商用默认的开关。

### 样子

**线上的请求**，样本在 `docs/designs/samples/drivers/openai-chat/`，一种写法一个文件，是请求字节加一个换行：

| 样本 | 哪种写法 |
|---|---|
| `plain-text.json` | 只有文字；没有工具、没有调用，不发 `tools` |
| `max-completion-tokens-without-usage.json` | `max_completion_tokens`，不报用量 |
| `tool-calls.json` | 供应商的编号、内核的编号、空的和坏的参数写 `{}`、没有输出的占位、`max_tokens` |
| `user-blocks.json` | user 里几块怎么补换行 |
| `empty-tools.json` | 工具面是空的、历史里有调用：`"tools":[]` |
| `unknown-block.json` | 不认识的块不写 |
| `media.json`、`media-omitted.json` | 图片、PDF 写成 data URL；不能收的换成占位 |
| `text-files.json` | 文本文件照字放进消息、带着文件名，空的只有开头收尾；二进制的、不是 UTF-8 的、读不了的 PDF 换成占位，带大小（施工 3-9 三补） |
| `image-names.json`、`image-names-omitted.json` | 人附的两张图带着名字：前后各一段标签；不能看图的占位写上名字（施工 3-9 四补）。不带名字的照旧，见 `media.json`、`tool-attachments.json` 这几份 |
| `tool-attachments.json`、`tool-attachments-omitted.json` | 工具结果里的图挪到后面；不能看图的就地换成占位 |
| `reasoning-dropped.json`、`reasoning-deepseek.json`、`reasoning-field.json` | 思考不回传；DeepSeek 每条都带；写进 `reasoning` |
| `continuation-deepseek.json` | 接着写：没有最后那句提示，半截带 `"prefix":true` |

探针的每一次请求编码以后的样子：`docs/designs/samples/probe/terminal/openai-chat/`（DeepSeek 那一套，模型 `deepseek-v4`，上限 8192）。

**流**，样本在同一目录的 `streams/` 下：`.sse` 是进去的字节，`.txt` 是解出来的，一行一条：`deepseek-reasoning-tools`、`openai-text`（CRLF）、`tool-call-fragments`、`late-name`、`moonshot-usage-in-choice`、`gateway-noise`、`stream-error`、`cut-off`、`content-filter`、`length`、`bad-json`、`done-without-finish`、`ended-early`。

**给模型看的几句**，原文在 `resources/core/drivers/`，行尾的换行也算，登记在 `26-提示词.md` 第十节：

| 文件 | 原文 | 字段 |
|---|---|---|
| `image-omitted.txt` | `An image was attached here, but this model cannot view images.` | 没有 |
| `file-omitted.txt` | `A file was attached here ({name}, {media_type}, {size} bytes), but this model cannot read it.` | `name`、`media_type`、`size`（字节数，施工 3-9 三补），照模板的规矩转义 |
| `no-output.txt` | `The tool returned no output.` | 没有 |
| `tool-attachments.txt` | `These images and files were returned by the tool calls above.` | 没有 |
| `tool-attachments-only.txt` | `The tool returned only images or files. They are in the next message.` | 没有 |
| `file-open.txt` | `<file name="{name}">` | `name`，照模板的规矩转义（施工 3-9 三补） |
| `file-cut.txt` | `Only the first {shown} of {total} bytes of this file are shown.` | `shown`、`total`（施工 3-9 三补） |
| `file-close.txt` | `</file>` | 没有（施工 3-9 三补） |
| `image-open.txt` | `<image name="{name}">` | `name`，照模板的规矩转义（施工 3-9 四补） |
| `image-close.txt` | `</image>` | 没有（施工 3-9 四补） |
| `image-omitted-named.txt` | `An image was attached here ({name}), but this model cannot view images.` | `name`，照模板的规矩转义（施工 3-9 四补） |

几份在 `DriverTexts::new` 时读成模板，拿字段试换一次：`file-omitted` 只能要 `name`、`media_type`、`size`，`file-open`、`image-open`、`image-omitted-named` 只能要 `name`，`file-cut` 只能要 `shown`、`total`，别的不能要字段。原文随会话的策略快照（`policy.md`）；文本文件的三句、带名字的图片的三句以前造的快照里没有。

`text-files.json` 这份样本里那条 user 的 `content`，写开来是：

```text
看看这几个文件
<file name="notes.md">
# 待办
- 写测试
- 跑 CI
</file>
<file name="empty.txt">
</file>
A file was attached here (data.bin, application/octet-stream, 12 bytes), but this model cannot read it.
A file was attached here (old.txt, text/plain, 5 bytes), but this model cannot read it.
A file was attached here (报告.pdf, application/pdf, 15 bytes), but this model cannot read it.
```

### 出错

报错是中文，给查问题的人看。

| 什么时候 | 分类 | 怎么说 |
|---|---|---|
| 编码要的 blob 执行器没交进来 | 编码失败 `EncodeError::MissingBlob` | `编码要用 blob <哈希>，执行器没交进来` |
| 流里有一段解不开 | `bad_stream` | `流里有一段不是 JSON：<那一段的前 200 个字>` |
| 流完了才冲刷出来的一段解不开 | `retryable` | `流断在半段 JSON 上：<前 200 个字>` |
| 两样都没等到 | `retryable` | `流断了：没等到 finish_reason，也没等到 [DONE]` |
| `finish_reason` 不对 | 见收尾那张表 | `finish_reason: <它>` |
| HTTP 出错、流里报错 | 见出错分类 | `HTTP <状态>: <原话>`，或者原话 |
| 占位的模板坏了、要了不该要的字段 | 造策略时报 | `bundled driver placeholders not usable: bad template: …`（`policy.md`、`kernel/request.md`） |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-drivers/tests/openai_chat.rs` | 只有文字；输出上限和用量两个开关；工具调用、编号、参数兜底、没有输出的占位；user 的换行；空工具面；不认识的块；空的 system 不发；每条消息的位置 |
| `crates/miyu-drivers/tests/openai_chat_media.rs` | 图片、PDF 写成 data URL；不能收的占位；工具结果里的附件挪到后面、或者就地占位；思考的三种回传；缺 blob 报错；要哪些 blob（文件每一个都要） |
| `crates/miyu-drivers/tests/openai_chat_files.rs` | 文本文件（施工 3-9 三补）：照字放进消息、带文件名，空的，二进制的、不是 UTF-8 的、读不了的 PDF 写占位带大小（样本）；能读 PDF 的照旧发 `file`；超过 64 KiB 的截掉、写明给了多少；工具结果里的照字进 `content`；以前造的快照没有那三句的写占位；缺 blob 报错 |
| `crates/miyu-drivers/tests/openai_chat_image_names.rs` | 带名字的图片（施工 3-9 四补）：能看图的前后各一段标签，和挨着的字拼成一段；不能看图的占位写名字（两份样本）；工具结果里带名字的连同标签一起挪、就地的占位写名字；以前造的快照没有那三句的，带名字的和不带名字的一字不差，不带名字的出厂这一份也照旧 |
| `crates/miyu-drivers/tests/openai_chat_described.rs`（施工 8-17） | 替它看的图：不能看图的换成带名字的标签和转述（样本 `image-descriptions.json`），转述原样、尖括号引号不转义，没转述的照旧占位；能看图的有没有转述一个字节不差；工具结果里不带名字的就地换、末尾有换行的不再补；以前造的快照没有标签的照旧写占位 |
| `crates/miyu-drivers/src/text_file/tests.rs` | 什么算文本：空的、UTF-8、BOM 算，NUL（在后面的也算）、Latin-1、PDF 不算；截到 64 KiB、截在字的边界上 |
| `crates/miyu-drivers/tests/openai_chat_effort.rs` | 思考强度（施工 8-18）：没写的不加；档位发 `reasoning_effort`、接在最后；`off`、`on` 照开关；没有开关的 `off` 发 `none`、`on` 不加；前面的字节一个不动 |
| `crates/miyu-drivers/tests/openai_chat_continuation.rs` | DeepSeek 接着写（样本、路径、半截带思考）；没有开关或者没有记号一字不变；`partial` 的写法 |
| `crates/miyu-drivers/tests/openai_chat_streams.rs` | 十三份流的样本；从哪里切开喂都一样；累积器一条都不拒；解出来的编码回去用供应商的编号；驱动的接口走一遍；`error` 是 `false`、`0`、`[]` 的是噪声，有内容的照旧出错；流里的限速连同要等多久交回；`finished()` 在 `finish_reason` 到了以后才说是 |
| `crates/miyu-drivers/src/sse/tests.rs` | 三种换行、切开的 CRLF、几行 data 和注释、只有注释、事件名、切开的汉字、断在半条上、从哪里切开都一样 |
| `crates/miyu-drivers/src/openai_chat/models/tests.rs` | 列模型：名字和报了的窗口、三种窗口的写法先后、坏的跳过；回应坏了说是模型列表（施工 8-7） |
| `crates/miyu-drivers/src/classify/excess/tests.rs` | 超了多少的几种写法；报了的上限单独读，后半段说不出也交、0 不交（施工 8-7） |
| `crates/miyu-drivers/src/classify/tests.rs` | 超长的交出上限、限速的不交（施工 8-7）；每一类的例子；提到 token 的限速不当超长；额度算认证失败；要等多久的四种写法；`x-should-retry`；原话和 2000 字节；HTTP 状态码另记一格，连不上的、流里报的没有（施工 3-5 三补） |
| `crates/miyu-drivers/src/texts/tests.rs` | 文件名换进去、转义；以前的 `file-omitted` 没有大小照样换得出；文本文件带文件名、补换行、空的、截过的写明、文件名转义内容原样；没有那三句的交回空的；带名字的图片的标签、占位带名字、转义，不带名字的照旧，没有那三句的照不带名字的写（施工 3-9 四补）；不该有的字段报错 |
| `crates/miyu-drivers/src/base64/tests.rs` | RFC 4648 的测试值，`+`、`/` |
| `crates/miyu-assemble/tests/probe.rs`、`random_logs.rs` | 编码以后也是上一次的前缀延伸（接着写那一次拿不接着写的编码比） |

### 出处

- `05-内核接口.md` 第七节：驱动的规格；「OpenAI 兼容的对话接口怎么编码」；「接着写被打断的回复」；「怎么解码」；「出错怎么分」。
- `15-模型与供应商.md` 第三节（模型能收哪些输入）、M5（断了接着来、前缀续写只开实测过的）。
- `03-事件模型.md` 第九节（驱动私有数据）、第五节（四种增量）。
- `26-提示词.md` 第八节、第十节（占位的原文和登记）。

### 还没有的

- 驱动规格里的 `cache`（缓存类型）、子进程的 `transport`（`05-内核接口.md` 第七节）。列模型分页的几家（随它们自己的驱动）。
- 配置里手写的 `compat` 一格格盖在档案上面（`models.md`「对外的样子」）：随用到它的那一步。
- 别的驱动家族：借用 agent CLI 的子进程（`15-模型与供应商.md` 第二节）。Anthropic 的消息接口施工 8-12 做了（`drivers/anthropic.md`），OpenAI 的 Responses 接口施工 8-13 做了（`drivers/openai-responses.md`）。
- 接 opencode Zen 要的：工具面缺 `read`、`shell` 时补同名的占位声明，带 `x-opencode-*` 头（`15-模型与供应商.md` 第二节）。驱动这边要做的见末尾「接 opencode Zen」。
- Kimi、通义的 `partial`、Mistral 的 `prefix`：写法有了，出厂没开，等实测（`05-内核接口.md` 第七节）。

### 接 opencode Zen（8-14 驱动这边的一半）

状态：2026-10-02 起草，2026-10-03 主会话审过、照实测收窄（免费档不做，见 `models.md`「八、opencode Zen」）。头在 `models.md` 第八条，这里只写驱动这边要做的。

1. **编码、解码、分类不加新写法**：Zen、Go 的 OpenAI 兼容那一路是标准的 `/chat/completions`，照这一页走。头由 HTTP 执行器照端点另配的头发，不进驱动。
2. **一家三种驱动**：目录里 Zen、Go 的模型各自写着 `provider.npm`（2026-10-02 的快照：Zen 114 个，51 个走这一页，32 个 `@ai-sdk/openai` 走 `openai-responses`，23 个 `@ai-sdk/anthropic` 走 `anthropic`，8 个 `@ai-sdk/google` 没有驱动；Go 33 个，23、7、3）。
   - 怎么挑是模型资料的 `driver` 那一格（`models.md`「模型的资料」）：先后是手写的供应商 `driver`、第 1、2 层对上的模型的 `provider.npm`（照档案的 `[npm]` 表换成驱动）、档案的、目录里那一家的 `npm`。路由照真发的那个模型造驱动（`route/base.rs`）。模型的 `npm` 换不出驱动的（Google 那几个），这个模型当场 `no_model`，原话 `model "<供应商>/<模型>" needs driver "<它>", which is not available yet`，同一家的别的照常。
   - 能不能关思考、要不要替它填输出上限，照模型的驱动算（Zen 上的 Claude 走 `anthropic`，开关是接口自带的）。
   - 三种驱动接同一个地址，各发各的路径（`/chat/completions`、`/messages`、`/responses`），key 一样，认证头照各自的驱动。
   - 列模型照供应商的驱动：这一页的 `GET /models`。
3. **开关**：照默认（没实测过的不开）：`max_tokens`、发 `stream_options`、不接着写、没有思考的开关（目录写着 `toggle` 的模型不多 `off`）。思考回传照第 4 条。
4. **思考回传照目录的 `interleaved`**（对所有走这一页的供应商都成立，不只 Zen）：
   - 目录给交错思考的模型写了 `interleaved`。`{"field":"reasoning_content"}` 的，`reasoning` 开关当 `Replay { ReasoningContent, always: true }`；`{"field":"reasoning"}` 的当 `Replay { Reasoning, always: true }`；别的写法（`true`、`{"field":"reasoning_details"}`）不认，照档案。
   - 只认第 1、2 层对上的（手写指定的、供应商对上了的）：字段名是供应商接口的写法，不是模型的性质，按名字对上的中转不借。
   - 档案写了 `reasoning` 的照档案（DeepSeek 官方）；只管走这一页的模型。
   - 为什么：Zen 的 OpenAI 兼容模型 51 个里 39 个写着 `reasoning_content`，Go 是 23 个里 17 个。照默认不回传，工具循环里她每一步都丢了上一步的思路。2026-10-03 实测 Go 上的 `deepseek-v4.1-flash`：不带、带空串、带字都不报 400，回传是为了接上思路。
   - `always` 是真的：照 DeepSeek 官方实测过的那一种，没有思考的 assistant 也带空串（Go 上实测收）。
   - 这是 `miyu_models` 的事：资料多一格 `interleaved`（只取第 1、2 层），路由造驱动时盖在档案的开关上（`Provider::for_model`）。驱动本身一行不改。
5. **出错**：Go 缺 `x-opencode-session` 回 400 `MissingSessionID`，是 `other`；带上了就不会遇到。Zen 免费档的 403 `FreeTierError` 照分类表是 `auth`。
6. **用量**：照「解码」那张表。
7. **守着它的**：`miyu-models` 的资料测试（`driver` 照模型、`interleaved` 第 1、2 层取、第 3、4 层不取、档案写了的照档案、认不得的写法不取）；档案的头（值里只认 `{session_digest}`）；路由的测试（同一家的模型照资料造三种驱动、没有驱动的模型 `no_model`、别的照常、请求带头）。
8. **真模型实测**：项目主人给的 Go key：`deepseek-v4.1-flash` 带工具的会话跑三轮，思考回传；走 `anthropic`、`openai-responses` 的 Go 模型各问一句。

**起草时定的**（2026-10-02，10-03 收窄时照改）：

| # | 定了什么 | 为什么 | 别的选法 |
|---|---|---|---|
| 1 | 思考回传照目录的 `interleaved`，按模型 | Zen 一家里的模型各有各的上游，档案只能按供应商写一份；目录本来就按模型写了，代码里不用有模型表 | Zen 的档案一律回传：不交错思考的上游收到多出来的字段，有的会拒 |
| 2 | `always` 是真的 | DeepSeek 官方实测要它；空串对不用它的模型没有意思 | 假的：DeepSeek 一类的示范对话那几条会被拒 |
| 3 | 驱动按模型挑，列模型照供应商的 | 目录写的就是按模型的；列表接口只有一个 | 一家一个驱动：Zen 上的 Claude、GPT 用不了 |
