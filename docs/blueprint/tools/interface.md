## 工具的接口

### 是什么

工具是内核之外的软件，照同一个接口来：每件工具报出自己的规格，核心起来时登记进工具目录，登记完就冻结。一次调用交给工具修正过的参数、这一轮的工作目录、家目录、数据根、她看过的文件；工具交回给模型看的内容块、出没出错、给人看的说法、效果。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-tool/src/lib.rs` | 规格 `Spec`、接口 `Tool` |
| `crates/miyu-tool/src/run.rs` | 一次调用：`Call`、`Seen`、`Target`、`Done`、`Effect`、`Progress`、`Running` |
| `crates/miyu-tool/src/catalog.rs` | 工具目录，登记时查的三条 |
| `crates/miyu-tool/src/testkit.rs` | 测试用的假工具（`testkit` 开关打开时才编） |
| `crates/miyu-core/src/lib.rs` | `tools()`：核心起来时登记基础系统 |
| `crates/miyu-session/src/open.rs` | 造会话时照目录把工具面写进策略快照 |
| `crates/miyu-session/src/tools.rs` | 执行工具的端口：造 `Call`、跑、量用时、叫停、没有的、崩了的 |
| `crates/miyu-session/src/effects.rs` | 效果存成 blob、换成内核的效果；她看过的 |
| `crates/miyu-session/src/guard.rs` | 权限策略：照 `targets` 判 |
| `crates/miyu-policy/src/tools.rs` | 快照里的工具面；执行器替工具写的两句 |
| `crates/miyu-store/src/human.rs` | 给人看的字：读 `human/<语言>.json`，把说法换成字 |
| `resources/core/tool-results/unavailable.txt`、`crashed.txt` | 执行器替工具写的两句 |

### 对外的样子

**规格** `Spec`：

| 格 | 是什么 |
|---|---|
| `name` | 工具名，模型照它调：只用 ASCII 字母、数字、`_`、`-`，1 到 64 个字节 |
| `description` | 给模型看的说明，英文，原样进 tools 数组 |
| `parameters` | 参数的 JSON Schema，原样进 tools 数组，一个字节不改（`RawJson`）；必须是 `{"type":"object",…}` |
| `access` | 访问类别：`read`、`write`、`execute`、`network`、`outbound`；不认识的原样留着，按最严的算 |

**接口** `Tool`（`Send + Sync`）：

| 方法 | 做什么 |
|---|---|
| `spec()` | 交出规格 |
| `targets(&Call)` | 这次调用要碰的路径、是读是写；默认一条都没有 |
| `run(Call, Progress)` | 执行一次调用，交回 `Running`：一个交回 `Done` 的 future |

**一次调用交给工具的** `Call`：

| 格 | 是什么 |
|---|---|
| `args` | 修正过的参数：一个 JSON 对象的原文 |
| `cwd` | 这一轮的工作目录：回合开始时的那一个，原样的字 |
| `home` | 系统的家目录；读不出来的是空的 |
| `data_root` | Miyu 的数据根；不知道的是空的（会话里总有） |
| `seen` | 她这个会话里看过的文件（`Arc<Seen>`） |

- `Seen`：换成真实位置以后的路径 → 她最后一次看到的整份文件的内容哈希（`sha256:` 加 64 位小写十六进制）。
- `Target`：`path` 是她给的原样，`write` 是真的就是要写（新建、改、删），不是就是读。

**工具交回的** `Done`：

| 格 | 是什么 |
|---|---|
| `error` | 出错了没有：工具执行了，但是出了错；错在哪写在内容块里给她看 |
| `blocks` | 给模型看的内容块 |
| `human` | 给人看的说法（`Said`：编号 `key`、字段 `fields`）；没交的是空的 |
| `effects` | 效果，照先后 |

- `Done::ok(字)`、`Done::error(字)`：一段字的内容块，`error` 各是假、真。
- `.said(说法)`：带上给人看的说法，再调一次就换成新的那一个；`.effect(效果)`：再报一样效果，接在后面。

**效果** `Effect`：路径都是换成真实位置以后的。

| 种类 | 格 | 进内核以后 |
|---|---|---|
| `Read` | `path`；`lines`：读了第几行到第几行（从 1 数起，含两头），一行都没显示的是空的；`hash`：整份文件的内容哈希 | `file.read` |
| `Changed` | `path`；`before`：改前的内容本身，新建的是空的；`after`：改后的内容本身 | `file.changed`，内容换成 blob 的哈希 |
| `Trashed` | `path`：移走之前的位置；`trash`：回收站里的位置，各平台自己的写法 | `file.trashed` |

**执行中的输出** `Progress`：`Progress::new(收的那一头)`，`push(一段字)`。

**工具目录** `Catalog`：`Catalog::new(几件)` 登记，`specs()` 照名字的先后交出每件的规格，`get(名字)` 找那一件；`Catalog::default()` 是空的；`Debug` 写成名字的列表。登记不上是 `CatalogError`：哪一件（`tool`）、哪一条（`problem`）。

### 怎么走

#### 一、登记

1. 核心起来时（`miyu-core` 的 `tools()`）：照资源目录造出基础系统的七件（`tools/read.md` 等），交给 `Catalog::new`。字读不出来、写法不对，或者登记查不过：核心起不来，说是哪一份、哪一件、哪一条。
2. 照交进来的先后一件件查，每件依次查三条，有一件不过，整个目录登记不上，报排在前面的那一件：
   1. 名字：1 到 64 个字节，只用 ASCII 字母、数字、`_`、`-`。不合的，每次请求都会被供应商拒收。
   2. 参数格式：读得成 JSON，顶层的 `type` 是字符串 `"object"`。`{"type":["object","null"]}`、没有 `type` 的都不算。
   3. 已经有一件同名的：她调的是哪一件，说不清。
3. 目录照名字排，交进来的先后不影响。登记完就冻结，核心跑着的时候不变。现在没有预设，目录里的全开。
4. 造会话时（`crates/miyu-session/src/open.rs`）：每件的名字、说明、参数格式、访问类别写进策略快照，照名字排（`crates/miyu-policy/src/tools.rs`）。这个会话以后一直照快照发，核心换了目录也不变。

#### 二、一次调用

1. 内核先查（`kernel/`）：工具面上没有这个名字的、参数不是 JSON 对象的，当场记出错的结果；只读时写文件的（访问类别是 `write` 和不认识的）当场拦下。别的照参数格式修正参数：被写成字符串的数组、对象、整数、数字、布尔还原回去，声明成字符串的一个字节不碰，什么都没写的当成 `{}`。
2. 轮到的先过权限策略（`crates/miyu-session/src/guard.rs`）：目录里没有这件工具的放行（执行时报用不了）；照 `targets` 报的路径判；一条都不报的，照访问类别判。交给 `targets` 的 `Call` 里 `seen` 是空的：报路径只看参数。
3. 派出去（`crates/miyu-session/src/tools.rs`）：
   1. 造 `Call`：`args` 是修正过的参数；`cwd` 是回合开始时的工作目录；`home` 是核心起来时读的系统家目录；`data_root` 是数据根；`seen` 是这个会话她看过的文件，共享一份。
   2. 快照里有、核心的目录里没有这件（核心升级拿掉了，老会话照样调）：不派，当场交回出错的结果（下面「执行器替工具写的两句」），没有用时。
   3. 在自己的任务里跑 `run` 交回的 future，记下开始跑的那一刻。
   4. `push` 的每一段，这次调用还在跑的，送回会话，推给头（瞬时的 `tool.progress`），不落盘；叫停了的不理。
   5. 跑完：效果里改前改后的内容在阻塞线程里存成 blob（这个账号的 `blobs/`，`store.md`），换成哈希；存不下来的（磁盘满了之类）照样算出哈希，写一条运行日志。先照效果记下她看过的，再把 `blocks`、`error`、`human` 原样交给内核，用时是从开始跑到这里的毫秒数。
   6. 工具 panic（存 blob 那一步 panic 的也算）：交回出错的结果，带用时；会话照常往下走。
4. 内核把它记成 `tool.result`：`error` 是真的，状态是 `error`，不然是 `ok`；`by` 是那次调用。`blocks` 给模型看，`human`、`effects` 不发给模型。
5. 一起跑的（内核照访问类别定）：连着的只读调用一起派，各跑各的任务；不是只读的，等它前面的都有了结果才派，它没结果，后面的都等着。
6. 叫停：掐掉跑它的那个任务，也就是丢掉 future；之后什么都不再报。不另给工具取消信号，工具靠 future 被丢掉收拾：
   - `shell` 整组杀掉命令（`tools/shell.md`）。
   - 在阻塞线程里干活的，future 被丢掉时举一面旗；`glob`、`grep` 走目录、搜文件的每一步看一眼，举了就不往下走。
   - 不看旗的活（`read`、`write`、`edit`、`trash`）在阻塞线程里干完为止，结果没人要了。
   - 会话停了，在跑的都掐掉。

#### 三、要碰的路径

1. 只看参数，不碰磁盘；参数不对的交回空的，执行时再报错。
2. 换成真实的位置、查边界、判放行问人还是拒绝，是权限策略的事（`fs.md`），工具不自己判。
3. 每件报什么：

   | 工具 | 报的 |
   |---|---|
   | `read` | `file_path`，读 |
   | `glob` | 搜的目录，读（`tools/glob.md`） |
   | `grep` | `path`，没给的是 `.`，读 |
   | `write`、`edit`、`trash` | `file_path`，写 |
   | `shell` | 一条都不报 |

#### 四、效果和她看过的

1. 执行器照工具交回的先后，把效果换成内核的：`Read` → `file.read`；`Changed` → `file.changed`，改前（有的话）、改后各存一份 blob，记哈希；`Trashed` → `file.trashed`。路径写成字。
2. 照内核的效果记下她看过的：`file.read` 记读的时候整份的哈希；`file.changed` 记改后的哈希；`file.trashed` 从里面拿掉；不认识的种类不管。
3. 新会话她看过的是空的；载入时照日志里每一条 `tool.result` 的效果照先后重建，现在还撤着的回合里的不算；撤销、恢复落了盘以后，照日志重算一遍。在跑的调用拿着的是交给它时的那一份。
4. 谁用：`write`、`edit` 改一个已经在了的文件之前照它核对（`tools/write.md`）。

#### 五、给人看的说法

1. 说法是一个编号加几个字段，字段的值都是字符串。基础系统的编号是 `software/basesystem/<名字>`；内核和执行器写的以 `core/` 开头。
2. 记进 `tool.result` 的 `human`，不发给模型，前缀不受影响；老日志里没有这一格。
3. 头照自己的界面语言换成字（`crates/miyu-store/src/human.rs`）：
   - 字在内核的 `resources/core/human/{zh,en}.json` 和每个软件包自己的 `human/` 下（基础系统的是 `resources/software/basesystem/human/{zh,en}.json`），一种语言一份，先有 `zh`、`en`。每份两样：`tools` 是每件工具的显示名 `name`、显示名后面跟哪个参数的值 `subject`；`said` 是每一种说法的模板，编号照这一份所在的地方往下写。
   - 哪一份没有这种语言的，照英文那一份；英文也没有，那一处没有字。
   - 换进去的字段不转义，控制字符换成 `�`。模板要的字段说法里没有的，这一句换不出字。
4. `miyu ask` 怎么印：`cli/ask.md`「每一步那一行」。

#### 六、执行器替工具写的两句

造会话、载入会话时从快照里拿（`crates/miyu-policy/src/tools.rs`），字段 `name` 是她调的工具名，照模板的规矩转义。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 快照里有、核心的目录里没有 | `The tool "{name}" is not available right now.` | `core/tool-results/unavailable`，字段 `name` |
| 工具崩了 | `The tool "{name}" stopped because of an internal error. It may have been partly done.` | `core/tool-results/crashed`，字段 `name` |

每一份以一个换行结尾；登记在 `26-提示词.md` 第十节。

### 出错

登记不上时写成的字（`CatalogError`，名字照 Rust 的写法带引号）：

| 哪一条 | 写成 |
|---|---|
| 名字 | `tool "<名字>": the name must be 1 to 64 ASCII letters, digits, '_' or '-'` |
| 参数格式 | `tool "<名字>": the parameters must be a JSON Schema of type "object"` |
| 同名 | `tool "<名字>": another tool has the same name` |

运行日志（target `miyu::session`，记在会话的 span 里）：

| 什么时候 | 级别 | 那一行 |
|---|---|---|
| 开始跑 | INFO | `running`，带 `call`、`tool` |
| 跑完 | INFO | `ran`，带 `call`、`took_ms`，出错的多一格 `error=true` |
| 叫停 | INFO | `stopped`，带 `call`、`took_ms` |
| 目录里没有 | WARN | `unavailable`，带 `call`、`tool` |
| 崩了 | ERROR | `crashed`，带 `call`、`tool`、`took_ms` |
| 改前改后存不下来 | WARN | `effect content not stored` |

参数、工具交回的字一个都不记。

### 给人看的字

| 说法 | 中文 | 英文 |
|---|---|---|
| `core/tool-results/unavailable` | 现在用不了 | not available right now |
| `core/tool-results/crashed` | 内部出错了，可能做了一部分 | crashed, may be partly done |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-tool/src/catalog/tests.rs` | 照名字排、空目录、同名、名字的写法（空的、65 个字节、空格、中文、`.`、`/`；64 个字节的行）、参数格式不是对象、报排在前面的那一件、参数格式一字不差 |
| `crates/miyu-policy/src/tools/tests.rs` | 快照里的工具面照名字排、读得回来；没有工具的快照字节不变；两件同名造不出策略；两句带上工具名；两句写坏了说是哪一份 |
| `crates/miyu-session/tests/tools.rs` | 请求照名字带工具面、载入的老会话照快照发、在这一轮的工作目录里跑、出错的结果、执行中的输出推给头不落盘、两件只读的一起跑、打断丢掉在跑的、目录里没有的、崩了会话照常、会话停了丢掉在跑的 |
| `crates/miyu-session/tests/tool_log.rs` | 运行日志的那几行，参数和结果的字不进日志 |
| `crates/miyu-session/src/effects/tests.rs` | 改前改后换成 blob、存不下来的照样有哈希、她看过的读的和写的、从日志重建 |
| `crates/miyu-session/tests/write.rs` | 重新载入以后她读过的照样算、改完接着改不用重读、删了的不再算看过 |
| `crates/miyu-session/tests/restore.rs` | 撤掉的回合里读过的不算、恢复以后又算 |
| `crates/miyu-store/tests/human.rs` | 内核的每一句两种语言都有、照语言换成字、显示名和跟的参数、控制字符换掉、坏了的说是哪一份 |
| `crates/miyu-basesystem/tests/human.rs` | 基础系统每一种结果都带说法，两种语言都换得出字 |
| `xtask/src/ledger.rs`（`cargo xtask check` 的「文档」） | 给模型看的每一份字在登记簿里、指纹对得上 |

### 出处

- `05-内核接口.md` 第六节（工具的规格、给人看的说法、要碰的路径、执行这一步）、第八节（第一版的目录）、I6。
- `10-自带软件.md` 第一节（B1：内核不内置工具）、第五节（效果、她看过的）。
- `02-内核.md` 第六节「工具怎么调、下一步怎么走」。
- `26-提示词.md` 第三节「双槽」、第八节（字放在哪）、第十节（登记簿）。

### 还没有的

- 规格里的 `timeout`、`venues`、`group`；显示名、跟的参数现在放在 `human/` 里，不在规格里（`05-内核接口.md` 第六节）。
- 交给工具的调用编号、会话、身份、沙盒范围、截止时间（第六节）。
- 启用集照预设挑；外部扩展的工具、目录缓存在磁盘上；装了新扩展换一份目录快照（第八节，`16-人格与预设.md`）。
- 工具面的 token 预算、说明不超过三句，由门禁守住（`10-自带软件.md` 第九节、`26-提示词.md` 第七节）：门禁现在只查登记簿的指纹。
