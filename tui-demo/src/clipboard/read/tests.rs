//! 读剪贴板：按平台挑命令，第一个读得到的算数，卡住的放弃。

use std::time::{Duration, Instant};

use super::{Os, Unreadable, candidates, read_with};

fn names(list: &[(&str, &[&str])]) -> Vec<String> {
    list.iter().map(|(cmd, _)| (*cmd).to_string()).collect()
}

#[test]
fn the_readers_follow_the_platform() {
    assert_eq!(
        names(&candidates(Os::Linux, true)),
        ["wl-paste", "xclip", "xsel"]
    );
    assert_eq!(
        names(&candidates(Os::Linux, false)),
        ["xclip", "xsel"],
        "没有 Wayland 不试 wl-paste"
    );
    assert_eq!(names(&candidates(Os::Mac, false)), ["pbpaste"]);
    assert_eq!(names(&candidates(Os::Windows, false)), ["powershell"]);
}

#[cfg(unix)]
#[test]
fn the_first_reader_that_works_wins() {
    let got = read_with(
        &[
            ("false", &[]),
            ("printf", &["甲\\n乙"]),
            ("printf", &["丙"]),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(got, Ok("甲\n乙".to_string()));
    let none = read_with(
        &[("false", &[]), ("no-such-clipboard-tool", &[])],
        Duration::from_secs(1),
    );
    assert_eq!(none, Err(Unreadable));
}

#[cfg(unix)]
#[test]
fn a_reader_that_hangs_is_cut_off() {
    let t = Instant::now();
    let got = read_with(&[("sleep", &["5"])], Duration::from_millis(200));
    assert_eq!(got, Err(Unreadable));
    assert!(t.elapsed() < Duration::from_secs(2), "等到点就放弃");
}

#[cfg(unix)]
#[test]
fn a_big_clipboard_is_read_whole() {
    // 超过管道缓冲（64KB）的：边跑边读，不会被当成卡住。
    let got = read_with(
        &[("head", &["-c", "300000", "/dev/zero"])],
        Duration::from_secs(2),
    );
    assert_eq!(got.map(|t| t.len()), Ok(300_000));
}
