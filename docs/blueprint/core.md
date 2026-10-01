## 核心进程 `miyu core`

### 是什么

一个数据根只跑一个的核心进程：由头拉起（`ipc.md`），平时不用人敲。起来时建骨架、拿单实例锁、装运行日志、建管理员的家目录、找资源目录、在本机的套接字上等连接、从环境变量拿模型、登记工具，然后往标准输出写一行 `ready`，在后台清一次回收处；之后一个个接连接，照协议说话（`protocol.md`）。没有连接、没有在跑的回合、也没有在跑的后台命令，空闲够久了自己退出；收到停的信号，先让在跑的会话有计划地停下（后台命令先记 `restarted`、再整组杀）再退出。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `core`、`--idle-seconds` |
| `crates/miyu-core/src/lib.rs` | `main`：起来的先后；管理员 `admin`；工具目录；写那一行 |
| `crates/miyu-core/src/models.rs` | 从环境变量拿模型 |
| `crates/miyu-core/src/serve.rs` | 接连接，空闲退出，停的信号 |
| `crates/miyu-core/src/sandbox.rs` | 起来时找沙盒的助手、探一次，记日志（施工 5-1）；探到了手段的，交回助手（施工 5-4 上） |
| `crates/miyu-core/src/trash.rs` | 起来时清一次回收处；删了的会话留多久 `KEEP`（施工 3-8 三补） |
| `crates/miyu-core/src/settings.rs` | 配置清单：登记各模块的几项；起来时读配置（施工 8-5 起连同密钥文件，环境照进程的）、照 `log.level` 换运行日志的级别（施工 8-2），生成两份 JSON Schema 和参考文件（施工 8-1，`config.md`「怎么走」第一、二条）；运行中跟着配置换级别、重写这三份（`settings/follow.rs`，施工 8-4） |
| `crates/miyu-sandbox/src/lifeline.rs`、`lifeline/` | 核心没了，它起的命令跟着没（施工 7-8，下面「子进程随核心退出」）：Unix 上每条命令的组里一个看门的，Windows 上核心进作业对象 |
| `crates/miyu-ipc` | 单实例锁、套接字、本机令牌、那一行的写法（`ipc.md`） |
| `crates/miyu-endpoint` | 协议端点：核心的家底 `Core`（里面有执行器的任务表，施工 7-3）、接连接、空不空闲（`protocol.md`） |
| `crates/miyu-basesystem`、`crates/miyu-tool` | 基础系统的七件工具、工具目录（`tools/interface.md`） |
| `crates/miyu-log` | 运行日志（`log.md`） |
| `crates/miyu-store` | 数据根、资源目录（`store.md`） |
| `crates/miyu-sandbox` | 沙盒：找助手、探测（`sandbox.md`） |

### 对外的样子

命令：`miyu core [--idle-seconds <秒>]`。它和它的参数都不写进帮助。

| 参数 | 做什么 |
|---|---|
| `--idle-seconds <秒>` | 空闲多少秒以后退出，非负整数；不写是 600 秒（10 分钟） |

用到的环境变量（拉起的核心照拉起它的头的环境，`ipc.md`）：

| 变量 | 做什么 |
|---|---|
| `MIYU_HOME` | 数据根；不设是家目录的 `.miyu`（`store.md`） |
| `MIYU_RESOURCES` | 资源目录，开发时指到源码树的 `resources/`（`store.md`） |
| `MIYU_LOG` | 运行日志记到哪一级，压过配置项 `log.level`（`log.md`、`config.md`） |
| `DEEPSEEK_API_KEY` | 模型的 key，起来时读一次 |
| `MIYU_DEV_BASE_URL`、`MIYU_DEV_MODEL` | 开发用：设了 key 的，替换地址、模型（下面「模型」第 1 条，施工 3-9 再补）。不进 `-h`，配置系统做好以后删掉 |
| `MIYU_DEV_WINDOW` | 开发用：设了 key 的，当这个模型的上下文窗口，压过模型资料里的（施工 6-3 上）。同上，不进 `-h` |
| `XDG_RUNTIME_DIR`（Linux）、`TMPDIR` | 套接字放哪（`ipc.md`） |
| `HTTPS_PROXY`、`HTTP_PROXY`、`NO_PROXY` 这些 | 请求模型走不走代理（`http.md`） |

