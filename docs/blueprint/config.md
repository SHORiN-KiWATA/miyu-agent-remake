## 配置和密钥

### 是什么

配置是默认值和规则，以后的每个会话都照着它来。人把它写在 TOML 文件里，核心读进来、校验、一层层合出最终值。界面和命令行要改，都把改动交给核心，由核心写回文件，注释和排版照原样留着。人也可以直接手改文件，核心看到文件变了就重读（`14-配置.md`）。

密钥单独放在一份文件里，配置里只写它的名字，或者环境变量的名字。密钥只能写入、替换、删除，不能读回。

这一页写配置的通用机制和密钥：清单、分层、项目配置和它的信任、校验和报错、写盘、留痕、监视和生效、密钥、协议上的 `config.*`、`secret.*`，命令行的 `miyu config`、`miyu login`、`miyu logout`。模型、供应商那一块有哪些键、各是什么意思，归 `models.md`，这一页只给它们留好位置。

状态：图纸，定稿（2026-10-01 起草，主会话审过，项目主人同一天批准）。M8 的 8-1 到 8-5 照它施工（施工方案第三节 M8 那张表），每一节标着由哪一步做。做完一步，这一页照做好的样子改写那几节，页末「要跟着改的别的页」列的几页跟着改。8-1、8-2、8-3 做完了（2026-10-01）：标着 8-1、8-2、8-3 的几节照做好的样子写，施工时定的记在「施工时定的」。命令行 `miyu config` 的样子和走法另有一页 `cli/config.md`。

### 在哪

施工时照这个放：

| 代码 | 管什么 | 哪一步 |
|---|---|---|
| `crates/miyu-config/`（新，第 2 层，纯逻辑） | 配置清单的类型、`settings!` 宏、分层合并和来源、校验和报错、离得最近的键名、项目配置的收紧、改一项的文字变换、生成 JSON Schema 和参考文件、密钥引用的写法。不碰磁盘，进来的是字，出去的是字 | 8-1 起 |
| `crates/miyu-config/src/item.rs` | 一项的声明 `Item`，`settings!` 宏 | 8-1 |
| `crates/miyu-config/src/value.rs` | 值 `Value`：写成 TOML、写成协议上的 JSON（8-1 只有字）；一份最终值 `Values`，设置类型从它变过来。读 TOML 的值随 8-2 | 8-1 |
| `crates/miyu-config/src/list.rs` | 查清单写得对不对：键不重复、不互为前缀、合写法，默认值过自己的校验，选项至少两个 | 8-1 |
| `crates/miyu-config/src/words.rs` | 给人看的字：`Words`（读资源的那一层实现）、资源里 `config` 那一格的样子 `ConfigWords`、查它和清单对不对得上；几个里的一个怎么连（「a、b 或 c」）、一项说明后面那几句 | 8-1 |
| `crates/miyu-config/src/schema.rs`、`reference.rs` | 生成 JSON Schema、参考文件 | 8-1 |
| `crates/miyu-config/src/parse.rs` | 读一份配置的字：解析 TOML，记下每一项在第几行，照清单查类型、层；写错的那一项报问题、不收，不认识的键报警告 | 8-2 |
| `crates/miyu-config/src/merge.rs` | 分层合出最终值和来源（`Origin`）。项目配置只认信任过的、收紧的；环境变量最后盖上；一项在它那一层下面合出来的（`below`，报错的「现在照什么用着」）；每一层写的（`explain`） | 8-2 |
| `crates/miyu-config/src/problem.rs`、`problem/tell.rs` | 报错：原因码、行列、收到的原文、离得最近的键名（`problem.rs`）；照一种语言说成话：期望、改法、现在照什么用着（`tell.rs`） | 8-2 |
| `crates/miyu-config/src/edit.rs` | 改一项、删一项：照 `toml_edit` 记下的位置只换那一段字，别的字节一个不变；新文件第一行 `#:schema`；人敲的字、协议上 JSON 的值照类型读（`input`、`from_json`） | 8-3 |
| `crates/miyu-config/src/secret.rs` | `{ secret = … }`、`{ env = … }` 的写法，密钥的名字 | 8-5 |
| `crates/miyu-store/src/config_file.rs` | 读配置文件：没有的是空的、上限、开头的 BOM、UTF-8、版本（8-2），记下有没有 BOM（8-3）。写（`write`）：顺着链接、先写临时文件再替换、同步、替换前再读一次、Windows 上重试（8-3）。临时文件的名字和生成的文件共用 `durable.rs` 的 `temp_name` | 8-2、8-3 |
| `crates/miyu-store/src/generated.rs` | 核心生成的派生文件：和磁盘上的逐字节比，一样的不写，不一样的先写临时文件再替换 | 8-1 |
| `crates/miyu-store/src/human.rs` | 给人看的字多一格 `config`，读好的字照 `Words` 交给配置清单 | 8-1 |
| `crates/miyu-store/src/journal.rs` | 系统日志、账号日志 `journal.jsonl`：每追加一条都重新打开、截半行、读最后一行接着数 `seq`、追加、同步 | 8-3 |
| `crates/miyu-store/src/watch.rs` | 监视几个目录，按文件名认，合并连着来的变动 | 8-4 |
| `crates/miyu-store/src/secrets.rs` | 密钥文件：读、写，Unix 上 0600 | 8-5 |
| `crates/miyu-endpoint/src/config.rs`、`config/` | 配置服务：手里的几份文件、当前的最终值（8-2）；改、重读（8-3）；推送、推送的订阅（8-4）。它住在核心家底的一把锁里（`Core::config`）。`config/file.rs` 一份文件读好的样子（8-3 起连同字和 BOM），`config/project.rs` 往上找项目配置，`config/methods.rs` 三个查询，`config/set.rs` 的 `config.set`（8-3），`config/journal.rs` 留痕（8-3），`config/wire.rs` 协议上的写法 | 8-2 起 |
| `crates/miyu-endpoint/src/settings.rs` | 端点自己的两项：`ui.language`（8-1 声明，`language_for` 照它和系统的语言算出用哪种语言）、`permission.start_read_only`（8-2） | 8-1、8-2 |
| `crates/miyu-endpoint/src/config/trust.rs`、`config/trusting.rs` | 项目配置的信任：读 `trust.toml`、照仓库和版本认信不信任（8-2），在字上记一个回答（`recorded`，8-3）；`trusting.rs` 是 `config.trust`（8-3） | 8-2、8-3 |
| `crates/miyu-endpoint/src/secrets.rs` | `secret.*` 方法，照名字、环境变量取出密钥 | 8-5 |
| `crates/miyu-store/src/env.rs` 的 `locale` | 系统的语言：`ui.language` 是 `auto` 时照它（第二条第 8 条）。先看 `LC_ALL`、`LC_MESSAGES`、`LANG`，都没设的用 `sys-locale` 看系统设置（`system_locale`，8-2） | 8-1、8-2 |
| `crates/miyu-log/src/settings.rs`、`lib.rs` | `log.level`（8-1）。换级别的把手 `Guard::set_level`：读完配置换一次（8-2），运行中换（8-4） | 8-1、8-2、8-4 |
| `crates/miyu-core/src/settings.rs` | 登记各模块的清单（`items`）；替还没进工作区的终端界面声明 `tui.startup`（`TuiSettings`，8-3）。起来时读配置（`read`，8-2）、照 `log.level` 换运行日志的级别（`log_level`，8-2）、写 Schema 和参考文件（`generate`，8-1）、读密钥、开始监视 | 8-1 起 |
| `crates/miyu-session/src/actor/` | 回合开始时取一份配置的快照，这一轮都用它 | 8-4 |
| `crates/miyu-cli/src/config.rs`、`config/` | `miyu config` 的子命令：`get`、`check`、`explain`、`path`（8-2，`config/check.rs`、`render.rs` 印的样子、`paths.rs` 文件在哪），`set`、`unset`（`config/set.rs`）、`edit`（`config/edit.rs`）、`trust`（`config/trust.rs`），人那一头的接口 `Console`（`config/console.rs`：是不是终端、读一行、开编辑器，8-3）。给人看的字在 `language/config.rs`、`language/config_write.rs`（8-3） | 8-2、8-3 |
| `crates/miyu-cli/src/login.rs`、`login/` | `miyu login`、`miyu logout`：选、贴 key 不回显、列出、删 | 8-5 |
| `crates/miyu-cli/src/help/{zh,en}/config.txt`、`login.txt`、`logout.txt` | 帮助页：`config.txt`（8-2，`miyu config` 和四个子命令印的都是它）、`login.txt`、`logout.txt`（8-5）；主帮助页多一行 `config` | 8-2、8-5 |
| `resources/core/human/{zh,en,ja}.json` | 多一格 `config`：每一项的名字、说明、选项名，页和组的名字。`said` 里多 `config/*`：报错的话、参考文件里的几句 | 8-1、8-2 |
| `docs/designs/samples/config/` | 样本：两份 JSON Schema、参考文件、命令行印的几样 | 8-1 起 |
| `docs/designs/samples/journal/` | 样本：`config.changed.jsonl`、`trust.changed.jsonl`（8-3），`secret.changed`（8-5） | 8-3、8-5 |

分层照 `01-架构.md` 第九节。`miyu-config` 放第 2 层（8-1 登记）：解析、合并、校验、改字都是纯的，单元测试不用磁盘。用白名单里的 `serde`、`serde_json`（JSON Schema 用 `serde_json` 写，参考文件是自己拼的字），和 8-2 加进白名单的 `toml_edit`（0.25，只开 `parse`：读的时候记下每一格的位置；8-3 改一项也只用它记的位置换字，不开写的功能）。端点读 `trust.toml` 也用它。真的读写文件、监视在 `miyu-store`（第 3 层）。配置服务在 `miyu-endpoint`（第 4 层），会话 actor 在同一层，经一个 `tokio::sync::watch` 拿当前的最终值，不反过来引用端点。

每个模块在自己的 crate 里声明自己的几项（「一个模块一种职责」），`miyu-core/src/settings.rs` 把它们登记成一张表，加一个模块只加一行。

### 对外的样子

#### 文件

| 文件 | 是什么 | 谁写 | 哪一步 |
|---|---|---|---|
| `system/config.toml` | 系统配置 | 管理员，经核心或者手改 | 8-2 |
| `home/<账号>/settings.toml` | 个人设置 | 本人，经核心或者手改 | 8-2 |
| 仓库里的 `.miyu/config.toml` | 项目配置 | 仓库的作者，只手改 | 8-2 |
| `system/secrets.toml` | 密钥 | 管理员，只经核心 | 8-5 |
| `system/journal.jsonl` | 系统日志：系统配置、系统密钥的改动 | 核心 | 8-3 |
| `home/<账号>/journal.jsonl` | 账号日志：个人设置的改动、项目配置的信任 | 核心 | 8-3 |
| `home/<账号>/trust.toml` | 信任过、不信任的项目配置：仓库在哪、哪一份内容 | 核心，经 `config.trust` | 8-3 |
| `state/config/config.schema.json` | 系统配置的 JSON Schema | 核心生成 | 8-1 |
| `state/config/settings.schema.json` | 个人设置的 JSON Schema | 核心生成 | 8-1 |
| `state/config/reference.toml` | 参考文件：全部配置项和默认值，只读 | 核心生成 | 8-1 |

- 文件都可以没有。没有的那一层是空的。
- `state/config/` 下的三份是派生的：删了，核心下次起来重新生成。核心不读它们。
- 数据根顶层没有 `config.toml`，所以在家目录里找项目配置时，`~/.miyu` 不会被当成一个仓库的 `.miyu`（`07-存储.md` 第二节）。

例子：`system/config.toml`。核心新建这份文件时写第一行，指向 Schema，写的是相对路径：

```toml
#:schema ../state/config/config.schema.json

[log]
level = "debug"

[ui]
language = "zh"
```

例子：`home/admin/settings.toml`：

```toml
#:schema ../../state/config/settings.schema.json

[ui]
language = "ja"
```

例子：仓库里的 `.miyu/config.toml`：

```toml
# 这个仓库里开的新会话一开始只读
[permission]
start_read_only = true
```

例子：`system/secrets.toml`：

```toml
# Miyu 的密钥：只经 Miyu 写入、替换、删除。不要把这份文件贴给别人。
deepseek = "sk-…"
bigmodel-2 = "…"
```

例子：`home/admin/trust.toml`：

```toml
# Miyu 记着的项目配置的信任：哪个仓库、哪一份内容、信不信任。
[[project]]
path = "~/src/app"
version = "sha256:…"
trusted = true
```

#### 配置清单（8-1）

每个配置项只在清单里声明一次（G1）。清单是 Rust 里的一张表，一项是一个 `Item`：

| 格 | 是什么 |
|---|---|
| `key` | 键，恒为英文，照 `.` 分成几段，例如 `ui.language`。第一段是声明它的模块的编号。第三方扩展的放在 `ext.<扩展>` 下，随扩展那一步 |
| `kind` | 类型，下面「类型」那张表 |
| `default` | 默认值，就是推荐值。必写，宏里不写编译不过 |
| `layers` | 能放在哪几层：`System`、`Personal`、`Project`，至少一层。8-1 有前两种，8-2 加了 `Project` |
| `tighten` | 项目配置怎么收紧，只有 `layers` 里有 `Project` 的才写，必写（下面「收紧」），`list::check` 查。8-2 加 |
| `env` | 这一次启动由哪个环境变量压过。只有 `log.level` 有：`MIYU_LOG`（`28-运行日志.md` LG2） |
| `applies` | 什么时候生效，下面「生效时机」 |
| `ui` | 界面提示：`page` 在哪一页，`group` 哪一组，`common` 是不是常用项（排在前面，不写是 `false`），`control` 用什么控件 |

- 名字和说明给人看，跟着界面语言，不在 Rust 里：放在资源目录的 `core/human/<语言>.json` 的 `config` 那一格，中文、英文、日文三份（下面「给人看的字」）。
- 「谁能改」不另写一格：M8 只有管理员一个人，系统配置由管理员改，个人设置由本人改。按管理能力细分随多用户那一段（`06-多用户与身份.md` 第四节），那时清单加一格、协议的回应加一格，字段只加不改。
- 「跟着人格、预设走，还是跟着人走」（`16-人格与预设.md` 第四节）随人格那一段加。M8 的几项都跟着人走。

**类型**：

| 类型 | TOML 里写 | 查什么 | 哪一步加 |
|---|---|---|---|
| 开关 `bool` | `true`、`false` | 只有这两个 | 8-2（`permission.start_read_only`） |
| 选项 `option` | `"zh"` | 只能是列出的几个之一，区分大小写 | 8-1（`ui.language`、`log.level`） |
| 整数 `int` | `3` | 必写最小、最大 | 第一项用到它的那一步 |
| 小数 `float` | `1.5` | 必写最小、最大。`nan`、`inf` 不收 | 同上（倍率，`models.md`） |
| 文字 `text` | `"…"` | 必写最多几个字 | 同上 |
| 时长 `duration` | `"30s"`、`"10m"`、`"1h"` | 写法照 `miyu ask --timeout`（`cli/ask.md`）。必写最短、最长 | 同上 |
| 路径 `path` | `"~/notes"` | 绝对路径，或者 `~`、`~/` 开头 | 同上 |
| 网址 `url` | `"https://…"` | `http://`、`https://` 开头，解析得了 | 同上（`models.md`） |
| 名字 `name` | `"deepseek"` | 小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符 | 同上 |
| 引用 `reference` | `"deepseek/deepseek-v4"` | 指的东西要在，跨项查 | 同上（`models.md`） |
| 密钥 `secret` | `{ secret = "deepseek" }`、`{ env = "DEEPSEEK_API_KEY" }` | 第九条 | 8-5 |
| 列表 `list` | `[…]` | 每一个照元素的类型查 | 第一项用到它的那一步 |
| 表 `table` | `[a.b]`，或者 `{ … }` | 键照名字的写法，值照元素的类型查 | 同上 |

- 类型照「不为以后写代码」一样一样加：哪一步第一次有一项用到它，哪一步加。8-1 只有选项，8-2 加开关，8-5 加密钥。
- 键里可以有一段是人起的名字，例如 `providers.<名字>.base_url`（`15-模型与供应商.md` 第二节）。这一段照名字的写法查，由第一项用到它的那一步加（8-6，`models.md`）。

**生效时机** `applies`（G7）：

| 取值 | 什么时候 | M8 的例子 |
|---|---|---|
| `now` | 当场（8-1） | `ui.language`、`log.level` |
| `new_session` | 以后开的会话。已经开着的会话不跟着变 | `permission.start_read_only`、`models.chat`（`models.md`） |
| `next_turn` | 下一个回合开始时（第八条） | 供应商的地址、key（`models.md`） |
| `restart` | 重启核心 | M8 没有 |
| `head_start` | 头下次启动。头自己读、启动时读一次的项，核心不管它（8-3） | `tui.startup` |

`new_session`、`head_start` 是这一页加的（`head_start` 8-3 施工时照 `tui.startup` 加）：G7 那张表只有三种，可「新会话默认用什么」这类项，改了以后已经开着的会话本来就不该跟着变，说成「下一个回合」会让人以为当前会话也换了（「要跟着改的别的页」`14-配置.md`）。

**收紧** `tighten`：项目配置信任过才算，算了也只能让限制更严（G3 照原样，2026-10-01 项目主人再确认）。每一项写明哪个方向是严：

| 取值 | 意思 | 哪一步加 |
|---|---|---|
| `true_only` | 开关：项目配置只能打开它 | 8-2（`permission.start_read_only`） |
| `lower` | 数越小越严 | 第一项用到它的那一步 |
| `higher` | 数越大越严 | 同上 |
| `order` | 选项从宽到严排好 | 同上 |
| `fewer` | 列表只能去掉几个，不能加 | 同上 |

- 项目配置写的值，和默认值、系统配置、个人设置合出来的那个比：不比它宽的收下，宽的不算、报 `not_tightening`（第三条）。一样的收下，等于没写。
- 清单里没写 `Project` 的项，项目配置里写了不算、报 `wrong_layer`。默认不能写。

**控件** `control`：`select` 下拉、`toggle` 开关、`number` 数、`text` 一行字、`list` 列表、`secret` 密钥（只显示已设置、未设置）、`custom:<名字>` 头自己做的专门编辑器（例如模型池、供应商的接入向导，`14-配置.md` 第九节）。M8 只用 `select`（8-1）、`toggle`（8-2）。生效时机、控件和类型一样，哪一步第一次用到哪一种，哪一步加。

**声明的写法**：`miyu-config` 的 `settings!` 宏，一处声明，生成设置类型和清单（G1：结构只在 Rust 类型里定义一次）。用 `macro_rules!`，不写过程宏。

例子（`crates/miyu-log/src/settings.rs`）：

```rust
miyu_config::settings! {
    /// 运行日志的配置。
    pub struct LogSettings in "log" {
        /// 记到哪一级，写法同 `MIYU_LOG`（[`crate::level()`]）。
        level: String = "info" {
            kind: option ["error", "warn", "info", "debug", "trace", "off"],
            layers: [System],
            env: "MIYU_LOG",
            applies: now,
            ui: { page: "advanced", group: "log", control: select },
        },
    }
}
```

- 每一项的格照这个先后写：默认值、`kind`、`layers`、`tighten`（能放进项目配置的必写，别的不写）、`env`（可以不写）、`applies`、`ui`（`common` 可以不写）。默认值不写、选项只有一个、一层都不写的，宏认不出来，编译不过（宏的文档里有 `compile_fail` 的例子守着）。
- `kind` 写 `option ["a", "b"]` 或者 `bool`（8-2），默认值照它变成值（字、`true`、`false`）；开关的字段用 `bool`。

例子（`crates/miyu-endpoint/src/settings.rs`，8-2）：

```rust
miyu_config::settings! {
    /// 权限的配置（施工 8-2）。
    pub struct PermissionSettings in "permission" {
        /// 新会话一开局就是只读：她只能查、写计划。项目配置只能把它打开。
        start_read_only: bool = false {
            kind: bool,
            layers: [System, Personal, Project],
            tighten: true_only,
            applies: new_session,
            ui: { page: "permissions", group: "sessions", control: toggle },
        },
    }
}
```
- 键是 `<段>.<字段名>`：`in "log"` 的 `level` 就是 `log.level`。

生成两样：

- `LogSettings::ITEMS`：清单里的这几项，照声明的先后。
- `LogSettings::from(&最终值)`（`From<&Values>`）：带类型的设置，代码只经它读值，不自己读文件、不另写常量（`14-配置.md` 第十节）。最终值 `Values` 是键到值，8-2 的分层合并交出它；8-1 还不读配置，用的是 `Values::defaults(清单)`，全是默认值。最终值里没有的项照默认值，最终值都校验过，这一步不会出错。字段的类型要能从值变过来（`From<&Value>`）：选项用 `String`，拿到的就是那个选项。

