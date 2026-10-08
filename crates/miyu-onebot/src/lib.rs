//! 通讯平台的桥 `miyu-onebot`（`docs/blueprint/onebot.md`，施工 O-8）：经 OneBot v11 接 QQ 的一个头，和终端界面、网页平级
//! （`docs/designs/18-通讯平台.md` 第三节）。O-8 只有骨架：主人的私聊、只有文字。
//!
//! - [`settings`]：起来时读系统配置的 `onebot.listen`、`onebot.token`。
//! - [`tuning`]：桥自己的数（`resources/software/onebot/bridge.json`）：路径、等回应多久、队列多长。
//! - [`serve`]：连核心、开监听，把下面几样接起来；核心断了就退。
//! - `listen`：NapCat 反连进来的反向 WebSocket：只听本机、令牌、一个机器人号一条连接。
//! - [`onebot`]：OneBot v11 的事件和动作：读私聊、写 `send_private_msg`、调用和回应照 `echo` 配对。
//! - `core`：跟核心的那一头：`venue.session`、带 `as` 的 `session.send`、订阅、她的回话。
//! - [`texts`]：说给人听的字（`resources/software/onebot/human/`）。
//! - `current`：桥手里的令牌：每次重读配置照读到的换上，改了不用重启（施工 O-16 补二）。
//! - [`web`]：桥自己的 WebUI（施工 O-16，第二条）：页面、`/ws` 转给核心、`/status`、`/token`、`/apply`，底子是共用的
//!   `miyu-webserve`。
//! - [`open`]：`miyu-onebot web`：桥在跑的话，开浏览器到 WebUI，还没设过登录密码的带上一次性码。
//!
//! 分层照 `01-架构.md` 第九节第 5 层：只用本机传输连核心（WebUI 那一头用 `miyu-webserve`）；场所、平台上的人的编号经第 2
//! 层的群聊内核 `miyu-chat` 拼；读配置借核心那一份代码（`miyu-core` 的清单、`miyu-endpoint` 的
//! 配置服务），只读。

mod core;
mod current;
mod listen;
pub mod onebot;
pub mod open;
pub mod serve;
pub mod settings;
pub mod texts;
pub mod tuning;
pub mod web;

/// 运行日志的目标。
pub const TARGET: &str = "miyu::onebot";

/// 还没有字可用时印原话的开头，也是程序的名字。
pub const PROGRAM: &str = "miyu-onebot";