- 标准输出上只写一行（下面「那一行」）。标准输入、标准错误不用。
- 家目录（`std::env::home_dir`）交给协议端点：权限策略照它换 `~`，工作目录太宽照它判（`protocol.md`）。

写到数据根里的：

| 文件 | 是什么 |
|---|---|
| `.miyu-root`、`system/`、`home/`、`state/`、`run/` | 骨架（`store.md`） |
| `home/admin/`、`home/admin/workspace/` | 管理员的家目录和工作区 |
| `home/admin/trash/sessions/` | 回收处：删掉的会话，留 7 天（`store.md` 第 12 条，施工 3-8 三补）。起来时清掉满了 7 天的 |
| `state/logs/core.log` | 运行日志，满 10 MiB 换一份，留 `core.log.1` 到 `core.log.5`（`log.md`） |
| `run/core.lock`、`run/token`、`run/socket` | 单实例锁、本机令牌、套接字的位置（`ipc.md`） |
| `run/core.sock` | 套接字本身，放在 `run/` 的时候（`ipc.md`「套接字放哪」） |

### 怎么走

**起来的先后**

1. 读一次环境的快照（`MIYU_HOME`、`MIYU_RESOURCES`、家目录、程序的真实位置这些），数据根、资源目录、沙盒的助手照它找；`MIYU_LOG`、放套接字的目录、`DEEPSEEK_API_KEY` 到用的那一步才读。
2. 找数据根，建骨架（`store.md`）。
3. 拿单实例锁 `run/core.lock`，不等。拿不到：已经有一个核心在跑，写 `running`，退出码 0，运行日志一个字都不写。先拿锁、再装日志：两个核心不写同一份日志。
4. 装运行日志 `state/logs/core.log`，级别照 `MIYU_LOG`，带上第 1 步的家目录（`log.md`）。记一条 `INFO starting version=<版本> pid=<进程号> root=<数据根> tz=<和 UTC 差多少>`，数据根里的家目录写成 `~`，例如 `root=~/.miyu tz=+09:00`。接着让它起的子进程随它结束（下面「子进程随核心退出」，施工 7-8）：Windows 上进作业对象，进不去记一条 `WARN children not bound error=…`，照样起来；别的平台这一步什么都不做。
5. 管理员的家目录 `home/admin/` 和工作区 `home/admin/workspace/`，没有就建；Unix 上新建的权限 0700。管理员的账号固定叫 `admin`。
6. 找资源目录（`store.md`）。找到以后读配置（施工 8-2，`config.md`「怎么走」第二条）：系统配置、管理员的个人设置、信任的记录、密钥文件（施工 8-5），读不进来的照空的，有问题的每份记一条 `WARN config problems`（密钥文件组、别人读得到的记一条 `WARN secrets readable by others`）；照 `log.level` 的最终值换运行日志的级别，记一条 `INFO log level`。再照配置清单生成两份 JSON Schema 和参考文件，放在 `state/config/`（施工 8-1，`config.md`「怎么走」第一条第 6 到 8 条）：字照 `ui.language` 的最终值，`auto` 的照系统的语言挑，一样的不重写；读好的配置交给协议端点（`Core::with_config`）；写不成的、字缺了的，一份记一条 `WARN config schema not written`（目标 `miyu::config`），不影响起不起得来。说「好了」之前开始监视配置文件（`Core::watch_config`，施工 8-4，`config.md` 第七条），起一个任务跟着配置换：`log.level` 变了换级别，`ui.language` 变了重写那三份（第八条）；监视起不来的记一条 `WARN config watch unavailable`，照样起来。
7. 起运行时：多线程，两个工作线程，接连接、会话、请求都在上面。
8. 算出套接字放哪、换本机令牌、在套接字上等连接、记下实际的位置（`ipc.md`）。
9. 从环境变量拿模型（下面「模型」）。
10. 找沙盒的助手、探一次（`sandbox.md`「怎么走」第 1、2 条）：记一行 `INFO` `sandbox` 或者 `WARN` `sandbox unavailable`（施工 5-1）。探到的结果（能不能用、为什么，`Availability`）交给协议端点：握手时报给头（施工 5-4 下）；能用的，造会话、载入时把助手交给会话，权限策略照它判执行命令，执行器照它带沙盒（施工 5-4 上）；用不了的，会话里当沙盒用不了。
    - 再算出缓存目录（`store.md` 第 3 条），沙盒的缓存放在它下面的 `sandbox/<账号>/`，造会话、载入时照属主交给会话（`session/tools.md` 第 1a 条，施工 5-4 下）。算不出来的：记一行 `WARN sandbox cache unavailable`，`reason` 是 `no home directory` 或者 `no LOCALAPPDATA`（运行日志一律英文），沙盒里不设工具链的变量。你的 cargo 目录：核心的环境里 `CARGO_HOME` 设了、不是空的照它，不然 `~/.cargo`。
    - 这两样都不影响起不起得来。