**登记**（`crates/miyu-core/src/settings.rs`）：`MODULES` 一个模块一行，现在四行，照这个先后：`UiSettings::ITEMS`、`TuiSettings::ITEMS`（8-3，终端界面还没进工作区，先在这个文件里替它声明，并进来以后挪进它自己的 crate）、`PermissionSettings::ITEMS`（`miyu-endpoint`，8-2）、`LogSettings::ITEMS`（`miyu-log`）。`items()` 把它们接成一张表。设置页的页照第一次出现的先后排：通用、界面、权限、高级。

**M8 的配置项**：

| 键 | 类型 | 默认 | 层 | 项目配置 | 生效 | 哪一步 |
|---|---|---|---|---|---|---|
| `ui.language` | 选项 `auto`、`zh`、`en`、`ja` | `auto`，跟着系统 | 系统、个人 | 不能写 | `now` | 8-1 声明，8-2 用上 |
| `log.level` | 选项 `error`、`warn`、`info`、`debug`、`trace`、`off` | `info` | 系统 | 不能写 | `now`，`MIYU_LOG` 压过 | 8-1 声明，8-2 读，8-4 当场换 |
| `permission.start_read_only` | 开关 | `false` | 系统、个人、项目 | `true_only` | `new_session` | 8-2 |
| `tui.startup` | 选项 `new`、`recent` | `new`，开一个新会话 | 系统、个人 | 不能写 | `head_start` | 8-3 |
| `providers.<名字>.*`、`models.*`、`pools.<名字>.*` | 见 `models.md` | | | | | 8-6 到 8-8 |

- `ui.language` 的 `auto`：跟着系统，终端的头照系统的语言，网页照浏览器（第二条第 8 条，2026-10-01 项目主人定）。它的界面提示：`general` 页的 `display` 组，常用项，下拉。`log.level` 的：`advanced` 页的 `log` 组，下拉。
- 项目配置能写的，M8 里只有 `permission.start_read_only` 这一项（2026-10-01 项目主人定）。
- `tui.startup`：终端界面启动时开一个新会话（`new`），还是接着最近的那一个（`recent`）。头自己用 `config.get` 读，核心不管它（2026-10-01 主会话和终端界面定）。界面提示：`interface` 页（界面）的 `tui` 组（终端界面），下拉。
- `log.level` 只能放在系统配置里：运行日志是整个核心的，一个人设了不能算数。
- 蓝图里写着「配置那一步能改」的几个数（压缩的几个数、`jobs.*`、回收处留几天、空闲多久退出）这次不挪：只挪真要调的（2026-10-01 主会话定）。有人要改哪一个，再为它开一张小单。

#### 最终值和来源（8-2）

每个最终值都说得出来自哪一层、哪个文件的第几行，像 `git config --show-origin`（G2）。协议上写成「来源」：

```json
{"file":"home/admin/settings.toml","layer":"personal","line":3}
{"file":"system/config.toml","layer":"system","line":5}
{"file":"~/src/app/.miyu/config.toml","layer":"project","line":2}
{"layer":"env","name":"MIYU_LOG"}
{"layer":"default"}
```

| 格 | 是什么 |
|---|---|
| `layer` | `default` 默认值、`system` 系统配置、`personal` 个人设置、`project` 项目配置、`env` 环境变量（只有带 `env` 的项有） |
| `file` | 哪个文件。数据根里的写成相对数据根的路径。项目配置写成人看的那种路径，家目录下的写成 `~/…`。默认值、环境变量没有 |
| `line` | 第几行，从 1 数，是这一项的键所在的那一行。默认值、环境变量没有 |
| `name` | 环境变量的名字，只有 `env` 有 |

从下往上，上面的盖掉下面的：默认值、系统配置、个人设置、项目配置，最后是环境变量（只对带 `env` 的项、只管这一次启动）。人格、预设、会话里临时改的三层不在 M8（「还没有的」）。

#### 报错（8-2）

读、校验、改配置时发现的每一处，写成一条「问题」：

| 格 | 是什么 |
|---|---|
| `level` | `error` 错误、`warning` 警告 |
| `code` | 原因码，下表 |
| `file`、`line`、`column` | 在哪。行、列从 1 数，列照 Unicode 字符数。整份文件的问题（读不了、太大）没有行列 |
| `key` | 哪一项。整份文件的问题没有 |
| `expected` | 期望什么，给人看的，照连接的语言 |
| `got` | 收到了什么：原文照抄，最多 80 个字符，多了截掉加 `…`。密钥文件里的问题不带这一格 |
| `suggest` | 拼错了的键名，离得最近的那一个。没有不写 |
| `using` | 现在照什么用着。一项的问题（`wrong_type`、`not_an_option`、`wrong_layer`、`not_tightening`）写 `{"value": …, "from": "default" 或 "system" 或 "personal"}`：丢掉这一项以后，它那一层下面几层合出来的。整份文件的问题写 `{"from": "last_good"}` 或 `{"from": "nothing"}`（8-2 起来时读不好的都是 `nothing`，`last_good` 随 8-4 的重读）。不认识的键、一组键写成值的没有。`config.check` 查的是一段字：一项的问题照核心手里另外几层算，整份的问题不写 |
| `message` | 整句给人看的话，照连接的语言，把上面几样说成一句：错在哪、期望、收到、改法、现在照什么用（第四条）。在哪不在句子里：头照 `file`、`line`、`column` 自己写在前面 |

| 原因码 | 级别 | 什么时候 | 哪一步 |
|---|---|---|---|
| `unreadable` | 错误 | 文件读不了：没有权限、是个目录……（没有这个文件不算） | 8-2 |
| `too_big` | 错误 | 文件超过 1 MiB | 8-2 |
| `not_utf8` | 错误 | 不是 UTF-8 | 8-2 |
| `syntax` | 错误 | TOML 写法不对 | 8-2 |
| `unknown_key` | 警告，`config.set` 里是错误 | 清单里没有这一项 | 8-2 |
| `wrong_type` | 错误 | 类型不对，例如开关写成了 `"yes"`，还有一组键下面写成了一个值，例如 `ui = "zh"` | 8-2 |
| `not_an_option` | 错误 | 选项不在列出的几个里 | 8-2 |
| `out_of_range` | 错误 | 数不在范围里，字太长 | 第一项有范围的那一步 |
| `bad_format` | 错误 | 时长、路径、网址、名字写法不对 | 同上 |
| `bad_reference` | 错误 | 引用的东西没有 | 8-6 起（`models.md`） |
| `wrong_layer` | 错误 | 这一项不能写在这一层 | 8-2 |
| `not_tightening` | 错误 | 项目配置写得比下面几层宽 | 8-2 |
| `untrusted_project` | 警告 | 项目配置还没信任，或者信任以后内容变了：这一份先不用（第三条第 2 条） | 8-2 |
| `unknown_secret` | 警告 | 引用的密钥还没设 | 8-5 |
| `env_not_set` | 警告 | 引用的环境变量核心起来时没有设 | 8-5 |

#### 协议（8-2 到 8-5）

写法照 `protocol.md`：参数表、回应、拒绝的原因码。回应里各层对象的格照名字的字母先后排。

**握手 `hello` 的回应多两格**（8-2）：

| 格 | 值 |
|---|---|
| `language` | `zh`、`en`、`ja` 之一：这个连接给人看的字用哪种（第二条第 8 条） |
| `config_errors` | 系统配置、个人设置、密钥文件里现在有几处错误（不算警告）。没有的不写 |

**`config.schema`**（查询，8-2）：配置清单，按这个连接的语言给出名字和说明。

| 参数 | 类型 | 说明 |
|---|---|---|
| `keys` | 字符串的数组，可以不写 | 只要这几项。不写是全部 |

回应：`items` 每一项，照清单登记的先后。`pages`、`groups` 页和组，照第一次出现的先后。例子（`ui.language` 一项，中文）：

```json
{"groups":[{"id":"display","name":"显示","page":"general"}],"items":[{"applies":"now","common":true,"control":"select","default":"auto","description":"终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。","group":"display","key":"ui.language","layers":["system","personal"],"name":"界面语言","options":[{"name":"跟随系统","value":"auto"},{"name":"中文","value":"zh"},{"name":"English","value":"en"},{"name":"日本語","value":"ja"}],"page":"general","type":"option"}],"pages":[{"id":"general","name":"通用"}]}
```

- 每一项的格：`key`、`type`，照类型带 `options`（选项：`value` 和给人看的 `name`）、`min`、`max`、`max_chars`、`element`，再是 `default`、`layers`、`tighten`（没有不写）、`env`（没有不写）、`applies`、`name`、`description`、`page`、`group`、`common`、`control`。
- 写了清单里没有的键：`unknown_config_key`，`data.problems` 里每个不认识的一条：`code` 是 `unknown_key`，`level` 是 `error`（请求写错了，不是文件里的警告），`key`、`message`，有最近的键名的带 `suggest`，没有行列。
- 名字、说明这种语言里没有的，照英文（`store/resources.md` 第 3 条的退法），英文也没有的名字照键、说明是空的；页、组的名字同样，都没有的照编号。

**`config.get`**（查询，8-2）：最终值，每个值附上来源。

| 参数 | 类型 | 说明 |
|---|---|---|
| `keys` | 字符串的数组，可以不写 | 只要这几项。不写是全部 |
| `cwd` | 字符串，可以不写 | 照这个目录找项目配置（第三条），算进最终值。不写不算项目配置 |
| `all` | 布尔，不写是 `false` | 每一项再列出写了它的每一层，从上往下（`miyu config explain` 用） |

回应：

```json
{"files":{"personal":{"file":"home/admin/settings.toml","version":"sha256:…"},"project":{"file":"~/src/app/.miyu/config.toml","trusted":true,"version":"sha256:…"},"system":{"file":"system/config.toml","version":null}},"items":{"ui.language":{"origin":{"file":"home/admin/settings.toml","layer":"personal","line":3},"value":"zh"}},"problems":[]}
```

- `items`：键到 `{value, origin}`。
- `all` 是 `true` 的，每一项多一格 `layers`：`[{origin, value, used}]`，从上往下，默认值在最后。`used` 是不是它生效。项目配置里写了、不算的（还没信任、不比下面宽、不能写在这一层），`used` 是 `false`，另带 `problem`，是那一条问题的原因码。
- `files`：每一层的文件在哪、版本（第五条第 6 条）。文件还没有的，`version` 是 `null`。没写 `cwd`、没找到项目配置的，没有 `project`。
- `files.project` 多一格 `trusted`：`true` 信任过这一份，`false` 选了不信任，`null` 还没问过或者信任以后内容变了。不是 `true` 的，项目配置不算进最终值（第三条第 2 条）。
- `problems`：这几份文件现在的问题，全部，不只 `keys` 那几项的。
- 写了清单里没有的键：`unknown_config_key`，同 `config.schema`。
- 不带 `cwd` 就不碰项目配置。`cwd` 照 `session.create` 的写法（`protocol.md`「工作目录太宽」第 2 条换成真实的位置），换不成的当没找到项目配置。

**`config.set`**（命令，8-3）：在一层里改一项或几项、恢复默认。或者整份换掉（`miyu config edit` 用）。

| 参数 | 类型 | 说明 |
|---|---|---|
| `layer` | `"system"` 或 `"personal"`，必写 | 改哪一层。项目配置只手改，不收 |
| `changes` | 数组，可以不写 | 每一项是一个对象，见下表 |
| `text` | 字符串，可以不写 | 整份换成这段字 |
| `version` | 字符串或 `null`，写了 `text` 的必写 | 读到的这份文件的版本。`null` 是读的时候文件还没有 |

`changes` 的每一项：

| 格 | 类型 | 说明 |
|---|---|---|
| `key` | 字符串，必写 | 哪一项 |
| `value` | 这一项的值，JSON 的写法 | 改成它 |
| `input` | 字符串 | 人敲的字，核心照这一项的类型读（第五条第 3 条） |
| `unset` | `true` | 从这一层删掉，回到下面一层的值：界面上的「恢复默认」 |
| `expect` | 对象，可以不写 | 这一层里这一项现在应当是什么：`{"value": …}` 是写着这个值，`{}` 是没写。对不上就拒绝。不写不查 |

回应：`keys` 改了的每一项（没变的不列），`version` 这份文件改完的版本：

```json
{"keys":{"ui.language":{"applies":"now","effective":"zh","origin":{"file":"home/admin/settings.toml","layer":"personal","line":3},"value":"zh"}},"version":"sha256:…"}
```

- 每一项：`value` 这一层现在的值（删掉的不写）。`effective`、`origin` 算上别的层以后的最终值和来源（不算项目配置）。`applies` 什么时候生效。
- 一项都没变的：`{"keys":{},"version":<现在的版本>}`，什么都不写，也不记日志。这一层本来就是这个值的、本来就没写又要删的，都算没变；文件还没有、又只是删的，不新建。

1. `changes`、`text` 正好写一个。两个都写、都不写，`changes` 是空的，一项里 `value`、`input`、`unset` 不是正好一个，同一个键写了两次，`layer` 不是那两种，写了 `text` 没写 `version`：`bad_params`。
2. 写了清单里没有的键：`unknown_config_key`。
3. 一次的几项一起查、一起写：有一项不对（类型、范围、不能写在这一层），整条不收，`config_invalid`，`data.problems` 里是每一处。这几条问题查的是请求，不带 `file`、行列，也不说「先照…用着」（什么都没变，8-3 施工时定）。
4. 文件现在读不进来（`syntax` 这类整份的问题），又是改几项的：`config_file_broken`，`data.problems` 里是那几处。这一项所在的那一组在文件里写成了别的东西、放不进去的（`ui = "zh"`、`[[ui]]`，第五条第 2 条第 3 款）也是它，`data.problems` 是这份文件现在的全部问题。整份换的不管这一条。
5. 写了 `expect`、对不上：`config_conflict`，`data.current` 是这一层里这一项现在的样子（`{"value": …}` 或 `{}`），什么都没写。几项里有一项对不上，整条不收，`data.current` 是头一个对不上的那一项的。这一项在文件里写错了（丢掉了）的当没写，是 `{}`。
6. 整份换的：`version` 和现在文件的版本对不上，`config_conflict`，`data.version` 是现在的版本。新的字里有错误（警告不算）：`config_invalid`，`data.problems` 里只有那几处错误（照 `config.check` 的写法，不带 `file`）。
7. 收下的：写盘（第五条）、记日志（第六条）、推 `config.changed`（8-4 起，给订阅着的连接，发这一条的那个连接先见推送、后见回应），再回应。第 3 到 6 条都在第五条第 2 条第 1 款重读过的文件上查：手改过的照新的字。替换前发现这一瞬间有人手改了，从头再来，三次还不行的：`config_conflict`，`data.version` 是现在的版本（第五条第 6 条）。
8. 先落盘，后回应：回应到的时候，文件已经写好、同步过了。配置服务同时换上新的最终值：之后握手的连接、造的会话照新的（已经连着的连接、开着的会话怎么跟，随 8-4）。

**`config.check`**（查询，8-2）：校验一段配置的字，不生效。给 `miyu config edit`、`miyu config check` 和编辑器插件用。

| 参数 | 类型 | 说明 |
|---|---|---|
| `layer` | `"system"`、`"personal"` 或 `"project"`，必写 | 当成哪一层的文件查 |
| `text` | 字符串，必写 | 要查的字 |

回应：`{"problems":[…]}`。项目配置照「收紧」和默认值、系统配置、个人设置合出来的比，不用目录，也不看信没信任。没有问题的，`problems` 是空数组。查的是一段字，问题不带 `file`，头照自己读的是哪份接上。`layer` 不是那三种、`text` 不是字：`bad_params`。

**`config.trust`**（命令，8-3）：信任、不信任一份项目配置（G3）。

| 参数 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 照这个目录找项目配置（第三条第 1 条） |
| `version` | 字符串，必写 | 人看过的那一份的版本：`config.get` 的 `files.project.version` |
| `trust` | 布尔，必写 | `true` 信任，`false` 不信任 |

回应：`{"file":"~/src/app/.miyu/config.toml","trusted":true}`。

1. 缺了格、类型不对：`bad_params`。
2. 这个目录找不到项目配置：`no_project_config`。
3. `version` 和这份文件现在的版本对不上（人看过以后它又变了）：`config_conflict`，`data.version` 是现在的版本。人信任的只能是他看过的那一份。
4. 收下的：写进 `trust.toml`（第三条第 3 条），记账号日志 `trust.changed`（第六条），落了盘才回应。写之前重读 `trust.toml`，手改过的在新的字上记；替换前发现有人手改，重来，三次还不行的：`config_conflict`。`trust.toml` 读不懂（手改坏了）、写不成：`internal_error`，记一条 `WARN config not written`，什么都没变（8-3 施工时定）。
5. 不推送：项目配置不推（第三条第 5 条）。下一个回合、下一个会话照新的。

**`session.create`、`session.send` 的回应多一格** `untrusted_project`（8-2）：这一次实际干活的目录找得到项目配置，又还没问过（`trust.toml` 里没有这个仓库，或者记的内容和现在的不一样），写它在哪，写法同来源的 `file`。信任着的、选了不信任的、没有项目配置的，不写。头照它问人：终端界面里问（M9），`miyu ask` 印一行（第十条第 10 条）。

**订阅配置的推送**（8-4）：`subscribe`、`unsubscribe` 的 `stream` 多一种 `config`，不带 `session`。

| 参数 | 类型 | 说明 |
|---|---|---|
| `stream` | `"events"` 或 `"config"`，必写 | `config` 是配置的推送 |
| `session` | 字符串 | `events` 必写。`config` 不写，写了是 `bad_params` |

回应 `{}`。订阅了以后，系统配置、个人设置每变一次，推一条 `config.changed`。读得太慢、掉了队，推 `resync`，`params` 是 `{"stream":"config"}`，这个订阅停了，头重新订阅、`config.get` 补上。

**推送 `config.changed`**（8-4）：

```json
{"jsonrpc":"2.0","method":"config.changed","params":{"by":{"kind":"person","account":"admin"},"keys":{"log.level":{"applies":"now","effective":"debug","origin":{"file":"system/config.toml","layer":"system","line":4},"value":"debug"}},"layer":"system","problems":[],"version":"sha256:…","via":"set"}}
```

| 格 | 是什么 |
|---|---|
| `layer` | `system` 或 `personal`。项目配置不推：不监视，每一轮开始时读（第三条） |
| `via` | `set` 经 `config.set` 改的，`edit` 经 `config.set` 整份换的，`file` 手改、核心看到文件变了 |
| `by` | 谁改的，`via` 是 `set`、`edit` 才有。写法照 `kernel/ids.md`，`kind` 在最前 |
| `version` | 这份文件现在的版本。文件被删了是 `null` |
| `keys` | 这一层里变了的每一项，格同 `config.set` 的回应 |
| `problems` | 这份文件现在的全部问题。改好了的推一条空的，头照它收起报错 |

- 只有 `keys` 空、`problems` 变了的也推：文件改坏了、又改好了，头都要知道（G8）。
- 个人设置的推给这个账号的连接，系统配置的推给全部。M8 只有管理员，都推。

**`secret.set`**（命令，8-5）：写入或替换一个密钥。

| 参数 | 类型 | 说明 |
|---|---|---|
| `name` | 字符串，必写 | 密钥的名字，照名字的写法 |
| `value` | 字符串，必写 | 密钥本身 |

回应 `{"replaced": <布尔>}`，落了盘才回：`replaced` 是这个名字原来有没有设。存的是去掉前后空白的值（粘贴时常带着换行）。名字不合写法、`value` 去掉前后空白是空的、有控制字符、超过 16 KiB：`bad_params`。M8 只写系统的 `system/secrets.toml`。成员自己的随多用户，那时加一格 `scope`，只加不改。

**`secret.delete`**（命令，8-5）：`name`，必写。回应 `{}`。没有这个密钥：`unknown_secret`。

**`secret.list`**（查询，8-5）：不带参数。只列名字和是否已设置，从不交出值：

```json
{"secrets":[{"name":"bigmodel-2","set":false,"used_by":["providers.bigmodel.keys"]},{"name":"deepseek","set":true,"used_by":["providers.deepseek.keys"]}]}
```

- 列的是设了的，和配置里引用了、还没设的，照名字排。
- `used_by`：系统配置、个人设置里哪几项引用了它，照键名排。没有是空数组。

**新的原因码**（`code` 都是 -32010）：

