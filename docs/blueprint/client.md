## 给界面用的门面 `miyu-client`

施工 S-1，设计 `32-仓库拆分.md` 第二节。终端、网页、接入QQ 这些头连核心要的几样都从这里拿：拆成各自的仓库以后，界面照 git 依赖钉住核心的一个提交，只依赖这一个 crate，核心里别的 crate 怎么改都碍不着它们。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-client/src/lib.rs` | 门面：几组 `pub use`，`CoreCommand`，`testkit` 打开时的 `WORKSPACE_ROOT` |
| `crates/miyu-client/src/open.rs` | 照终端的样子连核心（出示本机令牌）、一问一答（`Core`），系统的办法开浏览器（`Browser`、`SystemBrowser`），这台机器的语言（`locale`）。从 `miyu-webserve` 挪来，行为不变（`webserve.md` 的「怎么用」照旧） |
| `crates/miyu-client/src/tests.rs` | 门面上的每一样用一遍：拿掉了哪一样当场编不过 |

### 门面上有什么

| 一组 | 转出什么 | 原来在 |
|---|---|---|
| `connect` | `connect`、`connect_or_start`、`connect_or_start_bare`、`spawn_detached`、`Connection`、`Ready`、`ConnectError`、`StartError` | `miyu-ipc` |
| `open` | `Core`、`Browser`、`SystemBrowser`、`locale` | 本 crate |
| `places` | `DataRoot`、`ResourceRoot`、`Env`、`Platform`、`locale`、`cache_root` | `miyu-store` |
| `texts` | `Human`、`Face`、`clean` | `miyu-store` 的 `human` |
| `protocol` | `SessionId`、`Said`、`Template` | `miyu-kernel` |
| `manifest` | `read`、`Value` | `miyu-config` |
| `log` | `install`、`LevelFilter` | `miyu-log` |
| 根 | `CoreCommand`；`testkit` 打开时 `WORKSPACE_ROOT`（钉住的那份检出的仓库根，测试照它读 `resources/`、`docs/designs/samples/`） | 本 crate |

### 规矩

1. **代码不搬**：转出去的留在原来的 crate，门面只 `pub use`；`open` 是唯一挪进来的，因为它本来就只给头用。
2. **门面的样子是协议的一部分**：加、改、拿掉门面上的东西，照改协议的做法告诉两个头（提交号、改了什么）；两个头 2026-10-11 报的用法是这一版的全部。
3. **层序**：第 3 层（`01-架构.md` 第九节），只引同层、下层。
