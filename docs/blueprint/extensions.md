## 扩展进程

### 是什么

核心拉起 `kind = "process"` 的软件包（第一个是通讯平台的桥），经标准输入输出说同一套协议（`05-内核接口.md` 第四节、I2）。开关存在核心里：开着的常驻，核心重启时照开关拉起；崩了退避重启，连续失败停下、说明原因；随核心退出（第九节）。

状态：图纸，施工 9-4（上）（2026-10-08 主会话；方向是施工方案第三节 9-4 那一行和 `05-内核接口.md` 第四、九节；桥那一头的形状和通讯平台的会话对过）。能力审批、把包自己的配置交给它随 9-4（下），等项目主人拍板。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/extensions.rs` | 开关：`system/extensions.json`，照只给自己看的 JSON 小文件的规矩读写 |
| `crates/miyu-endpoint/src/extensions.rs` | 一张表：每个 `process` 包一个看管的任务；开、关、重启、列状态；核心空闲、退出时问它 |
| `crates/miyu-endpoint/src/extensions/supervise.rs` | 看管一个包：拉起、交给连接、等它退出、退避、停下 |
| `crates/miyu-endpoint/src/extensions/stderr.rs` | 标准错误的文件：拉起前太大的挪成 `.old`，状态里给最后几行 |
| `crates/miyu-endpoint/src/extensions/methods.rs` | `extension.enable`、`extension.disable`、`extension.restart`、`extension.status` |
| `crates/miyu-endpoint/src/connection.rs`、`hello.rs`、`login.rs` | 核心亲手给的连接：握手不看凭据（`Via::Spawned`） |
| `crates/miyu-core/src/lib.rs`、`serve.rs` | 写了 `ready` 以后照开关拉起，停的时候请扩展退出 |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展：照参数一步步做（握手、调方法、等标准输入读到头、不理它、退出码、往标准错误写），不随发行带出去 |

### 对外的样子

**开关的文件** `system/extensions.json`：

```json
{"on":{"onebot":true}}
```

没写的照清单的 `start`：`always` 的开着，`manual` 的关着。写了的照写的。读不成的当没有，记一行 `WARN`，照样起来；写的时候盖掉。

**协议**（只有本机连接能调，现在都是管理员）：

| 方法 | 参数 | 回应 |
|---|---|---|
| `extension.status` | 无 | `{"extensions": [<一个>…]}`，照编号排，只列读成了的 `process` 包 |
| `extension.enable` | `{"package"}` | `<一个>`：开着、记下来、没在跑的拉起 |
| `extension.disable` | `{"package"}` | `<一个>`：关了、记下来、在跑的请它退出 |
| `extension.restart` | `{"package"}` | `<一个>`：请它退出、重新拉起，连续失败从零数；关着的拒绝 `extension_off` |

一个：

| 格 | 有没有 | 是什么 |
|---|---|---|
| `package`、`name` | 都有 | 编号；名字照连接的语言挑（同 `package.list`） |
| `start` | 都有 | `manual`、`always` |
| `on` | 都有 | 开关 |
| `state` | 都有 | `off`（关着）、`starting`（拉起了、还没握手）、`running`（握了手）、`waiting`（退避中，等着下一次）、`stopped`（不再拉起，看 `reason`） |
| `pid` | `running`，起来了的 `starting` | 进程号 |
| `failures` | 都有 | 连续失败了几次 |
| `retry_in` | `waiting` | 还有几毫秒拉起 |
| `reason` | `stopped` | `config_error`（退出码 1）、`failed_repeatedly`（连续失败 5 次）、`not_installed`（程序不在 `miyu` 旁边）、`cannot_start`（起不来：系统不让起、建不了它的目录和标准错误的文件）、`protocol_mismatch`（清单说的协议版本不包含核心的，不拉起） |
| `stderr` | `stopped` | 标准错误的最后 20 行，最多 4 KiB |

拒绝：编号不是读成了的包 `unknown_package`；不是 `process` 包 `not_an_extension`；参数不对 `bad_params`。

### 怎么走

1. **拉起**：程序照 9-2 的找法（`miyu_store::packages::locate`，只找 `miyu` 真实位置旁边的，不走 `PATH`），参数是清单的 `[process] args`，环境照核心的，工作目录是这个包放状态的目录（`state/packages/<编号>/`，没有的建）。标准输入、输出接管道，交给协议的连接；标准错误直接接到 `state/logs/<编号>.stderr` 上（追加，拉起前超过 1 MiB 的先挪成 `<编号>.stderr.old`）。核心丢下它时杀掉（`kill_on_drop`），兜底。Windows 上不开新的控制台窗口。
2. **握手**：照头的样子（`hello`），不出示凭据：标准输入输出是核心亲手给的（`Via::Spawned`，写了的凭据不看）。握手的期限照头的。身份先是管理员本人，系统账号随 O-4；能力的审批、强制执行随 9-4（下）和之后。
3. **开关**：核心写了 `ready` 以后读一次开关的文件，照开关拉起开着的（`Core::start_extensions`）。`enable`、`disable` 先写文件再动进程，写不进的拒绝、不动进程（`internal_error`）；开、关、重启一件件办，两个连接同时开同一个不拉起两个。核心空闲的判断多一条：有要拉起、在跑、在等着再拉起的扩展，不算空闲；停下了的不拦着。
4. **退出**：核心停的时候、`disable`、`restart` 时关它的标准输入（读到头就是「请退出」，三个平台一样），等 5 秒，没退的杀掉。核心停的时候各个扩展一起等，不一个个排队。
5. **崩了**：它自己退出（退出码 1 以外的，包括 0）、被信号杀掉、标准输出关了、握手不成，都算一次失败：退避 1、2、4、8、16 秒以后再拉起（最多 60 秒），连续 5 次停下（`failed_repeatedly`）。退出码 1 是「配置错、端口被占」这类重启也没用的：不退避，直接停下（`config_error`）。握了手、连着跑满 60 秒的，下一次失败从 1 数。进程活着、连接连着就算活，不另做健康检查。停下的照旧开着：`restart` 再试，核心下次起来也再试。
6. **日志**：拉起、握了手、退出（退出码或信号）、退避、停下，各记一行运行日志，带包的编号。

### 出错

| 代码 | 什么时候 |
|---|---|
| `unknown_package` | 没有这个编号的包，或者它的清单读不成 |
| `not_an_extension` | 是 `ui` 包 |
| `extension_off` | `restart` 一个关着的 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/src/extensions/tests.rs` | 没有文件的是空的、写进去读得回来一字不差；坏了的、版本不认识的说是坏了；这一瞬间有人改了的不写 |
| `crates/miyu-endpoint/src/extensions/supervise/tests.rs` | 退避 1、2、4、8 秒、最多 60 秒、不溢出；连续 5 次停下；退出码 1 直接停下，0、别的、被杀掉的退避；跑满了的这一次从 1 数 |
| `crates/miyu-endpoint/src/extensions/stderr/tests.rs` | 追加；太大的挪成 `.old`、盖掉上一份；最后 20 行、最多 4 KiB、截了半截的那一行不要 |
| `crates/miyu-endpoint/tests/extensions.rs` | 真核心、真进程（测试用的小程序 `miyu-test-extension`，`crates/miyu-endpoint/src/bin/`，三个平台一样，拷在测试程序旁边）：`manual` 的开了才拉起、工作目录、握手不出示凭据、扩展调 `extension.*` 被拒、标准错误进日志、开关的文件；`always` 的不用开、新核心照开关拉起、随核心退出；有开着的核心不空闲；退出码 1 停下带标准错误的最后几行；别的退出连续 5 次停下；不握手也算失败；关了标准输入不退的到时杀掉；`restart` 换一个新进程、关着的拒绝；程序没找到、协议版本对不上、界面包、没有的包、参数不对 |

