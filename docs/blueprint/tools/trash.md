## `trash`

### 是什么

把一个文件或者目录移进系统的回收站，记下它在回收站里的真实位置，报 `file.trashed`；撤销时照它移回来。链接删的是链接本身。工作目录和它的上级、家目录、根目录不许删。回收站收不了的不删，说为什么。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/trash.rs` | 参数、要碰的路径、换成真实的位置（最后一段不跟链接）、不许删的、结果和效果 |
| `crates/miyu-fs/src/trash.rs` 和 `trash/` | 各平台的回收站：放进去、移回来（`fs.md` 第六节） |
| `crates/miyu-basesystem/src/common.rs` | 几件共用的几句 |
| `resources/software/basesystem/tools/trash.json` | 说明和参数格式 |
| `resources/software/basesystem/trash/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `write`。说明和参数的原文如下。

样本 `resources/software/basesystem/tools/trash.json`：

```json
{
  "description": "Move a file or directory to the system trash, where it can be restored. Use this instead of rm in the shell.",
  "parameters": {"type":"object","properties":{"file_path":{"type":"string","description":"Absolute, or relative to the working directory."}},"required":["file_path"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `file_path` | 是 | 要删的文件、目录或者链接：绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。写成 `path`、`filePath` 的也认；写了两种的，参数不对 |

- 一次删一个，要删几个就调几次。删之前不用先读。
- 别的参数不认，也不报错。
- 报给权限策略的路径：一条，`file_path` 原样，写，碰的是这一条本身（`itself`）。只读时内核当场拦下。权限策略照 `resolve_itself` 换真实的位置判（`session/guard.md`），和这件工具删的是同一个：工作区里的链接，不管指向不存在处、指进数据根还是指到外面，判的都是链接本身在哪，工作区这一级放行。

### 怎么走

1. 读参数，读不懂的：参数不对。
2. 照 `resolve_itself` 换成真实的位置，最后一段不跟链接（`fs.md` 第二节第 7 条）：
   1. `~` 开头的照 `tilde` 接家目录（`fs.md` 第二节第 1 条）：只有 `~` 自己的（`~`、`~/`，Windows 上还有 `~\`），不许删；家目录读不出来的：读的时候出错了。
   2. 没有名字可删的（`.`、`..`、根目录，以 `..` 结尾的）：不许删。以 `/.` 结尾的，删的是前面那一段（`src/.` 删的是 `src`）。
   3. 上级目录照 `resolve` 换成真实的位置（`fs.md` 第二节），再接上最后那个名字。换不成：读的时候出错了。
3. 不跟链接看它：看不到的（不在，或者读不了）：没有这个文件，带上相近的名字（`tools/read.md` 第 9 条）。
4. 不许删的：
   - 工作目录本身和它的每一层上级：这一轮的工作目录换成真实的位置，要删的就是它，或者它在要删的那个里面，不许删。根目录是工作目录的上级。工作目录换不成真实位置的，这一条不查。
   - 系统的家目录：家目录换成真实的位置（换不成照原样）以后，和要删的一样的，不许删。
   - 一段一段比，macOS、Windows 上不分大小写（`within`，`fs.md`）：最后一段是她写的原样（只换了上级），换个大小写写（工作目录是 `Proj`，她写 `../proj`）照样拦得住。
5. 放进系统的回收站（`fs.md` 第六节）：
   - 成了：`Moved "{path}" to the trash.`，路径照结果的规矩写（在这一轮的工作目录里的写相对的，在外面的写绝对的，Windows 上不写 `\\?\`）。报 `file.trashed`：`path` 是移走之前的真实位置（最后一段不跟链接），`trash` 是它在回收站里的真实路径。
   - 这块盘上没有能放的回收站：不删，出错，告诉她问人，或者真要删用 shell 里的 `rm`。
   - 挪了，可回收站里找不到它（只有 macOS、Windows）：出错，说它可能回不来了；不报效果。
   - 出错了：删不了，带上系统的原话；没删。
6. 目录整个挪进去。链接交给回收站的是链接本身（最后一段不跟链接）；Linux 上测过：链接挪走了，它指向的东西不动，指向不存在处的链接也挪得动。
7. 删掉的，会话从她看过的里拿掉（`tools/interface.md` 第四节）：只拿掉这一条路径，删的是目录的，里面她看过的文件还算看过。
8. 撤销这一轮时，照 `trash` 记下的位置把它移回原处（`fs.md` 第六节「移回来」）。
9. 叫停：删在阻塞线程里做。移进回收站之前看一眼叫停的旗，举了就不删，交回 `stopped`，什么都没动；已经移进去的照常交回。没有超时。

### 三个平台

| 平台 | 放在哪 | 记下的位置 | 收不了的 |
|---|---|---|---|
| Linux | 照 freedesktop 回收站规范：和家目录的回收站在同一块盘上的放那里，不在的放那块盘最上面那一层的 `.Trash/<uid>` 或 `.Trash-<uid>`；`info/` 里写 `.trashinfo` | `files/<名字>` 的绝对路径，重名的接 `.2`、`.3` | 建不了回收站、跨了盘 |
| macOS | 系统的 `NSFileManager` | 系统交回的新位置 | 系统说这块盘不支持 |
| Windows | `trash` 这个 crate 删进回收站，再读 `$I` 记录找回它 | `$R` 开头的那个真实路径 | 网络路径、盘的根目录下没有 `$Recycle.Bin` |
| 别的系统 | | | 一律收不了 |

细节见 `fs.md` 第六节。

### 样子

```text
Moved "a.txt" to the trash.
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 移进了回收站 | `trash/trashed.txt` | `Moved "{path}" to the trash.` |
| 收不了，没删 | `trash/unavailable.txt` | `"{path}" was not deleted: its drive has no trash to move it into. Ask the user, or use rm in the shell to delete it for good.` |
| 不许删 | `trash/protected.txt` | `"{path}" cannot be deleted: it is the working directory, one of its parents, the home directory, or the root.` |
| 挪了却找不到 | `trash/lost.txt` | `"{path}" was deleted, but it was not found in the trash afterwards, so it may not come back.` |
| 删不了 | `trash/failed.txt` | `Could not delete "{path}": {error}.` |

