//! herdr 通知：伪终端真 TUI、假核心和上报程序，不发真的桌面通知。
#![cfg(unix)]
mod support;
use miyu_session::testkit::{Play, Script};
use std::{
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use support::Home;

#[test]
fn a_reply_reports_to_herdr_and_sends_one_desktop_notification() {
    let home = Home::new(Script::new([Play::Says("通知验收回答。")]));
    let bin = home.root().join("notify-bin");
    std::fs::create_dir_all(&bin).expect("测试目录");
    let log = home.root().join("reports.log");
    std::fs::write(&log, "").expect("测试日志");
    for name in ["herdr-test", "notify-send", "osascript"] {
        let file = bin.join(name);
        std::fs::write(
            &file,
            format!("#!/bin/sh\nprintf '%s\\n' '{name}' \"$*\" >> \"$HERDR_TEST_REPORTS\"\n"),
        )
        .expect("假命令");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("可执行");
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let binary = bin.join("herdr-test").display().to_string();
    let log_name = log.display().to_string();
    let mut tui = home.tui_with(
        "zh_CN.UTF-8",
        &[
            ("PATH", &path),
            ("HERDR_ENV", "1"),
            ("HERDR_PANE_ID", "p-test"),
            ("HERDR_BIN_PATH", &binary),
            ("HERDR_TEST_REPORTS", &log_name),
        ],
    );
    tui.wait_for("工作区");
    tui.send(b"\x1b[O");
    tui.say("测试通知");
    tui.wait_for("通知验收回答。");
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        tui.pump(Duration::from_millis(50));
        let reported = std::fs::read_to_string(&log).expect("上报日志");
        if reported.matches("--state idle").count() >= 2
            && (reported.contains("notify-send") || reported.contains("osascript"))
        {
            assert!(reported.contains("--state working"));
            assert!(
                reported
                    .lines()
                    .filter(|line| *line == "notify-send" || *line == "osascript")
                    .count()
                    == 1,
                "{reported}"
            );
            break;
        }
        assert!(Instant::now() < until, "{reported}");
    }
}
