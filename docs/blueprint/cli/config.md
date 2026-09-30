## `miyu config`

### 是什么

看配置的命令（施工 8-2）：最终值和它从哪来、每一层写的什么、有没有写错、文件在哪。它只是协议的客户端（`22-命令行.md` O5）：连上核心，问 `config.get`、`config.schema`、`config.check`，照回应印。改、写、信任的 `set`、`unset`、`edit`、`trust` 随 8-3。

配置本身的机制（分层、项目配置、报错的话、协议）在 `config.md`，这一页只写命令行这一头。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `config`，四个子命令都换上 `config` 那一页帮助 |
| `crates/miyu-cli/src/config.rs` | 参数（`Config`、`ConfigCommand`）、连核心、握手、`get`、`explain`、`path`；`config_on` 在连上了的连接上办一次，测试照它走 |
| `crates/miyu-cli/src/config/check.rs` | `check`：读哪几份、一份份 `config.check`、印、合计、退出码 |
| `crates/miyu-cli/src/config/render.rs` | 印的样子：值照 TOML 写、报错一行、`explain` 的几行照显示的宽度对齐 |
| `crates/miyu-cli/src/config/paths.rs` | 核心报的文件换成真的位置、家目录下的写成 `~/…`；还没有项目配置时它该在哪 |
| `crates/miyu-cli/src/language/config.rs` | 给人看的字（命令行自己的几个词） |
| `crates/miyu-cli/src/help/{zh,en}/config.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 选项 |
|---|---|---|
| `get [键…]` | 印出最终值。只写一个键的只印值 | `--format text\|json` |
| `check [文件]` | 检查配置有没有写错 | `--system`、`--project`、`--format text\|json` |
| `explain <键>` | 这一项每一层写的什么、哪一个生效 | `--format text\|json` |
| `path` | 印出配置文件在哪 | `--system`、`--project` |

- `--system`、`--project` 只能写一个。子命令不认的选项、少了子命令、`explain` 没写键：参数不对，退出码 2（`cli/main.md`「参数写错时」）。
- 用到的环境变量：`MIYU_HOME`、`NO_COLOR`、`DEEPSEEK_API_KEY`（8-6 以前看它拉不拉起核心）；界面语言照 `cli/main.md`。

### 怎么走

1. **连核心**：照 `miyu recap`（`config.md` 第十条第 1 条）：设了 `DEEPSEEK_API_KEY` 的，没在跑就拉起来；没设的，核心在跑的照样连，没在跑的不拉起，说没有可用的模型，退出码 5。8-6 以后 key 来自配置，改成一律拉起。
2. **握手**：`caps.input` 是 `false`。之后给人看的字照回应的 `language`（`cli/main.md`「界面语言」）。
3. **被拒绝的**：`data.problems` 里有东西的（`unknown_config_key`），一条一句印在标准错误上（带最近的键名）；没有的印核心的原话。退出码 1。
4. **`get`**：`config.get`，带当前目录当 `cwd`，写了键的带 `keys`。只写一个键：标准输出上只印值，字不带引号，别的照 TOML 的写法（`true`）。写了几个、一个都没写：一行一个 `键 = 值`，照键名排。`--format json`：回应的 `items` 原样，一行。
5. **`explain`**：`config.get`（`keys` 是这一个、带 `cwd`、`all`），再 `config.schema`。第一行名字、键、说明、什么时候生效；下面每一层一行，从上往下（`config.md`「样子」）。值照 TOML 写；文件换成真的位置、家目录下的写成 `~/…`，后面接 `:行`；几列照显示的宽度对齐（中文算两列），后面还有东西的格补齐，最后一格不补。生效的那一行原色、末尾 `← 生效`；环境变量压着的写 `环境变量 MIYU_LOG`、`← 生效，只管这一次启动`；别的灰；写了、不算的末尾红字 `← 不算：<原因>`。`--format json`：那一项原样，多 `name`、`description`。
6. **`check`**：照 `config.md` 第十条第 7 条。
   - 不写文件：`config.get` 带 `cwd` 拿到几份文件在哪，系统配置、个人设置、当前目录的项目配置一份份读磁盘上现在的字（`miyu-store` 的 `config_file`），交 `config.check`（`layer` 照它是哪一层）。还没有的那一份跳过。写了 `--system`、`--project` 的只查那一份。
   - 写了文件：照 `--system`、`--project` 当那一层查，都不写的当个人设置。文件没有、读不了的报一条读不了。
   - 命令行自己读不了的（读不了、太大、不是 UTF-8）：照核心的说法报一条（`language/config.rs`），不交给核心。
   - 标准输出上一条一行：`<文件>:<行>:<列> <级别>：<那一句>`，整份的问题没有行列；文件写成 `~/…`；级别「错误」红、「警告」黄。最后一行合计，没有问题的印「没有问题」。`--format json`：`{"problems":[…]}`，每一条多一格 `file`（和一行开头的写法一样）。
   - 有错误退出码 1，只有警告、没有问题的 0。
7. **`path`**：`config.get`（`--project` 的带 `cwd`），照 `files` 里那一层的 `file` 换成真的位置，一行，文件还没有也印。不写 `--system`、`--project` 的是个人设置。`--project` 没找到项目配置的：从当前目录往上找有 `.git` 的那一层（仓库的根），没有的就是当前目录，印它下面的 `.miyu/config.toml`，标准错误上说「还没有这个文件」。

### 样子

见 `config.md`「样子」：报错一行、`explain`、`get`。

样本 `crates/miyu-cli/src/help/zh/config.txt`（帮助页，中文）：

```text
用法：miyu config <命令> [选项]

