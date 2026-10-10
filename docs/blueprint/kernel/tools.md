## 内核眼里的工具

### 是什么

工具由软件包提供，内核不内置。内核对一件工具只要知道两样：访问类别，定它能不能和别的调用一起跑、只读时拦不拦；参数格式，照它修正模型写坏了的参数。没派出去、没跑完的调用，内核替工具写一句给模型看的英文，连同给人看的说法一起记进 `tool.result`：每个调用都要有结果。

工具完整的规格（名字、说明、参数格式、访问类别）、一次调用要碰的路径、交给工具的和工具交回的、效果怎么报，在工具接口那一层（`crates/miyu-tool`，`tools/interface.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/tool.rs` | 访问类别 `Access`、内核要知道的一件工具 `ToolRule`、参数修正 `repair` |
| `crates/miyu-kernel/src/tool/texts.rs` | 内核替工具写的十三句 `ToolTexts`、它们的原文 `ToolTextSources`、写成的一句 `Worded` |
| `crates/miyu-kernel/src/event/tool.rs` | `tool.result`、状态 `ToolStatus`、给人看的说法 `Said`（`kernel/events-bodies.md`） |
| `crates/miyu-kernel/src/session/policy.rs` | 冻结在会话上的 `tools`（工具名到 `ToolRule`）和 `tool_texts` |
| `crates/miyu-kernel/src/session/tools.rs`、`approval.rs`、`question.rs`、`interrupt.rs`、`restart.rs`、`load.rs` | 什么时候写哪一句（`kernel/session.md`、`kernel/asking.md`） |
| `crates/miyu-policy/src/tools.rs` | 造策略时，把快照里的工具面拆成组装器的 `ToolSpec` 和内核的 `ToolRule`（`policy.md`） |
| `resources/core/tool-results/*.txt` | 给模型看的原文：内核写的十三句，执行器写的两句 |
| `resources/core/permissions/*.txt` | 权限策略拒绝时写的两句 |
| `resources/core/human/{zh,en}.json` | 给人看的字：上面每一句一条 |

### 对外的样子

**访问类别** `Access`：JSON 里是字符串，取值读到不认识的原样留着（`kernel/blocks.md`）。

| 值 | 是什么 | `writes()` | 派的时候 | 自带的工具 |
|---|---|---|---|---|
| `read` | 只读 | 假 | 和挨着的只读调用一起派 | `read`、`glob`、`grep` |
| `write` | 写文件 | 真 | 一个接一个 | `write`、`edit`、`trash` |
| `execute` | 执行命令 | 假 | 一个接一个 | `shell` |
| `network` | 访问网络 | 假 | 一个接一个 | 没有 |
| `outbound` | 对外发消息 | 假 | 一个接一个 | 没有 |
| 不认识的 | 新版本加的 | 真 | 一个接一个 | — |

- `writes()` 为真的，只读生效时内核当场拦下。别的类别只读时怎么判，是执行前的链的事（`session/guard.md`）。
- 请人确认时，`tool.approval_requested` 也写明要的是哪一类访问，取值一样。

**一件工具** `ToolRule`：

| 格 | 是什么 |
|---|---|
| `access` | 访问类别 |
| `parameters` | 参数的 JSON Schema，原样的 JSON，和工具面上的那一份是同一份 |

会话的策略里照工具名存着每件工具的 `ToolRule`（`Policy.tools`）：不在里面的名字是模型编的。造策略时从策略快照的工具面拆出来，工具面上两件同名的，造不出策略（`policy.md`）。

**参数修正** `repair(parameters, args)`：照参数格式修正模型给的参数原文，交回一个 JSON 对象的原文；参数不是 JSON 对象的，交回 `NotAnObject`。

**内核替工具写的几句** `ToolTexts`：十三句，各是一份读好的模板（`crates/miyu-kernel/src/template.rs`）。

