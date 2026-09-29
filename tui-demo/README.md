## 终端界面演示程序

和后端并行开发的前端演示：全屏的终端界面，连上重制版的核心跟她说话。现在长什么样，见蓝图 `docs/blueprint/tui.md`。

不进仓库的 workspace，自己单独编，门禁不查它；lints 照抄工作区那一套（`Cargo.toml`）。

### 怎么跑

先在工作树根目录编一次核心（`cargo build -p miyu`），再：

```sh
cd tui-demo
DEEPSEEK_API_KEY=<key> MIYU_HOME=/tmp/miyu-tui-home MIYU_RESOURCES=$PWD/../resources MIYU_CORE_BIN=$PWD/../target/debug/miyu cargo run
```

- 变量写在命令前面，不要 `export`：旧版 Miyu 也认 `MIYU_HOME`。
- 拉起核心只认 `MIYU_CORE_BIN`，不去 PATH 里找 `miyu`（PATH 上的可能是旧版）。核心已经在跑的，不给也能连上。
- 上下文窗口多大，核心还不告诉头，先记在 `resources/models.json`。

### 按键、鼠标、版式

都写在蓝图 `docs/blueprint/tui.md` 里（「对外的样子」「怎么走」「样子」），这里不再抄一份。
