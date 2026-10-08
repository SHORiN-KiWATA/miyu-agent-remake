## 软件包清单

### 是什么

一个软件包一份清单：它是什么、哪个程序、给 `miyu` 加哪个子命令、界面认哪几页、核心怎么拉起它、`miyu check` 怎么查它自己的文件（`05-内核接口.md` 第二节、I9）。核心起来时读一次，头经 `package.list` 列得出装了哪些。之后的几步照它走：包的配置项并进配置清单（9-1 下），`miyu <名字>` 转交给包的程序（9-2），界面照清单找（9-3），核心拉起 `process` 包（9-4），预设按包开关（P-2）。

状态：图纸，施工 9-1（上）做了（2026-10-07 主会话；方向是施工方案第三节 9-1 那一行和 I9，项目主人 2026-10-07 定；清单的形状和终端界面、通讯平台的会话对过）。包的配置项施工 9-1（下）做了（2026-10-07 主会话；设置页的挂法和终端界面的会话对过），列表随 9-1（补）（2026-10-07 主会话，通讯平台的会话要的）。转交子命令、跑包的检查随 9-2，照清单找界面随 9-3，拉起 `process` 随 9-4（上）（另见 `extensions.md`）。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-config/src/package.rs`、`package/reader.rs` | 一份清单读成样子：每一格的写法，写错的报第一处，带代码、第几行（纯逻辑）；`reader.rs` 一张表一张表地读 |
| `crates/miyu-config/src/package/settings.rs` | `[settings]` 的读法；拼成配置清单的项（施工 9-1 下） |
| `crates/miyu-config/src/phrases.rs` | 「语言到一句话」那一格的读法，和人格的名字、说明共用 |
| `crates/miyu-store/src/packages.rs` | 两层在哪、读出所有清单、同编号、子命令名撞了；包放状态的目录 |
| `crates/miyu-endpoint/src/packages.rs` | 核心起来时读一次、记运行日志；`package.list`；照核心自己的模块认撞没撞、拼包的配置项（`settle`，施工 9-1 下） |
| `crates/miyu-core/src/settings.rs` | 起来时照清单拼好包的配置项（`Packaged`），读配置、生成 Schema 和参考文件时并进去（施工 9-1 下） |
| `crates/miyu-store/src/human.rs` | 给人看的字并进包的配置项的名字、说明和组名（`Human::with_packages`，施工 9-1 下） |
| `crates/miyu-endpoint/src/check.rs` | `miyu check` 照磁盘查清单 |
| `resources/packages/web.toml` | 出厂的网页界面的清单 |
| `resources/packages/tui.toml` | 出厂的终端界面的清单（9-3 补：终端的会话给的，和 proto 上的一字不差；程序 `miyu-tui` 随 M9）；9-3 再补多配置项 `tui.icons`（图标：`nerd`、`plain`，第一次打开的引导写它） |

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
capabilities = ["network"]       # 要的扩展能力（施工 9-4 下上，extensions.md「能力」）：只认 05 第三节那十二个名字，可以不写

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
2. 核心起来时读清单那一次，读成了的清单的配置项接在核心自己的配置项后面，进配置清单：读配置、`miyu check`、`config.schema`、`config.get`、`config.set`、生成的 Schema 和参考文件都照它。装卸要重启核心。
3. 包的编号是核心自己某一段配置的第一段（`ui`、`persona`、`permission`、`models`、`providers`、`log`、`usage`、`external` 这些，照核心起来时的配置清单认）、又声明了配置项的，这一份报 `settings_taken`，当写错了的列出，配置项一项都不收。
4. 设置页：都在一页 `packages`（「软件包」），一个包一组，组的编号是包的编号、名字是包的名字；每一项的名字、说明用清单里的，照连接的语言挑（这种语言、`en`、`zh`、`ja`），不进 `core/human`。控件照类型：开关 `toggle`、整数 `number`、选项 `select`、列表 `list`，别的 `text`。`config.schema` 里列表照核心自己的列表写：多 `element`，元素是选项的多 `options`。

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
5. **转交**（施工 9-2，`cli/main.md`「怎么走」第 0 条）：`miyu <名字> …` 不是内置的子命令，照磁盘读两层的清单，`[command]` 的名字是它的那一个包：程序只找 `miyu` 真实位置旁边的（`miyu_store::packages::locate`；不找 `PATH`，别的程序冒充不了，2026-10-01 项目主人定），参数、环境、标准输入输出原样，Unix 上换成它，Windows 上起它、等它，退出码照它的；`miyu help <名字>` 转成 `--help`。没找到程序说没装、退出码 1。撞了内置子命令的：内置的优先，不转交，帮助页不列；装包时拦随装包那一步。`miyu -h` 多一节「软件包加的命令」。
6. **跑包的检查**（施工 9-2，`crates/miyu-endpoint/src/check/run.rs`）：核心的 `check` 不写文件时，照起来时读到的清单，有 `[check]` 的每个包跑 `<程序> <args…>`（程序同第 5 条找），标准输入是空的、标准错误不要、环境照核心的，最多等 30 秒、收 1 MiB。标准输出一行一个 JSON：`kind`、`file`、`level`（`error`、`warning`）、`message` 必有，`line`、`column`（正整数）、`code`、`key`、`rule`、`source` 有的才收，别的格不收，接在核心自己查的后面，照包的编号的先后。退出码 0、1 是正常的；别的、被信号杀掉的、跑不起来的、到时没完的报一条警告 `check_failed`；程序没找到的报 `check_unavailable`；有看不懂的行的报 `check_output`（几行），都写清单的位置。写了文件的照旧只认核心自己认得出的。
7. **入口**（施工 9-3，`cli/main.md`「怎么走」第 3 条）：不带子命令的 `miyu`、不带子命令的 `miyu config` 在终端里时照配置 `ui.head` 找界面包拉起，`miyu config` 带 `--page config`，要清单的 `[ui] opens` 认 `config` 这一页；`miyu web` 照子命令是 `web` 的那一份找网页软件。程序都只找 `miyu` 旁边的。

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
| `bad_setting_name`、`bad_type`、`bad_element`、`bad_default`、`bad_choices`、`bad_range`、`bad_layers`、`bad_applies`、`not_bool` | 配置项写错：名字、类型、列表的元素（写错、写成 `list`、不是字，施工 9-1 补）、默认值、选项、范围、几层、什么时候生效、`hidden`（施工 9-1 下） |
| `settings_taken` | 包的编号和核心自己的配置撞了、又声明了配置项（施工 9-1 下） |
| `bad_capability` | `[process] capabilities` 里有不认识的、重复的名字（施工 9-4 下上） |
| `protocol_mismatch` | 读成了，说的协议版本不包含 1（列表里照样带全；`miyu check` 是警告） |
| `check_failed`、`check_unavailable`、`check_output` | 跑包的检查（施工 9-2，都是警告）：跑坏了、到时没完；程序没找到；印了看不懂的行 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-config/src/package/tests.rs` | 两份样例（终端界面会话给的草稿、桥那种）每一格读对；最小的清单；每一种写错报对代码和行 |
| `crates/miyu-config/src/phrases.rs` 的测试 | 语言到一句话的读法、第一处错 |
| `crates/miyu-store/src/packages/tests.rs` | 两层照编号排、不是 `.toml` 的和编号不合写法的不算；空的；同编号认出厂的；子命令名先到先得、报在那一行；写错的照样列出；状态目录 |
| `crates/miyu-config/src/package/settings/tests.rs` | 每一种类型、默认值、几层、什么时候生效、隐藏；每一种写错；拼成配置项的键、类型、界面提示（施工 9-1 下）；列表的每一种元素、默认值一个个查、写错（施工 9-1 补） |
| `crates/miyu-endpoint/tests/package_settings.rs` | 包的配置项进 `config.schema`（「软件包」那一页、这个包那一组、名字说明照语言、隐藏的带标记）；最终值；写错的、写错层的 `check` 报；撞了核心的模块整份不收（施工 9-1 下）；列表进 Schema、读得到、写错 `check` 报（施工 9-1 补） |
| `crates/miyu-cli/src/packages/tests.rs`（施工 9-2） | 只有包的子命令转交（内置的、选项、`help`、不认识的不转）；没装的程序说哪份清单；帮助页多的那一节、接在「命令」后面、照语言 |
| `crates/miyu/tests/packages.rs`（施工 9-2） | 真二进制：参数原样交过去、退出码照它的、`help <名字>` 转成 `--help`、不拉起核心；帮助页列出包的子命令、撞了内置的不列、内置的照旧；没装的程序退出码 1；不认识的照旧退出码 2 |
| `crates/miyu-endpoint/src/check/run/tests.rs`、`tests/package_check.rs`（施工 9-2） | 一行输出收哪几格、哪些不收；0、1 以外的退出码、信号、到时、跑不起来；真的跑 `sh`、到时杀掉；真核心：包报的接在后面，看不懂的行、跑坏了的、程序没找到的各一条警告 |
| `crates/miyu-cli/src/head/tests.rs`、`crates/miyu/tests/heads.rs`（施工 9-3） | 照清单定怎么开（不带参数、带 `--page config`、不认这一页、没装、不是界面、有清单程序不在）、没装的列出装了的（只算程序在旁边的，9-3 补）；真二进制在伪终端里：`miyu`、`miyu config` 拉起清单里的界面、退出码照它的，不认设置页的印帮助，`ui.head` 指着没装的退出码 1；出厂的终端只有清单的说程序不在旁边（9-3 补） |
| `crates/miyu-endpoint/tests/packages.rs` | `package.list` 的每一格、照语言挑；写错的、同编号、撞名、协议版本对不上、`process` 和 `check`；起来时读一次；`check` 查清单、写了文件的认得出、别的文件认不出。只断言出厂的网页和测试自己放的几份，家目录里的编号、子命令名避开出厂会有的（施工 9-1 补：终端界面要出厂 `tui.toml`） |

