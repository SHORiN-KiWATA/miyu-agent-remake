# TUI herdr 会话恢复

目的：每个 herdr 窗格重启后恢复各自的 Miyu 会话。2026-10-03 项目主人批准试做。

蓝图：tui.md 增加指定会话启动、herdr 恢复命令上报。完整 ID 用于机器恢复；显式恢复优先于 recent；不存在的会话报错，不自动另开。新建或切换后更新恢复命令，/new 清除旧绑定。只改 TUI，复用核心 subscribe 和事件补发。

验收：先加 PTY 指定恢复测试观察失败，再实现；两段对话分别恢复，不串会话；错误 ID 不回退。上报顺序、切换、清除、退出以测试守住。Linux fmt、clippy、全套 TUI 测试；独立命名 herdr 会话实测两窗格重启恢复，不碰真实 MIYU_HOME 和默认 herdr 服务。

风险：herdr 文档最低版本与 0.9.3 源码不一致，以隔离实测为准；恢复程序需可在 PATH 找到，核心启动配置需在恢复环境可用。2026-10-03 项目主人真人验收通过；按 demo 流程合入 proto/tui-demo 并推送。


实测结果：herdr 0.9.3 两窗格分别使用假模型产生不同对话，停止隔离服务后重开，两窗格由不同的新终端进程恢复各自原对话；/new 清除绑定后再次重启，该窗格回 shell，另一窗格仍恢复。没有调用真实供应商、没有停止默认 herdr 服务。完整 TUI 测试、格式与 clippy 通过；macOS、Windows 未实测。

试用：`~/.cache/miyu-tui-herdr-resume/try-herdr` 打开独立命名会话，窗格中执行 `miyu-tui-demo`。两个窗格分别说话；在外部终端运行 `~/.cache/miyu-tui-herdr-resume/try-herdr server stop`，再运行启动脚本，看各自历史回来。启动脚本固定连接现有 demo 数据根 `~/.cache/miyu-tui-home`，保证重启环境一致；不覆盖 PATH 上的旧 Miyu。

真人验收：2026-10-03 项目主人确认通过。空会话未创建 ID，不保存恢复绑定；发出第一句话创建会话后可以恢复，符合既定设计。
