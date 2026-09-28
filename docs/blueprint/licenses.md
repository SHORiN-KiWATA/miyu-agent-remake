## 许可证

### 是什么

仓库用 GPL-3.0-or-later。第三方依赖的许可证都要能和它合在一起发，门禁的「许可证」一项查。

### 在哪

| 文件 | 管什么 |
|---|---|
| `LICENSE` | GPL-3.0 全文，照自由软件基金会发的原样：35149 字节，sha256 以 `3972dc97` 开头 |
| `Cargo.toml` | 工作区的 `license = "GPL-3.0-or-later"`，每个 crate 照它（`license.workspace = true`） |
| `README.md` 的「许可证」一节 | 对外怎么说 |
| `xtask/src/licenses.rs` | 门禁的「许可证」一项 |

### 对外的样子

README 那一节（例子）：

```text
## 许可证

GPL-3.0-or-later，见 `LICENSE`。

扩展、脚本、MCP 服务器是另外的进程，走协议和 Miyu 说话，用什么许可证都行。以后给扩展用的开发包用 MIT。
```

### 怎么走：门禁的「许可证」

1. 对发布的四个平台各跑一次 `cargo metadata --format-version 1 --locked --filter-platform <平台>`：`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`（`12-进程形态与分发.md` R11）。取依赖图里用得到的包，去掉工作区自己的。
2. 每个包读它的 `license`，照 SPDX 表达式算：
   - `OR` 连着的，有一个能用就行；老写法里的 `/` 也当 `OR`。
   - `AND` 连着的，每个都要能用。
   - 括号照括号算。
   - `WITH` 后面是例外条款：认得的只有 `LLVM-exception`，不影响前面那个；别的例外当不能用。
3. 能用的：`MIT`、`Apache-2.0`、`BSD-2-Clause`、`BSD-3-Clause`、`ISC`、`Zlib`、`0BSD`、`Unicode-3.0`、`Unicode-DFS-2016`、`Unlicense`、`CC0-1.0`、`BSL-1.0`、`MPL-2.0`、`CDLA-Permissive-2.0`。都能和 GPL-3.0 合在一起发。
4. 没写 `license`、只给了许可证文件的，当不能用：报出来，人看过再定。
5. 同一个包在几个平台上都有，只报一次，写上是哪几个平台。

### 出错

| 什么时候 | 报的话 |
|---|---|
| 一个包的许可证不能用 | `<包名> <版本>（<平台>、<平台>）：<license 原文> 和 GPL-3.0-or-later 合不到一起` |
| 没写 `license` | `<包名> <版本>（<平台>）：没写 license，要人看过` |
| `cargo metadata` 跑不起来 | `跑不了 cargo metadata（<平台>）：<原话>` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `xtask/src/licenses/tests.rs` | 表达式：`OR`、`AND`、`WITH`、括号、`/`；能用的、不能用的、没写的；同一个包几个平台只报一次；报的那一句 |

### 出处

- `12-进程形态与分发.md` R15：为什么是 GPL-3.0-or-later。
