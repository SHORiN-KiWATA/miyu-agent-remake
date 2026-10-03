# TUI 三平台测试补齐

目的：本步启用独立 TUI 三平台 CI 后，清掉旧测试的平台和异步收帧假设；不改运行代码和体验。

已见红：macOS sessions::a_long_list_shows_ten_rows_and_scrolls_with_the_pick 收到列表标题时只收到四行就断言十行；Windows clippy 发现 Unix 专用 Duration/Instant/Unreadable/read_with 导入未按平台限制。

办法：剪贴板导入与使用它们的测试同样 cfg(unix)；列表在同一个 WAIT 上限内等十行完整到达，不加固定延时、不减断言。另核对到 Ctrl+G 的旧 PTY 测试未 cfg(unix)，蓝图已明确 Windows 没有此功能，将测试限制到实现的平台。

蓝图：tui.md「按键」Ctrl+G 的 Windows 不支持约定不变；测试守卫记录 PTY 按完成条件而不是标题先到判定列表。与卡片实现分开提交，旧失败在 CI 中保留证据。验收 Linux 原有测试及 fmt/clippy，重新运行 TUI 三平台 CI。
