## 沙盒：macOS

### 是什么

macOS 上助手怎么收紧自己（`sandbox.md`「怎么走」第 4 条第 3 步）：把规格写成一份 Seatbelt 配置，装到自己身上，再换成命令。配置只管读写：

- 文件：能读的、能写的、能写的里面只读的、藏起来的，照规格；
- `.git`、数据根：它们和它们的上级落在能写的地方时，删不掉、改不了名，改个名绕不过去；
- 能替命令到别处读写的系统服务不放行：打开别的程序、AppleEvents、起 launchd 的任务、偏好设置、钥匙串。不挡的话，读写的限制一绕就过去。

网络不管：联网、本机的端口、Unix 套接字都照常（2026-09-29 项目主人定：沙盒管的是读写，不是完全隔离）。

装上就收不回来，命令和它起的子进程都在里面。探测时报的手段是 `seatbelt`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos.rs` | 入口：`run`（换成真实的位置、写配置、装上、换成命令），`mechanisms`（探测时试装一次） |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/resolve.rs` | 规格里的路径换成真实的位置 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/profile.rs` | 照规格写配置和参数：只拼字，不碰系统 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/seatbelt.rs` | 调系统的 `sandbox_init_with_parameters` 装配置：整个 crate 只有这里用 `unsafe` |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/base.sb` | 配置的底子：不看规格，每条命令都一样 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/probe.json` | 探测时用的规格 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/main.rs` | `macos` 这个模块在 macOS 上编；别的 Unix 上跑测试时也编进去，`resolve.rs`、`profile.rs` 的单元测试在 Linux 上也跑 |
| `crates/miyu-sandbox/Cargo.toml` | 放开 `unsafe` 的写法：照抄工作区的 lints，`unsafe_code` 从 `forbid` 改成 `deny`，只在 `seatbelt.rs` 和测试里调系统的那几处放开；每个 `unsafe` 块写 `// SAFETY:`（`undocumented_unsafe_blocks`）。macOS 上测试多一个 `libc`：在只读的文件描述符上试 `fcntl` 要它 |
| `docs/designs/samples/sandbox/macos-spec.json`、`macos.sb` | 例子：一份规格，和照它生成的规则 |

### 对外的样子

**探测**：装得上报 `seatbelt`，装不上报空的：

```json
{"version":1,"platform":"macos","mechanisms":["seatbelt"]}
```

**配置**：底子 `base.sb`，接着照规格生成的规则；连同参数交给系统的 `sandbox_init_with_parameters`（`sandbox-exec -D` 用的就是它）。

- 路径不写进配置的字里，写成参数：配置里写 `(param "READ_0")`，参数另交 `READ_0` 是哪条路径。路径里有引号、反斜杠、中文，都不用转义。
- 参数是一串 `名字, 值, 名字, 值, …`，最后一个空指针。

照规格生成的规则，每一种的写法（`<n>` 是序号）：

| 哪一种 | 写法 |
|---|---|
| `read` 的一条 | `(allow file-read* file-test-existence file-map-executable process-exec (subpath (param "READ_<n>")))` |
| `write` 的一条 | `(allow file-read* file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_<n>")))` |
| `readonly` 的一条 | `(deny file-write* (literal (param "READONLY_<n>")) (subpath (param "READONLY_<n>")))` |
| `hidden` 的一条 | `(deny file* process-exec (literal (param "HIDDEN_<n>")) (subpath (param "HIDDEN_<n>")))` |
| 能读、能写那几片的上级 | 一条 `(allow file-read-metadata file-test-existence …)`，每一片一行 `(path-ancestors (param "READ_<n>"))`（能写的是 `WRITE_<n>`） |
| 删不掉、改不了名的目录 | 每个一条 `(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_<n>"))))` |

- 能读、能写的放行执行（`process-exec`）和把文件映射成可执行的（`file-map-executable`）：工作区里编出来的程序、工具链里的动态库都要。藏起来的连执行也不许。
- `literal` 连同 `subpath` 一起写：还不存在的（例如没有 `.git/hooks` 的仓库），也不许新建。

### 怎么走

**`run`**：

1. 换成真实的位置（`resolve.rs`）。规格说路径都是真实的位置，助手再换一次，防着给的是经过链接的写法：Seatbelt 照真实的位置比，写法对不上的规则等于没写，要挡的就漏了。macOS 上 `/tmp`、`/var`、`/etc` 都是 `/private` 下的链接，`TMPDIR` 在 `/var/folders/…` 下。
   1. 先查每条路径：要是绝对路径，不能有 `.`、`..` 这样的段，不能有 NUL。不然收紧不成。
   2. 整条换成真实的位置（系统的 `realpath`）。换不了的（还不存在、没有权限），照最近一层换得了的上级换，再接上后面几段。
   3. 能读、能写的只用换过的。只读、藏起来的，换过的和原样不一样时两样都写：原样的那一条挡住链接本身，不然删了链接、换一个，就绕过去了。
   4. 同一样里重复的只写一次。
