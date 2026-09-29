## 沙盒：macOS

### 是什么

macOS 上助手怎么收紧自己（`sandbox.md`「怎么走」第 4 条第 3 步）：把规格写成一份 Seatbelt 配置，装到自己身上，再换成命令。很轻的一层，照 DeepSeek 的 dsh（2026-09-29 项目主人定）：

- 整盘能读；只有规格的 `write` 里的能写，外加空设备、自己的文件描述符、终端这几样设备；一条能写的都没有，就只剩底子，全盘只读（只读这一级，`sandbox.md`）；
- `hidden` 里的读、写、看、执行都不行（默认是数据根：里面有本机令牌）；
- 撑着规格的那几个目录删不掉、改不了名：Seatbelt 照路径管，改个名就绕过去了；
- 能替命令到别处读写的系统服务不放行：打开别的程序、AppleEvents、launchd 的任务、偏好设置、钥匙串；Unix 套接字只连得上能写的地方的，Docker、ssh-agent 这类连不上。

网络不管（2026-09-29 项目主人定）。装上就收不回来，命令和它起的子进程都在里面。探测时报的手段是 `seatbelt`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos.rs` | 入口：`run`（换成真实的位置、写配置、装上、换成命令），`mechanisms`（探测时试装一次） |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/resolve.rs` | 规格里的路径换成真实的位置 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/profile.rs` | 照规格写配置和参数：只拼字，不碰系统 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/seatbelt.rs` | 调系统的 `sandbox_init_with_parameters` 装配置：整个 crate 只有这里（和测试里调 `fcntl` 的那一处）用 `unsafe` |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/base.sb` | 配置的底子：不看规格，每条命令都一样 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/probe.json` | 探测时用的规格 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/main.rs` | `macos` 这个模块在 macOS 上编；别的 Unix 上跑测试时也编进去，`resolve.rs`、`profile.rs` 的单元测试在 Linux 上也跑 |
| `crates/miyu-sandbox/Cargo.toml` | 放开 `unsafe` 的写法：照抄工作区的 lints，`unsafe_code` 从 `forbid` 改成 `deny`，调系统接口的那几处单独放开；每个 `unsafe` 块写 `// SAFETY:`（`undocumented_unsafe_blocks`）。macOS 上测试多一个 `libc`：在只读的文件描述符上试 `fcntl` 要它 |
| `docs/designs/samples/sandbox/macos-spec.json`、`macos.sb` | 例子：一份规格，和照它生成的规则 |

### 对外的样子

**探测**：装得上报 `seatbelt`，装不上报空的：

```json
{"version":1,"platform":"macos","mechanisms":["seatbelt"]}
```

**配置**：底子 `base.sb`，接着照规格生成的规则，连同参数交给系统的 `sandbox_init_with_parameters`（`sandbox-exec -D` 用的就是它）。

- 路径不写进配置的字里，写成参数：配置里写 `(param "WRITE_0")`，参数另交 `WRITE_0` 是哪条路径。路径里有引号、反斜杠、中文，都不用转义。
- 参数是一串 `名字, 值, 名字, 值, …`，最后一个空指针。

照规格生成的规则，每一种的写法（`<n>` 是序号）：

| 哪一种 | 写法 |
|---|---|
| `write` 的一条 | `(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_<n>")))`，接着 `(allow network-outbound (remote unix-socket (subpath (param "WRITE_<n>"))))` |
| `hidden` 的一条 | `(deny file-read* file-read-metadata file-write* file-test-existence file-map-executable process-exec (literal (param "HIDDEN_<n>")) (subpath (param "HIDDEN_<n>")))`，接着 `(deny network-outbound (remote unix-socket (subpath (param "HIDDEN_<n>"))))` |
| 删不掉、改不了名的目录 | 每个一条 `(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_<n>"))))` |

- `literal` 连同 `subpath` 一起写：藏起来的那一处自己也挡住，还不存在的也不许新建。
- 能写的那一条也放行读：更深的能写的落在藏起来的里面（工作区退回了数据根里）时，要把读一起放开。

### 怎么走

**`run`**：

1. 换成真实的位置（`resolve.rs`）。规格说路径都是真实的位置，助手再换一次，防着给的是经过链接的写法：Seatbelt 照真实的位置比，写法对不上的规则等于没写，要挡的就漏了。macOS 上 `/tmp`、`/var`、`/etc` 都是 `/private` 下的链接，`TMPDIR` 在 `/var/folders/…` 下。
   1. 先查每条路径：要是绝对路径，不能有 `.`、`..` 这样的段，不能有 NUL。不然收紧不成。
   2. 整条换成真实的位置（系统的 `realpath`）。换不了的（还不存在、没有权限），照最近一层换得了的上级换，再接上后面几段。
   3. 能写的只用换过的：放开链接本身的话，删了它换一个，下一条命令就放开了别处。藏起来的，换过的和原样不一样时两样都写：原样的那一条挡住链接本身。
   4. 同一样里重复的只写一次。
