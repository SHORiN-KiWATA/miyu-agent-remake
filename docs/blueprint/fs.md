## 碰文件：`miyu-fs`

### 是什么

核心进程里碰文件的底子：她给的路径换成真实的位置，查它落在边界表的哪一片，安全地打开要读的文件，把一份文件整体换成新的内容，把东西放进系统的回收站、再移回来。文件工具跑在核心进程里，不在操作系统的沙盒里，边界靠这一层自己守。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-fs/src/lib.rs` | 交出去的几样 |
| `crates/miyu-fs/src/boundary.rs` | 边界表：几片、先后、各平台的清单；`within` |
| `crates/miyu-fs/src/resolve.rs` | 换成真实的位置（整条换的、最后一段不跟链接的）；`~` 的规矩 |
| `crates/miyu-fs/src/wide.rs` | 工作目录太宽 |
| `crates/miyu-fs/src/open.rs` | 安全地打开 |
| `crates/miyu-fs/src/replace.rs` | 整体换成新的内容 |
| `crates/miyu-fs/src/trash.rs` | 回收站：放进去、移回来 |
| `crates/miyu-fs/src/trash/linux.rs`、`macos.rs`、`windows.rs`、`other.rs` | 各平台的回收站；别的系统一律收不了 |
| `crates/miyu-fs/src/trash/recycled.rs` | Windows 回收站里的 `$I` 记录；每个平台都编，测试到处都跑 |

用它的：基础系统的几件工具（`tools/`）；权限策略 `crates/miyu-session/src/guard.rs`（换成真实的位置、查边界；判 `trash` 时最后一段不跟链接）；撤销时改回文件 `crates/miyu-session/src/restore.rs`（`replace`、`trash::put`、`trash::restore`）；开会话时挑工作区 `crates/miyu-endpoint/src/sessions.rs`（`resolve`、`too_wide`）。

### 对外的样子

| 名字 | 是什么 |
|---|---|
| `Zone` | 一个真实的位置落在哪一片：`Writable` 能读能写、`Readable` 只能读、`Forbidden` 谁都不能碰、`Outside` 边界以外 |
| `Places` | 造边界表要的几个地方，都是原样的路径：`workspace`、`dirs`（加进来的目录，一组（施工 5-10 上））、`data_root`、`temp`、`readable`（一组） |
| `Places::here(workspace, data_root, home)` | 照这台机器补上临时目录、系统目录、工具链目录 |
| `Boundary::new(&places)` | 边界表：每一片换成真实的位置 |
| `Boundary::zone(path)` | 真实的位置 `path` 落在哪一片 |
| `within(path, root)` | 真实的位置在不在目录 `root` 里 |
| `resolve(cwd, home, input)` | 她给的路径换成真实的位置 |
| `resolve_itself(cwd, home, input)` | 同上，最后一段不跟链接：碰的是这一条本身（`trash`）；没有名字可碰的交回空的 |
| `tilde(input)` | `~` 开头的，交回 `~` 后面那一截 |
| `too_wide(dir, home, data_root, own)` | 工作目录太不太宽 |
| `open_file(real)` | 安全地打开一份要读的普通文件 |
| `replace(real, bytes)` | 把一份文件整体换成 `bytes` |
| `trash::put(real, home)` | 放进系统的回收站，交回它在回收站里的真实路径 |
| `trash::restore(kept, to)` | 把回收站里的 `kept` 移回 `to` |
| `ResolveError`、`OpenError`、`Kind`、`trash::Refused` | 换不成、打不开、不是普通文件时是什么、放不进回收站：见第四节、第六节和「出错」 |

### 怎么走

#### 一、边界表

1. 几片重叠时照这个先后，先对上的算：

   | 先后 | 地方 | 哪一片 |
   |---|---|---|
   | 1 | 工作区里（工作区自己也算） | 能读能写；其中哪一层叫 `.git`、下一层叫 `hooks` 或 `config` 的，它和它下面的只能读（子目录里的仓库也算） |
   | 2 | 数据根里（数据根自己也算） | 谁都不能碰 |
   | 3 | 加进来的哪一个目录里（它自己也算）（施工 5-10 上） | 照工作区：能读能写，`.git/hooks`、`.git/config` 只能读 |
   | 4 | 临时目录里 | 能读能写 |
   | 5 | 系统目录、工具链目录里 | 只能读 |
   | 6 | 别的 | 边界以外 |

   - 工作区排在数据根前面：工作区是数据根里的 `home/<账号>/workspace/` 时，那一片照工作区算，数据根别处照样不能碰。
   - 加进来的目录排在数据根后面：落进了数据根的（报来以后被换成了链接），数据根照样谁都不能碰。
   - 数据根排在临时目录前面：数据根可能在临时目录里（测试、`MIYU_HOME` 指到那里）。
2. `Places::here` 照这台机器填：

   | 平台 | 系统目录 |
   |---|---|
   | Linux | `/usr`、`/bin`、`/sbin`、`/lib`、`/lib32`、`/lib64`、`/etc`、`/opt` |
   | macOS | `/usr`、`/bin`、`/sbin`、`/System`、`/Library`、`/Applications`、`/opt`、`/etc`（真实的位置在 `/private/etc`，换过以后自然对上） |
   | 别的 Unix | 一个都没有：要读就问人 |
   | Windows | 环境变量 `SystemRoot`、`ProgramFiles`、`ProgramFiles(x86)` 指到的，没设的、空的不算 |

   - 工具链目录：家目录下的 `.cargo`、`.rustup`、`.npm`、`.gitconfig`（家目录读不出来的，这四样不算），加上环境变量 `CARGO_HOME`、`RUSTUP_HOME` 指到的地方（没设的、空的不算）。都只能读。
   - 临时目录：标准库的 `temp_dir()`（Unix 上是 `TMPDIR`；没设的，macOS 上是系统给这个用户的临时目录（`confstr(_CS_DARWIN_USER_TEMP_DIR)`），别的是 `/tmp`；Windows 上照系统的 `GetTempPath2`，先看 `TMP`、`TEMP`）。
   - 工作区、数据根照交进来的。
3. 造表时每一片都换成真实的位置；换不成的（不存在、读不了，例如这台机器上没有 `/lib32`）那一片不算。
4. 交给 `zone` 的要是已经换成真实位置的路径。比的时候按路径一段一段比：`work-other` 不在 `work` 里。
5. 大小写：
   - 「是不是在数据根里」在 macOS、Windows 上不分大小写，免得换个大小写就绕过去；Linux 上分。
   - 工作区里那一段的 `.git`、`hooks`、`config` 三个名字，在 macOS、Windows 上也不分大小写（`.GIT/HOOKS` 是同一处）。
   - 「是不是在工作区里」「临时目录」「只能读的那几个」照原样比：换了大小写的对不上，往后落；落到边界以外的，多问一次人，不多放行。
6. `within(path, root)`：`root` 自己也算在里面；一段一段比；macOS、Windows 上每一段换成小写再比。往下走目录的工具照它跳过数据根（`tools/glob.md`）。

#### 二、换成真实的位置（`resolve`）

1. `~` 开头的照 `tilde` 认：只有 `~` 自己、`~` 后面紧跟 `/` 的算（Windows 上 `~\` 也算）；`~alice`、`a/~` 照原样。算的，把 `~` 后面那一截（去掉紧跟着的分隔符）接在家目录后面；没有家目录（读不出来）：`NoHome`。
2. 别的接在 `cwd` 后面；绝对路径接上去就是它自己。
3. 整条换成真实的位置（系统的 `canonicalize`）：符号链接、Windows 的目录联接都照指向的地方算，`.`、`..` 消掉。换得了就是它。
4. 换不了，因为不存在：
   1. 这一条自己在（不跟链接读得到）：它是一个指向不存在处的链接，`DanglingLink`。照它往下写，会写到链接指的别处去。
   2. 从长往短试它的上级目录：最近的、已经存在的那一层换成真实的位置，再接上后面还不存在的几段。试的时候碰到一层自己在、却换不了的（指向不存在处的链接），也是 `DanglingLink`；读上级出了别的错（例如没有权限），`Io`。
   3. 接上的那几段里有 `..` 的，`ParentOfMissing`：还不存在的目录往上走，说不清落在哪。
   4. 一层存在的上级都找不到：`Io`（找不到）。
5. 换不了，因为别的（例如没有权限）：`Io`，系统的原话。
6. 平台不一样的：
   - Windows：换出来的是 `\\?\C:\…` 这种写法；`/`、`\` 都当分隔符；系统找文件之前先照字面把 `..` 消掉，`newdir/../x.rs` 就是 `x.rs`，不报 `ParentOfMissing`。
   - macOS：`/tmp`、`/var`、`/etc` 的真实位置在 `/private` 下面，比的两边都换过，对得上。
7. **最后一段不跟链接**（`resolve_itself`，施工 4-9 再补二从 `trash` 挪过来）：
   1. `~` 开头的照 `tilde` 接家目录；只有 `~` 自己的（`~`、`~/`，Windows 上还有 `~\`）没有名字，交回空的；没有家目录：`NoHome`。
   2. 没有名字的（`.`、`..`、根目录，以 `..` 结尾的）交回空的。以 `/.` 结尾的，名字是前面那一段（`src/.` 是 `src`）。
   3. 别的：上级照上面整条换成真实的位置，再接上最后一段的原样。换不了的，照上面报错。
   4. `trash` 照它找要删的（`tools/trash.md`），权限策略判 `trash` 也照它（`session/guard.md`）：两边碰的是同一个，链接判的是链接本身在哪。

#### 三、工作目录太宽（`too_wide`）

几样都要先换成真实的位置再交进来。下面四样有一样就算太宽：

1. 是家目录（家目录读不出来的，不查这一条）；
2. 是根目录（没有上级）；
3. 包含数据根（数据根自己也算）；
4. 落在数据根里，又不在账号自己的工作区 `own` 里（`own` 自己和它下面的不算太宽）。

一段一段比，分大小写。开会话时照它挑工作区：太宽的退回账号的工作区（`crates/miyu-endpoint/src/sessions.rs`，`cli/ask.md` 第 4 条）。

#### 四、安全地打开（`open_file`）

交进来的要是换过、查过边界的真实位置。

1. 只读地打开：
   - Unix：带 `O_NOFOLLOW`（最后一层是链接就不开）、`O_NONBLOCK`（FIFO 没人写时不卡住）。
   - Windows：带 `FILE_FLAG_OPEN_REPARSE_POINT`，打开链接、目录联接本身，不跟着走。
2. 打不开的：
   - 没有这个文件：`NotFound`。
   - 别的错：先不跟链接看它是什么：不是普通文件的，报是什么（`NotAFile`）；Unix 上报 `ELOOP` 的（最后一层是链接），报 `NotAFile(Link)`；都不是，`Io`。
3. 打开了：再看开的这个是不是普通文件，是的交出去，不是的报是什么；读不出它的元数据：`Io`。
4. 「是什么」（`Kind`）：链接 `Link`；目录 `Directory`；Unix 上 FIFO `Fifo`，字符设备、块设备 `Device`，套接字 `Socket`；别的 `Other`。Windows 上没有 FIFO、设备、套接字这几种，链接、目录以外都是 `Other`。

`read`、`grep`、`write`、`edit` 照它开（施工 4-9 再补二）：`grep` 点名的文件、走目录找到的文件都照它开；`write`、`edit` 读原来的内容照它开。

#### 五、整体换成新的内容（`replace`）

`write`、`edit` 写文件，撤销时写回改前的内容，都用它。

1. `real` 要有上级目录，上级目录要已经在；没有上级的（例如根目录）：`InvalidInput`。
2. 看原来的文件（跟着链接）：
   - 是只读的：不写，`PermissionDenied`。只读指 Unix 上一个写位都没有、Windows 上带只读属性。Windows 上改名也盖不过只读的文件，三个平台照这一条一样。
   - 在、不是只读的：记下它的权限。
   - 不在：照系统默认的权限建。
   - 读不了（不在以外的错）：报那个错。
3. 在同一个目录里只许新建地建一个临时文件：名字 `.<原来的名字>.<进程号>-<序号>.miyu-tmp`，序号在这个进程里从 0 往上数。名字撞上了换下一个，最多 16 次；都撞上：`AlreadyExists`。建不了（撞名以外的错）：报那个错。
4. 写进 `bytes`；原来有的，照原来的权限设好；同步到磁盘。
5. 改名盖上去。
6. 第 4、5 步出错：删掉临时文件，报那个错；原来的文件没动。临时文件删不掉的，只记一条运行日志，目录里多一个以点开头的文件。

- 中途崩了，原来的文件还在，不会留下写了一半的；崩在改名之前的，目录里剩下那个临时文件，没有谁去清。
- 盖上去的是一份新的文件：留下的只有权限（Unix 的权限位，Windows 的只读属性）；属主、别处的硬链接不跟过来。

#### 六、回收站

**放进去**（`trash::put`）：交回它在回收站里的真实路径，三个平台都是。收不了的三种：

| 哪一种 | 是什么 |
|---|---|
| `Unavailable` | 这块盘上没有能放的回收站：没删 |
| `Lost` | 挪了，可回收站里找不到它，它可能回不来了（只有 macOS、Windows 会碰到） |
| `Failed` | 出错了，带着系统的原话 |

**Linux**（照 freedesktop 回收站规范自己放，不依赖桌面）：

1. 不跟链接读 `real` 的元数据，读不了：`Failed`。记下它在哪块盘上。
2. 挑回收站：
   1. 家目录的回收站：交进来的 `home` 就是这个进程的 `HOME` 时（照原样比，不换成真实的位置），照 `XDG_DATA_HOME`（要是绝对路径）下的 `Trash`；不然是 `<home>/.local/share/Trash`。
   2. 它和 `real` 在同一块盘上（它还不存在的，照最近一层已经在的上级算）：用它。
   3. 不在同一块盘上，或者没有 `home`：从 `real` 的上级往上走，走到上一层换了盘为止，那一层是这块盘最上面的目录。`.Trash` 在、不是链接、是目录、带粘滞位的，用 `.Trash/<uid>`；不行的，用 `.Trash-<uid>`。要用的那一个没有的建上（`0700`，只建这一层），有的要是目录、不是链接、属主是自己；两个都不行：`Unavailable`。`uid` 照 `/proc/self` 的属主，读不了：`Failed`。
3. 建好回收站下的 `files/`、`info/`（缺的上级一起建，建的每一层都是 `0700`），建不了：`Unavailable`。
4. 名字是 `real` 的最后一段（没有的：`Failed`）。第 1 次用原名，第 n 次（n 从 2 起）接 `.n`，最多试到 1000：
   1. 只许新建地建 `info/<名字>.trashinfo`；已经有了的，试下一个名字；别的错：`Failed`。
   2. 写进记录，同步到磁盘。
   3. `files/<名字>` 已经有东西（别的程序留下的、没有记录的）：删掉刚建的记录，试下一个名字，不盖掉它。
   4. 把 `real` 改名挪成 `files/<名字>`。写记录或者改名出错：删掉那份记录；跨了盘的 `Unavailable`，别的 `Failed`。
   5. 成了：交回 `files/<名字>` 的绝对路径。
   - 1000 个名字都用掉了：`Failed`。
5. 记录的样子：

   ```text
   [Trash Info]
   Path=<原来的位置>
   DeletionDate=<本地时间，YYYY-MM-DDTHH:MM:SS>
   ```

   - 原来的位置：家目录的回收站写绝对路径；那块盘上的回收站写相对那块盘最上面那一层的路径。
   - 转义：字母、数字和 `-._~/` 照写，别的每个字节写成 `%XX`（大写十六进制）。例如 `a b 中.txt` 写成 `a%20b%20%E4%B8%AD.txt`。
6. 放不进去的不删：不像 `trash` 这个 crate 那样拷一份再删原来的，拷过去的撤销时移不回来。

**macOS**：

1. 路径不是 UTF-8 的：`Failed`。
2. 交给系统的 `NSFileManager` 的 `trashItemAtURL:resultingItemURL:error:`。
3. 成了：交回它给的新位置（名字可能被系统改过）；它没给：`Lost`。
4. 出错：错误码是 `NSFeatureUnsupportedError`（收不了的盘，系统自己不删）的，`Unavailable`；别的 `Failed`，原话是系统的 `localizedDescription`。

**Windows**：

1. 去掉 `\\?\` 这个前缀（`\\?\UNC\…` 的不去）：`trash` 这个 crate 和回收站的记录都是普通的写法。
2. 取盘符的根目录（`C:\` 这样）；没有盘符的（网络路径）：`Unavailable`。
3. 根目录下没有 `$Recycle.Bin` 目录：`Unavailable`，不删。网络盘、U 盘这类没有回收站的，那个 crate 会直接永久删掉还报成功；本地盘上从没删过东西的也可能还没有它，宁可不删。
4. 用 `trash` 这个 crate 删（系统的 `IFileOperation`，进回收站）；出错：`Failed`。
5. 列回收站，列不了：`Lost`。挑出原来的上级目录和 `real` 的上级一样的（不分大小写）；读每一项 `$R` 旁边的 `$I` 记录，记录里的原路径和 `real` 一样的（不分大小写）留下；删的时间最晚的那一个，交回它 `$R` 开头的真实路径；一个都没有：`Lost`。不照列表里的名字拼原路径：那是给人看的显示名，扩展名可能被藏了（`a.txt` 列成 `a`）。
6. `$I` 记录（`recycled.rs`）：`$R` 开头的名字，头两个字换成 `$I` 就是它；不是 `$R` 开头的没有。

   | 字节 | 是什么 |
   |---|---|
   | 0–7 | 版本，小端 |
   | 8–15 | 大小 |
   | 16–23 | 删的时间：FILETIME，1601 年起的 100 纳秒数 |
   | 第 2 版：24–27，接着 | 字数（连结尾的 0），接着是那么多个 UTF-16LE 的字 |
   | 第 1 版：24 起 520 字节 | 定长 260 个 UTF-16LE 的字，不够的补 0 |

   路径取到第一个 0 为止，解不开的字换成替换符。认不出的版本、不够长的、字数说得比记录还长的：当没有。

**别的系统**：一律 `Unavailable`。

**移回来**（`trash::restore(kept, to)`）：

1. `to` 的上级目录没了的建上。
2. 把 `kept` 改名移回 `to`。
3. 删掉回收站给它记的那一份：Linux 上 `kept` 的上级叫 `files` 的，删掉同一个回收站里的 `info/<名字>.trashinfo`，不是的什么都不删；Windows 上删掉 `$R` 旁边的 `$I`；macOS 上没有要删的。不在的不算；删不掉的只记一条运行日志：东西已经回来了。

- `to` 要空着、`kept` 要还在：先查的是调用的一方（`crates/miyu-session/src/restore.rs`）。查和移之间 `to` 被别的程序占了的，改名会盖掉它。
- 出错：上级目录建不了、改名移不回去（例如 `kept` 已经没了）：报那个错。

### 出错

出错写成的字是英文。`ResolveError` 的字她看得到：权限策略拒绝时填进 `reason`（`core/permissions/unresolvable`），工具出错时填进 `error`（`tools/`）。

| 类型 | 哪一种 | 写成 |
|---|---|---|
| `ResolveError` | `NoHome` | `the home directory is not known` |
| | `ParentOfMissing` | `'..' after a directory that does not exist` |
| | `DanglingLink` | `a link on the path points nowhere` |
| | `Io` | 系统的原话 |
| `OpenError` | `NotFound` | `no such file` |
| | `NotAFile(Kind)` | `it is a directory`、`it is a link`、`it is a FIFO`、`it is a device`、`it is a socket`、`it is not a regular file` |
| | `Io` | 系统的原话 |

`OpenError` 的这几句现在没有哪一处给她看：`read` 照种类说自己的那几句（`tools/read.md`）。`replace`、`trash::restore` 出错是系统的 `io::Error`；`trash::put` 出错见上面那三种。

运行日志（target `miyu::fs`，级别 WARN）：

| 什么时候 | 那一行 |
|---|---|
| `replace` 的临时文件删不掉 | `temporary file left behind` |
| Linux 上用不着的 `.trashinfo` 删不掉 | `trash record left behind` |
| Windows 上移回来以后 `$I` 删不掉 | `recycle record left behind` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-fs/tests/boundary.rs` | 六片的先后、加进来的目录照工作区算、落进数据根的照样不能碰、不存在的不算（施工 5-10 上）、工作区里的 `.git/hooks`、`.git/config` 只能读（子仓库里的也算）、数据根在临时目录里也不能碰、工作区挪进数据根的照工作区算、一段一段比、不存在的那一片不算、macOS 和 Windows 上数据根不分大小写、`within`、这台机器的清单 |
| `crates/miyu-fs/tests/resolve.rs` | 相对的照工作目录接、`.` 和走过存在的目录再 `..`、绝对的照原样、`~` 和 `~alice`、没有家目录、还不存在的照上级算、还不存在的 `..`（Unix 报错、Windows 照字面消掉）、链接照指向的地方算、指向不存在处的链接、Windows 两种分隔符；最后一段不跟链接：指向不存在处的、指到别处的链接交回链接本身，`.`、`..`、`~` 交回空的，`src/.` 是 `src` |
| `crates/miyu-fs/tests/open.rs` | 普通文件打得开、目录和不存在的、最后一层是链接不跟（Unix、Windows）、FIFO 不卡住、设备和套接字 |
| `crates/miyu-fs/tests/wide.rs` | 太宽的四样；家目录读不出来时 |
| `crates/miyu-fs/src/replace/tests.rs` | 新建和覆盖、不留临时文件、只读的不写、盖不上去时临时文件删掉 |
| `crates/miyu-fs/tests/trash.rs` | Linux：移回来、`.trashinfo` 删了、上级目录没了的建上、只删回收站里的记录、回收站里没有了的移不回来；macOS、Windows（在 CI 上）：文件、目录放进系统的回收站再移回来，Windows 的 `$I` 删了 |
| `crates/miyu-fs/src/trash/recycled/tests.rs` | `$I` 第 2 版、第 1 版，认不出的、不够长的、字数说得比记录长的，`$I` 在 `$R` 旁边 |
| `crates/miyu-basesystem/tests/trash.rs` | 经 `trash` 这件工具：Linux 上放进家目录的回收站、记录的样子、`files/` 和 `info/` 是 `0700`、重名接 `.2`、同名却没有记录的不盖、目录和链接、转义、挪不动的不删也不留记录、家目录的回收站建不了的不删 |
| `crates/miyu-basesystem/tests/write.rs` | 经 `write`：原来的权限照留、不留临时文件、只读的不写 |

