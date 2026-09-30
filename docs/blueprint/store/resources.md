## 资源目录和给人看的字

### 是什么

出厂的人设、给模型的字、工具的说明、给人看的字，都放在资源目录里，随安装包分发，不编进二进制。这一页写怎么找到它，怎么读出一个人格要用的原文，怎么读给人看的字、照说法换成一句话。数据根另见 `store.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/resources.rs` | 找资源目录；读出一个人格要用的原文、子会话的场所说明 |
| `crates/miyu-store/src/human.rs` | 读给人看的字；照说法换成一句话；换进去的字段把控制字符换成 `�` |
| `crates/miyu-store/src/env.rs` | 找资源目录要看的 `MIYU_RESOURCES`、程序的位置（`store.md`） |
| `resources/` | 源码树里的资源目录，开发时 `MIYU_RESOURCES` 指到它 |

### 对外的样子

| 名字 | 做什么 |
|---|---|
| `ResourceRoot::locate(env)` | 照环境快照找资源目录 |
| `ResourceRoot::at(路径)` | 就用这个目录，测试、工具指定的 |
| `ResourceRoot::path()` | 资源目录本身 |
| `ResourceRoot::sources(人格)` | 读出这个人格要用的原文，交给 `miyu-policy` 拼策略快照（`policy.md`） |
| `ResourceRoot::subagent_venue()` | 读出子会话的场所说明 `core/jobs/subagent-venue.txt`，造子会话时接在人设后面（施工 7-5）；读不了的写明是哪一份 |
| `ResourceRoot::models()` | 模型资料的原文（`models/models-dev.json`，施工 6-3 上），怎么读由核心定（`core.md`「模型」） |
| `Human::load(资源目录, 语言)` | 读这种语言的给人看的字 |
| `Human::tool(工具名)` | 这件工具给人看的样子 `Face`；没有的是空的 |
| `Human::say(说法)` | 照说法换成的一句话；换不出来的是空的 |
| `Human::fields(编号)` | 这一句要哪些字段，照出现的先后，重复的算一次；没有这一句的是空的 |
| `clean(字)` | 控制字符换成 `�`，别的照原样 |
| `FALLBACK` | `"en"`：找不到别的语言时用的那一种 |

`Face` 有四格：`name` 是显示名，例如「读取」；`subject` 是显示名后面跟哪一个参数的值，例如 `file_path`，没有的只写显示名；`icon` 是写在最前面的符号，例如 `→`；`block` 是标题下面还印一块什么：`command` 印执行命令的输出，`edits` 印改动，没有的只印标题（施工 4-11，`cli/ask.md`「每一步」）。

说法（`Said`）是一个编号 `key` 加几个字段 `fields`，值都是字符串，记在 `tool.result` 的 `human` 里（`kernel/tools.md`）。

### 资源目录里有什么

```text
<资源目录>/
├── core/                                随核心附带的
│   ├── checkpoint-open.txt、checkpoint-close.txt、checkpoint-end.txt
│   ├── permission-rule.txt              没有程序读：还没进请求
│   ├── turn-ended/<原因>.txt             5 份
│   ├── facts/env.txt、permission.txt、reply-cut.txt
│   ├── tool-results/<哪一句>.txt         15 份
│   ├── permissions/forbidden.txt、unresolvable.txt
│   ├── drivers/<哪一句>.txt              5 份
│   ├── compaction/<哪一份>.txt           摘要指令、代码写的几段、重读的文件的头尾、截短重试的两份、隔离式那一句 system，12 份
│   ├── jobs/<哪一份>.txt                 两种回报的写法，11 份（施工 7-2）；回报截在中间的那一行（施工 7-6）；留言的标签，2 份（施工 7-7）；人停的那一句（施工 7-2 补）
│   └── human/zh.json、en.json            给人看的字
├── personas/<人格>/prompts/persona.md    人设；出厂的只有 engineer
└── software/<软件包>/                    出厂的只有 basesystem
    ├── tools/<工具>.json                 给模型看的说明和参数格式
    ├── <工具>/<名字>.txt、common/<名字>.txt  工具输出里给她看的几句
    └── human/zh.json、en.json            给人看的字
```

