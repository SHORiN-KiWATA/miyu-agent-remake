//! 交给系统弹的通知（蓝图 `tui.md`「系统通知」第 4 条）：别的终端、tmux、herdr 里用。Linux 用 `notify-send`，
//! macOS 用 `osascript`。丢出去不等，另起一个线程收尸；拉不起来什么都不做。

use std::process::{Command, Stdio};

/// 这一条通知要跑的命令：程序和参数。
pub fn command(app: &str, title: &str, body: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "macos") {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            quoted(body),
            quoted(title)
        );
        ("osascript".into(), vec!["-e".into(), script])
    } else {
        let args = vec![
            "-a".into(),
            app.to_string(),
            "--".into(),
            title.to_string(),
            body.to_string(),
        ];
        ("notify-send".into(), args)
    }
}

/// AppleScript 的字符串里，反斜杠和引号要转义。
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 跑一个命令，不等它：输入输出接空，另起一个线程等它退出（不留僵尸进程）。拉不起来的交回 `false`。
pub fn spawn(program: &str, args: &[String]) -> bool {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn the_system_gets_the_title_and_the_body() {
        let (program, args) = command("Miyu", "Miyu", "出错了");
        if cfg!(target_os = "macos") {
            assert_eq!(program, "osascript");
            assert_eq!(
                args[1],
                "display notification \"出错了\" with title \"Miyu\""
            );
        } else {
            assert_eq!(program, "notify-send");
            assert_eq!(args, ["-a", "Miyu", "--", "Miyu", "出错了"]);
        }
    }

    #[test]
    fn quotes_in_applescript_are_escaped() {
        assert_eq!(super::quoted(r#"说"好"\"#), r#"说\"好\"\\"#);
    }
}