11. 工具目录：登记基础系统，十一件：`edit`、`glob`、`grep`、`history`、`jobs`（施工 7-4）、`message_agent`（施工 7-7）、`read`、`shell`、`subagent`（施工 7-5；7-5 再补从 `agent` 改名，以前的名字照样找得到）、`trash`、`write`；工具的字从资源目录读，登记完就冻结（`tools/interface.md`）。
12. 核心的家底：数据根、资源目录、模型、工具目录、系统的家目录、管理员 `admin`、本机令牌，会话表是空的，执行器的任务表是空的（施工 7-3，`protocol.md`）。不载入任何会话，只打开管理员的会话列表的索引 `home/admin/index/sessions.db`，一直开着：没有的新建，读不了、坏了、版本不对的删掉换一份空的，列会话时照日志补；都不影响起不起得来（施工 3-8 七补，`store/index.md`「怎么走」第 1 条）。会话表造会话、载入时交给会话一份造子会话的端口（施工 7-5，`protocol.md`「会话表」第 7 条）。
13. 往标准输出写一行 `ready`。
14. 清一次回收处（施工 3-8 三补，`store.md` 第 12 条第 2 款）：管理员的回收处里删了满 7 天（`KEEP`，2026-09-30 项目主人定）的会话连目录删掉。写了 `ready` 以后在阻塞线程里清，不耽误头连上来、第 15 步照常；核心退出之前等它清完。钟是这时系统的钟，读不出的当 1970 年（什么都不满时限，一个都不删）。删了的记一条 `INFO trash purged removed=<几个>`，一个都没删的不记；读不出删的时刻、删不掉的，一个一条 `WARN trash entry kept session=… error=…`；回收处读不了的记 `WARN trash not read error=…`。都不影响起不起得来。
15. 一个个接连接，直到停下（下面「停下」）。

第 2 到 11 步哪一步出了错（第 3 步拿不到锁的除外；第 10 步探沙盒只记日志，不会出错）：写 `error <原因>`，退出码 1；运行日志已经装上了的（第 5 步起），再记一条 `WARN not started stage=<哪一步>`：第 5 到第 9 步依次是 `home`、`resources`、`runtime`、`socket`、`models`，第 11 步是 `tools`。原因是给人看的中文，只交给头，不进运行日志（施工 4-9 再补四上：原来 `reason=<原因>` 整句写进去）。第 8 步以后出的错，走的时候照样删掉套接字文件、放开锁（`ipc.md`）。

**那一行**

头拉起核心时，把一根管道的写端给它当标准输出，等它写来一行（`ipc.md`）：

