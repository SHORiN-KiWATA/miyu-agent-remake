## 软件包清单

### 是什么

一个软件包一份清单：它是什么、哪个程序、给 `miyu` 加哪个子命令、界面认哪几页、核心怎么拉起它、`miyu check` 怎么查它自己的文件（`05-内核接口.md` 第二节、I9）。核心起来时读一次，头经 `package.list` 列得出装了哪些。之后的几步照它走：包的配置项并进配置清单（9-1 下），`miyu <名字>` 转交给包的程序（9-2），界面照清单找（9-3），核心拉起 `process` 包（9-4），预设按包开关（P-2）。

状态：图纸，施工 9-1（上）做了（2026-10-07 主会话；方向是施工方案第三节 9-1 那一行和 I9，项目主人 2026-10-07 定；清单的形状和终端界面、通讯平台的会话对过）。包的配置项施工 9-1（下）做了（2026-10-07 主会话；设置页的挂法和终端界面的会话对过），列表随 9-1（补）（2026-10-07 主会话，通讯平台的会话要的）。转交子命令、跑包的检查随 9-2，照清单找界面随 9-3，拉起 `process` 随 9-4（上）（另见 `extensions.md`）。种类多内置、小程序，功能、平台接入、依赖施工 F-1 做了（2026-10-09 主会话；方向是设计 `30-插件框架.md`，项目主人同一天定）：这一步只读、只列；内置的包照清单启用施工 F-2 做了（同一天）；预设照功能开关随 F-3。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-config/src/package.rs`、`package/reader.rs` | 一份清单读成样子：每一格的写法，写错的报第一处，带代码、第几行（纯逻辑）；`reader.rs` 一张表一张表地读 |
| `crates/miyu-config/src/package/settings.rs` | `[settings]` 的读法；拼成配置清单的项（施工 9-1 下） |
| `crates/miyu-config/src/package/features.rs` | `[features]` 的读法；没写的照包算一个（`Manifest::features_of`，施工 F-1） |
| `crates/miyu-config/src/package/links.rs` | `[connection]`、`[depends]`、`[recommends]`、`[worker]` 的读法（施工 F-1） |
| `crates/miyu-config/src/package/code.rs` | 读不成时的代码（施工 F-1 从 `package.rs` 挪出来） |
| `crates/miyu-config/src/phrases.rs` | 「语言到一句话」那一格的读法，和人格的名字、说明共用 |
| `crates/miyu-store/src/packages.rs` | 两层在哪、读出所有清单、同编号、子命令名撞了、功能的编号撞了（施工 F-1）、系统账号撞了管理员（施工 O-4 下）；声明了的系统账号（`system_accounts`）；包放状态的目录；包自己的文件的目录（`Found::files_dir`，施工 R-5 三补） |
| `crates/miyu-endpoint/src/packages.rs` | 核心起来时读一次、记运行日志；`package.list`；照核心自己的模块认撞没撞、拼包的配置项（`settle`，施工 9-1 下） |
| `crates/miyu-core/src/settings.rs` | 起来时照清单拼好包的配置项（`Packaged`），读配置、生成 Schema 和参考文件时并进去（施工 9-1 下） |
| `crates/miyu-store/src/human.rs` | 给人看的字并进包的配置项的名字、说明和组名（`Human::with_packages`，施工 9-1 下） |
| `crates/miyu-endpoint/src/check.rs` | `miyu check` 照磁盘查清单 |
| `resources/packages/web.toml` | 出厂的网页界面的清单 |
| `resources/packages/tui.toml` | 出厂的终端界面的清单（9-3 补：终端的会话给的，和 proto 上的一字不差；程序 `miyu-tui` 随 M9）；9-3 再补多配置项 `tui.icons`（图标：`nerd`、`plain`，第一次打开的引导写它） |
| `resources/packages/onebot.toml` | 出厂的接入QQ 的清单（施工 O-18；`[settings]` 四项随 O-20，原来核心替它声明，`onebot.md` 第一条「软件包清单」；施工 F-2 改名、多平台接入和功能 `qq`） |
| `resources/packages/basesystem.toml`、`memory.toml`、`roleplay.toml`、`mermaid.toml`、`net.toml` | 出厂的内置包的清单（施工 F-2）：基础系统必需、九个功能；人格记忆、人设防失忆提醒各算一个功能；画 mermaid、联网不带功能；人格记忆推荐小程序 `embed`（施工 R-5 三补） |
| `crates/miyu-embed/package/embed.toml`、`package/embed/model.toml` | 「内置语义模型」这个小程序包的原本（施工 R-5 三补，`recall.md` 第四条）：出厂不装，不在资源目录里；模型文件不进仓库，做包时从 Release 取 |
| `crates/miyu-core/src/lib.rs`、`packages.rs` | 编进来的内置包那张表（`built_in`）；起来时照清单登记工具（`tools`）、查询（`packages::register`）（施工 F-2） |

### 对外的样子

**在哪**：出厂的放资源目录的 `packages/<编号>.toml`，管理员自己装的放 `home/<管理员>/packages/<编号>.toml`（`07-存储.md` 第二节）；包自己的文件放清单旁边的同名目录（`packages/<编号>/`，照 `miyu_store::packages::Found::files_dir` 算，用的一方不自己拼；施工 R-5 三补起有包用它）。编号就是文件名，写法同人格的编号（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个），不合写法的、不是 `.toml` 的不算。包自己在这台机器上的状态放 `<数据根>/state/packages/<编号>/`，包自己建、自己用。

**格式**（TOML；只收下面这些，不认识的表、键报错）：

```toml
[package]
kind = "ui"                      # ui：界面；process：核心拉起的扩展（9-4），通讯平台的接入也是这一种；
                                 # builtin：内置，代码编在核心里；worker：小程序，核心按需拉起（施工 F-1）
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
capabilities = ["network"]       # 要的扩展能力（施工 9-4 下上，extensions.md「能力」）：只认 05 第三节那十二个名字，可以不写
system_account = true            # 有没有自己的系统账号（施工 O-4 下）：账号名就是包的编号；不写是没有

