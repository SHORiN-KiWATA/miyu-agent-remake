//! `cargo xtask package-web`（施工 W-11，`web-ui.md`「怎么走」第四条）：照包管理器装好的样子摆出网页软件，摆完做发布前检查
//! （设计 12 R14）。
//!
//! - `cargo xtask package-web --out <目录> [--pages <页面目录>]`：发行版编译 `miyu-web`，摆成
//!   `<目录>/miyu-web-<版本>-<系统>-<架构>/`：`bin/miyu-web`、`share/miyu/web/web.json`、`share/miyu/web/pages/`。
//!   页面不写是仓库的 `resources/web/pages/`。只摆目录，不压缩：压成什么、怎么签名随主程序的发行定。
//! - `cargo xtask package-web --check <摆好的目录>`：只查。
//!
//! 摆好的目录解开就能跑：程序照「上一级的 `share/miyu/`」找到资源目录；和主程序装在一处时 `bin/`、`share/miyu/` 合在一起。

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// 程序的文件名（Windows 上带 `.exe`）。
pub(crate) const PROGRAM: &str = if cfg!(windows) {
    "miyu-web.exe"
} else {
    "miyu-web"
};

/// 跑 `package-web`。
pub(crate) fn run(root: &Path, args: &[String]) -> ExitCode {
    let version = env!("CARGO_PKG_VERSION");
    let dir = match args {
        [flag, dir] if flag == "--check" => PathBuf::from(dir),
        [flag, out] if flag == "--out" => {
            match build_and_lay_out(root, Path::new(out), None, version) {
                Ok(dir) => dir,
                Err(error) => return fail(&error),
            }
        }
        [flag, out, pages_flag, pages] if flag == "--out" && pages_flag == "--pages" => {
            match build_and_lay_out(root, Path::new(out), Some(Path::new(pages)), version) {
                Ok(dir) => dir,
                Err(error) => return fail(&error),
            }
        }
        _ => {
            eprintln!(
                "用法：cargo xtask package-web --out <目录> [--pages <页面目录>] | cargo xtask package-web --check <摆好的目录>"
            );
            return ExitCode::from(2);
        }
    };
    let problems = check(&dir, version, say_version);
    if problems.is_empty() {
        println!(
            "{} 查过了：程序、web.json、页面都在，版本 {version}",
            dir.display()
        );
        return ExitCode::SUCCESS;
    }
    for problem in &problems {
        eprintln!("{problem}");
    }
    ExitCode::FAILURE
}

fn fail(error: &str) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

/// 发行版编译 `miyu-web`，再摆出来。
fn build_and_lay_out(
    root: &Path,
    out: &Path,
    pages: Option<&Path>,
    version: &str,
) -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let built = Command::new(&cargo)
        .args(["build", "--release", "-p", "miyu-web"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("{cargo} 跑不起来：{error}"))?;
    if !built.success() {
        return Err(format!("cargo build --release -p miyu-web 没过（{built}）"));
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| root.join("target"), PathBuf::from);
    let resources = root.join("resources");
    let pages = pages.map_or_else(|| resources.join("web").join("pages"), Path::to_path_buf);
    lay_out(
        &target.join("release").join(PROGRAM),
        &resources,
        &pages,
        out,
        version,
    )
}

/// 摆出来：程序 `program`、资源目录 `resources` 里的 `web/web.json`、页面目录 `pages`，摆进 `out` 下面一个照版本和平台
/// 起名的目录（有旧的先整个删掉）。交回那个目录。
pub(crate) fn lay_out(
    program: &Path,
    resources: &Path,
    pages: &Path,
    out: &Path,
    version: &str,
) -> Result<PathBuf, String> {
    if !pages.join("index.html").is_file() {
        return Err(format!(
            "{} 里没有 index.html：页面还没进主仓库的，用 --pages 指到网页演示的页面目录",
            pages.display()
        ));
    }
    let dir = out.join(format!(
        "miyu-web-{version}-{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| format!("删不掉旧的 {}：{error}", dir.display()))?;
    }
    let web = dir.join("share").join("miyu").join("web");
    copy(program, &dir.join("bin").join(PROGRAM))?;
    copy(
        &resources.join("web").join("web.json"),
        &web.join("web.json"),
    )?;
    copy_tree(pages, &web.join("pages"))?;
    Ok(dir)
}

/// 拷一份文件，缺的上级目录先建。
fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("建不了 {}：{error}", parent.display()))?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{} 拷不过去：{error}", from.display()))
}

/// 整个目录拷过去：普通文件和下一层的目录，链接照它指的内容拷。
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(from).map_err(|error| format!("读不了 {}：{error}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读不了 {}：{error}", from.display()))?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else {
            copy(&path, &target)?;
        }
    }
    Ok(())
}

/// 发布前检查（设计 12 R14）：程序在、报的版本对；`web.json` 读得懂、认得 `html`；`index.html` 在。交回每一处不对的。
/// `say_version` 跑程序的 `--version`，交回它印的。
pub(crate) fn check(
    dir: &Path,
    version: &str,
    say_version: impl Fn(&Path) -> Result<String, String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let program = dir.join("bin").join(PROGRAM);
    if program.is_file() {
        let want = format!("miyu-web {version}");
        match say_version(&program) {
            Ok(said) if said.trim() == want => {}
            Ok(said) => problems.push(format!(
                "bin/{PROGRAM} 报的版本是「{}」，应该是「{want}」",
                said.trim()
            )),
            Err(error) => problems.push(format!("bin/{PROGRAM} 跑不起来：{error}")),
        }
    } else {
        problems.push(format!("少了 bin/{PROGRAM}"));
    }
    match miyu_web::settings::Settings::load(&dir.join("share").join("miyu")) {
        Ok(settings) if settings.types.contains_key("html") => {}
        Ok(_) => {
            problems.push("share/miyu/web/web.json 的 types 里没有 html：页面给不出去".to_string())
        }
        Err(error) => problems.push(format!("share/miyu/web/web.json 读不懂：{error}")),
    }
    if !dir.join("share/miyu/web/pages/index.html").is_file() {
        problems.push("少了 share/miyu/web/pages/index.html".to_string());
    }
    problems
}

/// 跑 `<程序> --version`，交回它印在标准输出上的。
fn say_version(program: &Path) -> Result<String, String> {
    let output = Command::new(program)
        .arg("--version")
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("退出码 {}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}
