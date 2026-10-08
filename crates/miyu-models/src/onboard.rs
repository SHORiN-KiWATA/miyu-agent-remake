//! 第一次接入的纯逻辑（`docs/blueprint/models.md`「怎么走」第七条，施工 8-11）：协议的 `provider.detect`、`provider.catalog`、
//! `provider.test` 和 `miyu setup` 要用的几样，不碰文件、网络、环境变量。
//!
//! - [`listed`]：目录和档案里的每一家合起来，名字、驱动、地址、能不能用（`listing.rs`）；[`key_vars`]、[`looked_for`]：找哪些
//!   环境变量；[`local_services`]：探本机的哪几家；[`search`]：`provider.catalog` 搜、排；
//! - [`recommend()`]：试哪个模型、推荐哪个（`recommend.rs`）；
//! - [`Candidate`]：还没写进配置的一家，写成一份最终值，和配好的一家走同一条路推（`candidate.rs`）；
//! - [`featured()`]：常用的几家怎么写（`featured.rs`，施工 8-11 再补）。
//!
//! 环境变量有没有设、本机的服务回没回，由调用的一方（协议端点、会话那一层）查好交进来。

mod candidate;
mod featured;
mod listing;
mod recommend;

pub use candidate::{CANDIDATE, Candidate, value_key};
pub use featured::{Featured, featured};
pub use listing::{Listed, key_vars, listed, local_services, looked_for, search};
pub use recommend::{MIN_WINDOW, Offered, recommend, released};

#[cfg(test)]
use miyu_config::secret::Reference as KeyRef;

#[cfg(test)]
mod tests;