| 原因码 | 什么时候 | `data` 多的格 | 哪一步 |
|---|---|---|---|
| `unknown_config_key` | `config.schema`、`config.get`、`config.set` 写了清单里没有的键 | `problems` | 8-2 |
| `config_invalid` | `config.set` 的值不对、不能写在这一层，整份换的字里有错误 | `problems` | 8-3 |
| `config_conflict` | `config.set` 的 `expect`、`version`，`config.trust` 的 `version` 对不上 | `current` 或 `version` | 8-3 |
| `config_file_broken` | 文件现在读不进来，没法只改几项 | `problems` | 8-3 |
| `no_project_config` | `config.trust` 时这个目录找不到项目配置 | | 8-3 |
| `unknown_secret` | `secret.delete` 删的密钥没有 | | 8-5 |

`data` 原来只有 `reason`，这几个多一格（`protocol.md`「回应」）。

#### 系统日志、账号日志（8-3）

- 两份都是 JSONL，一行一条，外壳照事件的写法（`kernel/events.md`：`seq`、`at`、`kind`、`by`、`cause`、`body`，没有 `turn`）。种类不进内核的种类表，内核读到照不认识的种类处理。
- `seq` 一份文件里从 1 数起。一份文件只有一个写者：M8 是配置服务。
- 系统配置、系统的密钥改动记进 `system/journal.jsonl`。个人设置的改动、项目配置的信任记进 `home/<账号>/journal.jsonl`。项目配置文件本身的改动不记：核心不写它，也不监视它。

`config.changed`：

样本 `docs/designs/samples/journal/config.changed.jsonl`（一行，文件里 `seq` 从 1 数起；`endpoint` 的 `config/journal/tests.rs` 照它逐字节比）：

```json
{"seq":1,"at":"2026-10-01T08:00:00.000Z","kind":"config.changed","by":{"kind":"person","account":"admin"},"cause":"config-9f2c4e1a7b3d5f60-1","body":{"layer":"personal","file":"home/admin/settings.toml","via":"set","changes":[{"key":"ui.language","old":"en","new":"zh"}]}}
```

- `body` 的格照这个先后：`layer`、`file`、`via`（`set`、`edit`、`file`）、`changes`。
- `changes` 每一项：`key`，`old` 改之前这一层的值，`new` 改之后的。之前没写的不带 `old`，删掉的不带 `new`。只记清单里的项，不认识的键、写错丢掉的、不能写在这一层的不记。照键名排。整份换的只动了注释、没有一项变了的，照样记一条，`changes` 是空的（落了盘就留痕，8-3 施工时定）。
- 手改被看到的（`via` 是 `file`）：`by` 是 `{"kind":"kernel"}`，没有 `cause`。谁手改的核心不知道。

`secret.changed`（8-5）：

```json
{"seq":4,"at":"2026-10-01T08:01:00.000Z","kind":"secret.changed","by":{"kind":"person","account":"admin"},"cause":"secret-9f2c4e1a7b3d5f60-1","body":{"name":"deepseek","action":"set","via":"set"}}
```

- `action`：`set` 新设、`replaced` 换掉、`deleted` 删掉。`via`：`set`（经 `secret.set`、`secret.delete`）、`file`（手改被看到的，`by` 同上）。
- 只记名字，从不记值（`07-存储.md` 第九节）。

`trust.changed`（8-3）：

样本 `docs/designs/samples/journal/trust.changed.jsonl`：

```json
{"seq":1,"at":"2026-10-01T08:02:00.000Z","kind":"trust.changed","by":{"kind":"person","account":"admin"},"cause":"config-9f2c4e1a7b3d5f60-2","body":{"path":"~/src/app","version":"sha256:…","trusted":true}}
```

- `path`：仓库在哪（`.miyu` 所在的那一层），写法同 `trust.toml`。`version`：信任、不信任的是哪一份。`trusted`：信不信任。手改 `trust.toml` 被看到的，照 `config.changed` 的规矩多一格 `via`（`file`），`by` 是内核。

#### 命令行 `miyu config`（8-2、8-3）

| 子命令 | 做什么 | 哪一步 |
|---|---|---|
| `get [键…]` | 印出最终值。只写一个键的只印值 | 8-2 |
| `set <键> <值>` | 改一项，默认改个人设置 | 8-3 |
| `unset <键>` | 从这一层删掉一项，回到下面一层的值 | 8-3 |
| `edit` | 用编辑器打开，存盘时先检查 | 8-3 |
| `check [文件]` | 检查配置有没有写错 | 8-2 |
| `explain <键>` | 这一项每一层写的什么、哪一个生效 | 8-2 |
| `path` | 印出配置文件在哪 | 8-2 |
| `trust` | 看当前目录的项目配置会改什么，信任或者不信任它 | 8-3 |

| 选项 | 做什么 | 哪几个子命令认 |
|---|---|---|
| `--system` | 系统配置 | `set`、`unset`、`edit`、`check`、`path` |
| `--project` | 当前目录的项目配置 | `edit`、`check`、`path`；`set` 也认，认了说项目配置只能手改、退出码 2 |
| `--format text\|json` | `text` 给人看（默认），`json` 给脚本 | `get`、`check`、`explain` |
| `--yes`、`--no` | 信任、不信任，不问 | `trust` |

- 查询类的三个有 `--format json`（`22-命令行.md` 第二节）。
- `--system`、`--project` 只能写一个。子命令不认的选项：参数不对，退出码 2（`cli/main.md`「参数写错时」）。
- 用到的环境变量：`MIYU_HOME`、`NO_COLOR`，`edit` 还有 `VISUAL`、`EDITOR`。
- `trust` 是这一页加的第八个子命令：`22-命令行.md` 第五节只列了七个，没有界面的时候，第一次遇到项目配置得有地方问（「要跟着改的别的页」）。

#### 命令行 `miyu login`、`miyu logout`（8-5）

管密钥的命令（2026-10-01 项目主人定，照 opencode 的 `auth login`、`auth list`、`auth logout` 和 codex 的 `login`、`logout`）。它们调的是 `secret.set`、`secret.delete`、`secret.list`，协议不变。以后借订阅的登录（Claude Code、Codex）也走 `miyu login`（「还没有的」）。

| 命令 | 做什么 |
|---|---|
| `miyu login [名字]` | 选一个供应商，贴它的 key，不回显。写了名字的直接贴这一个 |
| `miyu login --list` | 列出哪几家设了 key，不给看 key 本身 |
| `miyu logout [名字]` | 删掉这一家的 key。不写名字的，从设了的里面选 |

| 选项 | 做什么 |
|---|---|
| `--list` | 只给 `login` |
| `--format text\|json` | 只给 `login --list` |

- 名字就是密钥的名字，配置里写 `{ secret = "<名字>" }` 引用它（第九条）。一家配了几个 key 的，每个 key 一个名字，例如 `bigmodel`、`bigmodel-2`。
- key 从不写在命令行上：会进 shell 的历史。

### 怎么走

**一、清单和生成的文件**（8-1）

1. 清单是各模块 `ITEMS` 登记成的一张表（`miyu-core/src/settings.rs` 的 `MODULES`），照登记的先后，一个模块里照声明的先后。核心起来时合成一次，之后不变。
2. 键：至少两段，每一段是小写字母开头，只有小写字母、数字、`_`。第一段是声明它的模块的编号，`ext` 留给扩展，内置的不许用。
3. 两个键不指同一件事：键不重复。一个键也不能是另一个键按段数的前缀（有了 `ui.language` 就不能再有一项叫 `ui`，不然 `ui` 那一格是表还是值说不清）。照段比：`ui.lang` 不是 `ui.language` 的前缀。
4. 每一项的默认值要过它自己的校验（选项：是列出的之一，区分大小写）。选项至少两个、不重复，至少能放一层、层不写重。整数、小数、时长必写范围，文字必写最多几个字，随这几种类型加。
5. 清单写在代码里，写错了是程序的错：第 2 到 4 条由 `miyu_config::list::check` 查，核心的测试照登记的全部清单查一遍（「守着它的」），核心起来时不再查。资源里的字和清单对不对得上，由 `miyu_config::words::check` 查，同样只在测试里：每一项在中文、英文、日文里都有名字、说明，选项都有名字，用到的页和组都有名字，资源里没有多出来的项、选项、页、组。
6. 核心起来时，找到资源目录以后（`core.md`「起来的先后」第 6 步），照管理员的 `ui.language` 的最终值生成三份：`state/config/config.schema.json`（能放进系统配置的项）、`settings.schema.json`（能放进个人设置的项）、`reference.toml`（全部）。8-1 还不读配置，最终值就是默认值 `auto`；8-2 起读完配置（第二条）以后生成。`auto` 的照核心这边的系统的语言（`miyu-store` 的 `locale`）：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，别的、没有的是 `en`（`UiSettings::language_for`，和第二条第 8 条握手时算的一样）。8-1 的 `locale` 照 `LC_ALL`、`LC_MESSAGES`、`LANG` 的先后取第一个设了、不是空的，和命令行认的一样；8-2 换成 `sys-locale`，这几个都没设的再看 macOS、Windows 的系统设置。
7. 字照这种语言读（`Human::load`，退法照 `store/resources.md` 第 3 条），每一份和磁盘上已经有的逐字节比，一样的不写。不一样的先写旁边的临时文件 `.<文件名>.<进程号>-<计数>.tmp`（只许新建）、同步，再改名盖上，再同步目录（`miyu-store` 的 `generated.rs`）。比第五条第 4 到 7 条少几样：不顺着链接找本体、不带原来的权限位、替换之前不再读一次、Windows 上改名失败不重试。它们是派生的，没人链接、没人手改，这一次写不成下次起来再写。
8. 写不成的（目录建不了、写不进、改不了名），字读不懂的，要用的字缺了的：那一份不写，记一条 `WARN config schema not written file=state/config/<文件名> error=…`，照样起来：它们是派生的，缺了只是编辑器没有补全。缺字的原因写成 `no words for <哪一句>`（`config/facts` 这样的编号，一项的名字、说明缺了写 `config.items.<键>`）。
9. JSON Schema：draft-07。最上面三格 `$schema`、`properties`、`type: object`。键照 `.` 分段，前几段是一层层的表，每一张写成 `{"properties": {…}, "type": "object"}`；最后一段是这一项的属性：`title` 是名字，`description` 是说明接上「能写什么、能放在哪几层、什么时候生效」那几句（`config/schema-description`、`config/facts`），`default` 是默认值，选项写 `type: string` 和 `enum`（照清单的先后）；整数、小数的 `minimum`、`maximum` 随这几种类型加。不写 `additionalProperties: false`：不认识的键只是警告（G8），编辑器也不该标成错。格照名字的字母先后排，两格缩进，最后一个换行。这一层没有一项能放的，`properties` 是空的。
10. 参考文件：开头两行注释（`config/reference-header`、`config/reference-where`）说明它是生成的、改它没有用、要改的写在哪。接着键照表分开：表是键去掉最后一段，表照名字的字母先后，表里的项也照字母先后（不照登记的先后：和 `miyu config get` 一样好找）。表和表之间、项和项之间空一行。每一项先两行注释：名字和说明（`config/reference-item`）；能写什么、能放在哪几层、什么时候生效（`config/facts`）；再一行 `键 = 默认值`，值照 TOML 写（字写成双引号的字符串，引号、反斜杠、控制字符转义）。字里带换行的，每一行都写成注释。核心不读它。
11. 「能写什么」「能放在哪几层」里几个里的一个照 `cli/main.md`「参数写错时」的连法：一个的就是它，两个用「或」，三个以上前面的用顿号（英文逗号）。连词、标点也是字，在 `said` 里：值（写成代码的）用 `config/or-values`，中文「或」两边空一格（`trace 或 off`）；层的名字这类字用 `config/or`，不空格（`系统配置或个人设置`）；前面的都用 `config/list`。
12. `ui.language` 变了（第八条），这三份照新的语言重新生成（8-4）。

**二、读和分层**（8-2）

1. 核心起来时，在找到资源目录以后、在套接字上等连接之前，读系统配置、管理员的个人设置和 `trust.toml`，密钥文件随 8-5（`core.md`「起来的先后」多一步）。读不进来不影响起不起得来（G8）：问题记下，那一层照空的算，有问题的每份记一条 `WARN config problems file=… errors=… warnings=…`。`trust.toml` 读不进来的照没有记录（每一份项目配置都当还没问过），记一条 `WARN trust not read file=… error=…`；里面写法不对的那一条不算。
2. 读一份文件（`miyu-store` 的 `config_file.rs`）：
   1. 没有这个文件：这一层是空的，版本 `null`，不算问题。
   2. 顺着链接找到本体再读（链接指向的不在数据根里也读）。读不了：`unreadable`。
   3. 超过 1 MiB（1,048,576 字节）：`too_big`，不读。读到上限多一个字节就停。
   4. 开头的 UTF-8 BOM 去掉，写回时照样加回。不是 UTF-8：`not_utf8`。
   5. 版本是整份字节（带 BOM）的 SHA-256，写成 `sha256:` 加 64 位十六进制。
3. 解析（`miyu-config` 的 `parse.rs`，用 `toml_edit`）：
   1. TOML 写法不对：`syntax`，行、列照 `toml_edit` 报的位置（它报的错的开头，例如字没收尾的，指到那一行行尾）。`why` 只取它的原话（`TomlError::message` 的最后一行），不带它印出来的那一行原文：原文可能很长，密钥文件里更不能印。
   2. 照清单一项项认：键在清单里、这一层能写、类型和范围对的，收下，记下键所在的行。
   3. 不在清单里的键：`unknown_key`，警告，原样留在文件里，不进最终值。不认识的一张表往里走，每一项照整个键报（`weird.thing`），离得最近的键名才找得准；空的不认识的表没什么可报。一组键（`ui`）下面写成了值：`wrong_type`，期望一张表。清单里的一项写成了表：`wrong_type`。
   4. 类型、范围、写法不对：那一项报错，不进最终值。
   5. 这一层不能写的：`wrong_layer`，不进最终值；值写得对的照样记下来，`explain` 列得出它、写明不算。
   6. 位置：一项的问题指到值（`log.level = "verbose"` 的第 9 列），不认识的键指到键；点号连着写的键（`ui.langauge = …`）指到整个键的开头。来源的 `line` 是键所在的那一行。列照 Unicode 字符数，`\r\n` 的 `\r` 在行尾不算进列。
4. 一份文件里有错，照这样用（G8，2026-10-01 项目主人定只丢写错的那一项）：
   - 一项的问题：只丢这一项，照下面几层合出来的，下面都没写的就是默认值。别的项照常生效。
   - 整份的问题（`unreadable`、`too_big`、`not_utf8`、`syntax`）：TOML 读不懂，这份文件整份照上一次读好的用。起来时就读不好的，照空的。
   - 两种都记成问题，所有的头都看得到：`config.get` 的 `problems`、推送、握手的 `config_errors`、`miyu config check`。
5. 合并（`merge.rs`）：默认值、系统配置、个人设置、项目配置，上面的盖掉下面的，每一项记下来源（「最终值和来源」）。带 `env` 的项，环境变量设了、不是空的、读得懂的，最后盖上去：去掉前后空白，选项不分大小写（交回清单里的写法，`MIYU_LOG` 原来就不分），开关只认 `true`、`false`。读不懂的当没设，照配置，记一条 `WARN MIYU_LOG not understood, using config value=…`（`log.md` 第 2 条跟着改）。环境变量只在核心起来时读一次。
6. 配置服务（`miyu-endpoint` 的 `Config`，放在核心的家底里）手里有：每份文件在哪、版本、解析好的项、问题，信任的记录，起来时的环境变量，不算项目配置的最终值 `Resolved`。8-2 起来以后就不变：造会话时照它和项目配置合一次，会话不另拿。上一次读好的项、序号、经 `tokio::sync::watch` 交给会话随 8-4（有了重读才用得上）。
7. `log.level`：运行日志装上时照 `MIYU_LOG`（没设、读不懂的是 `INFO`）。读完配置，照 `log.level` 的最终值换（`Guard::set_level`，`log.md`）：`MIYU_LOG` 设了、读得懂的就是它；读不懂的先记那一条 `WARN`。再记一条 `INFO log level level=… from=…`，`from` 是 `env`、`config`、`default`。
8. 界面语言：握手时算这个连接的 `language`。`ui.language` 的最终值（默认值、系统配置、个人设置，项目配置不能写它）不是 `auto` 的，就是它。是 `auto` 的，跟着系统（2026-10-01 项目主人定），照这一次握手报的 `locale`：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，别的、没报的是 `en`。
   - 头报的 `locale` 是系统的语言。终端里的头照 `miyu-store` 的 `locale`，用 `sys-locale`：Unix 上先看 `LC_ALL`、`LC_MESSAGES`、`LANG`（和现在命令行认的一样），macOS 上这几个都没设的看系统的首选语言，Windows 上看用户的界面语言。网页照浏览器的 `navigator.language`（随 M9）。
   - 核心自己要用语言、又没有头的时候（生成 Schema 和参考文件，第一条第 6 条），照核心这边的 `locale`，认法同上。
   - 8-2 握手时算一次，记在连接上：起来以后 `ui.language` 不变。8-4 能改了以后，连接记着头报的 `locale`，每次说话都照这时的 `ui.language` 重算，不用再握手。
   - 核心拒绝时说的话现在只有中文、英文（`protocol.md`「给人看的字」），`ja` 的照英文。配置的名字、说明、报错的话照 `human/<语言>.json`，有日文。
9. `permission.start_read_only`：`session.create` 造会话时，照这个会话实际干活的目录（`protocol.md`「工作目录太宽」以后的那个）算最终值，带上信任着的项目配置。是 `true` 的，`session.created` 的权限是「工作区，只读开着」。子会话照旧抄父会话的（`agents.md` 第一条第 1 条），不另算。
10. `config_errors`：系统配置、个人设置里现在的错误数（密钥文件的随 8-5），握手时给。`miyu ask` 照它在最前面印一行（第十条第 10 条）。

**三、项目配置**（8-2 读，8-3 记信任。G3 照原样，2026-10-01 项目主人再确认）

1. 在哪：从目录（会话实际干活的目录、`config.get` 的 `cwd`）起往上一层层找 `.miyu/config.toml`，最近的那一份就是，只认一份，不叠。
   - 找到有 `.git`（目录、文件都算）的那一层就停：那是仓库的根。
   - 到了系统的家目录就停，家目录本身不看。到了根目录也停，根目录本身也不看。
   - 落在数据根里的目录不找。
   - 每一层只看在不在，读到的那一份照第二条第 2、3 条读。
2. 信任过才算：第一次遇到先问，按仓库在哪和内容的哈希记住，内容变了再问，像 direnv 的 `direnv allow`。
   - 找到了，看 `home/<账号>/trust.toml` 里有没有这个仓库（`.miyu` 所在的那一层）的记录，记的 `version` 和这份文件现在的版本一样不一样。
   - 记着信任、版本一样：算进最终值。
   - 记着不信任、版本一样：不算，也不再提醒。
   - 没有记录，或者版本不一样（内容变了）：不算，等人答。问题里有一条 `untrusted_project`（警告），造会话、说话的回应里带 `untrusted_project`（「协议」），头照它问人。
   - 仓库挪了地方，路径对不上：当没有记录，再问一次。
   - M8 里还没有能问的界面：`miyu ask` 印一行提醒，人用 `miyu config trust` 看一眼再定（第十条第 10、11 条）。终端界面里当场问，随 M9。
3. 信任怎么记：`config.trust`，写进 `trust.toml`，照第五条的规矩写盘。
   - 一个仓库一条 `[[project]]`，三格：`path` 仓库在哪（换成真实的位置，家目录下的写成 `~/…`），`version` 答的是哪一份，`trusted` 信不信任。同一个仓库再答一次，新的盖掉旧的：原地换那一条的 `version`、`trusted`，别的字节不动（有几条的换最后一条，读的时候也是它算）。没有这个仓库的，在末尾加一条，前面空一行。
   - 核心新建这份文件时写一行开头的注释（`config/trust-header`，照这个连接的语言），下面直接接第一条。
   - 手改它也认：`config.trust` 写之前重读一遍，手改过的在新的字上记，记完换上连手改的在内的记录（8-3）；照第七条监视、当场重读随 8-4。
4. 信任了也只认收紧：清单里写了 `Project` 的项，照 `tighten` 和下面几层合出来的比（「收紧」），宽的不算，报 `not_tightening`。别的项写了不算，报 `wrong_layer`。M8 里能写的只有 `permission.start_read_only`（2026-10-01 项目主人定）。
5. 不监视项目配置本身：每一个回合开始时照会话这时的目录重读一次（第八条第 3 条），造会话时读一次。改了项目配置，下一个回合、下一个会话就照新的。内容变了，信任跟着失效，再问。
6. 只手改：`config.set` 不收 `project`，`miyu config set --project` 是参数不对。`miyu config edit --project` 打开它，存盘前照样检查（第十条第 6 条）。
7. 不带 Schema 的那一行：仓库挪到别处，相对数据根的路径就不对了（「还没有的」）。