| 哪几份 | 谁读 | 什么时候 |
|---|---|---|
| `core/` 下的 `.txt`（两份 `*-rule.txt`、`jobs/subagent-venue.txt` 除外）、`personas/<人格>/prompts/persona.md` | `ResourceRoot::sources` | 造会话时，拼进策略快照 |
| `core/jobs/subagent-venue.txt` | `ResourceRoot::subagent_venue` | 造子会话时，接进 system（施工 7-5） |
| `core/human/`、`software/<软件包>/human/` | `Human::load` | `miyu ask` 起来时读一次，印每一步用（`cli/ask.md`） |
| `software/basesystem/` 下别的 | `miyu-basesystem` | 核心起来时登记工具（`tools/*.md`） |
| `models/models-dev.json` | `ResourceRoot::models` | 核心起来时读一次，查模型的窗口、最大输出（施工 6-3 上）。是数据，不发给模型，不进登记簿 |

给模型看的每一份字的原文、token 数、什么时候进请求，见 `26-提示词.md` 第十节的登记簿。给人看的字不进请求，不登记。

### 怎么走

**1. 找资源目录**（`ResourceRoot::locate`）

1. `MIYU_RESOURCES` 设了、不是空的：就是它，别处不看。开头的 `~` 和 `MIYU_HOME` 一样照家目录接（`store.md`），找不到家目录的当相对路径报错。接好以后要是绝对路径，相对的报错；它要是一个目录，不是的报错。
2. 没设或者是空的：看程序的真实位置（`Env` 的 `exe`，顺着链接找到的本体）。它旁边有 `resources/` 目录，就是它；不然它的上一级下有 `share/miyu/` 目录，就是它。
3. 都没有：报错，写明找过的两处。连程序在哪都不知道的，说不知道。
4. 不猜别的位置。

| 装法 | 程序 | 找到的资源目录 |
|---|---|---|
| 安装脚本 | `~/.local/lib/miyu/miyu` | 旁边的 `~/.local/lib/miyu/resources/` |
| deb、rpm、AUR、Homebrew | `<前缀>/bin/miyu` | 上一级下的 `<前缀>/share/miyu/` |
| 开发 | `target/debug/miyu` | 设 `MIYU_RESOURCES` 指到源码树的 `resources/` |

**2. 读出一个人格要用的原文**（`ResourceRoot::sources`）

1. 人格的编号要合写法：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。它是资源目录里的一层目录，不许带路径。不合的报错，不碰磁盘。
2. 照下表的先后读，路径一段一段地接上，三个平台一样。哪一份读不了，报那一份的路径和系统的原话，后面的不再读。
3. 原文照抄，行尾的换行也算。要是 UTF-8，不是的算读不了。

| 读的文件 | 放进哪一格 |
|---|---|
| `core/checkpoint-open.txt`、`core/checkpoint-close.txt`、`core/checkpoint-end.txt` | 检查点包装的开头、摘要的收尾、包装的结尾 |
| `core/turn-ended/interrupted.txt`、`error.txt`、`step_limit.txt`、`aborted.txt`、`restarted.txt` | 回合没走完的几句 |
| `core/facts/env.txt`、`permission.txt`、`reply-cut.txt` | 事实的模板 |
| `core/tool-results/unknown.txt`、`not-an-object.txt`、`cancelled-before.txt`、`cancelled-running.txt`、`skipped.txt`、`read-only.txt`、`denied.txt`、`denied-with-reason.txt`、`unattended.txt`、`question-interrupted.txt`、`question-voided.txt`、`question-unattended.txt`、`restarted.txt`、`unavailable.txt`、`crashed.txt` | 替工具写的结果 |
| `core/permissions/forbidden.txt`、`unresolvable.txt` | 权限策略拒绝时的话（`session/guard.md`） |
| `core/drivers/image-omitted.txt`、`file-omitted.txt`、`no-output.txt`、`tool-attachments.txt`、`tool-attachments-only.txt`、`file-open.txt`、`file-cut.txt`、`file-close.txt`、`image-open.txt`、`image-close.txt`、`image-omitted-named.txt` | 驱动的占位，文本文件照字放进消息的三句（施工 3-9 三补），带名字的图片的三句（施工 3-9 四补，`drivers/openai-chat.md` 第 9 条） |
| `core/compaction/summarize-task.txt`、`summarize-instructions.txt`、`summarize-end.txt`、`notes-files.txt`、`notes-files-more.txt`、`notes-retrieve.txt`、`notes-too-large.txt`、`restored-open.txt`、`restored-close.txt`、`truncated.txt`、`notes-uncovered.txt`、`summarize-system.txt` | 压缩的字：摘要指令（施工 6-2 上；施工 6-8 拆出最后那一句、加上手动压缩的要求前面那一行，`compaction.md` 第七条），检查点里代码写的几段、重读的文件那一块的头尾（施工 6-5，`compaction.md` 第八条），截过的摘要请求前面补的那一条、摘要没看到的那一段（施工 6-6 中，第三条第 10 条），隔离式那一句 system（施工 6-6 下，第四条） |
| `core/jobs/command-open.txt`、`command-exit.txt`、`command-signal.txt`、`command-duration.txt`、`command-output.txt`、`command-close.txt`、`subagent-open.txt`、`subagent-person.txt`、`subagent-truncated.txt`、`subagent-silent.txt`、`subagent-close.txt` | 两种回报的写法（施工 7-2，`kernel/request.md`「回报」） |
| `core/jobs/subagent-omitted.txt` | 子会话回报的正文截在中间的那一行，字段 `count`（施工 7-6，`kernel/session.md`「向上回报」第 3 条） |
| `core/jobs/stopped-by-user.txt` | 人停的那一句，两种回报共用（施工 7-2 补，`kernel/request.md`「回报」第 3 条） |
| `core/jobs/subagent-message-open.txt`、`subagent-message-close.txt` | 子代理发来的留言的标签，开头的字段 `job`、`title`（施工 7-7，`kernel/request.md`「子代理的留言」） |
| `personas/<人格>/prompts/persona.md` | 人设 |

