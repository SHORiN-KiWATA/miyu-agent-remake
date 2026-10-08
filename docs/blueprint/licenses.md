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

### 依赖记录

- 施工 R-5 上给 `miyu-embed`（新 crate，本机 embedding 的小程序，`recall.md` 第四条）加了 `ort` 2.0.0-rc.13（MIT OR Apache-2.0，钉死版本，关掉默认功能只开 `std`、`download-binaries`、`tls-rustls`），它带进来的 `ort-sys`（MIT OR Apache-2.0）、`ndarray` 0.17、`matrixmultiply`、`num-complex`、`num-integer`、`rawpointer`（MIT 或 Apache-2.0）；`ort-sys` 编译时下预编好的 ONNX Runtime 用的 `ureq`、`ureq-proto`、`utf8-zero`（MIT OR Apache-2.0）、`lzma-rust2`（Apache-2.0）、`hmac-sha256`（ISC）、`socks`（MIT/Apache-2.0）、`byteorder`（Unlicense OR MIT）、`winapi` 和它的两个 `*-pc-windows-gnu`（MIT/Apache-2.0），`rustls` 那一串原来就有。分词加了 `unicode-general-category` 1.1（Apache-2.0，没有别的依赖）；`toml_edit` 多开 `serde`，带进来 `serde_spanned`（MIT OR Apache-2.0）。都在能用的名单里，门禁过了。
  - ONNX Runtime 本身（MIT，微软）由 `ort-sys` 编译时从 pyke 的地址下预编好的静态库，静态链接进 `miyu-embed`，主程序不带：编这个 crate 要联网，没网的机器设 `ORT_LIB_PATH` 指到自己放好的库（`ort-sys` 的 `build/vars.rs`）。ONNX Runtime 静态库里还有它自己的第三方代码（protobuf、abseil、Eigen 等，BSD、Apache-2.0、MPL-2.0 这几种，都能和 GPL-3.0 合在一起发），发行包带上 `miyu-embed` 时随它发 ONNX Runtime 的 `ThirdPartyNotices.txt`（随发布那一步）。
