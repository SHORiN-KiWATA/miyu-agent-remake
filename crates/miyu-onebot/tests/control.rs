//! `miyu onebot status` 说什么（施工 O-18，`onebot.md` 第一条「对外的样子」、「状态文件」）：照 `extension.status` 的那一项
//! 说开没开、在不在跑、停下的原因和标准错误；在跑的、状态文件的进程号对得上的，再说 NapCat 和两个地址，对不上、读不懂的
//! 不说。怎么换成字由 `texts.rs` 守着。

use serde_json::{Value, json};

use miyu_onebot::control::{Halt, Line, Report, describe};

/// 在跑的那一项：进程号 `pid`。
fn running(pid: u64) -> Value {
    json!({"package": "onebot", "name": "接入QQ", "start": "manual", "on": true, "state": "running", "pid": pid, "failures": 0})
}

/// 状态文件：进程号 `pid`，NapCat 照 `napcat`。
fn board(pid: u64, napcat: Value) -> Value {
    json!({"pid": pid, "listen": 8301, "web": 8302, "napcat": napcat})
}

fn said(reports: &[Report]) -> Vec<Line> {
    reports.iter().cloned().map(Line::Say).collect()
}

#[test]
fn off_and_starting_say_one_line() {
    let off =
        json!({"package": "onebot", "start": "manual", "on": false, "state": "off", "failures": 0});
    assert_eq!(describe(&off, None), said(&[Report::Off]));
    let starting =
        json!({"package": "onebot", "on": true, "state": "starting", "pid": 7, "failures": 0});
    assert_eq!(
        describe(&starting, Some(&board(7, json!({"connected": false})))),
        said(&[Report::Starting]),
        "还没握手的不说 NapCat"
    );
}

#[test]
fn running_says_napcat_and_the_addresses_only_from_its_own_status_file() {
    let connected = json!({"connected": true, "self_id": "30003", "implementation": "NapCat.Onebot", "version": "4.8.0"});
    assert_eq!(
        describe(&running(7), Some(&board(7, connected.clone()))),
        said(&[
            Report::Running(7),
            Report::Napcat {
                implementation: "NapCat.Onebot".to_string(),
                version: "4.8.0".to_string(),
                bot: "30003".to_string(),
            },
            Report::Ports {
                listen: 8301,
                web: 8302
            },
        ])
    );
    assert_eq!(
        describe(
            &running(7),
            Some(&board(7, json!({"connected": true, "self_id": "30003"})))
        ),
        said(&[
            Report::Running(7),
            Report::NapcatBot("30003".to_string()),
            Report::Ports {
                listen: 8301,
                web: 8302
            },
        ]),
        "还没问到是哪个实现"
    );
    assert_eq!(
        describe(&running(7), Some(&board(7, json!({"connected": false})))),
        said(&[
            Report::Running(7),
            Report::NoNapcat,
            Report::Ports {
                listen: 8301,
                web: 8302
            },
        ])
    );
    assert_eq!(
        describe(&running(7), Some(&board(8, connected.clone()))),
        said(&[Report::Running(7)]),
        "上一个进程留下的不用"
    );
    assert_eq!(describe(&running(7), None), said(&[Report::Running(7)]));
    assert_eq!(
        describe(&running(7), Some(&json!({"pid": 7}))),
        said(&[Report::Running(7)]),
        "读不懂的不用"
    );
    assert_eq!(
        describe(&running(7), Some(&json!({"pid": "7", "napcat": connected}))),
        said(&[Report::Running(7)]),
        "进程号写成字的不算对得上"
    );
}

#[test]
fn waiting_rounds_up_to_whole_seconds() {
    for (millis, seconds) in [(1500, 2), (1000, 1), (1, 1), (0, 0), (60_000, 60)] {
        let waiting = json!({"package": "onebot", "on": true, "state": "waiting", "retry_in": millis, "failures": 3});
        assert_eq!(
            describe(&waiting, None),
            said(&[Report::Waiting {
                seconds,
                failures: 3
            }]),
            "{millis}"
        );
    }
}

#[test]
fn stopped_says_why_and_the_end_of_its_stderr() {
    for (reason, halt) in [
        ("config_error", Halt::ConfigError),
        ("failed_repeatedly", Halt::FailedRepeatedly(5)),
        ("not_installed", Halt::NotInstalled),
        ("cannot_start", Halt::CannotStart),
        ("protocol_mismatch", Halt::ProtocolMismatch),
        ("new_reason", Halt::Other("new_reason".to_string())),
    ] {
        let stopped = json!({"package": "onebot", "on": true, "state": "stopped", "reason": reason, "failures": 5, "stderr": ""});
        assert_eq!(
            describe(&stopped, None),
            said(&[Report::Halted(halt.clone())]),
            "{reason}：标准错误是空的不说"
        );
        let with_stderr = json!({"package": "onebot", "on": true, "state": "stopped", "reason": reason, "failures": 5, "stderr": "端口 8301 被占了。\nsecond line\n"});
        assert_eq!(
            describe(&with_stderr, None),
            vec![
                Line::Say(Report::Halted(halt)),
                Line::Say(Report::Stderr),
                Line::Tail("端口 8301 被占了。".to_string()),
                Line::Tail("second line".to_string()),
            ],
            "{reason}"
        );
    }
}

#[test]
fn an_unknown_state_is_said_as_it_is() {
    let odd = json!({"package": "onebot", "on": true, "state": "dreaming", "failures": 0});
    assert_eq!(
        describe(&odd, None),
        said(&[Report::Other("dreaming".to_string())])
    );
}
