## 资源目录和给人看的字

### 是什么

出厂的人设、给模型的字、工具的说明、给人看的字，都放在资源目录里，随安装包分发，不编进二进制。这一页写怎么找到它，怎么读出一个人格要用的原文，怎么读给人看的字、照说法换成一句话。数据根另见 `store.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/resources.rs` | 找资源目录；读出一个人格要用的原文、子会话的场所说明 |
| `crates/miyu-store/src/human.rs` | 读给人看的字；照说法换成一句话；换进去的字段把控制字符换成 `�`；配置那一格照 `Words` 交给配置清单（施工 8-1） |
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
| `ResourceRoot::catalog_snapshot()` | models.dev 目录的快照在哪（`models/models-dev.json`，旁边是 `models-dev.meta.json`，施工 8-7）：约 5 MB，核心写了 `ready` 以后在后台读，读不了的照样起来（`models.md`「怎么走」第二条） |
| `ResourceRoot::vendors()` | 认原厂的表的原文（`models/vendors.toml`，施工 8-7），怎么读由核心定 |
| `ResourceRoot::profiles()` | 供应商的档案的原文（`models/profiles.toml`，施工 8-6），怎么读由核心定（`core.md`「模型」，`models.md`） |
| `Human::load(资源目录, 语言)` | 读这种语言的给人看的字 |
| `Human::tool(工具名)` | 这件工具给人看的样子 `Face`；没有的是空的 |
| `Human::tools()` | 每件工具给人看的样子，照工具名排好（`human.get`，施工 W-1） |
| `Human::said_entries()` | 每一句说法的原文，照编号排好，一个字不换：编号已经带着它在资源目录里的位置（`human.get`，施工 W-1：换字段是头的事，核心不代换） |
| `Human::say(说法)` | 照说法换成的一句话；换不出来的是空的 |
| `Human::fields(编号)` | 这一句要哪些字段，照出现的先后，重复的算一次；没有这一句的是空的 |
| `clean(字)` | 控制字符换成 `�`，别的照原样 |
| `Human::page`、`Human::group` | 设置页的页、组的名字（施工 8-2，`config.schema`） |
| `Human` 实现的 `miyu_config::Words` | `item(键)`：配置那一格里这一项的名字、说明、选项名；`sentence(编号, 字段)`：内核那一份 `said` 里的一句，编号前面加 `core/`，例如 `config/facts` 就是说法 `core/config/facts`（施工 8-1，`config.md`「给人看的字」） |
| `FALLBACK` | `"en"`：找不到别的语言时用的那一种 |

`Face` 有四格：`name` 是显示名，例如「读取」；`subject` 是显示名后面跟哪一个参数的值，例如 `file_path`，没有的只写显示名；`icon` 是写在最前面的符号，例如 `→`；`block` 是标题下面还印一块什么：`command` 印执行命令的输出，`edits` 印改动，没有的只印标题（施工 4-11，`cli/ask.md`「每一步」）。

说法（`Said`）是一个编号 `key` 加几个字段 `fields`，值都是字符串，记在 `tool.result` 的 `human` 里（`kernel/tools.md`）。

### 资源目录里有什么

```text
<资源目录>/
├── core/                                随核心附带的
│   ├── checkpoint-open.txt、checkpoint-close.txt、checkpoint-end.txt
│   ├── permission-rule.txt、local-paths-rule.txt  核心的几行，拼进 system（施工 2-7 补）
│   ├── turn-ended/<原因>.txt             5 份
│   ├── facts/env.txt、permission.txt、reply-cut.txt、session.txt、permission-changed.txt
│   ├── tool-results/<哪一句>.txt         15 份
│   ├── permissions/forbidden.txt、unresolvable.txt
│   ├── drivers/<哪一句>.txt              5 份
│   ├── compaction/<哪一份>.txt           摘要指令、代码写的几段、重读的文件的头尾、截短重试的两份、隔离式那一句 system，12 份
│   ├── jobs/<哪一份>.txt                 两种回报的写法，11 份（施工 7-2）；回报截在中间的那一行（施工 7-6）；留言的标签，2 份（施工 7-7）；人停的那一句（施工 7-2 补）
│   └── human/zh.json、en.json、ja.json   给人看的字
├── personas/<人格>/prompts/persona.md    人设；出厂的只有 engineer
└── software/<软件包>/                    出厂的只有 basesystem
    ├── tools/<工具>.json                 给模型看的说明和参数格式
    ├── <工具>/<名字>.txt、common/<名字>.txt  工具输出里给她看的几句
    └── human/zh.json、en.json、ja.json   给人看的字
```