- 施工 W-10 给 `miyu-web` 直接加了 `getrandom` 0.4（票据的 32 个随机字节）、`base64` 0.22（核心一块块给的内容是 base64）：两个本来就在依赖图里，和别的 crate 用的同一份，没给图里添新的第三方包。门禁过了。
- 施工 W-9 给 `miyu-web`（新 crate，网页软件，`web-ui.md`）直接用了 `hyper` 1（MIT，开 `server`、`http1`：只听本机的 HTTP 端口）、`hyper-util`（开 `tokio`）、`http-body-util`、`futures-util`、`tokio`、`serde`、`serde_json`、`tracing`（都原来就在依赖图里）；新加 `tokio-tungstenite` 0.29（MIT，关掉默认功能只开 `handshake`：WebSocket 一帧一帧收发），它带进来的 `tungstenite`（MIT OR Apache-2.0）、`sha1` 0.10 和它的 `digest`、`block-buffer`、`crypto-common`、`generic-array`、`cpufeatures`（MIT 或 Apache-2.0）、`data-encoding`（MIT）、`httpdate`（MIT OR Apache-2.0）、`rand` 0.9、`rand_chacha`、`rand_core`、`ppv-lite86`、`zerocopy`（MIT 或 Apache-2.0，`zerocopy` 另可选 BSD-2-Clause）、`getrandom` 0.3（MIT OR Apache-2.0）。`getrandom` 0.3 在 wasm、UEFI 上才要的 `wasip2`、`wit-bindgen`、`r-efi` 不在发布的四个平台的依赖图里。都在能用的名单里，门禁过了。
- 施工 W-8 给 `miyu-store` 加了 `argon2` 0.6（MIT OR Apache-2.0，网页登录的密码哈希，`web-module.md`「怎么走」第一条），关掉默认功能、只开 `alloc`、`password-hash`，盐由原来就有的 `getrandom` 给；它带进来的 `password-hash`、`phc`、`base64ct`、`blake2`、`ctutils`、`cmov`（都是 MIT OR Apache-2.0）。都在能用的名单里。开发、测试编的也给 `argon2`、`blake2` 开优化（工作区 `Cargo.toml` 的 `[profile.dev.package.*]`），不然算一次要零点几秒。
- 施工 W-7 给 `miyu-net`（新 crate，可选软件包 `net`，默认打开）直接用了 `hyper-util` 0.1（MIT，开 `client-proxy`：判这一跳走不走代理，reqwest 自己照环境变量走代理用的就是它的 `Matcher`）、`http` 1（MIT OR Apache-2.0，`Matcher` 收的地址类型）；`reqwest`、`tokio`、`serde`、`serde_json`、`tracing` 照 `miyu-http` 的写法。这几个本来就在依赖图里（reqwest 带进来的），没给图里添新的第三方包。门禁过了。
- 施工 8-5 补（按 `Ctrl+C` 也把回显开回来）删了 `miyu-cli` 的 `rpassword`、它带进来的 `rtoolbox`：改成自己管终端的设置，图里少了这两个包。`miyu-cli` 多开了几个已经在图里的包的功能，没给图里添新的第三方包：`rustix`（本来就在 `miyu-basesystem`、`miyu-fs`、`miyu-ipc` 用着）开 `termios`（读写终端设置）、`pty`（只在 `dev-dependencies`，测试开真的伪终端）两个功能；`windows-sys`（本来就在别的 crate 用着）给 `miyu-cli` 直接加一条依赖、开 `Win32_System_Console`（控制台模式的读写）；`tracing`（本来就在依赖图里）给 `miyu-cli` 直接加一条依赖（终端/控制台设置写不回去时记一行）。都在能用的名单里，门禁过了。
- 施工 W-4 给 `miyu-mermaid`（新 crate，可选软件包 `mermaid`，默认打开）加了 `mermaid-rs-renderer` 0.3.1（MIT，mermaid 源码画成 SVG，关掉它默认的 `cli`、`png` 两个功能，只要出 SVG 的那一半，和它一起进来的 `clap`、`resvg`、`usvg` 都不进依赖图）；它带进来的 `anyhow`（MIT OR Apache-2.0）、`fontdb` 0.23（MIT，量字的宽要用；`miyu-mermaid` 自己也直接用它独立探一次字体库找不找得到字）、`json5`（MIT）、`once_cell`（MIT OR Apache-2.0）、`regex`（MIT OR Apache-2.0）、`serde`、`serde_json`（都在依赖图里）、`thiserror`（MIT OR Apache-2.0）、`ttf-parser`（MIT OR Apache-2.0）。`fontdb` 在 Linux 上默认功能还带来 `fontconfig-parser`（MIT，依赖 `roxmltree`，MIT OR Apache-2.0）、`memmap2`（MIT OR Apache-2.0）、`slotmap`（Zlib，依赖 `version_check`，MIT OR Apache-2.0）、`tinyvec`（Zlib OR Apache-2.0 OR MIT）、`log`（MIT OR Apache-2.0）；macOS、Windows 上不编进 `fontconfig-parser`（它只在 `cfg(all(unix, not(macos), not(android)))` 下才是依赖）。都在能用的名单里，门禁过了。`sha2`、`tracing` 原来就有。
- 施工 W-2 给 `miyu-fs` 加了 `ignore` 0.4（Unlicense OR MIT，`fs.find` 建清单走目录、认 `.gitignore`）：这个包本来就在依赖图里（`miyu-basesystem` 的 `glob`、`grep`），这次只是多一个 crate 直接用它，没给图里添新的第三方包。门禁过了。
- 施工 8-5 加了 `rpassword` 7.5（Apache-2.0，`miyu login` 贴 key 时关掉回显读一行），它带进来的 `rtoolbox`（Apache-2.0）；`libc`、`windows-sys` 原来就有。都在能用的名单里，门禁过了。
- 施工 8-4 加了 `notify` 8.2（CC0-1.0，监视配置文件，只开默认的 macOS FSEvents），它带进来的 `notify-types`（MIT OR Apache-2.0）、`inotify`、`inotify-sys`（ISC，Linux）、`fsevent-sys`（MIT，macOS）、`walkdir`、`same-file`（Unlicense OR MIT）、`mio`（MIT）、`bitflags`、`libc`、`log`、`windows-sys`（MIT 或 Apache-2.0，多数原来就有）。都在能用的名单里，门禁过了。
- 施工 8-2 加了 `toml_edit`（MIT OR Apache-2.0，读配置的 TOML，纯逻辑层的白名单里）和它带进来的 `toml_parser`、`toml_datetime`、`winnow`（MIT 或 Apache-2.0；`indexmap` 这些原来就有）；`sys-locale`（MIT OR Apache-2.0，系统设置里的语言）。都在能用的名单里，门禁过了。
- 施工 3-8 七补加了 `rusqlite`（MIT，会话列表的索引，`store/index.md`），开 `bundled`：`libsqlite3-sys`（MIT）自己带 SQLite 的源码编，SQLite 本身是公有领域；它带进来的 `hashlink`、`hashbrown`、`fallible-iterator`、`fallible-streaming-iterator`（MIT 或 Apache-2.0）、`foldhash`（Zlib），编的时候用的 `cc`、`pkg-config`、`vcpkg`（MIT 或 Apache-2.0）。都在能用的名单里，门禁过了。只在 wasm 上用的 `sqlite-wasm-rs`、`rsqlite-vfs` 不在发布的四个平台的依赖图里。

### 资源里的第三方数据

门禁只查 Rust 依赖。资源目录里随安装包发的第三方数据另记在这里，加一份之前先看许可证：

| 文件 | 从哪来 | 许可证 | 怎么守 |
|---|---|---|---|
| `crates/miyu-embed/tests/fixtures/bge-vocab.txt` | bge-small-zh-v1.5 的词表（施工 R-5 上，分词的测试用） | MIT（FlagEmbedding 仓库的 `LICENSE`：模型卡写明 bge 模型照它发布，可免费商用） | 原文放在旁边的 `bge-vocab.LICENSE`。只在测试里，不随安装包发；模型文件本身不进仓库，放在 Release `models-bge-small-zh-v1.5`，同一份许可证跟着 |
| `resources/models/models-dev.json` | models.dev 的 `api.json`（模型目录，施工 8-7） | MIT（models.dev 仓库的 `LICENSE`，2025 models.dev） | 原文放在旁边的 `models-dev.LICENSE`，跟着快照一起发。MIT 能和 GPL-3.0 合在一起发 |

- 施工 8-7 定的（2026-10-01，施工员确认，主会话同意照图纸带快照）：快照约 5.3 MB，进仓库压缩以后约 0.5 MB，仓库的包原来约 13.5 MB，涨不到 4%。刷新一次快照是一次替换，照「怎么刷新」（`store/resources.md`）做。

### 出处

- `12-进程形态与分发.md` R15：为什么是 GPL-3.0-or-later。
