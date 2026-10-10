## 施工单 S-1：给界面用的门面 `miyu-client`

状态：2026-10-11 合了（a9b8fca0，CI run 1664）。设计 `32-仓库拆分.md` 第二节；项目主人 2026-10-11 定终端、网页拆成各自的仓库，拆法交给主会话。

### 起因

两个界面连核心借的是核心仓库里的 `miyu-ipc`、`miyu-store`、`miyu-kernel`、`miyu-config`、`miyu-log`、`miyu-webserve`。拆开以后它们照 git 依赖钉住核心，要有一个只给界面用的门面，核心里别的 crate 怎么改都碍不着它们。

### 这一步做什么

1. 新 crate `miyu-client`（第 3 层）：照两个头 2026-10-11 报的用法 `pub use` 转出 `connect`、`places`、`texts`、`protocol`、`manifest`、`log` 六组；`testkit` 打开时有 `WORKSPACE_ROOT`。
2. `miyu-webserve` 的 `open.rs` 和 `CoreCommand` 挪进 `miyu-client`（`git mv`，行为不变）；`miyu-webserve` 只剩听端口、给页面、`/ws` 转发。
3. 用 `open`、`CoreCommand` 的两处改过来：`miyu-web`（`open.rs`、`settings.rs`、`serve.rs`）、`miyu-onebot`（`control.rs`，桥不再依赖 `miyu-webserve`）。别的 import 由两个头在 S-2 自己改。
4. 层序表登记、图纸 `client.md`、`webserve.md`、设计 32 第二节第 3 条。

### 验收

- `miyu-client` 的 `tests.rs`：门面上的每一样用一遍，拿掉哪一样当场编不过。
- 网页、桥原来的测试照旧过（`open` 行为不变）。
- `cargo xtask check` 八项；三台 CI。