2. 写配置和参数（`profile.rs`，下面「配置怎么写」）。
3. 装上：配置、参数交给 `sandbox_init_with_parameters`。装不上的，收紧不成，带上系统的原话的第一行（配置写坏了的，后面几行是它的回溯）；系统给的那句用完交还给它（`sandbox_free_error`）。
4. 换成命令：和别的 Unix 一样，用 `unix.rs` 的 `exec`。

**配置怎么写**：Seatbelt 定先后的规矩（施工 5-7 在 CI 的 macOS 26 上量的）：同一件事，写得细的规则压过笼统的（`file-read-metadata` 压过 `file-read*`、`file*`，不管谁在前）；写得一样细的，后面的压过前面的。所以要互相压的放行和不许，写的是同一串名字：底子里整盘放开读的那一条、规格里能写的、藏起来的，都写 `file-read* file-read-metadata file-test-existence file-map-executable process-exec`。

1. 底子 `base.sb`（下面「样子」）。
2. 规格的每一条两行，照路径的深浅（几段）排：浅的在前、深的在后；一样深的，藏起来的排在能写的后面；再一样的，照路径的字节。所以同一处由最深的那一条说了算，一样深的越严的算（`sandbox.md`）：
   - 数据根落在能写的临时目录里：藏起来的更深，碰不了；
   - 工作区退回了数据根里的 `home/<账号>/workspace/`：能写的更深，照样能读能写。它的上级都在藏起来的里面，看不了元数据：CI 上量过，`pwd -P`、`realpath`、cargo 都照常，不另外放行。
3. 删不掉、改不了名的目录：规格的每一条和它的每一层上级，凡是落在能写的那几片里的（能写的那一片自己也算），照路径的字节排，排在最后。
   - 为什么：Seatbelt 照路径管。数据根的上级能改名，数据根就挪到了藏起来的那条路径外面；能写的那一片（例如工作区）能删，删了换成一个指到别处的链接，下一条命令照它写规格，就放开了别处。
   - 只管目录：能写的普通文件照样能整个换掉，编辑器存盘就是先写临时文件、再改名盖上去。
   - 它写的 `file-write-unlink` 比 `file-write*` 细，放在哪都压得过放行的；排在最后只为好读。
4. 参数的名字：`WRITE_`、`HIDDEN_`、`KEEP_` 接序号，每一样从 0 起，照写进配置的先后数。

**底子里有什么**（`base.sb`）：

- 没写到的一律不许（`deny default`）：能替命令到别处读写的系统服务，都在「没写到的」里。
- 进程：能起子进程，子进程也在沙盒里；信号、进程信息只对沙盒里的进程：读不到别的进程的环境变量（核心的环境里有模型的 key）。setuid 的程序（例如 `ps`）Seatbelt 本来就不许起。
- sysctl：只读名单上的（CPU、内存、系统版本、自己的进程、网络接口），读不到别的进程的命令行参数。
- 网络照常：什么套接字都建得了，连得出去、绑得了、收得进来。Unix 套接字照读写的规矩：只连得上能写的地方的；域名解析要连的 mDNSResponder 单独放行。在别处绑 Unix 套接字，文件的规则本来就不许。
- 整盘能读、能执行、能把文件映射成可执行的。
- 写：只放行规格里能写的，外加空设备、`/dev/fd`（`> /dev/stdout` 要它）、终端、`dtrace` 的那一个设备。
- 系统服务只放行这几样：查用户和用户组、系统通知、系统日志、这个用户的临时目录、电源管理；联网要的：网络设置、域名解析的设置、验证书。CI 上量过：系统的 HTTPS（NSURLSession，`nscurl`）没有 `trustd.agent` 连不上；`curl` 自带证书，一样都不用；别的几样 CI 上量不出要不要，照 Codex 留着：它们替命令读写不了文件，去掉了，系统代理、VPN 的域名解析这类网络上可能坏（网络不管）。不放行的，例如偏好设置（`cfprefsd`：它替命令写 `~/Library/Preferences`）、钥匙串、打开别的程序（`open`）、AppleEvents（`osascript` 叫别的程序做事）、launchd 的任务（`launchctl` 起的在沙盒外面跑）、剪贴板。
- 经只读打开的文件也能改它的两个 `fcntl`（`F_MAKECOMPRESSED`、`F_TRANSFEREXTENTS`）：不许。整盘能读，读得到的文件都只能读，不能让这两个绕过去。