- 移进了回收站那一句的路径照结果的规矩写；别的几句里的 `{path}` 是她给的原样。换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）。

### 出错

出错的结果都标成出错，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 不许删 | `trash/protected.txt` | `trash/protected` |
| 家目录读不出来、上级目录换不成真实的位置 | `common/failed.txt`：`Could not read "{path}": {error}.` | `common/failed`，字段 `path`、`error` |
| 看不到（不在，或者读不了） | `common/missing.txt`，加上相近的名字 | `common/missing` 或 `common/missing-similar` |
| 这块盘上没有能放的回收站 | `trash/unavailable.txt` | `trash/unavailable` |
| 挪了却找不到 | `trash/lost.txt` | `trash/lost` |
| 删不了（例如所在的目录只读） | `trash/failed.txt` | `trash/failed`，字段 `error` |

- 删不了的 `{error}` 是系统的原话，例如 Linux 上的 `Permission denied (os error 13)`；macOS 上是系统给的说明。
- 先后：参数不对 → `~` 自己 → 家目录读不出来 → 没有名字可删的 → 上级目录换不成 → 看不到 → 工作目录和它的上级、家目录 → 放进回收站。

### 给人看的字

显示名：删除（Delete），后面跟 `file_path` 的值；符号 `←`。

| 说法 | 中文 | 英文 |
|---|---|---|
| `trash/trashed` | 移进了回收站 | moved to the trash |
| `trash/unavailable` | 放不进回收站，没删 | no trash here, not deleted |
| `trash/protected` | 工作目录、它的上级、家目录、根目录不能删 | the working directory, its parents, home and root can't be deleted |
| `trash/lost` | 删了，可回收站里找不到 | deleted, but not found in the trash |
| `trash/failed`（`error`） | `删不了：{error}` | `can't delete: {error}` |
| `common/missing`、`common/missing-similar`、`common/failed`、`common/bad-args` | 见 `tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/trash.rs` | 报要写的路径、`path` 也认；不许删的几种（`.`、`..`、`~`、`~/`、`~/.`、`/`、工作目录、它的上级、家目录）、没有的；Linux：放进家目录的回收站、记录的样子、报的位置、`files/` 和 `info/` 只有自己能进、重名接 `.2`、同名却没有记录的不盖、目录和链接（指向的东西不动、指向不存在处的也删得掉）、转义、挪不动的不删不留记录、家目录是链接的照样拦、家目录的回收站建不了的不删；macOS、Windows（在 CI 上）：文件、目录进了回收站，报的位置真有这个东西，换个大小写写的工作目录、家目录照样不许删 |
| `crates/miyu-fs/tests/trash.rs`、`src/trash/recycled/tests.rs` | 移回来、`$I` 记录（`fs.md`） |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-basesystem/tests/stop.rs` | 旗举了不删，文件还在原处 |
| `crates/miyu-session/tests/write.rs` | 会话里删一个文件（Linux）：日志里有 `file.trashed`，位置真有这个文件；原处再有同名的，她没看过 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`trash` 的细则」）、第五节（`file.trashed`）、第七节（撤销时移回来）。
- `26-提示词.md` 第十节：登记簿里的 `tools/trash.json`、`trash/*.txt`。

### 还没有的

- 放不进回收站时，问人要不要永久删除；没人能确认的照样不删：随 M8 的当场确认（`10-自带软件.md` 第三节）。
- Windows 的回收站设成「立即删除」、东西比回收站的容量上限还大时会怎样，还没验证（第三节）。
