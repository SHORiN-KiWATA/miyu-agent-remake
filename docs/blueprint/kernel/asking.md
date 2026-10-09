## 确认和提问

### 是什么

派一个调用之前，内核先把它交给执行前的链（权限策略和各扩展的守卫）判：放行、拒绝，或者要问人。要问人的，这个调用停下来，等人确认。在跑的调用也能问人一组题，等人回答，回答落了盘交给它。

这一页写链的结论、确认、提问，和在等的调用遇上来了一句话、打断、收紧成只读、重启、崩了时怎么了结。会话别的部分在 `session.md`，账本怎么查这几种事件在 `history.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/session/approval.rs` | 链的结论；人的确认 |
| `crates/miyu-kernel/src/session/grants.rs` | 本会话放行过的规则：每追加一条记一次，交给链（施工 D-1） |
| `crates/miyu-kernel/src/session/question.rs` | 在跑的调用问人；人的回答；来了一句话，在等人的作废 |
| `crates/miyu-kernel/src/session/step.rs` | 每个调用走到了哪 |
| `crates/miyu-kernel/src/session/tools.rs` | 交给链、允许了派、答完了交回答；打断、插话、收紧成只读时补结果 |
| `crates/miyu-kernel/src/session/input.rs` | `Verdict`、`Answer` |
| `crates/miyu-kernel/src/event/tool.rs`、`event/question.rs` | 请求、决定、题目、回答；回答对不对得上题目（`fits`） |
| `resources/core/tool-results/` | 给模型看的六句 |
| `resources/core/human/{zh,en}.json` | 这六句给人看的说法 |

### 对外的样子

**链的结论**（`Input::ToolGuarded { at, call_id, verdict }` 的 `Verdict`）：几个守卫合起来，最严者胜，由执行器合好交回；守卫超时的按拒绝交回。

| 结论 | 带着 |
|---|---|
| `Allow` | — |
| `Deny { module, text, human }` | 哪个模块拒的；写给模型的那一句；给人看的说法，模块没交的是空的 |
| `Ask { module, access, rule, detail }` | 哪个模块问的；要的是哪一类访问（`read`、`write`、`execute`、`network`、`outbound`，不认识的原样留着）；提的放行规则，没提就没有；给头看的为什么要问。`rule`、`detail` 是 JSON，内核原样记，不看里面 |

**回答**（`Command::Answer { call_id, answer }`，`session.answer`）：

| 回答 | 带着 |
|---|---|
| `Answer::Approval { decision, reason }` | 选了哪一项：`once` 允许这一次、`session` 本会话都允许、`workspace` 这个工作区以后都允许、`deny` 拒绝，不认识的原样留着；拒绝的理由，只有拒绝能带 |
| `Answer::Questions(回答)` | 照题目的先后一道一条：`Response { picked, text, notes }`，选了哪几项（写选项的标题）、自己写的、补的一句备注（施工 D-2）。`picked`、`text` 都没有的，这道没答 |

**题目**（`Input::ToolAsks { at, call_id, questions }`）：一组题，照先后，几道都行。一道题 `Question { header, question, options, multiple }`：顶上标签里的短名字（可以不写）、问的话、几个选项（`Choice { label, description, preview }`，一行标题、一行说明、一段文字画，后两样可以不写；选项可以一个都没有）、能不能多选。施工 D-2 起会问的工具是 `ask_user`（`tools/ask_user.md`）。

**动作**：`GuardTool { call_id, name, args, cwd, permission }` 交给链；`RunTool { call_id, name, args, cwd, permission, asked }` 派去跑，带着派出去那一刻实际生效的那一级（施工 5-4 上）；`AnswerTool { call_id, answers }` 把回答交给在等的调用；`CancelTool { call_id }` 叫停（`session.md`）。

**事件**：

| 事件 | `body` | `by` | `cause` |
|---|---|---|---|
| `tool.approval_requested` | `call_id`、`access`、`rule`、`detail`（后两样没有就不写） | 问的模块 | 回合的 |
| `tool.approval_decided` | `call_id`、`decision`、`reason`（拒绝时写了才有） | 回答的人 | 回答的命令 |
| `question.asked` | `call_id`、`questions` | 那次调用 | 回合的 |
| `question.answered` | `call_id`、`answers` | 回答的人 | 回答的命令 |

