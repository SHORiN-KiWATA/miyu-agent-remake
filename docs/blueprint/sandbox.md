## 沙盒：`miyu-sandbox`

### 是什么

沙盒的底子：
- 核心给每条要关起来的命令写一份规格：哪些能读，哪些能写，能写的里面哪些只能读，哪些藏起来；
- 小程序 `miyu-sandbox` 照规格先把自己收紧，再换成那条命令。

它是单独的一个小程序，Ubuntu、Mint 上只给它开命名空间（`11-权限与沙盒.md` 第六节）。

现在（施工 5-1）规格、助手、探测都接通了，还不收紧任何东西；5-2 起各平台一件件加上。

这一页管各平台共用的：规格、助手的命令行和退出码、探测、找助手、`shell` 怎么经助手起。各平台怎么收紧各有一页，随那一步的施工写：Linux `sandbox/linux.md`（5-2 起），macOS `sandbox/macos.md`（5-7），Windows `sandbox/windows.md`（5-8 起）。沙盒只管读写权限，不管网络（2026-09-29 项目主人定）。助手里收紧的代码也是各平台一个文件，几条线可以同时施工，各改各的（2026-09-28 项目主人同意分线并行）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/lib.rs` | 交出去的几样 |
| `crates/miyu-sandbox/src/spec.rs` | 规格 |
| `crates/miyu-sandbox/src/wrap.rs` | 照规格包一条命令：`miyu-sandbox run --spec … -- …` |
| `crates/miyu-sandbox/src/locate.rs` | 找助手：主程序旁边 |
| `crates/miyu-sandbox/src/probe.rs` | 探测：跑 `miyu-sandbox probe`，读它说的 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/main.rs` | 助手里各平台共用的：读参数、读规格、`probe`、出错时说的几句 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux.rs`、`macos.rs`、`windows.rs`、`other.rs` | 各平台收紧、再换成命令（`run`），探测时报的手段（`mechanisms`）；`other.rs` 是别的 Unix |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/unix.rs` | Unix 上换成命令（`exec`） |
| `crates/miyu-tool/src/run.rs` | 一次调用带的 `sandbox` |
| `crates/miyu-basesystem/src/shell.rs` | 带了规格的经助手起命令 |
| `crates/miyu-core/src/sandbox.rs` | 起来时探一次，记日志 |

### 对外的样子

**规格** `Spec`，助手的 `--spec` 收的就是它的 JSON。路径都是真实的位置：

| 格 | 是什么 |
|---|---|
| `read` | 能读的目录、文件 |
| `write` | 能读能写的 |
| `readonly` | 能写的那几片里只能读的，例如工作区的 `.git/hooks`、`.git/config` |
| `hidden` | 读写都不行、要藏起来的，例如数据根：它可能落在能写的临时目录里 |

改成（施工 5-1（补）合进来时标成样本 `docs/designs/samples/sandbox/spec.json`）（例子）：

```json
{"read":["/usr","/etc"],"write":["/home/me/project","/tmp"],"readonly":["/home/me/project/.git/hooks"],"hidden":["/home/me/.miyu"]}
```

读的时候，认不得的格、类型不对的，都当规格写坏了：助手不懂的限制，不能悄悄跳过。四格不写都是空的。规格里没有网络：沙盒只管读写权限（2026-09-29 项目主人定）。

**助手的命令行**：

- `miyu-sandbox run --spec <规格的 JSON> -- <程序> [参数…]`：照规格收紧自己，再换成这条命令。
  - Unix 上直接换成它（`exec`），进程还是同一个。
  - Windows 上起一个子进程，等它，照它的退出码退出。
  - 成了什么都不印：它的标准错误就是命令的标准错误，印了会混进给她看的输出。
- `miyu-sandbox probe`：标准输出上一行 JSON，说这台机器能收紧到什么程度，例如 `{"version":1,"platform":"linux","mechanisms":[]}`。`platform` 是 `linux`、`macos`、`windows`、`other` 之一；`mechanisms` 现在是空的，5-2 起各平台往里加。核心只认 `version` 是 1 的；多出来的格不管。

**退出码**，照 `env`、`timeout` 的约定：

| 码 | 什么时候 |
|---|---|
| 125 | 助手自己出了错：参数不对、规格写坏了、`--` 后面没有程序、收紧失败 |
| 126 | 找到了命令，执行不了 |
| 127 | 找不到命令 |
| 别的 | 命令自己的 |

**一次调用带的** `Call.sandbox`（`tools/interface.md`）：`Some(Sandboxed { helper, spec })` 的，`shell` 经助手起命令；`None` 的照旧直接起。现在核心还不带，5-4 接上权限策略。

### 怎么走

1. **找助手**（`locate`）：主程序真实位置旁边的 `miyu-sandbox`，Windows 上是 `miyu-sandbox.exe`。不是普通文件的、没有的，是空的。
2. **核心起来时探一次**：
   - 主程序的真实位置照环境的快照（`core.md` 第 1 步）。
   - 找到了就跑 `miyu-sandbox probe`，最多等 5 秒：读它的输出、等它退出加起来不过 5 秒，到时杀掉它。读输出在另一个线程里，它放出去的东西拿着管道不放，也不一直等。
   - 成了：记一行 `INFO` `sandbox`，字段 `helper`（路径，家目录写成 `~`）、`platform`、`mechanisms`（逗号连起来，空的写 `none`）。
   - 没找到、跑不了、超时、说的读不懂：记一行 `WARN` `sandbox unavailable`，字段 `reason`，是这几句之一：`helper not found`、`cannot run helper: <原话>`、`helper timed out`、`helper failed: <退出码>`、`helper output not understood: <原话>`（版本不是 1 的写 `version <几>`）。
   - 这一步只记日志，不影响别的。
