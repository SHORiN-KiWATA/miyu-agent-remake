## 预设

### 是什么

预设管「她以什么方式工作」：哪些软件开着、哪几件工具关掉、不指定人格时用哪个人格（`16-人格与预设.md`）。一个预设是一份 TOML 文件，和人格一样三层叠起来：出厂的、系统区的、管理员家目录里的。开会话必须有预设，没指定的照默认（Y12），钉在会话上（Y5）。

状态：图纸，施工 P-2（上）做了（2026-10-08 主会话；方向照人格与预设走查的 C 组、D2 和 Y12，项目主人 2026-10-07 定；出厂编号和默认预设和通讯平台的会话对过）。照预设挑工具面、装了没开的那一行（Y8）、开不开记忆和角色扮演施工 P-2（中）做了（2026-10-08，走查 C1、C4、E2）。改了预设的文件开着的会话下一个回合换上施工 P-2（下）做了（2026-10-08，K3）。预设里写配置值（D1）、子代理选人格和预设（C5）随后面两步，新建、改、删、`base` 随 P-3。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-policy/src/preset.rs` | 一份预设文件读成样子：每一格的写法，写错的报第一处，带代码、第几行；逐格叠（纯逻辑） |
| `crates/miyu-store/src/presets.rs` | 三层在哪、怎么叠、列出编号、`miyu check` 查每一份 |
| `crates/miyu-store/src/layers.rs` | 人格、预设共用的三层和编号的写法 |
| `crates/miyu-endpoint/src/presets.rs` | 造会话时照默认找、算装了哪些软件和没开的、`preset.list`、`preset.get` |
| `crates/miyu-tool/src/catalog.rs` | 工具目录按包登记，记下每件工具归哪个包（施工 P-2 中） |
| `crates/miyu-session/src/agents.rs`、`open.rs` | 造会话时照预设筛工具面、定记忆的范围（施工 P-2 中） |
| `crates/miyu-policy/src/compose.rs` | `with_preset`：快照记预设、去掉角色扮演提示、写装了没开的那一行（施工 P-2 中） |
| `resources/core/preset-off.txt` | 装了没开的那一行（给模型看，登记在 `26-提示词.md` 第十节） |
| `crates/miyu-endpoint/src/settings.rs` | 配置项 `preset.default` |
| `resources/presets/full.toml`、`dev.toml` | 出厂的两个 |

### 对外的样子

**在哪**：

```text
<资源目录>/presets/<编号>.toml        出厂的，只读
system/presets/<编号>.toml            系统区
home/<管理员>/presets/<编号>.toml      管理员自己的
```

1. 编号就是文件名，写法同人格的编号（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个）。不合写法的、不是 `.toml` 的、是目录的不算。
2. 同名的后面的叠在前面的上面，逐格盖：`[preset]` 逐格（`name`、`summary` 逐种语言），`[software]` 逐个键，关掉的工具叠在一起。空文件也算有这一层。

**格式**（只收下面这些，不认识的表、键报错，写明第几行）：

```toml
[preset]
name = { zh = "开发", en = "Dev", ja = "開発" }
summary = { zh = "只开写代码必需的", en = "Only what coding needs" }
default_persona = "engineer"   # 不指定人格时用哪个人格
unlisted = "off"               # 没列在 [software] 里的软件开不开：on、off

[software]                     # 软件包的编号 = 开不开
basesystem = true
net = true
goal = true