**四、校验和报错的话**（8-2）

1. 每一条问题说成一句（`message`），照连接的语言，字在 `human/<语言>.json` 的 `said` 里（`config/*`，下面「给人看的字」）。一句由几段接成：错在哪（期望、收到），改法，现在照什么用着。
2. 键名拼错：在清单里找编辑距离最近的键（插入、删除、替换一个字，相邻两个字换位，都算 1）。距离不超过 3、也不超过这个键长度的三分之一的，才给。几个一样近的，取清单里排在前面的。`ui.langauge` 和 `ui.language` 差一次换位，给。
3. `ext.` 开头、那个扩展没装的：不找最近的，说没装这个扩展（随扩展那一步）。
4. 选项的连法照 `cli/main.md`「参数写错时」：两个用「或」，三个以上前面用顿号（英文逗号），最后一个前面用「或」（`or`）。
5. 「改法」给一个能照抄的例子：选项的取默认值，开关的取默认值的另一个，写成 `键 = 值`。选项写成了别的类型（`ui.language = 3`）也照选项说：列出能写的几个，原因码还是 `wrong_type`。一组键写成了值（`ui = "zh"`）只说期望一张表，不给改法、不说照什么用。
6. 不认识的键只警告、不改文件：新版本写进去的配置，旧版本照样读得进来（`07-存储.md` S7）。
7. 几段接成一句（「给人看的字」最后一段）：一段不是以句末的标点结尾的，补上句号；段和段接起来。句号、句末的标点、段和段之间空不空格都是字（`config/sentence`、`config/stops`、`config/then`），代码里不写标点：中文「是不是想写 ui.language？」后面不再补「。」，英文段和段之间空一格。

**五、写盘**（8-3，G5）

1. 只有核心写配置文件（G4）。一个配置服务，所有的改、重读排着队一件件办。
2. 改几项（`changes`）：
   1. 先把文件现在的字节读一遍。和上一次读的不一样（手改过）：先照第七条第 3 条当手改重读（推送、记日志随 8-4），再在新的字上改（G5 第 4 条）。8-3 的做法是每一次都重读、换上，一样的换上也还是一样。
   2. 查 `expect`（第 1 条的新字上），对不上拒绝。
   3. 只改这几项：照 `toml_edit` 读的时候记下的位置，只换那一段字（不把整份读成可改的文档再写回去：写回去的样子由它定，前后字节比不住）。已经有的，原地换值，行尾注释留着。没有的，放进 `[<键的前几段>]` 那张表，有这张表就接在它最后一个键那一行后面（表里还没有键的接在表头那一行后面），没有就在文件末尾新开，前面空一行（已经空着一行的不再空，空文件不空）；只因为子表才有、没有表头的，当没有。那一组写成点号连着的键（`ui.langauge = …`）的，照样写成点号连着的；写成行内表（`ui = { … }`）的，接在行内表里。删掉的，连同它那一行删掉，表空了、里面也没有注释的，表头一起删；那张表在文件末尾的，连表头前面那一行空行一起删（新建再删回到原样）。行内表里的删掉这一格和它旁边的逗号。那一组写成了别的东西（`ui = "zh"`、`[[ui]]`、这一项本身是一张表）、改完读不懂的，放不进去：`config_file_broken`。
   4. 别的字节一个不动：注释、空行、顺序、不认识的键（S9）。
   5. 值的写法：字用双引号，照 TOML 转义。列表、表写成一行。换行照这份文件原来的（第一个换行是 `\r\n` 的用 `\r\n`，新文件用 `\n`）。
   6. 配置文件原来没有的，新建，第一行写 `#:schema` 和 Schema 的相对路径（系统配置是 `../state/config/config.schema.json`，个人设置是 `../../state/config/settings.schema.json`），空一行，再写这几项。已经有的文件不加这一行（G10）。
3. `input`（人敲的字）照类型读：开关只认 `true`、`false`。整数、小数照十进制。选项、文字、时长、路径、网址、名字、引用照原样，两头带着双引号、是一个 TOML 字符串的，去掉引号再用。列表、表、密钥照一行 TOML 读。读不成的是 `wrong_type`。
4. 顺着链接写：文件是符号链接的，一层层找到它指向的本体（相对的照链接所在的目录接，最多 40 层，绕圈的报错），写本体，链接本身不动。指向的地方还没有文件的，在那里新建。
5. 先写临时文件，再替换：临时文件建在本体所在的目录里，名字 `.<文件名>.<进程号>-<计数>.tmp`，只许新建。写完、同步，Unix 上带上原文件的权限位（新文件照系统默认），再改名盖上本体，再同步目录（`store.md` 第 4 条）。写到一半断电，磁盘上还是原来那一份。
6. 替换之前再读一次本体：和第 2 条第 1 款读的不一样（这一瞬间有人手改），放弃这一次，照第 2 条从头来，最多三次，还不行的回 `config_conflict`。这一条缩小了窗口，关不死：编辑器不加锁，同一瞬间的手改仍可能被盖掉。
7. Windows 上本体正被别的程序开着、改名失败的，歇 20 毫秒再试，最多 5 次。
8. 写成了，记下新的字节当「上一次读的」：监视随后看到的变动字节一样，不当手改（第七条第 3 条）。
9. 整份换（`text`）：版本对得上、没有错误的，照第 5 到 8 条整份写进去。
10. 写不成（没有权限、磁盘满了、目录是只读的）：`internal_error`，什么都没变，记一条 `WARN config not written file=… error=…`。
11. 两个头同时改同一项：带 `expect` 的后到的被拒，头照 `data.current` 告诉人现在的值。不带的后写的算（`miyu config set` 不带）。
12. `trust.toml`、密钥文件照同样的规矩写（第三条第 3 条、第九条第 4 条）。

**六、留痕**（8-3，G5 第 6 条）

1. 改动落了盘，照「系统日志、账号日志」的写法追加一条：系统配置的进系统日志，个人设置的进账号日志。一次 `config.set` 一条，`changes` 里是这一层真变了的那几项。`config.trust` 记一条 `trust.changed`，进账号日志。
2. 日志的写法（`miyu-store` 的 `journal.rs`）：打开时照会话日志的规矩截掉最后那半行（`store.md` 第 6 条），读最后一行拿 `seq`。追加一行、`sync_data`。每追加一条都重新打开一次：改配置是很少的事，不用在内存里留一个开着的文件，手改过的也认得（8-3）。最后一行完整、却读不懂的（手改坏了）不往后写，当写不进去。
3. 先写配置，后写日志：配置文件是真相，日志是留痕。日志写不进去（坏了、磁盘满了），记一条 `WARN journal not written file=… error=…`，配置照改、照推送、照回应。
4. 手改被看到的（8-4），照样记一条，`via` 是 `file`。核心没在跑时的手改看不到，不记。

**七、监视**（8-4）

1. 核心读完配置以后开始监视，用 `notify`：Linux 上是 inotify，macOS 上是 FSEvents，Windows 上是 ReadDirectoryChangesW。
2. 看的是目录，不是文件，不递归：`system/`、`home/admin/`。配置文件、密钥文件是链接的，另外看它本体所在的目录。很多编辑器存盘是先写新文件再改名，看文件会跟丢（`14-配置.md` 第五节）。
   - 路径都先换成真实的位置再比：macOS 的 FSEvents 报的是真实的路径，`/var` 这类是链接，照原样比会认不出来。
3. 一个目录里有变动，看是不是 `config.toml`、`settings.toml`、`secrets.toml`、`trust.toml` 这几个名字，别的不理。200 毫秒里没有新的变动了，再重读那一份。
   - 字节和上一次读的一样：什么都不做。核心自己写的也走这里，认得出来。
   - 不一样：照第二条读、解析、合并。这一层变了的项、问题有了变化的，推 `config.changed`（`via` 是 `file`），记日志（第六条第 4 条），换上新的最终值（第八条）。
   - 文件被删了：这一层变成空的，照样推、记。
4. 监视起不来（Linux 上 inotify 的名额用完了这类），记一条 `WARN config watch unavailable error=…`，退回每 2 秒看一次这几份文件的修改时间和长短（`notify` 的轮询）。
5. 网络文件系统上的变动可能收不到，手改以后要等核心重启（「还没有的」）。

**八、生效**（8-4，G7）

1. 最终值变了，配置服务把新的 `Resolved` 交给 `watch`，照每一项的 `applies`：
   - `now`：用它的地方当场换。`log.level` 经运行日志的重载把手换级别（`MIYU_LOG` 设了的不换，`log.md`）。`ui.language` 下一句话就照新的说（第二条第 8 条），头收到推送自己换。三份生成的文件照新语言重写（第一条第 12 条）。
   - `new_session`：造会话时读，已经开着的会话不变。
   - `next_turn`：第 3 条。
   - `restart`：M8 没有。有了以后，推送里写着 `restart`，界面上写「重启核心后生效」。
2. 正在进行的回合不受影响。
3. 回合开始时冻结一份快照：会话 actor 在记下 `turn.started` 的那一刻，从 `watch` 取当前的 `Resolved`，再照会话这时的目录读一次项目配置、合上，得到这一轮的配置，这一轮的每一次请求（出错再来、压缩的摘要请求也算）都照它。回合之间改的，下一轮才用上。M8 里用它的是供应商、模型那一块（8-6 起，`models.md`）。
4. 进策略快照的项（例如以后压缩的几个数）变了：下一个回合开始时换一份快照，记 `session.policy_changed`（`02-内核.md` K3）。M8 没有这样的项，这条路随第一项进快照的配置做（「还没有的」）。

**九、密钥**（8-5，G9）

1. 文件 `system/secrets.toml`：一行一个，`名字 = "值"`，平铺，没有表。名字照名字的写法（小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符）。核心新建时写一行开头的注释（`config/secrets-header`，照管理员的界面语言）。
2. 权限：Unix 上新建、替换时都是 0600，临时文件建的时候就是 0600。读的时候组、别人能读的，照用，记一条 `WARN secrets readable by others file=…`，不去改它（`miyu doctor` 以后报）。Windows 上照数据根继承的访问控制：用户目录本来只有本人、系统、管理员组能进。
3. 核心起来时读一次，之后照第七条监视它：手改的重读，记 `secret.changed`（`via` 是 `file`），只记变了的名字。读不进来的，照配置文件的规矩报问题（不带 `got`），照上一次读好的用。这些问题和配置文件的一起出现在 `config.get` 的 `problems`、握手的 `config_errors`、`miyu config check` 里。
4. 写：`secret.set`、`secret.delete` 照第五条写盘（只改那一行，注释留着），落了盘记 `secret.changed`，再回应。
5. 配置里引用密钥（类型 `secret`）：
   - `{ secret = "<名字>" }`：照名字到密钥文件里取。M8 只有系统的密钥文件。
   - `{ env = "<变量>" }`：照核心起来时的环境取。拉起的核心的环境是拉起它的那个头的（`ipc.md`），之后在别的终端里设的，核心看不到，要等它重启。
   - 引用的密钥没设：`unknown_secret`，警告：可以先写配置，后设密钥。环境变量没设：`env_not_set`，警告。
6. 取出来的密钥是一个单独的类型：`Debug` 只印 `Secret(…)`，没有 `Display`，不能序列化。它不进日志、事件、blob、策略快照、协议的回应、运行日志、报错的话（`07-存储.md` 第九节）。`secret.set` 的参数不进运行日志（端点的 `DEBUG request` 那一行本来只记方法名）。
7. 密钥变了，下一个回合开始时生效，和供应商的配置一样（第八条第 3 条，`models.md`）。
8. `secret.list` 的 `used_by`：照系统配置、个人设置的最终值，找类型是 `secret`、写的是 `{ secret = 这个名字 }` 的项。

**十、命令行**（8-2、8-3）

1. **连核心**：照 `miyu ask` 连（`cli/ask.md` 第 2 步）。读、改配置都用不着模型，可 8-6 以前核心只在起来时读 `DEEPSEEK_API_KEY`：没设 key、核心又没在跑的，照 `miyu undo` 不拉起，说那一句，退出码 5（`cli/undo.md`）。8-6 以后 key 来自配置，改成一律拉起。
2. **握手**：照 `miyu ask`，`caps.input` 是 `false`。之后给人看的字照回应的 `language`，没握手之前照环境（`cli/main.md`「界面语言」）。命令行自己的字只有中文、英文，`ja` 的照英文（「还没有的」）。
3. **`get [键…]`**：`config.get`，带上当前目录当 `cwd`。只写一个键：标准输出上只印值，字不带引号，别的照 TOML 的写法（`true`、`3`、`["a", "b"]`）。写了几个、一个都没写：一行一个 `键 = 值`，TOML 的写法，照键名排。`--format json`：`config.get` 回应的 `items` 原样。
4. **`set <键> <值>`**：`config.set`，`changes` 是 `[{key, input}]`，`layer` 照 `--system` 是 `system`，不写是 `personal`。不带 `expect`。成了：标准错误上印一行灰字（「样子」）：回应里这一项的来源是这一层的，说写进了哪一层、什么时候生效；来源是上面一层（或者环境变量）的，说那一层写着什么、用的还是它。回应里没有这一项（本来就是）：`config.get` 带 `all` 找这一层写的值，说本来就是。值有空格的照 shell 的规矩加引号。`--project` 连核心以前就说项目配置只能手改，退出码 2。
5. **`unset <键>`**：`config.set`，`changes` 是 `[{key, unset: true}]`。成了：说从哪一层删掉了、现在是哪一层的什么。这一层本来就没写（回应里没有这一项）：说一句本来就没写，退出码 0。
6. **`edit`**（像 visudo，G4）：
   1. 标准输入、标准错误不是终端：说「miyu config edit 要在终端里用」，退出码 2。连核心以前就查（8-3）。
   2. `config.get` 拿到这一层的文件在哪（`files`）和版本，读它现在的字。没有文件的是空的。
   3. 在原文件旁边（同一个目录，Schema 的相对路径照样对）写一份临时副本 `.<文件名>.edit-<随机>.toml`（8 位十六进制），Unix 上 0600。目录还没有的建上，最后没存的，这个新建的目录空了删掉。
   4. 用 `VISUAL`，没有用 `EDITOR`，都没有的 Unix 上是 `vi`、Windows 上是 `notepad`。Unix 上经 `sh -c` 跑，Windows 上经 `cmd /c`：带参数的（`code --wait`）也行，和 git 一样。编辑器退出码不是 0 的：说一句，副本删掉，退出码 1。
   5. 字没变：说「没改」，删掉副本，退出码 0。
   6. 变了：`config.check`。有错误：印出每一条，问「回车接着改，输入 q 放弃」（visudo 的做法，`14-配置.md` 第九节，2026-10-01 主会话定）。回车再打开编辑器，改的还在，存了再查。q 删掉副本，说放弃了，退出码 1。只有警告的：印出来，照样存。
   7. 存：`config.set`，带 `text` 和第 2 步的版本。`config_conflict`：说「你编辑的时候文件被改过了」，副本留着、印出它在哪，退出码 1。成了：删掉副本，印一行灰字，照回应里改了的几项什么时候生效（照键名的先后，一样的只说一次，几种接在一起）；一项都没变的（只动了注释）只说存好了。
   8. `--project`：项目配置核心不写，这一步改成由命令行自己照第五条第 4 到 7 条写回（仓库里的文件，写它的是人，命令行替人存盘；用的是 `miyu-store` 的同一个 `config_file::write`）。没找到项目配置的，在仓库的根（有 `.git` 的那一层，没有就是当前目录）新建 `.miyu/config.toml`。成了只说存好了：项目配置什么时候生效照信任（改了内容要重新信任）。
7. **`check [文件]`**：
   - 不写文件：照 `config.get` 的 `files` 读系统配置、个人设置、当前目录的项目配置现在的字，一份份 `config.check`。读的是磁盘上现在的字，不是核心手里的那份：8-4 以前，核心起来以后的手改它还没重读。
   - 写了文件：照 `--system`、`--project` 当那一层查，都不写的当个人设置。
   - 标准输出上一条一行（「样子」），最后一行合计。一个问题都没有的，印一行「没有问题」。`--format json`：`{"problems":[…]}`。
   - 有错误退出码 1，只有警告、没有问题的 0。
8. **`explain <键>`**：`config.get`，带 `cwd`、`all: true`，再 `config.schema` 拿名字和说明。印第一行是名字、键、说明、什么时候生效。下面每一层一行，从上往下（「样子」）。`--format json`：`config.get` 那一项原样，多 `name`、`description`。
9. **`path`**：印出这一层的文件在哪，绝对路径，一行，文件还没有也印（给 `$EDITOR $(miyu config path)` 这样用）。`--project` 没找到的，印仓库的根下的 `.miyu/config.toml`，标准错误上说一句还没有这个文件。不写 `--system`、`--project` 的是个人设置。
10. **`miyu ask` 起头那几行**：握手回应里有 `config_errors` 的，标准错误上最先印一行灰字，在沙盒用不了那一句前面（8-2）；`--format json` 的不印。造会话、说话的回应里有 `untrusted_project` 的，接着印一行灰字，提醒它还没信任、这次没用它、用 `miyu config trust` 看一眼再定（8-3，和 `miyu config trust` 一起做，2026-10-01 主会话定）：新开的会话照造会话的回应说，接着说的会话照说话的回应说，一次 `miyu ask` 只说一次，`--format json` 的不印。写法见「给人看的字」。
11. **`trust`**：
    1. `config.get`，带当前目录当 `cwd`、`all: true`。没有 `files.project`：说「这里没有项目配置」，退出码 1。
    2. 标准输出上印出这份文件在哪、它会改哪几项（`layers` 里项目那一层、信任了就算的：没有 `problem` 或者是 `untrusted_project` 的；`键 = 值` 和名字，`键 = 值` 那一列照显示的宽度补齐），有问题的（不能写在这一层、写宽了、写错了）照「样子」报错一行印在后面，`untrusted_project` 那一条不印。
    3. 已经信任着这一份、又没写 `--no`：说「已经信任过这一份了」，退出码 0。
    4. 写了 `--yes`、`--no` 的，照它 `config.trust`，不问。都没写：标准输入、标准错误不是终端的，说「要在终端里回答，或者写 --yes、--no」，退出码 2。是终端的，问「项目配置只能让限制更严。信任这一份吗？[y/N]」：`y`、`yes`（不分大小写）是信任，别的、直接回车是不信任，都记下。
    5. `config.trust` 带第 1 步读到的 `version`。`config_conflict`：说「你看的时候它又被改过了，再跑一次」，退出码 1。成了：印一行灰字，退出码 0。

**十一、`miyu login`、`miyu logout`**（8-5，2026-10-01 项目主人定）

1. 连核心、握手：照第十条第 1、2 条。
2. `login` 写了名字：照名字的写法查，不合的是参数不对，退出码 2。
3. `login` 没写名字：标准输入、标准错误都是终端的，`secret.list`，列出配置里用到的每一个密钥：编号、名字、谁在用（`used_by`）、设没设。输入编号选一个，也可以直接敲一个新名字。一个都没有：说「配置里还没有用到密钥的供应商：写 miyu login <名字>」，退出码 2。不是终端的：参数不对，退出码 2。8-11 以后这里还列出目录里的供应商，选了连配置一起写（`models.md`）。
4. 读 key：标准输入是终端的，在标准错误上问「粘贴 <名字> 的 key（不显示）：」，关掉回显读一行（`rpassword`）。标准输入是管道的，整份读进来（`echo "$KEY" | miyu login deepseek`）。去掉前后空白是空的：说「没收到 key」，退出码 1。
5. `secret.set`：成了，标准错误上印一行灰字，照回应的 `replaced` 说「存好了」或者「换掉了」。key 不印，也不印它的前几位。
6. `login --list`：`secret.list`，标准输出上一个一行：名字、已设置或未设置、谁在用。设了的在前，配置里用到、还没设的在后，灰字。一个都没有：印「还没有设过 key」。`--format json`：回应原样。
7. `logout` 写了名字：`secret.delete`，成了印一行灰字「删掉了」。`unknown_secret`：说「<名字> 没有设过 key」，退出码 1。没写名字：终端里从设了的里面选（照第 3 条），不是终端的，参数不对。
8. 退出码：0 成了。1 核心拒绝了、没收到 key、连不上核心。2 参数不对。5 同第十条第 1 条（8-6 以前）。


### 样子

**报错一条一行**（`miyu config check`、`edit`、`trust` 印的，一行的开头是 `路径:行:列`，2026-10-01 主会话定：编辑器、很多终端能照它点过去）：`<文件>:<行>:<列> <级别>：<那一句>`。文件照家目录写成 `~/…`。「错误」红、「警告」黄（`ESC[33m`），别的原色。上色的规矩照 `cli/ask.md`「上色」。

例子（中文）：