| 哪几份 | 谁读 | 什么时候 |
|---|---|---|
| `core/` 下的 `.txt`（两份 `*-rule.txt`、`jobs/subagent-venue.txt` 除外）、`personas/<人格>/prompts/persona.md` | `ResourceRoot::sources` | 造会话时，拼进策略快照 |
| `core/jobs/subagent-venue.txt` | `ResourceRoot::subagent_venue` | 造子会话时，接进 system（施工 7-5） |
| `core/permission-rule.txt`、`core/local-paths-rule.txt` | `ResourceRoot::core_lines` | 造会话时，接在 system 最后（施工 2-7 补，`policy.md` 的 `with_core_lines`） |
| `core/human/`、`software/<软件包>/human/` | `Human::load` | `miyu ask` 起来时读一次，印每一步用（`cli/ask.md`）；核心起来时照系统的语言读一次，生成配置的 Schema 和参考文件（施工 8-1，`config.md`） |
| `software/basesystem/` 下别的 | `miyu-basesystem` | 核心起来时登记工具（`tools/*.md`） |
| `models/models-dev.json`、`models-dev.meta.json` | `ResourceRoot::catalog_snapshot` | 核心写了 `ready` 以后读一次，和缓存目录里后台拉的那一份挑新的（施工 8-7，`models.md`）。原样的 `api.json` 和它是什么时候拉的。是数据，不发给模型，不进登记簿 |
| `models/models-dev.LICENSE` | 没人读 | models.dev 的 MIT 许可证原文，跟着快照一起发（`licenses.md`「资源里的第三方数据」） |
| `models/profiles.toml` | `ResourceRoot::profiles` | 核心起来时读一次，`[npm]`（包名 → 驱动，施工 8-7）、认得出的供应商的驱动、地址、开关、一张图怎么算（施工 8-6，`models.md`）。是数据，不发给模型，不进登记簿 |
| `models/vendors.toml` | `ResourceRoot::vendors` | 核心起来时读一次，认原厂（施工 8-7，`models.md`「怎么走」第二条第 6 条）。是数据，不发给模型，不进登记簿 |

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
| `core/facts/env.txt`、`permission.txt`、`reply-cut.txt`、`session.txt`、`permission-changed.txt` | 事实的模板（`session.txt` 施工 1-13 再补，`permission-changed.txt` 施工 2-7 补） |
| `core/tool-results/unknown.txt`、`not-an-object.txt`、`cancelled-before.txt`、`cancelled-running.txt`、`skipped.txt`、`read-only.txt`、`denied.txt`、`denied-with-reason.txt`、`unattended.txt`、`question-interrupted.txt`、`question-voided.txt`、`question-unattended.txt`、`restarted.txt`、`unavailable.txt`、`crashed.txt` | 替工具写的结果 |
| `core/permissions/forbidden.txt`、`unresolvable.txt` | 权限策略拒绝时的话（`session/guard.md`） |
| `core/drivers/image-omitted.txt`、`file-omitted.txt`、`no-output.txt`、`tool-attachments.txt`、`tool-attachments-only.txt`、`file-open.txt`、`file-cut.txt`、`file-close.txt`、`image-open.txt`、`image-close.txt`、`image-omitted-named.txt` | 驱动的占位，文本文件照字放进消息的三句（施工 3-9 三补），带名字的图片的三句（施工 3-9 四补，`drivers/openai-chat.md` 第 9 条） |
| `core/compaction/summarize-task.txt`、`summarize-instructions.txt`、`summarize-end.txt`、`notes-files.txt`、`notes-files-more.txt`、`notes-retrieve.txt`、`notes-too-large.txt`、`restored-open.txt`、`restored-close.txt`、`truncated.txt`、`notes-uncovered.txt`、`summarize-system.txt` | 压缩的字：摘要指令（施工 6-2 上；施工 6-8 拆出最后那一句、加上手动压缩的要求前面那一行，`compaction.md` 第七条），检查点里代码写的几段、重读的文件那一块的头尾（施工 6-5，`compaction.md` 第八条），截过的摘要请求前面补的那一条、摘要没看到的那一段（施工 6-6 中，第三条第 10 条），隔离式那一句 system（施工 6-6 下，第四条） |
| `core/jobs/command-open.txt`、`command-exit.txt`、`command-signal.txt`、`command-duration.txt`、`command-output.txt`、`command-close.txt`、`subagent-open.txt`、`subagent-person.txt`、`subagent-truncated.txt`、`subagent-silent.txt`、`subagent-close.txt` | 两种回报的写法（施工 7-2，`kernel/request.md`「回报」） |
| `core/jobs/subagent-omitted.txt` | 子会话回报的正文截在中间的那一行，字段 `count`（施工 7-6，`kernel/session.md`「向上回报」第 3 条） |
| `core/jobs/stopped-by-user.txt` | 人停的那一句，两种回报共用（施工 7-2 补，`kernel/request.md`「回报」第 3 条） |
| `core/jobs/subagent-message-open.txt`、`subagent-message-close.txt` | 子代理发来的留言的标签，开头的字段 `job`、`title`（施工 7-7，`kernel/request.md`「子代理的留言」） |
| `core/harness/message-open.txt`、`message-close.txt` | 别的 harness 发来的话的标签，开头的字段 `name`（施工 7-10，`kernel/request.md`「别的 harness 发来的话」） |
| `core/peers/message-open.txt`、`message-close.txt` | 别的会话发来的话的标签，开头的字段 `id`（施工 C-2，`kernel/request.md`「别的会话发来的话」） |
| `core/peers/idle-open.txt`、`idle-silent.txt`、`idle-expired.txt`、`idle-gone.txt`、`idle-close.txt` | 空了的通知：标签（字段 `id`、`reason`）、没说话的、作废了（字段 `hours`）、不在了、收尾（施工 C-6，`kernel/request.md`「空了的通知」） |
| `core/recap/instruction.txt`、`user.txt`、`assistant.txt`、`omitted.txt`、`excerpted.txt` | 回顾的请求的指令、两种标签、两句记号（施工 3-8 四补，`kernel/request.md`「回顾的请求」） |
| `core/title/instruction.txt` | 起标题的请求的指令（施工 3-8 五补，`kernel/request.md`「起标题的请求」） |
| `core/models/probe.txt` | `provider.test` 发的那一句（施工 8-11，`models.md`「怎么走」第七条第 4 条），`ResourceRoot::probe` 每试一次读一次 |
| `personas/<人格>/prompts/persona.md` | 人设 |

