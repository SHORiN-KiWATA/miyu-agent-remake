## 运行日志

### 是什么

核心进程自己的流水账：它起来了没有、在干什么、哪里出了错。一行一条英文，像 dmesg；写进数据根的 `state/logs/core.log`，满 10 MiB 换一份。它不是真相源，删了不丢任何东西；会话里发生的事在会话日志里（`store.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-log/src/lib.rs` | 装上（`install`）、订阅者怎么筛、换级别的把手（`Guard::set_level`，施工 8-2）、一份多大、留几份 |
| `crates/miyu-log/src/level.rs` | `MIYU_LOG` 的值怎么读 |
| `crates/miyu-log/src/settings.rs` | 配置项 `log.level`（施工 8-1，`config.md`「M8 的配置项」）：只声明，进配置清单；选项和 `MIYU_LOG` 的写法一样 |
| `crates/miyu-log/src/line.rs` | 一行怎么写：几列、转义、加不加引号、时刻、和 UTC 差多少 |
| `crates/miyu-log/src/home.rs` | 家目录写成 `~` |
| `crates/miyu-log/src/layer.rs` | 把一条事件写成一行；会话编号跟着 span 走 |
| `crates/miyu-log/src/rotate.rs` | 按大小轮换的文件 |
| `crates/miyu-core/src/lib.rs` | 核心起来时装上 |
| 发日志的各个 crate | 照 `tracing` 这个门面发，目标写 `miyu::<来源>`；这一页第 8 条列出每一行 |

### 对外的样子

| 名字 | 是什么 |
|---|---|
| `install(目录, 名字, 级别, 家目录)` | 装上：写进 `<目录>/<名字>.log`，先记到这一级（核心照 `MIYU_LOG` 读出来的），交回 `Guard`；家目录读不出来的是空的，路径照原样写（施工 8-2 起收级别，不收原值） |
| `Guard::set_level(级别)` | 换级别：核心读完配置照 `log.level` 换一次（施工 8-2），运行中换随 8-4 |
| `Guard` | 留着它，日志就一直写；丢掉时把文件 flush 一下 |
| `LIMIT` | 一份的上限：10 MiB（10,485,760 字节） |
| `KEEP` | 正在写的之外留几份：5 |
| `level(值)` | 读 `MIYU_LOG`：记到哪一级 `filter`，读不懂的原值 `unknown` |
| `subscriber(写到哪, 级别, 家目录)` | 一个筛好、写成一行的订阅者；测试拿它接住日志 |
| `utc_offset()` | 本机现在和 UTC 差多少：`+09:00` 这样 |
| `RotatingFile` | 按大小轮换的文件：`open(目录, 名字, 上限, 留几份)`、`path()`、`flush()` |
| `LineLayer`、`Sink`、`Memory` | 写成一行的那一层；写一行的地方；测试用的、留在内存里的 |

| 文件 | 是什么 |
|---|---|
| `<数据根>/state/logs/core.log` | 正在写的 |
| `<数据根>/state/logs/core.log.1` … `core.log.5` | 以前的：`.1` 是上一份，`.5` 最老 |

- `state/logs/` 没有就建（连同缺的上级），文件没有就建。Unix 上新建的目录 0700、文件 0600，只有本人能进、能读（`07-存储.md` 第二节；施工 4-9 再补四上：原来照系统默认的，一般是 0755、0644）；已经有的不改。Windows 上靠用户目录本身的访问控制。
- 用到的环境变量：`MIYU_LOG`。

### 怎么走

**1. 谁装**：只有核心进程（`miyu core`）装。它找到数据根、建好骨架、拿到单实例锁以后才装：先拿锁，免得两个核心写同一份（`core.md`）。装之前出的错，和头（`miyu ask`、`miyu undo`）进程里发的行，没人接，不写。

**2. 装上**（`install`）

