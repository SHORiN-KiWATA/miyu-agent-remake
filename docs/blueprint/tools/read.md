## `read`

### 是什么

读一份文本文件，按行分页、带行号；读到图片（PNG、JPEG、GIF、WebP）时交回图片本身（施工 4-13）；读到目录时照名字列出里面的每一项，一样分页。读到文本文件、图片、二进制文件时报 `file.read`：读了第几行到第几行、整份文件的内容哈希，写的工具改之前照它核对。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/read.rs` | 参数、要碰的路径、读文件还是列目录、结果和效果 |
| `crates/miyu-basesystem/src/read/lines.rs` | 按行读：编码、二进制、分页、行号、截长行、整份的哈希 |
| `crates/miyu-basesystem/src/read/dir.rs` | 列目录 |
| `crates/miyu-basesystem/src/read/image.rs` | 认图片、量宽高、上限（施工 4-13） |
| `crates/miyu-basesystem/src/common.rs` | 几件共用的几句：没有这个文件、读的时候出错、参数不对 |
| `crates/miyu-basesystem/src/common/shown.rs`、`similar.rs` | 路径怎么写给她看；相近的名字 |
| `crates/miyu-basesystem/src/load.rs` | 从资源目录读说明、参数格式和几句字 |
| `resources/software/basesystem/tools/read.json` | 说明和参数格式 |
| `resources/software/basesystem/read/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`。说明和参数的原文如下。

改成（施工 4-13 合进来时标成样本 `resources/software/basesystem/tools/read.json`）：

```json
{
  "description": "Read a text file or an image (PNG, JPEG, GIF, WebP), or list a directory. Lines come back in cat -n format, numbered from 1, up to 2000 at a time. Prefer this over `cat` in the shell: files read here come back after compaction.",
  "parameters": {"type":"object","properties":{"file_path":{"type":"string","description":"Absolute, or relative to the working directory."},"offset":{"type":"integer","description":"The line number to start reading from, counting from 1."},"limit":{"type":"integer","description":"The number of lines to read. Default 2000."}},"required":["file_path"]}
}
```

`description` 原样进 tools 数组，`parameters` 那一行一个字节不改。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `file_path` | 是 | 绝对路径、相对这一轮工作目录的路径、`~` 开头的路径（`fs.md` 第二节）。写成 `path`、`filePath` 的也认，参数格式里不写；写了两种的，参数不对。空串就是工作目录本身 |
| `offset` | 否 | 从第几行（目录：第几项）起，从 1 数起。没给、给了 0 或者负数，从第 1 行起 |
| `limit` | 否 | 最多几行（几项）。没给、给了 0 或者负数，照 2000；大过 2000 的也照 2000 |

- `offset`、`limit` 要是整数；写成字符串的，内核先修正（`tools/interface.md` 第二节）；写成小数的，参数不对。
- 别的参数不认，也不报错。
- 报给权限策略的路径：一条，`file_path` 原样，读。

### 怎么走

1. 读参数。读不懂的（没写 `file_path`、类型不对）：参数不对。
2. 换成真实的位置（`fs.md` 第二节），照调用的工作目录、家目录。换不成：读的时候出错了，原因是换不成的那一句。
3. 安全地打开（`fs.md` 第四节）：
   - 是目录：列目录（第 7 条）。
   - 没有：没有这个文件，带上相近的名字（第 9 条）。
   - 不是普通文件、也不是目录（FIFO、设备、套接字这类）：说一句，出错。
   - 别的错（例如没有权限）：读的时候出错了，系统的原话。
4. 开头的字节是图片的，走下面的「读图片」；别的认编码，看前 8 KiB（8192 字节）：
   1. 开头是 `FF FE` 或 `FE FF`：带 BOM 的 UTF-16（小端、大端）。整份读进来再解；BOM 不显示；解不开的换成 `�`；落单的最后一个字节不算。
   2. 别的，前 8 KiB 里有 NUL 字节：二进制文件，不读内容，出错；照样过一遍整份算哈希，报 `file.read`，没有 `lines`（施工 4-9 再补二）：她知道它在、是二进制，`write` 盖它之前照它核对（`tools/write.md`）。
   3. 别的当 UTF-8：开头的 BOM（`EF BB BF`）不显示；每一行里解不开的字节换成 `�`。边读边分行，不把整份读进内存。
5. 分行、挑出这一页：
   1. 照 `\n` 分行，行尾的 `\r` 去掉：CRLF 的文件读出来和 LF 的一样。单独的 `\r` 不算换行。以换行结尾的，最后那一段空的不算一行。
   2. 一直读到结尾，数出一共几行。
   3. 从第 `offset` 行起，最多 `limit` 行，每行写成 `<行号>\t<原文>\n`：行号前不补空格，行号后一个制表符。
   4. 一行最长 2000 个字，多的截掉，补一个 `…`。
   5. 这一页最多 64 KiB（65536 字节）：写上这一行就超过的，停在前一行。这一页的第一行照写。接在后面说没读完的那一句不算在里面。
6. 结果（都不算出错）：
   - 一行都没有（空文件、只有 BOM 的）：`(It is empty.)`
   - `offset` 过了结尾：`(The file has {total} lines; offset {offset} is past the end.)`，`{offset}` 是她给的那个数。
   - 有行：那几行；最后写到的一行不是最后一行的（`limit` 到了，或者 64 KiB 到了），接上 `(Showing lines {from}-{to} of {total}. Use offset={next} to continue.)`，`next` 是 `to + 1`。
   - 读的同一遍里算出整份文件的内容哈希，BOM 也算在里面；读了一段的、空的、过了结尾的，也是整份的。报 `file.read`：`path` 是真实的位置；`lines` 是 `[from, to]`，空的、过了结尾的、二进制的没有这一格；`hash` 是整份的。
7. 列目录：
   1. 读不了目录：读的时候出错了。
   2. 每一项写它的名字（解不开的字节换成 `�`），是目录的（不跟链接看）后面加 `/`。读不出来的那一项跳过。隐藏的照列，只列名字，不往下走。
   3. 一项都没有：`(It is empty.)`
   4. 照写出来的样子排（按字节比，目录带着那个 `/` 比）。`offset` 过了一共几项：`(The directory has {total} entries; offset {offset} is past the end.)`
   5. 从第 `offset` 项起，最多 `limit` 项，一项一行；最多 64 KiB（接在后面的那一句不算），第一项照写。
   6. 没列完的，接上 `(Showing entries {from}-{to} of {total}. Use offset={next} to continue.)`
   7. 列目录不报效果。
8. 换进这些句子的字段照模板的规矩转义（`kernel/request.md`）：`\` 写成 `\\`，`"`、`&`、`<`、`>` 写成 `\u0022` 这样，换行写成 `\n`。句子里的 `{path}` 是她给的原样。
9. 没有这个文件时的相近的名字（`glob`、`grep`、`edit`、`trash` 同一套）：
   1. 在换成的真实位置所在的那个目录里找；那个目录读不了的，一个都不列。
   2. 名字都换成小写再比，照这个算有多近，越小越近：只差大小写的是 0；主名一样的是 1（主名是最后一个 `.` 前面那一截；最后一个 `.` 就是打头那个的，例如 `.gitignore`，整个都是主名）；别的，两个名字都至少 4 个字、至多 128 个字，改 2 个字以内（增、删、换各算一个）就一样的，是 1 加改的字数。
   3. 最多列 3 个，越近的越前，一样近的照名字排。每个一行：`Did you mean "{path}"?`
   4. 那里的路径照结果的规矩写：在这一轮的工作目录里的写相对的（目录本身写 `.`），在外面的写绝对的；分隔符照平台原生的；Windows 上不写 `\\?\`（`\\?\UNC\host\share` 写成 `\\host\share`）。
10. 叫停：读在阻塞线程里做，叫停了照样读完，结果没人要。没有超时。

### 样子

读文件（`notes.txt` 有 5 行，`offset` 2、`limit` 2；行号后面是一个制表符）：

```text
2	line 2
3	line 3
(Showing lines 2-3 of 5. Use offset=4 to continue.)
```

读目录：

```text
a/
b.txt
c.rs
```

没有这个文件、有相近的名字：

```text
There is no file or directory at "note.txt".
Did you mean "notes.txt"?
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 没读完 | `read/more.txt` | `(Showing lines {from}-{to} of {total}. Use offset={next} to continue.)` |
| 空的 | `read/empty.txt` | `(It is empty.)` |
| 过了结尾 | `read/past-end.txt` | `(The file has {total} lines; offset {offset} is past the end.)` |
| 目录没列完 | `read/more-entries.txt` | `(Showing entries {from}-{to} of {total}. Use offset={next} to continue.)` |
| 目录过了结尾 | `read/past-end-entries.txt` | `(The directory has {total} entries; offset {offset} is past the end.)` |

