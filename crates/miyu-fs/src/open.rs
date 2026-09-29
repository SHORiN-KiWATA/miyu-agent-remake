//! 安全地打开一份要读的文件（`11-权限与沙盒.md` 第七节，施工 4-3 上）：先换成真实的位置、查过边界，
//! 再打开。打开时不跟随链接（Unix 上路上一层都不跟，施工 5-10 下）、不阻塞，开了以后看是不是普通文件。

use std::fmt;
use std::fs::File;
use std::io;
use std::path::Path;

/// 不是普通文件时，它是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 目录。
    Directory,
    /// 链接：检查完以后被换成了链接，或者本来就是。
    Link,
    /// FIFO（命名管道）。
    Fifo,
    /// 设备。
    Device,
    /// 套接字。
    Socket,
    /// 别的。
    Other,
}

/// 打不开。
#[derive(Debug)]
pub enum OpenError {
    /// 没有这个文件。
    NotFound,
    /// 在，但不是普通文件。
    NotAFile(Kind),
    /// 别的错，例如没有权限。
    Io(io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NotFound => f.write_str("no such file"),
            OpenError::NotAFile(kind) => {
                let what = match kind {
                    Kind::Directory => "a directory",
                    Kind::Link => "a link",
                    Kind::Fifo => "a FIFO",
                    Kind::Device => "a device",
                    Kind::Socket => "a socket",
                    Kind::Other => "not a regular file",
                };
                write!(f, "it is {what}")
            }
            OpenError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OpenError {}

/// 打开真实的位置 `real` 上的一份普通文件来读。
///
/// # Errors
///
/// 没有这个文件；在，但不是普通文件（目录、链接、FIFO、设备、套接字）；别的错。
pub fn open_file(real: &Path) -> Result<File, OpenError> {
    let file = match sys::open(real) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(OpenError::NotFound),
        Err(error) => {
            // 打不开的，先看它是不是一个不能开的东西：链接、目录、套接字……
            return Err(match std::fs::symlink_metadata(real) {
                Ok(metadata) if !metadata.is_file() => OpenError::NotAFile(kind(&metadata)),
                _ if sys::is_link_error(&error) => OpenError::NotAFile(Kind::Link),
                _ => OpenError::Io(error),
            });
        }
    };
    let metadata = file.metadata().map_err(OpenError::Io)?;
    if metadata.is_file() {
        Ok(file)
    } else {
        Err(OpenError::NotAFile(kind(&metadata)))
    }
}

/// 不是普通文件的，是什么。
fn kind(metadata: &std::fs::Metadata) -> Kind {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Kind::Link;
    }
    if file_type.is_dir() {
        return Kind::Directory;
    }
    sys::special(&file_type)
}

#[cfg(unix)]
mod sys {
    use std::fs::{File, FileType};
    use std::io;
    use std::os::unix::fs::FileTypeExt;
    use std::path::Path;

    use super::Kind;

    /// 只读、路上一层链接都不跟、不阻塞（FIFO 没人写时不卡住）：`crate::nofollow`（施工 5-10 下）。
    pub(super) fn open(real: &Path) -> io::Result<File> {
        crate::nofollow::open_read(real)
    }

    /// 最后一层或者路上有链接报的错。
    pub(super) fn is_link_error(error: &io::Error) -> bool {
        error.raw_os_error() == Some(libc::ELOOP)
    }

    /// FIFO、设备、套接字。
    pub(super) fn special(file_type: &FileType) -> Kind {
        if file_type.is_fifo() {
            Kind::Fifo
        } else if file_type.is_char_device() || file_type.is_block_device() {
            Kind::Device
        } else if file_type.is_socket() {
            Kind::Socket
        } else {
            Kind::Other
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::fs::{File, FileType, OpenOptions};
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::Path;

    use super::Kind;

    /// Win32 的 `FILE_FLAG_OPEN_REPARSE_POINT`：打开链接、目录联接本身，不跟着走。
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    /// 只读，不跟着链接、目录联接走。
    pub(super) fn open(real: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(real)
    }

    /// Windows 上打开链接本身不报错，开了以后看得出来。
    pub(super) fn is_link_error(_: &io::Error) -> bool {
        false
    }

    /// 别的：Windows 上没有 FIFO、设备、套接字这几种文件。
    pub(super) fn special(_: &FileType) -> Kind {
        Kind::Other
    }
}
