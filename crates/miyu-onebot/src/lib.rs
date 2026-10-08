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
//!
//! 分层照 `01-架构.md` 第九节第 5 层：只用本机传输连核心；场所、平台上的人的编号经第 2 层的群聊内核 `miyu-chat` 拼；
//! 读配置借核心那一份代码（`miyu-core` 的清单、`miyu-endpoint` 的
//! 配置服务），只读。

mod core;
mod listen;
pub mod onebot;
pub mod serve;
pub mod settings;
pub mod texts;
pub mod tuning;

/// 运行日志的目标。
pub const TARGET: &str = "miyu::onebot";
