## 存储

### 是什么

存储的执行器，做真的磁盘读写：找数据根、第一次用时建骨架；会话日志按段写、打开时自检；大内容存成 blob；删掉的会话挪进回收处，满了时限再真删。资源目录和给人看的字另见 `store/resources.md`，会话列表的索引另见 `store/index.md`（施工 3-8 七补）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/env.rs` | 环境快照：找数据根、资源目录要看的几样，从进程里读一次；系统的语言 `locale`（施工 8-1，都没设的看系统设置 `system_locale`，施工 8-2） |
| `crates/miyu-store/src/config_file.rs` | 读配置文件（施工 8-2，`config.md`「怎么走」第二条第 2 条）：没有的是空的，1 MiB 的上限，去掉开头的 BOM（记下有没有），不是 UTF-8 的报错，版本是整份字节的 SHA-256。写（施工 8-3，第五条第 4 到 7 条）：顺着链接写本体、临时文件在本体旁边、带上原来的权限位、替换前再读一次（和调用的一方读的版本不一样的放弃）、Windows 上改名失败歇 20 毫秒再试、最多 5 次 |
| `crates/miyu-store/src/watch.rs` | 监视几份文件（施工 8-4，`config.md`「怎么走」第七条）：看它们所在的目录（链接的另看本体所在的目录），照真实的位置和文件名认，只读的动静不理，一份 200 毫秒里没有新的变动了才交出去；系统的监视起不来的退回每 2 秒轮询，交回原因 |
| `crates/miyu-store/src/accounts.rs`、`logins.rs`、`private_json.rs` | 网页登录的凭据 `system/accounts.json`（argon2id，`m=19456`、`t=2`、`p=1`）、登录令牌的哈希 `home/<账号>/logins.json`（加的时候删过期的、最多 64 行）；只给自己看的 JSON 小文件照配置文件的规矩读、照密钥文件的规矩写（Unix 上 0600）（施工 W-8，`web-module.md`「怎么走」第一条） |
| `crates/miyu-store/src/secrets.rs` | 密钥文件 `system/secrets.toml`（施工 8-5，`config.md` 第九条）：照配置文件的规矩读，另看组、别人读不读得到；照配置文件的规矩写，Unix 上一律 0600，临时文件建的时候就是 |
| `crates/miyu-store/src/journal.rs` | 系统日志、账号日志 `journal.jsonl`（施工 8-3，`config.md`「系统日志、账号日志」）：每追加一条都重新打开、截掉最后那半行、读最后一行接着数 `seq`，外壳照事件的写法，追加、同步；8-15 起一个核心里照一把锁一条一条追加（写的不止配置服务），用量汇总从记下的字节往后读（`read_from`） |
| `crates/miyu-store/src/root.rs` | 数据根在哪、建骨架、认标记；账号的目录；缓存目录在哪 |
| `crates/miyu-store/src/durable.rs` | 建目录、同步目录；新建临时文件（只许新建，撞名换下一个；施工 8-5 起能建成 Unix 上 0600 的，`create_temp_with`）、删用不上的临时文件（施工 8-1 从 `blob.rs` 挪来，两处共用） |
| `crates/miyu-store/src/generated.rs` | 核心生成的派生文件：一样的不写，不一样的先写临时文件再替换（施工 8-1，`config.md`「怎么走」第一条第 7 条） |
| `crates/miyu-store/src/log.rs` | 会话日志：新建、追加、换段 |
| `crates/miyu-store/src/log/open.rs` | 打开时自检、截半行；只读地读；从记下的位置读起（施工 3-8 七补）；只读第一条 |
| `crates/miyu-store/src/blob.rs` | blob：存、取、核对哈希；分块暂存、改名进位置、扔掉、核心起来时清（施工 W-5）；读一段（施工 W-6） |
| `crates/miyu-store/src/jobs.rs` | 会话目录下后台命令的输出：`jobs/<编号>.out`（施工 7-3） |
| `crates/miyu-store/src/trash.rs` | 回收处：删掉的会话挪进来、满了时限的真删（施工 3-8 三补）；真删之前往账号日志留用量的底（施工 8-15） |
| `crates/miyu-store/src/resources.rs`、`human.rs` | 资源目录、给人看的字（`store/resources.md`） |
| `crates/miyu-store/src/index.rs`、`index/` | 会话列表的索引（`store/index.md`，施工 3-8 七补） |
| `crates/miyu-store/src/usage.rs`、`usage/` | 用量汇总 `state/usage.db`（施工 8-15，`models.md`「怎么走」第九条第 4、5 条） |
| `crates/miyu-store/src/sqlite.rs` | 派生数据的 SQLite 库怎么开、坏了怎么删掉重建（施工 8-15 从 `index.rs` 挪出来，索引和用量汇总共用） |

### 对外的样子

**环境快照 `Env`**：`Env::current()` 从进程里读一次。找数据根只照快照算，不直接读进程的环境。

| 格 | 取自 | 用来 |
|---|---|---|
| `platform` | 编译的目标：macOS 是 `Macos`，Windows 是 `Windows`，别的（Linux 和别的类 Unix）是 `Linux` | 缓存目录的默认位置 |
| `miyu_home` | 环境变量 `MIYU_HOME` | 数据根 |
| `home` | 标准库的 `std::env::home_dir()` | 数据根、缓存目录 |
| `xdg_cache_home` | `XDG_CACHE_HOME` | Linux 的缓存目录 |
| `local_app_data` | `LOCALAPPDATA` | Windows 的缓存目录 |
| `miyu_resources` | `MIYU_RESOURCES` | 资源目录 |
| `exe` | 程序的位置，顺着链接找到的本体；找不到本体的照原样 | 资源目录 |

没设的、读不到的是空的。空的、相对的算不算数，由用它的地方定。

**系统的语言 `env::locale()`**（施工 8-1）：`LC_ALL`、`LC_MESSAGES`、`LANG` 照这个先后，取第一个设了、不是空的（不是 UTF-8 的当没设），和命令行认界面语言的一样；都没设的是空的。不在快照里，只有核心要用语言、又没有头的时候读一次（生成配置的 Schema 和参考文件，`config.md`「怎么走」第一条第 6 条）。这几个都没设时看系统设置（`system_locale`，用 `sys-locale`：macOS 的首选语言、Windows 的界面语言，Linux 上它看 `LANGUAGE`，施工 8-2）。命令行握手以前也照它挑界面语言（`cli/main.md`）。

**生成的文件 `generated::write(路径, 字节)`**（施工 8-1）：和磁盘上已经有的逐字节比，一样的不写，交回 `false`；不一样的在旁边新建临时文件 `.<文件名>.<进程号>-<计数>.tmp`、写进去、同步、关上，改名盖上，再同步目录，交回 `true`；没有的目录建上（Unix 上 0700）。写到一半失败的，临时文件删掉。不顺着链接找本体、不带原来的权限位、Windows 上改名失败不重试：它们是派生的，下次起来再写（`config.md`「怎么走」第一条第 7 条）。

**数据根 `DataRoot`**：

| 方法 | 交回什么 |
|---|---|
| `locate(env)` | 数据根在哪，只找不建 |
| `prepare()` | 建骨架（「怎么走」第 2 条） |
| `path()` | 数据根本身 |
| `system()`、`homes()`、`state()`、`run()` | `system/`、`home/`、`state/`、`run/` |
| `account_dir(账号)` | `home/<账号>/` |
| `workspace(账号)` | `home/<账号>/workspace/` |
| `prepare_home(账号)` | 建 `home/<账号>/` 和里面的 `workspace/`，已经有的不动 |
| `sessions(账号)` | 这个账号的会话编号，从新到旧 |
| `session_dir(账号, 会话)` | `home/<账号>/sessions/<会话编号>/` |
| `trashed_sessions(账号)` | 回收处 `home/<账号>/trash/sessions/`（施工 3-8 三补） |
| `blobs(账号)` | `home/<账号>/blobs/` |
| `index(账号)` | `home/<账号>/index/`：派生数据，会话列表的索引放在这里（施工 3-8 七补，`store/index.md`） |

**缓存目录** `cache_root(env)`：只找不建。核心起来时算一次，沙盒的缓存放在它下面的 `sandbox/<账号>/`（施工 5-4 下，`core.md`），后台拉的 models.dev 目录放在 `models/`：`models-dev.json`、`models-dev.meta.json`（带 `ETag`，施工 8-7，`models.md`「文件」）。

**会话日志** `SessionLog`：`create(目录, 上限)`、`open(目录, 上限)`、`append(一批事件)`、`next_seq()`、`dir()`、`mark()`（写到哪了：正在写的那一段、它的长度、下一条该是几号，一起叫 `Mark`，施工 3-8 七补）；只读的 `read_events(目录)`、`read_segments(目录, 每一段)`、`read_marked(目录, 从哪里, 每一段)`（施工 3-8 七补）、`first_event(目录)`。一段的上限 `SEGMENT_LIMIT` 是 64 MiB（67,108,864 字节）。

**blob** `Blobs::new(目录)`：`put(内容)` 交回内容哈希，`get(哈希)` 交回内容，`path(哈希)` 交回它放在哪。

**回收处**（`trash`，施工 3-8 三补）：`discard(数据根, 账号, 会话, 删的时刻)` 把会话挪进回收处；`purge(数据根, 账号, 现在, 留多久)` 清一次，交回 `Purged { removed, failed }`：真删了几个，留下了、却不是因为没满时限的是哪几个、为什么。回收处里每个会话目录下的 `DELETED_AT`（`deleted_at`）写着删的时刻。

### 数据根里有什么

```text
<数据根>/
├── .miyu-root                          标记，一行字
├── system/
│   ├── config.toml                     系统配置（config.md，施工 8-2 读，8-3 写）
│   ├── secrets.toml                    密钥，Unix 上 0600，只经核心写（config.md，施工 8-5）
│   ├── accounts.json                   网页登录的用户名、argon2id 的密码哈希，Unix 上 0600，只经核心写（web-module.md，施工 W-8）
│   └── journal.jsonl                   系统日志：系统配置、密钥的改动（config.md，施工 8-3、8-5）
├── home/
│   └── <账号>/                         核心起来时给 admin 建；退回工作区时缺了再补建
│       ├── settings.toml               个人设置（config.md，施工 8-2 读，8-3 写）
│       ├── journal.jsonl               账号日志：个人设置的改动、项目配置的信任（config.md，施工 8-3）
│       ├── trust.toml                  项目配置的信任（config.md，施工 8-2 读，8-3 写）
│       ├── workspace/                  头报来的工作目录太宽时，退回这里（protocol.md）
│       ├── sessions/<会话编号>/         会话日志，一段一个文件
│       │   ├── 000000000001.jsonl
│       │   ├── …
│       │   └── jobs/<编号>.out          后台命令的输出，例如 jobs/j1.out（施工 7-3）
│       ├── trash/sessions/<会话编号>/   删掉的会话，整个会话目录挪过来，多一个 deleted_at（施工 3-8 三补）
│       ├── index/sessions.db            会话列表的索引，派生的；另有 -wal、-shm（施工 3-8 七补，store/index.md）
│       └── blobs/
│           ├── tmp/<进程号>-<计数>      存的时候的临时文件
│           ├── tmp/upload-<编号>        分块上传的暂存文件（protocol.md，施工 W-5）
│           └── <前两位>/<64 位十六进制>
├── state/
│   ├── logs/core.log、core.log.1 …     运行日志（log.md）
│   ├── config/                         核心起来时生成：config.schema.json、settings.schema.json、reference.toml（config.md，施工 8-1）
│   └── usage.db、usage.db-wal、usage.db-shm  用量汇总，派生的（models.md 第九条，施工 8-15）
└── run/                                core.lock、spawn.lock、token、socket；没有能用的 XDG_RUNTIME_DIR 的 Linux、macOS 上还有套接字 core.sock（ipc.md）；网页软件的 web.lock、web（地址，web-ui.md，施工 W-9）
```

- 这一页的代码新建的目录，Unix 上权限都是 0700；已经有的不改。Windows 上照系统默认的，靠用户目录本身的访问控制。
- 这一页的代码新建的文件（标记、段、blob、临时文件、配置文件、日志）照系统默认的权限建，靠上面 0700 的目录挡住别人。替换已经有的配置文件时带上它原来的权限位（施工 8-3）。
- `state/logs/` 由运行日志建（`log.md`），`state/config/` 由核心生成配置的 Schema 和参考文件时建（`config.md`），`state/models/` 由路由记下用出来的窗口（`learned.json`）、拉到供应商的列表（`providers/<编号>.json`）、池的指针往前走（`pools.json`，施工 8-8）时建（施工 8-7，`models.md`「文件」：派生数据，坏了当没有），`run/` 下的几样见 `ipc.md`。

### 怎么走

**1. 找数据根**（`DataRoot::locate`）

1. `MIYU_HOME` 设了、不是空的：就是它。开头是 `~` 的，照家目录接上：整个就是 `~` 的是家目录本身，`~/` 开头的接上后面（Windows 上 `~\` 也算）；`~别人` 不认，照原样当相对路径（施工 4-11：在终端里 `export X=~/…` 加了引号时，`~` 没被 shell 展开）。找不到家目录的，报找不到家目录。接好以后要是绝对路径，相对的报错，不猜。
2. 没设或者是空的：家目录下的 `.miyu`，三个平台一样。家目录没有、不是绝对路径的，报错。

**2. 建骨架**（`DataRoot::prepare`）

1. 建数据根本身，缺的上级一起建。
2. 看顶层有没有 `.miyu-root`。只看在不在：文件、目录、链接都算。
3. 没有标记：
   1. 目录里有任何东西（只有一个隐藏文件也算）：再看一眼标记，别处可能刚写下；还没有，报错，一个字节都不动它。
   2. 目录是空的：只许新建地写标记，内容是 `This directory is a Miyu data root (layout 1).` 加换行；同步这个文件，再同步数据根。别处刚写下了的，也算成。
4. 四个顶层目录 `system/`、`home/`、`state/`、`run/`，缺的才建。
5. 建两次不出错；两个进程同时第一次用同一个数据根，都成。该是目录的地方是个文件，报错。

**3. 缓存目录**（`cache_root`）：整台机器共用，不跟着 `MIYU_HOME` 变。

| 平台 | 在哪 | 找不到时 |
|---|---|---|
| Linux | `XDG_CACHE_HOME` 设了、不是空的、是绝对路径的：`<它>/miyu`；不然 `~/.cache/miyu` | 要用家目录，家目录却没有、不是绝对路径：报错 |
| macOS | `~/Library/Caches/Miyu` | 同上 |
| Windows | `<LOCALAPPDATA>\Miyu\cache` | `LOCALAPPDATA` 没有、不是绝对路径：报错 |

**4. 建目录、同步**（`durable.rs`，这一页新建的目录都走它）

1. 已经是目录的，不动。
2. 缺的上级先一层层建。
3. Unix 上新建的是 0700。同一刻别处建好了的，算成。
4. 新建的每一层，都同步它的上一层：建目录这件事本身才算落盘。
5. 同步目录：Unix 上打开目录、`sync_all`；Windows 上什么都不做。

**5. 会话日志：新建、追加**

1. 一个会话一个目录，一段一个文件，名字是这一段第一条的序号补零到 12 位，加 `.jsonl`：`000000000001.jsonl`。
2. 一行一条事件：事件写成的一行（`Event::to_line`，紧凑的 JSON，`kernel/events.md`）加 `\n`。UTF-8，三个平台都不写 `\r`。
3. 新建（`create`）：建目录，只许新建地建第一段 `000000000001.jsonl`，同步目录。第一段已经有了的报错，不覆盖。
4. 追加（`append`）一批：
   1. 空的一批，什么都不做。
   2. 序号要从下一条起一条接一条；有一条接不上，报错（`InvalidInput`），一个字节都不写。这是调用的一方的 bug。
   3. 整批拼成一块。
   4. 这一段已经写过、而且已经到了上限：先开下一段，名字照这一批的第一条，只许新建，建好同步目录。一批不拆到两段里，所以一段可以比上限大。
   5. 一次写入，再 `sync_data` 一次。返回时这一批都落了盘。
5. 一个会话只有一个写者，就是它的 actor（`session/actor.md`），不加锁。

**6. 会话日志：打开、自检**（`SessionLog::open`）

1. 目录里的段：名字正好是 12 位数字加 `.jsonl` 的，照数字排；别的文件不看。目录不存在、一段都没有：「没有这个会话」。
2. 照段的先后，一段整个读进来，照 `\n` 分行。序号从 1 数起，跨段接着数。
3. 最后一个 `\n` 后面还有字节（半行）：
   1. 不是最后一段：报坏了，行号是整行数加 1。
   2. 最后一段：截到最后一个 `\n`，再 `sync_all`。它一定从未被确认过。截在查这一段的每一行之前：这一段里前面有坏行的，半行照样先截掉，再报坏了。前面的段坏了的，读不到最后一段，不截。
4. 每一整行：要是 UTF-8；要读得出事件；序号要是该来的那一条。不对的报坏了，写明哪一段、第几行（从 1 数）。一行前后多出来的空白（空格、制表、`\r`）算 JSON 的空白，照样读得出：`\r\n` 结尾的行不算坏。
5. 有事件的段，第一条的序号要和段的名字一样，不一样的报坏了（第 1 行）。
6. 空的段：是最后一段的，名字要是下一条该来的序号，不是的报坏了（第 1 行），是的接着往里写；不是最后一段的，当没有，名字不查。
7. 除了截最后那半行，别的一律只报不修：日志是真相，修错了就是丢了。
8. 交回开着的日志（接着往最后一段追加，已有的长度照截后的算）和读出来的全部事件，交给内核载入。

**7. 只读地读**

- `read_events`：和打开时一样自检，只是最后一段末尾的半行跳过、不截，一个字节都不写：会话可能正在往里写。撤销、恢复以后会话重算她看过的（`session/actor.md`），撤销的回应读日志（`protocol.md`），用的都是它。
- `read_segments`：同 `read_events`，只是读一段交一段给 `每一段`，它交回 `false` 就不读下去（施工 6-4：`history` 翻长会话，叫停了不用读完整份）；撤销撤掉压缩时，会话读回更早的一段也用它，只留要的那几条（施工 6-9，`session/actor.md`）。
- `read_marked`（施工 3-8 七补，列会话照索引补时用，`store/index.md`）：同 `read_segments`，只是从 `从哪里` 读起，交回读到了哪里（最后一段、照到的整行末尾、下一条该是几号）。`从哪里` 是空的从头读。和日志对不上的交回空的，一条都不交：记的那一段没了、比记的短了、记的位置前面一个字节不是 `\n`。对得上的照旧自检，只是从中间读起的那一段，报坏了时的行号从记的位置数起，名字和第一条对不对不在那一段查。
- `first_event`：只读第一段开头那一行，不截、不写，列会话时用。第一段是空的、第一行还没写完：当没有这个会话。第一行读不懂：报坏了（第 1 行）。第一行不是 UTF-8：读写出错。

**8. 列会话**（`DataRoot::sessions`）：`home/<账号>/sessions/` 下名字合会话编号写法的（36 个字符的小写 UUID 写法），照编号的字倒着排：会话编号是 UUIDv7，倒着排就是从新到旧。不合写法的不算，是不是目录不看。目录还没有的，是空的。

**9. blob：存**（`Blobs::put`）

1. 内容哈希是 SHA-256，写成 `sha256:` 加 64 位小写十六进制（`kernel/ids.md`）。文件名是去掉 `sha256:` 的 64 位，放在前两位的目录里：`blobs/ba/ba7816bf…`。Windows 的文件名里不许有冒号。
2. 已经有这个文件的：不重写，把修改时间刷成现在，再同步前两位的目录（它可能是别处刚改好名、还没同步的）。回收的宽限期照修改时间算，刚又存了一遍的不能当成旧的。
3. 没有的：
   1. 建 `blobs/tmp/`。
   2. 只许新建地建临时文件，名字是 `<进程号>-<计数>`，计数一个进程从 0 数起。撞上崩溃留下的同名文件，换下一个名字，最多试 64 个名字。
   3. 写进去，`sync_data`，关上（Windows 上开着的文件改不了名）。
   4. 建前两位的目录，改名成最终的名字。改名失败、可最终的文件已经在了（两个会话同时存同一份），删掉自己的临时文件，算成。
   5. 同步前两位的目录。
   6. 这几步哪一步出错，删掉临时文件（删不掉就留着），报错。
4. 返回时它已经落了盘：先落 blob，再写引用它的事件。改名是原子的，最终的名字上不会有写了一半的。
5. 一个账号一份，不跨账号去重：两个账号存同一份内容，各存一个文件。
6. 分块暂存（`protocol.md` 的 `blob.open`、`blob.write`、`blob.close`，施工 W-5）：暂存文件也在 `blobs/tmp/`，叫 `upload-<编号>`，和第 3 条第 2 款的临时文件名（数字加连字符）撞不上。一块一块写进暂存文件，收齐了照第 3 条第 3 到 5 款同一个办法改名进位置（哈希是边收边算好的，不用整份重读）；已经有这份内容的，删掉暂存的，还是那一个 blob。核心起来时清一遍这个账号 `blobs/tmp/` 里的 `upload-*`：崩了、被杀留下的（`core.md`「起来的先后」）。

**10. blob：取**（`Blobs::get`）：读出来，重新算哈希。没有这个文件：「no blob <哈希>」。算出来和名字对不上：报错，写明是哪一个，不自动修，也不删。

读一段（`Blobs::read_range`，`blob.get`，施工 W-6，`web-module.md`「七、分块读」）：从 `offset` 起读最多 `length` 个字节，读到结尾就停；`offset` 过了结尾的是空的；`length` 是 0 只回大小。不重新核对整份内容的哈希：核对在整份取出来用的时候，就是上面这条。安全地打开照 `miyu-fs` 的 `read_range`，和 `fs.read` 共用一份，不另写一套跟链接的判断。没有这个文件：同上。

**11. 后台命令的输出**（施工 7-3，`output_path`、`create_output`）：会话目录下的 `jobs/<编号>.out`，一条后台命令一份，执行器的任务表边跑边写、不截（`session/tools.md` 第 5 条）。建的时候没有 `jobs/` 的先建（第 4 条，Unix 上 0700）；文件已经有的清空重写：编号在这个会话里不重复，已经有的只会是崩溃前起了、没来得及记下的那一条留下的。会话日志只认名字是 12 位数字的段（第 6 条），`jobs/` 不碍着它。删会话挪整个会话目录，`jobs/` 跟着一起进回收处（第 12 条，施工 3-8 三补）；造会话没成时收拾会话目录的 `abandon` 只删只剩空的第一段的目录，那时还起不了后台命令。结束了整份存成 blob，`job.reported` 里记它的哈希。

**12. 回收处**（施工 3-8 三补，`trash.rs`；2026-09-30 项目主人定：删了的进回收处，留 7 天再真删）

1. 删会话（`discard`）：会话目录 `home/<账号>/sessions/<会话编号>/` 整个挪进回收处 `home/<账号>/trash/sessions/<会话编号>/`。调的一方先停下这个会话、放开它的文件（Windows 上开着的文件挪不走，`protocol.md` 的 `session.delete`）。载入父会话时收掉的派到一半的空子会话也照这样挪（施工 7-8，`protocol.md`「会话表」第 8 条）。
   1. 先在会话目录里只写一个文件 `deleted_at`：删的时刻，事件的时刻写法（`2026-09-30T12:00:00.000Z`），加换行；`sync_all`。会话日志只认 12 位数字的段，写了没来得及挪的不碍着它，会话照旧在原处。
   2. 回收处没有的，照第 4 条建（Unix 上 0700）。
   3. 改名成回收处里的那一个：一次改名，要么挪了、要么没挪。回收处里已经有同名的，改名出错，报错。
   4. 同步两头的上一层：`sessions/`、`trash/sessions/`。
   5. 会话目录不在的：写 `deleted_at` 时就出错（找不到），什么都没建。
2. 清（`purge`）：核心起来时清一次（`core.md`「起来的先后」第 14 条），只清管理员的。
   1. 回收处里名字合会话编号写法的才看，别的不是这里放的。回收处还没有的，什么都不做。
   2. 读它的 `deleted_at`：现在减去删的时刻，满了留的时限（7 天，`miyu-core` 的 `KEEP`）的，连目录整个删掉；正好满的也删。删之前先留用量的底（施工 8-15，`models.md`「怎么走」第九条第 5 条）：照它的日志算好按小时的合计，往这个账号的 `journal.jsonl` 追加一条 `usage.purged`（时刻是现在，`by` 是内核）；一次请求都没有的不写。日志读不了（磁盘出错）、账号日志写不进去的：留着，报出来，下次再清。没满的、删的时刻比现在还晚的（时钟往回拨过）留着。
   3. `deleted_at` 读不了、写法不对的：留着，报出来（`Purged::failed`）。说不清它删了多久，不猜。
   4. 删不掉的：留着，报出来，下次起来再清。回收处本身读不了：报错。
3. blob 不动：删会话以后没人引用的 blob 随存储的回收那一步（第五节，「还没有的」）。找回删了的会话以后再做：回收处里的文件留着，挪回去就是。

**谁存、谁取**：造会话时存策略快照，载入时照 `session.created` 的哈希取（`session/actor.md`）；人附的文件，`blob.put` 存成管理员的 blob，`session.send` 造块之前取出来再认一遍（施工 3-9 三补，`protocol.md`）；工具效果里改前改后的内容存成 blob，撤销时取回来写回（`session/tools.md`），撤销的回应里比出改了什么时也取（`protocol.md`）；编码请求时取图片、文件（`session/actor.md`、`drivers/openai-chat.md`；文件每一个都取，施工 3-9 三补）。

### 出错

数据根的几句（`RootError`、`PrepareError`）是中文：`miyu ask`、`miyu undo` 找数据根、建骨架出错时，照原样印在标准错误上，退出码 1（`cli/ask.md`），等界面语言那一步照界面语言说。别的只进运行日志，是英文（施工 4-9 再补四中：原来是中文）。

| 类型 | 哪一种 | 说的话 |
|---|---|---|
| `RootError` | `RelativeMiyuHome` | `MIYU_HOME 要写绝对路径，写的是 <路径>` |
| | `NoHome` | `找不到家目录` |
| | `NoLocalAppData` | `找不到 LOCALAPPDATA，或者它不是绝对路径` |
| `PrepareError` | `NotOurs` | `<数据根> 里有别的东西，认不出是 Miyu 的数据根（顶层没有 .miyu-root），不动它。设 MIYU_HOME 指到一个空目录` |
| | `Io` | 系统的原话 |
| `OpenError` | `Missing` | `no session log in <目录>` |
| | `Broken` | `<段> line <行>: <为什么>`，为什么见下表 |
| | `Io` | 系统的原话 |
| `BlobError` | `Missing` | `no blob <哈希>` |
| | `Corrupt` | `blob <哈希> does not match its name; left as it is` |
| | `Io` | 系统的原话 |
| 追加时序号接不上 | `InvalidInput` | `the next event in the log should be <N>, got <M>` |
| 临时文件名一直撞 | `AlreadyExists` | `64 temporary file names in a row are taken in <目录>` |

`Broken` 的为什么：

| 什么时候 | 为什么 |
|---|---|
| 不是最后一段，末尾有半行 | `ends in a partial line, yet more segments follow` |
| 一行不是 UTF-8 | `not UTF-8` |
| 一行读不出事件 | `not readable: <解析的原话>` |
| 序号接不上 | `seq should be <N>, got <M>` |
| 段的名字和第一条对不上 | `the segment is named <N> but starts with <M>` |
| 空的最后一段名字不对 | `the empty last segment is named <N> but the next event is <M>` |
| `first_event` 读不懂第一行 | 解析的原话，前面不加字 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-store/src/root/tests.rs` | 三个平台的默认位置；`XDG_CACHE_HOME` 只挪 Linux 的缓存；`MIYU_HOME` 挪数据根、不挪缓存；空的当没设，相对的 `MIYU_HOME` 拒绝，开头的 `~` 照家目录接、`~别人` 当相对的、没有家目录时报错，相对的 `XDG_CACHE_HOME` 当没设；找不到家目录、`LOCALAPPDATA`；骨架建两次不出错、挡路的文件报错；八个线程同时建都成；账号的家目录建一次、0700；列会话从新到旧；新建的 0700、已经有的不改；新的数据根写下标记；认不出的一个字节不动、报错写明目录和 `.miyu-root`；只有隐藏文件也不算空；进程的环境找得到 |
| `crates/miyu-store/src/durable/tests.rs` | 一层层建、都是 0700、建两次不出错；挡路的文件报错 |
| `crates/miyu-store/src/log/tests.rs`、`log/tests/real.rs`、`log/tests/marked.rs` | 写了读得回，每行 `\n`、没有 `\r`；满了换段、一批不拆；截半行；中间一行坏了、序号接不上、段名对不上、不是最后一段有半行，都只报不修；空的最后一段接着写、名字不对报坏了；没有会话；第一段已有的不覆盖；只读第一条不动日志；只读地读跳过半行、一个字节不写；造到一半的会话没有第一条；序号接不上的一批不写；真会话写进去、读回来载入得了（`real.rs`，施工 1-13 再补挪出来）；从记下的位置读起（`marked.rs`，施工 3-8 七补，`store/index.md`） |
| `crates/miyu-store/src/blob/tests.rs` | 存了取得回、`tmp/` 是空的；放在前两位下、文件名没有冒号；同一份只存一个、刷修改时间；崩在改名前只留临时文件；撞名换名；改名时目标已经有了算成；读出来不对报错、不删；两个账号各存各的；分块暂存：两块写完照 `put` 一样收齐、已经有的删暂存；编号撞了拒绝；扔掉一个的暂存文件；核心起来时清 `upload-*`，不碰 `put` 自己的临时文件、不碰 `tmp/` 还没建过的账号（施工 W-5）；读一段：读到结尾就停、过了结尾是空的、`length` 写 0 只问大小；没有这个 blob（施工 W-6） |
| `crates/miyu-store/src/trash/tests.rs`（施工 3-8 三补） | 挪进回收处的整个目录一个字节不变、多一个 `deleted_at`，原处没了，列会话只剩别的；Unix 上 `trash/`、`trash/sessions/` 是 0700；会话不在的报找不到、回收处都没建；清：满 7 天的、正好满的删，差一毫秒的、删的时刻比现在晚的留，`deleted_at` 写法不对、没有的留并报出来，名字不合写法的不看；没有回收处什么都不做。时钟用测试的 |
| `crates/miyu-store/src/jobs.rs` 的测试（施工 7-3） | 输出放在会话目录的 `jobs/<编号>.out`，几段的编号照原样（`jobs/j2.1.3.out`，施工 7-1 补）；已经有的清空；Unix 上 `jobs/` 是 0700 |
| `crates/miyu-store/tests/snapshot.rs` | 策略快照存成 blob 的哈希就是快照的哈希，取回来重建，两份策略发的请求逐字节一样 |

