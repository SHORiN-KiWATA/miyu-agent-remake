//! `cargo xtask perf [量尺的参数…]`（施工 V-1，`docs/blueprint/perf.md`）：编 release 的 `miyu`、`miyu-sandbox`，
//! 再编量尺 `miyu-perf`、跑它，结果写进 `docs/perf/`。
//!
//! 两次分开编：量的二进制只照它自己的依赖编，别的包（测试夹具打开的开关）混不进去。沙盒放在 `target/perf/` 下：要在
//! 真的磁盘上，同步才算数。后面跟的参数原样交给量尺（`--runs 3` 这些）；带 `--render <原始数据>` 的只编量尺，照现在的预算
//! 重出表，不量。

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// 编、跑。
pub fn run(cargo: &str, root: &Path, extra: &[String]) -> ExitCode {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let release = target.join("release");
    let exe = |name: &str| release.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    // 照原始数据重出表（`--render`）不量，不用编要量的二进制。
    let render = extra.iter().any(|word| word == "--render");
    let builds: &[&[&str]] = if render {
        &[&["miyu-perf"]]
    } else {
        &[&["miyu", "miyu-sandbox"], &["miyu-perf"]]
    };
    for packages in builds {
        let mut args = vec!["build", "--release"];
        for package in packages.iter().copied() {
            args.extend(["-p", package]);
        }
        println!("── cargo {} ──", args.join(" "));
        match Command::new(cargo).args(&args).current_dir(root).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("cargo {} 没通过（{status}）", args.join(" "));
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("跑不了 cargo：{e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let status = Command::new(exe("miyu-perf"))
        .arg("--miyu")
        .arg(exe("miyu"))
        .arg("--resources")
        .arg(root.join("resources"))
        .arg("--design")
        .arg(root.join("docs/designs/23-性能预算.md"))
        .arg("--work")
        .arg(target.join("perf"))
        .arg("--out")
        .arg(root.join("docs/perf"))
        .args(extra)
        .current_dir(root)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("跑不了量尺：{e}");
            ExitCode::FAILURE
        }
    }
}
