//! 剪贴板里的截图暂存在哪（蓝图 `tui.md`「输入框」第 12 条）：机器缓存目录的 `tui/pasted/<进程号>/`，不放数据根
//! （数据根里的文件核心不给附）。界面退出时删掉自己那个目录；启动时删掉已经不在跑的界面留下的（崩了留的）。

use std::fs;
use std::path::{Path, PathBuf};

/// 暂存截图的那一层在缓存目录下的哪儿：`tui/pasted`。
const ROOT: [&str; 2] = ["tui", "pasted"];

/// 暂存截图的那一层：`<缓存目录>/tui/pasted`。
pub fn root(cache: &Path) -> PathBuf {
    ROOT.iter()
        .fold(cache.to_path_buf(), |dir, part| dir.join(part))
}

/// 这个文件是剪贴板贴进来暂存的（`<缓存目录>/tui/pasted/<进程号>/<文件>`）：不是人本机原来的文件。
pub fn is_staged(file: &Path) -> bool {
    let Some(layer) = file.parent().and_then(Path::parent) else {
        return false;
    };
    let tail: Vec<_> = layer.components().rev().take(ROOT.len()).collect();
    tail.iter()
        .rev()
        .map(|c| c.as_os_str())
        .eq(ROOT.iter().map(std::ffi::OsStr::new))
}

/// 这个界面暂存截图的目录：用到时才建，放下时删掉。
#[derive(Debug)]
pub struct Staging {
    dir: PathBuf,
}

impl Staging {
    /// 在 `root`（`tui/pasted`）下定这个界面的目录，顺手清掉不在跑的界面留下的。
    pub fn new(root: &Path) -> Self {
        let me = std::process::id();
        sweep(root, me, alive);
        Self {
            dir: root.join(me.to_string()),
        }
    }

    /// 暂存的目录。
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        // 删不掉（已经没了、没权限）就留着：下次启动清。
        fs::remove_dir_all(&self.dir).unwrap_or_default();
    }
}

/// 清掉 `root` 下的残留：名字是进程号、那个进程已经不在跑的目录；不是目录的（早先的暂存直接放在这一层）。
/// 自己的、还在跑的、名字不是进程号的目录不碰。
fn sweep(root: &Path, me: u32, alive: impl Fn(u32) -> bool) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            fs::remove_file(&path).unwrap_or_default();
            continue;
        }
        let pid = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok());
        if pid.is_some_and(|pid| pid != me && !alive(pid)) {
            fs::remove_dir_all(&path).unwrap_or_default();
        }
    }
}

/// 这个进程还在不在：发一个空信号试试，没权限的也算在。
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    use rustix::process::{Pid, test_kill_process};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    match test_kill_process(pid) {
        Ok(()) => true,
        Err(error) => error == rustix::io::Errno::PERM,
    }
}

/// 别的系统判不了：都当还在，不清。
#[cfg(not(unix))]
fn alive(_: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{Staging, sweep};

    #[test]
    fn leftovers_of_gone_views_are_swept_and_our_own_goes_on_drop() {
        let root = std::env::temp_dir().join(format!("miyu-staging-{}", std::process::id()));
        for dir in ["100", "200", "我的", "300"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join("a.png"), b"x").unwrap();
        }
        std::fs::write(root.join("0123abcd.png"), b"x").unwrap();
        // 100 不在跑了，200 还在，300 是自己。
        sweep(&root, 300, |pid| pid == 200);
        let left = |name: &str| root.join(name).exists();
        assert!(!left("100"), "不在跑的清掉");
        assert!(
            left("200") && left("300") && left("我的"),
            "在跑的、自己的、不认得的不碰"
        );
        assert!(!left("0123abcd.png"), "早先直接放在这一层的清掉");
        let staging = Staging::new(&root);
        std::fs::create_dir_all(staging.dir()).unwrap();
        let mine = staging.dir().to_path_buf();
        assert_eq!(mine, root.join(std::process::id().to_string()));
        drop(staging);
        assert!(!mine.exists(), "放下时删掉自己的");
        std::fs::remove_dir_all(&root).unwrap_or_default();
    }
}