### 起草时定的

- 开关放 `system/`、不放 `state/`：`state/` 是派生的、能删掉重建，开关是人做的决定（2026-10-08 主会话）。
- 退出码 0 也算失败：扩展该常驻，自己退了就是没在干活（2026-10-08 主会话，照施工方案「退出码 1 以外的退避重启」）。
- 「连续」照跑满 60 秒断开：握了手就从零数的话，一握手就崩的扩展会一直重启下去（2026-10-08 主会话）。
- 标准错误直接接文件、不经核心转：核心不读它、不拷它，不怕它写得多（2026-10-08 主会话）。
- 测试用的扩展是端点 crate 里的一个小程序（`miyu-test-extension`），不是 `sh` 脚本：包的程序在 Windows 上找的是 `.exe`，脚本只管得了 Unix（2026-10-08 主会话）。
- 协议版本对不上的不拉起：说不通的话起了也白起（2026-10-08 主会话）。

### 还没有的

- 能力的审批（清单 `capabilities`）、握手时把包自己的配置交给它：9-4（下），等项目主人拍板。
- 扩展的身份是系统账号：O-4。能力的强制执行、沙盒里跑：随多用户、沙盒那一段。
- 扩展提供工具、命令、挂接点：O-2。
- 状态变了推给头（`extension.changed`）：头的设置页要的时候加。