**读图片**（施工 4-13）

1. 认格式，看开头的字节，不看扩展名：PNG（`89 50 4E 47 0D 0A 1A 0A`）、JPEG（`FF D8 FF`）、GIF（`GIF87a`、`GIF89a`）、WebP（`RIFF`，第 9 到 12 个字节是 `WEBP`）。这四种是 DeepSeek 收的；别的图（BMP、TIFF、HEIC）照旧当二进制。
2. 文件大过 5 MiB（5,242,880 字节）的：不读内容，出错，图的文件太大（「出错」），`size` 写成 MiB、一位小数，例如 `7.3 MiB`。
3. 整份读进来，量宽高（`imagesize`，只看文件头，不解码）。量不出的当二进制。
4. 宽或者高大过 8000 像素的：出错，图的边太长。
5. 不然交回一张图片：字节、媒体类型（`image/png`、`image/jpeg`、`image/gif`、`image/webp`）、宽高（`tools/interface.md`「工具交回的」），执行器存成 blob、换成图片块。不另写字：驱动在这条结果里写现成的那一句，把图挪到后面一条 `user` 消息里（`drivers/openai-chat.md` 第 7 条）。说法 `read/image`。
6. 第 2 到第 5 款都报 `file.read`：整份文件的哈希，没有 `lines`。`offset`、`limit` 不管。
- 上限取几家接口里最严的：Anthropic 每边 8000 像素、5 MB，DeepSeek 每边 8192 像素、32 MiB。图跟着对话每次都发出去，一张被供应商拒掉的图会让这个会话以后的请求都失败，所以读的时候就拦下。
- 不自己缩图：要解码、再编码，库大；她能用命令缩。

