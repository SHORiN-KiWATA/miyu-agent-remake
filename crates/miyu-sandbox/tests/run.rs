//! 真跑助手（`docs/blueprint/sandbox.md`「助手的命令行」）：经它跑的命令，输出、退出码、工作目录、环境变量都和直接跑
//! 一样，它自己什么都不多印；参数不对、规格写坏了、没给命令、找不到命令、执行不了，各自的退出码和那一句；`probe`
//! 说的是这台机器的平台。

mod support;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use miyu_sandbox::{EXIT_CANNOT_RUN, EXIT_HELPER, EXIT_NOT_FOUND, Network, Platform, Probe, Spec};
use support::{Dir, HELPER};

/// 参数不对时的那一句。
const USAGE: &str = "miyu-sandbox: usage: miyu-sandbox run --spec <json> -- <program> [args...]\n";

/// 一份什么都不限的规格：这里测的是助手照常跑命令，不是收紧。Unix 上整个根目录能读能写（施工 5-2 起 Linux 真的
/// 照规格收紧，空的规格什么都不放行）；Windows 的收紧随 5-9，到时照它的写法放开。
fn spec() -> String {
    Spec {
        read: Vec::new(),
        write: if cfg!(unix) {
            vec![PathBuf::from("/")]
        } else {
            Vec::new()
        },
        readonly: Vec::new(),
        hidden: Vec::new(),
        network: Network::Off,
    }
    .to_json()
    .expect("写得成")
}

/// 跑助手，参数是 `args`。
fn helper(args: &[&str]) -> Output {
    Command::new(HELPER)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

/// 一条命令：输出一行、错误一行、工作目录、一个环境变量，退出码 3。
fn sample_command() -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            "cmd".into(),
            vec![
                "/d".into(),
                "/c".into(),
                "echo out& echo err 1>&2& cd& echo %MIYU_SANDBOX_TEST%& exit /b 3".into(),
            ],
        )
    } else {
        (
            "sh".into(),
            vec![
                "-c".into(),
                "echo out; echo err >&2; pwd; echo \"$MIYU_SANDBOX_TEST\"; exit 3".into(),
            ],
        )
    }
}

#[test]
fn a_command_runs_as_if_it_ran_directly() {
    let dir = Dir::new();
    let (program, args) = sample_command();
    let run = |command: &mut Command| {
        command
            .current_dir(dir.path())
            .env("MIYU_SANDBOX_TEST", "passed through")
            .stdin(Stdio::null())
            .output()
            .expect("起得来")
    };
    let direct = run(Command::new(&program).args(&args));
    let spec = spec();
    let mut wrapped: Vec<OsString> = vec!["run".into(), "--spec".into(), spec.into(), "--".into()];
    wrapped.push(program.into());
    wrapped.extend(args.into_iter().map(OsString::from));
    let through = run(Command::new(HELPER).args(&wrapped));
    assert_eq!(direct.status.code(), Some(3), "{direct:?}");
    assert_eq!(through.status.code(), direct.status.code());
    assert_eq!(through.stdout, direct.stdout);
    assert_eq!(through.stderr, direct.stderr, "助手自己什么都不多印");
    let stdout = String::from_utf8_lossy(&through.stdout);
    assert!(stdout.contains("passed through"), "{stdout}");
    let name = dir.path().file_name().expect("有名字").to_string_lossy();
    assert!(stdout.contains(&*name), "工作目录照旧：{stdout}");
}

#[test]
fn a_command_that_succeeds_exits_zero_and_prints_only_its_own() {
    let program = if cfg!(windows) { "cmd" } else { "sh" };
    let script = if cfg!(windows) { "/c" } else { "-c" };
    let out = helper(&["run", "--spec", &spec(), "--", program, script, "echo hi"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end(), "hi");
    assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
fn wrong_arguments_print_the_usage_and_exit_125() {
    let spec = spec();
    for args in [
        vec![],
        vec!["walk"],
        vec!["run"],
        vec!["run", "--spec", &spec],
        vec!["run", "--spec", &spec, "--"],
        vec!["run", "--spec", &spec, "echo", "hi"],
        vec!["run", "--policy", &spec, "--", "echo"],
        vec!["probe", "now"],
    ] {
        let out = helper(&args);
        assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{args:?}");
        assert_eq!(String::from_utf8_lossy(&out.stderr), USAGE, "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn a_bad_spec_says_why_and_exits_125() {
    let out = helper(&["run", "--spec", r#"{"network":"on"}"#, "--", "echo"]);
    assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("miyu-sandbox: bad spec: "), "{stderr}");
    assert!(stderr.contains("on"), "带着原话：{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

#[test]
fn a_missing_program_exits_127() {
    let dir = Dir::new();
    let missing = dir.path().join("no-such-program");
    let missing = missing.to_string_lossy();
    let out = helper(&["run", "--spec", &spec(), "--", &missing]);
    assert_eq!(
        out.status.code(),
        Some(i32::from(EXIT_NOT_FOUND)),
        "{out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with(&format!("miyu-sandbox: cannot run {missing}: ")),
        "{stderr}"
    );
    assert!(out.stdout.is_empty());
}

#[test]
fn a_file_that_is_not_a_program_exits_126() {
    let dir = Dir::new();
    // Unix 上它没有执行权限；Windows 上它不是程序。
    let file = dir.file("not-a-program.exe", b"just text\n");
    let file = file.to_string_lossy();
    let out = helper(&["run", "--spec", &spec(), "--", &file]);
    assert_eq!(
        out.status.code(),
        Some(i32::from(EXIT_CANNOT_RUN)),
        "{out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with(&format!("miyu-sandbox: cannot run {file}: ")),
        "{stderr}"
    );
}

/// Unix 上助手换成了命令（`exec`）：进程号是同一个，命令被信号杀掉的，拉起它的看到的也是信号。
#[cfg(unix)]
#[test]
fn on_unix_the_helper_becomes_the_command() {
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    let mut child = Command::new(HELPER)
        .args(["run", "--spec", &spec(), "--", "sh", "-c", "echo $$"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut printed = String::new();
    child
        .stdout
        .take()
        .expect("接了管道")
        .read_to_string(&mut printed)
        .expect("读得出");
    assert!(child.wait().expect("等得了").success());
    assert_eq!(printed.trim_end(), child.id().to_string(), "同一个进程");
    let out = helper(&["run", "--spec", &spec(), "--", "sh", "-c", "kill -9 $$"]);
    assert_eq!(out.status.signal(), Some(9), "{out:?}");
}

/// 手段各平台报各的，见各平台的测试；这里只看写法。
#[test]
fn the_probe_says_this_platform_in_one_line() {
    let out = helper(&["probe"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let expected = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => "other",
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.ends_with('\n') && stdout.lines().count() == 1,
        "{stdout}"
    );
    let prefix = format!("{{\"version\":1,\"platform\":\"{expected}\",\"mechanisms\":[");
    assert!(stdout.starts_with(&prefix), "{stdout}");
    let probe: Probe = serde_json::from_str(&stdout).expect("读得懂");
    assert_eq!(probe.platform, Platform::current());
    assert_eq!(Platform::current().name(), expected);
}
