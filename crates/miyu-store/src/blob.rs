//! blob（`docs/designs/07-存储.md` 第五节）：图片、文件、超长的工具输出、策略快照这些大内容，按
//! 内容哈希存成单独的文件，事件里只放引用（`03-事件模型.md` E4）。一个账号一份，不跨账号去重
//! （S5）：`home/<账号>/blobs/<前两位>/<64 位十六进制>`。
//!
//! 存：先写进 `tmp/` 里的临时文件、同步，再改名成它的哈希、同步目录。返回时它已经落了盘，这才能
//! 写引用它的事件（07 第四节「先落 blob，再写引用它的事件」）。改名是原子的，所以磁盘上不会有
//! 写了一半的 blob；崩溃留在 `tmp/` 里的，回收的时候再清。
//!
//! 分块上传（`web-module.md`「六、分块上传」，施工 W-5）：暂存文件也在 `tmp/` 里，叫 `upload-<编号>`，和
//! [`Blobs::put`] 自己的临时文件名（`<进程号>-<计数>`）撞不上；`blob.write` 一块一块写进去，`blob.close` 照
//! [`Blobs::put`] 同一个办法改名进位置。崩了、被杀留下的，核心起来时照 [`Blobs::clear_uploads`] 清一遍。
//!
//! 分块读（`web-module.md`「七、分块读」，施工 W-6）：[`Blobs::read_range`] 一块一块读出来，不用整份先取
//! 进内存。

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use miyu_kernel::id::ContentHash;

use crate::durable::{create_dir, create_temp, discard, sync_dir};

/// 放临时文件的目录。和前两位的目录（两位十六进制）撞不上。
const TMP: &str = "tmp";

/// 分块上传的暂存文件名打头的几个字（施工 W-5）：和 [`temp_name`] 起的名字（纯数字加一个连字符）区分开。
const UPLOAD: &str = "upload-";

/// 一个账号的 blob。
#[derive(Debug, Clone)]
pub struct Blobs {
    /// `home/<账号>/blobs/`。
    dir: PathBuf,
}

impl Blobs {
    /// 一个账号的 blob 目录，平时是 [`DataRoot::blobs`](crate::root::DataRoot::blobs)。用到时才建。
    pub fn new(dir: PathBuf) -> Blobs {
        Blobs { dir }
    }

    /// 这个哈希的 blob 放在哪：`<前两位>/<64 位>`。
    pub fn path(&self, hash: &ContentHash) -> PathBuf {
        self.fan(hash).join(hash.hex())
    }

    /// 存一份内容，返回它的哈希。返回时它已经落了盘。已经有了的不重写，只把修改时间刷成现在：
    /// 回收的宽限期按修改时间算，刚又传了一遍的不能当成旧的删掉（07 第五节）。
    ///
    /// # Errors
    ///
    /// 建不了目录、写不进、同步不了、改不了名。
    pub fn put(&self, content: &[u8]) -> io::Result<ContentHash> {
        let hash = ContentHash::of(content);
        let fan = self.fan(&hash);
        let path = fan.join(hash.hex());
        if path.is_file() {
            freshen(&path)?;
            // 它可能是同一时刻别处刚改好名、还没同步目录的。
            sync_dir(&fan)?;
            return Ok(hash);
        }
        let tmp = self.dir.join(TMP);
        create_dir(&tmp)?;
        let (temp, file) = create_temp(&tmp, temp_name)?;
        let stored = store(file, content, &temp, &fan, &path);
        if stored.is_err() {
            discard(&temp);
        }
        stored?;
        Ok(hash)
    }

