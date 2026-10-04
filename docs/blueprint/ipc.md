## 本机传输

### 是什么

核心在本机的套接字上等连接，头连过去：Linux、macOS 上是 Unix 域套接字，Windows 上是命名管道。只有本人连得上；连上以后头出示本机令牌（`protocol.md` 握手）。一个数据根只跑一个核心，靠 `run/core.lock` 这把锁。核心没在跑，头把它拉起来，等它说一声好了再连。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-ipc/src/lib.rs` | `open`、`open_locked`：核心起来；`connect`：头连核心 |
| `crates/miyu-ipc/src/place.rs` | 套接字放哪：`Dirs`、数据根的指纹、路径的上限 |
| `crates/miyu-ipc/src/lock.rs` | 单实例锁 `run/core.lock` |
| `crates/miyu-ipc/src/files.rs` | `run/token`、`run/socket` |
| `crates/miyu-ipc/src/listener.rs` | 等连接的一头 `Listener`、连上的一个连接 `Connection` |
| `crates/miyu-ipc/src/unix.rs` | Unix 域套接字：只有自己能进的目录、旧套接字、跟终端脱开 |
| `crates/miyu-ipc/src/windows.rs` | 命名管道：等连接、连过去、核对另一头、跟终端脱开 |
| `crates/miyu-ipc/src/ready.rs` | 核心说「好了」的那一行 |
| `crates/miyu-ipc/src/start.rs` | `connect_or_start`：连不上就拉起核心；`connect_or_start_bare` 同样拉起、不读本机令牌（施工 W-8） |
| `crates/miyu-ipc/src/error.rs` | 起不来、连不上、拉不起的几种情形和它们的话 |
| `crates/miyu-pipe/src/lib.rs`、`windows.rs` | Windows 的安全接口：只对本人开放的管道实例，当前用户和管道另一头的 SID。整个仓库只有这个 crate 放开了 `unsafe`，每个 `unsafe` 块都写着为什么安全 |

### 对外的样子

**`run/` 下的文件**

| 文件 | 内容 | 谁写 |
|---|---|---|
| `run/core.lock` | 空文件，锁在它上面 | 核心 |
| `run/token` | 本机令牌：64 位小写十六进制，加 `\n`；Unix 上权限 0600 | 核心，每次起来 |
| `run/socket` | 一行：套接字的实际位置，路径的原样字节，加 `\n`；Unix 上权限 0600 | 核心，每次起来 |
| `run/token.tmp`、`run/socket.tmp` | 写的时候先写它们，再改名 | 核心 |
| `run/spawn.lock` | 拉起锁，空文件 | 头 |

核心走了，`run/token`、`run/socket` 留着，下一个核心起来重写。

**套接字放哪**

| 平台 | 什么时候 | 位置 |
|---|---|---|
| Linux | 设了能用的 `$XDG_RUNTIME_DIR` | `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock` |
| Linux（没有能用的 `$XDG_RUNTIME_DIR`）、macOS | | `<数据根>/run/core.sock` |
| Linux、macOS | 上面那个超过上限 | `$TMPDIR/miyu-<uid>/<指纹>.sock` |
| Windows | | 命名管道 `\\.\pipe\miyu-<指纹>` |

- **指纹**：数据根路径的 SHA-256 的前 4 个字节，写成 8 位小写十六进制。照数据根写的路径算，不追链接：同一个数据根换个写法算出来不一样也不要紧，头照 `run/socket` 去连。
- **上限**按字节算：Linux 107，macOS 103（系统的上限 108、104 都算上了结尾的零）。临时目录下也放不下，或者没有用户编号：起不来。Windows 上没有上限。
- **能用的 `$XDG_RUNTIME_DIR`**：是绝对路径、顺着链接是一个目录、属主是自己（有效用户编号）、组和别人一点权限都没有。没设的、空的当没设；设了却不能用的也当没设，记一条 `WARN XDG_RUNTIME_DIR not usable, using run/ dir=…`。macOS、Windows 上放套接字不看它；但 macOS 上核对、记 WARN 这一步照样做（`Dirs::current` 在所有 Unix 平台上都调它），只是算出来的位置不用；Windows 上 `runtime_dir` 直接是 `None`，不核对也不记。
- `$TMPDIR` 没设的，是系统默认的临时目录。`<uid>` 是有效用户编号。
- 别的类 Unix 系统照 Linux 的走。

**类型**

| 类型 | 是什么 |
|---|---|
| `Dirs` | 找套接字放哪要看的快照：`platform`、`runtime_dir`（核对过的 `$XDG_RUNTIME_DIR`）、`temp_dir`、`uid`（Unix 上是有效用户编号，Windows 上没有）。`Dirs::current` 从进程里读一次；测试喂一份快照，不改进程的环境变量 |
| `Opened` | 核心起来了：`listener` 等连接，`token` 这一次的本机令牌。打出来时不带令牌 |
| `Listener` | 等连接：`accept` 等下一个，`path` 套接字在哪。丢掉它：先删套接字文件，放在 `$XDG_RUNTIME_DIR/miyu-<指纹>/` 里的连这一层目录一起删，再放锁 |
| `Connection` | 连上的一个连接：异步的字节流，能读能写，交给协议端点（`protocol.md`） |
| `Lock` | 拿着的单实例锁，丢掉就放开 |
| `Ready` | 核心写的那一行：`Ready`、`Running`、`Failed(原因)` |

### 怎么走

**核心起来**（`open` 先拿锁；`open_locked` 接着已经拿到的锁往下走，核心进程用它，`core.md`）

1. 拿锁：打开 `run/core.lock`（没有就建，不清空），标准库的文件锁，不等。拿不到：`Running`。一直拿到核心退出；进程没了，系统替它放开，崩了也不会留下死锁；同一个进程里打开两次也是两把，第二把拿不到。
2. 算出套接字放哪（上表）。
3. 换本机令牌：32 个系统给的随机字节，写成 64 位小写十六进制。先删掉上次留下的 `run/token.tmp`，新建它（Unix 上 0600，已经有的算错），写进令牌加 `\n`，再改名成 `run/token`。旧的就作废了。不同步：断电丢了，下次起来再写。
4. 在套接字上等连接：Unix、Windows 各见下。
5. 记下实际位置：同样先写 `run/socket.tmp`，再改名成 `run/socket`。记一条 `INFO listening socket=<位置>`。
6. 令牌在套接字能连之前就换好了：连上的头读到的一定是新的。

**Unix 域套接字**

1. 套接字所在的目录没有的，建成 0700：只建这一层，上一层要已经在。
2. 核对这个目录只有自己能进：是目录、不是链接，属主是自己，组和别人一点权限都没有。不是的：`NotPrivate`，不用它，也不去改它。放在 `run/core.sock` 的，核对的就是 `run/` 本身。
3. 看套接字的位置上有什么：
   - 什么都没有：绑。
   - 套接字文件，试着连一下被拒绝的：上一个核心崩了留下的旧套接字，删掉，记一条 `INFO stale socket removed socket=…`，再绑。
   - 不是套接字的文件，或者连得上的套接字（别的程序正在听，例如指纹撞了的另一个数据根的核心）：`Occupied`，不动它。
4. 套接字文件本身的权限不另外设：靠所在的目录挡住别人。
5. 核心走的时候删套接字文件（`DEBUG socket file not removed error=…`）。放在 `$XDG_RUNTIME_DIR/miyu-<指纹>/` 里的，这一层目录空了一起删（`DEBUG runtime dir not removed error=…`）；数据根的 `run/`、临时目录下几个数据根共用的 `miyu-<uid>/` 不删（施工 5-11 补：以前留着这一层，一个数据根留一个）。

**命名管道**（Windows）

1. 管道的每个实例都带安全描述符 `D:P(A;;GA;;;<当前用户的 SID>)`：受保护、不继承上一层，只有一条，允许当前用户全部权限。
2. 核心建第一个实例时声明「第一个实例」。名字已经被别的程序占了（拒绝访问）：`Occupied`，不动它。
3. 等连接：连上一个，先建好下一个实例等着，再把连上的交出去。等的时候出了错的实例也换成新建的，不然以后每次接都卡在同一个错上。建下一个实例这一步本身失败的，交回这个错；连上的那个实例没换出来，还留着等：下一次接时对它再等一次连接（已经连着的，系统当场说连上了），建成了下一个实例才把它交出去。少见。
4. 核心走的时候没有文件要删：句柄都关了，管道就没了。
5. `run/` 下的文件靠用户目录本身的访问控制，不另设。

**头连核心**（`connect`）

1. 读 `run/socket`：没有这个文件：`NotRunning`。去掉末尾的一个 `\n`；Unix 上照原样的字节，Windows 上要是 UTF-8；不是绝对路径的：出错（`run/socket 写的不是绝对路径：<路径>`）。
2. Unix：先核对套接字所在的目录只有自己能进（同上）：目录没了：`NotRunning`；不是只有自己能进：`NotPrivate`，不连，免得把令牌交给冒充核心的人。再连：没有套接字文件、被拒绝：`NotRunning`。
3. Windows：连管道。管道忙（核心刚接走一个、下一个实例还没建好），歇 50 毫秒再连，最多再连 100 次，一共 5 秒，还忙的出错；没有这个管道：`NotRunning`。连上以后问出管道服务端的进程，取它的用户 SID，和自己的比：不一样：`NotPrivate`，不交令牌。
4. 连上以后才读 `run/token`，去掉末尾的空白：核心刚换过令牌的，读到的是新的。
5. 交回连接和令牌；令牌在握手时出示（`protocol.md`）。

**不读本机令牌地连**（`connect_bare`、`connect_or_start_bare`，施工 W-8）：同上，只是第 4 步不读 `run/token`，只交回连接。网页软件转发浏览器的连接用它：浏览器的凭据由页面在握手时自己出示，网页软件的代码里拿不到本机令牌（`web-module.md`「起草时定的」第 4 条）。核对目录、核对管道另一头照旧；拉起照「连不上就拉起」。

**连不上就拉起**（`connect_or_start`）

1. 连。连上了就用。连不上、又不是因为没在跑的（`NotPrivate`、读写出错）：出错，不拉起。
2. 没在跑：拿拉起锁 `run/spawn.lock`（没有就建）。别的头正在拉，每 50 毫秒试一次，最多等 10 秒，还拿不到：`Busy`。
3. 拿到了，再连一次：别的头可能刚拉起来。
4. 还连不上，拉起核心：
   1. 命令由调用的一方给：主程序给的是自己加上 `core`（`cli/main.md`）。环境照头的，不另改：没设 key 的 shell 里拉起的核心就没有模型，之后换一个设了 key 的头来连，连上的还是它，一样没有可用的模型，要等它空闲退出（`core.md`）。
   2. 工作目录是数据根，不占着头的当前目录。
   3. 标准输入、标准错误接空；标准输出是一根管道的写端。
   4. 跟终端脱开：Unix 上自成一个进程组；Windows 上 `DETACHED_PROCESS`、`CREATE_NEW_PROCESS_GROUP`，不带控制台窗口。终端里按 Ctrl+C 打不到它。
   5. 头不等它退出：另起一个线程替它收尸。
5. 等它写来一行，最多 10 秒，最多读 4096 字节：
   - `ready`：连它；连不上的照连不上说。
   - `running`：已经有一个核心在跑，可能刚拿到锁、还没开始等连接。每 50 毫秒连一次，最多 10 秒，连上为止；到时还是没在跑：`NotRunning`。
   - `error <原因>`：`Refused`，原因照原样。认不出的一行也当起不来，原因就是这一行。
   - 什么都没写就退了：`Silent`。
   - 10 秒没等到：`Timeout`。
6. 拉起锁一直拿到交回为止（连上了，或者出了错）。
7. 真正保证一个数据根只跑一个核心的是核心自己拿的 `run/core.lock`；拉起锁只是免得几个头同时白拉。清旧套接字的是核心，头不碰。

**那一行**

| 核心写的 | 读成 |
|---|---|
| `ready\n` | `Ready`：好了 |
| `running\n` | `Running`：已经有一个核心在跑，这一个走了 |
| `error <原因>\n` | `Failed(原因)`：写的时候，原因里的 `\r`、`\n` 换成空格，只有一行 |
| 别的 | `Failed(这一行)` |

读的时候先去掉行尾的 `\r`、`\n`。

### 出错

| 情形 | 什么时候 | 说的话 |
|---|---|---|
| `OpenError::Running` | 已经有一个核心拿着锁 | 这个数据根上已经有一个核心在跑 |
| `OpenError::TooLong` | 套接字哪里都放不下 | 套接字的路径太长，放不下：<本该放的位置>；临时目录下也放不下 |
| `OpenError::NotPrivate` | 套接字要放的目录不是自己的，或者别人也能进 | <目录> 不是自己的，或者别人也能进：套接字不放在这里 |
| `OpenError::Occupied` | 位置上有别的东西；Windows 上管道名被占了 | <位置> 上有别的东西：不是套接字的文件，或者别的程序正在听。不动它 |
| `OpenError::Io` | 读写出错 | 系统的原话 |
| `ConnectError::NotRunning` | 核心没在跑 | 核心没在跑 |
| `ConnectError::NotPrivate` | 套接字的目录别人也能进；Windows 上管道另一头不是自己的进程 | <目录或管道> 不是自己的，或者别人也能进：不连 |
| `ConnectError::Io` | 读写出错，`run/socket` 写的不是绝对路径 | 系统的原话；`run/socket 写的不是绝对路径：<路径>` |
| `StartError::Refused` | 核心说了起不来 | 核心起不来：<原因> |
| `StartError::Silent` | 核心什么都没写就退了 | 核心没起来，也没说为什么：看数据根的 state/logs/core.log |
| `StartError::Timeout` | 10 秒没等到那一行 | 等核心起来等了太久 |
| `StartError::Busy` | 10 秒没拿到拉起锁 | 别的程序正在拉起核心，等了太久 |
| `StartError::Connect` | 连不上 | 照 `ConnectError` 的话 |
| `StartError::Io` | 读写出错，例如找不到程序、拉不起来 | 拉不起核心：<系统的原话> |

- 这些话只有中文，不跟界面语言。头（`miyu ask`、`miyu undo`）照原样印在标准错误上，退出码 1；`miyu ask` 没有 key、核心又没在跑的另说，退出码 5（`cli/ask.md`）。
- 运行日志（目标 `miyu::ipc`）：核心里记 `INFO listening socket=…`、`INFO stale socket removed socket=…`、`WARN XDG_RUNTIME_DIR not usable, using run/ dir=…`、`DEBUG socket file not removed error=…`（删套接字文件没删成）。头里发的 `DEBUG core not reaped error=…`（替拉起的核心收尸没收成）记不下来：头不装运行日志。

### 给人看的字

就是「出错」表里的那几句，只有中文。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-ipc/src/place/tests.rs` | 指纹是前 8 位十六进制；Linux 放 `$XDG_RUNTIME_DIR` 下；没有的放 `run/`；macOS 不看它；太长的放临时目录；上限按字节、不算结尾的零；哪里都放不下；Windows 是命名管道 |
| `crates/miyu-ipc/src/lock/tests.rs` | 第二把锁等第一把放开；两个数据根不共用锁 |
| `crates/miyu-ipc/src/files/tests.rs` | 令牌是 64 位小写十六进制、读回来一样、每次换新的、只有自己能读（上次留下的临时文件不沿用）；位置是一行、读回来一样、相对路径不认、不是 UTF-8 的照原样 |
| `crates/miyu-ipc/src/ready/tests.rs` | 三种写了读回来一样；原因只有一行；认不出的是起不来 |
| `crates/miyu-ipc/src/start/tests.rs` | 听得到每一种；没说话就退了的；等太久的 |
| `crates/miyu-ipc/src/unix/tests.rs` | `$XDG_RUNTIME_DIR` 要是绝对路径、只有自己能进；只有自己的真目录才算；别人的不算；新建的 0700，已经有的不动 |
| `crates/miyu-ipc/tests/socket.rs` | 照 `run/socket` 连上、拿到令牌；Linux 在 `$XDG_RUNTIME_DIR` 下；太长的在临时目录下；一个数据根一个核心、走了删套接字、每次换令牌；旧套接字清掉；别的东西不动；别人进得来的目录不用、链接不用；临时目录也要只有自己能进；头不从别人进得来的目录连；没有核心时连不上 |
| `crates/miyu-ipc/tests/pipe.rs` | Windows：照管道连上、拿到令牌，接走一个以后下一个在等；连上就走的不堵管道；一个数据根一个核心；名字被占了不动；没有核心时连不上 |
| `crates/miyu-pipe/src/tests.rs` | 安全描述符的写法 |
| `crates/miyu-pipe/src/windows/tests.rs` | Windows：管道只对本人开放；一个名字只有一个第一个实例；头看到另一头是自己；当前用户是一个 SID |
| `crates/miyu/tests/core.rs` | 真的主程序：拉起、再连不拉起、两个头同时只拉起一个、起不来的说原因、没说话就退了的、已经在跑还没开始等连接的等着连上它、核心的工作目录是数据根 |

### 出处

- `04-核心协议.md` 第二节「各平台的坑」：路径上限、0700 的目录和核对属主、只对本人开放的管道和核对另一头；P2。第四节：连接即身份，本机令牌。
- `07-存储.md` 第二节：套接字放哪、指纹、核对、旧套接字、`run/socket`、`run/token`；第十节：单实例与锁。
- `12-进程形态与分发.md` 第二节「拉起时的握手」。
- `11-权限与沙盒.md` 第五节：本机令牌只有本人读得到。

### 还没有的

- 浏览器、远程用 WebSocket，核心自己查 Origin 头；远程要登录（`04-核心协议.md` 第二节、第四节，P5、P6）。
- 核心拉起的扩展、桥经标准输入输出连（`04-核心协议.md` 第二节）。
- `miyu stdio`：替别的程序连上核心、出示令牌，之后原样转发（`22-命令行.md` 第四节）。
- 沙盒里的命令连不上核心、读不到令牌（`11-权限与沙盒.md` 第五节，A6）：沙盒还没做（M5）。
