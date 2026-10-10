## 沙盒：`miyu-sandbox`

### 是什么

沙盒的底子：
- 核心给每条要关起来的命令写一份规格：哪些能写，哪些藏起来（读写都不行）；别的都能读，不能写；
- 小程序 `miyu-sandbox` 照规格先把自己收紧，再换成那条命令。

它是单独的一个小程序（`11-权限与沙盒.md` 第六节 A7）。

现在 Linux（Landlock，施工 5-2、5-3）、macOS（Seatbelt，施工 5-7）上照规格收紧；Windows 还不收紧（5-9），探测报的手段是空的，核心照沙盒用不了办（施工 5-4 上）。

这一页管各平台共用的：规格、助手的命令行和退出码、探测、找助手、`shell` 怎么经助手起。各平台怎么收紧各有一页，随那一步的施工写：Linux `sandbox/linux.md`（5-2 起），macOS `sandbox/macos.md`（5-7），Windows `sandbox/windows.md`（5-8 起）。沙盒只管读写权限，不管网络（2026-09-29 项目主人定）。助手里收紧的代码也是各平台一个文件，几条线可以同时施工，各改各的（2026-09-28 项目主人同意分线并行）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/lib.rs` | 交出去的几样 |
| `crates/miyu-sandbox/src/spec.rs` | 规格 |
| `crates/miyu-sandbox/src/wrap.rs` | 照规格包一条命令：`miyu-sandbox run --spec … -- …` |
| `crates/miyu-sandbox/src/locate.rs` | 找助手：主程序旁边 |
| `crates/miyu-sandbox/src/probe.rs` | 探测：跑 `miyu-sandbox probe`，读它说的 |
| `crates/miyu-sandbox/src/availability.rs` | 这台机器上的沙盒能不能用、为什么（施工 5-4 下） |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/main.rs` | 助手里各平台共用的：读参数、读规格、`probe`、出错时说的几句 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux.rs`、`macos.rs`、`windows.rs`、`other.rs` | 各平台收紧、再换成命令（`run`），探测时报的手段（`mechanisms`）；`other.rs` 是别的 Unix |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/unix.rs` | Unix 上换成命令（`exec`） |
| `crates/miyu-tool/src/run.rs` | 一次调用带的 `sandbox` |
| `crates/miyu-basesystem/src/shell.rs` | 带了规格的经助手起命令 |
| `crates/miyu-core/src/sandbox.rs` | 起来时探一次，记日志；探到了手段的，助手交给协议端点（施工 5-4 上） |
| `crates/miyu-session/src/sandbox.rs` | 执行器给每次调用写规格（施工 5-4 上，`session/tools.md`） |
| `crates/miyu-sandbox/src/testkit.rs` | 测试用的：cargo 编出来的助手在哪（`testkit` 开关，施工 5-4 上）。会话、命令行的测试经它真的起命令 |

### 对外的样子

**规格** `Spec`，助手的 `--spec` 收的就是它的 JSON。路径都是真实的位置：

| 格 | 是什么 |
|---|---|
| `write` | 能写的目录、文件 |
| `hidden` | 读写都不行的，例如数据根：里面有本机令牌，读到就能冒充本人 |

- 规格以外的都能读，不能写；`/dev/null` 总能写（2026-09-29 项目主人定，照 DeepSeek 的 dsh：整盘能读，只管写）。平常的命令要用的几样设备，各平台另放行，写在各自那一页（macOS 上还有 `/dev/zero`、`/dev/fd`、终端，`sandbox/macos.md`）。
- 规格里一条能写的都没有，就是全盘只读：只读这一级（`11-权限与沙盒.md` A12）。
- 几条重叠的时候，越深的越算数：能写的落在藏起来的里面，照样能写（工作区在数据根里）；藏起来的落在能写的里面，照样藏（Linux 上挖不了洞，拒绝执行，`sandbox/linux.md`）。
- 规格里没有网络：沙盒只管读写权限（2026-09-29 项目主人定）。

样本 `docs/designs/samples/sandbox/spec.json`（例子）：

```json
{"write":["/home/me/project","/tmp/miyu-sandbox"],"hidden":["/home/me/.miyu"]}
```

读的时候，认不得的格（包括原来的 `readonly`、`network`）、类型不对的，都当规格写坏了：助手不懂的限制，不能悄悄跳过。两格不写都是空的。