**绕不过去的几种**（CI 上实测过）：能读不能写的文件，硬链接到能写的地方（`ln`）建不了；经符号链接写，照它指到的地方算，写不了；改名挪出来（`mv`）不许；克隆（`cp -c`）是一份新的，改它不动原来的。沙盒外早就建好的硬链接，照它自己的路径算。

**`mechanisms`**（探测）：照 `probe.json` 走一遍 `run` 的第 1 到 3 步（每一种规则都用上），装到自己身上。

- 装上了报 `["seatbelt"]`；装不上报 `[]`，原因不报，要看原因就手动跑一次 `run`。例如 Miyu 自己跑在一个不许再装沙盒的沙盒里（`deny default` 的沙盒一般都不许：装沙盒要调的系统调用没放行）。
- 装上以后这个进程就关进去了：只有 `probe` 调它，印完那一行就退出。

### 样子

样本 `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/base.sb`（底子）：

```scheme
; macOS 的沙盒配置的底子（docs/blueprint/sandbox/macos.md）：不看规格，每条命令都一样。
; 照规格生成的规则接在它后面。同一件事，写得细的规则压过笼统的（file-read-data 压过 file-read*、file*），
; 写得一样细的，后面的压过前面的。
; sysctl 和系统服务的名单参考 OpenAI Codex 的 seatbelt_base_policy.sbpl、seatbelt_network_policy.sbpl
; （Copyright 2025 OpenAI，Apache License 2.0：http://www.apache.org/licenses/LICENSE-2.0），
; 那两份又参考了 Chromium 的沙盒配置。

(version 1)

; 没写到的一律不许：能替命令到别处读写的系统服务（打开别的程序、AppleEvents、launchd 的任务、偏好设置、
; 钥匙串）都在这里挡住。
(deny default)

; 沙盒里能起子进程，子进程也在沙盒里；信号、进程信息只对沙盒里的进程：读不到别的进程的环境变量。
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))

; 读系统参数：CPU、内存、系统版本、自己的进程、网络接口。别的进程的命令行参数读不到。
(allow sysctl-read
  (sysctl-name "hw.activecpu")
  (sysctl-name "hw.busfrequency_compat")
  (sysctl-name "hw.byteorder")
  (sysctl-name "hw.cacheconfig")
  (sysctl-name "hw.cachelinesize_compat")
  (sysctl-name "hw.cpufamily")
  (sysctl-name "hw.cpufrequency")
  (sysctl-name "hw.cpufrequency_compat")
  (sysctl-name "hw.cputype")
  (sysctl-name "hw.l1dcachesize_compat")
  (sysctl-name "hw.l1icachesize_compat")
  (sysctl-name "hw.l2cachesize_compat")
  (sysctl-name "hw.l3cachesize_compat")
  (sysctl-name "hw.logicalcpu")
  (sysctl-name "hw.logicalcpu_max")
  (sysctl-name "hw.machine")
  (sysctl-name "hw.memsize")
  (sysctl-name "hw.model")
  (sysctl-name "hw.ncpu")
  (sysctl-name "hw.nperflevels")
  (sysctl-name "hw.packages")
  (sysctl-name "hw.pagesize")
  (sysctl-name "hw.pagesize_compat")
  (sysctl-name "hw.physicalcpu")
  (sysctl-name "hw.physicalcpu_max")
  (sysctl-name "hw.tbfrequency_compat")
  (sysctl-name "hw.vectorunit")
  (sysctl-name-prefix "hw.optional.arm.")
  (sysctl-name-prefix "hw.optional.armv8_")
  (sysctl-name-prefix "hw.perflevel")
  (sysctl-name "machdep.cpu.brand_string")
  (sysctl-name "kern.argmax")
  (sysctl-name "kern.hostname")
  (sysctl-name "kern.maxfilesperproc")
  (sysctl-name "kern.maxproc")
  (sysctl-name "kern.osproductversion")
  (sysctl-name "kern.osrelease")
  (sysctl-name "kern.ostype")
  (sysctl-name "kern.osvariant_status")
  (sysctl-name "kern.osversion")
  (sysctl-name "kern.secure_kernel")
  (sysctl-name "kern.sysv.semmns")
  (sysctl-name "kern.usrstack64")
  (sysctl-name "kern.version")
  (sysctl-name-prefix "kern.proc.pgrp.")
  (sysctl-name-prefix "kern.proc.pid.")
  (sysctl-name-prefix "net.routetable.")
  (sysctl-name "sysctl.proc_cputype")
  (sysctl-name "vm.loadavg"))

; Java 读 CPU 类型走的是写的接口，其实是读。
(allow sysctl-write (sysctl-name "kern.grade_cputype"))

; 联网不管：什么套接字都建得了，连得出去、绑得了、收得进来。
(allow system-socket)
(allow network-inbound network-outbound network-bind)

; Unix 套接字照读写的规矩：只连得上能写的地方的（照规格生成的规则放开），沙盒外的系统服务（Docker、
; ssh-agent）连不上。域名解析要连的 mDNSResponder 放行。
(deny network-outbound (remote unix-socket))
(allow network-outbound (remote unix-socket (literal "/private/var/run/mDNSResponder")))

; 读整盘放开：读、看元数据、执行、把文件映射成可执行的。藏起来的由照规格生成的规则拦。
(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec)

; 写：只放行规格里能写的（照规格生成的规则），和几样设备：空设备、自己的文件描述符、终端。
(allow file-write-data
  (literal "/dev/null")
  (literal "/dev/zero")
  (subpath "/dev/fd"))
(allow pseudo-tty)
(allow file-write* file-ioctl
  (literal "/dev/tty")
  (literal "/dev/ptmx")
  (regex #"^/dev/ttys[0-9]+$"))
(allow file-write-data file-ioctl (literal "/dev/dtracehelper"))

; 系统服务只放行这几样：查用户和用户组、系统通知、系统日志、这个用户的临时目录、电源管理；
; 联网要的：网络设置、域名解析的设置、验证书（系统的 HTTPS 要 trustd.agent）。
; 不放行偏好设置（cfprefsd）：它替命令写 ~/Library/Preferences，写到规格外的地方。
(allow mach-lookup
  (global-name "com.apple.system.opendirectoryd.libinfo")
  (global-name "com.apple.system.opendirectoryd.membership")
  (global-name "com.apple.system.notification_center")
  (global-name "com.apple.logd")
  (global-name "com.apple.bsd.dirhelper")
  (global-name "com.apple.PowerManagement.control")
  (global-name "com.apple.SystemConfiguration.configd")
  (global-name "com.apple.SystemConfiguration.DNSConfiguration")
  (global-name "com.apple.networkd")
  (global-name "com.apple.trustd")
  (global-name "com.apple.trustd.agent")
  (global-name "com.apple.ocspd"))
(allow ipc-posix-shm-read* (ipc-posix-name "apple.shm.notification_center"))
(allow iokit-open (iokit-registry-entry-class "RootDomainUserClient"))

; Python 的 multiprocessing 要信号量；PyTorch 带的 OpenMP 要登记这块共享内存。
(allow ipc-posix-sem)
(allow ipc-posix-shm-read-data ipc-posix-shm-write-create ipc-posix-shm-write-unlink
  (ipc-posix-name-regex #"^/__KMP_REGISTERED_LIB_[0-9]+$"))

; 这两个 fcntl 经只读的文件描述符也能改文件：F_MAKECOMPRESSED（80）、F_TRANSFEREXTENTS（110）。
(deny system-fcntl (fcntl-command 80 110))
```