| 写的 | 意思 | 退出码 |
|---|---|---|
| `ready` | 好了：在套接字上等连接了 | 之后照「停下」 |
| `running` | 已经有一个核心在跑：这一个走了，头去连那一个 | 0 |
| `error <原因>` | 起不来。原因是出错的原话，照原样给人看；里面的回车、换行换成空格 | 1 |

- 每一种都带一个 `\n`，写完马上送出去。写不出去的，记一条 `WARN ready line not written error=…`（运行日志装上了的话）。
- 原因是出错的原话：Miyu 自己写的多是中文，例如 `MIYU_HOME 要写绝对路径，写的是 …`、`找不到资源目录：… 都没有。开发时设 MIYU_RESOURCES 指到源码树的 resources/`（`store.md`、`ipc.md`）；工具的字读不出来的是 `<哪一份文件>: <为什么>`；工具登记时查不过的是英文，例如 `tool "read": another tool has the same name`（`tools/interface.md`）；系统和用到的库报的错（例如 HTTP 客户端造不出来的）照它们的写法。

**模型**

1. `DEEPSEEK_API_KEY` 去掉前后空白不是空的：接 DeepSeek 官方。

   | 项 | 值 |
   |---|---|
   | 地址 | `https://api.deepseek.com` |
   | 端点的编号 | `deepseek`：记进 `model.called` 和运行日志 |
   | 模型 | `deepseek-flash` |
   | 写法 | DeepSeek 的兼容写法（`drivers/openai-chat.md`） |
   | 输出的上限 | 不设 |
   | 模型能收的输入 | 能看图，不能读 PDF（施工 4-13：DeepSeek 2026-08-21 起收图，只收 `user` 消息里的，工具结果里的图由驱动挪过去，`drivers/openai-chat.md` 第 7 条） |
   | 空闲超时 | 180 秒：多久没收到新的字节就算断了（`http.md`） |
   | 代理 | 照环境变量（`http.md`） |
   | key | 去掉前后空白；只在内存里，不落盘、不写配置、不进日志 |

   开发用的两个变量（施工 3-9 再补，2026-09-29 项目主人定）：`MIYU_DEV_BASE_URL` 去掉前后空白不是空的，替换地址，端点的编号改成 `dev`；`MIYU_DEV_MODEL` 去掉前后空白不是空的，替换模型名，不合模型名写法的起不来，原因写明是这个变量。别的照上表。设了哪个，`INFO` 记一条 `dev endpoint base_url=<地址> model=<模型>`。

   **模型的限额**（施工 6-3 上）：照资源目录里的模型资料（`store/resources.md`）查窗口、最大输出。驱动用的是 DeepSeek 的写法，开发端点也照 `deepseek` 那一家查模型名；查不到的没有，不主动压。`MIYU_DEV_WINDOW` 去掉前后空白不是空的：是正整数的当窗口，压过查到的；不是的起不来，原因写明是这个变量。一张图怎么算跟着驱动的写法走：DeepSeek 的交官方计算器 v41 配置的算法（`miyu-drivers` 的 `DeepSeekImages`）。起来时 `INFO` 记一条 `model limits model=<模型> window=<窗口或 none> max_output=<最大输出或 none>`。会话 actor 造会话、载入以后，先把这些交给内核（`Input::Limits`，`session/actor.md`）。

   模型资料读不出来、格式坏了：起不来，原因写明是 `models/models-dev.json`。HTTP 客户端造不出来（系统的证书读不了之类）：起不来。
2. 没设、空的、全是空白：照样起来，记一条 `WARN DEEPSEEK_API_KEY not set, no model`。每次请求都当场说完、没发出去：出错，分类 `auth`（认证失败），原话 `no model: set DEEPSEEK_API_KEY`；`model.called` 里没有端点和模型（没发出去）。分类是认证失败，内核不重试（`kernel/session.md`）。运行日志里 `request` 那一行写 `endpoint=none model=none`。
3. key 只在起来时读一次：换了 key，要等这个核心退出、下一次拉起。

**子进程随核心退出**（施工 7-8，`12-进程形态与分发.md` R5，`agents.md` 第八条）