[tools]                        # 在开着的包里关掉单件工具：只能写 false
shell = false
```

1. `name`、`summary` 照人格的写法：只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。
2. `default_persona` 是人格的编号，这里只查写法，在不在开会话时查。
3. `unlisted`：几层都没写的照 `on`。功能全开是默认，`[tools]` 里只关一两件的写法也是建在「其余都开」上的。
4. `[software]` 的键是软件的编号，写法同包的编号。现在装了的：基础系统 `basesystem`、记忆 `memory`（三件工具和回合开始的召回）、角色扮演 `roleplay`（人格的角色扮演提示和风格锁），和清单装的 `process` 包（桥）；联网 `net`、长期目标 `goal` 等它们做出来。写了没装的不报错（这台机器上以后可能装），`preset.get` 写进 `missing`。
5. `[tools]` 的键是工具名：字母、数字、`-`、`_`，最多 64 个。单件打开某个包里的一件先不做（走查 C1），写 `true` 报错。
6. 权限级别、压缩模板、强调色这些配置值随 P-2（下）（D1、C3）。

**出厂的两个**（Y6、Y7，`10-自带软件.md` 第四节）：

| 编号 | 名字 | 内容 |
|---|---|---|
| `full` | 功能全开 | `unlisted = "on"`：装了的全开，以后新装的也开。`preset.default` 出厂是它 |
| `dev` | 开发 | `unlisted = "off"`，开 `basesystem`、`net`、`goal`；默认人格 `engineer` |

**配置**：`preset.default`，名字，出厂 `full`，系统配置、个人设置，以后开的会话照它（`config.md`）。设置页在「通用」那一页的「预设」一组。

**协议**（`protocol.md`）：

- `session.create`、`venue.session` 多 `preset`：不写的照默认找；`venue.session` 找回已有的会话时不看。
- `preset.list`、`preset.get`（施工 P-2 中起多 `missing`）；原因码 `unknown_preset`、`preset_invalid`。
- `session.created` 多 `preset`（`kernel/events-bodies.md`）；会话列表、`sessions.changed`、`subscribe` 的回应照它写 `preset`，以前的日志没有的不写。

### 怎么走

1. **新会话用哪个预设**：开会话时指定的（`session.create` 的 `preset`；场所会话是桥照场所规则算好交来的 `venue.session` 的 `preset`），没有就照这一刻的 `preset.default`（个人设置压着系统配置，都没写是 `full`）。同一个命令编号再来，照上一次造的那个，不再找。
2. **找**：在三层里照编号找、叠好。编号不合写法：`bad_params`；哪一层都没有：`unknown_preset`，默认预设指着没有的也一样，不悄悄换成别的（Y12）；文件写错：`preset_invalid`，`data.problem` 写明哪一层、哪个文件第几行（例如 `home dev.toml:3: preset.unlisted must be on or off`）；读不了：内部出错，记一行运行日志。找好了才造会话，什么都没找成的什么都不造。
3. **再找人格**：开会话时指定的人格 → 预设的 `default_persona` → `persona.default`（`personas.md`「怎么走」第 1 条）。预设的默认人格不存在照 `unknown_persona`。
4. **钉在会话上**：`session.created` 记下预设的编号；子会话照父会话的（C5 的默认）：照编号重新找，找不到、写错了的不派，同人格；父会话是以前造的、没有预设的，子会话也没有。
5. **照预设挑**（施工 P-2 中）：
   - 装了的软件：工具目录里有工具的包（按包登记，`Catalog::in_packages`；只交一串工具的老写法全算 `basesystem`）、`roleplay`、清单装的 `process` 包。`ui` 包是头，不算。
   - 一个软件开着：`[software]` 写了照写的，没写的照 `unlisted`（`PresetFile::opens`）。
   - 工具面：开着的包里的工具，减去 `[tools]` 关掉的（`PresetFile::keeps`），再照原来的几道筛（场所、子会话、能不能确认）。
   - 记忆：`memory` 没开的，范围一律 `off`，不管 `session.create` 的 `memory`（走查 E2：开不开归预设，范围归开会话时）。
   - 角色扮演：`roleplay` 没开的，快照里没有角色扮演提示、system 没有风格锁。
   - 装了没开的那一行（Y8，走查 C4）：装了、没开的软件（`roleplay` 除外：它没有工具，列出来反倒像是不许演），照编号逗号隔开，`core/preset-off.txt` 写成一行接在核心的几行后面、风格锁前面。都开着的不写，功能全开的会话 system、工具面和以前一个字节都不差。
   - 快照记 `preset`：编号、没开的软件和文件的指纹（`policy.md`「拼」第 6 条）。
6. **改了文件**（施工 P-2 下，K3）：每个回合开始，执行器照快照里预设的编号把几层重新找一遍，指纹或者没开的那几个变了的照新的重拼，下一轮用上，日志里一条内核记的 `session.policy_changed`（`session/actor.md`「换快照」，人格、预设一起改了的换一次）：
   - 工具面照新的预设重新筛；以前就有的那几件照旧快照里的原样（描述、`subagent` 能选的池都不跟着变），新打开的照现在的目录拿。
   - 记忆不跟着换：开不开记忆在开会话时定（L3），新预设里 `memory` 改了也照开会话时的。
   - 角色扮演、装了没开的那一行照新的重新算。
   - 找不着、写错了的照旧用原来的，记一行运行日志；P-2（中）造的快照没有指纹，不换。撤掉换快照那一轮不换回去。
7. **`preset.list`**：几层里所有的编号，照编号排，一个一个找；名字、说明照这个连接的语言挑（挑法同 `persona.list`）；写错了的只带 `problem`。
8. **`preset.get {preset}`**：叠好的各格；`unlisted` 是叠好以后的（几层都没写的是 `on`）；`tools` 写成工具名到 `false`；`missing` 是 `[software]` 里写了、这台机器上没装的，照编号排（施工 P-2 中；界面照它写「没安装」，16 第五节）。
9. **`miyu check`**（`cli/check.md`）：每一层里每一份预设各查各的，上面一层盖住了照样报，种类 `preset`；给人看的那一句照 `preset-problems/<code>`。写了文件的，某一层 `presets/` 下的 `<编号>.toml` 认作预设（两边换成真的路径比），还没有的报读不了。

### 出错

| 代码 | 什么时候 |
|---|---|
| `syntax` | 读不成 TOML |
| `unknown_table`、`not_a_table`、`unknown_key` | `[preset]`、`[software]`、`[tools]` 以外的表；这三样不是表；`[preset]` 里别的键 |
| `not_phrases`、`unknown_language`、`empty_phrase` | 「语言到一句话」那一格写错 |
| `bad_persona`、`bad_unlisted` | `default_persona` 不是合写法的编号；`unlisted` 不是 `on`、`off` |
| `bad_software`、`not_bool` | `[software]` 的键不合包编号的写法；值不是开关 |
| `bad_tool`、`not_false` | `[tools]` 的键不是工具名的写法；值不是 `false` |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-policy/src/preset/tests.rs` | 每一格读对；空文件什么都没有、`unlisted` 照 `on`；逐格叠；每一种写错报对代码和行；工具名的写法 |
| `crates/miyu-store/src/presets/tests.rs` | 三层逐格叠；一层也行、空文件也算；没有的、编号不合写法的、写错的写明哪一层；编号照文件名、不重复、照编号排；`check` 每一层各查、写了文件的认得出；出厂两份读得出、零问题、三种语言齐；每一种代码三种语言都有给人看的一句 |
| `crates/miyu-store/src/index/tests.rs` | 索引里存得下、读得回人格和预设 |
| `crates/miyu-endpoint/tests/presets.rs` | 不写预设照默认、个人设置压着系统配置、指定的压着默认；找人格的先后；没有的、编号不对的、写错的拒绝、什么都不造，默认预设指着没有的也拒；会话列表、`subscribe` 写 `preset`，以前的日志不写；`venue.session` 带预设造、找回时不看；`preset.list`、`preset.get`；`check` 查预设 |
| `crates/miyu-session/tests/spawn.rs` | 子会话照父会话的预设，父会话载入以后也照 |
| `crates/miyu-endpoint/tests/preset_face.rs`（施工 P-2 中） | 功能全开的工具面全有、没有那一行；开发预设没有记忆三件、范围 `off`（要了也没用）、那一行只写 `memory`；单件关掉的没有、不写那一行；整包关掉的工具全没、写进那一行；角色扮演开着的有提示和风格锁、那一行在风格锁前面，关着的都没有；`preset.get` 的 `missing` |
| `crates/miyu-tool/src/catalog/tests.rs` | 按包登记、以前的名字照现在的包、老写法算基础系统、两个包里同名的照样拒 |
| `crates/miyu-policy/src/preset/tests.rs` | 软件开不开、单件留不留；没开的照编号、不重复 |
| `crates/miyu-session/src/actor/persona/tests.rs` | 角色扮演没开的换人格以后照旧没有提示和风格锁，换过以后不再换 |
| `crates/miyu-session/src/actor/persona/preset_tests.rs`（施工 P-2 下） | 改了预设重新筛工具面、留着的那一件是快照里的原样、换过以后不再换、整包关掉的写进那一行、重新打开的照现在的目录拿；记忆照开会话时的（开着的照开，关着的照关）；角色扮演打开了提示回来；写错了的、没有指纹的不换 |
| `crates/miyu-endpoint/tests/preset_swap.rs`（施工 P-2 下） | 真核心：改了预设下一轮的工具面变、日志一条内核记的换快照；没改的不换；写错了的不换；重启以后照换上的那一份，载入的会话改了也换 |

