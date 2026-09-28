## `grep`

### 是什么

照正则搜文件的内容：往下走目录，遵守 `.gitignore`，二进制文件跳过。三种输出：只列文件（默认）、匹配的行、每个文件几行匹配；照 `head_limit`、`offset` 分页。正则、按行搜、二进制跳过、前后行都用和 ripgrep 同一套库。不报效果。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/grep.rs` | 参数、一页怎么定、要碰的路径、搜哪些文件 |
| `crates/miyu-basesystem/src/grep/search.rs` | 认正则；三种搜法；二进制；一行怎么截 |
| `crates/miyu-basesystem/src/grep/render.rs` | 三种输出的写法、分页、给人看的说法 |
| `crates/miyu-basesystem/src/walk.rs`、`pattern.rs` | 走目录、`glob` 的模式：和 `glob` 共用（`tools/glob.md`） |
| `crates/miyu-basesystem/src/common.rs`、`common/shown.rs` | 几件共用的几句；路径怎么写 |
| `resources/software/basesystem/tools/grep.json` | 说明和参数格式 |
| `resources/software/basesystem/grep/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`。`resources/software/basesystem/tools/grep.json` 原文：

```json
{
  "description": "Search file contents with a regular expression (ripgrep syntax), respecting .gitignore. Prefer this over grep or rg in the shell. `output_mode` picks file paths (default), matching lines, or counts per file.",
  "parameters": {"type":"object","properties":{"pattern":{"type":"string","description":"Literal braces need escaping, like interface\\{\\}."},"path":{"type":"string","description":"The file or directory to search. Default is the working directory."},"glob":{"type":"string","description":"Searches only files matching this glob, like *.rs."},"output_mode":{"type":"string","enum":["content","files_with_matches","count"],"description":"Default files_with_matches."},"-i":{"type":"boolean","description":"Case-insensitive."},"context":{"type":"integer","description":"Lines shown before and after each match in content mode."},"head_limit":{"type":"integer","description":"Results to show at most. Default 250. 0 means no limit."},"offset":{"type":"integer","description":"Results to skip first. Default 0."}},"required":["pattern"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `pattern` | 是 | 正则，Rust `regex` 的写法（ripgrep 用的那一套）；分大小写 |
| `path` | 否 | 搜的文件或目录：绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。没给、空串、`"undefined"`、`"null"` 的，是工作目录 |
| `glob` | 否 | 只搜对得上这个模式的文件，模式照 `tools/glob.md`，比的是相对搜的目录的路径。写成 `include` 的也认。空串、`"undefined"`、`"null"` 当没给 |
| `output_mode` | 否 | `content`、`files_with_matches`、`count`，默认 `files_with_matches`；别的值参数不对 |
| `-i` | 否 | 真的就不分大小写 |
| `context` | 否 | `content` 时匹配的行前后各带几行。写成 `-C` 的也认。在的时候压过 `-A`、`-B` |
| `head_limit` | 否 | 这一页最多几条，默认 250，0 是不限 |
| `offset` | 否 | 先跳过几条，默认 0 |

- 参数格式里不写、写了也认的：`-A`（匹配的行后面带几行）、`-B`（前面带几行），`context` 没给时才用。
- 负数的 `context`、`-A`、`-B`、`head_limit`、`offset` 当没给。
- `glob` 和 `include`、`context` 和 `-C`，两个都写的，参数不对。
- 「条」在三种输出里各是：文件、匹配的行、文件。
- 声明了的参数写成字符串的，内核先修正；没声明的 `-A`、`-B`、`-C` 写成字符串的，参数不对。别的参数不认，也不报错。
- 报给权限策略的路径：一条，`path`（没给的是 `.`），读。

### 怎么走

1. 读参数，读不懂的：参数不对。
2. 认正则：`-i` 为真的不分大小写。按行搜，一处匹配跨不了行；正则里写了换行（`\n`）的认不了。写法不对：正则写得不对，带上报错的最后一句（去掉开头的 `error: `）。
3. 认 `glob`，写法不对：通配写得不对。
4. `path` 换成真实的位置（`fs.md` 第二节），换不成：读的时候出错了。
5. 看它是什么（跟着链接）：
   - 目录：往下走（`tools/glob.md`「走目录」），留下 `glob` 对得上的普通文件，照新的在前；没给 `glob` 的全留。
   - 别的（点名的一个文件）：只搜它，不管 `glob`。不看它是不是普通文件：FIFO、设备照样普通地打开去搜（不加非阻塞），FIFO 没人写会一直等着，叫停了也还等着：那个阻塞线程收不回来。
   - 没有：没有这个文件，带上相近的名字（`tools/read.md` 第 9 条）。
   - 读不了：读的时候出错了。
6. 一个一个文件照先后搜：
   - 打不开、读不了的文件跳过，当没搜到。
   - 带 BOM 的 UTF-16 照认（转成 UTF-8 再搜）。
   - 读到 NUL 字节的当二进制文件，整个跳过，前面搜到的也不算。一次读 64 KiB；只读到停下来的地方为止（只列文件时搜到第一处就停，列行时搜够数就停），没读到的那一段里有 NUL 也不知道。
   - 叫停了，不往下搜。
7. 照 `output_mode`：
   - `files_with_matches`：每个文件搜到第一处就停，有的列出来。
   - `count`：每个文件数有几行匹配，有的写 `<路径>:<几行>`。
   - `content`：收下匹配的行和前后带出来的行，一共收够 `offset + min(head_limit, 65536) + 1` 行匹配就停（`head_limit` 是 0 的是不限，按 65536 算）：多收一行，才知道后面还有没有。
8. 一行的字：去掉行尾的换行和 `\r`，不是 UTF-8 的字节换成 `�`，最长 500 个字，多的截掉、补一个 `…`。
9. 路径照结果的规矩写：在这一轮的工作目录里的写相对的，在外面的写绝对的；分隔符照平台原生的；Windows 上不写 `\\?\`。列出来的路径、行不转义。
10. 叫停：丢掉这次调用时举旗，走目录、搜下一个文件之前看一眼。一个文件搜到一半不停。没有超时。

**只列文件、计数**（一共几条是知道的）：

1. 一个都没有：只列文件的说 `No files found`，计数的说 `No matches found`，都不算出错。
2. `offset` 不小于一共几条：`(There are only {total} results; offset {offset} skips them all.)`，不算出错。
3. 从第 `offset + 1` 条起，最多 `head_limit` 条，一条一行；最多 64 KiB，第一条照写。
4. 后面还有的，接上 `(Showing files {from}-{to} of {total}. Use offset={next} to continue.)`：`from` 是 `offset + 1`，`to` 是写到的最后一条，`next` 就是 `to`。计数也是这一句。

**匹配的行**（搜够数就停，不数一共几条）：

1. 一处都没有：`No matches found`，不算出错。
2. `offset` 不小于收下的几行匹配：`(There are only {total} results; offset {offset} skips them all.)`，不算出错。
3. 跳过前 `offset` 行匹配，写接下来的最多 `head_limit` 行。匹配的行写成 `<路径>:<行号>:<字>`；前后带出来的行写成 `<路径>-<行号>-<字>`，只写离这一页要写的某一行匹配够近的：同一个文件里，在它前面、前面带的行数以内，或者在它后面、后面带的行数以内（给了 `context` 的，前后都是它；不然前面是 `-B`、后面是 `-A`）。跳过的、这一页以外的匹配，前后行也不写。
4. 带前后行时（前面、后面带的行数有一个大过 0），不连着的两段之间写一行 `--`，换了文件也算不连着；第一段前面不写。
5. 最多 64 KiB：写到第一行匹配为止照写，之后写上这一行就超过的，停在那里。
6. 停在 64 KiB，或者收下的匹配比写了的多：接上 `(Showing matches {from}-{to}; there are more. Use offset={next} to continue.)`，`to` 是 `offset` 加写了的匹配数，`next` 就是 `to`。

### 样子

`files_with_matches`（`c.txt` 比 `a.rs` 新）：

```text
c.txt
a.rs
```

`content`：

```text
c.txt:1:the main road
a.rs:1:fn main() {
a.rs:2:    println!("main");
```

`count`：

```text
c.txt:1
a.rs:2
```

`content`、`context` 是 1，第 2、8 行匹配：

```text
f.txt-1-line 1
f.txt:2:hit 2
f.txt-3-line 3
--
f.txt-7-line 7
f.txt:8:hit 8
f.txt-9-line 9
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 只列文件，一个都没有 | `common/no-files.txt` | `No files found` |
| 匹配的行、计数，一处都没有 | `grep/no-matches.txt` | `No matches found` |
| 只列文件、计数，还有 | `grep/more-files.txt` | `(Showing files {from}-{to} of {total}. Use offset={next} to continue.)` |
| 匹配的行，还有 | `grep/more-matches.txt` | `(Showing matches {from}-{to}; there are more. Use offset={next} to continue.)` |
| `offset` 全跳过了 | `grep/past-end.txt` | `(There are only {total} results; offset {offset} skips them all.)` |

### 出错

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `The arguments are not right: {error}.` | `common/bad-args`，字段 `error` |
| 正则写得不对 | `The pattern is not a valid regular expression: {error}.`（`grep/bad-pattern.txt`） | `grep/bad-pattern`，字段 `error` |
| `glob` 写得不对 | `The glob "{glob}" is not valid: {error}.` | `common/bad-glob`，字段 `glob`、`error` |
| 换不成真实的位置、读不了 | `Could not read "{path}": {error}.` | `common/failed`，字段 `path`、`error` |
| 没有 | `There is no file or directory at "{path}".`，加上相近的名字 | `common/missing` 或 `common/missing-similar` |

- 正则的 `{error}` 例如 `unclosed group`（`(main`）。换进句子的字段照模板的规矩转义（`tools/read.md` 第 8 条）。
- 先认正则、再认 `glob`、再看路径：几样都错的，报前面那一样。

### 给人看的字

显示名：搜内容（Search），后面跟 `pattern` 的值。

| 说法 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `grep/files`（`count`） | 只列文件，全列了 | `{count} 个文件` | `{count} files` |
| `grep/files-part`（`from`、`to`、`total`） | 只列文件，列了一段 | `第 {from}–{to} 个文件，一共 {total} 个` | `files {from}–{to} of {total}` |
| `grep/counts`（`count`） | 计数，全列了 | `{count} 个文件` | `{count} files` |
| `grep/counts-part`（`from`、`to`、`total`） | 计数，列了一段 | `第 {from}–{to} 个文件，一共 {total} 个` | `files {from}–{to} of {total}` |
| `grep/matches`（`count`） | 匹配的行，从头写到完 | `{count} 处` | `{count} matches` |
| `grep/matches-part`（`from`、`to`） | 匹配的行，跳过了一些、后面没有了 | `第 {from}–{to} 处` | `matches {from}–{to}` |
| `grep/matches-more`（`from`、`to`） | 匹配的行，后面还有 | `第 {from}–{to} 处，后面还有` | `matches {from}–{to}, more after` |
| `grep/none` | 一个都没有（三种都是） | 没搜到 | no matches |
| `grep/past-end`（`total`、`offset`） | `offset` 全跳过了 | `跳过了全部，一共 {total} 条` | `all skipped, {total} results` |
| `grep/bad-pattern`（`error`） | 正则写得不对 | `正则写得不对：{error}` | `bad regex: {error}` |
| `common/bad-glob`、`common/missing`、`common/missing-similar`、`common/failed`、`common/bad-args` | 见 `tools/glob.md`、`tools/read.md` | | |

- 「全列了」是 `offset` 为 0、写到了最后一条；别的是「一段」。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/grep.rs` | 参数格式里的八个参数、报的路径；默认只列文件、新的在前；三种输出的写法；前后行和 `--`、`-C`、`context` 压过 `-A`、只要 `-A`；跳过的匹配不带它的前后行、`head_limit` 以外的也不带；`glob`、`include`、点名一个文件不管 `glob`；忽略的、二进制的、数据根里的不搜；`-i`、UTF-16；`head_limit`、`offset` 翻页、0 不限、`content` 够数就停；默认 250；一行截到 500；没搜到不算出错、正则和 `glob` 写错了算、没有的路径 |
| `crates/miyu-basesystem/tests/glob.rs`、`src/pattern/tests.rs`、`src/walk/tests.rs` | 走目录、模式、叫停（和 `glob` 共用） |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/search.rs` | 会话里真的调它：工作区里搜得到、越界的被拒；从上面搜下来不进数据根 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`glob`、`grep` 输出的写法」）、第八节（B9）、第十节（规范；超长的行截掉、只声明常用的几个参数，这两处不照 Claude Code）。
- `11-权限与沙盒.md` 第七节、A9。
- `26-提示词.md` 第十节：登记簿里的 `tools/grep.json`、`grep/*.txt`、`common/*.txt`。
