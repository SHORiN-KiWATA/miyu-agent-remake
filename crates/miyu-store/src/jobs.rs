//! 会话目录下后台命令的输出（施工 7-3，`docs/blueprint/store.md`「数据根里有什么」）：`jobs/<编号>.out`，一条命令一份，
//! 边跑边写、不截。会话日志只认名字是 12 位数字的段，`jobs/` 不碍着它（「打开、自检」第 1 条）；删会话删整个会话目录，
//! 它跟着一起删。

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::id::JobId;

use crate::durable;

/// 后台命令 `job` 的输出放在会话目录 `session_dir` 下的哪里：`jobs/<编号>.out`，例如 `jobs/j1.out`。
pub fn output_path(session_dir: &Path, job: JobId) -> PathBuf {
    session_dir.join("jobs").join(format!("{job}.out"))
}

/// 给后台命令 `job` 建一份空的输出文件，交回写的一头。没有 `jobs/` 的先建（Unix 上 0700，同步上一层）。已经有的
/// 清空重写：编号在这个会话里不重复，已经有的只会是崩溃前起了、没来得及记下的那一条留下的。
///
/// # Errors
///
/// 建不了目录、建不了文件。
pub fn create_output(session_dir: &Path, job: JobId) -> io::Result<File> {
    let path = output_path(session_dir, job);
    if let Some(dir) = path.parent() {
        durable::create_dir(dir)?;
    }
    File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_lives_under_jobs_in_the_session_dir() {
        let dir = std::env::temp_dir().join(format!("miyu-store-jobs-{}", std::process::id()));
        let job = JobId::new(3).unwrap();
        assert_eq!(output_path(&dir, job), dir.join("jobs").join("j3.out"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        std::fs::write(output_path(&dir, job), b"old").unwrap();
        drop(create_output(&dir, job).unwrap());
        assert_eq!(
            std::fs::read(output_path(&dir, job)).unwrap(),
            b"",
            "已经有的清空"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_jobs_dir_is_only_for_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("miyu-store-jobs-mode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        drop(create_output(&dir, JobId::new(1).unwrap()).unwrap());
        let mode = std::fs::metadata(dir.join("jobs"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
