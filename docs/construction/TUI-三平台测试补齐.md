# TUI 三平台测试补齐

目的：本步启用独立 TUI 三平台 CI 后，清掉旧测试的平台和异步收帧假设；不改运行代码和体验。

已见红：macOS sessions::a_long_list_shows_ten_rows_and_scrolls_with_the_pick 收到列表标题时只收到四行就断言十行；Windows clippy 发现 Unix 专用 Duration/Instant/Unreadable/read_with 导入未按平台限制。

办法：剪贴板导入与使用它们的测试同样 cfg(unix)；列表在同一个 WAIT 上限内等十行完整到达，不加固定延时、不减断言。另核对到 Ctrl+G 的旧 PTY 测试未 cfg(unix)，蓝图已明确 Windows 没有此功能，将测试限制到实现的平台。

蓝图：tui.md「按键」Ctrl+G 的 Windows 不支持约定不变；测试守卫记录 PTY 按完成条件而不是标题先到判定列表。与卡片实现分开提交，旧失败在 CI 中保留证据。验收 Linux 原有测试及 fmt/clippy，重新运行 TUI 三平台 CI。

Windows 测试再见红：拖文件测试只转义空格，原生路径的反斜杠被 shell 切词器当作转义而丢失。测试输入同时转义反斜杠；保持原有附件断言，运行代码不动。

第三轮 Windows：拖文件及卡片单元测试通过，四项旧 PTY 测试见红。源码证据：crossterm 0.29 的 event/source/windows.rs 使用 Console.read_single_input_event，sys/windows/parse.rs 解析 Win32 记录；Unix 的 sys/unix/parse.rs 才解析括号粘贴、kitty CSI u。ConPTY 转换输入与输出，原始输出光标断言也不能当程序写出字节。四项测试用 cfg_attr(windows, ignore = 原因) 显式保留限制，不减其他断言、不改运行代码；Windows 原生事件注入测具未建，不声称相关端到端行为已验证。参考 https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/ 。