- `ToolTexts::new(ToolTextSources)`：交进十三份原文，读好以后拿字段试换一次，模板坏了、要了不该有的字段的，交回 `TemplateError`。
- 每一句一个方法：`unknown(name)`、`not_an_object(name)`、`cancelled_before()`、`cancelled_running()`、`skipped()`、`read_only()`、`denied(reason)`（没有理由的写 `denied`，有的写 `denied-with-reason`）、`unattended()`、`question_interrupted()`、`question_voided()`、`question_unattended()`、`restarted()`。
- 每个方法交回 `Worded`：`text` 给模型看的字；`said` 给人看的说法，编号是 `core/tool-results/<那一句的名字>`，字段和给模型的那一句一样。内核写的都有说法；执行前的链拒绝时模块没交说法的，`said` 是空的。
- 这十三句的原文存在策略快照里：造会话时从资源目录读进快照，会话的策略照快照造 `ToolTexts`，冻结在会话上。资源目录里的字改了，老会话照旧（`policy.md`）。

### 怎么走

**查一个调用**：回复里的每个调用，照先后查（`crates/miyu-kernel/src/session/tools.rs` 的 `start_tools`，`kernel/session.md`）：

1. 请求发出去以后有人急着插过话（插话记在回合上，下一次请求发出去时清掉）：这条回复里的调用不查，每个都记 `skipped`。
2. 工具面上没有这个名字：记 `unknown`。
3. 参数修正交回 `NotAnObject`：记 `not-an-object`。
4. 这件工具 `writes()` 为真，而且只读正生效：记 `read-only`。
5. 都过了：修正过的参数留着，等回复落了盘，交给执行前的链。

**参数修正**：

6. 原文去掉前后的空白是空的：当成 `{}`。有的供应商给没有参数的调用发空字符串。
7. 原文读不成一个 JSON 对象（不是 JSON，或者是数组、`null`、字符串、数字、半截的）：`NotAnObject`。
8. 参数格式读不成 JSON，或者里面没有 `properties` 这个对象：原文照交。
9. 只看 `properties` 里声明了 `type`、而且 `type` 是一个字符串的顶层参数；参数里这一格的值又是字符串的，去掉前后的空白，照声明的类型试着还原：

| 声明的 `type` | 能还原的 | 还原成 |
|---|---|---|
| `array` | 以 `[` 开头，读得成 JSON 数组 | 那个数组 |
| `object` | 以 `{` 开头，读得成 JSON 对象 | 那个对象 |
| `integer` | 读得成 64 位整数，可以带 `+`、`-` | 那个整数 |
| `number` | 读得成有限的数（`1e3`、`.5` 也算；`NaN`、`inf` 不算） | 那个数，写成小数：`"20"` 还原成 `20.0` |
| `boolean` | 不分大小写的 `true`、`false`：模型发过 Python 风格的 `"False"` | 布尔 |

表里有测试的是 `" 0.5 "`、`"20"`（整数）、`"False"`、数组、对象和几种还原不了的（`NaN` 在里面）；带 `+` 的整数、`1e3`、`.5`、`inf`，和 `"20"` 声明成 `number` 时写成 `20.0`，是照 Rust 标准库和 serde_json 的行为写的，没有测试证实。

10. 还原不了的，原样留着，不报错：`{"limit":"twenty"}` 照交，错由工具自己报。
11. 声明成字符串的，一个字节都不碰，看着像 JSON 也不碰。声明的类型不在上表里的、`type` 写成数组的、没写 `type` 的、`properties` 里没有的，也不碰。
12. 换了哪怕一处，整个对象重新写一遍：紧凑的 JSON，键照字母的先后，没换的值也跟着重写。`{"path":"a","limit":"5"}` 修正以后是 `{"limit":5,"path":"a"}`。
13. 一处都没换的，原文照交，一个字节不变：`{ "limit": 20 }` 还是 `{ "limit": 20 }`。
14. 修正过的只用在执行上：执行前的链、工具拿到的是它；日志里的调用照模型给的原文记，发回去的字节不变。
15. 修正只还原类型，不查必填的有没有、多写了什么：内核只说自己真正知道的，别的交给工具执行时报。

