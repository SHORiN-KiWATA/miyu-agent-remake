## `edit`

### 是什么

在一个已经在了的文本文件里做精确替换，一次可以改几处。每一处都对照原文件找，要唯一、不重叠；有一处出错，一处都不改。改之前和 `write` 一样核对她看过的。对上的那一段换成 `new_string`（换行照文件的），别的地方一个字节不动，编码、BOM 照原来的。报 `file.changed`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/edit.rs` | 参数的几种写法、要碰的路径、读原文、核对、查重叠、换、结果和效果 |
| `crates/miyu-basesystem/src/edit/find.rs` | 找位置：精确的、宽松的、`replace_all`、不唯一、最接近的几行 |
| `crates/miyu-basesystem/src/text.rs` | 认编码、BOM、换行；严格地解成字；照原来的编码、BOM 写回 |
| `crates/miyu-basesystem/src/common.rs` | 改之前核对她看过的；几件共用的几句 |
| `crates/miyu-fs/src/replace.rs` | 整体换成新的内容（`fs.md` 第五节） |
| `resources/software/basesystem/tools/edit.json` | 说明和参数格式 |
| `resources/software/basesystem/edit/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `write`。说明和参数的原文如下。

样本 `resources/software/basesystem/tools/edit.json`：

```json
{
  "description": "Make exact text replacements in a file, one or more at a time. The file must have been read first. Each old_string must match the file exactly, with its indentation and without the line number prefix from read, and match only one place unless replace_all is set.",
  "parameters": {"type":"object","properties":{"file_path":{"type":"string","description":"Absolute, or relative to the working directory."},"edits":{"type":"array","description":"Each edit is matched against the file as it was before this call.","items":{"type":"object","properties":{"old_string":{"type":"string"},"new_string":{"type":"string"},"replace_all":{"type":"boolean","description":"Replace every match. Default false."}},"required":["old_string","new_string"]}}},"required":["file_path","edits"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `file_path` | 是 | 绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。写成 `path`、`filePath` 的也认；写了两种的，参数不对 |
| `edits` | 是 | 要改的几处，照先后是第 1、2……处。写成一个对象（不是列表）的，当只有一处 |
| `edits[].old_string` | 是 | 文件里的原文 |
| `edits[].new_string` | 是 | 换成什么 |
| `edits[].replace_all` | 否 | 真的就是对得上的每个地方都换，用不着唯一；默认 `false` |

- 参数格式里不写、写了也认的：
  - 没有 `edits`，顶层直接写 `old_string`、`new_string`（和 `replace_all`）的，当只有一处；只写了其中一个的，当没给要改的。`edits` 和顶层的都写了，照 `edits`。
  - `oldString`、`oldText` 当 `old_string`；`newString`、`newText` 当 `new_string`；`replaceAll` 当 `replace_all`。顶层、每一处里都认。
- `edits` 的列表写成了一段字符串的，内核先还原成列表（`tools/interface.md` 第二节）；只还原 `[` 开头的，一个对象写成字符串的不还原，参数不对。内核只修正顶层声明了的参数：`replace_all`（顶层的、每一处里的）写成字符串的不修正，参数不对。
- 别的参数不认，也不报错。
- 报给权限策略的路径：一条，`file_path` 原样，写。只读时内核当场拦下。

### 怎么走

1. 读参数，读不懂的（例如某一处缺 `new_string`）：参数不对。
2. 一处要改的都没有（`edits` 是空的，或者什么都没给）：说一句怎么写 `edits`，出错。这一条在看文件之前。
3. 换成真实的位置（`fs.md` 第二节），换不成：读的时候出错了。
4. 照「安全地打开」开它（`fs.md` 第四节，不跟最后一层的链接、不阻塞，施工 4-9 再补二），看它现在是什么：没有：没有这个文件，带上相近的名字（`tools/read.md` 第 9 条）；打不开：读的时候出错了；是目录、不是普通文件：各说一句，FIFO 没人写也不卡住；是普通文件：整份读进来，读不了：读的时候出错了。
5. 核对她看过的，和 `write` 一样（`tools/write.md` 第 4 条）：没看过的、看过以后又被改了的，不改。
6. 认写法，和 `write` 一样（`tools/write.md` 第 5 条），再严格地解成字：UTF-8 的要合写法；带 BOM 的 UTF-16 要是整数个两字节、没有落单的代理项；BOM 不算在字里。解不开的：不是文本，不改，要整份写的用 `write`。
7. 照先后一处一处找，出错的那一处就停，后面的不找：
   1. `old_string` 是空的：出错，新建文件要用 `write`。
   2. `old_string` 和 `new_string` 一样：出错。
   3. 在原文里找（下面「找位置」）。对上了：记下位置，`new_string` 里的换行照文件的：文件是 CRLF 的（照第一处换行认），`new_string` 里单个的 `\n` 换成 `\r\n`。对上好几个（没写 `replace_all`）：不唯一，出错。一个都对不上：没找到，出错。
8. 查重叠：对上的每一段照在原文里的先后排，相邻两段有重叠的（前一段的结尾过了后一段的开头），出错，说是哪两处（小的在前）。挨着的不算重叠。同一处的 `replace_all` 找出来的本来就不重叠。
9. 从后往前换，先换的不影响后换的位置。对上的那几段以外，一个字节不动：原文里混着的换行照旧。
10. 照原来的编码、BOM 写成字节，换行不再整份换。
11. 整体换成新的内容（`fs.md` 第五节），出错：写不了。
12. 成了：`Edited "{path}".`，路径照结果的规矩写（在这一轮的工作目录里的写相对的，在外面的写绝对的），照模板的规矩转义。
13. 报 `file.changed`：`path` 是真实的位置，`before` 是原来整份的字节，`after` 是写进去的。执行器存成 blob，会话记下她看到的是改后的：接着改不用重读。
14. 叫停：改在阻塞线程里做。改名盖上去之前看一眼叫停的旗，举了就不改，交回 `stopped`，什么都没改；已经盖上去的照常交回。没有超时。

**找位置**（`find.rs`）：

1. 两层看法，先精确、再宽松；原文和 `old_string` 照同一层的办法换过再比：
   - 精确：只把 `\r\n` 当 `\n` 看，别的照原样。她读到的每一行已经去掉了行尾的 `\r`，照读到的写，CRLF 的文件也对得上。
   - 宽松：另外，每一行末尾的空格、制表符不算（文末的也不算）；每个字先做 NFKC，再换成普通的：

     | 这些 | 当成 |
     |---|---|
     | `‘` `’` `‚` `‛` `′`（U+2018、U+2019、U+201A、U+201B、U+2032） | `'` |
     | `“` `”` `„` `‟` `″`（U+201C、U+201D、U+201E、U+201F、U+2033） | `"` |
     | U+2010 到 U+2015、U+2212 | `-` |
     | U+00A0、U+2000 到 U+200A、U+202F、U+205F、U+3000 | 空格 |

2. 哪一层对得上几个就是几个：精确的对上 1 个，就是它；对上 2 个以上（没写 `replace_all`），报不唯一，不往宽松那一层找；一个都没有，才找宽松的。`old_string` 在宽松那一层全是空白、换行的，不找宽松那一层。
3. 数有几个时，重叠的也算：`aaa` 里的 `aa` 是 2 个。写了 `replace_all` 的，从前往后不重叠地找：`aaa` 里只换第一个 `aa`。
4. 对上的那一段换回原文里的一段：从第一个字的来处起，到最后一个字的来处止。所以宽松那一层不算的行尾空白，在这一段两头以外的不在里面（`value = 1  ` 里对上 `value = 1`，后面两个空格不换）；夹在中间的在里面，一起换掉（跨行对上时，前一行末尾的空白）。对上 `\n` 的，原文的 `\r\n` 两个字节一起在这一段里。
5. 不唯一的，报一共几个，和前 10 个在原文的第几行（从 1 数起）。
6. 一个都对不上，找最接近的几行：
   1. 拿 `old_string` 第一行不空的那一行，照宽松的看法换过、去掉两头的空白。`old_string` 全是空白的，没得比。
   2. 跟原文的每一行（同样换过、去掉两头的空白）比像不像：相邻两个字一组，相同的组占两边组数的比例（Dice 系数）；一模一样的是 1；有一边不到两个字的是 0。
   3. 最像的那一行（一样像的取前面的）不到一半（0.5）的，没有最接近的。
   4. 到了一半（不小于 0.5）：从那一行往回退 `old_string` 开头的空行数，带上和 `old_string` 一样多的行（至少 1 行、最多 10 行，到文末为止），照 `read` 的样子写：`<行号>\t<原文>`，一行最长 2000 个字。

### 样子

三处一起改，每一处都对照原文件（原文 `one\ntwo\nthree\n`，改成 `one\ntwo\n3\n`）：

```json
{"file_path": "a.txt", "edits": [
  {"old_string": "three", "new_string": "3"},
  {"old_string": "one", "new_string": "one\ntwo"},
  {"old_string": "two\n", "new_string": ""}
]}
```

```text
Edited "a.txt".
```

没对上、有最接近的（行号后面是一个制表符）：

```text
Edit 1: old_string was not found in "main.rs".
The closest text is at lines 2-2:
2	    let total = count(items);
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 改好了 | `edit/edited.txt` | `Edited "{path}".` |
| 没给要改的 | `edit/no-edits.txt` | `No edits were given. Set edits, each with old_string and new_string.` |
| `old_string` 是空的 | `edit/empty.txt` | `Edit {index}: old_string is empty. To create a file, use write.` |
| 改前改后一样 | `edit/same.txt` | `Edit {index}: old_string and new_string are the same.` |
| 没对上 | `edit/not-found.txt` | `Edit {index}: old_string was not found in "{path}".` |
| 没对上、有最接近的 | `edit/closest.txt`，后面接那几行 | `The closest text is at lines {from}-{to}:` |
| 不唯一 | `edit/not-unique.txt` | `Edit {index}: old_string matches {count} places in "{path}", at lines {lines}. Add surrounding lines to pick one, or set replace_all.` |
| 重叠 | `edit/overlap.txt` | `Edits {first} and {second} overlap in "{path}". Merge them into one edit.` |
| 不是文本 | `edit/not-text.txt` | `"{path}" is not UTF-8 or UTF-16 text. To replace it whole, use write.` |

- `{index}`、`{first}`、`{second}` 是第几处，从 1 数起；`{lines}` 是行号，用 `, ` 连起来，例如 `1, 3`。
- 出错句子里的 `{path}` 是她给的原样，改好了的那一句是照结果的规矩写的。

### 出错

出错的结果都标成出错，一处都不改，不报效果。照这个先后查，先对上的算；「某一处」那几种是一处一处照先后查的，每一处先看是不是空的、再看一样不一样、再找：

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 没给要改的 | `edit/no-edits.txt` | `edit/no-edits` |
| 换不成真实的位置、看不了、读不了 | `common/failed.txt`：`Could not read "{path}": {error}.` | `common/failed`，字段 `path`、`error` |
| 没有 | `common/missing.txt`，加上相近的名字 | `common/missing` 或 `common/missing-similar` |
| 是目录 | `common/directory.txt` | `common/directory` |
| 不是普通文件 | `common/not-a-regular-file.txt` | `common/not-a-regular-file` |
| 没读过 | `common/not-read.txt` | `common/not-read` |
| 读过以后又改过了 | `common/stale.txt` | `common/stale` |
| 不是 UTF-8、也不是带 BOM 的 UTF-16 | `edit/not-text.txt` | `edit/not-text` |
| 某一处的 `old_string` 是空的 | `edit/empty.txt` | `edit/empty`，字段 `index` |
| 某一处改前改后一样 | `edit/same.txt` | `edit/same`，字段 `index` |
| 某一处没对上 | `edit/not-found.txt`，有最接近的接 `edit/closest.txt` 和那几行 | `edit/not-found`，字段 `index`；有最接近的是 `edit/not-found-near`，字段 `index`、`line`（最接近的第一行） |
| 某一处不唯一 | `edit/not-unique.txt` | `edit/not-unique`，字段 `index`、`count` |
| 两处重叠 | `edit/overlap.txt` | `edit/overlap`，字段 `first`、`second` |
| 写不进（只读的、磁盘满了之类） | `common/write-failed.txt`：`Could not write "{path}": {error}.` | `common/write-failed`，字段 `error` |

### 给人看的字

显示名：编辑（Edit），后面跟 `file_path` 的值；符号 `←`；标题下面印改动（`block` 是 `edits`，`cli/ask.md`「编辑那一块」）。

| 说法 | 中文 | 英文 |
|---|---|---|
| `edit/edited`（`count`：换了几段，`replace_all` 的每一个都算） | `改了 {count} 处` | `{count} replaced` |
| `edit/no-edits` | 没给要改的 | no edits given |
| `edit/empty`（`index`） | `第 {index} 处的原文是空的` | `edit {index} has an empty old_string` |
| `edit/same`（`index`） | `第 {index} 处改前改后一样` | `edit {index} changes nothing` |
| `edit/not-found`（`index`） | `第 {index} 处没找到` | `edit {index} not found` |
| `edit/not-found-near`（`index`、`line`） | `第 {index} 处没找到，最像的在第 {line} 行` | `edit {index} not found, closest at line {line}` |
| `edit/not-unique`（`index`、`count`） | `第 {index} 处对得上 {count} 个地方` | `edit {index} matches {count} places` |
| `edit/overlap`（`first`、`second`） | `第 {first} 处和第 {second} 处重叠了` | `edits {first} and {second} overlap` |
| `edit/not-text` | 不是文本文件 | not a text file |
| `common/*` | 见 `tools/write.md`、`tools/read.md` | |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/edit.rs` | 报要写的路径；几处一起改、对照原文件、前面变长不影响后面；顶层一处的写法、`edits` 写成对象、`path` 和驼峰的字段名、没给要改的；重叠的、有一处没对上的一处都不改；CRLF、BOM、UTF-16 照原来的，新加的行照 CRLF；宽松对上只换那一段；最接近的几行、不唯一的行号、`replace_all`；没读过、改过了、没有、目录、不是文本、空的、一样的 |
| `crates/miyu-basesystem/tests/write.rs` | FIFO 说不是普通文件、不卡住（和 `write` 一起测） |
| `crates/miyu-basesystem/src/edit/find/tests.rs` | 精确的位置；LF 对 CRLF 连着 `\r`；宽松的几种；宽松多出来的不算进去；精确的不唯一不往宽松找、重叠的也算不唯一；`replace_all` 不重叠；最多 10 个行号；最接近的几行、开头空行、没有像的、全是空白；Dice 系数 |
| `crates/miyu-basesystem/src/text/tests.rs` | 严格地解：解不开的、落单的字节、落单的代理项 |
| `crates/miyu-basesystem/tests/stop.rs` | 旗举了不改，文件照旧 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/write.rs` | 会话里读一次、连着改两次，不用重读 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第六节（B6、「`edit` 的细则」）、第五节（`file.changed`、「她看过的」）、第十节（「`edit` 一次改多处」不照 Claude Code）。
- `26-提示词.md` 第十节：登记簿里的 `tools/edit.json`、`edit/*.txt`、`common/*.txt`。