样本 `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/probe.json`（探测用的规格）：

```json
{"write":["/private/tmp"],"hidden":["/private/tmp/miyu-sandbox-probe/hidden"]}
```

照规格生成的规则，例子。规格里的路径已经是真实的位置，没有经过链接的。

样本 `docs/designs/samples/sandbox/macos-spec.json`（规格）：

```json
{"write":["/Users/me/project","/private/var/folders/x1/abc/T"],"hidden":["/Users/me/.miyu"]}
```

样本 `docs/designs/samples/sandbox/macos.sb`（生成的，接在底子后面）：

```scheme
(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_0")))
(allow network-outbound (remote unix-socket (subpath (param "WRITE_0"))))
(deny file-read* file-read-metadata file-write* file-test-existence file-map-executable process-exec (literal (param "HIDDEN_0")) (subpath (param "HIDDEN_0")))
(deny network-outbound (remote unix-socket (subpath (param "HIDDEN_0"))))
(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_1")))
(allow network-outbound (remote unix-socket (subpath (param "WRITE_1"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_0"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_1"))))
```

参数，照这个先后交：

| 参数 | 路径 | 为什么排在这 |
|---|---|---|
| `WRITE_0` | `/Users/me/project` | 三段，能写的在前 |
| `HIDDEN_0` | `/Users/me/.miyu` | 三段，藏起来的在后 |
| `WRITE_1` | `/private/var/folders/x1/abc/T` | 六段 |
| `KEEP_0` | `/Users/me/project` | 能写的那一片自己 |
| `KEEP_1` | `/private/var/folders/x1/abc/T` | 能写的那一片自己 |