1. 打开 `<目录>/<名字>.log` 接着往后写，目录没有就建；量出它已经多长。核心的是 `state/logs/` 下的 `core`。
2. 记下家目录：写成一行时把它换成 `~`（第 5 条）。核心给的是它环境里的家目录（`store.md` 的环境快照）。
3. 照给的级别记（核心给的是照 `MIYU_LOG` 读出来的，第 3 条）。筛的那一层能换（`tracing-subscriber` 的 `reload`）：核心读完配置以后照 `log.level` 换（施工 8-2）。
4. 装成这个进程全局的订阅者。一个进程只能装一次，第二次报错。
5. `MIYU_LOG` 读不懂的，装上时先照 `INFO`；核心读完配置以后记一条 `WARN`：`MIYU_LOG not understood, using config value=<原值>`（目标 `miyu::config`），照配置里的 `log.level`（施工 8-2，`config.md` 第二条第 5、7 条）。
6. 每一行写完就直接交给系统，不攒着；也不同步到磁盘。

**3. 级别**：`MIYU_LOG` 管这一次启动，压过配置项 `log.level`；没设、读不懂的照 `log.level` 的最终值（系统配置写的，没写的是 `info`，施工 8-2）。核心读完配置记一条 `INFO log level level=<级别> from=env|config|default`。

| 值（不分大小写，前后的空白不算） | 记到 |
|---|---|
| 没设、空的 | `INFO` |
| `error`、`warn`、`info`、`debug`、`trace` | 那一级 |
| `off` | 什么都不记 |
| 别的 | `INFO`，再记一条 `WARN` 说读不懂的是什么 |

- 发行版（编译时没开 `debug_assertions`）里 `TRACE` 的行编译时就去掉了（`tracing` 的 `release_max_level_debug`）：设 `trace` 也只记到 `DEBUG`。

| 级别 | 什么时候用 |
|---|---|
| `ERROR` | 一定是 bug：哪个任务 panic 了（会话的 actor、写盘的线程、工具、列会话、服务一个连接、撤销的回应里比改动的），走到了不该走到的状态（要把回答交给一个没问过的工具） |
| `WARN` | 坏事，但可能发生：重试、写不进去、找不到东西、清理不掉 |
| `INFO` | 来龙去脉：核心起停、在哪等连接、握手和断开、会话造好载入停下、每次请求怎么收场、每次调工具怎么收场 |
| `DEBUG` | 每一条输入、每一个动作的种类，HTTP 的来回，连接和请求 |
| `TRACE` | 模型的增量、工具执行中的输出这类一次成百上千条的 |

**4. 筛**

1. 目标以 `miyu` 开头的（照字符串的前缀比，`miyu::http` 就算）：照第 3 条的级别记。
2. 别人家的（`hyper`、`reqwest` 这些）：最多记到 `WARN`，免得调到 `DEBUG` 时被它们刷屏；第 3 条的级别比 `WARN` 还严的（`error`、`off`），照它。
3. span 也照级别筛。会话的 span 开在 `ERROR` 级（`session/actor.md`），调到 `WARN`、`ERROR` 也筛不掉它，底下的行照样带着会话编号。

**5. 一行怎么写**

```text
<时刻> <级别> <来源> <会话编号> <这件事> <键>=<值> <键>=<值> …
```

| 格 | 怎么写 |
|---|---|
| 时刻 | 本机时间，系统的时区，到毫秒：`2026-09-27 21:03:15.284`，23 个字符 |
| 级别 | `ERROR`、`WARN`、`INFO`、`DEBUG`、`TRACE`，左对齐占 5 格 |
| 来源 | 目标去掉开头的 `miyu::`：`miyu::http` 写成 `http`；别人家的照原样，例如 `hyper::proto`。左对齐占 8 格，长的不截，后面照样空一格。自带软件的工具发的行，目标写工具的名字：`miyu::shell`（施工 4-9 再补四上：原来写 `miyu::basesystem`，10 个字，这一列对不齐） |
| 会话编号 | 有的才写，没有的这一格连同它后面的空格都不写。事件自己带了 `session` 这一格的，用它；没带的，用包着它的 span 里离得最近、有 `session` 的那一个（开 span 以后才记进去的也算）。阻塞线程、自己起的线程里发的也一样：派活的地方把当时的 span 带过去，在那边进入它（施工 4-9 再补四上：原来 `spawn_blocking`、`thread::spawn` 里发的没有会话编号） |
| 这件事 | 事件的正文 |
| 键值 | 事件别的格，照发的先后，每个前面空一格。没有值的格不写 |