**只准读名单里的**（`read`，施工 5-12 下，外部身份的会话，`11-权限与沙盒.md` 第三节）：写了的，除了它列的和 `write` 的，别处都读不到、列不出、执行不了，`hidden` 不再另算；不写的照旧整盘能读。现在只有 Linux 照它收紧（Landlock：根目录不再整个放列目录、执行，名单里的每一条放读、列目录、执行）；别的平台（`miyu_sandbox::CONFINES_READS` 是假的）助手收到写了它的规格说 `cannot confine: reads cannot be confined on this platform`、退出 125、不跑，宁可不跑也不漏读。

**助手的命令行**：

- `miyu-sandbox run --spec <规格的 JSON> -- <程序> [参数…]`：照规格收紧自己，再换成这条命令。
  - Unix 上直接换成它（`exec`），进程还是同一个。
  - Windows 上起一个子进程，等它，照它的退出码退出。
  - 成了什么都不印：它的标准错误就是命令的标准错误，印了会混进给她看的输出。
- `miyu-sandbox probe`：标准输出上一行 JSON，说这台机器能收紧到什么程度，例如 `{"version":1,"platform":"linux","mechanisms":[]}`。`platform` 是 `linux`、`macos`、`windows`、`other` 之一；`mechanisms` 是这台机器上能用上的收紧手段，各平台自己报（Linux 上是 `landlock`，`sandbox/linux.md`；macOS 上是 `seatbelt`，`sandbox/macos.md`），空的就是收紧不了。核心只认 `version` 是 1 的；多出来的格不管。

**退出码**，照 `env`、`timeout` 的约定：

| 码 | 什么时候 |
|---|---|
| 125 | 助手自己出了错：参数不对、规格写坏了、`--` 后面没有程序、收紧失败 |
| 126 | 找到了命令，执行不了 |
| 127 | 找不到命令 |
| 别的 | 命令自己的 |

**能不能用** `Availability`（施工 5-4 下）：核心起来时探一次得出来，交给协议端点。能用（`Usable`）带着助手的路径；用不了（`Unusable`）带着原因，三种：

| 原因 | 什么时候 | 协议上写成 |
|---|---|---|
| `HelperMissing` | 主程序旁边没有助手，或者不知道主程序在哪 | `helper_missing` |
| `HelperFailed` | 助手跑不起来、超时、退出码不是 0、说的读不懂 | `helper_failed` |
| `NoMechanism` | 探成了，手段是空的 | `no_mechanism` |

**一次调用带的** `Call.sandbox`（`tools/interface.md`）：`Some(Sandboxed { helper, spec, env })` 的，`shell` 经助手起命令，`env` 里的环境变量照白名单之后设上（同名的盖掉）；`None` 的照旧直接起。执行器照这一刻实际生效的级别带（施工 5-4 上，`session/tools.md`）。

### 怎么走

1. **找助手**（`locate`）：主程序真实位置旁边的 `miyu-sandbox`，Windows 上是 `miyu-sandbox.exe`。不是普通文件的、没有的，是空的。
2. **核心起来时探一次**：
   - 主程序的真实位置照环境的快照（`core.md` 第 1 步）。
   - 找到了就跑 `miyu-sandbox probe`，最多等 5 秒：读它的输出、等它退出加起来不过 5 秒，到时杀掉它。读输出在另一个线程里，它放出去的东西拿着管道不放，也不一直等。
   - 成了：记一行 `INFO` `sandbox`，字段 `helper`（路径，家目录写成 `~`）、`platform`、`mechanisms`（逗号连起来，空的写 `none`）。
   - 没找到、跑不了、超时、说的读不懂：记一行 `WARN` `sandbox unavailable`，字段 `reason`，是这几句之一：`helper not found`、`cannot run helper: <原话>`、`helper timed out`、`helper failed: <退出码>`、`helper output not understood: <原话>`（版本不是 1 的写 `version <几>`）。
   - 探到了手段（`mechanisms` 不是空的）：能用，助手交给协议端点，造会话、载入时交给会话，权限策略照它判执行命令，执行器照它给每次调用写规格（施工 5-4 上，`core.md`）。手段是空的、探不成的：用不了，照上面的表记下原因，会话里工作区、只读两级执行命令都要问人（`session/guard.md`）。能不能用、为什么，握手时报给头（`protocol.md`，施工 5-4 下）。起不起得来不看它。