```text
$ miyu config check
~/.miyu/home/admin/settings.toml:7:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。
~/.miyu/system/config.toml:2:9 错误：log.level 只能是 error、warn、info、debug、trace 或 off，写的是 "verbose"。改成其中一个，例如 log.level = "info"。这一项先照 "info" 用着（默认值）。
~/src/app/.miyu/config.toml:3:19 错误：项目配置只能让限制更严。permission.start_read_only 现在是 true，这里写的 false 更宽，不算。
2 处错误，1 处警告
```

例子（英文）：

```text
$ miyu config check
~/.miyu/home/admin/settings.toml:7:1 warning: There is no ui.langauge. Did you mean ui.language? The line is ignored and kept as it is.
~/.miyu/system/config.toml:2:9 error: log.level must be error, warn, info, debug, trace or off, not "verbose". Write one of them, e.g. log.level = "info". Using "info" (the default) for now.
~/src/app/.miyu/config.toml:3:19 error: A project config can only make limits stricter. permission.start_read_only is true, and false here is looser, so it does not count.
2 errors, 1 warning
```

另外几种（中文、英文各一句，照原样）：

```text
~/.miyu/home/admin/settings.toml:4:12 错误：TOML 写法不对：invalid basic string。这份文件先照上一次读进来的用着。
~/.miyu/home/admin/settings.toml:4:12 error: Not valid TOML: invalid basic string. Using what was read from this file last time.
~/.miyu/home/admin/settings.toml:9:9 错误：log.level 只能写在系统配置里，写在个人设置里不算。挪到系统配置里去。
~/.miyu/home/admin/settings.toml:9:9 error: log.level belongs in the system config. It does not count in personal settings. Move it to the system config.
~/.miyu/home/admin/settings.toml:5:19 错误：permission.start_read_only 要写 true 或 false，写的是 "yes"。改成 permission.start_read_only = true。这一项先照 false 用着（默认值）。
~/.miyu/home/admin/settings.toml:5:19 error: permission.start_read_only needs true or false, not "yes". Write permission.start_read_only = true. Using false (the default) for now.
```

`TOML 写法不对：` 后面是 `toml_edit` 报的为什么，英文原话，不翻。

**`miyu config explain`**：

```text
$ miyu config explain ui.language
界面语言（ui.language）：终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。当场生效。
  "zh"    个人设置  ~/.miyu/home/admin/settings.toml:3  ← 生效
  "en"    系统配置  ~/.miyu/system/config.toml:5
  "auto"  默认值
```

```text
$ miyu config explain ui.language
Interface language (ui.language): The language terminals, the web page and the command line use for you. auto follows the terminal or browser. Takes effect at once.
  "zh"    personal settings  ~/.miyu/home/admin/settings.toml:3  ← in effect
  "en"    system config      ~/.miyu/system/config.toml:5
  "auto"  default
```

- 值照 TOML 的写法。几列照显示的宽度对齐（中文算两列）。
- 生效的那一行原色，别的灰。项目配置里写了、不算的那一行，末尾写 `← 不算：<原因>`，红。
- 环境变量压着的：最上面一行是 `"debug"  环境变量 MIYU_LOG  ← 生效，只管这一次启动`。

**`miyu config get`**：

```text
$ miyu config get ui.language
zh
$ miyu config get
log.level = "info"
permission.start_read_only = false
tui.startup = "new"
ui.language = "zh"
```

**`miyu config set`、`unset` 印的那一行**（标准错误，灰）：

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 改了 | `· ui.language = "zh" 写进了个人设置，当场生效` | `· ui.language = "zh" saved to personal settings, takes effect at once` |
| 改了，上面一层压着 | `· ui.language = "en" 写进了系统配置，个人设置里写着 "zh"，用的还是 "zh"` | `· ui.language = "en" saved to the system config, but personal settings say "zh", so "zh" stays in use` |
| 本来就是 | `· 本来就是 "zh"，没改` | `· Already "zh", nothing changed` |
| 删掉了 | `· 从个人设置里删掉了 ui.language，现在是 "en"（系统配置）` | `· Removed ui.language from personal settings. It is now "en" (system config)` |
| 本来就没写 | `· 个人设置里本来就没写 ui.language` | `· Personal settings did not have ui.language` |

「当场生效」按 `applies` 换：`当场生效`、`以后开的会话生效`、`下次打开界面时生效`、`下一轮生效`、`重启核心后生效`（`takes effect at once`、`applies to sessions opened from now on`、`takes effect the next time the interface opens`、`takes effect next turn`、`takes effect after the core restarts`）。M8 用得上前三种（`head_start` 8-3 加）。

- 上面一层压着的，那一层照句子里的叫法：个人设置、系统配置、环境变量（`personal settings say`、`the system config says`、`the environment says`）。
- 删掉了以后括号里是现在那个值从哪一层来：默认值、系统配置、个人设置（`default`、`system config`、`personal settings`），和 `explain` 的层名一样。
- 本来就没写的，英文句首大写：`· The system config did not have log.level`。

**`miyu config edit` 存了有错时**：

```text
$ miyu config edit
~/.miyu/home/admin/settings.toml:3:13 错误：ui.language 只能是 auto、zh、en 或 ja，写的是 "cn"。改成其中一个，例如 ui.language = "auto"。这一项先照 "auto" 用着（默认值）。
有 1 处错误，还没存。回车接着改，输入 q 放弃：
```

- 一项的问题照 `config.check` 带「这一项先照…用着」（8-2 定的，8-3 照它改了这个例子）。
- 问的那一句后面不换行，等人敲。英文的末尾冒号后面空一格。
- 放弃：删掉副本，说「放弃了，文件没动」，退出码 1。读到头（Ctrl+D、管道关了）也算放弃。
- 存好了：`· 存好了，当场生效`；改了几种生效时机不一样的项，接在一起（`· 存好了，当场生效、以后开的会话生效`）；只动了注释的 `· 存好了`。

**`miyu config trust`**：

```text
$ miyu config trust
~/src/app/.miyu/config.toml 会改这几项：
  permission.start_read_only = true  新会话开局只读
项目配置只能让限制更严。信任这一份吗？[y/N] y
· 信任了。内容变了会再问
```

```text
$ miyu config trust
~/src/app/.miyu/config.toml would set:
  permission.start_read_only = true  Start new sessions read-only
A project config can only make limits stricter. Trust this one? [y/N] y
· Trusted. You will be asked again if it changes
```

**`miyu ask` 起头那两行**（标准错误，灰，只在有的时候印）：

```text
· 配置里有 1 处错误：miyu config check 看是哪里
· 这里的项目配置 ~/src/app/.miyu/config.toml 还没信任，这次没用它：miyu config trust 看一眼再定
```

**`miyu login`、`miyu logout`**：

```text
$ miyu login
配置里用到的密钥：
  1  deepseek    providers.deepseek.keys  已设置
  2  bigmodel-2  providers.bigmodel.keys  未设置
选一个编号，或者敲一个新名字：2
粘贴 bigmodel-2 的 key（不显示）：
· bigmodel-2 的 key 存好了
$ miyu login --list
deepseek    已设置  providers.deepseek.keys
bigmodel-2  已设置  providers.bigmodel.keys
$ miyu logout bigmodel-2
· 删掉了 bigmodel-2 的 key
```

- 几列照显示的宽度对齐。key 从头到尾不出现在屏幕上。

**生成的文件**：样本在 `docs/designs/samples/config/`，中文、英文各三份（`config.schema.zh.json`、`settings.schema.zh.json`、`reference.zh.toml` 和英文的三份），8-1 施工时照生成的定稿，测试逐字节比（「守着它的」）。下面两份是中文的，门禁照样本块逐字节比。

样本 `docs/designs/samples/config/reference.zh.toml`（参考文件，中文）：

```toml
# Miyu 的全部配置项和默认值。这份是生成的，改它没有用。
# 系统配置写在 system/config.toml，个人设置写在 home/<账号>/settings.toml。

[log]
# 运行日志的级别：运行日志记到哪一级。排查问题时调成 debug。设了环境变量 MIYU_LOG 的，那一次启动照它。
# 能写：error、warn、info、debug、trace 或 off。只能写在系统配置里。当场生效。
level = "info"

[permission]
# 新会话开局只读：打开以后，新会话一开始就是只读。她只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。
# 能写：true 或 false。只能写在系统配置、个人设置或项目配置里。以后开的会话生效。
start_read_only = false

[tui]
# 启动时打开：终端界面启动时开一个新会话，还是接着最近的那一个。
# 能写：new 或 recent。只能写在系统配置或个人设置里。下次打开界面时生效。
startup = "new"

[ui]
# 界面语言：终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。
# 能写：auto、zh、en 或 ja。只能写在系统配置或个人设置里。当场生效。
language = "auto"
```

样本 `docs/designs/samples/config/settings.schema.zh.json`（个人设置的 JSON Schema，中文：能放进个人设置的 `permission.start_read_only`、`tui.startup`、`ui.language`）：

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "properties": {
    "permission": {
      "properties": {
        "start_read_only": {
          "default": false,
          "description": "打开以后，新会话一开始就是只读。她只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。能写：true 或 false。只能写在系统配置、个人设置或项目配置里。以后开的会话生效。",
          "title": "新会话开局只读",
          "type": "boolean"
        }
      },
      "type": "object"
    },
    "tui": {
      "properties": {
        "startup": {
          "default": "new",
          "description": "终端界面启动时开一个新会话，还是接着最近的那一个。能写：new 或 recent。只能写在系统配置或个人设置里。下次打开界面时生效。",
          "enum": [
            "new",
            "recent"
          ],
          "title": "启动时打开",
          "type": "string"
        }
      },
      "type": "object"
    },
    "ui": {
      "properties": {
        "language": {
          "default": "auto",
          "description": "终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。能写：auto、zh、en 或 ja。只能写在系统配置或个人设置里。当场生效。",
          "enum": [
            "auto",
            "zh",
            "en",
            "ja"
          ],
          "title": "界面语言",
          "type": "string"
        }
      },
      "type": "object"
    }
  },
  "type": "object"
}
```

- 英文的参考文件，`[log]` 那一项的两行注释是 `# Runtime log level: How much the runtime log records. …` 和 `# Allowed: error, warn, info, debug, trace or off. Only in the system config. Takes effect at once.`。
- 系统配置的 Schema 多一张 `log` 表（`log.level` 只能放在系统配置里）。

**给模型看的字**：没有。配置这一块不往请求里加字。开局只读的会话，第一轮的权限那一块事实照原来的模板写（`facts/permission.txt`），不加新的。

### 出错

协议的拒绝见「新的原因码」和下面「给人看的字」。核心这边：

| 什么时候 | 怎么办 |
|---|---|
| 配置文件读不进来、有错 | 照第二条第 4 条用，起得来。问题留着，`config.get`、推送、握手的 `config_errors`、`miyu config check` 都看得到 |
| 写不成 | `internal_error`，什么都没变 |
| 日志写不进去 | 配置照改，记 `WARN` |
| 监视起不来 | 退回轮询，记 `WARN` |
| 生成的文件写不成、要用的字缺了或者读不懂 | 那一份不写，记 `WARN`，照样起来（第一条第 8 条） |

`miyu config` 的退出码（`miyu login`、`logout` 的见第十一条第 8 条）：

| 码 | 什么时候 |
|---|---|
| 0 | 成了。`check` 没有错误。`edit` 没改。`unset` 本来就没写。`trust` 记下了、本来就信任着 |
| 1 | 核心拒绝了（不认识的键、值不对、冲突、文件读不进来）。`check` 有错误。`edit` 放弃了、编辑器出错、冲突。`trust` 这里没有项目配置、冲突。连不上核心、数据根的错 |
| 2 | 参数不对。`edit` 不在终端里。`trust` 不在终端里又没写 `--yes`、`--no`。`set --project` |
| 5 | 8-6 以前：核心没在跑、又没设 `DEEPSEEK_API_KEY`（第十条第 1 条） |

运行日志（目标 `miyu::config`，一律英文）：

| 级别 | 这件事 | 哪一步 |
|---|---|---|
| `WARN` | `config problems file=… errors=… warnings=…` | 8-2 |
| `WARN` | `trust not read file=home/<账号>/trust.toml error=…` | 8-2 |
| `INFO` | `log level level=… from=env\|config\|default` | 8-2 |
| `WARN` | `MIYU_LOG not understood, using config value=…` | 8-2 |
| `WARN` | `config schema not written file=state/config/<文件名> error=…`：一份一条 | 8-1 |
| `INFO` | `config changed layer=… via=… keys=…`（只有键名，不带值） | 8-3、8-4 |
| `WARN` | `config not written file=… error=…` | 8-3 |
| `WARN` | `journal not written file=… error=…` | 8-3 |
| `WARN` | `config watch unavailable error=…` | 8-4 |
| `WARN` | `secrets readable by others file=…` | 8-5 |
| `INFO` | `secret changed name=… action=… via=…` | 8-5 |
| `INFO` | `project trust path=… trusted=…` | 8-3 |

### 给人看的字

**配置项的名字、说明、选项名**（`core/human/<语言>.json` 的 `config.items`，一项一格：`name`、`description`、`options`（选项到名字）；8-1 有前两项，`permission.start_read_only` 随 8-2，`tui.startup` 随 8-3）：

| 键 | 中文 | 英文 | 日文 |
|---|---|---|---|
| `ui.language` 名字 | 界面语言 | Interface language | 表示言語 |
| 说明 | 终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。 | The language terminals, the web page and the command line use for you. auto follows the terminal or browser. | 端末、ウェブ、コマンドラインで表示する言語です。auto は端末やブラウザの言語に合わせます。 |
| 选项 | `auto` 跟随系统、`zh` 中文、`en` English、`ja` 日本語 | `auto` Follow the system，别的同左 | `auto` システムに合わせる，别的同左 |
| `log.level` 名字 | 运行日志的级别 | Runtime log level | 実行ログのレベル |
| 说明 | 运行日志记到哪一级。排查问题时调成 debug。设了环境变量 MIYU_LOG 的，那一次启动照它。 | How much the runtime log records. Set debug when chasing a problem. MIYU_LOG, when set, wins for that launch. | 実行ログにどこまで記録するかです。問題を調べるときは debug にします。環境変数 MIYU_LOG があれば、その起動ではそちらが優先されます。 |
| 选项 | `error` 只记错误、`warn` 错误和警告、`info` 来龙去脉、`debug` 更细，排查用、`trace` 全记、`off` 不记 | Errors only、Errors and warnings、What happens、More detail, for debugging、Everything、Nothing | エラーのみ、エラーと警告、経過も記録、詳細（調査用）、すべて、記録しない |
| `permission.start_read_only` 名字 | 新会话开局只读 | Start new sessions read-only | 新しいセッションを読み取り専用で始める |
| 说明 | 打开以后，新会话一开始就是只读。她只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。 | When on, new sessions begin read-only. She can look around and plan, and you turn read-only off when files should change. A project config can only turn it on. | オンにすると、新しいセッションは読み取り専用で始まります。調査と計画だけを行い、ファイルを変更するときに読み取り専用をオフにします。プロジェクト設定ではオンにすることしかできません。 |
| `tui.startup` 名字（8-3） | 启动时打开 | On start, open | 起動時に開く |
| 说明 | 终端界面启动时开一个新会话，还是接着最近的那一个。 | Whether the terminal interface starts a new session or picks up the most recent one. | 端末画面を起動したときに、新しいセッションを始めるか、最近のセッションを続けるかです。 |
| 选项 | `new` 新会话、`recent` 最近的会话 | A new session、The most recent session | 新しいセッション、最近のセッション |

页和组（`config.pages`、`config.groups`，编号到名字；资源里只放清单用到的，`permissions`、`sessions` 随 8-2 加，`interface`、`tui` 随 8-3 加）：

| 编号 | 中文 | 英文 | 日文 |
|---|---|---|---|
| 页 `general` | 通用 | General | 一般 |
| 页 `interface` | 界面 | Interface | 画面 |
| 页 `permissions` | 权限 | Permissions | 権限 |
| 页 `advanced` | 高级 | Advanced | 詳細 |
| 组 `display`（`general`） | 显示 | Display | 表示 |
| 组 `tui`（`interface`） | 终端界面 | Terminal interface | 端末画面 |
| 组 `sessions`（`permissions`） | 会话 | Sessions | セッション |
| 组 `log`（`advanced`） | 运行日志 | Runtime log | 実行ログ |

**生成的文件要的几句**（`core/human/<语言>.json` 的 `said`，编号前面加 `core/`，8-1）。日文照中文写，用词照终端界面的日文（施工 4-5 补），句子里用全角的「：」：

| 编号 | 字段 | 中文 | 英文 | 日文 |
|---|---|---|---|---|
| `config/reference-header` | | Miyu 的全部配置项和默认值。这份是生成的，改它没有用。 | Every Miyu setting and its default. This file is generated. Editing it has no effect. | Miyu のすべての設定項目と既定値です。このファイルは生成されたもので、編集しても効果はありません。 |
| `config/reference-where` | | 系统配置写在 system/config.toml，个人设置写在 home/<账号>/settings.toml。 | Write the system config in system/config.toml and personal settings in home/<account>/settings.toml. | システム設定は system/config.toml に、個人設定は home/<アカウント>/settings.toml に書きます。 |
| `config/reference-item` | `name`、`description` | {name}：{description} | {name}: {description} | {name}：{description} |
| `config/facts` | `values`、`layers`、`applies` | 能写：{values}。只能写在{layers}里。{applies}。 | Allowed: {values}. Only in {layers}. {applies}. | 書ける値：{values}。{layers}にだけ書けます。{applies}。 |
| `config/schema-description` | `description`、`facts` | {description}{facts} | {description} {facts} | {description}{facts} |
| `config/or-values` | `rest`、`last` | {rest} 或 {last} | {rest} or {last} | {rest} または {last} |
| `config/or` | `rest`、`last` | {rest}或{last} | {rest} or {last} | {rest}または{last} |
| `config/list` | `rest`、`next` | {rest}、{next} | {rest}, {next} | {rest}、{next} |
| `config/layer/system`、`personal` | | 系统配置、个人设置 | the system config、personal settings | システム設定、個人設定 |
| `config/applies/now` | | 当场生效 | Takes effect at once | すぐに反映されます |

- 连词、标点、句末的「。」也是字，不写在代码里：`config/or-values` 连写成代码的值，中文「或」两边空一格；`config/or` 连层的名字这类字，不空格（「怎么走」第一条第 11 条）。
- 说明（`description`）自己带句末的标点；英文里说明和后面那几句之间空一格，中文、日文不空。

**报错的话**（`core/human/<语言>.json` 的 `said`，编号前面加 `core/`，8-2 起）。日文那一份施工时照中文写，用词照终端界面的日文（施工 4-5 补）：

