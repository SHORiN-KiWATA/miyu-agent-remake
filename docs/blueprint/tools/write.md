## `write`

### 是什么

新建一个文件，或者整体覆盖一个文件。已经在了的，要她这个会话里看过、而且现在的内容和她看到的一样才写；照原来的编码、BOM、换行写。先写临时文件再改名盖上去。报 `file.changed`：改前改后的内容本身。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/write.rs` | 参数、要碰的路径、新建还是覆盖、结果和效果 |
| `crates/miyu-basesystem/src/text.rs` | 一份文本文件的写法：认编码、BOM、换行，照同样的写法写回；几行 |
| `crates/miyu-basesystem/src/common.rs` | 改之前核对她看过的；目录、不是普通文件、写不了这几句 |
| `crates/miyu-fs/src/replace.rs` | 整体换成新的内容（`fs.md` 第五节） |
| `resources/software/basesystem/tools/write.json` | 说明和参数格式 |
| `resources/software/basesystem/write/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `write`。`resources/software/basesystem/tools/write.json` 原文：

```json
{
  "description": "Create a file, or replace all of its content. A file that already exists must be read first. To change part of a file, use `edit`.",
  "parameters": {"type":"object","properties":{"file_path":{"type":"string","description":"Absolute, or relative to the working directory."},"content":{"type":"string"}},"required":["file_path","content"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `file_path` | 是 | 绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。写成 `path`、`filePath` 的也认，参数格式里不写；写了两种的，参数不对 |
| `content` | 是 | 要写的全部内容 |

- 别的参数不认，也不报错。
- 报给权限策略的路径：一条，`file_path` 原样，写。只读时内核当场拦下，走不到这里（`tools/interface.md` 第二节）。

### 怎么走

1. 读参数，读不懂的（缺一样、类型不对）：参数不对。
2. 换成真实的位置（`fs.md` 第二节）。换不成：写不了，原因是换不成的那一句。还不存在的，照最近的已经存在的上级换，再接上后面几段。
3. 照「安全地打开」开它（`fs.md` 第四节，不跟最后一层的链接、不阻塞，施工 4-9 再补二），看它现在是什么：
   - 没有：新建。
   - 是目录：`"{path}" is a directory.`，出错。
   - 不是普通文件（FIFO、设备、链接这类）：`"{path}" is not a regular file.`，出错。FIFO 没人写也不卡住。
   - 是普通文件：整份读进来，读不了：写不了。
   - 打不开（没有的以外的错）：写不了。
4. 已经在了的，改之前核对她看过的（`tools/interface.md` 第四节）：照真实的位置找。
   - 她没看过：`"{path}" already exists and has not been read. Read it first.`，出错，不写。
   - 看过，可现在整份内容的哈希和她看到的不一样（被人或者别的程序改过了）：`"{path}" has changed since it was last read. Read it again first.`，出错，不写。
   - 对得上：往下写。她自己刚写过的，看到的就是改后的，接着写不用重读。
   - `read` 读二进制文件不报效果，所以已经在了的二进制文件，除非这个会话里她自己写过，都说没读过。
5. 照原来的写法把 `content` 写成字节（`text.rs`）：
   - 编码：开头 `FF FE` 的写成小端 UTF-16，`FE FF` 的写成大端 UTF-16，都带着那个 BOM；别的写成 UTF-8，原来开头有 UTF-8 的 BOM（`EF BB BF`）的照带。认不出的别的编码当 UTF-8。
   - 换行：照原来第一处换行认，它是 `\r\n` 的，`content` 里的单个 `\n` 都换成 `\r\n`（原来就是 `\r\n` 的不重复换）；不是的，照给的写。
6. 新建的：上级目录没有的建上，建不了：写不了。写成 UTF-8、不带 BOM，换行照给的写。
7. 整体换成新的内容（`fs.md` 第五节）：先写同一个目录里的临时文件、同步，再改名盖上去；原来的权限照留；原来是只读的不写。出错：写不了，原来的文件没动（新建的，第 6 条建上的上级目录留着）。
8. 成了：新建的说 `Created "{path}".`，覆盖的说 `Updated "{path}".`。这里的路径照结果的规矩写：在这一轮的工作目录里的写相对的，在外面的写绝对的，Windows 上不写 `\\?\`；照模板的规矩转义（`tools/read.md` 第 8 条）。
9. 报 `file.changed`：`path` 是真实的位置；`before` 是原来整份的字节，新建的没有；`after` 是写进去的字节。执行器把两份存成 blob，会话记下她看到的是改后的。
10. 叫停：写在阻塞线程里做。核对完、建上级目录之前看一眼叫停的旗，举了就不写，交回 `stopped`，什么都没改，上级目录也不建；已经盖上去的照常交回。没有超时。

### 样子

```text
Created "src/new/a.rs".
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 新建了 | `write/created.txt` | `Created "{path}".` |
| 覆盖了 | `write/updated.txt` | `Updated "{path}".` |

