//! 通讯平台的桥 `miyu-onebot`（`docs/blueprint/onebot.md`，施工 O-8）：经 OneBot v11 接 QQ 的一个头，和终端界面、网页平级
//! （`docs/designs/18-通讯平台.md` 第三节）。O-8 只有骨架：主人的私聊、只有文字。
//!
//! - [`settings`]：核心在握手时交来、变了推过来的配置：两个端口、令牌（施工 O-20）；没交的端口照清单的默认值。
//! - [`tuning`]：桥自己的数（`resources/software/onebot/bridge.json`）：路径、等回应多久、队列多长。
//! - [`serve`]：在核心亲手给的管道（标准输入输出，施工 O-18）上握手、开监听，把下面几样接起来；核心关了管道就停。
//! - `listen`：NapCat 反连进来的反向 WebSocket：只听本机、令牌、一个机器人号一条连接。
//! - [`onebot`]：OneBot v11 的事件和动作：读私聊、写 `send_private_msg`、调用和回应照 `echo` 配对。
//! - `core`：跟核心的那一头：握手（不带凭据）、`venue.session`、带 `as` 的 `session.send`、订阅、她的回话。
//! - [`status_file`]：桥的状态文件 `state/packages/onebot/status.json`，`status` 读它（施工 O-18）。
//! - [`control`]：`miyu onebot start`、`stop`、`restart`、`status`：调核心的 `extension.*`（施工 O-18）。
//! - [`logs`]：`miyu onebot logs [-f]`：印运行日志和标准错误（施工 O-18）。
//! - [`texts`]：说给人听的字（`resources/software/onebot/human/`）。
//! - `current`：桥手里最新的配置：握手交来的，核心推来新的就换上，改了不用重启（施工 O-20）。
//! - [`web`]：桥自己的 WebUI（施工 O-16，第二条）：页面、`/ws` 转给核心、`/status`、`/token`、`/apply`，底子是共用的
//!   `miyu-webserve`。
//! - [`open`]：`miyu-onebot web`：桥在跑的话，开浏览器到 WebUI，还没设过登录密码的带上一次性码。
//!
//! 分层照 `01-架构.md` 第九节第 5 层：只用本机传输连核心（WebUI 那一头用 `miyu-webserve`）；场所、平台上的人的编号经第 2
//! 层的群聊内核 `miyu-chat` 拼；不依赖核心的 crate（`miyu-core`、`miyu-endpoint`，`18-通讯平台.md` 第一节）：配置由核心交
//! （施工 O-20）。

pub mod control;
mod core;
mod current;
mod listen;
pub mod logs;
pub mod onebot;
pub mod open;
pub mod serve;
pub mod settings;
pub mod status_file;
pub mod texts;
pub mod tuning;
pub mod web;

/// 运行日志的目标。
pub const TARGET: &str = "miyu::onebot";

/// 还没有字可用时印原话的开头，也是程序的名字。
pub const PROGRAM: &str = "miyu-onebot";

/// 这个软件包的编号（`resources/packages/onebot.toml`，施工 O-18）：`extension.*` 照它找桥，状态目录、标准错误的文件照它起名。
pub const PACKAGE: &str = "onebot";