**3. 读给人看的字**（`Human::load`）

1. 先读内核的：`core/human/<语言>.json`；读不到的读 `core/human/en.json`；也读不到的，这一处没有字，不算错。
2. 再照名字的先后读 `software/` 下的每个目录（链接不算），每个读 `human/<语言>.json`，同样退到英文。`software/` 读不了的，当没有软件包。
3. 读得到却读不懂的，报错，写明是哪一份：不是 JSON、写法不对（有不认识的格、工具少了 `name`、类型不对）、哪一句的模板坏了（写明是哪一句）。
4. `said` 里每一句的编号，前面加上这一份在资源目录里的位置：内核的加 `core/`，软件包的加 `software/<软件包>/`。例如 `core/human/zh.json` 里的 `tool-results/unattended`，就是说法 `core/tool-results/unattended`。
5. `tools` 合成一张表：后读的盖掉先读的同名工具。
6. 语言的编号由头交进来：`miyu ask` 交 `zh` 或 `en`（`cli/ask.md`）。

**4. 照说法换成一句话**（`Human::say`）

1. 照说法的编号找那一句。没有这一句：没有字。
2. 模板照 `{字段}` 写，`{{`、`}}` 是花括号本身（`kernel/request.md` 的模板）。模板要的字段说法里没有：没有字。说法里多出来的字段不用。
3. 换进去的每个字段先过 `clean`：控制字符（`U+0000` 到 `U+001F`、`U+007F` 到 `U+009F`，换行、制表也算）一律换成 `�`（`U+FFFD`），别的照原样，引号、尖括号、反斜杠都不转义。
   - 这些字不进请求，用不着防伪造记录行；可路径、参数是她给的，里面混着终端的控制序列，原样印出来会把终端弄乱。
4. 没有字的，头照工具名、状态写最泛的（`cli/ask.md`）。
5. 说法记进日志以后原样回放，不随界面语言变：换一种界面语言，照样换得出字。

### 样子

`human/<语言>.json` 只许有两格，都可以不写：

```json
{
  "tools": {
    "read": { "name": "读取", "subject": "file_path", "icon": "→" }
  },
  "said": {
    "read/lines": "{count} 行"
  }
}
```

- `tools` 里每件工具只许有 `name`（必填）、`subject`、`icon`、`block`（都可以不写）；`block` 只能是 `command` 或者 `edits`。
- 这一份在 `software/basesystem/human/zh.json` 里，`read/lines` 就是说法 `software/basesystem/read/lines`：字段 `count` 是 `37` 时，换成「37 行」。
- 每件工具的显示名、结果那一句，见 `tools/*.md` 和 `cli/ask.md`。

### 出错

报错的话只有中文。

