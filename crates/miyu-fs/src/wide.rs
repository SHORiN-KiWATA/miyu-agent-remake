//! 工作目录太宽（`11-权限与沙盒.md` 第四节「当前目录太宽」，施工 4-3 下）：系统的家目录、根目录，包含数据根
//! 或者落在数据根里的，不拿它当工作区，退回账号的工作区。否则装完第一次在家目录里敲 `miyu`，整个家目录都进了
//! 边界。

use std::path::Path;

/// 真实的位置 `dir` 太不太宽，不能拿它当工作区：是家目录 `home`、是根目录，包含数据根 `data_root`，或者落在
/// 数据根里而不在账号自己的工作区 `own` 里。几样都要先换成真实的位置再交进来。
pub fn too_wide(dir: &Path, home: Option<&Path>, data_root: &Path, own: &Path) -> bool {
    home == Some(dir)
        || dir.parent().is_none()
        || data_root.starts_with(dir)
        || (dir.starts_with(data_root) && !dir.starts_with(own))
}