**3. 读给人看的字**（`Human::load`）

1. 先读内核的：`core/human/<语言>.json`；读不到的读 `core/human/en.json`；也读不到的，这一处没有字，不算错。
2. 再照名字的先后读 `software/` 下的每个目录（链接不算），每个读 `human/<语言>.json`，同样退到英文。`software/` 读不了的，当没有软件包。
3. 读得到却读不懂的，报错，写明是哪一份：不是 JSON、写法不对（有不认识的格、工具少了 `name`、类型不对）、哪一句的模板坏了（写明是哪一句）。
4. `said` 里每一句的编号，前面加上这一份在资源目录里的位置：内核的加 `core/`，软件包的加 `software/<软件包>/`。例如 `core/human/zh.json` 里的 `tool-results/unattended`，就是说法 `core/tool-results/unattended`。
5. `tools` 合成一张表：后读的盖掉先读的同名工具。`config` 的项、页、组也各合成一张表，后读的盖掉先读的（现在只有内核那一份写它）。
6. 语言的编号由头交进来：`miyu ask` 交 `zh` 或 `en`（`cli/ask.md`）。
7. **一个的时候说单数**（施工 4-5 再补）：一句说法管着的数是 1 时，编号多接一段 `/one`，例如 `read/lines/one`；发说法的那一处自己挑，是 1 就发 `X/one`，别的数照旧发 `X`，模板本身不挑单复数（「模板只做字段替换」，`05-内核接口.md`）。读完一种语言的全部文件以后，凡是有 `X` 没有 `X/one` 的，拿 `X` 的内容原样补一份 `X/one`：英文那种需要单数的字段后面紧跟着可数名词的（`{count}`、`{total}` 这类），自己写了 `X/one` 那一句，照它；中文、日文不挑单复数，没写，退到这条规矩补出来，和 `X` 一个字不差。软件包自己写了 `X/one` 的，不补（已经有了）。

   因为是补在全部文件读完以后，`human.get`（施工 W-1）交出去的 `said_entries()` 里，每种语言都能找到 `X/one`，不止写了它的那一种。

   没选的两种：模板里写复数的语法（每个头都要会挑，逻辑进了模板）；英文改说法躲开单复数，例如 `lines: 1`（读着不像人话）。