3. **`shell` 带了规格的**：命令写成 `<助手> run --spec <规格的 JSON> -- <shell> <shell 的参数…>`。工作目录、环境变量白名单、标准输入输出、进程组、超时整组杀都和直接起一样：Unix 上助手换成了 shell，是同一个进程。规格写不成 JSON 的（里面有不是 UTF-8 的路径）：照「起不来」说，不会不经沙盒就跑。
4. **助手的 `run`**：
   1. 读参数：不是 `run --spec <JSON> -- <程序> …` 的样子，印 `miyu-sandbox: usage: miyu-sandbox run --spec <json> -- <program> [args...]`，退出 125。
   2. 读规格：读不懂的，印 `miyu-sandbox: bad spec: <原话>`，退出 125。
   3. 收紧：交给这个平台的文件（`linux.rs`、`macos.rs`、`windows.rs`，别的 Unix 是 `other.rs`），怎么收紧写在各平台那一页；Windows、别的 Unix 还什么都不做。收紧不成的，一律印 `miyu-sandbox: cannot confine: <原话>`，退出 125，不跑命令。
   4. 换成命令。Unix 上 `exec`，找不到的印 `miyu-sandbox: cannot run <程序>: <原话>`、退出 127，别的原因执行不了的一样印、退出 126。Windows 上起子进程：起不来的照这两条；起来了就等它，照它的退出码退出。
5. **助手的 `probe`**：印那一行 JSON，退出 0。手段（`mechanisms`）由这个平台的文件报，各平台自己定写什么，写进它那一页。
6. 助手的字一律英文：它印在命令的输出里，她看得到（`26-提示词.md` 第三节：给模型看的机械文字用英文）。这几句只在出错时出现，不常驻，不进登记簿。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/spec/tests.rs` | 样本读进来、写回去逐字节一样；两格都能不写；多格（包括原来的 `read`、`readonly`、`network`）、类型不对、不是 JSON 的读不了 |
| `crates/miyu-sandbox/src/wrap/tests.rs` | 包出来的命令：助手、`run`、`--spec` 和 JSON、`--`、程序和参数，照先后；命令没有参数的，最后一个是程序 |
| `crates/miyu-sandbox/src/probe/tests.rs` | 平台的名字和 JSON 里的一样；这次编的是哪个平台；说法是版本 1、一行；手段怎么连；探不成的每一种怎么说 |
| `crates/miyu-sandbox/tests/run.rs` | 真跑助手（规格什么都不限）：命令的输出、退出码、工作目录、环境变量和直接跑一样，它自己什么都不多印；Unix 上是同一个进程，被信号杀掉的照样是信号；参数不对、规格写坏了、没给命令、找不到命令、执行不了的退出码和那一句；`probe` 是一行、版本 1、这台机器的平台（手段各平台自己测） |
| `crates/miyu-sandbox/tests/probe.rs` | 真的助手说的是这台机器；不是程序的起不来；假的助手（Unix 上的脚本）：多出来的格不管，到时杀掉，关了输出不退出的、拿着管道不放的也不等，退出码不是 0、说的读不懂、版本不认得各是各的原因 |
| `crates/miyu-sandbox/tests/locate.rs` | 主程序旁边有的找得到，没有的、是目录的找不到；Windows 上名字带 `.exe` |
| `crates/miyu-basesystem/src/shell/program/tests.rs` | 带了规格的四种 shell 都经助手起：参数照先后，工作目录、环境变量照旧；规格写不成 JSON 的起不来 |
| `crates/miyu-basesystem/tests/shell_sandbox.rs` | 带了规格的真跑一次（Unix）：假助手 `/bin/echo` 收到的是 `run --spec <规格> -- <shell> <参数…>`；规格写不成 JSON 的说起不来，命令没跑；不带的照旧，见 `tests/shell.rs` |
| `crates/miyu/tests/core.rs` | 起来时探一次：旁边有助手的记 `sandbox` 那一行，平台是这台机器的，有手段那一格；没有的记找不到 |
| `crates/miyu-core/src/sandbox/tests.rs` | 旁边没有助手的、不知道主程序在哪的记 `helper not found`；助手跑不了的记原因；探到了手段的交回能用和助手，手段是空的、找不到、探不成的交回用不了和对应的原因（施工 5-4 上、下） |
| `crates/miyu-session/tests/sandbox.rs` | 执行器写的规格；Unix 上有收紧手段的，真的经助手跑 `shell`（`session/tools.md`，施工 5-4 上） |

### 出处

- `11-权限与沙盒.md` 第六节：各平台的实现、A7（沙盒助手单独一个小程序）、A3、A8。
- `01-架构.md` 第九节：第 3 层「执行器」放沙盒。

### 还没有的

- Windows 上以沙盒用户的身份起命令、受限令牌（5-9；沙盒用户由 `miyu sandbox setup` 装，5-8，`sandbox/windows.md`）。
- Windows 上主程序的真实位置（`std::fs::canonicalize`）带 `\\?\` 的前缀，日志里助手的路径跟着带：5-9 起看要不要去掉。