2. 写配置和参数（`profile.rs`，下面「配置怎么写」）。
3. 装上：配置、参数交给 `sandbox_init_with_parameters`。装不上的，收紧不成，带上系统的原话；系统给的那句用完交还给它（`sandbox_free_error`）。
4. 换成命令：和别的 Unix 一样，用 `unix.rs` 的 `exec`。程序不在能读的地方的，系统说 `Operation not permitted`，照 `sandbox.md` 退出 126。

**配置怎么写**：Seatbelt 从上往下读，同一件事，后面的规则压过前面的。

1. 底子 `base.sb`（下面「样子」）。
2. 规格的每一条一行，照路径的深浅（几段）排：浅的在前、深的在后；一样深的，照能读、能写、只读、藏起来的先后；再一样的，照路径的字节。所以同一处由最深的那一条说了算：
   - 数据根落在能写的临时目录里：藏起来的更深，碰不了；
   - 工作区退回了数据根里的 `home/<账号>/workspace/`：能写的更深，照样能读能写；
   - 工作区里的 `.git/hooks`、`.git/config`：只读的更深，写不了。
   - 能读的只放行读，不收回外面那一片的写；只读的只挡写，能不能读看外面那一片。
3. 能读、能写的每一片，它的上级放行看元数据（`stat`；列不出上级里有什么）：有的程序会一层层看上级（找当前目录、换成真实的位置），不放行的话，工作区在读不了的目录下面时（例如退回了数据根里），命令就找不到自己在哪。排在第 2 步后面：上级落在藏起来的里面，也看得了。
4. 删不掉、改不了名的目录：规格的每一条和它的每一层上级，凡是落在能写的那几片里的（能写的那一片自己也算）。排在最后，后面没有规则再放开它。
   - 为什么：Seatbelt 照路径管。`.git` 能改名，改名以后 `hooks` 就不在只读的那条路径上了，改完再改回来；数据根的上级能改名，数据根就挪到了藏起来的那条路径外面；能写的那一片（例如工作区）能删，删了换成一个指到别处的链接，下一条命令照它写规格，就放开了别处。
   - 只管目录：能写的普通文件照样能整个换掉，编辑器存盘就是先写临时文件、再改名盖上去。
5. 参数的名字：`READ_`、`WRITE_`、`READONLY_`、`HIDDEN_`、`KEEP_` 接序号，每一样从 0 起，照写进配置的先后数。

**底子里有什么**（`base.sb`）：

- 没写到的一律不许（`deny default`）：能替命令到别处读写的系统服务，都在「没写到的」里。
- 进程：能起子进程，子进程也在沙盒里；信号、进程信息只对沙盒里的进程：读不到别的进程的环境变量（核心的环境里有模型的 key），`ps` 看不到沙盒外面的进程。
- sysctl：只读名单上的（CPU、内存、系统版本、自己的进程、网络接口），读不到别的进程的命令行参数。
- 网络照常：什么套接字都建得了，连得出去、绑得了、收得进来，Unix 套接字也一样。
- 设备：`/dev/null`、`/dev/zero`、随机数、自己的文件描述符、终端。
- 找路径要经过的：根目录（找当前目录要读它）；链接本身（经过链接找真实的位置要看它，例如 `/tmp`，`~/.gitconfig` 链到别处的）；`/System/Volumes/Data` 的上级。
- 每个进程都可能读的几样：`/private/var/select`（`/bin/sh` 照它选用哪个 shell），`/private/var/db/xcode_select_link`（`/usr/bin` 下的开发工具照它找 Xcode），`/private/var/db/timezone`（本地时间），`/private/var/run/resolv.conf`（`/etc/resolv.conf` 链到它，域名解析的设置）。
- 系统服务只放行这几样：查用户和用户组、系统通知、系统日志、这个用户的临时目录、电源管理；联网要的：网络设置、域名解析的设置、验证书。不放行的，例如偏好设置（`cfprefsd`：它替命令读设置，读得到规格外的地方）、钥匙串、打开别的程序（`open`）、AppleEvents（`osascript` 叫别的程序做事）、launchd 的任务（`launchctl` 起的在沙盒外面跑）。
- 经只读打开的文件也能改它的两个 `fcntl`（`F_MAKECOMPRESSED`、`F_TRANSFEREXTENTS`）：不许。
- 系统目录（`/usr`、`/System` 这些）不在底子里：它们在规格的 `read` 里（`fs.md` 的清单）。规格是空的，连 shell 都起不来。