[check]                          # 要有 [command]
args = ["check"]                 # miyu check 跑 <program> <args…>（9-2）

[settings.port]                  # 一项一张表，见下面「配置项」
type = "int"
min = 1
max = 65535
default = 8300
layers = ["system"]
name = { en = "Web port", zh = "网页的端口" }
```

施工 F-1 加的几格（设计 `30-插件框架.md`）：

```toml
[package]
kind = "builtin"
required = true                  # 必需的：卸不掉。只有内置包能写，不写是假

[features.files]                 # 包带的功能，编号写法同包的编号，全局不重
name = { en = "Files", zh = "文件读写" }
summary = { en = "Read, write and search files" }   # 可以不写
tools = ["read", "write", "edit"]                    # 下面的工具，可以不写；同一个包里一件只列一次

[connection]                     # 平台接入：只有扩展包能写
platform = "qq"                  # 写法同包的编号

[depends]                        # 缺了就不起的小程序：包编号，不重复
workers = ["embed"]

[recommends]                     # 缺了照起、少一部分本事的小程序
workers = ["embed"]

[worker]                         # 小程序必写，别的种类不能写
program = "miyu-embed"           # 程序名，不带路径分隔符
args = ["serve"]                 # 可以不写
```

一个小程序包的样子：内置语义模型（施工 R-5 三补，`recall.md` 第四条；2026-10-09 项目主人定做成可选的包、只放 bge、出厂不装）。人格记忆的清单写 `[recommends] workers = ["embed"]`，核心照它拉：

```toml
# packages/embed.toml
[package]
kind = "worker"
protocol = [1, 1]
name = { zh = "内置语义模型", en = "Built-in semantic model", ja = "内蔵の意味モデル" }