**一个调用走到了哪**（`step::State`）：

| 状态 | 是什么 | 归哪一类 |
|---|---|---|
| `Waiting` | 还没轮到 | 没跑过 |
| `Guarding` | 交给了链，等结论 | 没跑过 |
| `Asking` | 链说要问人，等人确认 | 没跑过；等人 |
| `Approved { decided }` | 人允许了，等那条决定落盘 | 没跑过 |
| `Running` | 派出去了，等结果 | 在跑 |
| `Questioning` | 跑着的时候问了人，等人回答 | 在跑；等人 |
| `Answered { answered }` | 人答完了，等那条回答落盘 | 在跑 |
| `Done` | 有了结果 | — |

过链的、等人的、允许了还没派的、问着人的，都和在跑的一样占着位置（`session.md`「调工具」第 4 条）。请人确认过的，调用上记着请求要的那一类访问、提没提规则。

### 怎么走

**执行前的链**：

1. 每个调用轮到时都先过链，完全放开时也过：出 `GuardTool`，带上工具名、修正过的参数、这一轮的工作目录、实际生效的那一级。调用到 `Guarding`。
2. 结论只收这一步里在 `Guarding` 的调用的；别的不理：被打断、跳过、拦下以后迟到的，已经在跑的，没有这个调用的。
3. `Allow`：调用到 `Running`，当场出 `RunTool`，不记事件。
4. `Deny`：记一条 `denied` 的结果：`by` 是拒绝的模块，`cause` 是回合的，内容是模块写的那一句，说法是模块交的。
5. `Ask`，照这个先后判：
   1. 策略里没人能确认（`attended` 是假的）：不记请求，记一条 `denied` 的结果，`by` 是内核，那一句是 `unattended`。
   2. 实际生效的是只读，要的是写入（`write`，或者不认识的）：不记请求，记一条 `denied`，`by` 是内核，`read-only`。
   3. 别的：记一条 `tool.approval_requested`，调用到 `Asking`，停下来等人。
6. 记了结果的，这一步因此齐了就往下走（`session.md`「调工具」第 7 条）；然后派后面轮到的。
7. 内核只记放行规则，不判：选了 `session`、`workspace` 以后怎么放行，是链照日志去用的事。

**人确认**（`Answer::Approval`）：

1. 理由去掉空白是空的，当没写。
2. 照这个先后判，拒绝的什么都不记：
   1. 这个调用不在这一步里等确认（没问过、答过了、已经有了结果，或者它等的是回答题目）：`not_asking`。
   2. 选项不认识：`unknown_decision`。
   3. 请求没提规则，选了 `session`、`workspace`：`no_rule`。
   4. 不是拒绝，却带了理由：`unexpected_reason`。
3. 记一条 `tool.approval_decided`。
4. 允许（`once`、`session`、`workspace`）：调用到 `Approved`，那条决定落了盘才出 `RunTool`。
5. 拒绝（`deny`）：再记一条 `denied` 的结果，`by` 是拒绝的人，`cause` 是这个命令；写了理由的用 `denied-with-reason`，理由照模板转义换进去，没写的用 `denied`。这一步照常往下走，她看到结果接着干；这一步因此齐了的，往下走。
6. 回应附上这一次追加的全部；然后派后面轮到的。
7. 不设超时：人不回就一直等，打断随时进得来。

**在跑的调用问人**（`ToolAsks`）：

1. 只收这一步里 `Running` 的调用问的；别的不理：还没派的、已经在问的、答完了还没交回回答的、没有这个调用的。
2. 没人能回答（`attended` 是假的）：不记题目，调用有了结果：`skipped`，`by` 是内核，`cause` 是回合的，那一句是 `question-unattended`；出 `Append`、`CancelTool`，这一步因此齐了的往下走，再派后面轮到的。之后这个调用交回的结果不理。
3. 别的：记一条 `question.asked`，调用到 `Questioning`，记着题目。
4. 几道题都行，不设上限。答完了、交回给它以后，同一个调用还能再问。

**人回答**（`Answer::Questions`）：