### 起草时定的

- 清单的读法放在 `miyu-config`：和配置一样是带行号的 TOML，下一步 `[settings]` 也从这里接；「语言到一句话」那一格挪到 `miyu-config` 和人格共用（2026-10-07 主会话）。
- 写错的照样列出、只有编号和问题：头看得到装了一个写错的包（2026-10-07 主会话，照 `persona.list`）。
- 协议版本对不上的照样带全、多 `code`：不一样的头也许还能用一部分，核心不替它拿主意（2026-10-07 主会话）。
- 桥算 `process`，`[process]` 的 `args`、`start` 照通讯平台的会话要的定（`18-通讯平台.md` Q17，2026-10-07）。
- 包的配置项读的时候存自己的一份，只在核心起来时拼成配置项那一次把字留在进程里：配置清单的项是编译期常量的样子，`miyu check` 每次读盘，不能每次都留（2026-10-07 主会话，施工 9-1 下）。
- 设置页一页「软件包」、一个包一组：主菜单不会随装的包越来越长（2026-10-07 和终端界面的会话对过）。
- 列表元素的那一格叫 `element`，和 `config.schema` 里列表那一格一个词；元素带的几格不另开表，写在同一张表里（2026-10-07 主会话，施工 9-1 补）。

### 还没有的

- 能力的审批（清单多 `capabilities`）、握手时把包自己的配置交给扩展：9-4（下），见 `extensions.md`「还没有的」。
- 写了文件的 `check` 交给包自己的检查：包怎么认自己的文件、怎么交给它，和通讯平台的会话对好再做。
- 装包、卸包、锁文件（`07-存储.md` 第二节）。
