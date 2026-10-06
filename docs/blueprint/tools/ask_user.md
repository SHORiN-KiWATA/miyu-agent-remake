## `ask_user`

### 是什么

问人（施工 D-2，`10-自带软件.md` 第三节）：她干活中途遇到要人拿主意的事，问一组题、等人回答，回答成了这次调用的结果。参数照 Claude Code 的 AskUserQuestion（`10-自带软件.md` 第十节、B12），题数、选项数都不设上限（2026-09-26 项目主人定）。题目、回答记成 `question.asked`、`question.answered`，头照它们画抽屉，人用 `session.answer` 回答（`protocol.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/ask_user.rs` | 参数、题交给端口、回答写成结果 |
| `crates/miyu-tool/src/questions.rs` | 提问的端口 `QuestionPort`、那件工具的名字 `ASK_USER`（`tools/interface.md`） |
| `crates/miyu-session/src/tools/questions.rs` | 执行器这一头：造端口、题交给内核、回答送回来（`session/tools.md`「1e. 问人」） |
| `crates/miyu-session/src/agents.rs` 的 `Agents::asks` | 谁能问人：工具面、端口都照它 |
| `resources/software/basesystem/tools/ask_user.json` | 说明和参数格式 |
| `resources/software/basesystem/ask_user/*.txt` | 结果里给她看的几句 |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：只问人，什么都不改，只读开着也能问。说明照 `26-提示词.md` 附录的草稿，一字不差；参数说明每格一句，名字看得出来的不写（`25-工具加载.md` W2）。

样本 `resources/software/basesystem/tools/ask_user.json`：

```json
{
  "description": "Ask the user one or more questions with options and wait for the answers. Put the recommended option first and add \"(Recommended)\" to its label. Don't add an \"other\" option: the user can always type their own answer.",
  "parameters": {"type":"object","properties":{"questions":{"type":"array","items":{"type":"object","properties":{"question":{"type":"string"},"header":{"type":"string","description":"A short tag shown above the question."},"options":{"type":"array","items":{"type":"object","properties":{"label":{"type":"string"},"description":{"type":"string"},"preview":{"type":"string","description":"Text shown in monospace while the option is focused, such as code or a sketch."}},"required":["label"]}},"multiSelect":{"type":"boolean","description":"Allow picking more than one option."}},"required":["question"]}}},"required":["questions"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `questions` | 是 | 一组题，照先后；一道都没有的是参数不对 |
| `questions[].question` | 是 | 问的话，原样 |
| `questions[].header` | 否 | 顶上那一排标签里的短名字 |
| `questions[].options` | 否 | 几个选项，可以一个都没有：人自己写 |
| `questions[].options[].label` | 是 | 一行标题，回答里写的就是它 |
| `questions[].options[].description` | 否 | 一行说明 |
| `questions[].options[].preview` | 否 | 一段文字画，头照等宽字显示 |
| `questions[].multiSelect` | 否 | 能多选；记进事件是 `multiple` |

- 别的参数不认，也不报错。
- 一条路径都不报：权限策略照访问类别判，读的放行。
- **谁有**：只有能问人的会话（`Agents::asks`：有人能回答、本机、主会话）。子会话问父会话（`agents.md` 第十条）；`miyu ask` 开的、场所会话（QQ）没有提问的界面（`11-权限与沙盒.md` A11），她有问题就在对话里问（`session/tools.md`「工具面」）。

**结果**（给她看的）：一道一行 `"<问的话>" = <回答>`（`ask_user/answer.txt`）。回答是选了的标题、自己写的，逗号隔开；`picked`、`text` 都没有的写 `(no answer)`（`no-answer.txt`）；有备注的接在后面 ` (note: <备注>)`（`note.txt`）。最后一行 `Go on with these answers in mind.`（`end.txt`）。照 Claude Code 的结果改成一行一道：长回答也读得清。

```text
"用哪个？" = 甲 (note: 先这样)
"叫什么？" = 小美
"还有吗？" = (no answer)
Go on with these answers in mind.
```

### 怎么走

1. 读参数：读不成的（少了 `questions`、选项没有 `label`、类型不对）、一道题都没有的，交回参数不对的那一句，端口一次都不问。
2. 没有端口（这个会话不能问人；工具面上本来就不给，兜底）：出错，`No one can answer questions here. Ask in your reply instead.`（`unattended.txt`）。
3. 题交给端口，等回答。不设超时：人不回就一直等（`02-内核.md` 第六节「提问怎么走」）。
4. 答了：照「结果」写成一段字，成功。
5. 没答到就了结了（打断、等的时候来了一句话、收紧成只读、核心停了）：照叫停收场；这次调用的结果由内核照原来的规矩补（`kernel/asking.md`），这里交回的不再记。

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 显示名 `ask_user` | 提问 | Ask |
| `ask_user/answered` | 你答了 | You answered |
| `ask_user/unattended` | 这里没人能回答 | No one can answer here |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-basesystem/tests/ask_user.rs` | 结果一道一行逐字节比（选一项、多选带备注、自己写、没答）；题原样交给端口，`multiSelect` 记成 `multiple`、`preview` 带着；参数不对的四种端口一次都不问；没有端口的那一句；没答到照叫停收场；访问类别读 |
| `crates/miyu-session/tests/questions.rs` | 交题以后内核记 `question.asked`、回答落了盘工具收到；等的时候打断只有一条结果；没人能回答的会话没有端口；工具面只有能问人的会话有 `ask_user`、只少这一件 |
| `crates/miyu-endpoint/tests/questions.rs` | 真核心、真工具：`session.answer` 交回答（带 `notes`），她收到的结果对；选了没有的选项 `bad_answer`、什么都没记 |
| `crates/miyu-kernel/src/event/question/tests.rs` | `preview`、`notes` 读写原样，以前的日志照旧读得懂 |

### 出处

- `10-自带软件.md` 第三节（`ask_user` 的行）、第十节、B12；`26-提示词.md` 附录（说明的草稿）。
- `02-内核.md` 第六节「提问怎么走」；`03-事件模型.md` 第三节「提问的事件怎么写」。
- `11-权限与沙盒.md` A11：没有确认界面的场所也没有提问工具。

### 还没有的

- Claude Code 2.1.280 加的 `kind`（填数字、填字的题）、整组的 `title`：等头要了再加。
