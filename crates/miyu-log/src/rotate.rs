//! 按大小轮换的文件（`docs/designs/28-运行日志.md` 第一节）：满了在两行之间换一份，一行不拆开；
//! `core.log` 挪成 `core.log.1`，`.1` 挪成 `.2`，依此类推；正在写的之外留 `keep` 份，最老的删掉，
//! 和 logrotate 的 `rotate` 一个口径。进程再起来，接着写原来那一份。
//!
//! Unix 上新建的目录 0700、文件 0600，只有本人能进、能读（`07-存储.md` 第二节，施工 4-9 再补四上）；已经有的不改。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::layer::Sink;

/// 一份按大小轮换的日志文件。
#[derive(Debug)]
pub struct RotatingFile {
    dir: PathBuf,
    name: String,
    limit: u64,
    keep: usize,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    /// 正在写的那一份；换份的当中关掉了，是空的。
    file: Option<File>,
    size: u64,
}

impl RotatingFile {
    /// 打开 `dir` 下的 `<name>.log` 接着写，目录没有就建；满 `limit` 字节换一份，正在写的之外留
    /// `keep` 份（至少 1 份）。
    ///
    /// # Errors
    ///
    /// 建不了目录、打不开文件。
    pub fn open(dir: &Path, name: &str, limit: u64, keep: usize) -> io::Result<RotatingFile> {
        create_dir(dir)?;
        let path = dir.join(format!("{name}.log"));
        let file = options().append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(RotatingFile {
            dir: dir.to_path_buf(),
            name: name.to_string(),
            limit,
            keep: keep.max(1),
            state: Mutex::new(State {
                file: Some(file),
                size,
            }),
        })
    }

    /// 正在写的那一份。
    pub fn path(&self) -> PathBuf {
        self.nth(0)
    }

    /// 第 `n` 份：0 是正在写的，1 是上一份。
    fn nth(&self, n: usize) -> PathBuf {
        match n {
            0 => self.dir.join(format!("{}.log", self.name)),
            n => self.dir.join(format!("{}.log.{n}", self.name)),
        }
    }

    /// 写到磁盘的缓冲里。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "日志写不进去，没有别的地方可以报"
    )]
    pub fn flush(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(file) = state.file.as_mut() {
            let _ = file.flush();
        }
    }

    /// 换一份：先关掉正在写的（Windows 上开着的文件不好改名），最老的 `.<keep>` 删掉，其余的往后挪
    /// 一位，正在写的变成 `.1`，再开一份新的。
    fn rotate(&self, state: &mut State) -> io::Result<()> {
        state.file = None;
        let oldest = self.nth(self.keep);
        if oldest.exists() {
            fs::remove_file(&oldest)?;
        }
        for n in (1..self.keep).rev() {
            let from = self.nth(n);
            if from.exists() {
                fs::rename(&from, self.nth(n + 1))?;
            }
        }
        fs::rename(self.nth(0), self.nth(1))?;
        state.file = Some(options().write(true).truncate(true).open(self.nth(0))?);
        state.size = 0;
        Ok(())
    }

    /// 换份当中出了错，正在写的那一份关掉了：照原来的名字接着写。
    fn reopen(&self, state: &mut State) {
        if state.file.is_none()
            && let Ok(file) = options().append(true).open(self.nth(0))
        {
            state.size = file.metadata().map_or(0, |meta| meta.len());
            state.file = Some(file);
        }
    }
}

impl Sink for RotatingFile {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "日志写不进去、换不了份，没有别的地方可以报：接着往原来那一份写"
    )]
    fn write_line(&self, line: &str) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes = line.len() as u64 + 1;
        if state.size > 0 && state.size + bytes > self.limit {
            let _ = self.rotate(&mut state);
            self.reopen(&mut state);
        }
        let mut data = String::with_capacity(line.len() + 1);
        data.push_str(line);
        data.push('\n');
        if let Some(file) = state.file.as_mut()
            && file.write_all(data.as_bytes()).is_ok()
        {
            state.size += bytes;
        }
    }
}

/// 建目录，连同缺的上级：Unix 上新建的 0700。
fn create_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// 打开一份，没有就建：Unix 上新建的 0600。
fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

#[cfg(test)]
mod tests;
