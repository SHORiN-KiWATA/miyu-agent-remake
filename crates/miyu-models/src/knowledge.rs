//! 查模型资料要的几份（`docs/blueprint/models.md`「怎么走」第一、二条，施工 8-7）：档案、认原厂的表、在用的目录、用出来的、
//! 供应商的列表。核心一份，执行器拿着；查的时候借一份 [`Knowledge`] 进来，这一层不碰文件。

use std::collections::BTreeMap;

use crate::catalog::Loaded;
use crate::matching::Vendors;
use crate::observed::{Learned, ProviderList};
use crate::profile::Profiles;

/// 这一刻手头的资料。
#[derive(Debug, Clone, Copy)]
pub struct Knowledge<'a> {
    /// 认得出的供应商的档案。
    pub profiles: &'a Profiles,
    /// 认原厂的表。
    pub vendors: &'a Vendors,
    /// 在用的目录；还没读完、都读不了的没有。
    pub catalog: Option<&'a Loaded>,
    /// 用出来的。
    pub learned: &'a Learned,
    /// 供应商的列表：配好的编号 → 列表。
    pub lists: &'a BTreeMap<String, ProviderList>,
}