| 编号 | 字段 | 中文 | 英文 |
|---|---|---|---|
| `config/unreadable` | `why` | 读不了这份文件：{why} | Cannot read this file: {why} |
| `config/too-big` | | 这份文件超过 1 MiB，不读 | The file is over 1 MiB and is not read |
| `config/not-utf8` | | 这份文件不是 UTF-8 | The file is not UTF-8 |
| `config/syntax` | `why` | TOML 写法不对：{why} | Not valid TOML: {why} |
| `config/unknown-key` | `key`、`suggest` | 没有 {key} 这一项。是不是想写 {suggest}？ | There is no {key}. Did you mean {suggest}? |
| `config/unknown-key-plain` | `key` | 没有 {key} 这一项 | There is no {key} |
| `config/wrong-type` | `key`、`expected`、`got` | {key} 要写 {expected}，写的是 {got} | {key} needs {expected}, not {got} |
| `config/not-an-option` | `key`、`options`、`got` | {key} 只能是 {options}，写的是 {got} | {key} must be {options}, not {got} |
| `config/out-of-range` | `key`、`min`、`max`、`got` | {key} 要在 {min} 到 {max} 之间，写的是 {got} | {key} must be between {min} and {max}, not {got} |
| `config/wrong-layer` | `key`、`layers`、`layer` | {key} 只能写在{layers}里，写在{layer}里不算 | {key} belongs in {layers}. It does not count in {layer} |
| `config/not-tightening` | `key`、`current`、`got` | 项目配置只能让限制更严。{key} 现在是 {current}，这里写的 {got} 更宽，不算 | A project config can only make limits stricter. {key} is {current}, and {got} here is looser, so it does not count |
| `config/unknown-secret` | `key`、`name` | {key} 引用的密钥 {name} 还没设 | {key} refers to the secret {name}, which is not set |
| `config/untrusted-project` | | 这份项目配置还没信任，先不用 | This project config is not trusted yet and is not used |
| `config/sentence` | `text` | {text}。 | {text}. |
| `config/then` | `rest`、`next` | {rest}{next} | {rest} {next} |
| `config/stops` | | 。？！ | .?! |
| `config/untrusted` | | 还没信任 | not trusted yet |
| `config/env-not-set` | `key`、`name` | {key} 引用的环境变量 {name} 核心起来时没有设 | {key} refers to {name}, which was not set when the core started |
| `config/fix-example` | `example` | 改成其中一个，例如 {example} | Write one of them, e.g. {example} |
| `config/fix-write` | `example` | 改成 {example} | Write {example} |
| `config/fix-move` | `layers` | 挪到{layers}里去 | Move it to {layers} |
| `config/kept` | | 这一行先不管，原样留着 | The line is ignored and kept as it is |
| `config/using-value` | `value`、`from` | 这一项先照 {value} 用着（{from}） | Using {value} ({from}) for now |
| `config/using-last-good` | | 这份文件先照上一次读进来的用着 | Using what was read from this file last time |
| `config/using-nothing` | | 这份文件先不用 | The file is not used for now |
| `config/layer/default`、`project`、`env`（`system`、`personal` 8-1 就有；编号照层的写法） | | 默认值、项目配置、环境变量 | the default、a project config、the environment |
| `config/expected/bool`、`option`、`int`、`float`、`text`、`list`、`table` | | true 或 false、其中一个、整数、数、带引号的字、列表、一张表 | true or false、one of them、a whole number、a number、quoted text、a list、a table |
| `config/applies/new_session`、`head_start`、`next_turn`、`restart`（`now` 8-1 就有；编号照 `applies` 的写法，程序照它拼） | | 以后开的会话生效、下次打开界面时生效、下一轮生效、重启核心后生效 | Applies to sessions opened from now on、Takes effect the next time the interface opens、Takes effect next turn、Takes effect after the core restarts |
| `config/secrets-header` | | Miyu 的密钥：只经 Miyu 写入、替换、删除。不要把这份文件贴给别人。 | Miyu's secrets: written, replaced and deleted only through Miyu. Do not share this file. |
| `config/trust-header` | | Miyu 记着的项目配置的信任：哪个仓库、哪一份内容、信不信任。 | Which project configs Miyu trusts: the repository, the exact content, and the answer. |

一句由几段接成时（第四条第 7 条）：不是以 `config/stops` 里的字结尾的段照 `config/sentence` 补上句号，段和段照 `config/then` 接。中文、日文补「。」、段和段直接接，英文补「.」、段和段之间空一格。

8-2 加进资源的是用得上的几句：上表里除了 `config/out-of-range`、`config/unknown-secret`、`config/env-not-set`、`config/untrusted`、`config/secrets-header`、`config/trust-header`，`config/expected/` 只有 `bool`、`table`，`config/applies/` 只有 `new_session`；别的随用到它的那一步（8-3、8-5、第一项有范围的那一步）。日文的一份照中文写（施工 8-2）。8-3 加了 `config/trust-header`、`config/applies/head_start`（日文照中文写：「Miyu が記録しているプロジェクト設定の信頼：どのリポジトリの、どの内容を、信頼するかどうか。」「次に画面を開いたときに反映されます」）。

**协议拒绝时的话**（`protocol.md`「给人看的字」多的几行）：

| 原因码 | 中文 | 英文 |
|---|---|---|
| `unknown_config_key` | 没有这一项配置。 | There is no such setting. |
| `config_invalid` | 配置有几处不对，没有改。 | Some settings are not right. Nothing was changed. |
| `config_conflict` | 这一项刚被别处改过，没有改：先看看现在的值。 | This was just changed elsewhere. Nothing was changed. Look at the current value first. |
| `config_file_broken` | 配置文件现在读不进来，没法只改一项：先把它改好，比如用 miyu config edit。 | The config file cannot be read right now, so a single setting cannot be changed. Fix the file first, e.g. with miyu config edit. |
| `no_project_config` | 这个目录找不到项目配置。 | There is no project config for this directory. |
| `unknown_secret` | 没有这个密钥。 | There is no such secret. |

**命令行**（`crates/miyu-cli` 的界面语言，中文、英文）：

| 什么时候 | 中文 | 英文 |
|---|---|---|
| `check` 的合计 | `<n> 处错误，<m> 处警告`（没有的那一样不写） | `<n> errors, <m> warnings`（1 个时用单数） |
| `check` 没有问题 | 没有问题 | No problems |
| `edit` 不在终端里 | miyu config edit 要在终端里用 | miyu config edit needs a terminal |
| `edit` 没改 | 没改 | Nothing changed |
| `edit` 有错 | 有 <n> 处错误，还没存。回车接着改，输入 q 放弃： | <n> errors. Nothing saved yet. Press Enter to keep editing, or type q to give up: |
| `edit` 放弃 | 放弃了，文件没动 | Gave up. The file is unchanged |
| `edit` 冲突 | 你编辑的时候文件被改过了，没存。你改的在 <副本> | The file changed while you were editing. Nothing saved. Your edit is in <副本> |
| `edit` 编辑器出错 | 编辑器没有正常退出（<退出码>），没存 | The editor did not exit cleanly (<code>). Nothing saved |
| `edit` 存了 | · 存好了，<applies> | · Saved, <applies> |
| `path --project` 还没有 | 还没有这个文件 | This file does not exist yet |
| `check` 一行的级别（8-2） | 错误、警告，后面接「：」 | error、warning，后面接「: 」 |
| `check` 命令行自己读不了的文件（8-2） | 读不了这份文件：<原因>。这份文件超过 1 MiB，不读。这份文件不是 UTF-8。 | Cannot read this file: <why>. The file is over 1 MiB and is not read. The file is not UTF-8. |
| `explain` 第一行（8-2） | <名字>（<键>）：<说明><生效>。 | <name> (<key>): <description> <applies>. |
| `explain` 生效（8-2） | 当场生效、以后开的会话生效 | Takes effect at once、Applies to sessions opened from now on |
| `explain` 层的名字（8-2） | 默认值、系统配置、个人设置、项目配置、环境变量 | default、system config、personal settings、project config、environment |
| `explain` 生效的那一行（8-2） | ← 生效；环境变量的：← 生效，只管这一次启动 | ← in effect；环境变量的：← in effect, for this launch only |
| `explain` 不算的那一行（8-2） | ← 不算：还没信任、比下面几层宽、不能写在这一层 | ← does not count: not trusted yet、looser than the layers below、not allowed in this layer |
| `set --project` | 项目配置只能手改：miyu config edit --project | A project config is edited by hand: miyu config edit --project |
| `miyu ask` 起头：配置有错 | · 配置里有 <n> 处错误：miyu config check 看是哪里 | · <n> errors in the config: run miyu config check to see them（1 处时 `· 1 error in the config: run miyu config check to see it`） |
| `miyu ask` 起头：项目配置没信任 | · 这里的项目配置 <文件> 还没信任，这次没用它：miyu config trust 看一眼再定 | · The project config at <file> is not trusted yet, so it was not used: run miyu config trust to review it |
| `trust` 没有项目配置 | 这里没有项目配置 | There is no project config here |
| `trust` 列出 | <文件> 会改这几项： | <file> would set: |
| `trust` 问 | 项目配置只能让限制更严。信任这一份吗？[y/N] | A project config can only make limits stricter. Trust this one? [y/N] |
| `trust` 不在终端里 | 要在终端里回答，或者写 --yes、--no | Answer in a terminal, or pass --yes or --no |
| `trust` 本来就信任 | 已经信任过这一份了 | This one is already trusted |
| `trust` 冲突 | 你看的时候它又被改过了，再跑一次 | It changed while you were looking. Run this again |
| `trust` 信任了 | · 信任了。内容变了会再问 | · Trusted. You will be asked again if it changes |
| `trust` 不信任 | · 不信任，这一份不会用。内容变了会再问 | · Not trusted. It will not be used. You will be asked again if it changes |
| `login` 列出 | 配置里用到的密钥： | Keys the config uses: |
| `login` 选 | 选一个编号，或者敲一个新名字： | Pick a number, or type a new name: |
| `login` 一个都没有 | 配置里还没有用到密钥的供应商：写 miyu login <名字> | No provider in the config uses a key yet: run miyu login <name> |
| `login` 问 key | 粘贴 <名字> 的 key（不显示）： | Paste the key for <name> (it will not show): |
| `login` 没收到 | 没收到 key | No key was given |
| `login` 存好了 | · <名字> 的 key 存好了 | · Saved the key for <name> |
| `login` 换掉了 | · 换掉了 <名字> 的 key | · Replaced the key for <name> |
| `login --list` 已设置、未设置 | 已设置、未设置 | set、not set |
| `login --list` 一个都没有 | 还没有设过 key | No keys yet |
| `logout` 删掉了 | · 删掉了 <名字> 的 key | · Deleted the key for <name> |
| `logout` 没有 | <名字> 没有设过 key | <name> has no key |

**帮助页** `crates/miyu-cli/src/help/zh/config.txt`（8-2 施工时照 `cli/main.md`「帮助页」的规矩定稿，最宽 80 列；8-3 加了 `set`、`unset`、`edit`、`trust` 和 `--yes`、`--no`，样本在 `cli/config.md`）：

```text
用法：miyu config <命令> [选项]

看配置、改配置、信任项目配置。

命令：
  get [键…]      印出最终的值，只写一个键时只印值
  set <键> <值>  改一项
  unset <键>     从这一层删掉一项，回到下面一层的值
  edit           用编辑器打开，存盘时先检查
  check [文件]   检查配置有没有写错
  explain <键>   这一项每一层写的什么、哪一个生效
  path           印出配置文件在哪
  trust          看这里的项目配置会改什么，信任或者不信任它

选项：
      --system            系统配置，不写是个人设置（set、unset、edit、check、
                          path）
      --project           当前目录的项目配置（edit、check、path）
      --format text|json  get、check、explain：json 给脚本
      --yes               trust：信任，不问
      --no                trust：不信任，不问
  -h, --help              印帮助
```

`crates/miyu-cli/src/help/en/config.txt`：

```text
Usage: miyu config <command> [options]

See and change settings, and trust a project config.

Commands:
  get [key…]           Print the values in effect, or just the value of one key
  set <key> <value>    Change one setting
  unset <key>          Remove one from this layer, back to the one below
  edit                 Open in an editor, checked before it is saved
  check [file]         Look for mistakes
  explain <key>        What each layer says and which one wins
  path                 Print where the file is
  trust                See what the project config here would set, and trust
                       it or not

Options:
      --system            The system config, instead of personal settings
                          (set, unset, edit, check, path)
      --project           The project config here (edit, check, path)
      --format text|json  For get, check, explain: json for scripts
      --yes               For trust: trust it, without asking
      --no                For trust: do not trust it, without asking
  -h, --help              Print help
```

`crates/miyu-cli/src/help/zh/login.txt`、`logout.txt`（例子，8-5 定稿）：

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

```text
用法：miyu logout [名字]

删掉一个供应商的 key。不写名字的，从设了的里面选。

选项：
  -h, --help  印帮助
```

`crates/miyu-cli/src/help/en/login.txt`、`logout.txt`：

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

```text
Usage: miyu logout [name]

Delete a provider's key. Without a name, pick one of those that are set.

Options:
  -h, --help  Print help
```

### 守着它的

施工时照这个写：

| 测试 | 守哪几条 | 哪一步 |
|---|---|---|
| `crates/miyu-config/src/item/tests.rs`、`item.rs` 的文档 | 宏生成的清单和设置类型一一对上：键、类型、默认值、层、环境变量、生效、界面提示照声明的先后；设置类型照最终值，没有的照默认值。选项区分大小写。没写默认值、选项只有一个的编译不过（`trybuild` 不引，写成宏的文档里的 `compile_fail` 例子）；数没写范围的随整数、小数那一步 | 8-1 |
| `crates/miyu-config/src/list/tests.rs` | 查清单：键重复、按段互为前缀（`ui.lang` 不算）、写法不对（一段、大写、别的字、空段、数字或 `_` 开头）、第一段 `ext`、默认值过不了校验、选项少于两个或写重、一层都没有或层写重，各一例；几处都错的全报 | 8-1 |
| `crates/miyu-config/src/words/tests.rs` | 查资源的字：缺名字、说明、选项名，页和组没名字，资源里多了项、选项、页、组，各一例。几个里的一个怎么连（一个、两个、三个以上，值和字两种「或」）。一项说明后面那几句。缺了哪一句照实报 | 8-1 |
| `crates/miyu-config/src/schema/tests.rs`、`reference/tests.rs`、`value/tests.rs` | 拿假的字和手写的几项：Schema 只有这一层的项、一层层的表、格照字母先后、这一层什么都没有的；参考文件表照名字排、表里的项照名字排、不重开同一张表、每一项两行注释、项间空一行、多行的字每一行都是注释；缺字报是哪一句。值写成 TOML（引号、反斜杠、控制字符转义）、写成 JSON | 8-1 |
| `crates/miyu-core/tests/settings.rs` | 登记的全部清单过 `list::check`，照登记的先后（8-3 起有 `tui.startup`）。中文、英文、日文三份（直接读文件）过 `words::check`。照源码树的资源生成的两份 Schema、参考文件和样本逐字节一样（中文、英文），日文生成得出来 | 8-1 |
| `crates/miyu-core/src/settings/tests.rs` | 起来时生成：字照系统的语言挑（日文、没有的照英文）。资源里缺字、读不懂的，三份各记一条 `WARN`，什么都不写 | 8-1 |
| `crates/miyu/tests/settings.rs` | 真核心：照 `LANG` 写三份，和样本逐字节一样（中文、英文）；一样的不重写（修改时间不变），改过的写回来；该是目录的地方是个文件，三份各记一条 `WARN`，照样起来 | 8-1 |
| `crates/miyu-store/src/generated/tests.rs` | 没有的写上、目录建上；一样的不写（修改时间不变）；不一样的换掉、不留临时文件；目录建不了报错；临时文件点开头、不重名 | 8-1 |
| `crates/miyu-store/tests/human.rs`、`human_languages.rs` | 配置那一格照 `Words` 交出去：项、选项名，句子的编号加 `core/`，少了字段的没有字；配置那一格写错说是哪一份。三种语言的项、选项、页、组和英文的一样，名字都不空 | 8-1 |
| `crates/miyu-endpoint/src/settings/tests.rs`、`crates/miyu-log/src/settings/tests.rs` | 界面语言：`auto` 照系统的语言（`zh`、`ja` 开头的，别的、没有的是 `en`），定了的照定的。`log.level` 的每个选项 `MIYU_LOG` 都读得懂，默认值和没设一样 | 8-1 |
| `crates/miyu-config/src/parse/tests.rs` | 解析的每一种原因码各一例（不认识的键、类型不对、一组键写成值、一项写成表、不在选项里、层不对、写法不对），行、列（中文算一列、点号连着的键指到开头）、`got` 截到 80 个字符。BOM。`\r\n`。`toml_edit` 的报错只取为什么那一行。不认识的表往里报整个键，拼错的一组键也找得到最近的 | 8-2 |
| `crates/miyu-config/src/problem/tests.rs` | 键名拼错给最近的、太远的不给、一样近的取前面的、换位算一次；`got` 截到 80 个字符；每一种原因码说成一句（假的中文字），英文段和段怎么接；缺字报是哪一句 | 8-2 |
| `crates/miyu-core/tests/config_words.rs` | 照源码树的资源：图纸「样子」里的几句，中文、英文一字不差；每一种原因码中文、英文、日文都说得出来 | 8-2 |
| `crates/miyu-config/src/merge/tests.rs` | 四层的先后和来源。环境变量压过（不分大小写）、读不懂的当没设、空的不算。一项写错只丢这一项、照下面几层或默认值，别的照常。项目配置收紧的收、宽的不算、一样的收、不能写的不算、没信任的不算（不信任的不再提醒）。每一层写的从上往下列、哪个生效、不算的原因 | 8-2 |
| `crates/miyu-config/src/item/tests.rs`、`list/tests.rs`、`schema/tests.rs` | 开关、`tighten`、`new_session`、`toggle` 照宏生成；开关只认 `true`、`false`；只有关掉算宽；环境变量的读法。收紧写没写、写对没有。开关的 Schema 是 `boolean`、没有 `enum` | 8-2 |
| `crates/miyu-endpoint/tests/config.rs` | 真核心：握手的 `language`（`auto` 照 `locale`，`zh`、`ja`、别的、没报的；定了的照定的，拒绝的话也照它，`ja` 的照英文）、`config_errors`（只数错误）。`config.schema`（和例子一字不差、登记的先后、开关的几格、页的先后）、`config.get`（来源、`files` 和版本、`all`、环境变量、一项写错的样子和 `using`、读不懂的文件照空的）、`config.check`（三层、收紧、写法不对、参数不对）。不认识的键两个方法都拒、带最近的。个人设置打开了开局只读 | 8-2 |
| `crates/miyu-endpoint/src/config/tests/project.rs`、`trust.rs` | 记一个回答（8-3）：新建的带开头的注释，同一个仓库原地换、别的字节不动，没有的加在末尾，换行照原文件，读不懂的不改。往上找到仓库的根就停（`.git` 是文件、目录都算）、到家目录就停、不看家目录本身、家目录外面到根目录停、数据根里不找、只认最近的一份，家目录下的写成 `~/…`。信任的记录：仓库和版本都对上才算，挪了、变了的是还没问过，后面的记录盖掉前面的，写法不对的那一条不算 | 8-2 |
| `crates/miyu-endpoint/tests/config_trust.rs` | 没有记录的、内容变了的、仓库挪了的不算，造会话、说话的回应带 `untrusted_project`，子目录里开的也找得到。信任着、版本一样的算，开局只读。不信任的不算、不再提醒。`config.get` 带目录：`files.project` 的 `trusted`、版本，没问过的报 `untrusted_project`，信任了写宽的报 `not_tightening`、每一层写明不算（8-2）。`config.trust` 的版本对不上、没有项目配置。一个仓库一条，新的盖掉旧的。账号日志 `trust.changed`。手改 `trust.toml` 重读（8-3） | 8-2、8-3 |
| `crates/miyu-store/src/env/tests.rs` | 系统的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 的先后、空的不算（8-1），都没设的才看系统设置，macOS、Windows 上系统设置里总有一种语言（CI 上跑，8-2） | 8-1、8-2 |
| `crates/miyu-store/src/config_file/tests.rs` | 读（8-2）：没有的是空的；BOM 去掉、版本照带 BOM 的字节；正好 1 MiB 的读、多一个字节的不读；不是 UTF-8 的、是目录的报错；链接指向别处的照读 | 8-2 |
| `crates/miyu-log/tests/install.rs` | 装上照给的级别记，换级别以后照新的（8-2） | 8-2 |
| `crates/miyu-config/src/edit/tests.rs` | 改一项、加一项（有表、没表、表里还没有键、只因为子表才有的、点号连着的、行内表）、删一项（表空了连表头删、在末尾的连前面的空行、有注释的留、行内表连逗号）、只动那一项（前后字节比）、换行照原文件、新文件第一行是 `#:schema`、改了再删回到原样、放不进去的报出来、`input` 和 JSON 的值照类型读 | 8-3 |
| `crates/miyu-store/src/config_file/tests.rs` | 写（8-3）：顺着链接写、链接不动、绕圈报错、指向没有的新建。临时文件在本体旁边、崩在改名前原文件不变、改名前一瞬间有人手改了就放弃、不留临时文件。权限位留着。Windows 上开着的文件重试。BOM 记下、照样加回 | 8-3 |
| `crates/miyu-store/src/journal/tests.rs`、`crates/miyu-endpoint/src/config/journal/tests.rs` | 截半行、`seq` 从 1 数起、接着别人的数、一行一条照事件的外壳（没有 `turn`，内核照不认识的种类留着）、最后一行坏了不往后写；`config.changed`、`trust.changed` 和样本逐字节一样，没有的 `old`、`new` 不写 | 8-3 |
| `crates/miyu-endpoint/tests/config_set.rs` | `config.set` 每一种拒绝（参数不对、不认识的键、值不对、层不对、文件读不进来）。`expect` 对不上、没写的是 `{}`、整份换的版本对不上、新的字有错误、只有警告的照存。写之前发现手改过先重读，BOM、换行照新的字。一次几项全收或者全不收。没变的不写、不记，删一项不新建文件。回应的样子、之后握手照新的语言。日志记了一条，系统配置的进系统日志。写不成什么都没变。先见推送、后见回应随 8-4 | 8-3 |
| `crates/miyu-store/src/watch/tests.rs` | 先写新文件再改名的存法认得出。200 毫秒合并。别的文件名不理。链接指向的目录也看。macOS 上照真实的位置比 | 8-4 |
| `crates/miyu-endpoint/tests/config_watch.rs` | 手改推 `config.changed`（`via: file`）、记日志。核心自己写的不重推。改坏了推问题、改好了推空的。订阅、取消订阅、掉队推 `resync` | 8-4 |
| `crates/miyu-session/tests/turn_config.rs` | 回合中途改了配置，这一轮照开始时的，下一轮照新的。项目配置每一轮开始重读 | 8-4 |
| `crates/miyu-log/src/level/tests.rs` | 运行中换级别。`MIYU_LOG` 设了的不换 | 8-4 |
| `crates/miyu-store/src/secrets/tests.rs`、`crates/miyu-endpoint/tests/secrets.rs` | 写、换、删、列（`used_by`、没设的也列）。0600。值不进回应、运行日志、报错（整份运行日志里搜不到值）。`{ env }` 照核心起来时的环境。手改重读。日志只记名字 | 8-5 |
| `crates/miyu-cli/src/config/tests.rs`、`link/tests.rs`、`crates/miyu/tests/config.rs` | 8-2：`get` 的值、报错一行（级别上色）、`check` 的合计、`explain` 的几行照宽度对齐（和「样子」一样，两种语言）、环境变量和不算的那一行；文件换成真的位置、`~/…`；还没有项目配置时该在哪；握手以后照回应的语言。真核心带三层起来：日志的级别和 `config problems`，`get`（一个键、全部、`--format json` 的来源）、`explain`、`check`（现在的几份、写了文件、`--system`、`--format json`）、`path`（个人、`--system`、`--project` 没有的）印的对，照 `ui.language` 说话，不认识的键退出码 1；`miyu ask` 起头说配置有错（照系统配置的英文说）；帮助页两种语言；参数不对退出码 2；没有 key、核心没在跑的退出码 5。8-3：`config/tests.rs` 照图纸对 `set`、`unset`、`edit`、`trust` 印的字（两种语言）、编辑器照 `VISUAL`、`EDITOR` 挑、经 shell 跑。`crates/miyu-cli/tests/config.rs` 在进程里起核心、人那一头照剧本回：`set`、`unset` 每一种说法（上面一层压着、本来就是、本来就没写、握手以后换语言），`edit` 用假编辑器（改错再改好、改的还在、副本在旁边、只有警告的照存、不改、放弃、读到头、编辑器出错、冲突留着副本、不在终端里、项目配置由命令行写），`trust` 没有项目配置、列出、问、`--yes`、`--no`、直接回车、不在终端里、本来就信任着、有问题的印在后面、看的时候又变了。`crates/miyu/tests/config.rs` 真核心：`set`、`unset`、`trust` 写进文件、运行日志，`set --project`、不在终端里的 `edit` 连核心以前退出码 2，`miyu ask` 起头没信任那一行 | 8-2、8-3 |
| `crates/miyu-cli/tests/login.rs`、`crates/miyu/tests/login.rs` | `login` 写名字、选、管道进来的 key、假终端里不回显，`--list` 不带值（输出里搜不到 key），`logout`，两种语言，退出码 | 8-5 |
| `crates/miyu-cli/src/help/tests.rs` | `config`（八个子命令的选项合在一起，8-2、8-3）、`login`、`logout`（8-5）三页列的选项和程序真有的对得上，最宽 80 列，照文件名读的是自己那一页 | 8-2、8-3、8-5 |

