//! 退出了以后怎么办（施工 9-4 上，`extensions.md`「怎么走」第 5 条）：退避 1、2、4、8 秒，最多 60 秒；连续 5 次停下；退出码 1
//! 直接停下；跑满了的这一次从 1 数。

use std::time::Duration;

use super::*;

fn seconds(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn the_backoff_doubles_from_one_second_up_to_a_minute() {
    let timing = Timing::default();
    let waits: Vec<Duration> = (1..=8).map(|n| backoff(&timing, n)).collect();
    assert_eq!(
        waits,
        [1, 2, 4, 8, 16, 32, 60, 60].map(seconds),
        "第 7 次本该 64 秒，封顶 60 秒"
    );
    assert_eq!(backoff(&timing, u32::MAX), seconds(60), "数再大也不溢出");
}

#[test]
fn five_failures_in_a_row_stop_it() {
    let timing = Timing::default();
    let mut failures = 0;
    let mut waits = Vec::new();
    loop {
        match next(&timing, failures, Some(2), false) {
            Next::Retry {
                failures: now,
                after,
            } => {
                failures = now;
                waits.push(after);
            }
            Next::Stop { reason, failures } => {
                assert_eq!((reason, failures), (Reason::FailedRepeatedly, 5));
                break;
            }
        }
    }
    assert_eq!(waits, [1, 2, 4, 8].map(seconds), "前四次退避，第五次停下");
}

#[test]
fn exit_code_one_stops_at_once_and_other_exits_retry() {
    let timing = Timing::default();
    assert_eq!(
        next(&timing, 0, Some(1), false),
        Next::Stop {
            reason: Reason::ConfigError,
            failures: 1
        }
    );
    assert_eq!(
        next(&timing, 3, Some(1), true),
        Next::Stop {
            reason: Reason::ConfigError,
            failures: 1
        },
        "跑满了也一样停下"
    );
    for code in [Some(0), Some(2), Some(-1), None] {
        assert_eq!(
            next(&timing, 0, code, false),
            Next::Retry {
                failures: 1,
                after: seconds(1)
            },
            "{code:?}：0、别的退出码、被杀掉的都退避"
        );
    }
}

#[test]
fn a_run_that_stayed_up_starts_the_count_again() {
    let timing = Timing::default();
    assert_eq!(
        next(&timing, 4, Some(2), true),
        Next::Retry {
            failures: 1,
            after: seconds(1)
        },
        "跑满了：这一次从 1 数"
    );
    assert_eq!(
        next(&timing, 4, Some(2), false),
        Next::Stop {
            reason: Reason::FailedRepeatedly,
            failures: 5
        },
        "没跑满：接着数，到 5 停下"
    );
}

#[test]
fn a_process_the_core_killed_is_a_failure_not_a_config_error() {
    // Windows 上杀掉的进程退出码是 1（施工 9-4 修）：核心杀的当没有退出码，照一次失败退避，不当「配置错」停下。
    assert_eq!(exit_code(Some(1), true), None);
    assert_eq!(exit_code(Some(1), false), Some(1), "自己退出的照旧");
    assert_eq!(exit_code(None, false), None);
    let timing = Timing::default();
    assert!(matches!(
        next(&timing, 0, exit_code(Some(1), true), false),
        Next::Retry { failures: 1, .. }
    ));
}