**4. 照说法换成一句话**（`Human::say`）

1. 照说法的编号找那一句。没有这一句：没有字。
2. 模板照 `{字段}` 写，`{{`、`}}` 是花括号本身（`kernel/request.md` 的模板）。模板要的字段说法里没有：没有字。说法里多出来的字段不用。
3. 换进去的每个字段先过 `clean`：控制字符（`U+0000` 到 `U+001F`、`U+007F` 到 `U+009F`，换行、制表也算）一律换成 `�`（`U+FFFD`），别的照原样，引号、尖括号、反斜杠都不转义。
   - 这些字不进请求，用不着防伪造记录行；可路径、参数是她给的，里面混着终端的控制序列，原样印出来会把终端弄乱。
4. 没有字的，头照工具名、状态写最泛的（`cli/ask.md`）。
5. 说法记进日志以后原样回放，不随界面语言变：换一种界面语言，照样换得出字。

### 样子

`human/<语言>.json` 只许有三格，都可以不写（`config` 那一格施工 8-1 加）：

```json
{
  "tools": {
    "read": { "name": "读取", "subject": "file_path", "icon": "→" }
  },
  "said": {
    "read/lines": "{count} 行"
  },
  "config": {
    "items": {
      "log.level": { "name": "运行日志的级别", "description": "运行日志记到哪一级。…", "options": { "error": "只记错误", "…": "…" } }
    },
    "pages": { "advanced": "高级" },
    "groups": { "log": "运行日志" }
  }
}
```

