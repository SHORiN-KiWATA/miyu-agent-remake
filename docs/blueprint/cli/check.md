## `miyu check`

### 是什么

一个命令查完人手写的、Miyu 读的文件：系统配置、个人设置、当前目录的项目配置、密钥文件，三层里每个人格的 `persona.toml`、`prompts/examples.md`，两层里每一份软件包清单（施工 9-1 上，`packages.md`）。每一处印出文件、第几行、错在哪、怎么改，有错误退出码是 1，像 `nginx -t`（施工 8-30，2026-10-07 项目主人提、定）。只留这一个检查：原来的 `miyu config check` 并进来去掉了，人格、以后的预设、软件包都不另开分开的 check 命令。

它只是协议的客户端（`22-命令行.md` O5）：连上核心，问 `check`（`protocol.md`），照回应印。查什么、怎么查由核心定：命令行不认识每一种文件。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-cli/src/config.rs` | `Check` 参数、`check()`：借 `miyu config` 那一套连核心、握手（`ConfigCommand::Check` 不出现在 `miyu config` 下面） |
| `crates/miyu-cli/src/config/check.rs` | 问 `check`、印、合计、退出码 |
| `crates/miyu-cli/src/help/{zh,en}/check.txt` | 帮助页 |
| `crates/miyu-endpoint/src/check.rs` | 核心那一半：几份文件在哪、照磁盘上现在的字查、人格每一层各查各的、只查一份的照位置认 |

### 对外的样子

| 参数 | 是什么 |
|---|---|
| `[文件]` | 只查这一份；相对的照当前目录接。照它在哪认是哪一种：`system/config.toml`、个人设置、某个目录下的 `.miyu/config.toml`、密钥文件、某一层人格目录里的 `persona.toml` 或 `prompts/examples.md`、某一层 `packages/` 下的 `<编号>.toml`（施工 9-1 上）。认不出的照核心的原话报（`unknown_file`） |
| `--format text\|json` | `text` 给人看（默认），`json` 给脚本：核心的回应原样，`{"problems":[…]}` |

退出码：0 没有错误（只有警告也是 0）；1 有错误、核心拒绝了、连不上核心；2 参数不对。

### 怎么走

1. 连核心、握手照 `miyu config`（`cli/config.md` 第 1、2 条），给人看的字照回应的 `language`。
2. 问 `check`，带当前目录当 `cwd`，写了文件的带 `file`（绝对路径）。
3. 标准输出上一处一行：`<文件>:<行>:<列> <级别>：<那一句>`，人格的只到行；文件在家目录下的写成 `~/…`、数据根里的接上数据根；「错误」红、「警告」黄。最后一行合计，没有问题的印「没有问题」。

### 样子

```text
$ miyu check
~/.miyu/system/config.toml:2:9 错误：log.level 只能是 error、warn、info、debug、trace 或 off，写的是 "verbose"。改成其中一个，例如 log.level = "info"。这一项先照 "info" 用着（默认值）。
~/.miyu/home/admin/personas/miyu/prompts/examples.md:12 错误：user 和 assistant 要一问一答交替
2 处错误
```

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu/tests/config.rs` | 真核心：全部查、只查一份（照位置认层）、人格写错的那一行、认不出的文件退出码 1、`--format json` |
| `crates/miyu/tests/login.rs` | 密钥文件写错的那一行，不带值；只查密钥文件的只印它 |
| `crates/miyu-endpoint/tests/check.rs` | 核心的 `check`（`protocol.md`） |
| `crates/miyu-cli/src/help/tests.rs` | 帮助页和参数对得上、80 列、文件和编进去的一样 |

### 还没有的

- 预设（P-2）、软件包清单（9-1）、软件包自己的文件（例如通讯平台桥的 `venues.d`，由包在清单里声明怎么查，`miyu check` 挨个调，9-1 时和通讯平台的会话对形状）。