变异测试照施工的规矩跑（每一步的施工单写）。

### 出处

- `14-配置.md`：G1 到 G10，第二节（清单）、第三节（分层、项目配置、说得出来源）、第四节（写盘）、第五节（监视、生效）、第六节（校验、报错、不认识的键）、第七节（密钥）、第八节（协议）、第九节（四种改法）、第十节（代码里的规矩）。
- `07-存储.md`：第二节（目录布局、`system/`、`home/<账号>/`），第三节（系统日志、账号日志记什么），第九节、S8（密钥），S7、S9（保留格式、没有迁移）。
- `06-多用户与身份.md` 第三、四节：谁能改、管理能力（M8 只有管理员，留口子）。
- `11-权限与沙盒.md` 第二节：只读、工作区，开局取哪一级。
- `22-命令行.md` 第二节（输出的规矩、退出码、`--format json`）、第五节（`miyu config` 的七个子命令）、O5（管理命令只是协议的客户端）。
- `26-提示词.md` 第三节（双槽：给人看的字跟着界面语言，不进请求）、第八节（`human/` 放在哪）。
- `28-运行日志.md` 第三节、LG2：`log.level`，`MIYU_LOG` 管这一次启动。
- `02-内核.md` K3：影响请求的，下一个回合开始时生效。
- `04-核心协议.md` 第九节：配置与密钥的方法、`config.changed`。
- `15-模型与供应商.md` 第二节：`keys = [{ secret = … }]`、`{ env = … }`。
- 施工方案第三节 M8：2026-10-01 项目主人定项目配置只做收紧、M8 只有管理员一个人。
- 别家：`git config --show-origin`（说得出来源），visudo（先查再存），dconf 的 locks，direnv 的 `allow`、`deny`（信任），opencode 的 `auth login`、`auth list`、`auth logout` 和 codex 的 `login`、`logout`（管密钥的命令），Taplo 的 `#:schema`，Claude Code 的项目设置、codex 的 `config.toml`（找项目配置的范围）。

### 还没有的

- 人格、预设两层，每一项跟着谁走：随人格那一段（`16-人格与预设.md` 第四节）。
- 会话里临时改的那一层：随 8-10（`session.configure`，`models.md`）。
- 锁：管理员锁住某些项，上层写了也不算，界面显示只读（`14-配置.md` 第三节）：随多用户。
- 谁能改按管理能力细分，清单的那一格，拒绝的原因码：随多用户（`06-多用户与身份.md` 第四节）。
- 成员自己的密钥 `home/<账号>/secrets.toml`、成员自己的供应商，个人设置里的引用先找自己的，`miyu login` 存到自己的家目录：随多用户（`15-模型与供应商.md` M6）。
- 每个账号照自己的语言生成 Schema、参考文件（现在照管理员的）：随多用户。
- 改了名的键：旧名字继续有效、提示新名字（`14-配置.md` 第六节）：第一次改名时加。
- 扩展的配置项：扩展清单的 `[settings]`、`ext.<扩展>` 下的键、握手时交给扩展它那一块、推送给它（`05-内核接口.md` 第二节）：随扩展。
- 进策略快照的项和 `session.policy_changed`（第八条第 4 条）：随第一项进快照的配置。
- `restart` 的项，核心空闲时自己重启：随第一项这样的配置。
- 终端界面里第一次遇到项目配置当场问信不信任：随 M9。M8 里靠 `miyu ask` 提醒、`miyu config trust` 答。
- 项目配置能写的别的项（例如「最高到工作区」）：有人要的时候一项一张小单。
- 项目配置的 Schema：发布到网上的地址，或者照编辑器的 Taplo 配置按路径认。
- 网络文件系统上收不到变动：可以加一项「总是轮询」，真有人撞上再做。
- 密钥推送：两个头同时看着「已设置、未设置」时要推，随界面（M9）。
- 借订阅的登录（Claude Code、Codex）走 `miyu login`：随借订阅那一步（`15-模型与供应商.md` 第二节）。`miyu login` 不写名字时从目录里选供应商：随 8-11（`models.md`）。
- 系统钥匙串：可选的密钥存法（`07-存储.md` S8）。
- 终端界面、网页的设置页：照 `config.schema` 画，随 M9（`14-配置.md` 第九节）。直接敲 `miyu config`、不带子命令、在终端里时打开设置页，不在终端里照旧印帮助（2026-10-01 项目主人定）。
- `miyu doctor` 报配置的错误、密钥文件的权限（`22-命令行.md` 第六节）。
- 命令行自己的字（帮助页、旁白）的日文：现在界面语言是 `ja` 时照英文。拒绝时的话的日文同。

### 起草时定的

技术细节照推荐定，写进了正文。标着「主会话定」的是 2026-10-01 主会话照推荐定的，标着「主会话认了」的是主会话 2026-10-01 审过认下的，别的等主会话审：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 新 crate `miyu-config` 放第 2 层（纯逻辑），读写文件、监视在 `miyu-store`，服务在 `miyu-endpoint`（主会话认了） | 解析、合并、校验、改字都能不碰磁盘地测。照 01 第九节的分层 | 全放 `miyu-store`：混着 I/O，测什么都要临时目录 |
| 声明用 `macro_rules!` 的 `settings!`，一处生成设置类型和清单 | G1：只声明一次。不多一个过程宏的 crate，编译不变慢 | 过程宏 `#[derive(Settings)]`：写着顺，多一个 crate、一套 `syn`。清单和结构体分开写：两处，要靠测试对 |
| 保留格式用 `toml_edit`（MIT 或 Apache-2.0），加进纯逻辑两层的白名单 | S9 要保留注释、排版、不认识的键。它是 cargo 自己在用的 | 自己写：不值。`toml`：读写不保留注释 |
| 监视用 `notify`（CC0-1.0），起不来退回每 2 秒轮询 | 三个平台一份代码，底下就是设计点名的三种接口 | 自己调三个平台的接口：三份代码。只轮询：慢、一直醒着 |
| 版本：整份换用文件字节的 SHA-256。改一项用 `expect` 比这一项在这一层的值（主会话认了） | G5 第 5 条说的是「这一项被别处改过」：改别的项不该冲突。哈希和值都不随核心重启丢 | 只有文件的版本：改无关的两项也冲突。核心里的计数：重启就丢 |
| 生效时机多一种 `new_session`（主会话认了） | 「新会话默认用什么」改了，开着的会话本来不该变，写成下一轮会让人以为当前会话也换了 | 照 G7 只有三种，把这类写成 `now`：界面上说「当场生效」，人以为开着的会话也变了 |
| `ui.language` 的 `auto` 在核心里照握手的 `locale` 算，回应带 `language`。改了以后连接照新的说，不用重新握手（主会话认了） | 头拿到的一种语言就是它该说的。核心的拒绝和头的字一致 | 头自己读配置再算：每个头都写一遍 |
| 报错一行的开头是 `路径:行:列`（主会话定） | 编辑器、很多终端能照它点过去，和编译器的写法一样 | 「第 2 行第 9 列」：读着顺，点不过去 |
| `miyu config edit` 存盘有错：指出来，回车接着改，q 放弃（主会话定） | 照 `14-配置.md` 第九节本来的 visudo 说法 | 错误写成注释放顶上再打开：动了人的字。印了就退：改的还得自己找回来 |
| 这次只把真要调的数挪进配置：界面语言、日志级别、模型那一块、开局只读（主会话定） | 不为以后写代码。别的数有人要改再一项一张小单 | 蓝图里写着「配置那一步能改」的都挪：压缩、任务的几个数进策略快照，要连 `session.policy_changed` 一起做 |
| 系统的语言用 `sys-locale`（MIT 或 Apache-2.0） | 三个平台的系统语言一个函数拿到，Unix 上看的变量和现在的命令行一样 | 自己调 macOS、Windows 的接口：要一段 `unsafe` |
| 项目配置不监视，造会话、每一轮开始时读一次 | 找一下只是往上 stat 几层。监视每个会话的目录要开一堆监视 | 监视：改了当场推，要管一堆目录的增减 |
| 找项目配置：往上找到仓库的根（有 `.git`）为止，不到家目录，只认最近的一份 | 在子目录里开的会话也找得到。不会把家目录、数据根当成项目 | 只看当前目录：子目录里找不到。几份叠起来：说不清来源，收紧还要逐份比 |
| 项目配置只手改，`config.set` 不写它。`miyu config edit --project` 由命令行自己存盘（照第五条的写法） | 仓库里的文件是人的，核心不往别人的仓库里写。少一种写法。`edit` 是人在改，命令行替人存盘，和人用编辑器存是一回事 | `config.set` 也能写：要带目录，核心要判这个目录能不能写。`edit --project` 直接打开真文件、存了再查：错的已经落在文件里 |
| 信任没答之前项目配置不算。M8 里 `miyu ask` 只提醒，在 `miyu config trust` 里答 | M8 没有能当场问的界面，`miyu ask` 本来不问人（`22-命令行.md` O3）。没问过的先不生效最稳 | `miyu ask` 里当场问：管道里问不了，也破了 O3。没问过先算上：只能收紧、风险小，可 G3 说先问 |
| 加一个子命令 `miyu config trust`（`--yes`、`--no`） | 没有界面时，第一次遇到项目配置得有地方看一眼、答一句 | 只让人手改 `trust.toml`：要自己算哈希 |
| 选了不信任也记下，版本一样的不再提醒，内容变了再问 | 答过「不」的不该每次都被提醒，像 direnv 的 `deny` | 只记信任的：不信任的每一次 `miyu ask` 都提醒 |
| `config.trust` 带人看过的那一份的 `version` | 人信任的只能是他看过的那一份，看完以后又变了的不算 | 只带目录：信任的可能是人没看过的内容 |
| `trust.toml` 一个仓库一条，按 `.miyu` 所在那一层的真实位置，家目录下的写成 `~/…` | 数据根搬家、换了家目录的写法照样认得。同一个仓库只有一个答案 | 按配置文件的绝对路径：一回事，写法更长。每一份内容一条：越攒越多 |
| 配置的推送要订阅（`subscribe` 的 `stream: "config"`） | 一次性的 `miyu ask` 用不着，也不用管它读不读 | 握手过的都推：老的头收到不认识的推送 |
| Schema、参考文件放 `state/config/`，照管理员的界面语言生成。核心新建配置文件时第一行写 `#:schema` 相对路径 | 它们是派生的，放 `state/`。相对路径整个数据根搬家也对 | 每个账号一份放 `home/<账号>/index/`：多用户以前用不着 |
| 系统日志、账号日志照事件的外壳写，种类不进内核的表 | 07 第三节：和会话日志同一种格式。内核不认识它们，照不认识的种类原样留 | 另起一种格式：多一套读写 |
| 配置写成了、日志写不进去，配置照改 | 配置文件是真相，日志是留痕。日志坏了不该让人改不了配置 | 日志写不进就拒绝：一个坏了的日志文件卡住全部配置 |
| 手改被看到的也记进日志（`via: file`，`by` 是内核） | 日志回答得了「这一项什么时候变成这样的」 | 只记经核心改的：手改的变化查不到 |
| 系统的密钥改动记进系统日志 | 07 第三节只写了账号日志记 `secret.changed`，没说系统的密钥 | 记进管理员的账号日志：系统的东西散到个人那里 |
| 密钥文件平铺 `名字 = "值"`。Unix 0600。Windows 照数据根继承的访问控制 | 最简单。用户目录在 Windows 上本来只有本人、系统、管理员组能进 | 每个密钥一张表（能加元数据）：用不上。Windows 上另设访问控制列表：多一段要 `unsafe` 的代码 |
| `miyu login` 的名字就是密钥的名字 | 一家配了几个 key 时各有名字，和配置里的 `{ secret }` 对得上 | 按供应商的名字：一家只能存一个 key |
| 贴 key 不回显用 `rpassword`（Apache-2.0） | 三个平台都能关回显 | 自己写：Windows 上要调控制台的接口 |
| `secret.set` 回应带 `replaced` | `miyu login` 要说「存好了」还是「换掉了」，不用先多问一次 | 回应 `{}`：先 `secret.list` 再设，多一个来回 |
| `input`（人敲的字）由核心照类型读 | 各个头不用各带一份类型表。扩展的配置项也认得 | 头先要 `config.schema` 再自己转：多一个来回，每个头写一遍 |
| 键名拼错：编辑距离不超过 3、也不超过键长的三分之一 | 短键上差 3 个字已经是另一个词了 | 固定不超过 2：长键上的拼错给不出 |
| `MIYU_LOG` 读不懂的当没设，照配置 | 有了配置以后，退到配置比退到 `INFO` 更对 | 照原来退到 `INFO` |
| `edit` 的临时副本放在原文件旁边 | `#:schema` 是相对路径，副本在别处编辑器就找不到 Schema，补全没了 | 放系统的临时目录：visudo 的做法，Schema 失效 |
| `toml_edit` 的报错只取为什么那一行 | 它印的那一行原文可能很长。密钥文件里更不能印 | 原样转：一行报错变成好几行，还可能带出密钥 |
| 握手带 `config_errors`，`miyu ask` 起头说一句 | G8：所有头都显示。一次性的命令不该为这个多一个来回 | `miyu ask` 不说：配置写错了人不知道 |

### 项目主人定的（2026-10-01）

1. **项目配置照 `14-配置.md` G3 原样**：只认清单里标明能写的项，只能收紧。第一次遇到先问信不信任，按仓库在哪和内容的哈希记住，内容变了再问。`trust.toml`、`config.trust` 留着，G3 不改。M8 里项目配置能写的只有「新会话开局只读」`permission.start_read_only` 这一项。写进了第三条、「M8 的配置项」、`config.trust`。
2. **没设界面语言时是 `auto`，跟着系统**：终端照系统的语言，网页照浏览器。写进了第二条第 8 条。
3. **管密钥的命令叫 `miyu login`、`miyu logout`、`miyu login --list`**，照 opencode 的 `auth login`、`auth list`、`auth logout` 和 codex 的 `login`、`logout`：`login` 选供应商、贴 key 不回显，`logout` 删掉，`--list` 只列哪几家设了、不给看 key 本身。以后借订阅的登录也走它。协议照旧是 `secret.set`、`secret.delete`、`secret.list`。写进了「命令行 `miyu login`、`miyu logout`」、第十一条。
4. **写错一项只丢这一项**：照下面几层或默认值，其余照常生效，所有的头都显示这条错。整份 TOML 读不懂的，才整份照上一份好的。写进了第二条第 4 条，`14-配置.md` 的 G8 在这个分支上补了一句。

主会话照推荐定的几条（报错的开头、`edit` 有错时怎么办、只挪真要调的数）写在「起草时定的」。

### 施工时定的

8-1 施工时照推荐定的技术细节（2026-10-01，施工员定，写进了正文）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 选项的字段用 `String`，经 `From<&Value>` 从值变过来 | 选项只在宏里写一遍，字段拿到的就是那个选项，对不上的情形不存在 | 每个选项一个枚举：清单和枚举两处写，要靠测试对。`FromStr`：变不成的还要兜底 |
| 最终值 `Values`（键到值）8-1 就有，这一步用全是默认值的那一份 | `from` 要有东西读，核心要照 `ui.language` 的最终值挑语言；8-2 的合并交出同一个类型 | 8-1 不生成 `from`：核心直接读清单里的默认值，绕过设置类型 |
| 类型、层、生效、控件只加用到的：选项、系统和个人、当场、下拉；`tighten` 随 8-2 | 不为以后写代码 | 照图纸的表一次加全 |
| 宏里选项至少两个、至少一层，写不对的编译不过；`list::check` 再查一遍 | 能在编译时拦的就在编译时拦；手写的 `Item` 也逮得住 | 只在测试里查 |
| 查清单、查资源的字只在测试里，核心起来时不查 | 清单写在代码里，写错是程序的错，测试一定逮得住；起来时不白花时间 | 起来时查，查出来记日志 |
| `ui.language` 8-1 就声明在 `miyu-endpoint/src/settings.rs`，`language_for` 也在那里 | 那是它最后的位置（8-2 握手在端点算语言），8-2 不用挪；核心生成文件、握手算语言用同一个函数 | 8-1 先放核心，8-2 再挪 |
| 系统的语言 8-1 只看 `LC_ALL`、`LC_MESSAGES`、`LANG` | 施工单说有现成的就用现成的：和命令行认的一样；`sys-locale` 连同 macOS、Windows 的测试随 8-2 | 8-1 就引 `sys-locale` |
| 连词、标点、句子的拼法都放在 `said` 里：`config/or-values`、`config/or`、`config/list`、`config/facts`、`config/schema-description`、`config/reference-item` | 无硬编码；中文的「或」连代码两边空格、连字不空格，英文、日文各有各的 | 标点写在代码里，照语言分支 |
| 参考文件照名字排，不照登记的先后；登记的先后是 `ui` 在前 | 和 `miyu config get` 一样好找，同一张表不会分两处；设置页的页照第一次出现的先后排（8-2），「通用」在前 | 照登记的先后：表可能分两处写，TOML 不许 |
| 生成的文件的写法比配置文件的简单：不顺着链接、不带权限位、替换前不再读、Windows 上不重试；新建临时文件、删临时文件两个小函数从 `blob.rs` 挪进 `durable.rs` 共用 | 派生的文件没人链接、没人手改，写不成下次起来再写；两处用的同一段代码只留一份 | 8-1 先做 8-3 的完整写法 |
| 字缺了、读不懂的，那一份不写，也记 `config schema not written`，原因 `no words for …` | 不写出一份缺字的；一条日志说清是哪一份、缺哪一句 | 缺的字照编号印进文件 |
| 和样本逐字节比放在 `miyu-core/tests/settings.rs`；`miyu-config` 自己的测试拿假的字测样子 | `miyu-config` 在第 2 层，拿不到上层声明的清单和读资源的 `Human` | 样本测试放 `miyu-config` 里：只能拿手写的清单，样本就不是出厂的样子 |
| `Words` 是 trait，`miyu-store` 的 `Human` 实现它；资源里 `config` 那一格的样子 `ConfigWords` 在 `miyu-config` 定 | `miyu-config` 不碰磁盘；字的样子和用字的地方在一起 | 核心把字抄成一个结构再交进去 |
| JSON 的格照字母先后靠 `serde_json` 的 `Map`（工作区没开 `preserve_order`） | 不用自己排；哪天有依赖打开它，样本测试当场红 | 自己拼 JSON 的字 |

