## 软件包清单

### 是什么

一个软件包一份清单：它是什么、哪个程序、给 `miyu` 加哪个子命令、界面认哪几页、核心怎么拉起它、`miyu check` 怎么查它自己的文件（`05-内核接口.md` 第二节、I9）。核心起来时读一次，头经 `package.list` 列得出装了哪些。之后的几步照它走：包的配置项并进配置清单（9-1 下），`miyu <名字>` 转交给包的程序（9-2），界面照清单找（9-3），核心拉起 `process` 包（9-4），预设按包开关（P-2）。

状态：图纸，施工 9-1（上）做了（2026-10-07 主会话；方向是施工方案第三节 9-1 那一行和 I9，项目主人 2026-10-07 定；清单的形状和终端界面、通讯平台的会话对过）。`[settings]` 随 9-1（下），转交子命令、跑包的检查随 9-2，照清单找界面随 9-3，拉起 `process` 随 9-4。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-config/src/package.rs`、`package/reader.rs` | 一份清单读成样子：每一格的写法，写错的报第一处，带代码、第几行（纯逻辑）；`reader.rs` 一张表一张表地读 |
| `crates/miyu-config/src/phrases.rs` | 「语言到一句话」那一格的读法，和人格的名字、说明共用 |
| `crates/miyu-store/src/packages.rs` | 两层在哪、读出所有清单、同编号、子命令名撞了；包放状态的目录 |
| `crates/miyu-endpoint/src/packages.rs` | 核心起来时读一次、记运行日志；`package.list` |
| `crates/miyu-endpoint/src/check.rs` | `miyu check` 照磁盘查清单 |
| `resources/packages/web.toml` | 出厂的网页界面的清单 |

### 对外的样子

**在哪**：出厂的放资源目录的 `packages/<编号>.toml`，管理员自己装的放 `home/<管理员>/packages/<编号>.toml`（`07-存储.md` 第二节；包自己的文件以后放同名目录）。编号就是文件名，写法同人格的编号（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个），不合写法的、不是 `.toml` 的不算。包自己在这台机器上的状态放 `<数据根>/state/packages/<编号>/`，包自己建、自己用。

**格式**（TOML；只收下面这些，不认识的表、键报错）：

```toml
[package]
kind = "ui"                      # ui：界面；process：核心拉起的扩展（9-4），通讯平台的桥也是这一种
version = "0.0.1"                # 可以不写
protocol = [1, 1]                # 说得了的协议主版本 [最低, 最高]，和握手一样
name = { en = "Terminal interface", zh = "终端界面", ja = "ターミナル画面" }
summary = { en = "…", zh = "…" } # 可以不写

[command]                        # 可以没有
name = "tui"                     # miyu 后面敲的那个词：小写字母开头，小写字母、数字、-，最多 32 个
program = "miyu-tui"             # 程序名，不带路径分隔符
about = { en = "Open the terminal interface", zh = "打开终端界面" }

[ui]                             # 只有 kind = "ui" 的能写
opens = ["config"]               # 界面认的页（--page），照配置清单的页名；可以是空的
pages_dir = "web/pages"          # 页面文件的目录，相对资源目录；网页那种才写

[process]                        # 只有 kind = "process" 的能写，要有 [command]
args = ["serve"]                 # 拉起时带的参数，没写的是空的
start = "manual"                 # manual：开关打开才拉起（默认）；always：核心起来就拉起

[check]                          # 要有 [command]
args = ["check"]                 # miyu check 跑 <program> <args…>（9-2）

