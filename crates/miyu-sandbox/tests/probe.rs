//! 探测（`docs/blueprint/sandbox.md`「怎么走」第 2 条）：真的助手说的是这台机器；假的助手（Unix 上的脚本）起不来、
//! 到时、退出码不是 0、说的读不懂、版本不认得，各是各的原因。等它有上限：拿着管道不放的也不一直等。

mod support;

use std::path::Path;
use std::time::Duration;

use miyu_sandbox::{Platform, ProbeError, VERSION, probe};
use support::{Dir, HELPER, serial};

/// 平时等多久：够真的助手说完。
const WAIT: Duration = Duration::from_secs(5);

#[test]
fn the_real_helper_says_this_machine() {
    let _serial = serial();
    let probe = probe(Path::new(HELPER), WAIT).expect("探得成");
    assert_eq!(probe.version, VERSION);
    assert_eq!(probe.platform, Platform::current());
}

#[test]
fn a_file_that_is_not_a_program_cannot_run() {
    let _serial = serial();
    let dir = Dir::new();
    let file = dir.file("not-a-program.exe", b"just text\n");
    let error = probe(&file, WAIT).expect_err("起不来");
    assert!(matches!(error, ProbeError::Start(_)), "{error:?}");
    assert!(
        error.to_string().starts_with("cannot run helper: "),
        "{error}"
    );
}

#[cfg(unix)]
mod fake {
    use std::time::Instant;

    use miyu_sandbox::Probe;

    use super::*;

    /// 探一个假助手：脚本的内容是 `body`，最多等 `timeout`。交回结局和用了多久。
    fn fake(body: &str, timeout: Duration) -> (Result<Probe, ProbeError>, Duration) {
        let _serial = serial();
        let dir = Dir::new();
        let helper = dir.script("miyu-sandbox", body);
        let started = Instant::now();
        let result = probe(&helper, timeout);
        (result, started.elapsed())
    }

    #[test]
    fn a_helper_that_says_the_same_line_is_understood() {
        let (result, _) = fake(
            r#"echo '{"version":1,"platform":"linux","mechanisms":["landlock"],"later":true}'"#,
            WAIT,
        );
        let probe = result.expect("读得懂");
        assert_eq!(probe.mechanisms, vec!["landlock".to_string()]);
        assert_eq!(probe.mechanisms_text(), "landlock");
    }

    #[test]
    fn a_helper_that_hangs_is_killed_in_time() {
        let (result, took) = fake("sleep 10", Duration::from_millis(300));
        assert!(matches!(result, Err(ProbeError::TimedOut)), "{result:?}");
        assert!(took < Duration::from_secs(3), "没一直等：{took:?}");
    }

    #[test]
    fn a_helper_that_closes_its_output_but_keeps_running_is_killed_in_time() {
        // 读到了结尾，它却不退出：等它退出也有上限。
        let (result, took) = fake("exec >&-; exec sleep 10", Duration::from_millis(300));
        assert!(matches!(result, Err(ProbeError::TimedOut)), "{result:?}");
        assert!(took < Duration::from_secs(3), "没一直等：{took:?}");
    }

    #[test]
    fn a_helper_whose_leftovers_hold_the_pipe_is_not_waited_for() {
        // 说完了、退出了，放出去的还拿着标准输出：读不到结尾，也到时就走。
        let (result, took) = fake(
            r#"echo '{"version":1,"platform":"linux","mechanisms":[]}'; sleep 10 &"#,
            Duration::from_millis(300),
        );
        assert!(matches!(result, Err(ProbeError::TimedOut)), "{result:?}");
        assert!(took < Duration::from_secs(3), "没一直等：{took:?}");
    }

    #[test]
    fn a_helper_that_exits_non_zero_failed() {
        let (result, _) = fake("exit 3", WAIT);
        let error = result.expect_err("退出码不是 0");
        assert!(matches!(error, ProbeError::Exited(_)), "{error:?}");
        assert!(error.to_string().starts_with("helper failed: "), "{error}");
        assert!(error.to_string().contains('3'), "{error}");
    }

    #[test]
    fn a_helper_that_says_something_else_is_not_understood() {
        for body in [
            "echo hello",
            "printf '\\377\\376'",
            r#"echo '{"version":1,"platform":"plan9","mechanisms":[]}'"#,
            r#"echo '{"version":1,"platform":"linux","mechanisms":[]}'; echo '{}'"#,
            "true",
        ] {
            let (result, _) = fake(body, WAIT);
            let error = result.expect_err(body);
            assert!(
                matches!(error, ProbeError::Unreadable(_)),
                "{body}: {error:?}"
            );
        }
    }

    #[test]
    fn a_helper_of_another_version_is_not_understood() {
        let (result, _) = fake(
            r#"echo '{"version":2,"platform":"linux","mechanisms":[]}'"#,
            WAIT,
        );
        let error = result.expect_err("版本不认得");
        assert_eq!(error.to_string(), "helper output not understood: version 2");
    }
}