没测到的：Linux 另一块盘上的 `.Trash/<uid>`、`.Trash-<uid>`（测试机上造不出另一块盘）；`XDG_DATA_HOME` 那一条（测试里不改环境变量）。

### 出处

- `11-权限与沙盒.md` 第四节（边界的默认值、几片重叠时谁说了算、第一版的清单、当前目录太宽）、第七节（核心进程里的文件工具、第一版怎么做）、A4、A9。
- `10-自带软件.md` 第三节（「`write` 的细则」：先写临时文件再改名盖上去；「`trash` 的细则」：三个平台怎么放）、第七节（「改回文件的细则」：移回来）。

### 还没有的

- 检查完、打开前，上级目录被别的进程换成了链接：这一版挡不住；M5 有了沙盒，换成按真实位置逐层打开（Linux 的 `openat2`、macOS 的 `O_NOFOLLOW_ANY`）（`11-权限与沙盒.md` 第七节）。
- 边界清单是策略数据，配置那一步能改（第四节）：现在写在代码里。
- 工具链的缓存可以写：说的是沙盒里的命令，随 M5；核心进程里的文件工具对它们只读（第四节）。
- 成员的边界（只能碰自己的工作区）、别人分享来的工作区加进边界（第三节、第四节，`06-多用户与身份.md` U12）。
- 放不进回收站时问人要不要永久删除：随 M8 的当场确认（`10-自带软件.md` 第三节）。
- Windows 的回收站设成「立即删除」、东西比回收站的容量上限还大：还没验证会怎样（第三节）。