1. 自己写的去掉空白是空的，当没写。
2. 这个调用不在这一步里等回答（没问过、答过了、已经有了结果，或者它等的是确认）：`not_asking`。几个头同时回答，先答的记下，后到的就是这一种。
3. 回答对不上题目：`bad_answer`。对得上是：几道题几条；每一条选的都是那道题的选项标题，不重复；单选的至多选一项。
4. 记一条 `question.answered`（空的自己写已经去掉），调用到 `Answered`；回应只附这一条。
5. 那条回答落了盘，出 `AnswerTool`，调用回到 `Running`，之后照常等它交回结果。

**在等的怎么了结**：调用一有了结果，在等的请求、题目就了结了（账本随之清掉，`history.md`）；之后到的结论不理，之后到的回答拒绝，`not_asking`。

| 什么时候 | 在过链 | 等人确认 | 允许了还没派 | 问着人 | 答完了等落盘 |
|---|---|---|---|---|---|
| 来了一句话（`session.send`） | 照常 | `skipped`，`skipped` | 照常 | `skipped`，`question-voided`，叫停 | 照常 |
| 急着插话 | `skipped`，`skipped` | 同上一行 | `skipped`，`skipped` | 同上一行 | 照常 |
| 打断 | `cancelled`，`cancelled-before` | 同左 | 同左 | `cancelled`，`question-interrupted`，叫停 | `cancelled`，`cancelled-running`，叫停 |
| 收紧成只读 | 工具要写入的：`denied`，`read-only` | 工具要写入、或者请求要写入的：同左 | 同左 | 照常 | 照常 |
| 有计划的重启 | `cancelled`，`restarted` | 同左 | 同左 | 同左，叫停 | 同左，叫停 |
| 崩了，载入时 | `cancelled`，`restarted` | 同左 | 同左 | 同左 | 同左 |
| 交回了结果（`ToolDone`） | 不理 | 不理 | 不理 | 照收，题目了结 | 照收，回答不再交给它 |

- 格里写的是补的结果的状态和那一句；「照常」是不动它；「叫停」是出 `CancelTool`。
- 来了一句话、急着插话：`by` 是说话的人，`cause` 是这句话的命令。急着插话时，先作废等人的，再跳过还没跑的。
- 打断：`by` 是打断的人，`cause` 是打断的命令。
- 收紧成只读、重启、崩了：`by` 是内核，`cause` 是回合的。
- 这一步因此齐了的，往下走。

### 样子：给模型看的

内核写的结果，原文在 `resources/core/tool-results/<名字>.txt`，末尾带一个换行；字段照模板的规矩转义。token 数、指纹在 `26-提示词.md` 第十节的登记簿里。链拒绝时，结果的内容是那个模块写的那一句，不是这里的（权限策略写的三句：`resources/core/permissions/` 的两句，和只读时的 `read-only`，见 `session/guard.md`）。

| 名字 | 什么时候 | 状态 | 原文 |
|---|---|---|---|
| `denied` | 人拒绝了，没写理由 | `denied` | `The call was not run: the user denied it.` |
| `denied-with-reason` | 人拒绝了，写了理由 | `denied` | `The call was not run: the user denied it and said "{reason}".` |
| `unattended` | 要确认，没人能确认 | `denied` | `The call was not run: it needs the user's approval, which no one can give here.` |
| `question-interrupted` | 问着人的时候被打断 | `cancelled` | `The question was not answered: the user interrupted the turn.` |
| `question-voided` | 问着人的时候来了一句话 | `skipped` | `The question was not answered: the user sent a new message instead.` |
| `question-unattended` | 要问人，没人能回答 | `skipped` | `The question was not answered: no one can answer here.` |

请求、决定、题目、回答都不进请求：她看到的只有那次调用的结果（默认的组装不渲染它们，`crates/miyu-assemble/src/render.rs`）。

### 给人看的字

说法是 `core/tool-results/<名字>`，`denied-with-reason` 带字段 `reason`，别的不带。字在 `resources/core/human/{zh,en}.json` 的 `said` 里：

| 名字 | 中文 | 英文 |
|---|---|---|
| `denied` | 你拒绝了 | you said no |
| `denied-with-reason` | 你拒绝了：{reason} | you said no: {reason} |
| `unattended` | 要确认，这里没人能确认 | needs a confirmation nobody here can give |
| `question-interrupted` | 打断了，没回答 | interrupted before the answer |
| `question-voided` | 你发了一句话，这一题作废了 | dropped: you sent a message |
| `question-unattended` | 这里没人能回答 | nobody here can answer |