### 出错

出错的结果都标成出错，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `The arguments are not right: {error}.`（`common/bad-args.txt`） | `common/bad-args`，字段 `error` |
| 换不成真实的位置、打开出错、读的时候出错、目录读不了 | `Could not read "{path}": {error}.`（`common/failed.txt`） | `common/failed`，字段 `path`、`error` |
| 没有这个文件或目录 | `There is no file or directory at "{path}".`（`common/missing.txt`），后面一个相近的名字一行 `Did you mean "{path}"?`（`common/similar.txt`） | `common/missing`，字段 `path`；有相近名字的是 `common/missing-similar`，字段 `path`、`similar`（第一个） |
| 不是普通文件，也不是目录 | `"{path}" is not a regular file or a directory.`（`read/not-a-file.txt`） | `read/not-a-file`，字段 `path` |
| 二进制文件 | `"{path}" is a binary file.`（`read/binary.txt`） | `read/binary`，字段 `path` |
| 图的文件太大 | `"{path}" is {size}, too large to view. Images must be at most 5 MiB. Make a smaller copy with a command and read that.`（`read/image-too-big.txt`） | `read/image-too-big`，字段 `path`、`size` |
| 图的边太长 | `"{path}" is {width}×{height} pixels, too large to view. Images must be at most 8000 pixels on each side. Make a smaller copy with a command and read that.`（`read/image-too-wide.txt`） | `read/image-too-wide`，字段 `path`、`width`、`height` |

- 参数不对的 `{error}` 是 JSON 解析的原话，例如 ``missing field `file_path` at line 1 column 12``。
- 读的时候出错的 `{error}`：换不成的那几句见 `fs.md`「出错」，别的是系统的原话。

### 给人看的字

显示名：读取（Read），后面跟 `file_path` 的值；符号 `→`。