3. **`shell` 带了规格的**：命令写成 `<助手> run --spec <规格的 JSON> -- <shell> <shell 的参数…>`。工作目录、环境变量白名单、标准输入输出、进程组、超时整组杀都和直接起一样：Unix 上助手换成了 shell，是同一个进程。规格写不成 JSON 的（里面有不是 UTF-8 的路径）：照「起不来」说，不会不经沙盒就跑。
4. **助手的 `run`**：
   1. 读参数：不是 `run --spec <JSON> -- <程序> …` 的样子，印 `miyu-sandbox: usage: miyu-sandbox run --spec <json> -- <program> [args...]`，退出 125。
   2. 读规格：读不懂的，印 `miyu-sandbox: bad spec: <原话>`，退出 125。
   3. 收紧：交给这个平台的文件（`linux.rs`、`macos.rs`、`windows.rs`，别的 Unix 是 `other.rs`），施工 5-1 都还什么都不做。收紧不成的，一律印 `miyu-sandbox: cannot confine: <原话>`，退出 125，不跑命令。
   4. 换成命令。Unix 上 `exec`，找不到的印 `miyu-sandbox: cannot run <程序>: <原话>`、退出 127，别的原因执行不了的一样印、退出 126。Windows 上起子进程：起不来的照这两条；起来了就等它，照它的退出码退出。
5. **助手的 `probe`**：印那一行 JSON，退出 0。手段（`mechanisms`）由这个平台的文件报，各平台自己定写什么，写进它那一页。
6. 助手的字一律英文：它印在命令的输出里，她看得到（`26-提示词.md` 第三节：给模型看的机械文字用英文）。这几句只在出错时出现，不常驻，不进登记簿。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/spec/tests.rs` | 样本读进来、写回去逐字节一样；四格都能不写；多格（包括原来的 `network`）、类型不对、不是 JSON 的读不了 |
| `crates/miyu-sandbox/src/wrap/tests.rs` | 包出来的命令：助手、`run`、`--spec` 和 JSON、`--`、程序和参数，照先后；命令没有参数的，最后一个是程序 |
| `crates/miyu-sandbox/src/probe/tests.rs` | 平台的名字和 JSON 里的一样；这次编的是哪个平台；说法是版本 1、一行；手段怎么连；探不成的每一种怎么说 |
| `crates/miyu-sandbox/tests/run.rs` | 真跑助手（规格什么都不限）：命令的输出、退出码、工作目录、环境变量和直接跑一样，它自己什么都不多印；Unix 上是同一个进程，被信号杀掉的照样是信号；参数不对、规格写坏了、没给命令、找不到命令、执行不了的退出码和那一句；`probe` 是一行、版本 1、这台机器的平台（手段各平台自己测） |
| `crates/miyu-sandbox/tests/probe.rs` | 真的助手说的是这台机器；不是程序的起不来；假的助手（Unix 上的脚本）：多出来的格不管，到时杀掉，关了输出不退出的、拿着管道不放的也不等，退出码不是 0、说的读不懂、版本不认得各是各的原因 |
| `crates/miyu-sandbox/tests/locate.rs` | 主程序旁边有的找得到，没有的、是目录的找不到；Windows 上名字带 `.exe` |
| `crates/miyu-basesystem/src/shell/program/tests.rs` | 带了规格的四种 shell 都经助手起：参数照先后，工作目录、环境变量照旧；规格写不成 JSON 的起不来 |
| `crates/miyu-basesystem/tests/shell_sandbox.rs` | 带了规格的真跑一次（Unix）：假助手 `/bin/echo` 收到的是 `run --spec <规格> -- <shell> <参数…>`；规格写不成 JSON 的说起不来，命令没跑；不带的照旧，见 `tests/shell.rs` |
| `crates/miyu/tests/core.rs` | 起来时探一次：旁边有助手的记 `sandbox` 那一行，平台是这台机器的，有手段那一格；没有的记找不到 |
| `crates/miyu-core/src/sandbox/tests.rs` | 旁边没有助手的、不知道主程序在哪的记 `helper not found`；助手跑不了的记原因 |

### 出处

- `11-权限与沙盒.md` 第六节：各平台的实现、A7（沙盒助手单独一个小程序）、A3、A8。
- `01-架构.md` 第九节：第 3 层「执行器」放沙盒。

### 还没有的

- Linux：挂载命名空间护住 `.git` 和数据根、AppArmor 配置（5-3）；文件的 Landlock 见 `sandbox/linux.md`（5-2）。
- 权限策略给调用带规格、沙盒用不了时改成问人、`miyu ask` 开头说一句（5-4）。
- macOS 的 Seatbelt（5-7）；Windows 的沙盒用户、受限令牌（5-8、5-9）。
- Windows 上主程序的真实位置（`std::fs::canonicalize`）带 `\\?\` 的前缀，日志里助手的路径跟着带：5-8 起看要不要去掉。
