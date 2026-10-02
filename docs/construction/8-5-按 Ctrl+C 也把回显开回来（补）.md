## 施工单 8-5（补）：按 Ctrl+C 也把回显开回来

状态：已完成（2026-10-02，8-5 施工时发现；怎么修主会话定；验收结果见最后一节）。

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

### 验收结果

- **测试**（先写；新模块改之前编译不过，改了行为的那几条退回去是红的——`crates/miyu-cli/src/config/console/hidden.rs`、`unix.rs`、`windows.rs` 是新文件，`rpassword` 删掉之后旧实现不存在；`exit::CANCELLED`、`key_paste_cancelled()` 是新加的，退回去编不过）：
  - `crates/miyu-cli/src/config/console/hidden.rs`（内联 `mod tests`，10 个）：一个字节一个字节地认哪一步是什么——回车（`\r`、`\n`）结束，空行回车不算取消；`Ctrl+C` 随时取消（空行、攒了字都一样）；`Ctrl+D` 只在空行取消，攒了字的当没按；退格（`DEL`、`BS`）删上一个字节，空行退格不下溢；读到头是空的（攒了一半的也是，不交回半截）；不是 UTF-8 报 `InvalidData`。这些测试不碰真的终端，纯逻辑。
  - `crates/miyu-cli/src/config/console/unix.rs`（内联 `mod tests`，2 个，开真的伪终端）：`Guard::enter` 关 `ECHO`、`ISIG`、`ICANON`，丢掉照原样写回去；`panic` 半路丢掉（`catch_unwind`）照样写回去。
  - `crates/miyu-cli/src/config/console/windows.rs`（内联 `mod tests`，2 个，开这个进程自己的 `CONIN$`，没有控制台时跳过）：模式读写一来一回；`Guard::enter` 关三个位，丢掉照原样写回去。
  - `crates/miyu-cli/tests/login.rs` 加了 `cancelling_the_key_paste_saves_nothing_and_exits_130`（假终端报 `Interrupted`）：两种语言都说「没存，取消了」/「Not saved, cancelled」，退出码 130，密钥文件一个字都没写。
  - `crates/miyu-cli/tests/setup.rs`、`tests/support/onboarding.rs` 加了 `cancelling_the_key_paste_writes_nothing`（`Typist` 多一个 `cancel_key` 开关）：贴 key 那一步取消，退出码 130，系统配置、密钥文件都没动，一个请求都没发给假服务器。
  - `crates/miyu/tests/login_tty.rs`（新文件，`#![cfg(unix)]`，真的伪终端、真的 `miyu` 子进程、真起一个核心）两条：
    - `ctrl_c_while_pasting_cancels_exits_130_and_restores_echo`：贴了一截 key 就送 `0x03`，子进程以 130 退出、屏幕上印了「没存，取消了」、密钥文件没写、主端查到的 `ECHO`/`ISIG`/`ICANON` 和建伪终端时一样。
    - `a_normal_paste_with_enter_is_read_and_echo_stays_as_it_was`：送 key 加 `\n`，退出码 0、印了「· deepseek 的 key 存好了」、密钥文件写进去了、终端设置原样。
    - 这两条本机实测跑了 5 次没有抖动；读主端用非阻塞 `ioctl(FIONBIO)` 轮询，等到提示语那一句再送按键，避免贴早了被还没切到读模式的进程吃掉。伪终端默认开着 `ONLCR`，子进程写的每个 `\n` 在从端变成 `\r\n`，断言前把 `\r` 去掉。
  - 「`miyu setup` 贴 key 那一步取消」没有开真的伪终端：那条逻辑（`setup/flow.rs` 的 `read_key`）只是看 `Console::hidden()` 回的是不是 `Interrupted`，不碰终端设置本身，终端设置这一半已经在 `login_tty.rs` 测过了，照剧本回的假终端够用（见下「施工时定的」）。
  - Windows 的整段读取流程（真按键按 `Ctrl+C`）没法在 CI 上模拟，只测了 `Guard` 和控制台模式的读写两条，照施工单「验收」第 1 条最后一行写明。
