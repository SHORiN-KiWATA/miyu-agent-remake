use super::*;

/// 拼一份环境。
fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

/// 挑出来的名字，照先后。
fn names(passed: &[(OsString, OsString)]) -> Vec<String> {
    passed
        .iter()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn only_the_listed_names_pass_and_git_never_asks() {
    let passed = passed_as(
        vars(&[
            ("PATH", "/usr/bin"),
            ("DEEPSEEK_API_KEY", "secret"),
            ("AWS_ACCESS_KEY_ID", "secret"),
            ("HOME", "/home/a"),
            ("LC_ALL", "C.UTF-8"),
            ("SSH_AUTH_SOCK", "/run/agent"),
            ("CARGO_HOME", "/home/a/.cargo"),
        ]),
        false,
    );
    assert_eq!(
        names(&passed),
        [
            "PATH",
            "HOME",
            "LC_ALL",
            "CARGO_HOME",
            "GIT_TERMINAL_PROMPT"
        ]
    );
    assert_eq!(passed[4].1, "0");
}

#[test]
fn git_terminal_prompt_is_always_ours() {
    let passed = passed_as(vars(&[("GIT_TERMINAL_PROMPT", "1")]), false);
    assert_eq!(passed, vars(&[("GIT_TERMINAL_PROMPT", "0")]));
}

#[test]
fn windows_names_do_not_care_about_case() {
    let given = vars(&[("Path", "C:\\Windows"), ("windir", "C:\\Windows")]);
    assert_eq!(
        names(&passed_as(given.clone(), true)),
        ["Path", "windir", "GIT_TERMINAL_PROMPT"]
    );
    // 别处分大小写：`Path` 不是 `PATH`。
    assert_eq!(
        names(&passed_as(given, false)),
        ["windir", "GIT_TERMINAL_PROMPT"]
    );
}

#[test]
fn a_prefix_only_counts_at_the_start() {
    assert_eq!(
        names(&passed_as(
            vars(&[("XLC_ALL", "x"), ("LC_CTYPE", "y")]),
            false
        )),
        ["LC_CTYPE", "GIT_TERMINAL_PROMPT"]
    );
}
