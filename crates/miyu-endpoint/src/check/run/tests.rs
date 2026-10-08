//! 跑包自己的检查（施工 9-2）：输出的每一行怎么收、跑的结果怎么说；真的跑一个程序（Unix 上的 `sh`）。

use super::*;

fn words() -> Human {
    Human::default()
}

#[test]
fn a_line_is_a_problem_only_with_the_fields_check_knows() {
    let full = r#"{"kind":"venue","file":"system/venues.d/x.toml","line":3,"column":2,"level":"error","message":"m","code":"c","key":"k","rule":"r","source":"s","extra":1}"#;
    assert_eq!(
        problem(full),
        Some(
            json!({"kind":"venue","file":"system/venues.d/x.toml","line":3,"column":2,"level":"error","message":"m","code":"c","key":"k","rule":"r","source":"s"})
        ),
        "别的格不收"
    );
    let least = r#"{"kind":"k","file":"f","level":"warning","message":"m"}"#;
    assert_eq!(
        problem(least),
        Some(json!({"kind":"k","file":"f","level":"warning","message":"m"}))
    );
    for bad in [
        "not json",
        "[1]",
        r#"{"kind":"k","file":"f","level":"fatal","message":"m"}"#,
        r#"{"kind":"k","file":"f","message":"m"}"#,
        r#"{"kind":1,"file":"f","level":"error","message":"m"}"#,
        r#"{"kind":"k","file":"f","level":"error","message":"m","line":0}"#,
        r#"{"kind":"k","file":"f","level":"error","message":"m","line":"3"}"#,
    ] {
        assert_eq!(problem(bad), None, "{bad}");
    }
}

#[test]
fn exits_other_than_zero_and_one_and_unreadable_lines_are_reported() {
    let line = r#"{"kind":"k","file":"f","level":"error","message":"m"}"#;
    let ok = read(
        &words(),
        "p.toml",
        Ran::Exited(Some(1), format!("{line}\n\n")),
    );
    assert_eq!(ok, [problem(line).unwrap()]);
    let mixed = read(
        &words(),
        "p.toml",
        Ran::Exited(Some(0), format!("{line}\nhello\n")),
    );
    assert_eq!(mixed.len(), 2, "一行看不懂的也报");
    assert_eq!(mixed[1]["code"], "check_output");
    assert_eq!(mixed[1]["message"], "1", "没有给人看的字时照原话：几行");
    assert_eq!(mixed[1]["level"], "warning");
    assert_eq!(mixed[1]["file"], "p.toml");
    for (ran, detail) in [
        (Ran::Exited(Some(3), line.to_string()), "exit code 3"),
        (Ran::Exited(None, String::new()), "killed by a signal"),
        (Ran::TimedOut, "timed out"),
        (Ran::Failed("nope".to_string()), "nope"),
    ] {
        let said = read(&words(), "p.toml", ran);
        assert_eq!(said.len(), 1, "跑坏了的不收它的输出");
        assert_eq!(said[0]["code"], "check_failed");
        assert_eq!(said[0]["message"], detail, "没有给人看的字时照原话");
    }
}

#[cfg(unix)]
#[test]
fn a_real_program_runs_and_a_slow_one_is_stopped() {
    let sh = Path::new("/bin/sh");
    let ran = run(
        sh,
        &["-c".to_string(), "echo one; echo two; exit 1".to_string()],
        LIMIT,
    );
    assert_eq!(ran, Ran::Exited(Some(1), "one\ntwo\n".to_string()));
    let slow = run(
        sh,
        &["-c".to_string(), "sleep 5".to_string()],
        Duration::from_millis(200),
    );
    assert_eq!(slow, Ran::TimedOut);
    let missing = run(Path::new("/no/such/program"), &[], LIMIT);
    assert!(matches!(missing, Ran::Failed(_)));
}
