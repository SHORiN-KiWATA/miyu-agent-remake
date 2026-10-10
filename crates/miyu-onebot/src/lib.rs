//! 通讯平台的桥 `miyu-onebot`（`docs/blueprint/onebot.md`，施工 O-8）：经 OneBot v11 接 QQ 的一个头，和终端界面、网页平级
//! （`docs/designs/18-通讯平台.md` 第三节）。O-8 是骨架：终端管理员的私聊、只有文字；O-22 起群消息记进场所会话（旁听，不开回合）。
//!
//! - [`settings`]：核心在握手时交来、变了推过来的配置：NapCat 的端口、令牌（施工 O-20）；没交的端口照清单的默认值。
//! - [`tuning`]：桥自己的数（`resources/software/onebot/bridge.json`）：路径、等回应多久、队列多长。
//! - [`serve`]：在核心亲手给的管道（标准输入输出，施工 O-18）上握手、开监听，把下面几样接起来；核心关了管道就停。
//! - `listen`：NapCat 反连进来的反向 WebSocket：只听本机、令牌、一个机器人号一条连接。
//! - [`onebot`]：OneBot v11 的事件和动作：读私聊、群消息、撤回（施工 O-22）、认消息段、群成员的名字，写 `send_private_msg`、
//!   `send_group_msg`，调用和回应照 `echo` 配对。
//! - `core`：跟核心的那一头：握手（不带凭据）、`venue.session`、带 `as`、`venue` 的 `session.send`、撤回、订阅、她的回话。
//! - [`status_file`]：桥的状态文件 `state/packages/onebot/status.json`，`status` 读它（施工 O-18）。
//! - [`control`]：`miyu onebot start`、`stop`、`restart`、`status`：调核心的 `extension.*`（施工 O-18）。
//! - [`logs`]：`miyu onebot logs [-f]`：印运行日志和标准错误（施工 O-18）。
//! - [`texts`]：说给人听的字（`resources/software/onebot/human/`）。
//! - `current`：桥手里最新的配置：握手交来的，核心推来新的就换上，改了不用重启（施工 O-20）。
//! - `running`：桥跑着的样子：实际听的端口、连着的号、最新的配置；后台页的 `status`、状态文件照它说，推来的端口变化照它换
//!   （施工 O-28 下从桥自己的 WebUI 挪出来；桥自己的网页随 O-28 下去掉）。
//! - [`rules`]：场所规则和出厂数据（施工 O-21）：出厂的起来时读一次，系统的照群聊内核读好、合起来、套到场所上，变了下一次
//!   用就照新的。
//! - [`venue`]：`miyu onebot venue show <场所>`：一个场所每一项的值和来处（施工 O-21）。
//! - [`web`]：`miyu onebot web`：跑旁边的 `miyu web --package onebot`，打开网页软件里接入QQ 的后台页（施工 O-28 补）。
//!
//! 分层照 `01-架构.md` 第九节第 5 层：只用本机传输连核心（`start` 这几样照终端的样子连，用 `miyu-webserve` 的 `open::Core`）；场所、平台上的人的编号经第 2
//! 层的群聊内核 `miyu-chat` 拼；不依赖核心的 crate（`miyu-core`、`miyu-endpoint`，`18-通讯平台.md` 第一节）：配置由核心交
//! （施工 O-20）。

pub mod control;
mod core;
mod current;
mod listen;
pub mod logs;
pub mod onebot;
pub mod rules;
mod running;
pub mod serve;
pub mod settings;
pub mod status_file;
pub mod texts;
pub mod tuning;
pub mod venue;
pub mod web;

/// 运行日志的目标。
pub const TARGET: &str = "miyu::onebot";

/// 还没有字可用时印原话的开头，也是程序的名字。
pub const PROGRAM: &str = "miyu-onebot";

/// 这个软件包的编号（`resources/packages/onebot.toml`，施工 O-18）：`extension.*` 照它找桥，状态目录、标准错误的文件照它起名。
pub const PACKAGE: &str = "onebot";