| 类型 | 哪一种 | 说的话 |
|---|---|---|
| `ResourceError` | `Relative` | `MIYU_RESOURCES 要写绝对路径，现在是 <路径>` |
| | `Missing` | `MIYU_RESOURCES 指的 <路径> 不是一个目录` |
| | `NotFound`，找过两处 | `找不到资源目录：<程序旁边的 resources>、<上一级的 share/miyu> 都没有。开发时设 MIYU_RESOURCES 指到源码树的 resources/` |
| | `NotFound`，不知道程序在哪 | `找不到资源目录：不知道程序在哪。开发时设 MIYU_RESOURCES 指到源码树的 resources/` |
| `SourceError` | `Persona` | `persona id "<编号>" is not valid: it starts with a lowercase letter and has only lowercase letters, digits, - and _` |
| | `Read` | `cannot read <路径>: <系统的原话>` |
| `HumanError` | | `<哪一份>: <为什么>`；模板坏了的，为什么是 `<哪一句>: bad template: <哪里坏了>` |

`ResourceError` 给人看（核心起不来时交给头、`miyu ask` 印出来），是中文，等界面语言那一步；`SourceError` 只进运行日志，是英文（施工 4-9 再补四中：原来是中文）。

- 核心起来时找不到资源目录，起不来，原因交给头（`core.md`）。
- 造会话时读不出人格：编号不合写法的，协议端点回 `bad_params`；读不了文件的，回 `unknown_persona`（`protocol.md`）。
- `miyu ask` 读给人看的字出错、找不到资源目录，都当没有字（`cli/ask.md`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-store/src/resources/tests.rs` | `MIYU_RESOURCES` 优先、开头的 `~` 照家目录接、要是绝对路径、要是目录；程序旁边的 `resources/`、上一级的 `share/miyu/`；都没有时写明找过哪两处、不知道程序在哪；读出软件工程师的人设和随核心附带的字；人设文件缺了写明是哪一份；不合写法的编号拒绝；子会话的场所说明是它自己那份文件，没有的写明是哪一份（施工 7-5） |
| `crates/miyu-store/tests/human.rs` | 内核给模型的每一句（`core/tool-results/`、`core/permissions/`）两种语言都有给人看的一句，要的字段不多于给模型的；照语言换成字，没有的语言照英文，没有这一句、少了字段的换不出；工具的显示名、后面跟的参数、符号、下面那一块，`block` 写别的读不懂；控制字符换掉、引号反斜杠照原样；什么都没有不算错，只有英文的照英文，读不懂的写明是哪一份、哪一句 |
| `crates/miyu-store/tests/snapshot.rs` | 从源码树的资源目录拼出软件工程师的快照 |

### 出处

- `12-进程形态与分发.md` 第三节（资源目录的位置、怎么找）、R3（不编进二进制）。
- `26-提示词.md` 第三节（给人看的字和给模型看的字分两份，「双槽」）、第八节（东西放在哪：`core/`、`personas/`、`software/`、`human/`）、第十节（登记簿）。
- `07-存储.md` 第二节：出厂的放在资源目录里，只读，不在数据根里。

### 还没有的

- 同名覆盖：自己的家目录、系统区、出厂的三层，出厂的排在最后（`26-提示词.md` 第八节、J9，`16-人格与预设.md` 第四节）。现在只读资源目录这一处。
- 人格目录里别的文件：`persona.toml`、示范对话、角色扮演提示，和预设（`16-人格与预设.md` 第三节）。
- 两份 `*-rule.txt` 进 system（`26-提示词.md` 第十节的登记簿）。
- 网页、字体这类资源（`12-进程形态与分发.md` 第三节）。

**模型资料怎么刷新**（施工 6-3 上）：从 models.dev 的 `api.json` 抽出驱动认得的供应商（现在只有 `deepseek`），每个模型只留 `limit.context`、`limit.output`，顶上写出处和日期：

```sh
curl -s https://models.dev/api.json | python3 -c 'import json,sys,datetime; d=json.load(sys.stdin)["deepseek"]["models"]; print(json.dumps({"source":"https://models.dev/api.json","fetched":str(datetime.date.today()),"providers":{"deepseek":{"models":{k:{"limit":{"context":v["limit"]["context"],"output":v["limit"]["output"]}} for k,v in sorted(d.items())}}}}, indent=2))' > resources/models/models-dev.json
```