**`mechanisms`**（探测）：照 `probe.json` 走一遍 `run` 的第 1 到 3 步（每一种规则都用上），装到自己身上。

- 装上了报 `["seatbelt"]`；装不上报 `[]`，原因不报，要看原因就手动跑一次 `run`。例如 Miyu 自己跑在别的沙盒里：Seatbelt 不许套两层。
- 装上以后这个进程就关进去了：只有 `probe` 调它，印完那一行就退出。

### 样子

底子（施工 5-7 合进来时标成样本 `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/base.sb`）：

```scheme
; macOS 的沙盒配置的底子（docs/blueprint/sandbox/macos.md）：不看规格，每条命令都一样。
; 照规格生成的规则接在它后面；同一件事，后面的规则压过前面的。
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

; 网络照常：什么套接字都建得了，连得出去、绑得了、收得进来，Unix 套接字也一样。
(allow system-socket)
(allow network-inbound network-outbound network-bind)

; 设备：空设备、随机数、自己的文件描述符、终端。
(allow file-read* file-test-existence file-write-data
  (literal "/dev/null")
  (literal "/dev/zero"))
(allow file-read* file-test-existence
  (literal "/dev/random")
  (literal "/dev/urandom"))
(allow file-read-data file-test-existence file-write-data (subpath "/dev/fd"))
(allow pseudo-tty)
(allow file-read* file-write* file-ioctl
  (literal "/dev/tty")
  (literal "/dev/ptmx")
  (regex #"^/dev/ttys[0-9]+$"))
(allow file-read* file-write-data file-ioctl (literal "/dev/dtracehelper"))

; 找路径要经过的：根目录（找当前目录要读它），链接本身（经过 /tmp 这样的链接找真实的位置要看它），
; /System/Volumes/Data 的上级。
(allow file-read* (literal "/"))
(allow file-read-metadata
  (vnode-type SYMLINK)
  (path-ancestors "/System/Volumes/Data/private"))

; 每个进程都可能读的几样：/bin/sh 照 select 选用哪个 shell，/usr/bin 下的开发工具照 xcode_select_link 找 Xcode，
; 本地时间照 timezone，域名解析的设置照 resolv.conf。
(allow file-read*
  (subpath "/private/var/select")
  (literal "/private/var/db/xcode_select_link")
  (subpath "/private/var/db/timezone")
  (literal "/private/var/run/resolv.conf"))

; 系统服务只放行这几样：查用户和用户组、系统通知、系统日志、这个用户的临时目录、电源管理；
; 联网要的：网络设置、域名解析的设置、验证书。
; 不放行偏好设置（cfprefsd）：它替命令读设置，读得到规格外的地方。
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

探测用的规格（施工 5-7 合进来时标成样本 `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/probe.json`）：

```json
{"read":["/"],"write":["/private/tmp"],"readonly":["/private/tmp/miyu-sandbox-probe/readonly"],"hidden":["/private/tmp/miyu-sandbox-probe/hidden"]}
```

照规格生成的规则，例子。规格里的路径已经是真实的位置，没有经过链接的（施工 5-7 合进来时标成样本 `docs/designs/samples/sandbox/macos-spec.json`）：

```json
{"read":["/usr","/bin","/private/etc","/Users/me/.cargo"],"write":["/Users/me/project","/private/var/folders/x1/abc/T"],"readonly":["/Users/me/project/.git/hooks","/Users/me/project/.git/config"],"hidden":["/Users/me/.miyu"]}
```

生成的，接在底子后面（施工 5-7 合进来时标成样本 `docs/designs/samples/sandbox/macos.sb`）：

```scheme
(allow file-read* file-test-existence file-map-executable process-exec (subpath (param "READ_0")))
(allow file-read* file-test-existence file-map-executable process-exec (subpath (param "READ_1")))
(allow file-read* file-test-existence file-map-executable process-exec (subpath (param "READ_2")))
(allow file-read* file-test-existence file-map-executable process-exec (subpath (param "READ_3")))
(allow file-read* file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_0")))
(deny file* process-exec (literal (param "HIDDEN_0")) (subpath (param "HIDDEN_0")))
(deny file-write* (literal (param "READONLY_0")) (subpath (param "READONLY_0")))
(deny file-write* (literal (param "READONLY_1")) (subpath (param "READONLY_1")))
(allow file-read* file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_1")))
(allow file-read-metadata file-test-existence
  (path-ancestors (param "READ_0"))
  (path-ancestors (param "READ_1"))
  (path-ancestors (param "READ_2"))
  (path-ancestors (param "READ_3"))
  (path-ancestors (param "WRITE_0"))
  (path-ancestors (param "WRITE_1")))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_0"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_1"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_2"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_3"))))