- **给模型看的字**：没有。这一步只碰 `miyu-cli` 的终端输入层（`Console`、`login.rs`、`setup/flow.rs`），不碰提示词、工具描述、请求组装，这个仓库也没有「请求形状探针」这类测具——没有什么可量的，零变化。
- **变异**：手写 11 个，一次改一处、跑相关的测试、改回去，全部逮住：
  - `hidden.rs` 5 个：`Ctrl+C` 的字节从 `0x03` 改成别的（`ctrl_c_cancels_whether_the_line_is_empty_or_not` 红）；`Ctrl+D` 去掉「只在空行」的判断、一律取消（`ctrl_d_after_typing_something_is_ignored_not_cancelled` 红）；退格改成不删（`backspace_deletes_the_last_byte_del_and_bs_both_work` 红）；读到头改成报取消而不是空的（`eof_with_nothing_typed_is_none`、`eof_mid_line_is_none_not_the_partial_line` 两条红）；回车的匹配去掉 `\r`、只认 `\n`（`enter_ends_the_line_cr_and_lf_both_work` 红）。
  - `unix.rs` 2 个：`Guard::enter` 不关 `ISIG`（`enter_turns_off_echo_isig_icanon_and_drop_restores_them` 红）；`Drop` 整段改成空的、不写回去（两条守卫测试一起红，`panic` 那条还顺带证明了 `catch_unwind` 真的接住了 `panic`）。
  - `login.rs`、`setup/flow.rs`、`ask.rs` 4 个：`login.rs` 取消那一支的退出码改成 `exit::ERROR`（`cancelling_the_key_paste_saves_nothing_and_exits_130` 红：130 变 1）；`login.rs` 整段取消分支删掉，落进通用的 `Err` 分支（同一条测试红，泄出内部错误文字「test cancel」而不是「没存，取消了」）；`setup/flow.rs` 同样删掉取消分支（`cancelling_the_key_paste_writes_nothing` 红，退出码变 1、泄出「test cancel」）；`exit::CANCELLED` 从 130 改成 3（`cancelling_the_key_paste_saves_nothing_and_exits_130` 红：130 变 3）。
  - 每个变异跑完都从暂存的原文件整份覆盖回去，`diff` 比对确认和改动前逐字节一样。
- `CARGO_BUILD_JOBS=5 cargo xtask check` 本地（Linux）八项全过（格式、clippy、文档、分层、纯逻辑、行数、许可证、测试）；第一遍「格式」「文档」没过——「格式」是测试代码自己没跑过 `cargo fmt`；「文档」是模块文档里的 `[`unix`]`、`[`windows`]`、`[`super::unix`]`、`[`super::windows`]`、`[`hidden::read_line`]` 几处链接写错了（`unix`/`windows` 两个子模块各只在一个平台编，链接不出来；`hidden::read_line` 从 `unix.rs`/`windows.rs` 里是平级模块，得写 `super::hidden::read_line` 或者直接写导入进来的 `read_line`），改成纯代码（不加 `[...]`）或者改对路径，第二遍全过。
- **第一次推送，三台机器的 CI**：Linux 绿，macOS、Windows 两条都红，本地（Linux）的门禁逮不到，因为出的都是别的平台才有的问题：
  - **macOS**：`login_tty.rs` 开头的 `echoing()` 对着伪终端的主端调 `tcgetattr`，Linux 上主端、从端的 termios 是同一份（本机验证过，见上「测试」），但 macOS 的 pty 是 BSD 那一套，主端不认 `tcgetattr` 这个 ioctl，报 `ENOTTY`（Inappropriate ioctl for device）。改成照从端的路径另开一份短命的文件描述符来问，两条测试都改了调用的地方。
  - **Windows**：`console/windows.rs` 的 `read_hidden()` 里，`Guard::enter(&console)` 借着 `console`（一份 `OpenOptions` 开出来的 `File`，不是靠 `.lock()` 包一层的）活到函数结束（`_guard` 丢的时候 `Drop` 要用它），闭包里又写 `console.read(&mut byte)`——这会被 Rust 自动借成 `&mut console`，跟 `_guard` 已经借着的 `&console` 撞上，`E0502`。Unix 那一边没有这个问题：`unix.rs` 读的是 `stdin.lock()` 另外造出来的 `StdinLock`，和 `_guard` 借的 `&stdin` 是两个不同的东西，不会撞。改法：闭包里写成 `(&console).read(&mut byte)`，走 `&File` 那个 `Read` 实现，只借不夺；`console` 这个变量本身也不用再声明成 `mut` 了。
  - 这两处本地都没法直接编出来验证（这台机器没有 macOS，Windows 目标装了但没有 MSVC 的 `lib.exe`，`cargo check --target x86_64-pc-windows-msvc` 过不了 `libsqlite3-sys` 的构建脚本）：`windows.rs` 的借用问题另起一个不带 `libsqlite3-sys` 的最小工程，用 `cargo check --target x86_64-pc-windows-msvc`（只查类型、不链接）复现、验证改法；macOS 那处改法凡是本机能想到的别的办法都没法在这台机器上实际跑一遍，照读到的错误信息（`ENOTTY`）直接改。推送以后等三台机器的 CI 重新跑一遍确认。
