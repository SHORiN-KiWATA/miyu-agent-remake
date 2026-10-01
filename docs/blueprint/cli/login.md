## `miyu login`、`miyu logout`

### 是什么

管密钥的命令（施工 8-5，2026-10-01 项目主人定，照 opencode 的 `auth login`、`auth list`、`auth logout` 和 codex 的 `login`、`logout`）：存一个供应商的 key、列出哪几个设了、删掉一个。它们只是协议的客户端（`22-命令行.md` O5）：连上核心，问 `secret.list`、`secret.set`、`secret.delete`，照回应印。

密钥本身怎么存、配置里怎么引用、协议上的样子在 `config.md`（第九条、「协议」），这一页只写命令行这一头。key 从不写在命令行上（会进 shell 的历史），也从不印出来，前几位也不印。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `login`、`logout`，各换上自己那一页帮助 |
| `crates/miyu-cli/src/login.rs` | 参数（`Login`、`Logout`，变成 `KeyCommand`）、连核心以前就拦的参数不对（`early`）、连核心、握手、存、列、删；`login_on` 在连上了的连接上办一次，要问人、读 key 的经 `Console`，测试照它走 |
| `crates/miyu-cli/src/login/pick.rs` | `--list` 的表、选的时候的编号表（几列照显示的宽度对齐，整列都空的不占位置）、敲的字认成哪一个 |
| `crates/miyu-cli/src/config/console.rs` | 人那一头 `Console`（和 `miyu config` 共用）：在不在终端里、读一行、标准输入是不是终端、关掉回显读一行（`rpassword`）、整份读管道 |
| `crates/miyu-cli/src/language/login.rs` | 给人看的字 |
| `crates/miyu-cli/src/help/{zh,en}/login.txt`、`logout.txt` | 帮助页 |

### 对外的样子

| 命令 | 做什么 |
|---|---|
| `miyu login [名字]` | 存一个 key，不回显。写了名字的直接贴这一个；不写的在终端里选 |
| `miyu login --list [--format text\|json]` | 列出哪几个设了、谁在用，不给看 key |
| `miyu logout [名字]` | 删掉一个。不写名字的在终端里从设过的里面选 |

- `--list` 和名字只能写一个；`--format` 只给 `--list`。别的写错：参数不对，退出码 2（`cli/main.md`「参数写错时」）。
- 用到的环境变量：`MIYU_HOME`、`NO_COLOR`；界面语言照 `cli/main.md`。

### 怎么走

照 `config.md` 第十一条。施工时补的细节：

1. **连核心以前**：写了名字、不合名字的写法：说「<名字> 不能当 key 的名字：……」，退出码 2。没写名字、标准输入或标准错误不是终端的（`--list` 不算）：说「要在终端里选，或者写 miyu login <名字>」（`logout` 写 `miyu logout <名字>`），退出码 2。都不拉起核心。
2. **连核心、握手**：照 `miyu config`（`cli/config.md` 第 1、2 条），没在跑就拉起来（施工 8-6 起一律拉起）。
3. **选**（没写名字，在终端里）：`secret.list`。`login` 列全部（设了的、配置里用到还没设的），`logout` 只列设了的。一个都没有：`login` 说「配置里还没有用到密钥的供应商：写 miyu login <名字>」退出码 2，`logout` 说「还没有设过 key」退出码 1。有的：标准错误上印头一行（`login` 是「配置里用到的密钥：」，`logout` 是「设过的 key：」），一个一行 `  编号  名字  谁在用  设没设`，再问（`login`「选一个编号，或者敲一个新名字：」，`logout`「选一个编号：」）。编号表、问的话都在标准错误上，标准输出留给 `--list`。敲的是列出的编号，就是那一个；不是编号、合名字写法的，就是这个名字；直接回车、读到头：说「没选」，退出码 1；别的：说名字不合写法那一句，退出码 2（2026-10-01 主会话定）。
4. **读 key**：标准输入是终端的，标准错误上问「粘贴 <名字> 的 key（不显示）：」，关掉回显读一行（`rpassword`：读的是终端本身，读完把回显开回来）。不是终端的，整份读标准输入。去掉前后空白是空的：说「没收到 key」，退出码 1。原样交给 `secret.set`（核心去掉前后空白）。
5. **存**：`secret.set`。成了：标准错误上一行灰字，照 `replaced` 说「存好了」或「换掉了」。被拒绝的（有控制字符、太长……）：印核心的原话，退出码 1。
6. **`--list`**：`secret.list`。标准输出上一个一行：名字、已设置或未设置、谁在用（几项用「、」连，英文用「, 」）；设了的在前、原色，没设的在后、灰；各自照名字的先后。几列照显示的宽度对齐，整列都空的不占位置。一个都没有：印「还没有设过 key」。`--format json`：回应原样，一行。
7. **删**：`secret.delete`。成了：灰字「删掉了」。`unknown_secret`：说「<名字> 没有设过 key」，退出码 1。

