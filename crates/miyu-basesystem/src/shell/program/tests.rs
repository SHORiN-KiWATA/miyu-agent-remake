use super::*;

/// 测试用的临时目录，里面放几个假的程序。
struct Bin(PathBuf);

impl Bin {
    fn with(names: &[&str]) -> Bin {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-shell-bin-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        for name in names {
            std::fs::write(dir.join(name), b"").expect("写得进");
        }
        Bin(dir)
    }

    fn path(&self) -> OsString {
        std::env::join_paths([Path::new("/nowhere"), &self.0]).expect("拼得起来")
    }
}

impl Drop for Bin {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn each_system_gets_its_own_shell() {
    let bin = Bin::with(&["bash", "pwsh.exe"]);
    let path = bin.path();
    assert_eq!(
        choose("linux", Some(&path), None),
        Program {
            kind: Kind::Bash,
            path: bin.0.join("bash"),
        },
        "照 PATH 找到的"
    );
    assert_eq!(
        choose("linux", None, None).path,
        PathBuf::from("/bin/bash"),
        "找不到用 /bin/bash"
    );
    assert_eq!(
        choose("macos", Some(&path), None),
        Program {
            kind: Kind::Zsh,
            path: PathBuf::from("/bin/zsh"),
        }
    );
    assert_eq!(
        choose("windows", Some(&path), None),
        Program {
            kind: Kind::PowerShell7,
            path: bin.0.join("pwsh.exe"),
        }
    );
    let bare = Bin::with(&[]);
    let root = OsString::from("/win");
    assert_eq!(
        choose("windows", Some(&bare.path()), Some(&root)),
        Program {
            kind: Kind::WindowsPowerShell,
            path: Path::new("/win/System32/WindowsPowerShell/v1.0/powershell.exe").to_path_buf(),
        },
        "没装 PowerShell 7 的用系统自带的"
    );
    assert_eq!(
        choose("windows", None, None).path,
        PathBuf::from("powershell.exe")
    );
}

#[test]
fn the_names_tell_the_editions_apart() {
    assert_eq!(Kind::Bash.name(), "bash");
    assert_eq!(Kind::Zsh.name(), "zsh");
    assert_eq!(Kind::PowerShell7.name(), "PowerShell 7");
    assert_eq!(Kind::WindowsPowerShell.name(), "Windows PowerShell 5.1");
}

/// 一条命令的参数，写成字。
fn args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn startup_files_are_skipped_and_only_the_given_variables_pass() {
    let env = vec![(OsString::from("PATH"), OsString::from("/usr/bin"))];
    let bash = Program {
        kind: Kind::Bash,
        path: PathBuf::from("/bin/bash"),
    }
    .command("echo hi", Path::new("/work"), env.clone(), None)
    .expect("造得出");
    assert_eq!(args(&bash), ["--noprofile", "--norc", "-c", "echo hi"]);
    assert_eq!(bash.get_current_dir(), Some(Path::new("/work")));
    let envs: Vec<_> = bash.get_envs().collect();
    assert_eq!(
        envs,
        [(OsStr::new("PATH"), Some(OsStr::new("/usr/bin")))],
        "只有给的这些"
    );
    let zsh = Program {
        kind: Kind::Zsh,
        path: PathBuf::from("/bin/zsh"),
    }
    .command("echo hi", Path::new("/work"), env, None)
    .expect("造得出");
    assert_eq!(args(&zsh), ["-f", "+o", "nomatch", "-c", "echo hi"]);
}

#[test]
fn powershell_gets_the_command_encoded_after_its_prelude() {
    let command = Program {
        kind: Kind::PowerShell7,
        path: PathBuf::from("pwsh.exe"),
    }
    .command("dir", Path::new("/work"), Vec::new(), None)
    .expect("造得出");
    let args = args(&command);
    assert_eq!(
        args[..4],
        [
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand"
        ]
    );
    assert_eq!(args[4], encoded(&format!("{PRELUDE}dir")));
    // 「dir」的 UTF-16LE 是 64 00 69 00 72 00。
    assert_eq!(encoded("dir"), "ZABpAHIA");
    // 中文也照 UTF-16LE：「中」是 2d 4e。
    assert_eq!(encoded("中"), "LU4=");
}

/// zsh 没匹配到的通配符照原样传下去，和 bash 一样（施工 4-9 再补二）。有 zsh 的机器上真跑一次：macOS 总有，
/// 别的系统没装就不跑。
#[cfg(unix)]
#[test]
fn zsh_passes_an_unmatched_glob_through() {
    let Some(path) = ["/bin/zsh", "/usr/bin/zsh"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
    else {
        return;
    };
    let zsh = Program {
        kind: Kind::Zsh,
        path,
    };
    let output = zsh
        .command("echo *.nothing-here", Path::new("/"), Vec::new(), None)
        .expect("造得出")
        .output()
        .expect("跑得起来");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "*.nothing-here\n");
}

/// 一份什么都不限的规格，助手在 `/opt/miyu/miyu-sandbox`。
fn sandboxed() -> Sandboxed {
    Sandboxed {
        helper: PathBuf::from("/opt/miyu/miyu-sandbox"),
        spec: miyu_sandbox::Spec::from_json(r#"{"write":["/work"]}"#).expect("读得懂"),
        env: vec![(OsString::from("TMPDIR"), OsString::from("/work/tmp"))],
    }
}

#[test]
fn a_sandboxed_command_goes_through_the_helper_and_keeps_the_rest() {
    let sandboxed = sandboxed();
    let env = vec![(OsString::from("PATH"), OsString::from("/usr/bin"))];
    for kind in [
        Kind::Bash,
        Kind::Zsh,
        Kind::PowerShell7,
        Kind::WindowsPowerShell,
    ] {
        let program = Program {
            kind,
            path: PathBuf::from("/bin/shell"),
        };
        let direct = program
            .command("echo hi", Path::new("/work"), env.clone(), None)
            .expect("造得出");
        let wrapped = program
            .command("echo hi", Path::new("/work"), env.clone(), Some(&sandboxed))
            .expect("造得出");
        assert_eq!(wrapped.get_program(), OsStr::new("/opt/miyu/miyu-sandbox"));
        let json = sandboxed.spec.to_json().expect("写得成");
        let mut expected = vec![
            "run".to_string(),
            "--spec".to_string(),
            json,
            "--".to_string(),
            "/bin/shell".to_string(),
        ];
        expected.extend(args(&direct));
        assert_eq!(args(&wrapped), expected, "{kind:?}");
        assert_eq!(wrapped.get_current_dir(), direct.get_current_dir());
        let mut expected_envs: Vec<_> = direct.get_envs().collect();
        expected_envs.push((OsStr::new("TMPDIR"), Some(OsStr::new("/work/tmp"))));
        expected_envs.sort();
        let mut envs: Vec<_> = wrapped.get_envs().collect();
        envs.sort();
        assert_eq!(envs, expected_envs, "白名单照旧，沙盒要设的加上");
    }
}

/// 规格里有不是 UTF-8 的路径，写不成 JSON：起不来，不会不经沙盒就跑。
#[cfg(unix)]
#[test]
fn a_spec_that_cannot_be_written_stops_the_command() {
    use std::os::unix::ffi::OsStringExt;
    let mut sandboxed = sandboxed();
    sandboxed
        .spec
        .write
        .push(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])));
    let program = Program {
        kind: Kind::Bash,
        path: PathBuf::from("/bin/bash"),
    };
    assert!(
        program
            .command("echo hi", Path::new("/work"), Vec::new(), Some(&sandboxed))
            .is_err()
    );
}