有计划地停下时核心自己先记 `restarted`、再整组杀掉后台命令（下面「停下」第 4 条）。这一条管的是核心崩了、被 `SIGKILL`、被任务管理器结束的时候：没人来杀，命令得自己知道核心没了。三个平台各用自己最稳的现成做法（`crates/miyu-sandbox/src/lifeline.rs`）：

1. Unix（Linux、macOS 一样）：核心手里一根「生命线」管道，写端带 `O_CLOEXEC`、只在核心手里，核心怎么没的，内核都会关掉它。`shell` 起每条命令（前台、后台，经不经沙盒的助手都一样）之前，先起一个看门的 `/bin/sh`（环境变量清空、工作目录是 `/`、标准输入是生命线的读端、标准输出和错误接空的），自成一组；命令起在它的组里，整组杀照这个组号。看门的只做一件事：`while read -r _; do :; done; kill -s KILL 0`：读到结尾（核心没了）就杀掉自己所在的整个组，命令和它起的孙进程都在里面。看门的只认自己的组，它在组里组号就不会被别人占，不会杀错。命令自己退出、被停、超时时，整组杀会连它一起杀掉；丢掉这条命令时收掉它，不留僵尸进程。看门的起不来的（`/bin/sh` 没有、生命线建不起来），记一行 `WARN command lifeline not started error=…`（目标 `miyu::shell`），照旧自成一组：核心崩了它不跟着死，载入时照样补 `aborted`。
2. Windows：核心起来时把自己放进一个带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的作业对象（起来的先后第 4 步），句柄拿到进程结束。之后起的子进程、子进程再起的，都自动在里面；核心没了，系统关掉句柄，里面的全部结束。有计划地退出时后台命令已经杀了，剩下的（`taskkill`、跑到一半的前台命令）也跟着结束。
3. 没选的：Linux 的 `PR_SET_PDEATHSIG` 盯的是起子进程的那个线程，运行时线程池里的线程一退，命令就被误杀，而且只管直接的子进程，组里别的照样活；子进程收割者（`PR_SET_CHILD_SUBREAPER`）只管收养孤儿，核心没了它也没了；macOS 的 kqueue 盯父进程要另起一个进程盯着，和生命线一样，却只有一个平台能用；Windows 上一条命令一个作业对象要在起好以后才放进去，中间起的孙进程漏掉。
4. 再载入时内核照样给没有结束记录的后台命令补 `aborted`（`kernel/session.md`「载入和崩溃」第 9 条）：进程是真的没了。

**停下**

1. 每隔一会儿看一次空不空闲：空闲时限的四分之一，最少 100 毫秒，最多 30 秒；600 秒的是每 30 秒。起来时马上看第一次。
2. 空闲：没有连接（握没握手都算），没有忙着的会话，也没有在跑的后台命令（结束了、记录还没落盘的也算；`12-进程形态与分发.md` R1，施工 7-3；`protocol.md`「空闲和停下」）。
3. 看到空闲，从这一次看的时刻记起；哪一次看的时候还空闲、已经够了空闲时限，就停。中间哪一次看到不空闲，从头记。所以从真的空下来算，到退出，至少是空闲时限，最多再多两个间隔；600 秒的，在 600 到 660 秒之间：记起的时刻和后来看的时刻都是当时的钟，正好满 600 秒的那一次看可能差几微秒不够，要等下一次。`--idle-seconds 0` 的，第一次看到空闲就停。
4. 停的信号：Ctrl+C（SIGINT），Unix 上还有 SIGTERM。收到了，先有计划地停下全部在跑的会话（跑到一半的回合记成「重启了」，下次载入接着干；在跑的后台命令先记 `restarted`、落了盘再整组杀，R5，施工 7-3），再停。拉起的核心自成一个进程组，终端里按 Ctrl+C 打不到它，要停得明着发。
5. Unix 上装不上 SIGTERM 的，记一条 `WARN SIGTERM not watched error=…`，只等 Ctrl+C。只等 Ctrl+C 的（Windows 上、Unix 上装不上 SIGTERM 的）连 Ctrl+C 也装不上，记一条 `WARN Ctrl+C not watched error=…`，当它不会来。SIGTERM 装得上时，等的是 Ctrl+C 和 SIGTERM 一起 `select!`：这时 Ctrl+C 装不上，照样记一条 `WARN Ctrl+C not watched error=…`，当它不会来，只等 SIGTERM（施工 4-9 再补三上）。
6. 停之前记一条 `INFO stopped reason=idle`（空闲）或 `reason=signal`（信号）：那时锁还在手里，下一个核心起来之前，这份日志还是它一个人写。
7. 然后删掉套接字文件、放开锁，连接跟着没了，退出码 0。

