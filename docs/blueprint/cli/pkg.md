## `miyu pkg`

### 是什么

在终端里看装了哪些软件包，从一份清单装、把卸掉的出厂的装回来、卸掉（施工 T-3，设计 `30-插件框架.md` 第九节、第十二节；名字照 `22-命令行.md` 第五节）。连上核心（没在跑就拉起来），走 `package.list`、`package.install`、`package.remove`（`packages.md`「装卸」），不另起一套：装卸当场生效，和头上装卸的是同一条路。给人看的只放人要的：编号、名字，卸掉了的、写错了的标一句；种类、哪一层、版本、功能不印（2026-10-08 项目主人定的设计原则）。搜索、软件源随以后的软件源那一段。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `pkg`；换上帮助页 |
| `crates/miyu-cli/src/pkg.rs` | 参数、找数据根、连核心、握手、照子命令发请求、退出码 |
| `crates/miyu-cli/src/pkg/shown.rs` | 一个软件包印成一行（纯的，测试里照它算） |
| `crates/miyu-cli/src/language/pkg.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/pkg.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出来，照编号 | `package.list` |
| `install <清单\|编号>` | 带 `/`、`\` 或者以 `.toml` 结尾的是一份清单的路径（相对的照现在的目录换成绝对的）；别的是卸掉了的出厂的包的编号，装回来 | `package.install`，带 `path` 或者 `package` |
| `remove <编号>` | 卸掉：家目录里装的删掉，出厂的记一笔 | `package.remove` |

| 选项 | 做什么 |
|---|---|
| `--format text\|json` | `list`：`json` 原样印核心回的那一串，给脚本 |

界面语言照 `cli/main.md`。用到的环境变量：`MIYU_HOME`。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu memory`（`cli/memory.md` 第 1 步）。请求的编号是 `pkg-<16 位十六进制>-<序号>`。
2. **发**：照上面的表。
3. **印**（标准输出）：
   - `list`：一个一行，`<编号>  <名字>`，照编号排（核心把卸掉了的出厂的排在最后，这里照编号排进去），编号照最长的那个对齐、后面空两格；卸掉了的出厂的后面接「（已卸载）」；写错的没有名字，接「（写错了：<哪里不对>）」。
   - `install`：「装好了：<编号>」；`remove`：「卸掉了：<编号>」。
4. **被拒绝**：核心的原话照原样印在标准错误上，退出码 1（`protocol.md` 的 `package.*`）。清单装不上的（`package_invalid`）例外：核心的原话是给程序看的（说的是去 `data` 里看），这里照 `data.problem`、`data.line` 说「装不上：<哪里不对>（第 <几> 行）」。核心断开、请求写不出去照 `miyu rename`。

### 样子

```text
basesystem  基础系统
memory      人格记忆
mermaid     画 mermaid 图（已卸载）
net         联网
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了 |
| 1 | 找不到数据根、连不上核心；被拒绝（没有这个包、必需的、清单装不上、和出厂的撞了……）；核心断开 |
| 2 | 参数不对：子命令写错、少了清单或编号（`cli/main.md`） |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 装好了 | 装好了：{id} | Installed {id} |
| 卸掉了 | 卸掉了：{id} | Removed {id} |
| 装不上 | 装不上：{problem}（第 {line} 行）、装不上：{problem} | Cannot install: {problem} (line {line}), Cannot install: {problem} |
| 已卸载 | （已卸载） | (removed) |
| 写错了 | （写错了：{problem}） | (broken: {problem}) |

核心拒绝时说的话照核心写的原样印。

**帮助页**：`-h`、`--help`、`miyu help pkg`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」（`crates/miyu-cli/src/help/zh/pkg.txt`、`en/pkg.txt`）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/pkg/tests.rs` | 一行的样子：照编号排、对齐，卸掉了的、写错了的各接一截，别的不印；带 `/`、以 `.toml` 结尾的是清单（照现在的目录换成绝对的），别的是编号 |
| `crates/miyu/tests/pkg.rs` | 真跑：列出、照相对路径装、卸；卸掉出厂的标「已卸载」、照编号装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json`；`-h`、子命令的 `-h`、`help pkg` 印的都是那一页，跟着界面语言 |

### 还没有的

- 搜索、软件源、停用（`22-命令行.md` 第五节的 `miyu pkg …`）：随软件源那一段。
