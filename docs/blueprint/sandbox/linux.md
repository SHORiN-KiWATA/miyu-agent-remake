## 沙盒：Linux

### 是什么

Linux 上助手怎么照规格收紧自己。各平台共用的（规格、助手的命令行、退出码、探测）在 `sandbox.md`。

现在（施工 5-2）管文件：用 Landlock 照规格放行读写。它在内核里，不要任何特权，也不要命名空间。Landlock 只能放行，不能在放行的范围里再挖掉一块；挖的那部分（`.git` 只读、数据根藏起来）随 5-3 的挂载命名空间，网络随 5-6。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux.rs` | `mechanisms`、`run`：查规格收不收得住，照规格放行，再换成命令 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux/landlock.rs` | 建 Landlock 的规则集、加规则、收紧自己 |

### 对外的样子

**探测时报的手段**：内核有 Landlock、至少是第 2 版的（第 2 版起才管得了跨目录的改名、链接；更老的一律不许，很多工具会坏），报 `landlock`；没有的报空。

**规格的每一格**：

| 格 | Landlock 怎么放 |
|---|---|
| `read` | 读文件、列目录、执行 |
| `write` | 这个内核的 Landlock 管得到的全部文件操作：读、写、建、删、改名、链接、截断、设备的 ioctl |
| `readonly` | 落在某一条 `write` 里（它本身或者下面）的：挖不掉，这一步拒绝执行 |
| `hidden` | 落在某一条 `read`、`write` 里的：同上，拒绝执行。在放行的范围外的，本来就碰不到，照常执行 |
| `network` | 这一步不管 |

- 规格以外的一律碰不到：读、写、执行都不行，命令起的子进程一样。收紧以前已经打开的（标准输入、输出、错误）照旧能用。
- 规格里写的路径不在的，跳过：碰不到它，也建不出来（上一级没放行）。
- 收紧的时候设 `no_new_privs`：沙盒里的 setuid 程序（例如 `sudo`）拿不到更高的权限。

**收紧不成的那一句**（`sandbox.md` 定的写法：`miyu-sandbox: cannot confine: <原话>`，退出 125，不跑命令），原话是下面之一：

| 原话 | 什么时候 |
|---|---|
| `landlock is not available: <原话>` | 内核没有 Landlock，或者不到第 2 版 |
| `cannot keep <路径> read-only inside a writable path` | `readonly` 落在 `write` 里 |
| `cannot hide <路径> inside an allowed path` | `hidden` 落在 `read`、`write` 里 |
| `cannot open <路径>: <原话>` | 规格里的路径在，但打不开 |
| `landlock: <原话>` | 加规则、收紧自己出了错 |

### 怎么走

1. **`run`**：
   1. 查规格：`readonly` 的每一条，是某条 `write` 本身或者在它下面的，拒绝；`hidden` 的每一条，是某条 `read`、`write` 本身或者在它下面的，拒绝。照路径一段段比，不照字符串的开头比（`/a/bc` 不在 `/a/b` 下面）。
   2. 建规则集：第 2 版的全部文件操作必须管得住，管不住就拒绝；更新的版本多出来的（截断、设备的 ioctl）能管就管。
   3. 每条 `read` 加「读」的规则，每条 `write` 加「全部」的规则。打不开的：不在的跳过，别的拒绝。
   4. 设 `no_new_privs`，收紧自己（助手这时只有一个线程）。
   5. 换成命令（`unix.rs`）：执行权限也在规则里，规格外的程序执行不了，照「执行不了」退出 126。
2. **`mechanisms`**：建一个要求第 2 版文件操作的规则集，建得成就报 `landlock`。只建不收紧，探测的进程不受影响。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/tests/linux.rs` | 真跑助手：规格里的读得了、写得了；规格外的读不了、写不了、执行不了，子进程一样；只读目录里写不了；不在的路径跳过；只读的、藏起来的落在放行范围里拒绝执行（125、那一句）；探测报 `landlock`。内核没有 Landlock 的机器上，只测拒绝执行 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux/tests.rs` | 查规格：本身、下面、旁边（`/a/bc` 和 `/a/b`）、上一级各算不算落在里面 |

### 出处

- `11-权限与沙盒.md` 第四节（边界是白名单）、第六节 Linux（A7：文件用 Landlock）。
- Landlock 的内核文档（各版本管得到的操作）；Codex 的 `linux-sandbox`（只借思路）。

### 还没有的

- 挂载命名空间：`.git/hooks`、`.git/config` 挂成只读，数据根藏起来（5-3）。那以后 `readonly`、`hidden` 落在放行范围里的也照常执行。
- `/proc`：规格里不放它，命令就读不到它，别的进程的环境变量也读不到；要放的话，5-3 起换成只看得到沙盒自己的。
- 网络命名空间、seccomp 禁止新建 Unix 套接字（5-6）。
- 核心给调用带规格、默认放行的清单（5-4）。
