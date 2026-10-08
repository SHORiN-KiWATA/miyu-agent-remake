//! 门禁程序，和开发用的几样小工具（`cargo xtask dev-home`：造一个带配置的数据根，施工 8-6）。`cargo xtask check` 依次跑格式、clippy、文档、三道门禁、许可证和测试，最后打一张结果表。
//!
//! 三道门禁都照图纸查：`docs/designs/01-架构.md` 第九节「代码的分层」。「文档」那一项还查登记簿：
//! 给模型看的字和 `docs/designs/26-提示词.md` 第十节一一对上（施工 3-5 再补）；「给模型看的字」那一页和资源、登记簿
//! 对得上，蓝图里标着「样本」的块和它指到的文件一字不差（施工 4-9 三补）。`cargo xtask prompts` 重新生成那一页。
//! 「许可证」一项查发布的四个平台上用得到的第三方依赖，许可证都能和 GPL-3.0-or-later 合在一起发（施工 4-12）。
//! 里面的 cargo 一个接一个跑，不并行。

mod dev_home;
mod drawing;
mod files;
mod layers;
mod ledger;
mod licenses;
mod prompts;
mod purity;
mod samples;
mod size;
mod test_targets;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use drawing::Drawing;
use layers::Package;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("check") => check(),
        Some("dev-home") => dev_home::run(std::env::args().nth(2).as_deref()),
        Some("prompts") => match prompts::write(&root()) {
            Ok(()) => {
                println!("写好了 {}", prompts::PATH);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "用法：cargo xtask check | cargo xtask prompts | cargo xtask dev-home <目录>"
            );
            ExitCode::from(2)
        }
    }
}

/// 一项检查的结果。`problems` 是空的就算通过。
struct Outcome {
    name: &'static str,
    problems: Vec<String>,
}

/// 仓库的根目录。
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 在仓库根目录下的 xtask/ 里，一定有上一级目录")
        .to_path_buf()
}

fn check() -> ExitCode {
    let root = root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut outcomes = vec![
        cargo_step(&cargo, &root, "格式", &["fmt", "--all", "--check"], &[]),
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
            &[],
        ),
        // 注释里的链接断了、写法坏了，生成文档时才看得出来，所以警告也当错。
        // 私有的也要生成：只生成公开的，私有代码注释里的断链查不出来。
        cargo_step(
            &cargo,
            &root,
            "文档",
            &[
                "doc",
                "--workspace",
                "--no-deps",
                "--document-private-items",
                "--quiet",
            ],
            &[("RUSTDOCFLAGS", "-D warnings")],
        ),
    ];
    println!("\n── 文档：登记簿 {} 第十节 ──", ledger::PATH);
    println!(
        "── 文档：{} 和资源、登记簿；蓝图里的样本块 ──",
        prompts::PATH
    );
    if let Some(docs) = outcomes.last_mut() {
        docs.problems.extend(ledger::check(&root));
        docs.problems.extend(prompts::check(&root));
        docs.problems.extend(samples::check(&root));
    }
    outcomes.extend(gates(&cargo, &root));
    println!(
        "\n── 许可证：cargo metadata，{} ──",
        licenses::PLATFORMS.join("、")
    );
    outcomes.push(Outcome {
        name: "许可证",
        problems: licenses::check(&root, &cargo),
    });
    let mut tests = cargo_step(
        &cargo,
        &root,
        "测试",
        &["test", "--workspace", "--quiet"],
        &[],
    );
    println!("\n── 测试：每个测试文件都进了测试程序（autotests = false 的 crate）──");
    tests.problems.extend(test_crates(&cargo, &root));
    outcomes.push(tests);
    report(&outcomes)
}

/// 跑一条 cargo 命令，输出照常打在终端上。`env` 是只加给这一条命令的环境变量，
/// 标题里照样打出来，方便照着手动重跑。
fn cargo_step(
    cargo: &str,
    root: &Path,
    name: &'static str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Outcome {
    let shown: String = env.iter().map(|(k, v)| format!("{k}=\"{v}\" ")).collect();
    println!("\n── {name}：{shown}cargo {} ──", args.join(" "));
    let problems = match Command::new(cargo)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(root)
        .status()
    {
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

/// 查每个 crate 的测试文件都进了测试程序（`test_targets`）；读不了 cargo metadata 的说一句。
fn test_crates(cargo: &str, root: &Path) -> Vec<String> {
    match layers::read_packages(root, cargo) {
        Ok(packages) => {
            let dirs: Vec<&Path> = packages
                .iter()
                .map(|package| package.dir.as_path())
                .collect();
            test_targets::check(root, &dirs)
        }
        Err(e) => vec![e],
    }
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
