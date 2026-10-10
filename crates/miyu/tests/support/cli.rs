//! 测试里跑 `miyu <args>` 命令行（施工 8-3 补从 `config.rs` 挪来，`config.rs`、`config_keys.rs` 共用）：数据根、工作目录、
//! 界面语言照传进来的；没有 key，去掉 `LC_ALL`、`LC_MESSAGES`、`XDG_RUNTIME_DIR`、`NO_COLOR`，不受开发环境影响。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::MIYU;

/// 在数据根 `root` 上、工作目录 `cwd` 里跑 `miyu <args>`：没有 key，界面语言是 `lang`。
pub fn miyu(root: &Path, cwd: &Path, lang: &str, args: &[&str]) -> Output {
    command(root, cwd, lang, args).output().expect("跑得起来")
}

/// 同 [`miyu`] 的那一条命令，还没跑：要往标准输入写字的照它自己接（施工 F-8 下补）。
pub fn command(root: &Path, cwd: &Path, lang: &str, args: &[&str]) -> Command {
    let mut command = Command::new(MIYU);
    command
        .args(args)
        .current_dir(cwd)
        .env("MIYU_HOME", root)
        .envs(super::offline(root))
        .env("MIYU_RESOURCES", super::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("NO_COLOR");
    command
}

/// 在阻塞线程里跑：核心在这个测试的运行时里。
pub async fn run(root: &Path, cwd: &Path, lang: &str, args: &[&str]) -> Output {
    let (root, cwd, lang): (PathBuf, PathBuf, String) =
        (root.to_path_buf(), cwd.to_path_buf(), lang.to_string());
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    tokio::task::spawn_blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        miyu(&root, &cwd, &lang, &args)
    })
    .await
    .expect("没 panic")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
