## 施工单 8-5（补）：按 Ctrl+C 也把回显开回来

状态：待施工（2026-10-02，8-5 施工时发现；怎么修主会话定）。

### 目的

`miyu login`、`miyu setup` 贴 key 时关掉了回显（`crates/miyu-cli/src/config/console.rs` 用 `rpassword` 读终端本身）。这时按 Ctrl+C，进程被信号直接打断，`rpassword` 来不及把回显开回来，终端就一直不回显，人得自己敲 `reset`。改成：读 key 的那段时间里按 Ctrl+C，照「人不要了」办，回显照原样开回来，再退出。

修法（主会话定）：
- 读 key 的那一段，自己管终端的设置，不交给信号打断：
  - Unix：读之前存下终端原来的设置（`termios`），关回显、关信号键（`ISIG`），自己一个字节一个字节读；读到回车结束，读到 Ctrl+C（`0x03`）当取消、读到 Ctrl+D 在空行时也当取消。不管怎么结束，都照存下的原样写回终端设置（一个守卫，`Drop` 里写回，`panic` 也写回）。
  - Windows：照控制台的办法关回显、关 Ctrl+C 的处理（`ENABLE_PROCESSED_INPUT`），读到 Ctrl+C 当取消，同样守卫写回原来的模式。
  - 用仓库已有的依赖能做就不加新的（看 `rustix`、`windows-sys` 现在有没有、开了哪些特性）；要加的先过许可证门禁。能把 `rpassword` 去掉就去掉。
- 取消以后：印一行「没存，取消了」（照现在的语言），退出码 130（照 Unix 被 Ctrl+C 打断的习惯）。`miyu setup` 里在贴 key 那一步取消，整个 setup 照取消办，配置一个字都不写。
- 粘贴的内容照旧：回车前的整段，去掉前后空白。

### 蓝图改哪几节

- `docs/blueprint/cli/login.md`、`cli/setup.md` 讲贴 key 的那一段：写明按 Ctrl+C 取消、回显开回来、退出码 130。
- 先例：`crates/miyu-cli/src/config/console.rs`（`Console`）和它的假终端（`crates/miyu-cli/tests/support/configuring.rs`）；`miyu-cli/tests/login.rs`。

### 不做什么

- 别的读输入的地方（`miyu setup` 里敲数字那几步、`miyu config edit`）：只管贴 key 那一段。

### 验收

1. 测试（先写，退回改之前的代码要红）：
   - 真的伪终端里（Unix 上，照仓库已有的伪终端测试的办法）：读 key 时送 Ctrl+C，进程以 130 退出、印了取消那一句，结束后终端设置和开始前一样（回显开着）；
   - 送正常的 key 加回车：读到的对，终端设置照原样；
   - 读到一半出错、`panic`：守卫照样写回（用单元测试测守卫本身）；
   - `miyu setup` 贴 key 那一步取消：配置文件、密钥文件都没动；
   - Windows：守卫和控制台模式的读写各一条（CI 的 Windows 上跑；真按键模拟不了的写明）。
2. 给模型看的字：没有。请求形状探针零变化。
3. 手写变异 10 个左右，挑关键的，全被逮住；`CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑全绿。
4. 真终端（主会话合并前做）：`miyu login` 贴 key 时按 Ctrl+C，回到 shell 以后敲字有回显。