- **第二次推送**：Windows 绿了（借用那处改对了），macOS 又红了一处新的——`echoing()` 改完以后不再报 `ENOTTY`，但两条测试都在最后一句断言上失败，`screen` 里只有「粘贴 ... 的 key（不显示）：」，后面「没存，取消了」或者「· ... 的 key 存好了」那一句没收到，退出码那一条断言却是过的（没在它那里报错，说明子进程确实照期望的退出码退出了）。说明子进程退出前确实把那一句写出去了，只是 `wait_for_exit` 在 `child.wait()` 之后只读一轮，`child.wait()` 只保证进程真的退出，不保证它写给从端的最后几个字节这时已经能在主端这头非阻塞读到——这一步在 macOS 上看来会晚一点。先改成读到空的连着攒够 5 次（100 毫秒没有新字节）才算收全、最多等 10 秒，推送以后再等一遍。
- **第三次推送**：这条退路没堵住——macOS 还是在同一句断言上红，连着读 5 次空的也没等到。说明问题不是「晚一点才能读到」，是「从端的最后一份文件描述符关掉以后，这时候还没读走的字节直接被丢掉了」（BSD 的 pty 是这样，Linux 不会丢——这台机器是 Linux，复现不出「字节真的被丢」这个现象本身，只能照 macOS CI 日志里的症状倒推根因）。等子进程退出以后再读，本来就是在跟「从端关闭」赛跑，赛跑赢不了就会丢。改法是不赛跑：开一个专门的线程，从子进程起来那一刻就一直阻塞着读主端，攒到一个 `Arc<Mutex<Vec<u8>>>` 里，这个线程自己在从端全关、读到错误的时候结束；主线程只负责等按键时机、送按键、等子进程退出、等读的线程收尾（`join()`），不再自己去读。`wait_for`、`drain`、`wait_for_exit` 三个函数合并进一个 `Capture` 类型（`start`/`wait_for`/`text`/`finish`），不再要非阻塞 `ioctl(FIONBIO)` 轮询。这一处 Linux 上也验证不出当时的「丢字节」现象（Linux 从来不丢），只能照"子进程退出和主端读到最后一句有时间差"这个症状推断根因、换一种结构上更稳妥的办法（边写边收，不事后补读），跑了五次确认 Linux 上照样稳（~0.2s/两条）。
  - 同一次推送还顺手发现 `console/windows.rs` 的两个单元测试会互相踩：`mode_round_trips_through_get_and_set` 红了（读到的模式位和自己刚存的不一样，`left: 496, right: 503`，正好差 `ENABLE_ECHO_INPUT`、`ENABLE_LINE_INPUT`、`ENABLE_PROCESSED_INPUT` 这三个位）。控制台的模式是整个进程共用的一份，不是跟着文件描述符走的（和 Unix 的 `termios` 不一样，每个测试自己开一对伪终端、各自独立）；`cargo test` 默认多线程并行跑，这个测试的「存、读回、比对」和另一个测试（`enter_turns_off_the_three_bits_and_drop_restores_them`）的 `Guard::enter`/`Drop` 在同一个进程里抢同一份全局状态，谁的改动被另一个测试夹在中间读到，断言就会不一致。改法是把两个测试合成一个（`mode_round_trips_and_guard_restores_it`），不再并行。这处本机 Windows 都没有（这台机器是 Linux），改法照读到的现象推断，推送以后再等一遍确认。
