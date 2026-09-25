//! 门禁程序。`cargo xtask check` 依次跑格式、clippy、三道门禁和测试，最后打一张结果表。
//!
//! 三道门禁都照图纸查：`docs/designs/01-架构.md` 第九节「代码的分层」。
//! 里面的 cargo 一个接一个跑，不并行。

mod drawing;
mod files;
mod layers;
mod purity;
mod size;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use drawing::Drawing;
use layers::Package;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("check") => check(),
        _ => {
            eprintln!("用法：cargo xtask check");
            ExitCode::from(2)
        }
    }
}

/// 一项检查的结果。`problems` 是空的就算通过。
struct Outcome {
    name: &'static str,
    problems: Vec<String>,
}

fn check() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 在仓库根目录下的 xtask/ 里，一定有上一级目录")
        .to_path_buf();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut outcomes = vec![
        cargo_step(&cargo, &root, "格式", &["fmt", "--all", "--check"]),
        cargo_step(
            &cargo,
            &root,
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--quiet",
                "--",
                "-D",
                "warnings",
            ],
        ),
    ];
    outcomes.extend(gates(&cargo, &root));
    outcomes.push(cargo_step(
        &cargo,
        &root,
        "测试",
        &["test", "--workspace", "--quiet"],
    ));
    report(&outcomes)
}

/// 跑一条 cargo 命令，输出照常打在终端上。
fn cargo_step(cargo: &str, root: &Path, name: &'static str, args: &[&str]) -> Outcome {
    println!("\n── {name}：cargo {} ──", args.join(" "));
    let problems = match Command::new(cargo).args(args).current_dir(root).status() {
        Ok(status) if status.success() => Vec::new(),
        Ok(status) => vec![format!(
            "cargo {} 没通过（{status}），原因见上面的输出",
            args[0]
        )],
        Err(e) => vec![format!("跑不了 cargo {}：{e}", args[0])],
    };
    Outcome { name, problems }
}

/// 三道照图纸查的门禁。
fn gates(cargo: &str, root: &Path) -> Vec<Outcome> {
    let (drawing, packages) = match load(cargo, root) {
        Ok(loaded) => loaded,
        Err(e) => {
            return ["分层", "纯逻辑", "行数"]
                .into_iter()
                .map(|name| Outcome {
                    name,
                    problems: vec![e.clone()],
                })
                .collect();
        }
    };

    let mut purity = Vec::new();
    let mut size = Vec::new();
    for package in &packages {
        let pure = drawing.layer_of(&package.name).is_some_and(|l| l.pure);
        for (label, text) in sources(root, &package.dir, &mut size) {
            if pure && Path::new(&label).starts_with(src_of(root, package)) {
                purity.extend(purity::scan(&label, &text));
            }
            size.extend(size::check(&label, &text, drawing.max_lines));
        }
    }
    vec![
        Outcome {
            name: "分层",
            problems: layers::check(&drawing, &packages),
        },
        Outcome {
            name: "纯逻辑",
            problems: purity,
        },
        Outcome {
            name: "行数",
            problems: size,
        },
    ]
}

fn load(cargo: &str, root: &Path) -> Result<(Drawing, Vec<Package>), String> {
    let text = std::fs::read_to_string(root.join(drawing::PATH))
        .map_err(|e| format!("读不了图纸 {}：{e}", drawing::PATH))?;
    let drawing =
        drawing::parse(&text).map_err(|e| format!("图纸 {} 读不出来：{e}", drawing::PATH))?;
    let packages = layers::read_packages(root, cargo)?;
    Ok((drawing, packages))
}

/// 一个 crate 的 `src/`，相对仓库根。
fn src_of(root: &Path, package: &Package) -> PathBuf {
    relative(root, &package.dir.join("src"))
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// `dir` 下面每个 `.rs` 文件的相对路径和内容。读不了的记进 `problems`。
fn sources(root: &Path, dir: &Path, problems: &mut Vec<String>) -> Vec<(String, String)> {
    let files = match files::rust_files(dir) {
        Ok(files) => files,
        Err(e) => {
            problems.push(e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for path in files {
        let label = relative(root, &path).display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => out.push((label, text)),
            Err(e) => problems.push(format!("读不了 {label}：{e}")),
        }
    }
    out
}

fn report(outcomes: &[Outcome]) -> ExitCode {
    println!("\n── 门禁结果 ──");
    for outcome in outcomes {
        if outcome.problems.is_empty() {
            println!("  ✓ {}", outcome.name);
        } else {
            println!("  ✗ {}：{} 处", outcome.name, outcome.problems.len());
            for problem in &outcome.problems {
                println!("      {problem}");
            }
        }
    }
    if outcomes.iter().all(|o| o.problems.is_empty()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
