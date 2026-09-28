## 沙盒：`miyu-sandbox`

### 是什么

沙盒的底子：
- 核心给每条要关起来的命令写一份规格：哪些能读，哪些能写，能写的里面哪些只能读，哪些藏起来，网络怎么走；
- 小程序 `miyu-sandbox` 照规格先把自己收紧，再换成那条命令。

它是单独的一个小程序，Ubuntu、Mint 上只给它开命名空间（`11-权限与沙盒.md` 第六节）。

现在（施工 5-1）规格、助手、探测都接通了，还不收紧任何东西；5-2 起各平台一件件加上。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/lib.rs` | 交出去的几样 |
| `crates/miyu-sandbox/src/spec.rs` | 规格 |
| `crates/miyu-sandbox/src/wrap.rs` | 照规格包一条命令：`miyu-sandbox run --spec … -- …` |
| `crates/miyu-sandbox/src/locate.rs` | 找助手：主程序旁边 |
| `crates/miyu-sandbox/src/probe.rs` | 探测：跑 `miyu-sandbox probe`，读它说的 |
| `crates/miyu-sandbox/src/main.rs` | 助手本身：`run`、`probe` |
| `crates/miyu-tool/src/run.rs` | 一次调用带的 `sandbox` |
| `crates/miyu-basesystem/src/shell.rs` | 带了规格的经助手起命令 |
| `crates/miyu-core/src/lib.rs` | 起来时探一次，记日志 |

### 对外的样子

**规格** `Spec`，助手的 `--spec` 收的就是它的 JSON。路径都是真实的位置：

| 格 | 是什么 |
|---|---|
| `read` | 能读的目录、文件 |
| `write` | 能读能写的 |
| `readonly` | 能写的那几片里只能读的，例如工作区的 `.git/hooks`、`.git/config` |
| `hidden` | 读写都不行、要藏起来的，例如数据根：它可能落在能写的临时目录里 |
| `network` | `"off"`：不能联网；`{"proxy": "127.0.0.1:<端口>"}`：只能连 Miyu 的代理 |

例子：

```json
{"read":["/usr","/etc"],"write":["/home/me/project","/tmp"],"readonly":["/home/me/project/.git/hooks"],"hidden":["/home/me/.miyu"],"network":"off"}
```

**助手的命令行**：

- `miyu-sandbox run --spec <规格的 JSON> -- <程序> [参数…]`：照规格收紧自己，再换成这条命令。
  - Unix 上直接换成它（`exec`），进程还是同一个。
  - Windows 上起一个子进程，等它，照它的退出码退出。
  - 成了什么都不印：它的标准错误就是命令的标准错误，印了会混进给她看的输出。
- `miyu-sandbox probe`：标准输出上一行 JSON，说这台机器能收紧到什么程度，例如 `{"version":1,"platform":"linux","mechanisms":[]}`。`platform` 是 `linux`、`macos`、`windows`、`other` 之一；`mechanisms` 现在是空的，5-2 起各平台往里加。

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
   - 找到了就跑 `miyu-sandbox probe`，最多等 5 秒。
   - 成了：记一行 `INFO` `sandbox`，字段 `helper`（路径，家目录写成 `~`）、`platform`、`mechanisms`（逗号连起来，空的写 `none`）。
   - 没找到、跑不了、超时、说的读不懂：记一行 `WARN` `sandbox unavailable`，字段 `reason`。
   - 这一步只记日志，不影响别的。
3. **`shell` 带了规格的**：命令写成 `<助手> run --spec <规格的 JSON> -- <shell> <shell 的参数…>`。工作目录、环境变量白名单、标准输入输出、进程组、超时整组杀都和直接起一样：Unix 上助手换成了 shell，是同一个进程。
4. **助手的 `run`**：
   1. 读参数：不是 `run --spec <JSON> -- <程序> …` 的样子，印 `miyu-sandbox: usage: miyu-sandbox run --spec <json> -- <program> [args...]`，退出 125。
   2. 读规格：读不懂的，印 `miyu-sandbox: bad spec: <原话>`，退出 125。
   3. 收紧：现在什么都不做。
   4. 换成命令。Unix 上 `exec`，找不到的印 `miyu-sandbox: cannot run <程序>: <原话>`、退出 127，别的原因执行不了的一样印、退出 126。Windows 上起子进程：起不来的照这两条；起来了就等它，照它的退出码退出。
5. **助手的 `probe`**：印那一行 JSON，退出 0。
6. 助手的字一律英文：它印在命令的输出里，她看得到（`26-提示词.md` 第三节：给模型看的机械文字用英文）。这几句只在出错时出现，不常驻，不进登记簿。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/spec/tests.rs` | 规格写成 JSON、读回来一样；网络的两种写法；缺格、多格的读法 |
| `crates/miyu-sandbox/src/wrap/tests.rs` | 包出来的命令：助手、`run`、`--spec` 和 JSON、`--`、程序和参数，照先后 |
| `crates/miyu-sandbox/tests/run.rs` | 真跑助手：命令的输出、退出码、工作目录、环境变量和直接跑一样；参数不对、规格写坏了、没给命令、找不到命令的退出码和那一句；成了的什么都不多印；`probe` 的平台 |
| `crates/miyu-sandbox/tests/locate.rs` | 主程序旁边有的找得到，没有的、是目录的找不到 |
| `crates/miyu-basesystem/tests/shell.rs` | 带了规格的经助手起（一个假助手，看它收到的参数）；不带的照旧 |
| `crates/miyu/tests/core.rs` | 起来时的日志里有 `sandbox` 那一行 |

### 出处

- `11-权限与沙盒.md` 第六节：各平台的实现、A7（沙盒助手单独一个小程序）、A3、A8。
- `01-架构.md` 第九节：第 3 层「执行器」放沙盒。

### 还没有的

- Linux：Landlock 管文件（5-2），挂载命名空间护住 `.git` 和数据根、AppArmor 配置（5-3），网络命名空间和 seccomp（5-6）。
- 权限策略给调用带规格、沙盒用不了时改成问人、`miyu ask` 开头说一句（5-4）。
- 代理（5-5）。
- macOS 的 Seatbelt（5-7）；Windows 的沙盒用户、受限令牌（5-8、5-9）。
