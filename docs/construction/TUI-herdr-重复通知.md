# herdr 通知调查与回归修正

目的：恢复后台桌面弹窗，守住 herdr 中的通知与声音分工。

已核实：本机 herdr 未开启 toast，默认 delivery=off；声音独立开启。此前假定 herdr 与 TUI 各发一次并禁止 TUI 弹窗，造成没有弹窗的回归。恢复 TUI 桌面通道，保留 herdr 声音和状态上报；不改核心、不改全局 herdr 配置。

蓝图：tui.md「系统通知」恢复 herdr 中桌面通知，明确声音由 herdr 发出。

验收：后台 plan 返回 System、sound=false；PTY 假通知命令记录恰好一次桌面调用，herdr working/idle 仍到达。先证实测试在回归版本失败，再修复。独立工作树；2026-10-03 项目主人实测当前版本没有重复通知，验收通过。历史重复原因未确认，不记为已定位的代码缺陷。

验证：回归测试修复前失败、修复后通过；herdr 相关单元测试和 PTY 测试通过；fmt、clippy 通过。