[worker]
program = "miyu-embed"
```

模型清单、模型文件放在包目录 `packages/embed/` 里：`model.toml`（这个小程序认的名字，小程序清单里不另写）、`model_quantized.onnx`、`vocab.txt`。

每种包能写的表（别的写了报 `wrong_kind`）：

| 种类 | 能写的表 |
|---|---|
| `ui` | `[package]`、`[command]`、`[ui]`、`[check]`、`[settings]`、`[depends]`、`[recommends]` |
| `process` | `[package]`、`[command]`、`[process]`、`[check]`、`[settings]`、`[features]`、`[connection]`、`[depends]`、`[recommends]` |
| `builtin` | `[package]`、`[features]`、`[depends]`、`[recommends]` |
| `worker` | `[package]`、`[worker]` |

**功能**（施工 F-1）：写了 `[features]` 的照写的先后；写了空的 `[features]` 的一个都没有；没写的内置包、扩展包整个算一个，编号、名字、说明照包的，下面的工具不列（都归它，F-3 照这一条挂）。界面、小程序没有功能。两个包的功能编号撞了：照读的先后（出厂的先于家目录，同一层照编号）先到先得，后到的那一份报 `feature_taken`、整份不收，报在那个功能那一行（照包算的那一个没有行号）。依赖的小程序装没装，这一步只读不查（F-2）。

1. `name`、`summary`、`about` 照人格的写法：只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。
2. `[check]` 的程序在标准输出上一行一处、每处一个 JSON，格子照 `check` 的回应（`kind`、`file`、`line`、`column`、`level`、`message`），可以多带 `key`、`rule`、`source`；`file` 照核心的写法：数据根里的写相对数据根的，资源目录里的写真的路径。只有警告时退出码 0，有错误 1（和通讯平台的会话 2026-10-07 对过）。

**配置项**（`[settings.<名字>]`，施工 9-1 下）：

| 格 | 是什么 |
|---|---|
| `type` | 必写：`bool`、`int`（可以带 `min`、`max`，不写是整数的全部范围）、`option`（带 `choices`，至少两个不重复的字）、`text`（可以带 `max`，最多几个字符，不写是 200）、`name`、`url`、`secret`（`{ secret = … }` 那种引用）、`list`（带 `element`，施工 9-1 补） |
| `element` | 只有列表写，必写：元素的类型，上面除 `list` 以外的一种；元素带的 `min`、`max`、`choices`、`max` 写在同一张表里，管每一个元素 |
| `default` | 合这个类型的值；选项要在 `choices` 里；列表写数组、每一个照元素查，空数组也算；`secret`、元素是 `secret` 的列表不能写。没写的没有默认值 |
| `layers` | `system`、`personal` 里的一个或两个；不写是两个。项目配置不给包用 |
| `applies` | `now`、`new_session`、`next_turn`、`program_start`（这个程序下次启动时，协议上写 `head_start`）；不写是 `program_start` |
| `name`、`description` | 语言到一句话，`name` 必写 |
| `hidden` | `true` 的设置页不画：照样能写、能查、进 Schema |

1. 名字：小写字母开头，只有小写字母、数字、`_`，最多 64 个。键是 `<包的编号>.<名字>`，例如 `web.port`。
2. 核心起来时读清单那一次，读成了的清单的配置项接在核心自己的配置项后面，进配置清单：读配置、`miyu check`、`config.schema`、`config.get`、`config.set`、生成的 Schema 和参考文件都照它。经 `package.install`、`package.remove` 装卸以后照新的清单再拼一次（`Builtins::settings`，`miyu-core` 装上），配置服务换上（`Config::refit`）：系统配置、个人设置照手里的字重新认，认得的项、问题变了的推 `config.changed`（`via: package`）；生成的三份照新的清单重写（施工 F-5 补）。卸掉的包的键照旧报不认识。
3. 包的编号是核心自己某一段配置的第一段（`ui`、`persona`、`permission`、`models`、`providers`、`log`、`usage`、`external` 这些，照核心起来时的配置清单认）、又声明了配置项的，这一份报 `settings_taken`，当写错了的列出，配置项一项都不收。
4. 设置页：都在一页 `packages`（「软件包」），一个包一组，组的编号是包的编号、名字是包的名字；每一项的名字、说明用清单里的，照连接的语言挑（这种语言、`en`、`zh`、`ja`），不进 `core/human`。控件照类型：开关 `toggle`、整数 `number`、选项 `select`、列表 `list`，别的 `text`。`config.schema` 里列表照核心自己的列表写：多 `element`，元素是选项的多 `options`。 平台接入的包（写了 `[connection]` 的）不在这一页：它的配置项挂在 `connections`（「接入」）那一页，一个包一组，头单独画（施工 F-4，设计 30 第五节）。核心替内置包声明的配置项（现在是人格记忆的三项）也挂在这一页、这个包那一组；包没装的照样认、照样有最终值，设置页不画（`hidden`，施工 F-4，设计 30 第七节）。

**两层怎么认**：一个包只有一份清单，不像人格那样一层层叠。同一个编号两层都有的，认出厂的，家目录那一份报 `duplicate`。两个包要同一个子命令名的，出厂的先于家目录、同一层照编号，先读到的得，后读到的那一份报 `command_taken`（报在子命令名那一行）。和内置子命令撞的，由 9-2 在命令行那一头拦。声明了系统账号、编号和管理员的账号一样的报 `account_taken`，整份不收（施工 O-4 下）。

**系统账号**（施工 O-4 下，`06-多用户与身份.md` U14、`18-通讯平台.md` Q19）：`[process] system_account = true` 的包有一个系统账号，账号名就是包的编号（一个包一个，编号两层里不重）。核心起来时读清单那一次建它的 `home/<编号>/` 和 `workspace/`（已经有的不动，建不成的记 `WARN system account not prepared`、账号照样算有），拉起扩展前开它自己的会话列表索引（`home/<编号>/index/sessions.db`）；名单照核心手里这时的清单算，不落盘；装上、装回来的当场建，卸掉的不再算，家目录、索引留着、装回来接着用（施工 F-5 下）。核心拉起的这个包的扩展以它的身份连进来（`extensions.md`「握手」），群、陌生人私聊的场所会话归它（`venues.md`）。没有密码、不能登录；人格、预设、软件包照管理员的找，记忆归管理员（`personas.md`「怎么走」第 5 条）。彻底卸载时连数据一起删：随 `12-进程形态与分发.md` R9。

**协议**（`protocol.md`）：`package.list`，不带参数，交回 `{"packages": [...]}`，照编号排（同编号出厂的在前）。

| 格 | 什么时候有 | 是什么 |
|---|---|---|
| `package`、`layer` | 每一项 | 编号；`shipped`、`home` |
| `kind`、`protocol`、`name`、`state` | 读成了的 | `ui`、`process`、`builtin`、`worker`；`[最低, 最高]`；照连接的语言挑的名字（挑法同 `persona.list`：这种语言、`en`、`zh`、`ja`）；放状态的目录的真路径 |
| `version`、`summary` | 写了的 | 原样；照语言挑 |
| `required` | 必需的（施工 F-1） | `true` |
| `features` | `builtin`、`process` 包（施工 F-1） | `[{"id", "name", "summary"}]`：照包算的那一个也列；名字、说明照语言挑，没说明的没有 `summary`；不列工具 |
| `connection` | 写了的（施工 F-1） | `{"platform"}` |
| `depends`、`recommends` | 写了、不空的（施工 F-1） | `{"workers": [...]}` |
| `worker` | `worker` 包（施工 F-1） | `{"program", "args"}` |
| `command` | 有 `[command]` 的 | `{"name", "program", "about"}`，`about` 照语言挑 |
| `opens`、`pages_dir` | `ui` 包；`pages_dir` 写了的 | |
| `process` | `process` 包 | `{"args", "start"}` |
| `check` | 有 `[check]` 的 | `{"args"}` |
| `code`、`problem`、`line` | 写错的、撞了的、读不了的、协议版本对不上的 | 代码；给人看的一句（照连接的语言，`core/human` 的 `package-problems/<code>`，读不了的是 `config/unreadable`）；第几行，有的才有。写错的、撞了的、读不了的只有 `package`、`layer` 和这几格；协议版本对不上的（`code` 是 `protocol_mismatch`）照样带全，头自己决定用不用 |

**`miyu check`**（`cli/check.md`）：多查两层里每一份清单，种类 `package`，写法同人格；协议版本对不上的是警告。写了文件的，某一层 `packages/` 下的 `<编号>.toml` 认作清单（两边换成真的路径比），还没有的报读不了。

### 怎么走

1. **核心起来时读一次**（`Core::new`）：两层的目录读不了的当没有。写错的、撞了的各记一行运行日志 `WARN package invalid package=… file=… error=…`，读不了的 `WARN package unreadable`；照样起来。读完照编进来的内置包那张表标一遍（施工 F-2）：清单是内置包、这一份核心没编进它的代码的，照读坏了的报 `not_built_in`。
   - **内置包照清单启用**（施工 F-2，设计 30 第二节第 3 条）：有读成了的清单、种类是内置的才算装了。装了的才登记：基础系统、人格记忆的工具进工具目录，画 mermaid、联网的查询进查询表；没装的工具、查询都没有（查询照 `unknown_method`）。必需的基础系统没装，记 `WARN required package missing`，照样起来。经 `package.install`、`package.remove` 装卸的当场换（「装卸」）；手改了磁盘上的清单的，照旧要重启核心才认。
2. **读一份**：照「格式」查，报第一处；必写的少了报在表头那一行（缺 `[package]` 的整份，没有行号）。
3. **`package.list`**：照起来时读到的答，名字照这个连接的语言挑。
4. **`miyu check`**：照磁盘上现在的读，改了马上查得出。
5. **转交**（施工 9-2，`cli/main.md`「怎么走」第 0 条）：`miyu <名字> …` 不是内置的子命令，照磁盘读两层的清单，`[command]` 的名字是它的那一个包：程序只找 `miyu` 真实位置旁边的（`miyu_store::packages::locate`；不找 `PATH`，别的程序冒充不了，2026-10-01 项目主人定），参数、环境、标准输入输出原样，Unix 上换成它，Windows 上起它、等它，退出码照它的；`miyu help <名字>` 转成 `--help`。没找到程序说没装、退出码 1。撞了内置子命令的：内置的优先，不转交，帮助页不列；装包时拦随装包那一步。`miyu -h` 多一节「软件包加的命令」。
6. **跑包的检查**（施工 9-2，`crates/miyu-endpoint/src/check/run.rs`）：核心的 `check` 不写文件时，照起来时读到的清单，有 `[check]` 的每个包跑 `<程序> <args…>`（程序同第 5 条找），标准输入是空的、标准错误不要、环境照核心的，最多等 30 秒、收 1 MiB。标准输出一行一个 JSON：`kind`、`file`、`level`（`error`、`warning`）、`message` 必有，`line`、`column`（正整数）、`code`、`key`、`rule`、`source` 有的才收，别的格不收，接在核心自己查的后面，照包的编号的先后。退出码 0、1 是正常的；别的、被信号杀掉的、跑不起来的、到时没完的报一条警告 `check_failed`；程序没找到的报 `check_unavailable`；有看不懂的行的报 `check_output`（几行），都写清单的位置。写了文件的照旧只认核心自己认得出的。
7. **入口**（施工 9-3，`cli/main.md`「怎么走」第 3 条）：不带子命令的 `miyu`、不带子命令的 `miyu config` 在终端里时照配置 `ui.head` 找界面包拉起，`miyu config` 带 `--page config`，要清单的 `[ui] opens` 认 `config` 这一页；`miyu web` 照子命令是 `web` 的那一份找网页软件。程序都只找 `miyu` 旁边的。

### 装卸（施工 F-5 上，设计 `30-插件框架.md` 第九节）

1. **只动管理员家目录那一层**（`miyu_store::packages::install`）：装是把清单拷成 `<编号>.toml`，旁边同名的目录（包自己的文件）拷成 `<编号>/`；先拷到点开头的暂存处再换进去，原来就有的先挪到点开头的备份处，装成了删备份、装不成放回去。卸家目录的是删掉清单和同名目录。卸出厂的是在家目录记一笔 `<编号>.removed`（空文件，像 systemd 的 mask），资源目录不动；装回来是删掉这一笔。
2. **当场生效**：装、卸以后照两层重读、标没编进来的内置包、认配置项撞没撞，换掉核心手里的那一份（`Core::reload_packages`）。`package.list`、预设的功能、新开的会话、开着的会话下一个回合都照新的。内置包的工具照 `Builtins` 端口（`miyu-core` 装上）重新要，新装上的换进工具目录、卸掉的拿掉并记下随包卸掉了（施工 F-5 中，`Catalog::placing`、`removing`）：用过它的会话工具面不变、调到时报「已卸载」，开着的会话下一个回合拿到新装上的包的工具。查询记着属于哪个包，包没装的当没有（`unknown_method`）。扩展进程照装卸前后的清单停下、拉起、升级了的重起，经提供者登记的工具随包卸掉的同样报「已卸载」，声明了系统账号的当场建账号（施工 F-5 下，`extensions.md`「怎么走」第 7 条）；卸是先停用着它的再删文件（施工 F-5 补）。配置项照新的清单当场换（「配置项」第 2 条，施工 F-5 补）。人格记忆装没装最先照新的清单设（`Memory::set_installed`，施工 R-10，`memory.md` 第十一条）：开着的会话照它交不交摘要、抽不抽。本机的向量模型照新的清单当场换（`Builtins::embed` 拼、`Vectors::replace_local` 换，一样的不动，施工 F-5 再补）。这几样走同一个入口（`Core::switch_packages`）：卸包、升级在动文件以前照去掉它的清单换一遍，删不成、换不成的照原来的换回来。
3. **装之前查**：路径要是绝对的 `<编号>.toml`；照规矩读得成；编号不和出厂的撞。拷进去以后照两层重读一遍，这一份撞了别的包（子命令名、功能编号、系统账号）、是核心没编进来的内置包的，撤回（原来那一份放回去）、报 `package_invalid`。
4. **卸之前查**：没装的 `unknown_package`；必需的（基础系统）`package_required`。
5. **卸掉的出厂的**：读两层时不算装了（`Packages::read`）；`package.list` 照样列它，带 `removed: true`（`Packages::read_removed`），好让头给人装回来。
6. 装、卸一次只做一件；只给本机的人用，扩展进程调回 `local_only`；做成了记一行运行日志 `INFO package installed`、`package removed`、`package restored`。

### 出错

| 代码 | 什么时候 |
|---|---|
| `syntax` | 读不成 TOML |
| `unknown_table`、`not_a_table`、`unknown_key` | 不认识的表、该是表的不是表、表里不认识的键 |
| `missing_key` | 少了 `[package]`、`kind`、`protocol`、`name`、`[command]` 的三格；功能的 `name`、`[connection] platform`、小程序的 `[worker]`、`program`（施工 F-1；少了 `[worker]` 的整份，没有行号） |
| `wrong_kind` | 写了这种包不能写的表（「格式」那张表）；`required` 写在不是内置包的清单里（施工 F-1） |
| `needs_command` | `[process]`、`[check]` 没有 `[command]` |
| `bad_kind`、`bad_protocol`、`bad_start` | `kind` 不是 `ui`、`process`、`builtin`、`worker`；`protocol` 不是两个非负整数、最低不大于最高；`start` 不是 `manual`、`always` |
| `not_text`、`not_texts` | `version` 不是字；`args` 不是字的数组 |
| `not_phrases`、`unknown_language`、`empty_phrase` | 「语言到一句话」那一格写错 |
| `bad_command_name`、`bad_program`、`bad_page`、`bad_pages_dir` | 子命令名、程序名、页名、页面目录的写法不对（`pages_dir` 不能是绝对路径、带 `..`、`\`、`:`） |
| `duplicate`、`command_taken`、`account_taken` | 两层同编号的家目录那一份；子命令名被先读到的占了；声明了系统账号、编号是管理员的账号（施工 O-4 下） |
| `bad_setting_name`、`bad_type`、`bad_element`、`bad_default`、`bad_choices`、`bad_range`、`bad_layers`、`bad_applies`、`not_bool` | 配置项写错：名字、类型、列表的元素（写错、写成 `list`、不是字，施工 9-1 补）、默认值、选项、范围、几层、什么时候生效、`hidden`（施工 9-1 下）；`[process] system_account` 不是开关（施工 O-4 下） |
| `settings_taken` | 包的编号和核心自己的配置撞了、又声明了配置项（施工 9-1 下） |
| `bad_capability` | `[process] capabilities` 里有不认识的、重复的名字（施工 9-4 下上） |
| `bad_feature`、`bad_tool`、`bad_platform`、`bad_dependency` | 功能的编号写法不对；功能下的工具名写法不对（英文字母、数字、`_`、`-`，1 到 64 个）、同一个包里列了两次；平台名写法不对；依赖的包编号写法不对、重复（施工 F-1） |
| `feature_taken` | 功能的编号被先读到的包占了（施工 F-1） |
| `not_built_in` | 清单是内置包，这一份核心没编进它的代码（施工 F-2，核心起来时标；`miyu check` 不查） |
| `protocol_mismatch` | 读成了，说的协议版本不包含 1（列表里照样带全；`miyu check` 是警告） |
| `check_failed`、`check_unavailable`、`check_output` | 跑包的检查（施工 9-2，都是警告）：跑坏了、到时没完；程序没找到；印了看不懂的行 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-config/src/package/tests.rs` | 两份样例（终端界面会话给的草稿、桥那种）每一格读对；最小的清单；每一种写错报对代码和行 |
| `crates/miyu-config/src/phrases.rs` 的测试 | 语言到一句话的读法、第一处错 |
| `crates/miyu-store/src/packages/tests.rs` | 两层照编号排、不是 `.toml` 的和编号不合写法的不算；空的；同编号认出厂的；子命令名先到先得、报在那一行；写错的照样列出；状态目录；系统账号照编号、没声明的和写错的不算、撞了管理员的报 `account_taken`（施工 O-4 下）；功能的编号先到先得、照包算的那一个也算（施工 F-1） |
| `crates/miyu-config/src/package/features/tests.rs`（施工 F-1） | 功能照写的先后读对、编号在第几行；没写的照包算一个；空表一个都没有；界面、小程序没有；编号、表、名字、工具的每一种写错；只有内置包、扩展包能写 |
| `crates/miyu-config/src/package/links/tests.rs`（施工 F-1） | 小程序的程序和参数；依赖、推荐；平台接入；必需只给内置包；每种包只能写自己的表；种类四选一 |
| `crates/miyu-config/src/package/settings/tests.rs` | 每一种类型、默认值、几层、什么时候生效、隐藏；每一种写错；拼成配置项的键、类型、界面提示（施工 9-1 下）；列表的每一种元素、默认值一个个查、写错（施工 9-1 补） |
| `crates/miyu-endpoint/tests/package_settings.rs` | 包的配置项进 `config.schema`（「软件包」那一页、这个包那一组、名字说明照语言、隐藏的带标记）；最终值；写错的、写错层的 `check` 报；撞了核心的模块整份不收（施工 9-1 下）；列表进 Schema、读得到、写错 `check` 报（施工 9-1 补） |
| `crates/miyu-cli/src/packages/tests.rs`（施工 9-2） | 只有包的子命令转交（内置的、选项、`help`、不认识的不转）；没装的程序说哪份清单；帮助页多的那一节、接在「命令」后面、照语言 |
| `crates/miyu/tests/packages.rs`（施工 9-2） | 真二进制：参数原样交过去、退出码照它的、`help <名字>` 转成 `--help`、不拉起核心；帮助页列出包的子命令、撞了内置的不列、内置的照旧；没装的程序退出码 1；不认识的照旧退出码 2 |
| `crates/miyu-endpoint/src/check/run/tests.rs`、`tests/package_check.rs`（施工 9-2） | 一行输出收哪几格、哪些不收；0、1 以外的退出码、信号、到时、跑不起来；真的跑 `sh`、到时杀掉；真核心：包报的接在后面，看不懂的行、跑坏了的、程序没找到的各一条警告 |
| `crates/miyu-cli/src/head/tests.rs`、`crates/miyu/tests/heads.rs`（施工 9-3） | 照清单定怎么开（不带参数、带 `--page config`、不认这一页、没装、不是界面、有清单程序不在）、没装的列出装了的（只算程序在旁边的，9-3 补）；真二进制在伪终端里：`miyu`、`miyu config` 拉起清单里的界面、退出码照它的，不认设置页的印帮助，`ui.head` 指着没装的退出码 1；出厂的终端只有清单的说程序不在旁边（9-3 补） |
| `crates/miyu-core/tests/embed_package.rs`、`src/embed/tests.rs`（施工 R-5 三补） | 仓库里的内置语义模型的清单读得成小程序包；出厂的人格记忆推荐它、不依赖它，出厂的资源里没有它；照装了的包拼本机 embedding（`recall.md` 第四条第 1 款） |
| `crates/miyu-core/tests/tools.rs`、`tests/packages.rs`（施工 F-2） | 没装记忆、基础系统的工具目录里没有它们的工具；读坏了的清单不算装了；出厂的内置包清单和编进来的一一对得上；没装画 mermaid、联网的查询是 `unknown_method` |
| `crates/miyu-endpoint/tests/packages_embed.rs`、`crates/miyu-core/src/embed/tests.rs`（施工 F-5 再补） | 真核心装上内置语义模型的包，`config.schema` 里「内置模型」当场写它的模型名，升级成另一个模型的换成新的，卸掉又没了；装卸时拼的和起来时一样、说得出缺的是哪一样，只有起来时记「没装」那一行 |
| `crates/miyu-endpoint/tests/packages_config.rs`、`crates/miyu/tests/packages_live.rs`、`crates/miyu-core/tests/tools.rs`、`crates/miyu-tool/src/catalog/tests.rs`（施工 F-5 补） | 装卸以后配置项当场换（见 `config.md`「守着它的」）；端口交的整份配置清单和起来时读配置用的一样、没装的人格记忆的几项不画；卸掉的提供者装回来登记了不再算卸掉 |
| `crates/miyu-endpoint/tests/packages_extensions.rs`、`tests/system_account.rs`、`src/system_accounts/tests.rs`（施工 F-5 下） | 真核心装上的扩展当场拉起、卸掉的当场停下、旧会话调到它的工具报「已卸载」、升级了的重起、没变的不动；起来以后装上、装回来的声明了系统账号的包当场有账号；再走一遍时开过的索引不再开 |
| `crates/miyu-endpoint/tests/packages_live.rs`（施工 F-5 中） | 真核心卸掉一个内置包：查询当没有、用过它的会话工具面不变、调到报「已卸载」、新开的会话没有；装回来工具、查询都回来 |
| `crates/miyu-endpoint/tests/packages_install.rs`（施工 F-5 上） | 装一份清单、同名目录一起拷、列表和预设的功能当场有；升级换掉、升级撞了放回原来的、不留暂存；写错的、和出厂撞了的、和别的包撞了的不装；卸家目录的删掉；卸出厂的记一笔、列表里标卸掉、装得回来；必需的、没装的不能卸 |
| `crates/miyu-endpoint/tests/packages.rs` | `package.list` 的每一格、照语言挑；施工 F-1 的几格（必需、功能、平台接入、依赖、小程序）；没编进来的内置包报 `not_built_in`、只认读成了的内置包算装了（施工 F-2）；写错的、同编号、撞名、协议版本对不上、`process` 和 `check`；起来时读一次；`check` 查清单、写了文件的认得出、别的文件认不出。只断言出厂的网页和测试自己放的几份，家目录里的编号、子命令名避开出厂会有的（施工 9-1 补：终端界面要出厂 `tui.toml`） |