- `tools` 里每件工具只许有 `name`（必填）、`subject`、`icon`、`block`（都可以不写）；`block` 只能是 `command` 或者 `edits`。
- 这一份在 `software/basesystem/human/zh.json` 里，`read/lines` 就是说法 `software/basesystem/read/lines`：字段 `count` 是 `37` 时，换成「37 行」。
- 中文不挑单复数，这一份不用写 `read/lines/one`；`software/basesystem/human/en.json` 里那一句是 `"{count} lines"`，另写了一句 `"read/lines/one": "{count} line"`（上面「怎么走」第 3 条第 7 款）。
- 每件工具的显示名、结果那一句，见 `tools/*.md` 和 `cli/ask.md`。
- `config` 里只许有 `items`、`pages`、`groups`；一项只许有 `name`、`description`（必填）、`options`（可以不写）。写了什么、和配置清单怎么对上，见 `config.md`「给人看的字」「怎么走」第一条第 5 条。内核那一份的 `said` 里还有生成文件要的几句、报错的话和接句子的三句 `config/…`（施工 8-1、8-2，`config.md`「给人看的字」），`trust.toml` 开头那一行注释 `config/trust-header`、生效时机 `config/applies/head_start`（施工 8-3）；8-6 加的类型、生效时机、报错要的 `config/applies/next_turn`、`config/expected/int`、`url`、`name`、`reference`、`list`、`id`、`model-name`、`config/bad-format`、`config/out-of-range`、`config/bad-segment`，`config` 那一格多了模型那一块的六项、页 `models`、组 `uses`、`providers`；8-7 加的类型要的 `config/expected/float`、`text`、`duration`，`config` 那一格多了模型资料、目录更新的十六项、组 `catalog`。

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
| `crates/miyu-store/src/resources/tests.rs` | `MIYU_RESOURCES` 优先、开头的 `~` 照家目录接、要是绝对路径、要是目录；程序旁边的 `resources/`、上一级的 `share/miyu/`；都没有时写明找过哪两处、不知道程序在哪；读出软件工程师的人设和随核心附带的字（会话编号的模板是它那份文件，施工 1-13 再补；切了级别以后的权限那一份也是，施工 2-7 补）；人设文件缺了写明是哪一份；不合写法的编号拒绝；子会话的场所说明是它自己那份文件，没有的写明是哪一份（施工 7-5）；核心的几行是它们那两份文件，没有的写明是哪一份（施工 2-7 补） |
| `crates/miyu-store/tests/human.rs` | 内核给模型的每一句（`core/tool-results/`、`core/permissions/`）两种语言都有给人看的一句，要的字段不多于给模型的；照语言换成字，没有的语言照英文，没有这一句、少了字段的换不出；工具的显示名、后面跟的参数、符号、下面那一块，`block` 写别的读不懂；控制字符换掉、引号反斜杠照原样；什么都没有不算错，只有英文的照英文，读不懂的写明是哪一份、哪一句；配置那一格照 `Words` 交出去、句子的编号加 `core/`、写错了说是哪一份（施工 8-1）；中文、日文没写 `X/one` 的，退到 `X` 的字，英文自己写的不一样；每种语言的 `said_entries()` 都交得出清单上每一句的 `/one`；软件包自己写了 `X/one` 的，补的规矩不盖掉它（施工 4-5 再补） |
| `crates/miyu-store/tests/human_languages.rs`（施工 4-5 补；4-5 再补加了单数的门禁） | 内核和每个软件包都有中文、英文、日文三份，说法的键、每一句要的字段、工具的样子（显示名以外）、配置那一格的项和选项、页、组（施工 8-1）都和英文那一份一样（没写 `X/one` 的按 `Human::load` 的规矩补齐了再比）；每件工具都有显示名，配置的名字、说明都不空；日文照语言换得出（找不到的语言会退回英文，所以直接查文件）；门禁：英文 `{count}`、`{total}` 后面紧跟着词的每一句，都有 `/one` 那一句对着 |
| `crates/miyu-store/tests/snapshot.rs` | 从源码树的资源目录拼出软件工程师的快照 |

### 出处

- `12-进程形态与分发.md` 第三节（资源目录的位置、怎么找）、R3（不编进二进制）。
- `26-提示词.md` 第三节（给人看的字和给模型看的字分两份，「双槽」）、第八节（东西放在哪：`core/`、`personas/`、`software/`、`human/`）、第十节（登记簿）。
- `07-存储.md` 第二节：出厂的放在资源目录里，只读，不在数据根里。

### 还没有的

- 同名覆盖：自己的家目录、系统区、出厂的三层，出厂的排在最后（`26-提示词.md` 第八节、J9，`16-人格与预设.md` 第四节）。现在只读资源目录这一处。
- 人格目录里别的文件：`persona.toml`、示范对话、角色扮演提示，和预设（`16-人格与预设.md` 第三节）。
- 网页、字体这类资源（`12-进程形态与分发.md` 第三节）。

**目录的快照怎么刷新**（施工 8-7）：下载原样的 `api.json`，旁边的 `meta` 写出处和服务器回的时刻（UTC），许可证照 models.dev 仓库的 `LICENSE` 原文：

```sh
curl -sS -D /tmp/models-dev.headers -o resources/models/models-dev.json https://models.dev/api.json
python3 -c 'import json,email.utils; d=[l for l in open("/tmp/models-dev.headers") if l.lower().startswith("date:")][0].split(":",1)[1].strip(); print(json.dumps({"source":"https://models.dev/api.json","fetched":email.utils.parsedate_to_datetime(d).strftime("%Y-%m-%dT%H:%M:%S.000Z")}))' > resources/models/models-dev.meta.json
```

