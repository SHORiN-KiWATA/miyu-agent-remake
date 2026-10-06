## 施工单 8-12：Anthropic 消息接口

状态：已完成（2026-10-03 施工，项目主人验收通过；图纸 `docs/blueprint/drivers/anthropic.md` 10-02 起草，10-03 主会话审过，改的几处见「蓝图改哪几节」第 1 条）。

### 目的

多一个驱动家族 `anthropic`：Anthropic 官方、opencode Zen 上的 Claude、走这种写法的兼容接口能直接配上用。缓存打点照前缀契约放四处，思考块带签名原样回传，工具、图片、PDF 照这一家的写法，思考强度照它的 `thinking`、`output_config`。请求形状探针多一张 Anthropic 的脸。

### 蓝图改哪几节

1. `drivers/anthropic.md`（主会话审，2026-10-03）：
   - 状态改成审过；「要项目主人拍板的」第 1 题定了 A（没配强度什么都不加，照供应商的默认），挪进「起草时定的」第 17 条。
   - 「缓存打点」第 1 条第 2 处：改前「第 `stable` 条消息在线上落在哪一条，打在那一条它自己的最后一块上」；改后「前 `stable` 条消息写出的最后一块」。合并以后一条线上的消息可能装着几条统一的消息，空的消息不写块，原写法说不清落在哪。
   - 「思考强度」第 1 条补一句：目录里只有 Sonnet 5 标了开关；Opus 5.5、Sonnet 5.5、Fable 收 `disabled` 会报 400，它们目录里没有开关，不会多出 `off`。
   - 「要实测确认的」「还没有的」加「保留思考的校验」（见「要拍板的」）。
   - 做完照做好的样子改写，施工时定的另记「施工时定的」一节。
2. `models.md`：认得的驱动多 `anthropic`；思考强度的开关 openai-chat 照档案、anthropic 自带；路由给 anthropic 填 `Call.max_output`；档案加 `[providers.anthropic]`；「驱动要守的约定」第 11、13 条写上这一家的取法。
3. `drivers/openai-chat.md`：换成字的那几步挪到 `media.rs`；超长的说法加 `exceed context limit`；「还没有的」删掉 Anthropic。
4. `http.md` 认证头一条、`kernel/request.md` 缓存标记、`05-内核接口.md` 第七节加两张表，都照图纸指过去。

### 要拍板的

已定（2026-10-03 项目主人定 A，记进图纸「起草时定的」第 18 条；8-12 补合进 main 时登记进施工方案）。

1. **保留思考的校验**（审图时查出来，图纸漏了）：Fable 5.1、Opus 5.5、Sonnet 5.5 的思考块签名绑着产生它时的前缀（system、工具面、前面每条消息）。2026-08-31 以后建的账号，发回去的思考块前面的东西被改过，整个请求 400。我们会改前缀的地方：压缩留尾巴（尾巴里的思考块绑着压缩前的历史）、回合开始换了工具面或 system。撞上以后这个会话每次请求都 400，直到尾巴被下一次压缩压掉。撤销只删后面的，不碰。
   - A（推荐）：这一步照图纸原样回传，记进「还没有的」；有官方 key 以后实测，另开一步（8-12 补）做「这一种 400 去掉思考块重发一次」。理由：只影响新账号的三个模型，修法要动执行器，要实测出真的报错原话才写得准。
   - B：这一步就只回传这一轮的思考块（上一个人说的话以后的）。代价：每一轮第一次请求都会让上一轮那段对话的缓存重写一次，所有人都付。

### 不做什么

- 1 小时的缓存、保温；图片的官方算法；1M 的 beta 头；服务端工具、引用、文件接口、思考预算；接着写（图纸「还没有的」）。
- 8-13、8-14：8-12 合了以后一张一张做。
- 顶层的自动缓存、任何 `anthropic-beta` 头。

### 验收

1. 测试（先写，新类型、新函数改之前编译不过）：图纸「守着它的」那张表每一行；`media.rs` 挪完 openai-chat 的样本一个字节不变；`miyu-models` 认 `anthropic`、目录有开关的模型多 `off`；路由给 anthropic 填输出上限（一次性入口写了的、模型资料的、都没有的 8192）。
2. 请求形状探针：`MIYU_PROBE_WRITE=1` 重写，`openai-chat` 的存档 `git diff` 是空的，`terminal/anthropic/` 是新加的；每次请求去掉打点以后是上一次的前缀延伸，随机日志三百例也查。
3. 给模型看的字：没有新的。
4. 手写变异 15 个左右，挑关键的，全被逮住；`cargo xtask check` 八项全过；三台机器的 CI 全绿。
5. 真模型实测（主会话合并前，DeepSeek 的 Anthropic 兼容接口）：编码、解码、带工具的循环、思考（配一档、签名回传不报 400）、人附图、打断再接着说、写错 key 是 `auth`；`"tools":[]`、PDF 放在 `tool_result` 里收不收照实记。缓存命中这一条它测不了（自动缓存、不认打点）：记成「待 Anthropic 官方的 key」，或者用 Zen 上的 Claude 测（收费，用之前问项目主人）。