### 起草时定的

- 清单的读法放在 `miyu-config`：和配置一样是带行号的 TOML，下一步 `[settings]` 也从这里接；「语言到一句话」那一格挪到 `miyu-config` 和人格共用（2026-10-07 主会话）。
- 写错的照样列出、只有编号和问题：头看得到装了一个写错的包（2026-10-07 主会话，照 `persona.list`）。
- 协议版本对不上的照样带全、多 `code`：不一样的头也许还能用一部分，核心不替它拿主意（2026-10-07 主会话）。
- 桥算 `process`，`[process]` 的 `args`、`start` 照通讯平台的会话要的定（`18-通讯平台.md` Q17，2026-10-07）。
- 包的配置项读的时候存自己的一份，只在核心起来时拼成配置项那一次把字留在进程里：配置清单的项是编译期常量的样子，`miyu check` 每次读盘，不能每次都留（2026-10-07 主会话，施工 9-1 下）。
- 设置页一页「软件包」、一个包一组：主菜单不会随装的包越来越长（2026-10-07 和终端界面的会话对过）。
- 列表元素的那一格叫 `element`，和 `config.schema` 里列表那一格一个词；元素带的几格不另开表，写在同一张表里（2026-10-07 主会话，施工 9-1 补）。
- 施工 F-1（2026-10-09 主会话）：每种包能写哪几张表列成一张表（`PackageKind::tables`），写错了统一报 `wrong_kind`，比一张张写判断好查；内置包、小程序先收得紧，要用了再放开。没写 `[features]` 的整个包算一个功能，写了空表的一个都没有：别人的扩展不写也有一个开关，只接平台、不带工具的写空表就不列。`package.list` 的功能不列工具：头画预设页用 `preset.get`（F-3），这里只给人看装了什么。依赖分两种照 Debian 的 Depends、Recommends。功能编号、平台名、依赖的包编号都照包编号的写法（`miyu_config::secret::valid_name`）。
- 施工 F-4（2026-10-09 主会话）：没装的内置包，核心替它声明的配置项照样登记、只是不画：从配置清单里拿掉会让写过它们的配置文件多出「不认识的键」的警告，包装回来又得重新写。别人做的包卸了以后它的键照旧报不认识：核心不知道它曾经有过。
- 施工 F-2（2026-10-09 主会话）：编进来的内置包那张表放在 `miyu-core`：只有它知道 cargo 开关开了哪几个。`miyu check` 不查 `not_built_in`：查清单时不知道是哪一份核心在跑，起来时的运行日志和 `package.list` 已经说了。必需的没装照样起来：工具全没有也能聊天，比起不来好查。联网的清单先不写功能：网络搜索、抓取网页两件工具还没有，写了功能预设里就多两个空开关。

### 还没有的

- 能力的审批（清单多 `capabilities`）、握手时把包自己的配置交给扩展：9-4（下），见 `extensions.md`「还没有的」。
- 写了文件的 `check` 交给包自己的检查：包怎么认自己的文件、怎么交给它，和通讯平台的会话对好再做。
- 锁文件（`07-存储.md` 第二节）；从软件源装（12 第四节）；命令行 `miyu package …`；装卸以后工具、查询、扩展、配置项当场换（F-5 中、下）。