链拒绝的，说法是模块交的，没交就没有。`not_asking`、`unknown_decision`、`no_rule`、`unexpected_reason`、`bad_answer` 这几个原因码，核心现在还没配专门的话，照「被拒绝了。」「Refused.」说（`crates/miyu-endpoint/src/refusal.rs`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/session/tests/approval.rs` | 每个调用都先过链、带上实际生效的那一级；链拒绝的 `by` 是模块、说法原样；要问人记请求、调用等着；等着的占着位置；允许的决定落了盘才派；拒绝的记决定和结果、空理由当没写、她接着干；四个原因码；没人能确认当场拒绝；收紧成只读拦下要写入的；打断、急着插话时在等的补结果；过时的结论不理 |
| `crates/miyu-kernel/src/session/grants/tests.rs`、`tests/grants.rs`（施工 D-1） | 只记本会话都允许的；没提规则的、对不上请求的不算；照先后、一样的只记一次；交给链的动作带上；载入以后照日志算回来、撤销不收回 |
| `crates/miyu-kernel/src/session/tests/question.rs` | 在跑的调用问人、等人；回答落了盘才交给工具；答完了还能再问；对不上的回答、答错了种类、先答者胜；没人能回答跳过、叫停；打断时问着人的、答完了的；来了一句话作废提问、跳过确认；工具先交回结果了结题目；不是在跑的调用问的不理 |
| `crates/miyu-kernel/src/session/tests/load.rs` | 崩在等确认、等回答的时候，载入时补结果，之后的回答拒绝 |
| `crates/miyu-kernel/src/session/tests/scenario/asking.rs` | 执行器替身跑整轮：她问、工具拿到回答；允许、拒绝两种决定以后她都接着干 |
| `crates/miyu-kernel/src/session/tests/random/asking.rs`、`random/watch/approval.rs`、`random/watch/question.rs` | 随机输入里的结论、题目、回答；每一步查请求只在链要问人时记、没人能确认和只读的当场拒绝、回答照规矩接受或拒绝、`by` 写对 |
| `crates/miyu-kernel/src/event/question/tests.rs` | 回答对不对得上题目 |
| `crates/miyu-kernel/src/tool/texts/tests.rs`、`crates/miyu-kernel/tests/resources.rs` | 拒绝的理由照模板转义、只收 `reason` 一个字段；样本里被拒绝的那一条就是资源里带理由的那一句 |

**本会话放行过的**（施工 D-1，`grants.rs`；`02-内核.md` 第六节「确认怎么走」第 3 条：记住的放行规则由链照日志去用，内核只记，不判）

1. 人回答确认选了 `session`（本会话都允许）：那一次 `tool.approval_requested` 提的 `rule` 记进这个会话放行过的规则，照先后，一样的只记一次。别的决定不记；请求没提规则的，内核本来就拒（`no_rule`）。
2. 照日志算：活着时每追加一条记一次，载入时照日志从第 1 条再走一遍（载入本来就读整份日志）。不另开事件，不进上下文。
3. 撤销、压缩都不收回：放行是人的决定，不是她说的话（Claude Code 回退也不收回权限）。子会话有自己的日志，不继承父会话的。
4. 交给链：每一个 `GuardTool` 带上 `grants`，放行过的规则原样照先后，链照它判（`session/guard.md`「判一次调用」第 7 条）。
5. 请求没等到决定就了结的（打断、跳过、收紧成只读拦下），它的规则留着不再有人认领：调用编号在一个会话里不重复，留着不放行任何东西。

### 出处

- `02-内核.md` 第六节「确认怎么走」「提问怎么走」。
- `03-事件模型.md` 第三节「确认的事件怎么写」「提问的事件怎么写」。
- `05-内核接口.md` 第五节：执行前的链最严者胜，守卫超时按拒绝。
- `11-权限与沙盒.md` 第二节（四个选项）、A11（没有确认界面的场所）、A13（拒绝以后她接着干）。
- `08-上下文投影.md` 第四节：请求和决定、题目和回答都不进上下文。

### 还没有的

- 这个工作区以后都允许：要存进工作区的配置，存哪还没定（`11-权限与沙盒.md` 第二节，2026-09-28 项目主人定以后再加）；协议上先不认这一项（`protocol.md` 的 `session.answer`）。
- 扩展的守卫（`05-内核接口.md` 第五节）：现在链里只有权限策略。