### 出错

收紧不成的，照 `sandbox.md`：印 `miyu-sandbox: cannot confine: <原话>`，退出 125，命令没跑。macOS 上的原话：

| 什么时候 | 原话 |
|---|---|
| 规格里的路径不是绝对路径 | `path is not absolute: "<路径>"` |
| 路径里有 `.`、`..` 这样的段 | `path has . or ..: "<路径>"` |
| 路径里有 NUL | `path has a NUL byte: "<路径>"` |
| 系统装不上 | 系统的原话的第一行，例如套在不许再装的沙盒里是 `Operation not permitted` |

- 系统装不上时，它自己先在标准错误上印一句 `sandbox initialization failed: <原话>`，接着才是助手那一句：助手那一句总是最后一行。
- 路径照 Rust 的 `{:?}` 写：带引号，NUL 这样的字转义。
- 这几句只在出错时出现，不常驻，不进登记簿（`sandbox.md` 第 6 条）。
- 被沙盒挡住的操作，命令自己说 `Operation not permitted`（`EPERM`）。程序不在能读的地方（藏起来的里面）的，照 `sandbox.md` 退出 126。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/profile/tests.rs`（Linux、macOS 上都跑） | 例子生成的规则和样本逐字节一样，参数照上面的先后；深浅、一样深时藏起来的在后；一样深、一样的照字节；两种写法；删不掉的目录：规格的每一条和它的上级、落在能写的里面的，排在最后；根目录能写的，根目录自己也算；底子在最前面，空的规格只有底子 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/resolve/tests.rs`（Linux、macOS 上都跑） | 经过链接的换成真实的位置：能写的只留换过的，藏起来的原样也留；还不存在的照上级换；已经是真实位置的、重复的只写一次；多出来的 `/` 去掉；相对路径、`.`、`..`、NUL 收紧不成 |
| `crates/miyu-sandbox/tests/macos/`（只在 macOS 上编，真跑助手） | `files.rs`：整盘能读，规格外的写不了、建不了、删不了，空设备、标准输出写得了；藏起来的读不了、`stat` 不了、列不了、`test -e` 看不见、写不了、执行不了；工作区在数据根里照样能读能写，`pwd -P`、`realpath` 对，cargo 不说换不成；能写的那几片、藏起来的在能写的地方里的上级改不了名；规格写成 `/var/…` 照样挡；中文、带引号的路径；能写的地方里硬链接、符号链接、改名、删照常。`bypass.rs`：能读不能写的文件，硬链接、符号链接、改名、`fcntl` 改不了；`open`、`launchctl submit` 做不成，偏好设置、钥匙串写不进。`network.rs`：连得上沙盒外的 TCP 服务，听得了端口；Unix 套接字能写的地方连得上，别处、藏起来的连不上，`socketpair` 能用；解析得了名字，系统的 HTTPS（`nscurl`）下载得下来。`probe.rs`：探测报 `seatbelt`；套在不许再装沙盒的沙盒里，探测报空的，`run` 收紧不成、命令没跑、最后一行是助手那一句。`compat.rs`：照核心会写的规格跑得起 `/bin/sh`、`zsh`、`git`、`cargo`、`rustc`、`cc`。`child.rs`：测试程序自己当命令时做的那几件事，照环境变量做 |
| `crates/miyu-sandbox/tests/run.rs` | 各平台共用的那几条：macOS 上助手真收紧，照样对 |

### 出处

- `11-权限与沙盒.md` 第四节（能替命令在沙盒外读写的系统服务照样挡）、第六节（macOS、A8）。
- `sandbox.md`：规格、几条重叠时谁算数、助手的命令行、退出码、收紧不成的那一句。
- 整盘能读、只管写：2026-09-29 项目主人定，照 DeepSeek 的 dsh。网络不管：2026-09-29 项目主人定。
- Codex（Apache-2.0）的 Seatbelt 配置：底子里 sysctl、系统服务的名单，上级目录不许改名，两个 `fcntl`。照它的思路自己写；照抄的名单，`base.sb` 开头写明了出处和许可证。
