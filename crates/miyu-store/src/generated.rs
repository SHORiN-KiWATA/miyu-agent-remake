//! 核心生成的派生文件（`docs/blueprint/config.md`「怎么走」第一条第 7 条，施工 8-1）：配置的两份 JSON Schema 和参考
//! 文件，放在 `state/config/`。删了，核心下次起来重新生成；核心不读它们。
//!
//! 和磁盘上已经有的逐字节比，一样的不写：编辑器、监视它的程序看不到没用的改动。不一样的先写旁边的临时文件
//! `.<文件名>.<进程号>-<计数>.tmp`、同步，再改名盖上，再同步目录：写到一半断电，磁盘上还是原来那一份。
//!
//! 比配置文件的写法（8-3）少几样：不顺着链接找本体、不带原来的权限位、替换之前不再读一次、Windows 上改名失败不重试。
//! 它们是派生的，没人链接、没人手改；这一次写不成，下次起来再写。
//!
//! [`Staged`]（施工 R-5 中）：太大、不好一次拿在手里的（下载的本机 embedding 模型，几十 MB），一块一块写进同一种临时文件，
//! 提交时同步、改名盖上、同步目录；没提交就放下的删掉临时文件。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::durable::{create_dir, create_temp, discard, sync_dir, temp_name};

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

/// 一块一块写、提交了才出现的文件（施工 R-5 中）。写进旁边的临时文件；[`Staged::commit`] 才同步、改名盖上 `path`、同步
/// 目录；没提交就放下的（出错、核对不上）删掉临时文件，`path` 原来的不动。
#[derive(Debug)]
pub struct Staged {
    /// 开着的临时文件；提交了就是空的。
    file: Option<File>,
    temp: PathBuf,
    path: PathBuf,
    dir: PathBuf,
}

impl Staged {
    /// 为 `path` 开一个临时文件。没有的目录建上。
    ///
    /// # Errors
    ///
    /// `path` 不是某个目录里的文件；建不了目录、建不了临时文件。
    pub fn create(path: &Path) -> io::Result<Staged> {
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a file in a directory", path.display()),
            ));
        };
        create_dir(dir)?;
        let name = name.to_string_lossy();
        let (temp, file) = create_temp(dir, || temp_name(&name))?;
        Ok(Staged {
            file: Some(file),
            temp,
            path: path.to_path_buf(),
            dir: dir.to_path_buf(),
        })
    }

    /// 接着写 `bytes`。
    ///
    /// # Errors
    ///
    /// 写不进（磁盘满了这类）。
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        match &mut self.file {
            Some(file) => file.write_all(bytes),
            None => Err(io::Error::other("already committed")),
        }
    }

    /// 同步、关上，改名盖上 `path`，再同步目录。
    ///
    /// # Errors
    ///
    /// 同步不了、改不了名：临时文件删掉，`path` 原来的不动。
    pub fn commit(mut self) -> io::Result<()> {
        let Some(file) = self.file.take() else {
            return Err(io::Error::other("already committed"));
        };
        let stored = file
            .sync_data()
            .and_then(|()| {
                drop(file);
                fs::rename(&self.temp, &self.path)
            })
            .and_then(|()| sync_dir(&self.dir));
        if stored.is_err() {
            discard(&self.temp);
        }
        stored
    }
}

impl Drop for Staged {
    /// 没提交就放下的：关上、删掉临时文件。
    fn drop(&mut self) {
        if self.file.take().is_some() {
            discard(&self.temp);
        }
    }
}

#[cfg(test)]
mod tests;