    /// 读一段：从 `offset` 起读最多 `length` 个字节，读到结尾就停；`offset` 过了结尾的是空的；`length` 写 0
    /// 只问大小。`size`（第二个值）是打开那一刻的大小；不重新核对整份内容的哈希，核对在核心自己用整份内容的
    /// 时候（施工 W-6，`web-module.md`「七、分块读」第 1、3、4 条）。安全地打开照 [`miyu_fs::read_range`]，
    /// 和 `fs.read` 共用一份，不另写一套跟链接的判断。
    ///
    /// # Errors
    ///
    /// 没有这个 blob；读不了。
    pub fn read_range(
        &self,
        hash: &ContentHash,
        offset: u64,
        length: u64,
    ) -> Result<(Vec<u8>, u64), BlobError> {
        // 先换成真实的位置：数据根本身可能经过一层链接（macOS 的临时目录在 `/var` 下，`/var` 是链接），安全地打开
        // 路上一层链接都不跟，不换就一个都打不开。blob 在核心自己的数据根里，不是人给的路径（2026-10-02 主会话定）。
        let real = match std::fs::canonicalize(self.path(hash)) {
            Ok(real) => real,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(BlobError::Missing(hash.clone()));
            }
            Err(error) => return Err(BlobError::Io(error)),
        };
        let segment = miyu_fs::read_range(&real, offset, length).map_err(|error| match error {
            miyu_fs::OpenError::NotFound => BlobError::Missing(hash.clone()),
            other => BlobError::Io(io::Error::other(other)),
        })?;
        Ok((segment.data, segment.size))
    }

    /// 取一份内容，核对它的哈希：读出来和名字对不上，报错，不悄悄用，也不删（07 第五节）。
    ///
    /// # Errors
    ///
    /// 没有这个 blob；读出来和名字对不上；读不了。
    pub fn get(&self, hash: &ContentHash) -> Result<Vec<u8>, BlobError> {
        let content = match fs::read(self.path(hash)) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(BlobError::Missing(hash.clone()));
            }
            Err(error) => return Err(BlobError::Io(error)),
        };
        if ContentHash::of(&content) != *hash {
            return Err(BlobError::Corrupt(hash.clone()));
        }
        Ok(content)
    }

    /// 前两位的目录。
    fn fan(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.hex();
        self.dir.join(hex.get(..2).unwrap_or(hex))
    }

    /// 一个分块上传的暂存文件在哪：`tmp/upload-<编号>`（施工 W-5）。
    pub fn upload_path(&self, upload: &str) -> PathBuf {
        self.dir.join(TMP).join(format!("{UPLOAD}{upload}"))
    }

    /// 开一个新的分块上传：建好 `tmp/`，新建它的暂存文件（只许新建），交回在哪（施工 W-5）。`upload`
    /// 撞了名字算不该走到的状态：编号是 `uploads.rs` 现造的，撞上要么是随机数坏了，要么是重用了没清
    /// 掉的编号，两种都往上报成 `io::Error`，不在这里猜。
    ///
    /// # Errors
    ///
    /// 建不了 `tmp/`；这个文件已经在、或者新建不了。
    pub fn create_upload(&self, upload: &str) -> io::Result<PathBuf> {
        let tmp = self.dir.join(TMP);
        create_dir(&tmp)?;
        let path = self.upload_path(upload);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(path)
    }

    /// 写一块进暂存文件：从 `offset` 开始写 `data`（施工 W-5，`blob.write`）。重开文件写，不是一直占着：
    /// 一块一个来回，之间可以隔好一会儿。
    ///
    /// # Errors
    ///
    /// 打不开、定位不了、写不进。
    pub fn write_upload_chunk(&self, path: &Path, offset: u64, data: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)
    }

    /// 收齐了：同步、关上（Windows 上开着的文件改不了名），改名进位置；已经有这份内容的，删掉暂存的，
    /// 还是那一个 blob（施工 W-5，`blob.close`，照 [`Blobs::put`] 第 5 条同一个办法，`hash` 是边写边算好的）。
    ///
    /// # Errors
    ///
    /// 打不开、同步不了、建不了目录、改不了名。
    pub fn finish_upload(&self, path: &Path, hash: &ContentHash) -> io::Result<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        file.sync_data()?;
        drop(file);
        let fan = self.fan(hash);
        let dest = fan.join(hash.hex());
        if dest.is_file() {
            freshen(&dest)?;
            sync_dir(&fan)?;
            discard(path);
            return Ok(());
        }
        create_dir(&fan)?;
        settle(path, &dest)?;
        sync_dir(&fan)
    }

    /// 扔掉一个分块上传的暂存文件：作废了、连接断了（施工 W-5）。删不掉就留着，下次核心起来照
    /// [`Blobs::clear_uploads`] 清。
    pub fn discard_upload(&self, path: &Path) {
        discard(path);
    }

    /// 核心起来时清掉这个账号 `tmp/` 里分块上传留下的暂存：`upload-*`，崩了、被杀留下的（施工 W-5，
    /// `web-module.md`「怎么走」第六条第 6 款）。交回删了几个；`tmp/` 还没建过（这个账号还没传过东西）
    /// 当没有，交回 0。单个文件删不掉的跳过，接着清别的，不整个停下。
    ///
    /// # Errors
    ///
    /// 读不了 `tmp/`（不是「没有这个目录」的那种）。
    pub fn clear_uploads(&self) -> io::Result<usize> {
        let tmp = self.dir.join(TMP);
        let entries = match fs::read_dir(&tmp) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let left_over = name.to_str().is_some_and(|name| name.starts_with(UPLOAD));
            if left_over && fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// 取不出来。
#[derive(Debug)]
pub enum BlobError {
    /// 没有这个 blob。
    Missing(ContentHash),
    /// 读出来的内容和它的名字对不上：磁盘坏了，或者被别的程序改过。不自动修，也不删。
    Corrupt(ContentHash),
    /// 读不了。
    Io(io::Error),
}

impl fmt::Display for BlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobError::Missing(hash) => write!(f, "no blob {hash}"),
            BlobError::Corrupt(hash) => {
                write!(f, "blob {hash} does not match its name; left as it is")
            }
            BlobError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BlobError {}

/// 写进临时文件、同步、关上（Windows 上开着的文件改不了名），改名成它的哈希，再同步目录。
fn store(mut file: File, content: &[u8], temp: &Path, fan: &Path, path: &Path) -> io::Result<()> {
    file.write_all(content)?;
    file.sync_data()?;
    drop(file);
    create_dir(fan)?;
    settle(temp, path)?;
    sync_dir(fan)
}

/// 改名成它的哈希。改名失败、可目标已经有了，算成功：两个会话同时存同一份，Windows 上后改名的
/// 那个可能失败，内容是一样的，删掉自己的临时文件就行。
fn settle(temp: &Path, path: &Path) -> io::Result<()> {
    match fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(_) if path.is_file() => {
            discard(temp);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// 临时文件的名字：进程号加一个计数。
fn temp_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// 把修改时间刷成现在。
fn freshen(path: &Path) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

#[cfg(test)]
mod tests;
