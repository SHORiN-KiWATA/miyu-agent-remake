## 终端界面演示程序

和后端并行开发的前端演示：全屏的终端界面，连上重制版的核心跟她说话。现在长什么样，见蓝图 `docs/blueprint/tui.md`。

不进仓库的 workspace，自己单独编，门禁不查它；lints 照抄工作区那一套（`Cargo.toml`）。

### 怎么跑

先在工作树根目录编一次核心（`cargo build -p miyu`），再：

```sh
cd tui-demo
DEEPSEEK_API_KEY=<key> MIYU_HOME=$HOME/.cache/miyu-tui-home MIYU_RESOURCES=$PWD/../resources MIYU_CORE_BIN=$PWD/../target/debug/miyu cargo run
```

- 变量写在命令前面，不要 `export`：旧版 Miyu 也认 `MIYU_HOME`。
- `MIYU_HOME` 放在持久目录里，别放 `/tmp`：`/tmp` 是 tmpfs，一重启模型配置、会话记录全没（2026-10-02 撞见过一次）。
- 拉起核心只认 `MIYU_CORE_BIN`，不去 PATH 里找 `miyu`（PATH 上的可能是旧版）。核心已经在跑的，不给也能连上。
- 上下文窗口多大，核心还不告诉头，先记在 `resources/models.json`。

### 按键、鼠标、版式

都写在蓝图 `docs/blueprint/tui.md` 里（「对外的样子」「怎么走」「样子」），这里不再抄一份。

### herdr 中恢复会话

`miyu-tui-demo --resume <完整会话 ID>` 恢复指定会话，优先于 `tui.startup=recent`。在 herdr 中创建或切换主会话后自动上报恢复命令；服务重启后每个窗格恢复各自的会话。`/new` 清掉旧绑定。只退出 herdr 客户端时原进程继续运行。

恢复程序 `miyu-tui-demo` 必须在恢复 shell 的 PATH 中，MIYU_HOME、MIYU_RESOURCES、MIYU_CORE_BIN 也须可用；临时写在原启动命令前的环境变量不会被 herdr 保存。demo 试用可用固定这些环境的启动脚本，放到 PATH 中；独立命名 herdr 会话先试，不停止默认服务。

Linux 隔离验收（需要 herdr）：`cargo test --test herdr_resume -- --ignored`。使用临时数据根、假模型和独立 XDG 配置，验证两窗格各自恢复、/new 清掉旧绑定，不读写真实会话。