### 样子

见 `config.md`「样子」的 `miyu login`、`miyu logout`。谁都没在用的（M8 里还没有类型是密钥的配置项，8-6 起才有），「谁在用」那一列不占位置：

```text
$ miyu login --list
bigmodel-2  已设置
deepseek    已设置
```

样本 `crates/miyu-cli/src/help/zh/login.txt`（帮助页，中文）：

```text
用法：miyu login [名字]
      miyu login --list [--format text|json]

存一个供应商的 key。不写名字的，从配置里用到的里面选。贴的时候不显示，
也可以从管道进来：echo "$KEY" | miyu login deepseek

选项：
      --list              列出哪几家设了 key，不给看 key 本身
      --format text|json  --list 的输出：json 给脚本
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/login.txt`（帮助页，英文）：

```text
Usage: miyu login [name]
       miyu login --list [--format text|json]

Save a provider's key. Without a name, pick one the config uses. The key
does not show as you paste it, and it can come from a pipe:
echo "$KEY" | miyu login deepseek

Options:
      --list              List which providers have a key, never the key
      --format text|json  Output of --list: json for scripts
  -h, --help              Print help
```

样本 `crates/miyu-cli/src/help/zh/logout.txt`（帮助页，中文）：

```text
用法：miyu logout [名字]

删掉一个供应商的 key。不写名字的，从设了的里面选。

选项：
  -h, --help  印帮助
```

样本 `crates/miyu-cli/src/help/en/logout.txt`（帮助页，英文）：

```text
Usage: miyu logout [name]

Delete a provider's key. Without a name, pick one of those that are set.

Options:
  -h, --help  Print help
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 成了 |
| 1 | 核心拒绝了；没收到 key；没选；要删的没设过；`logout` 选的时候一个都没设过；连不上核心、数据根的错 |
| 2 | 参数不对：名字不合写法（写的、选的时候敲的）；不写名字又不在终端里；`login` 选的时候一个能选的都没有；`--list` 带名字、`--format` 不带 `--list` |

### 给人看的字

见 `config.md`「给人看的字」的「命令行」那张表。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/login/pick/tests.rs` | `--list` 设了的在前、对齐、没设的灰；编号表照样子；敲的编号、新名字、超出的编号、写错的、直接回车 |
| `crates/miyu-cli/tests/login.rs` | 在进程里起核心、假终端照剧本回：写名字的贴 key 走关掉回显的那一条、管道的整份读、空的不收、核心拒了照原话；参数不对的三种；选（编号、新名字、没选、读到头、写错的、一个都没有）；`--list` 两种格式；`logout` 写名字、选、没设过的；两种语言；屏幕上从头到尾没有 key |
| `crates/miyu/tests/login.rs` | 真核心：管道进来的 key 存进文件、0600；`--list`、`logout`、两种语言；参数不对 2；核心没在跑的拉起来、存进去（施工 8-6）；帮助页；整份运行日志（`trace`）、系统日志、屏幕上没有 key |
| `crates/miyu-cli/src/help/tests.rs` | 两页列的选项和程序真有的对得上，最宽 80 列 |

### 出处

- `config.md` 第九条（密钥）、第十一条（`miyu login`、`miyu logout`）、「样子」、「给人看的字」。
- `22-命令行.md` 第二节（输出的规矩、退出码、`--format json`）、第五节、O5。
- 别家：opencode 的 `auth login`、`auth list`、`auth logout`，codex 的 `login`、`logout`。

### 还没有的

- 不写名字时列出目录里的供应商、选了连配置一起写：随 8-11（`models.md`）。
- 借订阅的登录（Claude Code、Codex）：随借订阅那一步。
- 成员自己的 key 存到自己的家目录：随多用户。
- 命令行自己的字的日文：界面语言是 `ja` 时照英文。