看配置。改配置、信任项目配置的命令随后加上。

命令：
  get [键…]     印出最终的值，只写一个键时只印值
  check [文件]  检查配置有没有写错
  explain <键>  这一项每一层写的什么、哪一个生效
  path          印出配置文件在哪

选项：
      --system            系统配置，不写是个人设置（check、path）
      --project           当前目录的项目配置（check、path）
      --format text|json  get、check、explain：json 给脚本
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/config.txt`（帮助页，英文）：

```text
Usage: miyu config <command> [options]

See settings. Commands to change them and to trust a project config come
later.

Commands:
  get [key…]      Print the values in effect, or just the value of one key
  check [file]    Look for mistakes
  explain <key>   What each layer says and which one wins
  path            Print where the file is

Options:
      --system            The system config, instead of personal settings
                          (check, path)
      --project           The project config here (check, path)
      --format text|json  For get, check, explain: json for scripts
  -h, --help              Print help
```

- 8-3 加 `set`、`unset`、`edit`、`trust` 和 `--yes`、`--no`，照 `config.md` 里 8-3 的那一版。

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 成了；`check` 没有错误 |
| 1 | 核心拒绝了（不认识的键）；`check` 有错误；连不上核心、数据根的错 |
| 2 | 参数不对 |
| 5 | 核心没在跑、又没设 `DEEPSEEK_API_KEY`（8-6 以前） |

### 给人看的字

见 `config.md`「给人看的字」的「命令行」那张表。报错的整句话是核心照连接的语言说的，命令行只加级别、合计和 `explain` 的几个词。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/config/tests.rs` | `get` 的值、报错一行（上色）、合计、`explain` 的几行和图纸一样（两种语言）、环境变量和不算的那一行、文件在哪、还没有项目配置时该在哪 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和四个子命令真有的合在一起一一对得上，最宽 80 列 |
| `crates/miyu/tests/config.rs` | 真核心带三层配置起来：`get`、`explain`、`check`、`path` 印的对，照 `ui.language` 说话，退出码；帮助页；参数不对 2；没有 key、核心没在跑 5 |

### 出处

- `config.md` 第十条（命令行）、「样子」、「给人看的字」；`14-配置.md` 第九节（四种改法里命令行那一种）。
- `22-命令行.md` 第二节（输出的规矩、退出码、`--format json`）、第五节（`miyu config` 的子命令）、O5。
- 别家：`git config --show-origin`（说得出来源）。

### 还没有的

- `set`、`unset`、`edit`、`trust`：8-3。
- 命令行自己的字的日文：界面语言是 `ja` 时照英文（`cli/main.md`）。
