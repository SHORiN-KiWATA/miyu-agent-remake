//! 外部身份的会话的沙盒（施工 5-12 下，`11-权限与沙盒.md` 第三节「外部身份」）：读写都只限这一轮的工作区和加进来的目录，
//! 程序要的系统目录、`/dev`、自己的 `/proc/self` 只读；临时目录用工作区里的 `.tmp`（系统的临时目录里有别的程序的文件）；
//! 工具链的缓存不给。不看级别：完全放开也照样关进来，只读的照样能读工作区、哪儿都不能写。
//!
//! 不给整个 `/proc`：同一个用户的进程的环境变量都在 `/proc/<编号>/environ`，核心的环境里有各家的 key。只有关得住读的
//! 平台（[`miyu_sandbox::CONFINES_READS`]）才会走到这里：权限策略在别的平台上拒命令。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use miyu_fs::{resolve, system_dirs};
use miyu_kernel::event::Permission;
use miyu_sandbox::{Sandboxed, Spec};

use super::{Sandbox, real};
use crate::guard::{Effective, effective};

impl Sandbox {
    /// 外部身份的会话一次调用的沙盒。
    ///
    /// # Errors
    ///
    /// 工作区里的 `.tmp` 建不成。
    pub(super) fn confined_call(
        &self,
        permission: &Permission,
        cwd: &str,
        dirs: &[String],
    ) -> io::Result<Sandboxed> {
        let home = self.home.as_deref();
        let place =
            |dir: &str| resolve(Path::new(dir), home, dir).unwrap_or_else(|_| PathBuf::from(dir));
        let cwd = place(cwd);
        let temp = cwd.join(".tmp");
        std::fs::create_dir_all(&temp)?;
        let mut own: Vec<PathBuf> = vec![cwd];
        own.extend(dirs.iter().map(|dir| place(dir)));
        own.push(temp.clone());
        let mut read = system_dirs();
        read.extend([PathBuf::from("/dev"), PathBuf::from("/proc/self")]);
        read.extend(own.iter().cloned());
        let write = match effective(permission) {
            Effective::ReadOnly => Vec::new(),
            Effective::Workspace | Effective::Full => own,
        };
        Ok(Sandboxed {
            helper: self.helper.clone(),
            spec: Spec {
                write,
                hidden: vec![real(&self.data_root)],
                read: Some(read),
            },
            env: vec![(OsString::from("TMPDIR"), temp.into_os_string())],
        })
    }
}