覆盖时照原来的写法（`content` 都是 `x\ny\n`）：

| 原来 | 写进去 |
|---|---|
| `a\r\nb\r\n` | `x\r\ny\r\n` |
| BOM + `a\n` | BOM + `x\ny\n` |
| UTF-16LE 的 `a\n` | UTF-16LE 的 `x\ny\n`，带 BOM |
| `a\nb\n` | `x\ny\n` |

### 出错

出错的结果都标成出错，不写，不报效果。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `The arguments are not right: {error}.` | `common/bad-args`，字段 `error` |
| 没读过 | `"{path}" already exists and has not been read. Read it first.`（`common/not-read.txt`） | `common/not-read` |
| 读过以后又改过了 | `"{path}" has changed since it was last read. Read it again first.`（`common/stale.txt`） | `common/stale` |
| 是目录 | `"{path}" is a directory.`（`common/directory.txt`） | `common/directory` |
| 不是普通文件 | `"{path}" is not a regular file.`（`common/not-a-regular-file.txt`） | `common/not-a-regular-file` |
| 换不成真实的位置、看不了、读不了、建不了上级目录、写不进、只读的 | `Could not write "{path}": {error}.`（`common/write-failed.txt`） | `common/write-failed`，字段 `error` |

- 出错句子里的 `{path}` 是她给的原样。
- `{error}` 是系统的原话；只读的是 `permission denied`；换不成的见 `fs.md`「出错」。

### 给人看的字

显示名：写入（Write），后面跟 `file_path` 的值。

| 说法 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `write/created`（`count`） | 新建了 | `新建，{count} 行` | `new, {count} lines` |
| `write/updated`（`count`） | 覆盖了 | `覆盖，{count} 行` | `replaced, {count} lines` |
| `common/not-read` | 没读过 | 没读过，要先读 | not read yet, read it first |
| `common/stale` | 读过以后又改过了 | 读过以后又改过了，要重读 | changed since read, read it again |
| `common/directory` | 是目录 | 是个目录 | is a directory |
| `common/not-a-regular-file` | 不是普通文件 | 不是普通文件 | not a regular file |
| `common/write-failed`（`error`） | 写不了 | `写不了：{error}` | `can't write: {error}` |
| `common/bad-args`（`error`） | 参数不对 | `参数不对：{error}` | `bad arguments: {error}` |

`count` 是她给的 `content` 有几行：空的是 0 行；以换行结尾的，最后那一段空的不算；`\r\n` 算一个换行。

### 三个平台

- 只读：Unix 上一个写位都没有、Windows 上带只读属性的，不写（`fs.md` 第五节）。
- 不是普通文件：Unix 上 `/dev/null` 这类，不写；Windows 上没有这几种。
- 编码、BOM、换行只看文件原来的字节，三个平台一样：Windows 上新建的文件也照给的换行写。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/write.rs` | 报要写的路径；新建带建上级目录、工作区里的绝对路径照样写相对的；没读过的、读过以后被改了的不写，看的是现在的样子就写，自己刚写过的接着写；覆盖时照原来的 CRLF、BOM、UTF-16；目录、只读的不写；原来的权限照留、不留临时文件；`/dev/null` 不写；读过的二进制盖得了；FIFO 说不是普通文件、不卡住 |
| `crates/miyu-basesystem/src/text/tests.rs` | 从原来的字节认写法、照原来的写法写回、UTF-16 写得回去解得回来、几行怎么数 |
| `crates/miyu-basesystem/tests/stop.rs` | 旗举了：新建的不建、上级目录也不建，已经在了的不盖；没看过的照旧先说没看过 |
| `crates/miyu-fs/src/replace/tests.rs` | 整体换、只读的不写、盖不上去时临时文件删掉 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/write.rs` | 会话里先读后写，日志里两次结果的效果对得上、blob 里有改前改后；新建的不用先读；没读过的被拒；重新载入以后她读过的照样算 |
| `crates/miyu-session/tests/restore.rs` | 撤销时照 `write` 报的 `file.changed` 写回改前的，恢复时再写回改后的 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`write` 的细则」）、第五节（`file.changed`、「她看过的」）、第十节（参数名）。
- `11-权限与沙盒.md` 第二节：只读时拦下写入。
- `26-提示词.md` 第十节：登记簿里的 `tools/write.json`、`write/*.txt`、`common/*.txt`。
