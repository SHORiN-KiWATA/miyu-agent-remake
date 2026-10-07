## 人格（核心那一半）

### 是什么

人格管「她是谁」：人设、示范对话，以后还有角色扮演提示、声音、记忆的默认范围、能查的知识库（`16-人格与预设.md`）。一个人格是一个目录，三层叠起来：出厂的、系统区的、管理员家目录里的。造会话时照这一刻的文件拼进策略快照，之后钉在会话上（Y5）。

- 出厂只带软件工程师那一句（Y6）。Miyu 是项目主人自己的人格，放在他的家目录里，不进仓库（Y10）。
- 核心不读场所规则：场所会话用哪个人格，由通讯平台的桥照场所规则算好，经 `venue.session` 交来（`18-通讯平台.md` Q1、第四节）。

状态：图纸，施工 P-1（上）做了（2026-10-07 主会话；方向照人格与预设走查，项目主人同一天定；场所会话的人格由桥交来和通讯平台的会话对过；记忆归哪个账号和记忆的会话对过）。角色扮演提示、改了文件下一个回合换上随 P-1（下）；预设随 P-2；新建、改、删、`base` 随 P-3。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-policy/src/persona.rs` | 两份字怎么读：`persona.toml` 的名字、说明、记忆的默认范围（R-3 下），`prompts/examples.md` 的示范对话；写错的写明哪个文件第几行（纯逻辑） |
| `crates/miyu-policy/src/compose.rs`、`snapshot.rs` | 人设进 system 第一块，示范对话进快照的 `demos`，组装时排在 system 后面、历史前面 |
| `crates/miyu-store/src/personas.rs` | 三层在哪、怎么叠、列出编号 |
| `crates/miyu-endpoint/src/personas.rs` | 造会话时照默认找人格、记忆归哪个账号、`persona.list`、`persona.get` |
| `crates/miyu-endpoint/src/settings.rs` | 配置项 `persona.default`（`config.md`） |

### 对外的样子

**目录**：

```text
<资源目录>/personas/<编号>/      出厂的，只读
system/personas/<编号>/          系统区
home/<管理员>/personas/<编号>/   管理员自己的
├── persona.toml                 名字、说明、记忆的默认范围
└── prompts/
    ├── persona.md               人设
    └── examples.md              示范对话
```

1. 编号就是目录名：小写字母开头，小写字母、数字、`-`、`_`，最多 64 个（和配置里的名字一个写法）。
2. 同名的后面的叠在前面的上面：`persona.toml` 逐项盖（同一格里逐种语言盖），`prompts/` 里的文件同名替换、没写的沿用。只有一个空目录也算有这一层。
3. 每样都可以没有：出厂的软件工程师现在只有 `prompts/persona.md`。

**`persona.toml`**：

```toml
[persona]
name = { zh = "美羽", en = "Miyu" }
summary = { en = "My own persona." }

[memory]
scope = "session"
```

两张表，都可以不写：

- `[persona]` 里只有 `name`、`summary`，各是语言到一句话的表（`zh`、`en`、`ja`），话不能是空的（去掉前后空白）。
- `[memory]` 里只有 `scope`：用这个人格的会话，记忆的默认范围，`persona`（跟着人格）或 `session`（只在这个会话里）；`off` 不能写在这里，不记是开会话时的事（施工 R-3 下，`memory.md`「范围」）。不写是 `persona`。上一层写了的盖下面的。

别的表、别的键、别的值报错，写明第几行：`unknown table [<表>]`、`unknown key <表>.<键>`、`memory must be a table`、`memory.scope must be persona or session`。声音、知识库随它们的软件包，到时候再加。

**`prompts/examples.md`**（照旧版 `miyu-dialogs.md` 的写法）：

```text
user: 在吗
assistant: 在

