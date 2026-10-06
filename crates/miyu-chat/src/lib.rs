//! 群聊内核（`docs/blueprint/chat.md`，`docs/designs/18-通讯平台.md` 第一节）：通讯平台里和平台无关的那一层，第 2 层的
//! 纯逻辑。进来的是字和事件，出去的是判定，不碰磁盘、网络、时钟；软件包 `miyu-onebot` 链接它，以后别的平台的桥也链接
//! 同一个库，它不编进核心（`docs/designs/01-架构.md` 第九节）。
//!
//! 现在只有场所规则（`chat.md` 第一条，施工 O-1）：
//!
//! - [`Rules::parse`]：读一组规则文件（[`File`]，出厂的和系统的），照文件名排好先后；写错的变成 [`Problem`]，其余照收；
//! - [`Rules::resolve`]：套到一个场所（[`Venue`]）上，得出每一项的值和来处（[`Resolved`]、[`Origin`]）。
//!
//! 读文件、文件大小、是不是 UTF-8 由读文件的一方管（`chat.md` 施工时定的第 5 条）。进站链、线路规程、主动回复判断、出站链
//! 随后面的 O 步加。

mod rules;

pub use rules::{Entry, File, Origin, Problem, Read, Resolved, Rules, Source, Venue, VenueKind};
