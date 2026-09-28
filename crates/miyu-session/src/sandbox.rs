//! 一次调用带的沙盒（`docs/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：照派出去那一刻实际生效的那一级写
//! 规格。完全放开不进沙盒；工作区能写这一轮的工作目录、临时目录；只读哪儿都不能写；两级都藏数据根。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use miyu_fs::resolve;
use miyu_kernel::event::Permission;
use miyu_sandbox::{Sandboxed, Spec};

use crate::guard::{Effective, effective};

/// 给调用写沙盒要的：助手在哪、系统的家目录、数据根。一个会话一份，这台机器上的沙盒能用才有。
#[derive(Debug, Clone)]
pub(crate) struct Sandbox {
    helper: PathBuf,
    home: Option<PathBuf>,
    data_root: PathBuf,
}

impl Sandbox {
    /// 助手是 `helper`，`~` 照 `home` 换，藏的是 `data_root`。
    pub(crate) fn new(helper: PathBuf, home: Option<PathBuf>, data_root: PathBuf) -> Sandbox {
        Sandbox {
            helper,
            home,
            data_root,
        }
    }

    /// 照实际生效的那一级 `permission`、这一轮的工作目录 `cwd` 写一次调用的沙盒：完全放开的不带。碰磁盘（换真实的
    /// 位置、建沙盒自己的临时目录），在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 数据根落在临时目录里，沙盒自己的临时目录建不成，或者有了却不是只给本人的。
    pub(crate) fn for_call(
        &self,
        permission: &Permission,
        cwd: &str,
    ) -> io::Result<Option<Sandboxed>> {
        let level = effective(permission);
        if level == Effective::Full {
            return Ok(None);
        }
        let data_root = real(&self.data_root);
        let mut env = Vec::new();
        let write = if level == Effective::Workspace {
            // 工作目录照权限策略的办法换（头报来的可能是 `~`），换不成的照原样。
            let cwd = resolve(Path::new(cwd), self.home.as_deref(), cwd)
                .unwrap_or_else(|_| PathBuf::from(cwd));
            let temp = real(&std::env::temp_dir());
            // 数据根在临时目录里：藏的落在能写的里面，挖不了洞。不放整个临时目录，放沙盒自己的一个。
            let temp = if data_root.starts_with(&temp) {
                let own = own_temp(&data_root)?;
                env.push((OsString::from("TMPDIR"), own.clone().into_os_string()));
                own
            } else {
                temp
            };
            vec![cwd, temp]
        } else {
            Vec::new()
        };
        Ok(Some(Sandboxed {
            helper: self.helper.clone(),
            spec: Spec {
                write,
                hidden: vec![data_root],
            },
            env,
        }))
    }
}

/// 换成真实的位置；换不成的照原样，助手会再换一次，换不成就不跑。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 沙盒自己的临时目录：数据根旁边的 `<数据根>-sandbox-tmp`，没有就建，只给本人。
fn own_temp(data_root: &Path) -> io::Result<PathBuf> {
    let mut name = data_root
        .file_name()
        .map(OsString::from)
        .unwrap_or_default();
    name.push("-sandbox-tmp");
    let dir = data_root.with_file_name(name);
    private_dir(&dir, data_root)?;
    Ok(dir)
}

/// 建只给本人的目录（0700）；已经有的，要是目录、不是链接、和数据根 `owner_of` 同一个属主，组和别人一点权限都没有。
#[cfg(unix)]
fn private_dir(dir: &Path, owner_of: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {}
    }
    let found = std::fs::symlink_metadata(dir)?;
    let owner = std::fs::metadata(owner_of)?.uid();
    if found.is_dir() && found.uid() == owner && found.mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(not_private(dir))
    }
}

/// 建目录；已经有的，要是目录、不是链接。临时目录在本人的用户目录里，别人进不来。
#[cfg(windows)]
fn private_dir(dir: &Path, _owner_of: &Path) -> io::Result<()> {
    match std::fs::create_dir(dir) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {}
    }
    if std::fs::symlink_metadata(dir)?.is_dir() {
        Ok(())
    } else {
        Err(not_private(dir))
    }
}

/// 有了却不是只给本人的。
fn not_private(dir: &Path) -> io::Error {
    io::Error::other(format!("{} is not a private directory", dir.display()))
}
