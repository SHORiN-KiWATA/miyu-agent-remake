//! 崩溃记录（蓝图 `tui.md`「崩了」）：把这一次的时刻、报错、位置和完整的调用栈追加进机器缓存目录下的
//! `tui/crash.log`，下一次复现不出来的崩溃也留得下位置。

use std::backtrace::Backtrace;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::PathBuf;

use miyu_store::env::Env;

/// 记下这一次崩溃，交回记在哪；缓存目录找不到、写不进去的交回 `None`（已经在崩了，不再多事）。
pub fn record(info: &PanicHookInfo<'_>) -> Option<PathBuf> {
    let dir = miyu_store::root::cache_root(&Env::current())
        .ok()?
        .join("tui");
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("crash.log");
    let time = jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S").to_string();
    let text = entry(
        &time,
        &info.to_string(),
        &Backtrace::force_capture().to_string(),
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    file.write_all(text.as_bytes()).ok()?;
    Some(path)
}

/// 一次崩溃写成的一段：时刻和版本一行，报错连位置，调用栈，末尾空一行隔开下一次。
fn entry(time: &str, message: &str, backtrace: &str) -> String {
    format!(
        "── {time} miyu-tui-demo {}\n{message}\n{backtrace}\n\n",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::entry;

    #[test]
    fn an_entry_keeps_the_time_the_message_and_the_backtrace() {
        let text = entry(
            "2026-09-30 03:10:00",
            "panicked at src/ui/menu.rs:40:9:\nrange end index 1 out of range for slice of length 0",
            "   0: miyu_tui_demo::ui::menu::lines\n             at ./src/ui/menu.rs:40:9",
        );
        assert!(text.starts_with("── 2026-09-30 03:10:00 miyu-tui-demo "));
        assert!(text.contains("range end index 1") && text.contains("src/ui/menu.rs:40:9"));
        assert!(text.ends_with("\n\n"), "和下一次之间空一行");
    }
}
