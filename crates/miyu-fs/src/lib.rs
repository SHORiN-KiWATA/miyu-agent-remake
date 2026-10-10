//! 核心进程里的文件工具怎么碰文件（`docs/designs/11-权限与沙盒.md` 第四节、第七节 A9，施工 4-3 上）。
//!
//! 文件工具跑在核心进程里，不在操作系统的沙盒里，所以要自己守边界：
//!
//! - [`resolve()`]：把她给的路径换成真实的位置，`~` 开头的照 [`tilde()`] 接家目录；
//! - [`Boundary`]：真实的位置落在哪一片（[`Zone`]）：能读能写、只能读、谁都不能碰、边界以外；
//! - [`open_file`]：安全地打开一份要读的文件：不跟随链接（Unix 上路上一层都不跟，施工 5-10 下）、不阻塞、只开普通文件；
//! - [`too_wide`]：头报来的工作目录太宽，不能拿它当工作区（施工 4-3 下）；
//! - [`within`]：一个真实的位置在不在某个目录里，往下走目录的工具照它跳过数据根（施工 4-4 下）；
//! - [`replace()`]：把一份文件整体换成新的内容，先写临时文件再改名盖上去（施工 4-6 上，4-7 上挪来）；
//! - [`trash`]：系统的回收站，放进去、移回来（施工 4-6 下，4-7 上挪来）；
//! - [`list_dir()`]：列一层目录，像 shell 补全那样（`fs.list`，施工 W-2）；
//! - [`Index`]：模糊找文件的清单；[`score()`]：打分，`fs.find` 用（施工 W-2）；
//! - [`read_range()`]：安全地打开以后读一段，`blob.get`、`fs.read` 共用（施工 W-6）。

mod boundary;
mod find;
mod list;
#[cfg(unix)]
mod nofollow;
mod open;
mod range;
mod replace;
mod resolve;
pub mod trash;
mod wide;

pub use boundary::{Boundary, Places, Zone, system_dirs, within};
pub use find::{Built, CAP, DEPTH, FRESH_SECS, Found, Index, MAX_INDEXES, SHOWN, score};
pub use list::{Entry, list_dir};
pub use open::{Kind, OpenError, open_file};
pub use range::{MAX_LENGTH, Segment, read_range};
pub use replace::replace;
pub use resolve::{ResolveError, resolve, resolve_itself, tilde};
pub use wide::too_wide;
