//! 命令行的规矩（`docs/designs/22-命令行.md` 第二节，施工 3-9 上）：不认识的子命令就报错、退出码 2，绝不当成
//! 对话发给核心；给人看的话跟着界面语言。

use std::process::{Command, Output};

use crate::support::{Home, MIYU};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;

/// 在临时的数据根上跑 `miyu <args>`，界面语言是 `lang`。
fn miyu(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来")
}

#[test]
fn an_unknown_command_is_refused_and_never_sent() {
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    let said = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        said.trim_end(),
        "没有 hello 这个子命令。想和她对话，用 miyu ask \"…\""
    );
    assert!(output.stdout.is_empty());
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty(), "核心没起来过");

    let output = miyu(&home, "C", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "There is no hello command. To talk to her, use miyu ask \"…\""
    );
}

#[test]
fn plain_miyu_off_a_terminal_prints_the_help() {
    // 不在终端里（这里标准输入、输出都接着管道）：不拉起界面，印帮助、退出码 2（施工 9-3）。
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        page(Language::Chinese, Page::Miyu),
        "{output:?}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
}

#[test]
fn the_help_is_the_page_and_the_version_says_miyu() {
    let home = Home::new();
    // `-h`、`--help`、`help` 印的都是自己写的那一页，一个字节不差（施工 4-11）。
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [&["-h"][..], &["--help"], &["help"]] {
            let output = miyu(&home, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Miyu),
                "{lang} {args:?}"
            );
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
    let output = miyu(&home, "C", &["--version"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("miyu "));
}

#[test]
fn a_mistake_is_one_sentence_and_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str, &str); 4] = [
        (&["ask"], "zh_CN.UTF-8", "少了要说的话：miyu ask \"…\""),
        (
            &["ask", "--format", "xml", "hi"],
            "C",
            "--format must be text or json",
        ),
        // 短写的 `-s`、`-c` 就是 `--session`、`--continue`。
        (
            &["ask", "-s", "x", "-c", "hi"],
            "zh_CN.UTF-8",
            "--session 和 --continue 只能写一个",
        ),
        (&["undo", "-s"], "zh_CN.UTF-8", "--session 后面少了值"),
    ];
    for (args, lang, said) in cases {
        let output = miyu(&home, lang, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{said}\n"),
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

/// 数据根里有什么：每个文件、目录的相对路径，排好序。
#[cfg(not(windows))]
fn listing(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("读得了目录") {
            let path = entry.expect("读得了").path();
            if path.is_dir() {
                pending.push(path.clone());
            }
            found.push(path.strip_prefix(root).expect("在数据根里").to_path_buf());
        }
    }
    found.sort();
    found
}

/// 别的平台上不用装：`setup`、`remove` 说一句、退出 0，数据根一样东西都没多（施工 5-8）。Windows 上这两条会真装，
/// 在虚拟机上验（`docs/blueprint/sandbox/windows.md`「守着它的」）。
#[cfg(not(windows))]
#[test]
fn elsewhere_the_sandbox_needs_no_setup() {
    let home = Home::new();
    let before = listing(home.root.path());
    for action in ["setup", "remove"] {
        for (lang, said) in [
            ("zh_CN.UTF-8", "这个平台不用装沙盒。"),
            ("C", "Nothing to set up on this platform."),
        ] {
            let output = miyu(&home, lang, &["sandbox", action]);
            assert_eq!(output.status.code(), Some(0), "{action}：{output:?}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), format!("{said}\n"));
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
    assert_eq!(listing(home.root.path()), before, "数据根一样东西都没多");
}

#[test]
fn a_sandbox_mistake_is_one_sentence_and_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str, &str); 3] = [
        (
            &["sandbox"],
            "zh_CN.UTF-8",
            "miyu sandbox 后面要写：setup 或 remove",
        ),
        (
            &["sandbox", "frob"],
            "C",
            "miyu sandbox has no frob command",
        ),
        (
            &["sandbox", "setup", "--owner-sid", "S-1-5-21-1-2"],
            "C",
            "Missing --owner-home",
        ),
    ];
    for (args, lang, said) in cases {
        let output = miyu(&home, lang, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{said}\n"),
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

#[test]
fn the_sandbox_help_is_its_page() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["sandbox", "-h"][..],
            &["sandbox", "--help"],
            &["sandbox", "setup", "-h"],
            &["sandbox", "remove", "--help"],
            &["help", "sandbox"],
        ] {
            let output = miyu(&home, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Sandbox),
                "{lang} {args:?}"
            );
        }
    }
}

#[test]
fn the_program_calls_itself_miyu_whatever_its_file_is_called() {
    // Windows 上可执行文件叫 `miyu.exe`，clap 默认照文件名说话，会说成「miyu.exe sandbox …」（施工 5-8 在 CI 上查出来的）。
    // 换个名字跑，这台机器上也照得出来。换名字用硬链接，放在主程序旁边（test-ext 补）：拷的时候开着写的句柄，别的测试这时
    // 拉起的子进程在 exec 以前也开着它，接着跑刚拷好的这个，Linux 回 `ETXTBSY`。临时目录可能和主程序不在一个文件系统上
    // （tmpfs），连不成，所以放主程序旁边；连不成的才拷。
    let home = Home::new();
    let renamed = Renamed(std::path::Path::new(MIYU).with_file_name(format!(
        "renamed-miyu-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    )));
    let renamed = &renamed.0;
    if std::fs::hard_link(MIYU, renamed).is_err() {
        std::fs::copy(MIYU, renamed).expect("拷得了主程序");
    }
    let output = Command::new(renamed)
        .arg("sandbox")
        .env("MIYU_HOME", home.root.path())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "miyu sandbox needs one of: setup or remove\n"
    );
}

/// 换了名字的主程序：用完删掉。
struct Renamed(std::path::PathBuf);

impl Drop for Renamed {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.0));
    }
}
