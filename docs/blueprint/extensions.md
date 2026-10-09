## 扩展进程

### 是什么

核心拉起 `kind = "process"` 的软件包（第一个是通讯平台的桥），经标准输入输出说同一套协议（`05-内核接口.md` 第四节、I2）。开关存在核心里：开着的常驻，核心重启时照开关拉起；崩了退避重启，连续失败停下、说明原因；随核心退出（第九节）。

状态：图纸，施工 9-4（上）（2026-10-08 主会话；方向是施工方案第三节 9-4 那一行和 `05-内核接口.md` 第四、九节；桥那一头的形状和通讯平台的会话对过）。能力的声明和审批施工 9-4（下上）做了（2026-10-08，项目主人照推荐定：在开的那一下批）；把包自己的配置交给它随 9-4（下下）。状态的推送随 9-4（补）（2026-10-08 主会话，两个头的设置页要的）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/extensions.rs` | 开关：`system/extensions.json`，照只给自己看的 JSON 小文件的规矩读写 |
| `crates/miyu-endpoint/src/extensions.rs` | 一张表：每个 `process` 包一个看管的任务；开、关、重启、列状态；核心空闲、退出时问它 |
| `crates/miyu-endpoint/src/extensions/supervise.rs` | 看管一个包：拉起、交给连接、等它退出、退避、停下 |
| `crates/miyu-endpoint/src/extensions/stderr.rs` | 标准错误的文件：拉起前太大的挪成 `.old`，状态里给最后几行 |
| `crates/miyu-endpoint/src/extensions/methods.rs` | `extension.enable`、`extension.disable`、`extension.restart`、`extension.status`；推送用的一项（`entry`，施工 9-4 补） |
| `crates/miyu-endpoint/src/subscriptions/extensions.rs` | 扩展的状态的订阅（施工 9-4 补）：先写回应，收到哪个包变了照这一刻算那一项推 `extension.changed`，掉了队推 `resync` |
| `crates/miyu-endpoint/src/connection.rs`、`hello.rs`、`login.rs` | 核心亲手给的连接：握手不看凭据（`Via::Spawned`） |
| `crates/miyu-core/src/lib.rs`、`serve.rs` | 写了 `ready` 以后照开关拉起，停的时候请扩展退出 |
| `crates/miyu-endpoint/src/bin/miyu-test-extension.rs` | 测试用的扩展：照参数一步步做（握手、调方法、带参数调方法（参数里的 `{session}` 换成最近回应里的会话编号，施工 O-4 下）、等标准输入读到头、不理它、退出码、往标准错误写、记下一个环境变量（施工 O-18）），不随发行带出去 |

### 对外的样子

**开关的文件** `system/extensions.json`：

```json
{"version":1,"on":{"echo":true},"approved":{"echo":["events.read","network"]}}
```

没写的照清单的 `start`：`always` 的开着，`manual` 的关着。写了的照写的。`approved` 是批过的能力（施工 9-4 下上）：批的时候这个包声明的全部，照能力表的先后；一个都没批过的不写这一格，以前的文件读进来当一个都没批过。读不成的当没有，记一行 `WARN`，照样起来；写的时候盖掉。

**协议**（只有本机连接能调，现在都是管理员）：

| 方法 | 参数 | 回应 |
|---|---|---|
| `extension.status` | 无 | `{"extensions": [<一个>…]}`，照编号排，只列读成了的 `process` 包 |
| `extension.enable` | `{"package", "approve"?}` | `<一个>`：要的能力还有没批的，`approve`（能力名的列表）要盖住它们，批了记下（见「能力」）；开着、记下来、没在跑的拉起 |
| `extension.disable` | `{"package"}` | `<一个>`：关了、记下来、在跑的请它退出 |
| `extension.restart` | `{"package"}` | `<一个>`：请它退出、重新拉起，连续失败从零数；关着的拒绝 `extension_off`，还有没批的能力拒绝 `needs_approval` |

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
| `reason` | `stopped` | `needs_approval`（开着，要的能力还有没批的，施工 9-4 下上）、`config_error`（退出码 1）、`failed_repeatedly`（连续失败 5 次）、`not_installed`（程序不在 `miyu` 旁边）、`cannot_start`（起不来：系统不让起、建不了它的目录和标准错误的文件）、`protocol_mismatch`（清单说的协议版本不包含核心的，不拉起） |
| `stderr` | `stopped` | 标准错误的最后 20 行，最多 4 KiB |
| `capabilities` | 声明了能力的 | 声明的每一个 `{"id", "name", "summary"}`，照能力表的先后；名字、一句说明照连接的语言挑（内核那份 `human` 的 `extensions/capabilities/<名字>`、`…/summary`），没有字的名字是编号、说明是 `null`（施工 9-4 下上） |
| `unapproved` | 还有没批的 | 没批的能力名，照能力表的先后（施工 9-4 下上）。出厂的包没有 |

拒绝：编号不是读成了的包 `unknown_package`；不是 `process` 包 `not_an_extension`；参数不对 `bad_params`（`approve` 里有不是这个包声明的名字的也是）；要的能力还有没批的 `needs_approval`，`data.capabilities` 是没盖住的那几个（施工 9-4 下上）。

**能力**（施工 9-4 下上，`05-内核接口.md` 第三节、I5；项目主人 2026-10-08 照推荐定）：

1. 清单 `[process] capabilities` 声明要哪些（`packages.md`），只认那张表里的十二个名字：`tools`、`commands`、`context.inject`、`tool.guard`、`tool.rewrite`、`events.read`、`events.write`、`sessions.drive`、`act_for_external`、`network`、`fs.read`、`fs.write`。
2. 管理员自己装的（`home/<管理员>/packages/`）要批：在 `extension.enable` 那一下带上 `approve`。批的时候记下它这时声明的全部；还没批的是「声明的减批过的」，升级以后多了的只问多的；关掉不清批准。
3. 出厂的（资源目录里随 Miyu 一起装的，通讯平台的桥）算批过的：和编译进核心的模块一样本来就被信任（05 第三节「内置模块」）。
4. 开着、还有没批的（开关文件里开着的，`start = "always"` 没写开关的）不拉起：`stopped`、`needs_approval`，不拦核心空闲退出；批了就拉起。
5. 照批过的能力挡方法、沙盒里跑，随多用户、沙盒那一段。

**推送**（施工 9-4 补，两个头的设置页要的，照会话列表的推送那一路，`protocol.md`「会话列表的推送」）：

1. `subscribe {"stream": "extensions"}`：回应 `{"extensions": [<一个>…]}`，和 `extension.status` 一样。不带 `session`、`after`，带了 `bad_params`。一个连接至多一个，再订阅换一个新的；`unsubscribe` 停掉，没订阅过的也回 `{}`。
2. 之后一个包的 `state`、`on`、`failures`、`reason` 变了推 `extension.changed`：`{"entry": <一个>}`，头照 `package` 整项替换。推的那一项照这一刻的状态、这个连接这一刻的语言算，晚到的、重复的都对，不编号。
3. 读得太慢、掉了队：推 `resync`（`{"stream": "extensions"}`），这个订阅停了，头重新订阅。

**配置**（施工 9-4 下下；项目主人 2026-10-08 照推荐定，形状和通讯平台的会话对过）：核心拉起的扩展不自己读系统配置、密钥文件，要的值由核心交给它。只在核心亲手拉起、走标准输入输出的连接上，只给这个包自己的键：`config.md` 第九条「密钥不进协议的回应」的例外。

1. **哪些键**：「包的编号加点」开头的配置项（核心替它声明的、它清单里 `[settings]` 声明的都算；带 `<…>` 的模板键不算）。
2. **握手**：`hello` 的回应多 `config`：`{"onebot.listen": 8301, "onebot.token": "<真值>", …}`。值是最终值（没写的照默认值，系统配置、个人设置合出来的，不看项目配置），密钥引用（`{ secret }`、`{ env }`）解成真值的字；没设、没默认值、引用取不到的那一键不放。头的连接没有这一格。
3. **变了推** `extension.config`：`{"keys": {"onebot.token": "<新真值>"}}`。握手的回应写出去以后，核心盯着配置服务交给核心的那一份（配置文件改了、`secret.set` 换了值、手改了密钥文件，都会换上新的一份），把这个包自己的那份重算一遍，跟上一次交给它的比，变了推，只放变了的键；变成没设、取不到的写 `null`。不用订阅。不叫 `config.changed`：那是头订阅的推送，形状带 `origin`、`applies`（通讯平台的会话提的，主会话定）。
4. **不留痕**：值不进运行日志、不进事件；运行日志只记 `DEBUG extension config handed package=… keys=<几个>`、`extension config pushed`，不带值。

### 怎么走

1. **拉起**：程序照 9-2 的找法（`miyu_store::packages::locate`，只找 `miyu` 真实位置旁边的，不走 `PATH`），参数是清单的 `[process] args`，环境照核心的，另把 `MIYU_HOME`、`MIYU_RESOURCES` 设成核心手上正在用的数据根、资源目录的路径（施工 O-18：扩展和核心认同一份，不管核心是怎么找到它们的；测试里的核心跑在测试程序里，环境里没有临时的数据根，靠它把扩展指过去），工作目录是这个包放状态的目录（`state/packages/<编号>/`，没有的建）。标准输入、输出接管道，交给协议的连接；标准错误直接接到 `state/logs/<编号>.stderr` 上（追加，拉起前超过 1 MiB 的先挪成 `<编号>.stderr.old`）。核心丢下它时杀掉（`kill_on_drop`），兜底。Windows 上不开新的控制台窗口。
2. **握手**：照头的样子（`hello`），不出示凭据：标准输入输出是核心亲手给的（`Via::Spawned`，写了的凭据不看）。握手的期限照头的。包声明了系统账号的（`packages.md`「系统账号」），这个连接就是它：握手回应的 `account` 写它，通讯平台的场所会话归它（`venues.md`）；没声明的是管理员本人（施工 O-4 下）。人格、预设、软件包、上传的 blob 照旧照管理员的。能力批过的才拉起（「能力」）；照能力挡方法随多用户、沙盒那一段。
3. **开关**：核心写了 `ready` 以后读一次开关的文件，照开关拉起开着的（`Core::start_extensions`）。`enable`、`disable` 先写文件再动进程，写不进的拒绝、不动进程（`internal_error`）；开、关、重启一件件办，两个连接同时开同一个不拉起两个。核心空闲的判断多一条：有要拉起、在跑、在等着再拉起的扩展，不算空闲；停下了的不拦着。
4. **退出**：核心停的时候、`disable`、`restart` 时关它的标准输入（读到头就是「请退出」，三个平台一样），等 5 秒，没退的杀掉。核心停的时候各个扩展一起等，不一个个排队。
5. **崩了**：它自己退出（退出码 1 以外的，包括 0）、被信号杀掉、标准输出关了、握手不成，都算一次失败：退避 1、2、4、8、16 秒以后再拉起（最多 60 秒），连续 5 次停下（`failed_repeatedly`）。退出码 1 是「配置错、端口被占」这类重启也没用的：不退避，直接停下（`config_error`）。握了手、连着跑满 60 秒的，下一次失败从 1 数。进程活着、连接连着就算活，不另做健康检查。停下的照旧开着：`restart` 再试，核心下次起来也再试。
6. **日志**：拉起、握了手、退出（退出码或信号）、退避、停下，各记一行运行日志，带包的编号。
7. **装卸**（施工 F-5 下，`packages.md`「装卸」）：装、卸、装回来做成了以后，照装卸以前、以后两份清单对一遍（`Core::follow_packages`）。卸掉的照「退出」那一条停下，它经提供者登记的工具拿掉、记下随包卸掉了：用过它的会话照旧留着、调到时报「已卸载」。新装上的照开关拉起，和核心起来时一条路（没批的能力照旧记成停下）。清单变了的算升级：先停下再照开关拉起。清单没变的不动。声明了系统账号的照这时的清单先建好账号（`packages.md`「系统账号」）。

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
| `crates/miyu-endpoint/tests/extensions.rs` | 真核心、真进程（测试用的小程序 `miyu-test-extension`，`crates/miyu-endpoint/src/bin/`，三个平台一样，拷在测试程序旁边）：`manual` 的开了才拉起、工作目录、握手不出示凭据、扩展调 `extension.*` 被拒、标准错误进日志、开关的文件；`always` 的不用开、新核心照开关拉起、随核心退出；有开着的核心不空闲；退出码 1 停下带标准错误的最后几行；别的退出连续 5 次停下；不握手也算失败；关了标准输入不退的到时杀掉；`restart` 换一个新进程、关着的拒绝；程序没找到、协议版本对不上、界面包、没有的包、参数不对；拉起的环境里 `MIYU_HOME`、`MIYU_RESOURCES` 是核心的数据根、资源目录（施工 O-18）。夹具（测试用的扩展、装清单、造核心、等状态）在 `tests/support/extensions.rs` |
| `crates/miyu-endpoint/tests/extension_stream.rs`（施工 9-4 补） | 订阅回整份、名字照语言；开了推到在跑、关了推到关着；带 `after` 和会话的参数不对；取消了不推 |
| `crates/miyu-endpoint/tests/extension_approval.rs`、`crates/miyu-config/src/package/tests.rs`、`crates/miyu-store/src/extensions/tests.rs`（施工 9-4 下上） | 清单的能力照表的先后、不认识的和重复的报 `bad_capability`；开关文件的 `approved` 读写、以前的文件当没批过、没批的不写这一格；管理员装的：不带 `approve` 拒绝 `needs_approval`、只批一部分说剩下的、不是声明的名字 `bad_params`、批了开起来、记下、关了不清；`always` 的没批不拉起、不拦空闲、`restart` 拒绝、批了拉起；升级多了的只问多的；出厂的桥不用批；名字照连接的语言 |
| `crates/miyu-endpoint/tests/extension_config.rs`、`src/extensions/config/tests.rs`（施工 9-4 下下） | 握手交自己的键（默认值、密钥真值、没设的不放），头的连接没有；改了自己的项推、`secret.set` 换了值推新真值、去掉了推 `null`、别的项变了不推；只放变了的键 |
| `crates/miyu-endpoint/tests/packages_extensions.rs`（施工 F-5 下） | 真核心装上一个 `always` 的扩展当场拉起、它登记的工具进工具面、调得到；卸掉当场停下、`extension.status` 不列、用过它的会话照旧留着、调到报「已卸载」；升级了的停下再拉起；装别的包时清单没变的进程不换 |
| `crates/miyu-endpoint/tests/system_account.rs`、`crates/miyu-store/src/packages/tests.rs`、`crates/miyu-store/src/personas/tests.rs`（施工 O-4 下） | 声明了系统账号的包：起来时建它的工作区；起来以后才装上的、装回来的当场建（施工 F-5 下），会话写进它自己的索引；拉起的扩展握手的 `account` 是它；群的场所会话归它（目录、工作目录、回应的 `account`）、找回同一个；对应表里的主人的私聊照旧归主人、这个连接照样能对它说话；群里主人说的记成外部身份带 `account`；附件拷进它名下；用量记在它名下；记忆归管理员、删了照管理员埋墓碑；没声明的扩展照旧 `no_system_account`、不建账号；撞了管理员的 `account_taken` |

### 起草时定的

- 开关放 `system/`、不放 `state/`：`state/` 是派生的、能删掉重建，开关是人做的决定（2026-10-08 主会话）。
- 退出码 0 也算失败：扩展该常驻，自己退了就是没在干活（2026-10-08 主会话，照施工方案「退出码 1 以外的退避重启」）。
- 「连续」照跑满 60 秒断开：握了手就从零数的话，一握手就崩的扩展会一直重启下去（2026-10-08 主会话）。
- 标准错误直接接文件、不经核心转：核心不读它、不拷它，不怕它写得多（2026-10-08 主会话）。
- 测试用的扩展是端点 crate 里的一个小程序（`miyu-test-extension`），不是 `sh` 脚本：包的程序在 Windows 上找的是 `.exe`，脚本只管得了 Unix（2026-10-08 主会话）。
- 协议版本对不上的不拉起：说不通的话起了也白起（2026-10-08 主会话）。
- 拉起时明着设 `MIYU_HOME`、`MIYU_RESOURCES`，值取核心手上的数据根、资源目录，不从核心自己的环境变量转抄；只加这两个，别的环境照旧继承，三个平台一条路（2026-10-08 主会话，施工 O-18 要的：真核心拉起真的桥的测试）。

- 装卸时照清单前后对：卸掉的停、新的起、变了的重起、没变的不动（2026-10-09 主会话，施工 F-5 下）。比较整份清单，不另记版本号：清单里哪一格变了，起来的进程都可能不一样（参数、能力、配置项）。卸掉的那一格状态留着、记成关着：装回来从零数，不留着没用的进程。

### 还没有的

- 装包时批能力：有了装包那一步，审批挪到装的时候。
- 能力的强制执行、照连接的身份挡方法、沙盒里跑：随多用户、沙盒那一段（扩展的身份是系统账号，施工 O-4 下做了）。
- 扩展提供工具、命令、挂接点：O-2。
