## 内容块、原样的 JSON、认不出的取值

### 是什么

消息和工具结果的内容由内容块组成：文字、思考、图片、文件、工具调用，一共五种。两样零件跟着它们，也用在事件的别处：原样的 JSON，一个字节都不改的一段 JSON；用字符串写的取值，认识的读成对应的一种，不认识的原样留着。新版本加了种类、加了取值，旧版本照样读得进来，写出去也不抹掉。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/block.rs` | 五种块 `Block`、驱动私有数据 `Private`、不认识的块 |
| `crates/miyu-kernel/src/raw.rs` | 原样的 JSON `RawJson`；照 `type` 或 `kind` 分派的读法 `read_tagged`，块、`by`、效果都用它 |
| `crates/miyu-kernel/src/text_enum.rs` | 生成字符串取值的宏 `text_enum!`，内核里的十一种取值都由它生成 |

### 对外的样子

**内容块** `Block`：JSON 里用 `type` 分开。

| `type` | 格 | 例子 |
|---|---|---|
| `text` | `text`：文字 | `{"type":"text","text":"我先看一下目录。"}` |
| `reasoning` | `text`：思考的文字；`private`：驱动私有数据，可以没有 | `{"type":"reasoning","text":"先看目录","private":{"driver":"anthropic","data":{"signature":"sig"}}}` |
| `image` | `blob`：图片存成的 blob 的内容哈希；`media_type`：媒体类型；`width`、`height`：宽、高，像素 | `{"type":"image","blob":"sha256:…","media_type":"image/png","width":800,"height":600}` |
| `file` | `blob`：文件存成的 blob 的内容哈希；`name`：文件名，只是名字，不带路径；`media_type`：媒体类型 | `{"type":"file","blob":"sha256:…","name":"报告.pdf","media_type":"application/pdf"}` |
| `tool_call` | `call_id`：内核分的调用编号；`name`：模型说要调用的工具名；`args`：模型给的参数原文，一个字符串；`private`：驱动私有数据，可以没有 | `{"type":"tool_call","call_id":"call_44_1","name":"read","args":"{\"path\":\"src\"}"}` |

- 除了标着「可以没有」的，每一格都必有。
- 写法照 `kernel/ids.md`：`blob` 是内容哈希，`media_type` 是媒体类型，`name`（`file` 的）是文件名，`call_id` 是调用编号。`width`、`height` 是 0 到 4294967295 的整数（`u32`）。`text`、`args`、`tool_call` 的 `name` 是任意字符串。

**驱动私有数据** `Private`：供应商要原样传回的东西，例如思考块的签名、供应商自己的调用编号。

| 格 | 是什么 |
|---|---|
| `driver` | 驱动家族（写法见 `kernel/ids.md`）：哪一类驱动的数据 |
| `data` | 数据本身，任意 JSON，原样的 |

两格都必有。内核不解读；哪一类驱动认哪一份，照 `driver` 分（`drivers/openai-chat.md`）。

**原样的 JSON** `RawJson`：

- 只能从 serde_json 读出来，没有别的造法：它记下的就是读到的原文，所以能一字不差。
- `get()` 交回原文。
- 两份原文一字不差才算相等：`{"a":1}` 和 `{ "a": 1 }` 不相等。
- 写出去就是原文：空格、数字的写法（`1.50`）、字段的先后都不变。

用在这些地方：

| 哪里 | 放的是什么 | 见 |
|---|---|---|
| `Private` 的 `data` | 驱动私有数据 | 本页 |
| 不认识的块、不认识的 `by`、不认识的效果 | 整块 | 本页、`kernel/ids.md`、`kernel/events-bodies.md` |
| 不认识的事件种类 | 整个 `body` | `kernel/events.md` |
| `tool.approval_requested` 的 `rule`、`detail` | 提问的模块定的写法 | `kernel/events-bodies.md` |
| 工具的参数格式 `ToolRule.parameters`、`ToolSpec.parameters` | JSON Schema，原样进 tools 数组 | `kernel/tools.md`、`kernel/request.md` |

**字符串取值**（宏 `text_enum!`）：每一种列出认识的几个值，再多一种 `Other(String)`，放不认识的。`as_str()` 是它在 JSON 里的写法。

| 取值 | 在哪（`crates/miyu-kernel/src/` 下） | 认识的值 | 见 |
|---|---|---|---|
| `Access` | `tool.rs` | `read`、`write`、`execute`、`network`、`outbound` | `kernel/tools.md` |
| `Level` | `event/session.rs` | `workspace`、`full` | `kernel/events-bodies.md` |
| `EndReason` | `event/turn.rs` | `completed`、`interrupted`、`error`、`step_limit`、`aborted`、`restarted` | 同上 |
| `RestoreAction` | `event/restore.rs` | `write`、`trash`、`untrash` | 同上 |
| `RestoreOutcome` | `event/restore.rs` | `restored`、`changed`、`missing`、`occupied`、`gone`、`unsaved`、`unavailable`、`failed` | 同上 |
| `ToolStatus` | `event/tool.rs` | `ok`、`error`、`cancelled`、`denied`、`skipped` | 同上 |
| `Decision` | `event/tool.rs` | `once`、`session`、`workspace`、`deny` | 同上 |
| `Part` | `event/model.rs` | `tools`、`system`、`message` | 同上 |
| `MessageRole` | `event/model.rs` | `user`、`assistant`、`tool` | 同上 |
| `CallResult` | `event/model.rs` | `ok`、`error`、`interrupted` | 同上 |
| `ErrorClass` | `event/model.rs` | `retryable`、`rate_limited`、`context_too_long`、`auth`、`content_policy`、`other`、`bad_stream`、`empty_reply` | 同上 |

`ErrorClass` 里写成 `other` 的那一个认识的值，代码里叫 `Unclassified`：`Other` 这个名字留给读到的不认识的分类。

### 怎么走

**读一块**（`read_tagged`，`by` 照 `kind`、效果照 `kind` 也是这样读）：

1. 先把整块原样读下来。
2. 不是 JSON 对象的，报 serde_json 的原话（`expected a map`）。
3. 没有 `type` 的，报「缺了 type 字段」。
4. `type` 不是字符串的，报 serde_json 的原话（`invalid type`）。
5. 认识的五种，照那一种读整块。缺了格、某一格不合写法的，报那一格的错，例如「missing field `width`」「内容哈希的写法不对：…」「调用编号的写法不对：…」「驱动家族的写法不对：…」。
6. 认识的种类多出来的格不管：读进内存时丢掉，写出去不再有。
7. 不认识的种类：整块原样留着（`Block::Unknown`），写出去一字不差，空格、数字的写法都不变。

**写一块**：

8. `type` 在最前，其余几格照上表的先后。
9. `private` 没有就不写这一格。
10. 不认识的块照原文写。

**块里的规矩**：

11. 工具名和参数不检查：模型说了什么就记什么，名字不对、参数坏了，是执行时报给模型的错，日志照样读得进来（`kernel/tools.md`）。
12. 参数存模型给出的原文，不解析以后重新写：重新写会改变字节，前缀缓存随之失效。执行用的是修正过的另一份，日志里的不动（`kernel/tools.md`）。
13. 调用编号由内核分，照这条回复的序号写成 `call_<序号>_<第几个>`（`kernel/request.md` 的流式累积器）；供应商自己的编号放在 `private` 里。中途换模型、换供应商，编号照样一致。
14. 图片、文件本身不进事件：块里只放 blob 的内容哈希，内容存成 blob（`store.md`）。图片块的宽、高必有：量得出尺寸才当图片。
15. 格式上哪一种块放在哪里都读得进来。谁放什么，见各种事件的 `blocks`（`kernel/events-bodies.md`）。
16. 投影跳过不认识的块：给模型看的只有认识的五种（`crates/miyu-assemble/src/render.rs` 的 `known`，`kernel/request.md`）。

**字符串取值**：

17. 读：必须是 JSON 字符串，数字、`null` 读不进来。认识的值一定读成对应的一种；别的读成 `Other(原文)`，不报错。
18. 写：认识的写它的写法，`Other` 写原文。所以读进来再写出去一字不差。
19. 不认识的值当什么，由用到它的地方决定：

| 取值 | 读到不认识的 | 在哪 |
|---|---|---|
| `Access` | 算写入：只读时当场拦下；不和别的调用一起跑 | `crates/miyu-kernel/src/tool.rs` 的 `writes`，`crates/miyu-kernel/src/session/step.rs` 的 `ready`（`kernel/tools.md`） |
| `Level` | 按最严的算：实际生效的是只读，告诉她的也是 `read_only`，执行前的链也按只读判。切级别的命令带着不认识的级别，拒绝，原因码 `unknown_level` | `crates/miyu-kernel/src/session/permission.rs` 的 `rank`、`set_permission`，`crates/miyu-kernel/src/facts.rs` 的 `effective_level`，`crates/miyu-session/src/guard.rs` 的 `effective` |
| `EndReason` | 投影不写回合没走完的那一句（`resources/core/turn-ended/` 下的几句一句都不用），和 `completed` 一样 | `crates/miyu-assemble/src/texts.rs` 的 `for_reason` |
| `ToolStatus` | 投影里算没成（tool 消息的 `error` 是真） | `crates/miyu-assemble/src/render.rs` |
| `Decision` | 回答确认的命令带着不认识的决定，拒绝，原因码 `unknown_decision` | `crates/miyu-kernel/src/session/approval.rs` 的 `answer` |
| `ErrorClass` | 不重试 | `crates/miyu-kernel/src/session/retry.rs` 的 `retryable` |
| `RestoreAction`、`RestoreOutcome` | 改回文件时不当「改成了」：下一次撤销、恢复照上一次的地方找 | `crates/miyu-kernel/src/session/restore.rs` 的 `where_is`（`kernel/history.md`） |
| `Part`、`MessageRole`、`CallResult` | 内核只写不读 | — |

### 出错

读不进来的，交回 serde_json 的错误，原话见上面「读一块」第 2 到 5 条；在一行事件里的，还要套上外壳的报法（`kernel/events.md`「出错」）。这些报错只有中文和 serde_json 的英文原话，给查问题的人看。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/block/tests.rs` | 五种读写一字不差、认得出种类（`every_block_from_the_drawing_round_trips`）；驱动私有数据一字不差（`private_data_is_kept_byte_for_byte`）；第 11、12 条工具名和参数照原文（`tool_call_keeps_name_and_args_as_the_model_wrote_them`）；第 7 条（`unknown_block_is_kept_byte_for_byte`）；第 3、5 条的坏写法（`broken_blocks_are_errors`） |
| `crates/miyu-kernel/src/text_enum/tests.rs` | 第 17、18 条：认识的读成对应的一种、不认识的原样留着、只收字符串 |
| `crates/miyu-kernel/src/event/*/tests.rs`、`crates/miyu-kernel/src/tool/tests.rs` | `Level`、`EndReason`、`ToolStatus`、`Decision`、`ErrorClass`、`Access` 每个认识的值读成自己那一种，不认识的原样留着（`each_…_reads_into_its_own_variant`、`an_unknown_…_is_kept_as_it_is`、`access_is_written_as_text_and_unknown_kinds_are_kept`）；`RestoreAction`、`RestoreOutcome` 读几个认识的、不认识的原样留着（`every_field_reads_back_as_written`、`a_new_action_or_outcome_is_kept_as_it_is`）；`Part`、`MessageRole`、`CallResult` 只查了几个值的写法，没有不认识的值的测试 |
| `crates/miyu-kernel/src/tool/tests.rs` 的 `writing_files_and_unknown_kinds_count_as_writing`；`crates/miyu-kernel/src/session/tests/permission.rs` 的 `an_unknown_level_is_rejected`；`crates/miyu-kernel/src/facts/tests.rs` 的 `the_permission_block_names_the_level_in_effect`；`crates/miyu-session/src/guard/tests.rs`；`crates/miyu-kernel/src/session/tests/approval.rs` 的 `answers_that_do_not_fit_are_rejected` | 第 19 条表里的 `Access`、`Level`、`Decision` 几行 |

### 出处

- `03-事件模型.md` 第四节：内容块，每种块的写法，工具调用存原文，调用编号由内核分，不认识的原样留着。
- `03-事件模型.md` 第八节：格式演进，可以加字段、加种类；第九节：供应商的原样数据。
- `03-事件模型.md` 第三节「会话与回合的事件怎么写」：取值读到不认识的，原样留着，当什么由用到它的地方决定。
- `03-事件模型.md` E1（工具调用是内容块）、E2（驱动私有附件）、E4（大内容存成 blob）。

### 还没有的

- 图片块、文件块读写都有了，还没有哪里造：附件（`22-命令行.md` 第三节）、`read` 读图片和 PDF（`26-提示词.md` 第十节登记簿 `read.json` 那一行）做到时才有。那时照块上的规矩：进来时量好宽和高，量不出尺寸的当文件。
- 超长的工具输出存成 blob、事件里只放引用（`03-事件模型.md` 第三节「大输出的全文」，`08-上下文投影.md` C9）。
- 中途换供应商时，思考按策略降级成普通文本（`03-事件模型.md` 第九节）。
