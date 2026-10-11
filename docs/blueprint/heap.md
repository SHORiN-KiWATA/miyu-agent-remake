## 进程的堆 `miyu-heap`

### 是什么

核心用系统的分配器（`23-性能预算.md` F3）。glibc 释放的内存多半留在自己手里、不还给系统：读完模型目录剩下的、关掉的大会话放下的，核心的内存一直不回落（施工 V-2 中量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。这个 crate 只做两件：glibc 上调两个参数（arena 的个数、大块的门槛），把堆里空着的还给系统。别的平台（macOS、Windows、musl）的分配器自己会还，两件都什么都不做。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-heap/src/lib.rs` | `tune`、`trim`；glibc 上声明、调 `mallopt`、`malloc_trim`，别的平台是空的 |
| `crates/miyu-core/src/lib.rs` | 起来时最先调好（`core.md`「起来的先后」第 0 步） |
| `crates/miyu-core/src/models.rs` | 读完模型目录 trim 一次 |
| `crates/miyu-session/src/open/load.rs` | 载入一个会话以后 trim 一次（`session/actor.md` 第 2 条） |
| `crates/miyu-session/src/actor/life.rs` | 会话的 actor 结束、它占的放掉以后 trim 一次（`session/actor.md`）：停下、删掉、闲够了退下（`protocol.md`「会话表」第 9 条）都算 |

### 对外的样子

- `ARENAS`：8。glibc 默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`store.md` 第 6 条，最多 8 块），少于它就抢锁（限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms）。
- `MMAP_FROM`：128 KiB，多大的一块直接向系统要、放掉当场还。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛抬到它那么大（最多 32 MB），之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、`malloc_trim` 也还不回去。
- `tune() -> bool`：起别的线程之前调一次，glibc 上照 `M_ARENA_MAX`、`M_MMAP_THRESHOLD` 设成这两个，交回都设成了没有；别的平台交回假。
- `trim() -> bool`：`malloc_trim(0)`，交回还了没有；别的平台交回假。要走一遍堆，在阻塞线程里调。

### 怎么走

1. 工作区不许 `unsafe`（`forbid`）；这个 crate 照 `miyu-sandbox` 的办法改成 `deny`，只在声明、调用 glibc 的三处单独放开，每处写明为什么安全：`mallopt` 只改分配器自己的参数，`malloc_trim` 只把分配器手里空着的页还回去，已经分出去的不动、glibc 自己加锁。
2. 什么时候 trim：读完模型目录（原文、解析的半成品放掉了）；载入一个会话以后（读、解的整份日志只留最近一次压缩以后的，`session/actor.md` 第 2 条）；一个会话的 actor 结束以后（它的状态机、事件放掉了：停下、删掉、闲够了退下都算）。都在阻塞线程里、不挡着谁。不定时 trim：空着的时候没有新的可还。
3. 量到的（施工 V-2 再补，`docs/perf/2026-10-10-linux-x86_64-v2再补.md`）：只 trim 的时候，大会话退下以后堆里还有 16 MB 空着的碎片、核心匿名内存比空闲时多 8.8 MB；写明大块的门槛以后只多 3.9 MB，重开大会话、说一句以后 92 MB 降到 49 MB。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-heap/src/tests.rs` | glibc 上调得成；放掉一堆小块以后还得回去；别的平台都交回假 |
| 量尺（`perf.md`） | 大会话没人订阅、闲够了退下以后核心的内存回落 |