**派的先后**：照调用的先后（`crates/miyu-kernel/src/session/step.rs` 的 `ready`）。

16. 不是 `read` 的（包括不认识的类别）：前面的调用都有了结果才派；它没有结果，后面的都等着。
17. `read` 的：前面没有还没结果、又不是 `read` 的调用，就派，和挨着的只读调用一起跑。
18. 在过执行前的链的、在等人确认的，和在跑的一样占着位置。

**内核写的十三句**：什么时候写哪一句（每一种情况细写在 `kernel/session.md`、`kernel/asking.md`）。

| 那一句 | 什么时候 | 状态 | `by` |
|---|---|---|---|
| `unknown` | 工具面上没有这个名字 | `error` | 内核 |
| `not-an-object` | 参数不是 JSON 对象 | `error` | 内核 |
| `read-only` | 只读时要写入的：回复一到就拦；收紧成只读时，这一步里还没跑过、要写入的（工具的类别是写入，或者请人确认的是写入）；有人能确认、只读生效，执行前的链却要问人、要的是写入 | `denied` | 内核 |
| `unattended` | 执行前的链要问人，这里没有人能确认 | `denied` | 内核 |
| `denied` | 人拒绝了，没写理由。空白的理由当没写 | `denied` | 拒绝的人 |
| `denied-with-reason` | 人拒绝了，写了理由 | `denied` | 拒绝的人 |
| `cancelled-before` | 打断时还没跑过的：没轮到、在过链、在等人确认、人允许了还没派；请求在路上时打断，半截回复里留下的调用 | `cancelled` | 打断的人 |
| `cancelled-running` | 打断时在跑的；答完了题、等着交给工具的也算 | `cancelled` | 打断的人 |
| `question-interrupted` | 打断时正问着人 | `cancelled` | 打断的人 |
| `skipped` | 急着插话时还没跑过的（在过链、在等人确认、人允许了还没派的也算）；请求在路上时急着插话，那条回复里的调用；等人确认的时候来了一句话 | `skipped` | 说话的人 |
| `question-voided` | 正问着人的时候来了一句话 | `skipped` | 说话的人 |
| `question-unattended` | 在跑的调用要问人，这里没有人能回答 | `skipped` | 内核 |
| `restarted` | 有计划的重启时还没有结果的；载入时发现崩了，还没有结果的 | `cancelled` | 内核 |

- 内核写的结果只有一块文字，就是那一句；没有用时（没真执行过），没有效果。
- `cause`：人引起的（拒绝、打断、急着插话、发话作废）是那个命令；别的是那一轮的。
- 执行器交回的结果只有 `ok` 和 `error` 两种，`by` 是那次调用（`kernel/events-bodies.md`）。

**字段怎么换进去**：

19. 给模型看的：每个字段先转义（`crates/miyu-kernel/src/template.rs` 的 `escape`）：照 JSON 字符串的写法，反斜杠、换行、回车、制表写成 `\\`、`\n`、`\r`、`\t`，别的控制字符写成 `\u001b` 这样；`"`、`&`、`<`、`>` 和 U+2028、U+2029 也写成 `\u` 加四位小写十六进制。转出来是一行字，没有引号、没有尖括号：模型编的工具名、人写的理由伪造不了标签，也伪造不了一行记录。例如工具名 `x"><tool` 写成 `There is no tool named "x\u0022\u003e\u003ctool".`。
20. 给人看的说法里，字段是原样的值，不转义（换成字时怎么清理，见 `store.md`）。

### 样子

给模型看的原文，一份一个文件，行尾一个换行，换行也算在字里。每一份都登记在登记簿里（`26-提示词.md` 第十节）：进到哪、什么时候加进来、多少 token、为什么加、指纹。

`resources/core/tool-results/` 下内核写的十三句：