8-2 施工时照推荐定的技术细节（2026-10-01，施工员定，写进了正文；标着「主会话定」的是主会话同一天定的）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| `miyu ask` 起头「配置有错」那一行随 8-2，「项目配置没信任」那一行随 8-3（主会话定） | 没信任那一行叫人去跑 `miyu config trust`，这个命令 8-3 才有；协议上的 `untrusted_project` 8-2 就带着 | 两行都随 8-2：提醒里的命令还不存在 |
| 握手以后，命令行的每个子命令都照回应的 `language` 说；握手以前照 `sys-locale` 兜底（主会话定） | 设了 `ui.language` 的，`miyu config` 和 `miyu ask` 说同一种话。改法小：握手回来换一份计划的语言，换了的给人看的字重读一份 | 只有 `miyu config` 照回应：两个命令说两种话 |
| 命令行握手时报的 `locale` 照它的界面语言（`zh-CN`、`en`），不照系统原样的 | 命令行自己的字只有中文、英文，日文系统上报 `en`，核心回 `en`，命令行上下一致；日文随命令行有日文那一步 | 报系统原样的：日文系统上报错的话是日文、命令行的字是英文 |
| 登记的先后 `ui`、`permission`、`log` | 设置页的页照第一次出现的先后排：通用、权限、高级 | 加在最后：高级排到权限前面 |
| 宏里 `tighten` 写在 `layers` 后面，默认值照 `kind` 变成值（`__settings_kind!`、`__settings_default!`） | 收紧是跟着层的；一个 `macro_rules!` 认两种类型，不另写宏 | 写在最后：和它管的层隔得远 |
| 解析时一项的问题指到值，不认识的键指到键；点号连着写的键指到整个键的开头 | 编辑器照 `路径:行:列` 跳过去，落在要改的地方；样子里的例子（`2:9`、`3:19`）就是值的位置 | 一律指到键 |
| 不认识的一张表往里走，每一项照整个键报；空的不报 | 整个键才找得到最近的（`uii.language`）；一张表报一处，拼错的表头找不到最近的键 | 只报表本身 |
| 层不对的只报 `wrong_layer`，不再查类型；值写得对的记下来、不算 | 挪了层还要再改值的，挪过去再报；`explain` 列得出写了什么 | 类型、层都报：一项两条 |
| 一句话的句号、段和段怎么接也放进 `said`（`config/sentence`、`config/then`、`config/stops`） | 无硬编码：中文「？」后不补「。」、英文段间空一格都是字，代码只看这一段是不是已经收了尾 | 标点写在代码里、照语言分支 |
| `config/wrong-type` 中文写成「{key} 要写 {expected}」，`{expected}` 前空一格 | 样子里的例子是「要写 true 或 false」：开关期望的字以代码开头；一组键的「要写 一张表」读着也顺 | 不空格：和样子对不上 |
| 选项写成了别的类型也照选项说，原因码是 `wrong_type`；`config/expected/option` 不加 | 「要写其中一个」不说是哪几个，改不了；列出能写的几个才改得了 | 照 `config/expected/option` 说 |
| 开关的改法取默认值的另一个 | 选项取默认值，开关取默认值就是没写，给另一个才是能照抄的改法 | 取现在用着的另一个 |
| `unknown_config_key` 里每一条的 `level` 是 `error`，没有「这一行先不管」 | 这是请求写错了，不是文件里的一行 | 照文件里的写成警告 |
| `config.check` 的问题不带 `file`；一项的 `using` 照核心手里另外几层算，整份的不写 | 查的是一段字，头知道它是哪份；`miyu config check` 的样子里一项的问题带「这一项先照…用着」 | 都不带 `using`：和样子对不上。`edit` 那一段（8-3）的例子没带，到时照这里改 |
| `using` 只给值的问题（类型、选项、层、收紧），话里只有类型、选项两种说「先照…用着」 | 层不对、写宽了的，话里已经说了不算；JSON 里照样带，头要用自己拿 | 每一种都说 |
| 起来时读不好的文件 `using` 是 `nothing`；`last_good` 随 8-4 | 8-2 没有重读，没有「上一次读好的」 | 8-2 就留上一次的：用不上的代码 |
| 配置服务 `Config` 放在核心的家底里，8-2 起来以后不变，不经 `watch` | 8-2 没有会变的配置；会话造的时候读一次就够，`watch` 随 8-4 | 8-2 就上 `watch`：不为以后写代码 |
| 握手时算一次语言记在连接上 | 8-2 起来以后 `ui.language` 不变，重算的结果一样；8-4 当场变的时候改成每次重算 | 连接记着 `locale` 每次重算 |
| 读 `trust.toml` 用 `toml_edit`（端点直接依赖）；读不进来的照没有记录，记 `WARN trust not read` | 和读配置同一个；信任的记录坏了不该拦住起来，照没问过最稳 | 另写一个格式：多一套读写 |
| 找项目配置时根目录本身也不看 | 和家目录一样，根目录不是谁的仓库 | 看根目录：`/.miyu` 会算成所有目录的项目配置 |
| 信任的记录路径一段段接在家目录后面比 | Windows 上真实的位置以 `\\?\` 开头，那里 `/` 不算分隔符，整段接会对不上 | 字符串直接比：Windows 上认不出 |
| 运行日志装上时照 `MIYU_LOG`，读完配置经 `Guard::set_level` 换（`tracing-subscriber` 的 `reload`）；`install` 改成收级别，不再自己记读不懂的那一条 | 照第二条第 7 条：先有日志再读配置，读配置的问题记得下来；8-4 运行中换用同一个把手 | 读完配置再装日志：读配置时的问题没地方记 |
| `sys-locale` 只在 `LC_ALL`、`LC_MESSAGES`、`LANG` 都没设时问；Linux 上它看 `LANGUAGE` | 和命令行原来认的一样，环境变量设了的照旧 | 一律问它：Linux 上先看 `LANGUAGE`，变了原来的先后 |
| `config.schema` 里名字、说明这种语言没有的照英文，都没有的照键、空的 | 和 `store/resources.md` 的退法一样 | 缺了就拒绝：设置页打不开 |
| 命令行 `check` 不写文件时只查有的那几份；写了文件、文件没有的报读不了 | 还没有的那一层没什么可查；人点名的文件不在是写错了 | 没有的也报：每次都报个人设置没有 |
| 命令行 `explain` 的几列：后面还有东西的补齐，最后一格不补；环境变量的行没有文件那一格 | 和「样子」一字不差，行尾没有空格 | 每一格都补齐：行尾一串空格 |
| `explain` 里不算的原因：还没信任、比下面几层宽、不能写在这一层 | 图纸只写了「← 不算：<原因>」，照原因码各一句 | 印原因码 |
| `miyu ask` 起头那一行英文 1 处时用单数 | 照 `check` 合计的规矩 | 一律复数 |

8-3 施工时照推荐定的技术细节（2026-10-01，施工员定，写进了正文）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 改一项照 `toml_edit` 读的时候记下的位置只换那一段字，`toml_edit` 照旧只开 `parse` | 前后字节比得住；用的是已经在读的那一半，不多开功能 | 读成可改的文档改完写回：写回去的样子由它定，空格、引号的写法可能变 |
| 那一组写成点号连着的、行内表的，照它的写法接上；只因为子表才有、没有表头的当没有，末尾新开；写成值、数组，这一项本身是表，改完读不懂的：放不进去，`config_file_broken` | 手写的文件什么样都有，写回去还得读得懂；放不进去是文件本身的毛病 | 一律在末尾开 `[ui]`：和点号写的 `ui.x = …` 撞，TOML 不许 |
| 表空了删表头时，在文件末尾的连前面那一行空行一起删 | 新建时前面空了一行，改了再删回到原样 | 只删表头：每改一次多一行空行 |
| 配置服务住在核心家底的一把锁里（`std::sync::Mutex`），拿着它不 `.await`；每一次改之前都重读这一层、换上 | 改、查排着队；一把锁最简单，改配置是很少的事；每次都重读比先比再读少一个分支，结果一样 | 一个 actor 收命令：多一套收件箱 |
| 写成了就换上新的最终值：之后握手的连接、造的会话照新的；已经连着的连接、开着的会话怎么跟随 8-4 | 配置服务手里的必须是磁盘上的那一份，不然下一次改会当成手改；最终值顺手重算 | 8-4 以前不换：下一次改当成手改重读 |
| `config.set` 里请求写错的问题不带 `file`、行列，也不说「先照…用着」 | 查的是请求，什么都没变 | 照文件里的说「先照…用着」：说的是没变的那个值，像是改了 |
| 整份换的 `config_invalid` 只带错误；`expect` 几项对不上的 `current` 是头一个的 | 警告不拦；协议上只有一格 `current` | 全带；`current` 换成按键的表：多一种形状 |
| 替换前有人手改、重来三次都不行：`config_conflict` 带 `version` | 头照版本重读再来 | `internal_error`：不是核心出了问题 |
| 整份换的只动了注释也记一条日志，`changes` 是空的；什么都没写的不记 | 落了盘就留痕 | 只动注释的不记：日志里少一次写 |
| 日志每追加一条都重新打开、读整份拿最后一行；`cause` 是这条命令的编号，时刻照核心的钟 | 改配置很少；不用留着开着的文件，手改过的也认得；和会话日志一样查得到是哪一次命令 | 开着文件、只读末尾：多一份要管的状态 |
| 日志的样本 `seq` 是 1 | 样本是一份新日志的第一行，测试照它逐字节比 | 照起草时的 3、5：测试得先垫几行 |
| `trust.toml` 同一个仓库有几条的换最后一条，没有的加在末尾；新建的开头注释照这个连接的语言；读不懂、写不成的 `internal_error` | 读的时候最后一条算；回答的人就在这个连接上；手改坏了的不替人修 | 照管理员的 `ui.language` 另算一次：结果一样，多一段代码 |
| 生效时机多一种 `head_start`，字是「下次打开界面时生效」 | 头自己读、启动时读一次，核心不管它；「界面」不说是哪一种头，网页以后也用得上 | 写成 `restart`：那是重启核心 |
| `tui.startup` 先在 `miyu-core/src/settings.rs` 替终端界面声明；登记在 `ui` 后面，页排成通用、界面、权限、高级 | 终端界面还没进工作区，核心不用它；界面的设置挨着通用 | 放端点：端点也不用它；放最后：界面排到高级后面 |
| 名字、说明：「启动时打开」「终端界面启动时开一个新会话，还是接着最近的那一个。」，选项「新会话」「最近的会话」，页「界面」、组「终端界面」（中英日） | 照施工单给的形状写，说的是做什么，不说怎么做 | |
| 命令行的 `set --project`、不在终端里的 `edit` 连核心以前就拦下，退出码 2 | 参数不对不该拉起核心；没设 key 时也该是 2，不是 5 | 连上核心再说：没设 key 的先报 5 |
| 人那一头做成 `Console` 接口（是不是终端、读一行、开编辑器），`config_on` 收它 | 测试不用真终端、真编辑器也走得到每一条路 | 测试里起伪终端：要加依赖，三个平台各一套 |
| `edit` 副本名 `.<文件名>.edit-<8 位十六进制>.toml`；为副本新建的目录没存时删掉（空的才删） | 不留下空的 `home/<账号>/`、`.miyu/` | 留着：没存也在仓库里多一个目录 |
| `edit` 存好了照改了的几项什么时候生效接在一起说，只动注释的、项目配置只说「存好了」 | 图纸只有一项的写法 | |
| `set` 的说法：回应里这一项的来源是上面一层（或环境变量）的，说那一层写着什么；回应里没有这一项（本来就是）的，再 `config.get` 带 `all` 拿这一层写的值 | 协议上「没变」的回应不带值，不为一句话加格 | 回应里另加一格：多一种形状 |
| `trust` 列出的只有信任了就算的几项；不能写、写宽了的照报错一行印在后面，`untrusted_project` 那一条不印 | 列的是「会改什么」；没信任正是要问的 | 全列：列出了不会改的 |
| `miyu ask` 的开头那几行挪进 `ask/follow/opening.rs` | `follow.rs` 加了没信任那一句就超过 500 行 | |
| 端点 `Config::defaults` 里系统配置的路径原来是 `system/` 目录，改成 `system/config.toml` | 8-2 的小错：没读配置的核心上 `config.set` 会写错地方 | |

### 要跟着改的别的页

施工时改。8-1 改了的写在每一条末尾：

- `protocol.md`：方法表加 `config.schema`、`config.get`、`config.set`、`config.check`、`config.trust`、`secret.set`、`secret.delete`、`secret.list`。握手的回应加 `language`、`config_errors`，`locale` 写明是系统的语言、`ui.language` 是 `auto` 时才用。`session.create`、`session.send` 的回应加 `untrusted_project`，`session.create` 第 2 条的开局权限照 `permission.start_read_only`。`subscribe`、`unsubscribe` 的 `stream` 加 `config`（不带 `session`），推送表加 `config.changed`，`resync` 加配置流。出错表加六个原因码，拒绝的 `data` 多 `problems`、`current`、`version`。「给人看的字」加六行。「还没有的」删掉配置那几项。 8-3 改了：方法表加 `config.set`、`config.trust`，出错表、「给人看的字」加四个原因码，拒绝的 `data` 多的几格，「还没有的」删掉这两个方法。 8-2 改了：方法表加 `config.schema`、`config.get`、`config.check`；握手的回应加 `language`、`config_errors`，`locale` 写明是系统的语言；`session.create`、`session.send` 的回应加 `untrusted_project`，开局权限照 `permission.start_read_only`；出错表加 `unknown_config_key`，拒绝的 `data` 可以多几格；「给人看的字」加一行；「还没有的」配置那几项改成指到 8-3、8-4。
- `store.md`：「数据根里有什么」加 `system/config.toml`、`system/secrets.toml`、`system/journal.jsonl`、`home/<账号>/settings.toml`、`home/<账号>/trust.toml`、`home/<账号>/journal.jsonl`、`state/config/`。「在哪」加 `config_file.rs`、`journal.rs`、`watch.rs`、`secrets.rs`，`env.rs` 多 `locale`。「还没有的」删掉系统日志、账号日志、配置、密钥、信任那几条。 8-3 改了：「在哪」加 `journal.rs`、`config_file.rs` 写的那一半；数据根里加两份 `journal.jsonl`；「还没有的」删掉系统日志、账号日志、配置。8-1 改了：`state/config/`、`generated.rs`、`env.rs` 的 `locale`、`durable.rs` 的临时文件。 8-2 改了：「在哪」加 `config_file.rs`（读的那一半），`env.rs` 的 `locale` 换成 `sys-locale` 兜底；数据根里加 `system/config.toml`、`home/<账号>/settings.toml`、`home/<账号>/trust.toml`（只读）。
- `store/resources.md`：`human/<语言>.json` 多一格 `config`（「只许有两格」改成三格）。说法多 `config/*`。`human_languages.rs` 也查 `config` 那一格三种语言对得上。8-1 改了：三格、`Words`、生成文件要的几句。 8-2 改了：`config` 那一格多 `permission.start_read_only`、页 `permissions`、组 `sessions`；`said` 多报错的话和接句子的三句；`Human` 多 `page`、`group`。 8-3 改了：`said` 多 `config/trust-header`、`config/applies/head_start`。
- `core.md`：起来的先后在找到资源目录以后加「读配置、读密钥、读信任」，读完以后「写 Schema 和参考文件」「开始监视」。环境变量表的 `MIYU_LOG` 写明压过配置。运行日志加 `miyu::config` 那几行。模型那一节（`DEEPSEEK_API_KEY`、`MIYU_DEV_*`）由 `models.md` 那边改。8-1 改了：第 6 步找到资源目录以后写 Schema 和参考文件，`settings.rs`，`WARN config schema not written`。 8-2 改了：第 6 步找到资源目录以后读配置、照 `log.level` 换运行日志的级别，再写 Schema 和参考文件；`MIYU_LOG` 压过配置；运行日志加 `config problems`、`trust not read`、`log level`、`MIYU_LOG not understood, using config`。
- `log.md`：级别由 `log.level` 定、`MIYU_LOG` 压过。读不懂的 `MIYU_LOG` 退到配置。运行中换级别。「还没有的」删掉 `log.level`。8-1 改了：`settings.rs` 声明 `log.level`，「还没有的」那一条写明声明了、8-2 读。 8-2 改了：`install` 收级别、交回的 `Guard` 能换级别（`set_level`）；读不懂的 `MIYU_LOG` 由核心读完配置以后记、退到配置；「还没有的」那一条改成只剩运行中换（8-4）。
- `session/actor.md`：回合开始时取一份配置的快照，带上信任着的项目配置，这一轮都用它（第八条第 3 条）。
- `cli/main.md`：子命令表加 `config`、`login`、`logout`。帮助页多三页，主帮助页加三行。界面语言握手以前照系统的语言（`sys-locale`，不再只看 `LANG` 这几个变量），握手以后照回应的 `language`，`ja` 的照英文。 8-2 改了：子命令表、主帮助页加 `config`，帮助页九页；界面语言握手以前照 `sys-locale` 兜底，握手以后照回应的 `language`，每个子命令都是。
- `cli/ask.md`：起头配置有错、项目配置没信任的两行。给人看的字照握手回的语言。 8-2 改了：起头配置有错那一行；握手以后照回应的语言。 8-3 改了：没信任那一行，开头几行挪进 `ask/follow/opening.rs`。
- 命令行这两节施工时拆成 `cli/config.md`（8-2）、`cli/login.md`（8-5），照「每条命令一页」。 8-2 拆了 `cli/config.md`：四个子命令的走法、样子、帮助页的样本。 8-3 改了：加 `set`、`unset`、`edit`、`trust`，帮助页的样本，退出码。
- `01-架构.md` 第九节：登记 `miyu-config` 在第 2 层。白名单加 `toml_edit`。8-1 改了：登记在第 2 层；8-1 不用 `toml_edit`，白名单随 8-2 加。 8-2 改了：白名单加 `toml_edit`。
- `licenses.md`：新依赖 `toml_edit`（MIT OR Apache-2.0）、`notify`（CC0-1.0）、`sys-locale`（MIT OR Apache-2.0）、`rpassword`（Apache-2.0），都在能用的名单里。8-1 没有新依赖。 8-2 加了 `toml_edit`（和它带进来的 `toml_parser`、`toml_datetime`、`winnow`）、`sys-locale`，门禁过了。
- `14-配置.md`：G3 不改。G7 的表加「以后开的会话」。G8 补了一句只丢写错的那一项（这个分支上已经改了）。第九节「命令行」那一行加 `trust`。8-1 改了：状态那一行记一句做到哪了。 8-2 改了：状态那一行记一句做到哪了。 8-3 改了：状态那一行；G7 的表加「以后开的会话」「头下次启动」；第九节「命令行」那一行加 `trust`。
- `07-存储.md`：第二节加 `state/config/`。第三节系统日志加 `secret.changed`（系统的密钥），账号日志的 `trust.*` 写成 `trust.changed`。 8-3 改了：第三节这两处。
- `22-命令行.md` 第五节：`miyu config` 加 `trust`。加 `miyu login`、`miyu logout`（`miyu login --list`）。 8-3 改了：`miyu config` 加 `trust`。
- `models.md`（另一个分身在画）：供应商、模型、池的键用这一页的清单声明。要的类型（整数、小数、网址、名字、引用、列表、表、密钥）和「人起的名字那一段」在它用上的那一步加。`models.chat` 这类的生效时机是 `new_session`。引用类的项「没有默认值」怎么算（G1 的测试要每一项有默认值），由它定，这一页的门禁照着改。取密钥经这一页的 `{ secret }`、`{ env }`。回合开始冻结的配置由它用上。`miyu login` 不写名字时列出目录里的供应商、借订阅的登录，由它接上。`DEEPSEEK_API_KEY` 的特判、`MIYU_DEV_*`、没 key 不拉起的规矩由它删，删的同一步把第十条第 1 条改成一律拉起。
- `26-提示词.md` 第十节：不用改，配置这一块没有给模型看的字。
- 跨会话的图纸（另一个分身在画）：没有交叉。