### 出错

| 什么时候 | 那一行 | 退出码 |
|---|---|---|
| 已经有一个核心在跑 | `running` | 0 |
| 找不到数据根、建不了骨架、拿锁出了别的错、装不上运行日志、建不了管理员的家目录、找不到资源目录、起不了运行时、套接字放不下或者绑不上、令牌写不进、HTTP 客户端造不出来、工具的字读不出来或者登记时查不过 | `error <原因>` | 1 |
| 参数不对（`--idle-seconds` 不是非负整数、不认识的参数） | 不写那一行，clap 把错印在标准错误上 | 2 |

运行日志（目标 `miyu::core`）：

| 级别 | 这件事 |
|---|---|
| `INFO` | `starting version=… pid=… root=… tz=…` |
| `WARN` | `DEEPSEEK_API_KEY not set, no model` |
| `WARN` | `children not bound error=…`（Windows：进不了作业对象，施工 7-8） |
| `INFO` | `sandbox helper=… platform=… mechanisms=…`（施工 5-1） |
| `WARN` | `sandbox unavailable reason=…`（施工 5-1） |
| `WARN` | `sandbox cache unavailable reason=…`（施工 5-4 下） |
| `WARN` | `not started stage=…` |
| `WARN` | `ready line not written error=…` |
| `INFO` | `trash purged removed=…`（施工 3-8 三补） |
| `WARN` | `trash entry kept session=… error=…`、`trash not read error=…` |
| `ERROR` | `trash purge panicked error=…` |
| `WARN` | `SIGTERM not watched error=…`、`Ctrl+C not watched error=…` |
| `INFO` | `stopped reason=idle`、`stopped reason=signal` |

配置那几行的目标是 `miyu::config`：`WARN config schema not written file=state/config/<文件名> error=…`（第 6 步，施工 8-1）；`WARN config problems file=… errors=… warnings=…`、`WARN trust not read file=… error=…`、`WARN MIYU_LOG not understood, using config value=…`、`INFO log level level=… from=…`（第 6 步，施工 8-2）；`WARN config watch unavailable error=…`、`INFO config changed layer=… via=file keys=…`（第 6 步以后，施工 8-4）；`WARN secrets readable by others file=system/secrets.toml`（第 6 步、手改重读时）、`INFO secret changed name=… action=… via=…`（只有名字，施工 8-5）。