(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_4"))))
```

参数，照这个先后交：

| 参数 | 路径 | 为什么排在这 |
|---|---|---|
| `READ_0` | `/bin` | 一段 |
| `READ_1` | `/usr` | 一段，字节在 `/bin` 后面 |
| `READ_2` | `/private/etc` | 两段 |
| `READ_3` | `/Users/me/.cargo` | 三段，能读的在前 |
| `WRITE_0` | `/Users/me/project` | 三段，能写的 |
| `HIDDEN_0` | `/Users/me/.miyu` | 三段，藏起来的在最后 |
| `READONLY_0` | `/Users/me/project/.git/config` | 五段 |
| `READONLY_1` | `/Users/me/project/.git/hooks` | 五段，字节在 `config` 后面 |
| `WRITE_1` | `/private/var/folders/x1/abc/T` | 六段 |
| `KEEP_0` | `/Users/me/project` | 能写的那一片自己 |
| `KEEP_1` | `/Users/me/project/.git` | `hooks`、`config` 的上级 |
| `KEEP_2` | `/Users/me/project/.git/config` | 规格的一条，落在能写的里面；是文件，这条管不着它，它自己的只读挡着 |
| `KEEP_3` | `/Users/me/project/.git/hooks` | 同上，是目录 |
| `KEEP_4` | `/private/var/folders/x1/abc/T` | 能写的那一片自己 |

### 出错

收紧不成的，照 `sandbox.md`：印 `miyu-sandbox: cannot confine: <原话>`，退出 125，命令没跑。macOS 上的原话：

| 什么时候 | 原话 |
|---|---|
| 规格里的路径不是绝对路径 | `path is not absolute: "<路径>"` |
| 路径里有 `.`、`..` 这样的段 | `path has . or ..: "<路径>"` |
| 路径里有 NUL | `path has a NUL byte: "<路径>"` |
| 系统装不上 | 系统的原话，换行换成空格 |

- 路径照 Rust 的 `{:?}` 写：带引号，NUL 这样的字转义。
- 这几句只在出错时出现，不常驻，不进登记簿（`sandbox.md` 第 6 条）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/profile/tests.rs`（Linux、macOS 上都跑） | 例子生成的规则和样本逐字节一样，参数照上面的先后；深浅、一样深时的先后；只读、藏起来的连路径自己也写上；上级的元数据只给能读、能写的；删不掉的目录：规格的每一条和它的上级、落在能写的里面的，排在最后；底子在最前面 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/macos/resolve/tests.rs`（Linux、macOS 上都跑） | 经过链接的换成真实的位置：能读、能写的只留换过的，只读、藏起来的原样也留；还不存在的照上级换；相对路径、`.`、`..`、NUL 收紧不成 |
| `crates/miyu-sandbox/tests/macos/`（只在 macOS 上编，真跑助手） | `files.rs`：规格外的读不到；能读的写不了；只读的写不了、新建不了、删不了；`.git`、工作区、数据根的上级改不了名；藏起来的读不了、写不了、`stat` 不了；工作区在数据根里照样能读能写，`pwd -P` 对；规格写成 `/tmp`、`/var/…` 照样挡；中文路径。`bypass.rs`：硬链接、`fcntl` 改不了只读的文件；`open`、`osascript`、`launchctl` 叫不动沙盒外的程序。`network.rs`：网络照常：连得上沙盒外起的 TCP 服务和 Unix 套接字，绑得了端口，`socketpair` 能用，解析得了 `localhost`，连得上 HTTPS（CI 上连 `github.com`）。`probe.rs`：探测报 `seatbelt`；在别的沙盒里跑，探测报空的，`run` 收紧不成、命令没跑。`compat.rs`：照真的规格（系统目录、工具链、工作区、临时目录）跑得起 `/bin/sh`、`zsh`、`git`、`rustc`。`child.rs`：测试程序自己当命令时做的那几件事（解析域名、`fcntl`），照环境变量做 |
| `crates/miyu-sandbox/tests/run.rs` | 各平台共用的那几条：macOS 上助手真收紧，照样对 |

### 出处

- `11-权限与沙盒.md` 第四节（读写都是白名单、`.git` 为什么只读）、第六节（macOS、A8）。
- `sandbox.md`：规格、助手的命令行、退出码、收紧不成的那一句。
- 网络不管：2026-09-29 项目主人定，沙盒管的是读写，不是完全隔离。
- Codex（Apache-2.0）的 Seatbelt 配置：底子里 sysctl、系统服务的名单，上级目录不许改名，两个 `fcntl`。照它的思路自己写；照抄的名单，`base.sb` 开头写明了出处和许可证。
