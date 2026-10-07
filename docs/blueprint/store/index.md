## 会话列表的索引

### 是什么

一个账号一份 SQLite，一个会话一行，记着照日志算出来的几样：标题、置顶、工作目录、最近一次动静这些。列会话（`session.list`、她的 `sessions`）读它，不再每次把每个会话的日志整份读一遍。它是派生的：日志才是真相，索引随时可以删掉，照日志重建（`07-存储.md` 第六节、S3，施工 3-8 七补）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/index.rs` | 打开（坏了的删掉重建）、读全部的行、写回一行、会话落盘时往上盖、删行、用着用着坏了的重建 |
| `crates/miyu-store/src/sqlite.rs` | 怎么开、查版本和 `quick_check`、WAL、坏了连同 `-wal`、`-shm` 删掉：施工 8-15 从 `index.rs` 挪出来，和用量汇总共用；`IndexError`、`Opened` 是它的 `DbError`、`Opened` |
| `crates/miyu-store/src/index/row.rs` | 一行记什么，照一条事件怎么盖（`Row::see`）；这一条记下的工作目录（`cwd`） |
| `crates/miyu-store/src/log.rs`、`log/open.rs` | 日志里的一个位置 `Mark`；会话日志写到哪了（`SessionLog::mark`）；从记下的位置读起（`read_marked`，`store.md` 第 7 条） |
| `crates/miyu-session/src/store.rs` | `Indexed`：会话日志每落一批，顺手更新索引（`session/actor.md` 第 5 条第 7 点） |
| `crates/miyu-endpoint/src/list.rs` | 起来时打开（`open_index`）；列会话照索引读、照日志补（`scan`）；删会话删行（`forget`） |

依赖 `rusqlite`，开 `bundled`：自己带 SQLite 的源码编，三个平台一样，不用系统装的 SQLite（`licenses.md`）。

### 在哪个文件

`home/<账号>/index/sessions.db`（`DataRoot::index(账号)` 加 `FILE`）。SQLite 另有两个文件：`sessions.db-wal`、`sessions.db-shm`，删的时候三个一起删。目录没有的照 `store.md` 第 4 条建（Unix 上 0700）。现在只有管理员的一份，核心起来时开（`core.md`「起来的先后」第 12 条）。

### 一行记什么

表 `sessions`，一个会话一行，主键是会话编号：

| 列 | 是什么 | 取自 |
|---|---|---|
| `id` | 会话编号 | 目录名 |
| `owner` | 属主 | `session.created` 的 `owner` |
| `parent` | 父会话，主会话是空的 | `session.created` 的 `parent` |
| `oneshot` | 一次性的（0、1） | `session.created` 的 `oneshot` |
| `title` | 标题，空的是没有 | `session.meta_changed` 一条条盖上：写了 `title` 的换成它 |
| `pinned` | 置顶（0、1） | `session.meta_changed` 写了 `pinned` 的换成它 |
| `cwd` | 工作目录，空的是日志里一条都没记（很早以前的日志） | 最后一条带 `cwd` 的 `turn.started`，没有就照 `session.created` 的（`protocol.md`「会话表」第 5 条，同一个函数 `cwd`） |
| `created` | 造的时刻，毫秒 | `session.created` 的 `at` |
| `last_active` | 最近一次动静，毫秒 | 照到的最后一条事件的 `at`，哪种事件都算 |
| `segment` | 照到哪一段：这一段第一条的序号，就是段的名字 | 写日志的一方、读日志的一方交回的位置 |
| `bytes` | 这一段照到第几个字节：一整行的末尾，不含后面没写完的半行 | 同上 |
| `next` | 下一条该是几号 | 同上 |
| `preview` | 第一句话的第一行（施工 9-5）：空的是还没人说过话、第一句没有字 | 第一条 `message.user` 的头一块文字，跳过开头的空行、去掉前后空白，最多 50 个字（按 Unicode 的字数）；后来的话盖不过它 |
| `venue` | 场所（施工 O-3）：本机的是 `local` | `session.created` 的 `venue`。找回场所会话（`venues.md`）、列会话时跳过场所会话照它 |

- 忙不忙不进索引：列会话时从会话表的内存里拿（`protocol.md` 的 `session.list` 第 5 条）。
- 结构的版本记在 SQLite 的 `user_version` 里，现在是 3（施工 9-5 多了 `preview`、O-3 多了 `venue`；老的照版本不对删掉重建）。结构一变就加一；对不上的删掉重建，不写迁移（`07-存储.md` 第六节）。
- 日志模式是 WAL，`synchronous` 是 `NORMAL`：提交时不同步，断电最多丢最后几次更新，库不会坏。丢了的那几次，照到的位置落在后面，列会话照日志补（下面第 3 条）。

### 怎么走

**1. 打开**（`SessionIndex::open`，核心起来时，`open_index`）

1. 目录没有的建上，打开这个文件，看 `user_version`：
   - 0（没有这个文件、是个空文件）：建表，记下版本。算新建的，记一行 `INFO session index created`。
   - 1：跑一次 `quick_check`（一个账号几千行，几毫秒），不是 `ok` 的算坏了。
   - 别的：版本不对。
2. 读不了（不是 SQLite 的文件、读写出错）、坏了、版本不对：关上，连同 `-wal`、`-shm` 删掉，照第 1 款建一份空的，记一行 `WARN session index rebuilt reason=…`。空的索引照日志补，就是照日志重建：列会话时没有那一行的，整份读一遍，读完写进去（第 3 条）。重建期间列会话和没有索引时一样整份读，不卡住。
3. 删了重建也打不开：记一行 `WARN session index unusable error=…`，这一回不用索引：读出来没有一行，写什么都不写，每次列会话都整份读。
4. 一个核心开一个连接，一直开着，拿锁护着，会话、列会话、删会话共用。同一个进程里打开再关上同一个库文件，会丢掉 SQLite 在这个进程里的文件锁（`07-存储.md` 第六节，旧版把库弄坏过），所以不为一次读写开一个。

**2. 会话写日志时**（`SessionIndex::advance`，`session/actor.md` 第 5 条第 7 点）

同一批事件落了盘，在同一个阻塞线程里顺手更新它那一行；写之前日志在 `before`，写完在 `after`（`SessionLog::mark`）。在一个 `IMMEDIATE` 事务里：

1. 这一批以 `session.created` 开头、写在日志的最前面（`before` 的下一条是 1）：照它新起一行（有同编号的旧行，盖掉）。
2. 别的：表里那一行照到的正好是 `before`，才照这一批一条条盖上去（`Row::see`：最近一次动静、工作目录、标题、置顶），照到 `after`。
3. 没有这一行、照到的不是 `before`（中间哪一批没盖上：更新失败了、崩在更新之前）：不动。不会有照到的位置对、内容却少了几条的一行：落后的就停在原处，列会话时照日志补。
4. 更新失败（写不进、表没了）：记一行 `WARN session index not updated error=…`，照样算落了盘，会话照常。索引是派生的，那一行停在上一次照到的地方，下次列会话照日志补上。
5. 没有索引的（测试里自己造的会话，`Create::index`、`Load::index` 是空的）不更新。

**3. 列会话**（`scan`，`protocol.md` 的 `session.list`，她的 `sessions`）

1. 一次读出全部的行。读出坏了的（有一行读不懂、SQLite 说坏了）：记一行 `WARN session index not read error=…`，删掉重建（`reset`，建不成的记 `WARN session index unusable`），这一次当一行都没有。
2. 照放会话的目录列会话，从新到旧（`store.md` 第 8 条）；读下一个之前看一眼叫停；够了 `limit` 就停。目录里没有的会话，索引里有它的行也不列：删会话时没删掉的行不碍事。
3. 有这一行的：先照它挑（属主、父会话、是不是一次性的只在 `session.created` 里，不会变），不要的不读。要的，从它照到的位置读起（`read_marked`）：
   1. 照到的就是日志现在的末尾：只读到末尾那一个字节，直接用。
   2. 日志比它长（崩在更新之前、手动加过几条）：只读多出来的那一截，一条条盖上去；后面换了段的接着读后面的段。补好的写回去，只换照到的还是原来那里的那一行（会话同时自己盖过了的，照它的）。
   3. 对不上的：记的那一段没了、比记的短了（手动截过）、记的位置前面一个字节不是 `\n`（不在一行的开头）、多出来的那一截读不下去：这一个会话整份重读（第 4 款）。
4. 没有这一行的、对不上的：照没有索引时的读法（`protocol.md` 的 `session.list` 第 2 到 4 条），先读第一条挑，再从头读一遍盖上去。读完了的写进索引：没有这一行的，表里还没有才写；对不上的，照到的还是原来那里才换。后面读不下去的（日志坏了）：记一行 `WARN meta not read`，照坏的那一段以前的算，不写进索引，下次还整份读。
5. 所以结果和每次整份读的一字不差（测试里两种算法比）。末尾没写完的半行照旧不算：照到的是它前面那一整行的末尾，写完了下次补。
6. 写回失败的记一行 `WARN session index not updated`（目标 `miyu::endpoint`），照样列。
7. 记着的位置对、日志却在原处改了内容、长度没变的（手动改了一个字），认不出来：日志是只追加的，只有手动改才会这样，照记的列。

**4. 删会话**（`forget`）

会话目录挪进回收处以后，删掉它那一行（`protocol.md` 的 `session.delete` 第 6 条；载入时收掉的派到一半的空子会话也是，「会话表」第 8 条）。没有这一行的也算成。删不掉的记一行 `WARN session index row not removed`：列会话照放会话的目录走，那一行不碍事。

### 出错

| 什么时候 | 说的话 |
|---|---|
| SQLite 出错 | SQLite 的原话 |
| 建目录、删文件出错 | 系统的原话 |
| 版本不对 | `version <N>, not 1` |
| 查出坏了 | `quick_check: <SQLite 说的>` |
| 日志模式换不成 WAL | `journal mode <模式>` |
| 一行读不懂 | `row <会话编号>: <哪一格>`，哪一格是 `id`、`owner`、`parent`、`time`、`mark` |
| 位置的数超过 SQLite 的整数 | `row <会话编号>: mark too large` |

都只进运行日志，是英文。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-store/src/index/tests.rs` | 新建的是空的、下次照旧、目录 0700；一批批盖上：属主、一次性、工作目录、标题、置顶、最近一次动静、照到哪，造的时刻不变；没有这一行、照到的不是这一批之前的，不动；写回只填空的、只换照到的没变过的；删行、删没有的也成；乱写的文件、版本不对的删掉重建成空的；读不懂的一行报错、重建以后是空的 |
| `crates/miyu-store/src/log/tests/marked.rs` | 从头读照到的就是写的一方记的；只读多出来的；接着读后面的段；末尾的半行不算、一个字节不动；比记的短了、不在一行开头、记的那一段没了，交回空的；序号接不上报坏了 |
| `crates/miyu-endpoint/src/list/tests/indexed.rs` | 一批各种各样的会话（没记过工作目录的、改名置顶又去掉标题的、两段的、末尾半行的、日志坏了的、第一条不对的、空目录、一次性的子会话）：没有索引、照日志填、照索引读，列出来一字不差，只要一次性的也一样；日志坏了的不写进索引；索引没看见的几条补上、换了段的补上；截短了的、位置落在一行中间的、记的那一段没了的整份重读、写回去；没有、乱写、版本不对的起来时重建、照日志填好；用着用着读出坏了的，这一次照样列得对、重建好 |
| `crates/miyu-endpoint/src/list/tests/speed.rs` | 量尺，标 `#[ignore]`：250 个会话、日志共 13 MB，整份读、照日志填、照索引读、几个会话多写了几条以后，各要多久 |
| `crates/miyu-endpoint/tests/index.rs` | 造会话就有那一行；说了一轮、改标题置顶以后那一行和日志对得上，照到日志的末尾；删会话删掉那一行 |
| `crates/miyu-endpoint/tests/index_log.rs` | `INFO session index created`、`WARN session index rebuilt`、`WARN session index not read` 这几行；读出坏了的照样列得对 |
| `crates/miyu-session/tests/index_log.rs` | 每落一批那一行照到日志的末尾；表没了，更新失败只记一行带会话编号的 `WARN session index not updated`，会话照常说完下一轮 |