[settings]                       # 9-1（下）解读；这一步写了照收、不解读
```

1. `name`、`summary`、`about` 照人格的写法：只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。
2. `[check]` 的程序在标准输出上一行一处、每处一个 JSON，格子照 `check` 的回应（`kind`、`file`、`line`、`column`、`level`、`message`），可以多带 `key`、`rule`、`source`；`file` 照核心的写法：数据根里的写相对数据根的，资源目录里的写真的路径。只有警告时退出码 0，有错误 1（和通讯平台的会话 2026-10-07 对过）。

**两层怎么认**：一个包只有一份清单，不像人格那样一层层叠。同一个编号两层都有的，认出厂的，家目录那一份报 `duplicate`。两个包要同一个子命令名的，出厂的先于家目录、同一层照编号，先读到的得，后读到的那一份报 `command_taken`（报在子命令名那一行）。和内置子命令撞的，由 9-2 在命令行那一头拦。

**协议**（`protocol.md`）：`package.list`，不带参数，交回 `{"packages": [...]}`，照编号排（同编号出厂的在前）。

| 格 | 什么时候有 | 是什么 |
|---|---|---|
| `package`、`layer` | 每一项 | 编号；`shipped`、`home` |
| `kind`、`protocol`、`name`、`state` | 读成了的 | `ui`、`process`；`[最低, 最高]`；照连接的语言挑的名字（挑法同 `persona.list`：这种语言、`en`、`zh`、`ja`）；放状态的目录的真路径 |
| `version`、`summary` | 写了的 | 原样；照语言挑 |
| `command` | 有 `[command]` 的 | `{"name", "program", "about"}`，`about` 照语言挑 |
| `opens`、`pages_dir` | `ui` 包；`pages_dir` 写了的 | |
| `process` | `process` 包 | `{"args", "start"}` |
| `check` | 有 `[check]` 的 | `{"args"}` |
| `code`、`problem`、`line` | 写错的、撞了的、读不了的、协议版本对不上的 | 代码；给人看的一句（照连接的语言，`core/human` 的 `package-problems/<code>`，读不了的是 `config/unreadable`）；第几行，有的才有。写错的、撞了的、读不了的只有 `package`、`layer` 和这几格；协议版本对不上的（`code` 是 `protocol_mismatch`）照样带全，头自己决定用不用 |

**`miyu check`**（`cli/check.md`）：多查两层里每一份清单，种类 `package`，写法同人格；协议版本对不上的是警告。写了文件的，某一层 `packages/` 下的 `<编号>.toml` 认作清单（两边换成真的路径比），还没有的报读不了。

### 怎么走

1. **核心起来时读一次**（`Core::new`）：两层的目录读不了的当没有。写错的、撞了的各记一行运行日志 `WARN package invalid package=… file=… error=…`，读不了的 `WARN package unreadable`；照样起来。之后不再读：装、卸、改了清单要重启核心才认（「起来时读一次、装卸要重启」）。
2. **读一份**：照「格式」查，报第一处；必写的少了报在表头那一行（缺 `[package]` 的整份，没有行号）。
3. **`package.list`**：照起来时读到的答，名字照这个连接的语言挑。
4. **`miyu check`**：照磁盘上现在的读，改了马上查得出。

### 出错

| 代码 | 什么时候 |
|---|---|
| `syntax` | 读不成 TOML |
| `unknown_table`、`not_a_table`、`unknown_key` | 不认识的表、该是表的不是表、表里不认识的键 |
| `missing_key` | 少了 `[package]`、`kind`、`protocol`、`name`、`[command]` 的三格 |
| `wrong_kind` | `[process]` 写在 `ui` 包里、`[ui]` 写在 `process` 包里 |
| `needs_command` | `[process]`、`[check]` 没有 `[command]` |
| `bad_kind`、`bad_protocol`、`bad_start` | `kind` 不是 `ui`、`process`；`protocol` 不是两个非负整数、最低不大于最高；`start` 不是 `manual`、`always` |
| `not_text`、`not_texts` | `version` 不是字；`args` 不是字的数组 |
| `not_phrases`、`unknown_language`、`empty_phrase` | 「语言到一句话」那一格写错 |
| `bad_command_name`、`bad_program`、`bad_page`、`bad_pages_dir` | 子命令名、程序名、页名、页面目录的写法不对（`pages_dir` 不能是绝对路径、带 `..`、`\`、`:`） |
| `duplicate`、`command_taken` | 两层同编号的家目录那一份；子命令名被先读到的占了 |
| `protocol_mismatch` | 读成了，说的协议版本不包含 1（列表里照样带全；`miyu check` 是警告） |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-config/src/package/tests.rs` | 两份样例（终端界面会话给的草稿、桥那种）每一格读对；最小的清单；每一种写错报对代码和行 |
| `crates/miyu-config/src/phrases.rs` 的测试 | 语言到一句话的读法、第一处错 |
| `crates/miyu-store/src/packages/tests.rs` | 两层照编号排、不是 `.toml` 的和编号不合写法的不算；空的；同编号认出厂的；子命令名先到先得、报在那一行；写错的照样列出；状态目录 |
| `crates/miyu-endpoint/tests/packages.rs` | `package.list` 的每一格、照语言挑；写错的、同编号、撞名、协议版本对不上、`process` 和 `check`；起来时读一次；`check` 查清单、写了文件的认得出、别的文件认不出 |

### 起草时定的

- 清单的读法放在 `miyu-config`：和配置一样是带行号的 TOML，下一步 `[settings]` 也从这里接；「语言到一句话」那一格挪到 `miyu-config` 和人格共用（2026-10-07 主会话）。
- 写错的照样列出、只有编号和问题：头看得到装了一个写错的包（2026-10-07 主会话，照 `persona.list`）。
- 协议版本对不上的照样带全、多 `code`：不一样的头也许还能用一部分，核心不替它拿主意（2026-10-07 主会话）。
- 桥算 `process`，`[process]` 的 `args`、`start` 照通讯平台的会话要的定（`18-通讯平台.md` Q17，2026-10-07）。

### 还没有的

- `[settings]` 并进配置清单、网页那几项挪进来、`head_start` 的说法：9-1（下）。
- 转交子命令、和内置子命令撞名、跑包的检查、`miyu -h` 列出子命令：9-2。界面照清单找：9-3。拉起 `process` 包、开关：9-4。
- 装包、卸包、锁文件（`07-存储.md` 第二节）。
