//! 核心生成的派生文件（`docs/blueprint/config.md`「怎么走」第一条第 7 条，施工 8-1）：配置的两份 JSON Schema 和参考
//! 文件，放在 `state/config/`。删了，核心下次起来重新生成；核心不读它们。
//!
//! 和磁盘上已经有的逐字节比，一样的不写：编辑器、监视它的程序看不到没用的改动。不一样的先写旁边的临时文件
//! `.<文件名>.<进程号>-<计数>.tmp`、同步，再改名盖上，再同步目录：写到一半断电，磁盘上还是原来那一份。
//!
//! 比配置文件的写法（8-3）少几样：不顺着链接找本体、不带原来的权限位、替换之前不再读一次、Windows 上改名失败不重试。
//! 它们是派生的，没人链接、没人手改；这一次写不成，下次起来再写。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::durable::{create_dir, create_temp, discard, sync_dir};

/// 把 `path` 写成 `content`：一样的不写，交回 `false`；写了交回 `true`。没有的目录建上。
///
/// # Errors
///
/// 建不了目录（例如该是目录的地方是个文件）；写不进、同步不了、改不了名。
pub fn write(path: &Path, content: &[u8]) -> io::Result<bool> {
    if fs::read(path).is_ok_and(|old| old == content) {
        return Ok(false);
    }
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file in a directory", path.display()),
        ));
    };
    create_dir(dir)?;
    let name = name.to_string_lossy();
    let (temp, file) = create_temp(dir, || temp_name(&name))?;
    let stored = store(file, content, &temp, path, dir);
    if stored.is_err() {
        discard(&temp);
    }
    stored.map(|()| true)
}

/// 写进临时文件、同步、关上（Windows 上开着的文件改不了名），改名盖上，再同步目录。
fn store(mut file: File, content: &[u8], temp: &Path, path: &Path, dir: &Path) -> io::Result<()> {
    file.write_all(content)?;
    file.sync_data()?;
    drop(file);
    fs::rename(temp, path)?;
    sync_dir(dir)
}

/// 临时文件的名字：点开头（大多数系统上不显示），带上原来的名字、进程号和一个计数。
fn temp_name(name: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests;