- **第四次推送**：Windows 绿了（两个测试合成一个，不再互踩），macOS 又红了一处新的，不在 `login_tty.rs`（那两条这次过了，`Capture` 改法是对的）：`unix.rs` 自己的单元测试 `drop_restores_even_when_a_panic_unwinds_through_it` 红了——`panic` 以后 `Guard` 照样写回去了（这条本身没问题，`left`/`right` 两边 `ECHO`、`ISIG`、`ICANON` 都一致），但 `left` 多一个 `PENDIN`，整份 `local_modes` 比对不相等。`PENDIN`（有要重打一遍的输入）是 BSD 的 `termios` 自己的记账位，在关、开 `ICANON` 这样切一轮以后会自己冒出来，不是 `Guard` 该管、也不是它承诺要照原样写回的那几个位——这个测试的姊妹测试（`enter_turns_off_echo_isig_icanon_and_drop_restores_them`）本来就只查 `ECHO`/`ISIG`/`ICANON` 三个位、没比对整份 `local_modes`，所以没中这个坑；这一条改成照同样的办法，只查这三个位，不再 `assert_eq!` 整份结构。这处本机 Linux 验证不出 `PENDIN`（Linux 的 `termios` 没有这个位），照 macOS CI 日志里的位差直接改。
- **第五次推送（`69f298e6`）：三台机器全绿**（<https://github.com/SHORiN-KiWATA/miyu-agent-remake/actions/runs/36960742876>），长跑（`randomized`）也绿。`check (ubuntu-24.04)`、`check (windows-latest)`、`check (macos-latest)` 四个 job 都过。到这一步算验收通过，待主会话真终端实测、合并。
- **新依赖**：没加新 crate。删了 `rpassword` 7.5、它带进来的 `rtoolbox`。`miyu-cli` 多开了已经在依赖图里的几个包的功能：`rustix`（本来就在 `miyu-basesystem`、`miyu-fs`、`miyu-ipc` 用着）开 `termios`，只给测试用的 `pty` 放进 `[dev-dependencies]`；`windows-sys`（本来就在别的 crate 用着）给 `miyu-cli` 直接加一条依赖、开 `Win32_System_Console`；`tracing`（本来就在依赖图里）给 `miyu-cli` 直接加一条依赖。门禁的「许可证」过了，记进了 `docs/blueprint/licenses.md`「依赖记录」。
- **施工时定的**（技术细节照推荐定）：
  - 不开 `/dev/tty`（Windows 上 `CONIN$` 例外，照以前 `rpassword` 的办法）：`hidden()` 只在 `typed()` 是真的时才会被调用，这时标准输入本来就是终端，直接操作 `io::stdin()`/控制台输入句柄就够，不用另开一个设备文件（2026-10-02 施工时定，图纸没写到这处细节）。
  - Unix 上除了图纸点名的 `ECHO`、`ISIG`，连 `ICANON`（行缓冲）一起关：图纸要的是「自己一个字节一个字节读」，`ICANON` 不关的话内核会攒到一整行才交出来，`Ctrl+C` 要等按了回车才能让程序看到，不算「随时」取消，所以照「一个字节一个字节读」的字面意思把 `ICANON` 也关了（2026-10-02 施工时定）。关了 `ICANON` 以后内核不再替它处理退格，所以 `read_line`（`hidden.rs`）自己接了退格（`DEL`/`BS` 删上一个字节），不然原来能退格改字的体验会丢。
  - 「`miyu setup` 贴 key 那一步取消」用照剧本回的假终端（`Typist`）测，不开真的伪终端：那一步的逻辑只看 `Console::hidden()` 的 `Err` 是不是 `Interrupted`，终端层面的东西（真按 `Ctrl+C`、`Guard` 写不写回去）和 `miyu login` 走的是同一段代码，已经在 `login_tty.rs` 里用真的伪终端测过，这里重复开一遍只是多测「`Console` 报错以后 `setup` 的几步函数是不是真的都没被调用」，没必要再搭一遍真终端（`config.md` 原来就定过「测试换成照剧本回的假终端」这条，照旧）。
  - `Guard` 的单元测试也开真的伪终端（不是直接造一个假的 `Fd`）：`tcgetattr`/`tcsetattr` 只对终端设备有意义，给普通文件或管道调会直接报错，没法测出「关没关、写回没写回」。
  - 退出码常量 `exit::CANCELLED = 130` 放进 `ask.rs` 的 `pub mod exit`（`miyu login`、`miyu setup` 共用 `crate::exit`），和已有的 `OK`/`ERROR`/`INTERRUPTED`/`UNATTENDED`/`NO_MODEL` 放在一起，不是同一套语义（`INTERRUPTED = 3` 是 `miyu ask` 执行中途按 `Ctrl+C`，这里是贴 key 这一步本身取消），图纸里已经写明是「照 Unix 被 Ctrl+C 打断的习惯」，不是这个仓库自己的编号方案，两套并存。
  - 读到头（EOF）一律当「没收到」（`Ok(None)`），不分「一个字都没读到」和「读了一半设备就断了」：后一种只有终端挂掉才会发生，是真正的异常情况，不在「取消」「正常读到」两条路里，掉了也不丢东西（后面照「没收到 key」那条老路处理，退出码 1）。

**主会话合并前实测**（2026-10-02，真的伪终端）：`miyu login deepseek`，提示贴 key 以后敲了半截 key 再按 Ctrl+C：退出码 130，印「没存，取消了」；半截 key 没回显到屏幕上；退出以后终端的 `ECHO`、`ICANON`、`ISIG` 都照原样开着；系统配置目录下没有生成密钥文件。