套接字的几行（`listening`、`stale socket removed`、`XDG_RUNTIME_DIR not usable`）见 `ipc.md`，连接、会话的见 `protocol.md`、`session/actor.md`。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu/tests/core.rs` | 头拉起真的 `miyu core`，等它说好了再连；管理员叫 `admin`，建好了它的家目录；再连不再拉起；两个头同时只拉起一个；起不来的说原因（找不到资源目录），日志里只写 `stage=resources`；已经在跑的写 `running` 就走、不写日志；什么都没写就退了的；空闲了自己走，日志里一条 `starting`、一条 `stopped reason=idle`；`starting` 那一行的数据根在家目录下的写成 `~`、有和 UTC 差多少；工作目录是数据根；起来时清一次回收处：删了满 7 天的删、没满的留，记一条 `trash purged removed=1`（施工 3-8 三补）；起来时探一次沙盒的助手：旁边有助手的记 `sandbox` 那一行、平台是这台机器的、有手段那一格，没有的记找不到（施工 5-1）；握手报的沙盒和记下的对得上（施工 5-4 下） |
| `crates/miyu/tests/crash.rs`（施工 7-8） | 真的 `miyu core`，模型是本机回环上的假服务器：放一个心跳到后台（Unix 上是命令起的孙进程），硬杀核心（Unix `SIGKILL`，Windows 结束进程、不连子进程），心跳几秒内停下；三个平台都跑 |
| `crates/miyu-sandbox/src/lifeline/tests.rs`（施工 7-8，Unix） | 生命线还在，组里的照常跑；写端一关，看门的把整个组杀掉，孙进程也在里面 |
| `crates/miyu-core/tests/serve.rs` | 空闲退出、放开锁和套接字；有头连着不退；空闲的钟从最后一个头走时算起；在跑的回合不退；有在跑的后台命令不退、结束了记下再退（施工 7-3）；收到停的信号先停下会话、跑到一半的记成重启了，后台命令先记 `restarted`、落了盘再杀（施工 7-3）；没有 key（没设、全是空白）每次请求都说没有模型、分类是认证失败、没发出去 |
| `crates/miyu-core/src/serve/tests.rs` | 多久看一次：四分之一，最多 30 秒，最少 100 毫秒；装不上的 Ctrl+C 当它不会来（造不出真的装不上，测的是等它的那一小段） |
| `crates/miyu-core/tests/tools.rs` | 工具目录里是基础系统的七件；资源目录坏了，说是哪一份 |
| `crates/miyu-core/src/models/tests.rs` | 请求 DeepSeek 时的模型名、不写输出上限、收图不收 PDF（施工 4-13） |
| `crates/miyu-core/src/sandbox/tests.rs` | 探沙盒的助手：旁边没有的、不知道主程序在哪的记找不到，跑不了的记原因（施工 5-1）；探到了手段的交回能用和助手，手段是空的、找不到、探不成的交回用不了和原因（施工 5-4 上、下）；沙盒的缓存在缓存目录下的 `sandbox`，cargo 目录照 `CARGO_HOME`、空的当没设、不然 `~/.cargo`，算不出缓存目录的记一行、交回空的（施工 5-4 下） |

### 出处

- `12-进程形态与分发.md` 第二节：按需运行、空闲多久、停的信号；「拉起时的握手」（那一行、先拿锁再装日志、清旧套接字的是核心）。第三节：一个主程序，`miyu core` 是它的子命令。
- `07-存储.md` 第二节（数据根、骨架、`run/`）、第十节（单实例与锁）。
- `06-多用户与身份.md` U13：管理员固定叫 `admin`。
- `15-模型与供应商.md` 第七节：配置做出来之前，只认 `DEEPSEEK_API_KEY`。
- `28-运行日志.md`：写到 `state/logs/`，`MIYU_LOG`。
- `04-核心协议.md` 第九节 `session.delete`：删了的进回收处、留 7 天（2026-09-30 项目主人定，施工 3-8 三补）。

### 还没有的

设计里有、还没做的：

- 常驻：开了通讯平台桥、定时任务、远程访问、桌面语音时不退出，登记成登录时启动的服务（`12-进程形态与分发.md` 第二节，`miyu service install`）。
- 空闲时限放进配置（第二节）。
- 恢复会话、回收 blob 这类重活放到说好了之后（第二节「拉起时的握手」）。清回收处（施工 3-8 三补）已经照这样放在 `ready` 之后。
- 「核心先 bind 好套接字、能接受连接了就往管道里写」（第二节「拉起时的握手」）：现在绑好以后还要造模型端口、登记工具（读资源目录的字）才写 `ready`，比设计说的晚。
- 头发现核心比自己旧，请求它空闲时重启（`04-核心协议.md` 第八节）。
- 模型从配置、供应商、池来（`15-模型与供应商.md`）；首次运行时的引导（`12-进程形态与分发.md` 第七节）。
- `miyu status`、`miyu doctor`（`12-进程形态与分发.md` 第三节、R13）。