- 值：字符串照原样；数字、布尔照原样；用 `%` 发的照它的 Display，用 `?` 发的照它的 Debug。
- 家目录写成 `~`（施工 4-9 再补四上）：这件事和每个值里，照字面找装上时给的家目录（末尾的 `/`、`\` 不算），它前面是开头、或者不是路径里的字（字母、数字、`_`、`-`、`.`、`/`、`\`、`:`），后面是结尾、或者不是名字里的字（字母、数字、`_`、`-`、`.`）的，换成 `~`。Windows 上 `\\?\` 开头的，连同这四个字一起换。家目录是空的、是根（`/`、`C:\` 这样没有名字的）不换。例如家目录是 `/home/ai`：`/home/ai/.miyu` 写成 `~/.miyu`，`/home/ai` 写成 `~`，`/home/aim`、`/srv/home/ai` 不换。先换再加引号、转义。
- 会话编号、这件事：换行、回车、制表写成 `\n`、`\r`、`\t`，别的控制字符写成 `\x1b` 这样（两位小写十六进制），别的照原样。
- 值是空的，或者带空白（全角空格也算）、控制字符、`"`、`=` 的，加双引号，里面的 `\`、`"` 前面加反斜杠，控制字符照上一条转；别的值照原样。
- 所以一行里不出现换行和别的控制字符，`cat` 日志的时候终端不会把它们当成指令。
- 行尾加 `\n`。

**6. 轮换**（`RotatingFile`）

1. 写一行之前：这一份已经写过，再写这一行（连换行）就超过上限的，先换一份。一行不拆开。
2. 这一份还是空的，不换：比上限还长的一行照样整行写进去，不会留下一份空的 `.1`。
3. 换一份：先关掉正在写的；最老的 `core.log.5` 有就删掉；`.4` 挪成 `.5`，依此类推，`.1` 挪成 `.2`；`core.log` 挪成 `core.log.1`；再新建一份空的 `core.log`。和 logrotate 的 `rotate 5` 一个口径。
4. 换份当中出了错：照原来的名字 `core.log` 重新打开，接着往后写，已有的长度量进去。
5. 一行写不进去的就丢了，不报：没有别的地方可以报。
6. 进程再起来，接着写原来那一份，已有的长度算进去：满了照样换。
7. 几个线程同时写，一行一行排着写。留几份至少是 1。

**7. 不写什么**

1. 对话的内容：人说的话、她的回复和思考、工具的参数和结果，一个字都不写。写编号：会话编号、请求的序号（`seen`）、调用编号（`call`）。
2. key、令牌：不写。
3. 供应商出错的原话：不写，只写分类（`class`）。原话里可能回显请求里的字。
4. HTTP：只写主机名、字节数、状态码、分类、供应商说要等多久、用时；地址的路径和参数不写。
5. 这几条靠发日志的地方只交编号、长度、状态：接口上没有写内容的口子。由测试查（「守着它的」）。
6. 出错的原因（`error` 这一格）照原样写：系统的原话，或者核心自己的报错，都是英文（施工 4-9 再补四中：核心自己的几种报错原来是中文）；路径里的家目录照第 5 条写成 `~`。核心起不来的那一行不写原因，写没过的是哪一步（`stage`）：原因是给人看的中文，只交给头，头印给人看（`core.md`）。

**8. 每一行**

用时、要等多久都是毫秒的整数，键名带 `_ms`。「带会话编号」的写在来源后面（第 5 条）。

| 来源 | 级别 | 这件事 | 键 | 什么时候 |
|---|---|---|---|---|
| `config` | WARN | `MIYU_LOG not understood, using config` | `value` | 第 2 条（施工 8-2 起由核心读完配置以后记） |
| `config` | INFO | `log level` | `level`、`from` | 第 3 条（施工 8-2） |
| `core` | INFO | `starting` | `version`、`pid`、`root`（数据根）、`tz`（本机和 UTC 差多少，`+09:00` 这样） | 装上日志以后，第一件事就记它（`MIYU_LOG` 读不懂的那一条 `WARN`，施工 8-2 起排在读完配置以后） |
| `core` | WARN | `not started` | `stage`：`home`、`resources`、`runtime`、`socket`、`models`、`tools` | 起不来，原因交给头（`core.md`） |
| `core` | WARN | `ready line not written` | `error` | 往标准输出写那一行写不了 |
| `core` | INFO | `stopped` | `reason`：`idle` 或 `signal` | 空闲够久了，或者收到停的信号 |
| `core` | WARN | `SIGTERM not watched`、`Ctrl+C not watched` | `error` | 装不上信号的监听 |
| `core` | WARN | `DEEPSEEK_API_KEY not set, no model` | | 没设 key，或者只有空白 |
| `core` | INFO | `sandbox` | `helper`（助手的路径）、`platform`、`mechanisms`（逗号连起来，空的写 `none`） | 起来时探沙盒的助手，探成了（`sandbox.md`，施工 5-1） |
| `core` | WARN | `sandbox unavailable` | `reason` | 起来时探沙盒的助手：没找到、跑不了、到时、说的读不懂 |
| `session` | | | | 会话的每一行带会话编号，见 `session/actor.md` 的「运行日志」 |
| `http` | DEBUG | `sent` | `host`、`bytes` | 请求发出去 |
| `http` | DEBUG | `ended` | `host`、`status`、`took_ms` | 正常说完 |
| `http` | DEBUG | `failed` | `host`、`status`（收到了响应头的）、`class`、`retry_after_ms`（供应商说了的）、`took_ms` | 出错 |
| `http` | DEBUG | `cancelled` | `host`、`took_ms` | 被叫停 |
| `endpoint` | DEBUG | `connected` | | 接到一个连接 |
| `endpoint` | WARN | `accept failed` | `error` | 接连接出错 |
| `endpoint` | ERROR | `connection task failed` | `error` | 服务一个连接的任务没正常结束 |
| `endpoint` | WARN | `line too long, closed` | | 一行太长，断开 |
| `endpoint` | DEBUG | `request` | `method` | 收到一个请求 |
| `endpoint` | WARN | `protocol mismatch` | `head`、`low`、`high` | 握手时协议版本对不上 |
| `endpoint` | WARN | `bad token` | `head` | 握手时令牌不对 |
| `endpoint` | INFO | `connected` | `head`、`version`、`protocol` | 握手成了 |
| `endpoint` | INFO | `disconnected` | | 握过手的连接断开 |
| `endpoint` | WARN | `lagged, resync` | 带会话编号 | 订阅掉了队 |
| `endpoint` | ERROR | `list panicked` | `error` | 列会话的任务 panic 了 |
| `endpoint` | WARN | `sessions not listed` | `error` | 列不出会话 |
| `endpoint` | WARN | `first event not read` | 带会话编号；`error` | 列会话时一个会话的第一条读不出来：坏了、读写出错（还没造好的不算） |
| `endpoint` | WARN | `create failed` | `error` | 造会话失败，人格读不出以外的原因 |
| `endpoint` | WARN | `load failed` | 带会话编号；`error` | 载入失败，没有这个会话以外的原因 |
| `endpoint` | DEBUG | `already stopped` | 带会话编号 | 有计划地停下时，会话已经停了 |
| `endpoint` | WARN | `workspace not prepared` | `kind` | 退回账号的工作区时建不了它 |
| `endpoint` | ERROR | `undo report panicked` | `error` | 撤销的回应里比改动的任务 panic 了 |
| `endpoint` | WARN | `undo report not written` | `error` | 撤销的回应里读不了会话日志 |
| `ipc` | INFO | `listening` | `socket` | 在套接字上等连接 |
| `ipc` | INFO | `stale socket removed` | `socket` | 删掉上一个核心崩了留下的套接字 |
| `ipc` | DEBUG | `socket file not removed` | `error` | 退出时删不掉套接字文件 |
| `ipc` | WARN | `XDG_RUNTIME_DIR not usable, using run/` | `dir` | `XDG_RUNTIME_DIR` 不合要求 |
| `ipc` | DEBUG | `core not reaped` | `error` | 头等拉起的核心退出时出错；在头里发，没人接 |
| `fs` | WARN | `temporary file left behind` | `error` | 整份换成新内容时没盖上去，临时文件也删不掉 |
| `fs` | WARN | `trash record left behind` | `error` | Linux：用不着的 `.trashinfo` 删不掉 |
| `fs` | WARN | `recycle record left behind` | `error` | Windows：移回来以后 `$I` 记录删不掉 |
| `shell` | WARN | `command still running after kill` | | 到时整组杀了，再等 5 秒还没结束 |
| `shell` | DEBUG | `command output still open after the command ended` | | 命令退出了，输出还没关 |
| `shell` | DEBUG | `command output not readable` | `error` | 读命令的输出出错 |
| `shell` | WARN | `command group not killed`、`command tree not killed` | `error` | Unix 杀不掉进程组；Windows 杀不掉进程树 |

`http` 的几行没有 `session` 这一格，可发它们的请求任务带着会话的 span，照第 5 条也带会话编号（`session/actor.md` 第 8 条）。

每一行的细节见各部件的页：`core.md`、`drivers/openai-chat.md` 和 `http.md`、`protocol.md`、`ipc.md`、`fs.md`、`tools/shell.md`。

### 样子

例子（照 `crates/miyu-log/src/layer/tests.rs`、`crates/miyu-session/tests/log.rs` 的写法，编号、时刻、数是编的）：

```text
2026-09-27 21:03:15.284 INFO  core     starting version=0.0.0 pid=4242 root=~/.miyu tz=+09:00
2026-09-27 21:03:16.002 INFO  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 created persona=engineer venue=local tools=7
2026-09-27 21:03:18.410 INFO  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 request seen=6 endpoint=deepseek model=deepseek-flash
2026-09-27 21:03:18.411 DEBUG http     0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 sent host=api.deepseek.com bytes=5120
2026-09-27 21:03:20.104 DEBUG http     0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 ended host=api.deepseek.com status=200 took_ms=1693
2026-09-27 21:03:20.105 INFO  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 ended seen=6 took_ms=1695 in=104 hit=0 out=149
2026-09-27 21:03:20.107 INFO  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 running call=call_8_1 tool=read
2026-09-27 21:03:20.139 INFO  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 ran call=call_8_1 took_ms=32
2026-09-27 21:05:02.771 WARN  session  0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 retrying seen=9 attempt=1 limit=5 wait_ms=3000 class=rate_limited
2026-09-27 21:05:02.900 WARN  hyper::proto something went wrong
```

最后一行是别人家的：来源照原样，比 8 格长，后面照样空一格。

### 出错

| 什么时候 | 怎么说 |
|---|---|
| 建不了目录、打不开文件 | `运行日志写不了：<系统的原话>`；核心照它起不来，原因交给头（`core.md`） |
| 这个进程已经装过了 | `运行日志已经装过了：<原话>` |

装上以后写不进去、换不了份的，不报（第 6 条）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-log/src/level/tests.rs` | 不分大小写、前后空白不算、没设和空的是 `INFO`、读不懂的照 `INFO` 并交回原值 |
| `crates/miyu-log/src/line/tests.rs` | 几列、占几格；什么时候加引号、怎么转义；会话编号、正文不断行；来源去掉 `miyu::`；时刻到毫秒、23 个字符；和 UTC 差多少的写法 |
| `crates/miyu-log/src/home/tests.rs` | 家目录换成 `~`：本身、后面接着路径的；前缀相同的别的目录、前面还接着路径的不换；一段里有几处；末尾带分隔符的家目录；Windows 的 `\\?\`；空的、根不换 |
| `crates/miyu-log/src/layer/tests.rs` | 一条事件一行；会话编号从 span 来，调到 `WARN` 也在；离得最近的 span 胜、后来记进去的也算；低于级别的、别人家低于 `WARN` 的不写；`off` 什么都不写，比 `WARN` 严的别人家也照它；正文和值里的家目录写成 `~` |
| `crates/miyu-log/src/rotate/tests.rs` | 在两行之间换、只留几份、每一份都是整行；比上限长的一行整行写、空的不换；再起来接着写、量了原来多长；Unix 上新建的目录 0700、文件 0600，已经有的不改 |
| `crates/miyu-log/tests/install.rs` | 装上写进 `<名字>.log`，照给的级别记；换了级别照新的（施工 8-2）；一个进程只能装一次 |
| `crates/miyu-session/tests/log.rs` | 会话的每一行、`DEBUG` 的输入和动作、增量在 `TRACE`；日志里没有人说的、她说的、供应商出错的原话 |
| `crates/miyu-session/tests/tool_log.rs` | 调工具的几行；参数、工具交回的字、工作目录都不在日志里 |
| `crates/miyu-session/tests/blocking_log.rs` | 存效果的 blob 存不进去那一行在阻塞线程里发，带会话编号（Unix） |
| `crates/miyu-basesystem/tests/log.rs` | `shell` 的行来源是 `shell`；命令退出了输出还没关那一行在阻塞线程里发，带会话编号（Linux） |
| `crates/miyu-session/tests/recap_log.rs` | 回顾的请求的几行前面带 `recap`（施工 3-8 四补）；不写对话的字 |
| `crates/miyu-session/tests/title_log.rs` | 起标题的请求的几行前面带 `title`（施工 3-8 五补）：两次都没起成就只有两对 `title request`、`title failed`；起成了的 `title ended`；不写对话的字 |
| `crates/miyu-session/tests/http_log.rs` | HTTP 的两行带会话编号；key 不在日志里 |
| `crates/miyu-http/tests/log.rs` | HTTP 的几行；key、请求体、回复的字、地址的路径和参数、出错的原话都不在日志里 |
| `crates/miyu/tests/core.rs` | 真的核心：起来写一行 `starting`，空闲了写 `stopped reason=idle`；第二个核心不写；`starting` 那一行有进程号、数据根（家目录写成 `~`）、和 UTC 差多少；起不来的那一行只写 `stage` |

### 出处

- `28-运行日志.md` 第一节（写到哪、10 MB、留 5 份）、第二节（一行怎么写、字一律英文）、第三节（级别、`MIYU_LOG`）、第四节（写什么，不写什么）；LG1 到 LG3。
- `07-存储.md` 第二节：`state/` 里放运行日志。

### 还没有的

- Windows 上系统报错的原话照系统的语言：标准库取的，管不着。
- 运行中照配置项 `log.level` 换级别：8-1 声明了（`settings.rs`），8-2 起来时读、换一次，运行中改了当场换随 8-4（`28-运行日志.md` 第三节、LG2，`config.md`）。
- 每个软件一份 `state/logs/<软件>.log`，核心记它们的起停和退出码（`28-运行日志.md` 第一节、LG4）。
- `miyu logs`：最后 100 行、`-f`、`--level`、`--session`（`28-运行日志.md` 第五节，`22-命令行.md` 第五节）。