### 风险

- 改 `media.rs` 动了 openai-chat：样本、探针存档逐字节比着，变一个字节就红。
- 路由里驱动从一个具体类型换成两种：会话、一次性入口、列模型、`provider.test` 四处都走一遍。
- 兼容接口认不认 `display`、`adaptive`、签名：实测照实记，不认的写进「要实测确认的」，不为它改写法。
- 保留思考的校验：见「要拍板的」第 1 条。

### 验收结果

- 测试（先写；新的类型、函数改之前编译不过）：
  - `miyu-drivers`：`tests/anthropic.rs`（12 个）、`anthropic_marks.rs`（8 个，随机会话 60 个种子）、`anthropic_thinking.rs`（5 个）、`anthropic_media.rs`（8 个）、`anthropic_streams.rs`（7 个，15 份流的样本、从每个字节切开喂都一样）、`src/anthropic/models/tests.rs`（2 个）、`classify/tests.rs` 多一个（这一家的错误体 12 类）。样本 18 份在 `docs/designs/samples/drivers/anthropic/`。
  - `miyu-models`：`provider/tests.rs`、`facts/tests.rs` 各多一个；`miyu-session`：`tests/route_anthropic.rs`（4 个）；`miyu-core`：出厂档案那一条多 `anthropic`。
  - 拿 `anthropic` 当「还没有的驱动」的几处测试换成 `openai-responses`。
- 请求形状探针：`MIYU_PROBE_WRITE=1` 重写，`openai-chat` 的存档 `git diff` 是空的；新加 `terminal/anthropic/`（22 份）。每个探针、随机日志的每一次请求编码成 Anthropic、去掉打点以后都是上一次的前缀延伸。
- 手写变异 19 个，逮住 18 个，一个一个改、跑相关的测试、改回去：稳定区不打；第 3 处打在 assistant 上；思考块也打；没 system 不打工具；不合并；空字也写；别家的思考也回传；用别家的编号；`is_error` 总写；档位不写 `display`；签名不理；`refusal` 当正常；用量照 openai-chat 减命中的；什么图都收；`exceed context limit` 不算超长；openai-chat 也填输出上限；anthropic 照档案的开关；不带版本头。没逮住的「同一块打两次」：编码时照有没有打点写，重复的记录本来就不会写两遍，那一行检查是多余的，改成用集合记打点、删掉它。
- `cargo xtask check`：格式、clippy、文档、分层、纯逻辑、行数、许可证都过；测试只有 `config_set` 的 `a_write_that_fails_changes_nothing` 不过，容器里是 root 写得进只读文件，和这一步无关，CI 上过。
- 新依赖：没有。给模型看的字：没有新的。

**主会话合并前真模型实测**（2026-10-03，DeepSeek 的 Anthropic 兼容接口，模型 `deepseek-v4-flash`，临时数据根，驱动 `anthropic`、地址和 key 都照环境变量取）：

| 项 | 结果 |
|---|---|
| 带工具的循环 | 读两个文件、加起来，两次请求都成；三轮接着说，思考块带签名原样回传，不报 400 |
| 思考强度 | `off`：没有思考块，输出 1 个 token；`low`、`high`、`max`：有思考的摘要、带签名；没配：照供应商的默认，回来的是只有签名、没有字的思考块。换了强度的那一次命中掉成 0，和图纸「缓存打点」第 8 条说的一样 |
| 附图 | 人附的 PNG、`read` 读出来的 PNG（在 `tool_result` 里）都看得到 |
| 打断 | 思考里断（没签名，下一次丢掉）、正文里断，再接着说都不报 400 |
| 写错 key | `auth`，HTTP 401 |
| 历史里有调用、`"tools":[]` | 收（直接发请求试的） |
| PDF | 在 `tool_result` 里、在 user 里都不报错，可是 DeepSeek 读不了 PDF：收不收要官方的 key 才说得准 |
| 列模型 | DeepSeek 的兼容接口没有 `/models`（404），测不了 |
| 缓存打点命中 | 测不了：DeepSeek 是自动缓存，不认打点（实测每次请求都照前缀命中，2304、2432 这样按块算）。**待 Anthropic 官方的 key**，或者用 Zen 上的 Claude 测（收费，要项目主人点头）。同时也就测得出「保留思考的校验」 |

key 只在环境变量里：数据根、日志、仓库里查过都没有。