| 说法 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `read/lines`（`count`） | 从第 1 行读到最后一行 | `{count} 行` | `{count} lines` |
| `read/lines-part`（`from`、`to`、`total`） | 只读了一段 | `第 {from}–{to} 行，一共 {total} 行` | `lines {from}–{to} of {total}` |
| `read/entries`（`count`） | 目录从第 1 项列到最后一项 | `{count} 项` | `{count} entries` |
| `read/entries-part`（`from`、`to`、`total`） | 目录列了一段 | `第 {from}–{to} 项，一共 {total} 项` | `entries {from}–{to} of {total}` |
| `read/empty` | 空文件、空目录 | 空的 | empty |
| `read/past-end`（`total`、`offset`） | 文件过了结尾 | `过了结尾，一共 {total} 行` | `past the end, {total} lines` |
| `read/past-end-entries`（`total`、`offset`） | 目录过了结尾 | `过了结尾，一共 {total} 项` | `past the end, {total} entries` |
| `read/not-a-file`（`path`） | 不是普通文件，也不是目录 | 不是普通文件，也不是目录 | not a regular file or directory |
| `read/binary`（`path`） | 二进制 | 是二进制文件，没读 | binary file, not read |
| `read/image`（`width`、`height`） | 读了一张图 | `图片 {width}×{height}` | `image {width}×{height}` |
| `read/image-too-big`（`path`、`size`） | 图的文件太大 | `图太大（{size}），没读` | `image too large ({size}), not read` |
| `read/image-too-wide`（`path`、`width`、`height`） | 图的边太长 | `图太大（{width}×{height}），没读` | `image too large ({width}×{height}), not read` |
| `common/missing`（`path`） | 没有 | 没有这个文件 | no such file |
| `common/missing-similar`（`path`、`similar`） | 没有，有相近的 | `没有这个文件，是不是 {similar}` | `no such file, did you mean {similar}` |
| `common/failed`（`path`、`error`） | 读的时候出错 | `读不了：{error}` | `can't read it: {error}` |
| `common/bad-args`（`error`） | 参数不对 | `参数不对：{error}` | `bad arguments: {error}` |

`miyu ask` 怎么印这一行：`cli/ask.md`「每一步那一行」。

### 三个平台

| | Linux、macOS | Windows |
|---|---|---|
| 打开 | 不跟最后一层的链接、不阻塞 | 打开链接、目录联接本身，不跟着走 |
| 不是普通文件的 | FIFO、设备、套接字都认得出 | 没有这几种 |
| 路径 | `~/` | `~/`、`~\`；两种分隔符；写给她看的不带 `\\?\` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/read.rs` | 从资源目录造、参数格式一字不差、报的路径；资源坏了说是哪一份；行号和翻页；`path`、`filePath`；`~`、空的、二进制；列目录、翻页、过了结尾；没有的、参数不对、FIFO；相近的名字；一次最多 2000 行；`file.read` 的范围和整份的哈希，二进制的也报、没有范围，目录、没有的不报；读过的二进制 `write` 盖得了 |
| `crates/miyu-basesystem/src/read/lines/tests.rs` | 行号加制表符、CRLF、过了结尾、结尾的换行；空的、二进制、BOM；UTF-16 两种字节序；截长行；64 KiB；整份的哈希 |
| `crates/miyu-basesystem/src/common/tests.rs` | Windows 的前缀怎么去；相近的名字怎么算 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/read.rs` | 会话里真的调它：读得到、带行号、下一次请求里有；越界的读在没人能确认的会话里被拒 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`read` 输出的写法」）、第五节（`file.read`）、第八节（认 UTF-8 与 UTF-16；路径的写法）、第十节（工具的规范；兼列目录、说从哪接这两处不照 Claude Code）。
- `11-权限与沙盒.md` 第七节：安全地打开。
- `26-提示词.md` 第十节：登记簿里的 `tools/read.json`、`read/*.txt`、`common/*.txt`。

### 还没有的

- 读 PDF：DeepSeek 不收 PDF，另算（`10-自带软件.md` B11）。
- 模型看不了图时，由配置里的看图模型替它看（替看图，`10-自带软件.md` 第三节末尾）：以后。
- 说明里说读过的文件压缩以后会读回来：压缩随 M6（`09-压缩.md` 第四节「压后重建」），现在还没有。
- 读技能正文时登记「用过哪个技能」（`10-自带软件.md` 第三节说明、`09-压缩.md` 第四节）。
