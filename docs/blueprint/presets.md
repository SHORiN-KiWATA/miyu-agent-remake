## 预设

### 是什么

预设管「她以什么方式工作」：哪些软件开着、哪几件工具关掉、不指定人格时用哪个人格（`16-人格与预设.md`）。一个预设是一份 TOML 文件，和人格一样三层叠起来：出厂的、系统区的、管理员家目录里的。开会话必须有预设，没指定的照默认（Y12），钉在会话上（Y5）。

状态：图纸，施工 P-2（上）做了（2026-10-08 主会话；方向照人格与预设走查的 C 组、D2 和 Y12，项目主人 2026-10-07 定；出厂编号和默认预设和通讯平台的会话对过）。这一步预设还不改变请求：照预设挑工具面、装了没开的那一行（Y8）、开不开记忆和角色扮演提示随 P-2（中），预设里写配置值（D1）、子代理选预设（C5）随 P-2（下），新建、改、删、`base` 随 P-3。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-policy/src/preset.rs` | 一份预设文件读成样子：每一格的写法，写错的报第一处，带代码、第几行；逐格叠（纯逻辑） |
| `crates/miyu-store/src/presets.rs` | 三层在哪、怎么叠、列出编号、`miyu check` 查每一份 |
| `crates/miyu-store/src/layers.rs` | 人格、预设共用的三层和编号的写法 |
| `crates/miyu-endpoint/src/presets.rs` | 造会话时照默认找、`preset.list`、`preset.get` |
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
4. `[software]` 的键是软件包的编号（基础系统 `basesystem`、联网 `net`、长期目标 `goal`、记忆 `memory` 这些自带的，和清单装的包），写法同包的编号。这一步只查写法，装没装、哪几件工具归它随 P-2（中）。
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
- `preset.list`、`preset.get`；原因码 `unknown_preset`、`preset_invalid`。
- `session.created` 多 `preset`（`kernel/events-bodies.md`）；会话列表、`sessions.changed`、`subscribe` 的回应照它写 `preset`，以前的日志没有的不写。

### 怎么走

1. **新会话用哪个预设**：开会话时指定的（`session.create` 的 `preset`；场所会话是桥照场所规则算好交来的 `venue.session` 的 `preset`），没有就照这一刻的 `preset.default`（个人设置压着系统配置，都没写是 `full`）。同一个命令编号再来，照上一次造的那个，不再找。
2. **找**：在三层里照编号找、叠好。编号不合写法：`bad_params`；哪一层都没有：`unknown_preset`，默认预设指着没有的也一样，不悄悄换成别的（Y12）；文件写错：`preset_invalid`，`data.problem` 写明哪一层、哪个文件第几行（例如 `home dev.toml:3: preset.unlisted must be on or off`）；读不了：内部出错，记一行运行日志。找好了才造会话，什么都没找成的什么都不造。
3. **再找人格**：开会话时指定的人格 → 预设的 `default_persona` → `persona.default`（`personas.md`「怎么走」第 1 条）。预设的默认人格不存在照 `unknown_persona`。
4. **钉在会话上**：`session.created` 记下预设的编号；子会话照父会话的（C5 的默认），父会话是以前造的、没有预设的，子会话也没有。这一步预设不进快照：请求、前缀一个字节不变。
5. **`preset.list`**：几层里所有的编号，照编号排，一个一个找；名字、说明照这个连接的语言挑（挑法同 `persona.list`）；写错了的只带 `problem`。
6. **`preset.get {preset}`**：叠好的各格；`unlisted` 是叠好以后的（几层都没写的是 `on`）；`tools` 写成工具名到 `false`。
7. **`miyu check`**（`cli/check.md`）：每一层里每一份预设各查各的，上面一层盖住了照样报，种类 `preset`；给人看的那一句照 `preset-problems/<code>`。写了文件的，某一层 `presets/` 下的 `<编号>.toml` 认作预设（两边换成真的路径比），还没有的报读不了。

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

### 起草时定的

- 一个预设一份文件，不像人格是目录（16 第三节）：它只是一张开关表（2026-10-08 主会话）。
- `unlisted` 几层都没写的照 `on`（2026-10-08 主会话）：理由见「格式」第 3 条。
- `[tools]` 只收 `false`（走查 C1）；关掉的工具几层叠在一起，上面一层不能把下面关掉的再打开：打开要等单件打开那一步一起定（2026-10-08 主会话）。
- 预设先于人格找：人格要看预设的默认人格（2026-10-08 主会话）。
- 这一步预设不进快照：它还不改变请求，进了反而让以前的快照和新的字节不一样；P-2（中）让它改工具面时一起进（2026-10-08 主会话）。
- 人格、预设的三层抽成 `layers.rs` 共用（2026-10-08 主会话）。

### 还没有的

- 照预设挑工具面、装了没开的那一行（Y8）、开不开记忆和角色扮演提示、预设进快照、改了下一个回合换上（K3）：P-2（中）。
- 预设里写配置值（D1：配置清单每一项标跟着人格、预设还是人）、子代理选人格和预设（C5）：P-2（下）。
- 强调色（`[preset]` 的 `color`）：随 D1 以后。
- 新建、改、删、`base`：P-3。