### 量出来的

250 个会话、日志共 12,956,750 字节（一个会话 130 条事件），同一台机器上列一次（施工 3-8 七补，2026-10-01）：

| | release | debug |
|---|---|---|
| 整份读（改之前的读法） | 53–55 毫秒 | 600–632 毫秒 |
| 照日志填索引的那一次 | 59 毫秒 | 621 毫秒 |
| 之后照索引读 | 0.9–1.4 毫秒 | 2.6–2.8 毫秒 |
| 十个会话各多写了五条 | 1.2 毫秒 | 4.0 毫秒 |

### 出处

- `07-存储.md` 第六节：派生数据放在账号的 `index/`，结构变了删掉重建，不写迁移，活库不直接复制；S3：派生索引用 SQLite；第七节：核心启动时只读会话列表索引。
- 施工 3-8 七补：这一页；技术细节照推荐定（2026-10-01 施工时定）：照到的位置记段、字节、下一条三样；会话只盖照到的正好是这一批之前的那一行，落后的等列会话补；列会话时没有、对不上的整份读、读完写回，就是照日志重建，不另起后台任务；WAL 加 `NORMAL`；新建的记 `INFO`，读不了、坏了、版本不对的记 `WARN`。

### 还没有的

- 别的账号的索引：现在只有管理员（`06-多用户与身份.md`）。
- `history` 的全文索引：随记忆、知识库（`07-存储.md` 第七节）。
- 用量总表：8-15 做了，照这一页的办法（`models.md`「怎么走」第九条第 4 条，`state/usage.db`）。
- 会话列表流 `sessions.changed`：施工 9-5 做了，算一项照这里的索引、只算那一个会话（`protocol.md`「会话列表的推送」）。
- 删会话时没删掉的行，现在留在表里不碍事，不清。