### 起草时定的

- 一个预设一份文件，不像人格是目录（16 第三节）：它只是一张开关表（2026-10-08 主会话）。
- `unlisted` 几层都没写的照 `on`（2026-10-08 主会话）：理由见「格式」第 3 条。
- `[tools]` 只收 `false`（走查 C1）；关掉的工具几层叠在一起，上面一层不能把下面关掉的再打开：打开要等单件打开那一步一起定（2026-10-08 主会话）。
- 预设先于人格找：人格要看预设的默认人格（2026-10-08 主会话）。
- P-2（上）预设不进快照：它还不改变请求；P-2（中）让它改工具面时一起进（2026-10-08 主会话）。
- 角色扮演算一个软件、开发预设不开（16 第八节），但不进装了没开的那一行：它没有工具，她不会去用它，列出来反倒像是不许演（2026-10-08 主会话）。
- 记忆没开的，范围一律 `off`：开不开归预设、范围归开会话时（走查 E2）；快照里的范围就是照它算好的，载入不用再看预设（2026-10-08 主会话）。
- 装了没开的那一行只陈述是什么、不写该怎么做（26 第十节的规矩）：没开的原因摆在那里，她自己会说换预设（2026-10-08 主会话）。
- 预设里单独关掉的工具哪里都不出现：不在工具列表里，也不写进装了没开的那一行（2026-10-08 项目主人定）。真模型实测时关了 `shell`，她把跑不了命令猜成了权限问题，主会话问过要不要也写进那一行，项目主人定不写。
- 人格、预设的三层抽成 `layers.rs` 共用（2026-10-08 主会话）。
- 换预设时以前就有的工具照旧快照里的原样，新打开的照现在的目录拿：留着的不跟着程序升级变，`swappable` 就不用比工具面；新打开的本来就没有旧的一份（2026-10-08 主会话，施工 P-2 下）。
- 换预设时记忆照开会话时的：范围钉在会话上（L3），记忆开着的会话中途关掉，回合库、召回都不好收场（2026-10-08 主会话，施工 P-2 下）。

### 还没有的

- 预设里写配置值（D1：配置清单每一项标跟着人格、预设还是人）、子代理选人格和预设（C5）：后面两步。
- 强调色（`[preset]` 的 `color`）：随 D1 以后。
- 新建、改、删、`base`：P-3。
