## `glob`

### 是什么

按名字的模式找文件：往下走目录，遵守 `.gitignore`，只列文件，按修改时间新的在前，最多 100 个。模式照 ripgrep 的 `--glob`。不报效果。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/glob.rs` | 参数、搜哪个目录、要碰的路径、结果 |
| `crates/miyu-basesystem/src/pattern.rs` | 模式的规矩；绝对路径的模式拆出目录。`grep` 的 `glob` 也照它 |
| `crates/miyu-basesystem/src/walk.rs` | 往下走目录：忽略文件、版本库目录、链接、数据根、排序。`grep` 也照它 |
| `crates/miyu-basesystem/src/blocking.rs` | 在阻塞线程里干，叫停时举旗 |
| `crates/miyu-basesystem/src/common.rs`、`common/shown.rs` | 几件共用的几句；路径怎么写 |
| `resources/software/basesystem/tools/glob.json` | 说明和参数格式 |
| `resources/software/basesystem/glob/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`。`resources/software/basesystem/tools/glob.json` 原文：

```json
{
  "description": "Find files by glob pattern, like `**/*.rs` or `src/*.ts`, respecting .gitignore. Returns up to 100 paths, most recently modified first.",
  "parameters": {"type":"object","properties":{"pattern":{"type":"string","description":"A pattern without / matches file names at any depth."},"path":{"type":"string","description":"The directory to search in. Default is the working directory."}},"required":["pattern"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `pattern` | 是 | 模式（下面「模式」） |
| `path` | 否 | 搜的目录：绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。没给、写成空串、`"undefined"`、`"null"` 的，都是工作目录 |

- 别的参数不认，也不报错。
- 模式本身是绝对路径（或者是 `~`、以 `~/`、`~\` 开头）的，`path` 不管，搜的目录从模式里拆出来（下面第 2 条）。
- 报给权限策略的路径：一条，搜的目录（她给的写法，或者从模式里拆出来的那一截），读。

### 怎么走

1. 读参数。读不懂的（没写 `pattern`、类型不对）：参数不对。
2. 定搜哪个目录、照什么模式：
   - 模式是绝对路径的（或者是 `~`、以 `~/`、`~\` 开头）：照路径一段一段看，第一段带通配字（`*`、`?`、`[`、`{`）的前面那几段是搜的目录，从这一段起用 `/` 连起来、开头补一个 `/`，是模式；一段通配字都没有的，最后一段以前是目录，最后一段是模式（`/proj/Cargo.toml` 只找 `/proj` 那一层的 `Cargo.toml`）。只看一段一段的名字：Windows 换真实位置得来的 `\\?\C:` 里的 `?` 不算通配。拆出来的目录是空的（整条只有一段，例如 `~` 自己，或者 Unix 上 `~\` 开头的：那里 `\` 不是分隔符），搜的就是工作目录。
   - 别的：搜的目录是 `path`（没给的是工作目录 `.`），模式照原样。
3. 认模式，写法不对（例如 `[` 没配上 `]`）：通配写得不对，带上哪里不对。
4. 搜的目录换成真实的位置（`fs.md` 第二节），换不成：读的时候出错了。
5. 看它是什么（跟着链接）：不是目录：`"{path}" is not a directory.`，出错；没有：没有这个文件，带上相近的名字（`tools/read.md` 第 9 条）；读不了：读的时候出错了。
6. 往下走（下面「走目录」），留下相对搜的目录的路径对得上模式的普通文件。
7. 一个都没有：`No files found`，不算出错。
8. 有的：照新的在前，最多列 100 个，一个一行。路径在这一轮的工作目录里的写相对工作目录的（不是相对搜的目录），在外面的写绝对的；分隔符照平台原生的；Windows 上不写 `\\?\`。列出来的路径不转义。
9. 多过 100 个的，列完接上 `(Showing {shown} of {total} matching files; {rest} more are not listed. Narrow the pattern or path to see the rest.)`：`shown` 是 100，`total` 是一共几个，`rest` 是没列的几个。
10. 叫停：丢掉这次调用时举旗，走目录每走一步看一眼，举了就不往下走。没有超时。

**模式**（`pattern.rs`）：

1. Windows 上 `\` 先换成 `/`，和 `/` 一样当分隔符；所以 Windows 上写不了 `\` 的转义。别的平台 `\` 是转义。
2. 开头的 `/` 只是说「从搜的那个目录算起」，去掉以后再认。
3. 开头有 `/` 或者里面带 `/` 的：照相对搜的目录、用 `/` 连起来的整条路径比；不带的：只比文件名，在任何一层都算。
4. `**` 跨目录（`**/` 开头的连最上面一层也算）；`*`、`?` 不跨 `/`；`{a,b}` 二选一；`[abc]` 挑一个字。
5. 大小写照原样比，三个平台一样。

| 模式 | 对得上 | 对不上 |
|---|---|---|
| `*.rs` | `a.rs`、`src/deep/b.rs` | `src/b.rs.bak` |
| `src/*.rs` | `src/a.rs` | `src/deep/a.rs`、`other/src/a.rs` |
| `src/**/*.rs` | `src/a.rs`、`src/deep/a.rs` | |
| `**/*.ts` | `a.ts`、`web/app/a.ts` | |
| `/Cargo.toml` | `Cargo.toml` | `sub/Cargo.toml` |
| `*.{ts,tsx}` | `a.ts`、`src/a.tsx` | `a.js` |

**走目录**（`walk.rs`，`grep` 一样）：

1. 忽略文件：`.gitignore`、`.ignore`，不在 git 仓库里也遵守。
   - 在仓库里（搜的目录或者它的哪一层上级里有 `.git`）：上级目录里的忽略文件也算；git 的规矩照仓库来。
   - 不在仓库里：只认搜的那个目录往下的。家目录里有一份写着 `*` 的 `.gitignore`（拿 git 管 dotfiles 的常这么配），往上找就什么都搜不到了。
   - git 的全局忽略（`core.excludesFile`）和 `.git/info/exclude` 也算：`ignore` 这个库默认开着。
2. 隐藏文件照找。版本库自己的目录 `.git`、`.svn`、`.hg`、`.bzr`、`.jj`、`.sl` 不进（只看目录；搜的目录自己不算）。
3. 不跟着链接走：链接到的目录不进，链接到的文件也不列，和 ripgrep 不加 `-L` 一样（本机实测过）。链接可能指到工作区外面，权限策略只判了要搜的那个目录（`session/guard.md`）。
4. 数据根不进：要进的目录在数据根里（数据根自己也算）、又不在这一轮的工作目录里（工作目录自己也算）的，不进。搜的目录自己不查：它是报给权限策略的那一条。macOS、Windows 上比的时候不分大小写（`fs.md` 第一节第 6 条）。所以从数据根上面搜下来，连在数据根里的账号工作区也不进；工作目录就在账号工作区里时，从工作目录往下照样走。
5. 读不了的目录（没有权限这类）跳过，别的照走。
6. 只留普通文件。
7. 按修改时间排，新的在前；读不出修改时间的当最早；时间一样的，照相对搜的目录的路径（用 `/` 连起来）排。
8. 叫停了，不往下走，交回已经走到的。

### 样子

`*.rs`，`src/deep/c.rs` 最新、`a.rs` 最旧：

```text
src/deep/c.rs
src/b.rs
a.rs
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 一个都没找到 | `common/no-files.txt` | `No files found` |
| 多过 100 个 | `glob/more.txt` | `(Showing {shown} of {total} matching files; {rest} more are not listed. Narrow the pattern or path to see the rest.)` |

### 出错

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `The arguments are not right: {error}.` | `common/bad-args`，字段 `error` |
| 模式写得不对 | `The glob "{glob}" is not valid: {error}.`（`common/bad-glob.txt`） | `common/bad-glob`，字段 `glob`、`error` |
| 搜的目录换不成真实的位置、读不了 | `Could not read "{path}": {error}.` | `common/failed`，字段 `path`、`error` |
| 搜的目录没有 | `There is no file or directory at "{path}".`，加上相近的名字 | `common/missing` 或 `common/missing-similar` |
| 搜的不是目录 | `"{path}" is not a directory.`（`glob/not-a-directory.txt`） | `glob/not-a-directory`，字段 `path` |

- `{glob}` 是她写的 `pattern` 原样，`{error}` 是 `globset` 的原话，例如 `unclosed character class; missing ']'`。
- `{path}` 是搜的目录（她给的写法，或者从模式里拆出来的那一截）。换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）。

### 给人看的字

显示名：找文件（Find files），后面跟 `pattern` 的值。

| 说法 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `glob/files`（`count`） | 全列了 | `{count} 个文件` | `{count} files` |
| `glob/files-more`（`shown`、`total`） | 多过 100 个 | `{total} 个文件，列了 {shown} 个` | `{total} files, {shown} listed` |
| `common/no-files` | 一个都没找到 | 一个都没找到 | none found |
| `glob/not-a-directory`（`path`） | 搜的不是目录 | 不是目录 | not a directory |
| `common/bad-glob`（`glob`、`error`） | 模式写得不对 | `通配写得不对：{error}` | `bad glob: {error}` |
| `common/missing`、`common/missing-similar`、`common/failed`、`common/bad-args` | 见 `tools/read.md` | | |

### 三个平台

| | Linux | macOS | Windows |
|---|---|---|---|
| 模式里的 `\` | 转义 | 转义 | 分隔符 |
| 数据根跳过时比大小写 | 分 | 不分 | 不分 |
| `~\` 开头的模式、`path` | 模式照上面第 2 条拆出空目录；`path` 里的 `~\` 不当家目录 | 同 Linux | 都当家目录 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/glob.rs` | 从资源目录造、参数格式、报的路径（`undefined` 当没给、模式是绝对路径的报拆出来的目录）；只比文件名在任何一层、带 `/` 的比路径；时间一样照路径排；不在仓库里也遵守 `.gitignore`、`.ignore`；在仓库里上级的算、不在仓库里上级的不算；隐藏的照找、版本库目录不进；外面的写绝对路径、数据根不进；工作区在数据根里照样往下走；绝对路径的模式；多过 100 个；没找到不算出错、写错了算；搜的要是存在的目录 |
| `crates/miyu-basesystem/src/pattern/tests.rs` | 模式的每一条规矩、写错了说哪里错、绝对路径的模式怎么拆（Windows 的前缀里的 `?`） |
| `crates/miyu-basesystem/src/walk/tests.rs` | 丢掉这次调用，走目录就停 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/search.rs` | 会话里真的调它：工作区里找得到、越界的在没人能确认的会话里被拒；从上面搜下来不进数据根 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`glob`、`grep` 输出的写法」）、第八节（B9：自带实现，三个平台一样）、第十节（规范；遵守 `.gitignore`、新的在前这两处不照 Claude Code）。
- `11-权限与沙盒.md` 第七节、A9：数据根谁都不能碰。
- `05-内核接口.md` 第六节「执行这一步」：交给工具的数据根、叫停时举旗。
- `26-提示词.md` 第十节：登记簿里的 `tools/glob.json`、`glob/*.txt`、`common/*.txt`。
