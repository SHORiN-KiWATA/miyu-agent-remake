//! 装、卸在磁盘上怎么做（施工 F-5 上，设计 `30-插件框架.md` 第九节，`docs/blueprint/packages.md`「装卸」）：只动管理员家目录
//! 那一层 `home/<管理员>/packages/`。
//!
//! - 装：清单拷成 `<编号>.toml`，旁边同名的目录（包自己的文件）拷成 `<编号>/`。先拷到点开头的暂存处，再换进去；原来就有的先
//!   挪到点开头的备份处，[`Placed::keep`] 删备份，[`Placed::undo`] 放回去。
//! - 卸家目录的：删掉清单和同名目录。卸出厂的：记一笔 `<编号>.removed`（空文件），资源目录不动，删掉这一笔就回来。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 卸掉的出厂的包记的那一笔的后缀。
pub const REMOVED: &str = ".removed";

/// 换进去了的一个包：留着原来那一份的备份，好撤回。
#[derive(Debug)]
pub struct Placed {
    /// 换进去的清单和目录。
    manifest: PathBuf,
    files: PathBuf,
    /// 原来那一份挪到的地方；原来没有的没有。
    old_manifest: Option<PathBuf>,
    old_files: Option<PathBuf>,
}

impl Placed {
    /// 就这样了：删掉备份。删不掉的留着，不碍事（点开头的不算清单）。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "备份删不掉就留着：点开头的不算清单，下一次装同一个包时先清掉"
    )]
    pub fn keep(self) {
        if let Some(old) = &self.old_manifest {
            let _ = fs::remove_file(old);
        }
        if let Some(old) = &self.old_files {
            let _ = fs::remove_dir_all(old);
        }
    }

    /// 撤回：删掉换进去的，原来的放回去。
    ///
    /// # Errors
    ///
    /// 删不掉、放不回去。
    pub fn undo(self) -> io::Result<()> {
        remove_if_there(&self.manifest)?;
        remove_dir_if_there(&self.files)?;
        if let Some(old) = &self.old_manifest {
            fs::rename(old, &self.manifest)?;
        }
        if let Some(old) = &self.old_files {
            fs::rename(old, &self.files)?;
        }
        Ok(())
    }
}

/// 把清单 `manifest`（旁边同名的目录 `files` 有的一起）装成家目录 `home` 里的包 `id`。
///
/// # Errors
///
/// 建不了目录、拷不了、换不进去；出了错的不留暂存。
pub fn place(home: &Path, id: &str, manifest: &Path, files: Option<&Path>) -> io::Result<Placed> {
    fs::create_dir_all(home)?;
    let staged_manifest = home.join(format!(".{id}.toml.new"));
    let staged_files = home.join(format!(".{id}.new"));
    remove_if_there(&staged_manifest)?;
    remove_dir_if_there(&staged_files)?;
    let staged = (|| {
        fs::copy(manifest, &staged_manifest)?;
        if let Some(files) = files {
            copy_dir(files, &staged_files)?;
        }
        Ok(())
    })();
    if let Err(error) = staged {
        discard(&staged_manifest, &staged_files);
        return Err(error);
    }
    let target_manifest = home.join(format!("{id}.toml"));
    let target_files = home.join(id);
    let old_manifest = aside(&target_manifest, home.join(format!(".{id}.toml.old")))?;
    let old_files = aside(&target_files, home.join(format!(".{id}.old")))?;
    fs::rename(&staged_manifest, &target_manifest)?;
    if files.is_some() {
        fs::rename(&staged_files, &target_files)?;
    }
    Ok(Placed {
        manifest: target_manifest,
        files: target_files,
        old_manifest,
        old_files,
    })
}

/// 删掉家目录 `home` 里的包 `id`：清单和同名目录。
///
/// # Errors
///
/// 删不掉。
pub fn take_out(home: &Path, id: &str) -> io::Result<()> {
    remove_if_there(&home.join(format!("{id}.toml")))?;
    remove_dir_if_there(&home.join(id))
}

/// 记一笔：出厂的包 `id` 卸掉了。
///
/// # Errors
///
/// 建不了目录、写不了。
pub fn mark_removed(home: &Path, id: &str) -> io::Result<()> {
    fs::create_dir_all(home)?;
    fs::write(home.join(format!("{id}{REMOVED}")), b"")
}

/// 删掉那一笔：出厂的包 `id` 装回来。
///
/// # Errors
///
/// 删不掉。
pub fn unmark_removed(home: &Path, id: &str) -> io::Result<()> {
    remove_if_there(&home.join(format!("{id}{REMOVED}")))
}

/// 家目录 `home` 里记着卸掉的出厂的包的编号。目录读不了的当没有。
pub fn removed(home: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(home) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(REMOVED)?;
            crate::personas::valid(id).then(|| id.to_string())
        })
        .collect();
    ids.sort();
    ids
}

/// 拷到一半出错，删掉暂存：删不掉的留着（点开头的不算清单，下一次装同一个包时先清掉），报原来那个错。
#[expect(
    clippy::let_underscore_must_use,
    reason = "删暂存是收拾，删不掉不改报的错"
)]
fn discard(manifest: &Path, files: &Path) {
    let _ = remove_if_there(manifest);
    let _ = remove_dir_if_there(files);
}

/// 有的话挪到 `to`，交回挪到哪；没有的没有。
fn aside(path: &Path, to: PathBuf) -> io::Result<Option<PathBuf>> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    remove_if_there(&to)?;
    remove_dir_if_there(&to)?;
    fs::rename(path, &to)?;
    Ok(Some(to))
}

/// 一层层拷目录 `from` 到 `to`：文件照内容拷，链接照它指的拷。
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if fs::metadata(entry.path())?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn remove_if_there(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn remove_dir_if_there(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