| 文件 | 原文 |
|---|---|
| `unknown.txt` | `There is no tool named "{name}".` |
| `not-an-object.txt` | `The arguments for "{name}" are not a JSON object.` |
| `read-only.txt` | `The call was not run: the session is read-only.` |
| `unattended.txt` | `The call was not run: it needs the user's approval, which no one can give here.` |
| `denied.txt` | `The call was not run: the user denied it.` |
| `denied-with-reason.txt` | `The call was not run: the user denied it and said "{reason}".` |
| `cancelled-before.txt` | `The call was cancelled before it ran: the user interrupted the turn.` |
| `cancelled-running.txt` | `The call was cancelled while it was running: the user interrupted the turn. It may have been partly done.` |
| `question-interrupted.txt` | `The question was not answered: the user interrupted the turn.` |
| `skipped.txt` | `The call was skipped: the user sent a new message.` |
| `question-voided.txt` | `The question was not answered: the user sent a new message instead.` |
| `question-unattended.txt` | `The question was not answered: no one can answer here.` |
| `restarted.txt` | `The call was cancelled: Miyu restarted before it finished. It may have been partly done.` |

同一处还有几句，不是内核写的，编号的写法一样：

| 文件 | 原文 | 谁写、什么时候 | 状态、`by` |
|---|---|---|---|
| `tool-results/unavailable.txt` | `The tool "{name}" is not available right now.` | 执行器：快照里有、核心的工具目录里没有这件（`session/actor.md`） | `error`，那次调用，没有用时 |
| `tool-results/crashed.txt` | `The tool "{name}" stopped because of an internal error. It may have been partly done.` | 执行器：工具执行时崩了 | `error`，那次调用，用时算到崩为止 |
| `permissions/forbidden.txt` | `"{path}" is inside Miyu's own data, which no tool can read or change.` | 权限策略：要碰的路径在数据根里（`session/guard.md`） | `denied`，模块 `permissions` |
| `permissions/unresolvable.txt` | `Can't tell where "{path}" points: {reason}.` | 权限策略：路径换不成真实的位置 | `denied`，模块 `permissions` |

权限策略在只读时要写的，也用 `read-only` 那一句，`by` 是模块 `permissions`。

### 给人看的字

`resources/core/human/{zh,en}.json` 的 `said` 下，编号去掉前面的 `core/`：

| 说法 | 中文 | 英文 |
|---|---|---|
| `core/tool-results/unknown` | 工具不存在 | Tool not found |
| `core/tool-results/not-an-object` | 参数格式错误 | Invalid arguments |
| `core/tool-results/read-only` | 只读模式，未执行 | Read-only mode, not run |
| `core/tool-results/unattended` | 需要确认，无人确认 | Needs confirmation, no one to confirm |
| `core/tool-results/denied` | 已拒绝 | Denied |
| `core/tool-results/denied-with-reason` | 你拒绝了：{reason} | you said no: {reason} |
| `core/tool-results/cancelled-before` | 已取消 | Cancelled |
| `core/tool-results/cancelled-running` | 已中断 | Interrupted |
| `core/tool-results/question-interrupted` | 已中断，未回答 | Interrupted, not answered |
| `core/tool-results/skipped` | 已跳过 | Skipped |
| `core/tool-results/question-voided` | 已作废 | Voided |
| `core/tool-results/question-unattended` | 无人回答 | No one to answer |
| `core/tool-results/restarted` | 重启中断 | Interrupted by restart |
| `core/tool-results/unavailable` | 不可用 | not available right now |
| `core/tool-results/crashed` | 内部错误，可能已部分执行 | Internal error, may be partly done |
| `core/permissions/forbidden` | 这是 Miyu 自己的数据，谁都不能碰 | Miyu's own data, off limits |
| `core/permissions/unresolvable` | 说不清它指向哪里：{reason} | can't tell where it points: {reason} |

- 给人看的只用给模型的那一句有的字段，可以少用：`unknown` 给模型的有 `name`，给人的不用。
- 没有这种界面语言的，照英文那一份（`store.md`）。

### 出错