user: 192 乘以 45 呢？
assistant: 8640
```

- `user:` 或 `assistant:` 开头起一条，不分大小写；后面不带开头的行接在这一条后面，空行不算。
- 要从人说的开始、一问一答交替、她答的结束；每一句去掉前后空白，不能是空的。
- 进请求：每轮一条 user、一条 assistant，各一块文字，排在 system 后面、历史前面，算进 `stable`（`kernel/request.md`）。不落日志、不被压缩。

**配置**：`persona.default`，名字，出厂 `engineer`，系统配置、个人设置，以后开的会话照它（`config.md`）。

**协议**（`protocol.md`）：

- `session.create` 的 `persona` 不写的照默认找；`venue.session` 多一格 `persona`，只在造会话时用。
- `persona.list`、`persona.get`；原因码 `persona_invalid`。

### 怎么走

1. **新会话用哪个人格**：开会话时指定的（`session.create` 的 `persona`；场所会话是桥交来的 `venue.session` 的 `persona`），没有就照这一刻的 `persona.default`（个人设置压着系统配置，都没写是 `engineer`）。同一个命令编号再来，照上一次造的那个，不再找。
2. **找**：在三层里照编号找、叠好。编号不合写法：`bad_params`；哪一层都没有：`unknown_persona`，指着没有的默认人格也一样，不悄悄换成别的；文件写错：`persona_invalid`，`data.problem` 写明哪一层、哪个文件第几行（例如 `home prompts/examples.md:2: user and assistant must take turns`）；读不了：内部出错，记一行运行日志。找好了才造会话，什么都没找成的什么都不造。
3. **拼快照**：人设原样进 system 第一块，后面接场所说明、核心的几行（`26-提示词.md` 第四节）；示范对话进快照的 `demos`。没有示范对话的快照里不写这一格，以前造的会话、软件工程师的快照字节和以前一样。
4. **子会话**照执行器填的人格（现在是软件工程师）一样找、叠。
5. **记忆归哪个账号**（和记忆的会话对过，`17-记忆.md` L16；规则在 `Personas::memory_account`，造会话时端点照叠好的人格算好交进 `Create.memory_account`，载入时 `Load.personas` 照快照里的人格再算一遍）：人格住在谁的家目录（有家目录那一层），记忆就归谁；出厂、系统区的人格归会话的属主；属主是系统账号的归管理员（O-4 以后才有）。群里的她和终端里的她因此用同一份记忆，隐私靠每一条的听众过滤。系统账号永远没有自己的记忆。
6. **钉在会话上**：拼好的快照存成 blob，载入照快照，不再读人格目录。文件改了，以后造的会话用新的；已经开着的会话下一个回合换上随 P-1（下）（K3）。
7. **`persona.list`**：几层里所有的编号（目录名不合写法的、不是目录的不算），照编号排，一个一个找；名字、说明照这个连接的语言挑，这种语言没写的照 `en`、`zh`、`ja` 的先后，都没写的是 `null`；写错了的只带 `problem`。
8. **`persona.get {persona}`**：叠好的 `persona.toml` 各种语言原样给，人设、示范对话来自哪一层（没有的是 `null`），示范对话几轮。提示词原文不经协议交出去。

### 出错

| 情形 | 原因码 |
|---|---|
| 编号不合写法 | `bad_params` |
| 哪一层都没有 | `unknown_persona` |
| `persona.toml`、`examples.md` 写错 | `persona_invalid`，`data.problem` |
| 读不了（权限、坏盘） | `internal_error`，运行日志 `WARN persona unreadable` |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-policy/src/persona/tests.rs` | `persona.toml` 三种语言、写错的九种写明第几行（`[memory]` 的三种在内，R-3 下）、`[memory] scope` 两种、不写是没有、上一层盖下面的、读不成 TOML 也说第几行；逐种语言叠；示范对话照旧版写法读（大小写、冒号后的空格、接着的行、空行）、写错的七种写明第几行；示范对话进请求在 system 后面历史前面、`stable` 数对、软件工程师的快照里没有 `demos` |
| `crates/miyu-store/src/personas/tests.rs` | 三层逐项、逐文件叠、来自哪一层；只有出厂、系统区的不住在谁家，空目录也算住在家里；没有的、编号不合写法的、是文件不是目录的；写错的写明哪一层；列编号不重复、照编号排 |
| `crates/miyu-endpoint/src/personas/tests.rs` | 记忆归哪个账号；名字照语言挑、退回的先后 |
| `crates/miyu-endpoint/tests/personas.rs` | 家目录里的人格进 system、示范对话排在前面；不写人格照默认、个人设置压着系统配置；没有的、编号不对的、写错的拒绝，默认人格指着没有的也拒，什么都不造；场所会话带人格造、找回时不看、新场所指着没有的不造；`persona.list`、`persona.get` |

### 起草时定的

- 编号的写法用配置里的名字那一种（带 `_`），和以前资源目录认的一样，出厂的 `engineer` 照旧（2026-10-07 主会话）。
- 读法同配置文件：只有本人写自己的家目录，顺着链接读，不另走安全打开（2026-10-07 主会话）。
- 指着没有的人格不悄悄换成默认：通讯平台出厂的场所规则因此不写 `persona`，主人在自己的规则里写（2026-10-07 和通讯平台的会话定）。
- 不做开场白（2026-10-07 项目主人定，16 Y2）。

### 还没有的

- 角色扮演提示（每 3 轮一条事实，`26-提示词.md` J10）、改了人格文件下一个回合换上（`02-内核.md` K3）：P-1（下）。
- 预设、`unlisted`、装了没开的那一行：P-2。
- 新建、改、删、`base`：P-3。
- 记忆的默认范围、能查的知识库、声音：R-3（下）、知识库、语音。
- 分享给组、给指定的人，系统账号的会话：多用户、O-4。