### 出处

- `07-存储.md` 第二节：目录布局、默认位置、怎么找、认得出自己的数据根才动它、第一次用时建骨架；S6（数据根放在家目录的 `.miyu` 里）。
- `07-存储.md` 第三节：一个会话一个目录、段怎么存、一个写者、一批一次写入一次同步。
- `07-存储.md` 第四节：先落盘后推送、先落 blob、打开日志时自检三件事、各平台的坑（同步目录）。
- `07-存储.md` 第五节、S5：blob 的路径、写、读的时候核对哈希、不跨账号去重。
- `04-核心协议.md` 第九节 `session.delete`：删会话进回收处、留 7 天（2026-09-30 项目主人定，施工 3-8 三补）。
- `03-事件模型.md` E4：大内容存成 blob，事件里只放引用。
- `22-命令行.md` 第六节：权限不对由 `miyu doctor` 报告。

### 还没有的

- 投影缓存：打开长会话先读它，自检只查它记下的位置之后那一截（`07-存储.md` 第四节、第六节）。
- 隐私抹除：重写一段、原子替换（`07-存储.md` 第三节）。
- 导出一个会话（`07-存储.md` 第三节）。
- blob 的回收：删会话以后没人引用的删掉（回收处里的会话清掉以后），`tmp/` 里 `blob.put` 自己崩溃留下的那种清掉，都照宽限期（`07-存储.md` 第五节）。分块上传崩溃留下的暂存（`upload-*`）已经做了：核心起来时清一遍，不等宽限期（施工 W-5，第九条第 6 款）。
- 找回删了的会话；清别的账号的回收处（现在只有管理员）；留多久放进配置（施工 3-8 三补）。
- 派生数据：全文搜索、用量汇总的 SQLite（`07-存储.md` 第六节）。会话列表的索引做了（`store/index.md`，施工 3-8 七补）。
- 数据根里别的文件：人格、预设、放行规则（`07-存储.md` 第二节、第九节）。成员自己的密钥 `home/<账号>/secrets.toml` 随多用户。
- `miyu doctor` 查数据根的权限（`22-命令行.md` 第六节）。
- 用缓存目录的东西，例如语音的模型文件（`07-存储.md` 第二节）。