- `repair` 只有一种错：参数不是 JSON 对象（`NotAnObject`），内核把它写成 `not-an-object` 那一句，这个调用不派。
- `ToolTexts::new` 交回 `TemplateError`：模板的 `{` 没配上、单独一个 `}`、字段的名字不合写法，或者要了不该有的字段（`unknown`、`not_an_object` 只能要 `name`，`denied_with_reason` 只能要 `reason`，别的十句一个都不能要）。报错是英文：「bad template: missing field tool」这样。出厂的原文造不出来，会话就造不出来（`policy.md`）。
- 方法本身不会出错：造的时候已经试换过。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/tool/tests.rs` | 第 9 条每一种还原（`each_declared_shape_is_restored_from_a_string`）；第 10、11、13 条（`what_cannot_be_restored_or_is_a_string_is_left_alone`）；第 12 条（`only_changed_arguments_are_rewritten_and_the_rest_are_kept`）；第 6 条（`empty_arguments_are_an_empty_object`）；第 7 条（`arguments_that_are_not_an_object_are_refused`）；第 8 条（`a_schema_without_properties_passes_arguments_through`）；访问类别的写法和 `writes()` |
| `crates/miyu-kernel/src/tool/texts/tests.rs` | 第 19 条转义工具名和理由；要了别的字段的拒收；带不带理由；每一句都带着说法，编号和字段对得上 |
| `crates/miyu-kernel/tests/resources.rs` | 出厂的十三句读得进、换得出（`the_tool_result_sentences_are_usable`）；样本 71 号就是带理由的那一句（`the_sample_denial_is_the_sentence_with_the_reason`） |
| `crates/miyu-kernel/src/session/tests/tools.rs` | 第 2、3 条当场记下、说法对（`unknown_tools_and_bad_arguments_are_answered_on_the_spot`）；第 14 条（`arguments_are_repaired_for_running_but_logged_as_given`）；第 16 条（`calls_that_are_not_read_only_run_alone_and_in_order`） |
| `crates/miyu-kernel/src/session/tests/interrupt.rs`、`approval.rs`、`question.rs`、`restart.rs`、`load.rs`、`permission.rs` | 十三句那张表：每一种情况写哪一句、什么状态、`by` 是谁 |
| `crates/miyu-store/tests/human.rs` | `core/tool-results/`、`core/permissions/` 下每一份，中文、英文都有给人看的一句，要的字段不多于给模型的那一句 |
| `xtask/src/ledger.rs`（门禁「文档」） | `resources/` 下每一份给模型看的字都在登记簿里，指纹对得上 |

### 出处

- `05-内核接口.md` 第六节：工具的规格，`access`，能否并发由 `access` 推出，畸形参数由内核统一修正，给人看的说法。
- `02-内核.md` 第六节：「工具怎么调、下一步怎么走」「打断和急着插话」「权限级别怎么切」「确认怎么走」「提问怎么走」「载入、崩溃、重启」。
- `03-事件模型.md` 第三节「消息和工具结果怎么写」：已取消、被拒绝、已跳过的也有给模型看的内容；执行之前就拦下的是 `error`。
- `08-上下文投影.md` 第五节「模板与转义怎么写」、C7。
- `26-提示词.md` 第三节（双槽、机械文字一律英文短句）、J3、J12、第十节登记簿。
- `11-权限与沙盒.md` A11（没有确认界面的场所）、A12（只读）、A13（拒绝以后她接着干）。
- `10-自带软件.md` 第一节：内核不内置工具。

### 还没有的

- 规格里给内核看的另外几格：`timeout` 默认超时、`venues` 哪些场所能用、`group` 按组加载属于哪一组（`05-内核接口.md` 第六节）。
- 存根和按组加载：直接调存根时照完整格式校验，错了在报错里附上整组的契约（`25-工具加载.md` 第二节、W6）。现在内核只查参数是不是 JSON 对象。
- 参数类型不对时报「期望整数，收到字符串 "1"」这样的错（`05-内核接口.md` 第六节）：现在还原不了的原样交给工具。
- 执行时交给工具的调用编号、会话、身份、沙盒范围、截止时间（`05-内核接口.md` 第六节，`tools/interface.md`）。
- `network`、`outbound` 两类还没有工具。
