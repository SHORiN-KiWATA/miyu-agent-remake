//! 启动意图：配置页直接进入，恢复会话只接受完整编号；不依赖正在运行的 TUI。

use std::io;

/// 同一个界面程序的启动模式。
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// 对话模式，可显式恢复一个会话。
    Talk(Option<String>),
    /// 独立配置页，不创建/恢复会话。
    Config,
}

/// 在进入备用屏前解析参数；`--page config` 是启动器传来的页面意图。
///
/// # Errors
/// 参数组合不认、恢复编号不完整时返回 InvalidInput。
pub fn parse(args: impl IntoIterator<Item = String>) -> io::Result<Mode> {
    let args: Vec<_> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(Mode::Talk(None)),
        [page] if page == "config" => Ok(Mode::Config),
        [flag, page] if flag == "--page" && page == "config" => Ok(Mode::Config),
        [flag, id] if flag == "--resume" && miyu_kernel::id::SessionId::parse(id).is_ok() => {
            Ok(Mode::Talk(Some(id.clone())))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Usage: miyu-tui-demo [config | --page config | --resume <session UUID>]",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_is_a_startup_intent_not_a_running_process_signal() {
        assert_eq!(parse(["config".into()]).unwrap(), Mode::Config);
        assert_eq!(
            parse(["--page".into(), "config".into()]).unwrap(),
            Mode::Config
        );
        assert_eq!(parse(Vec::new()).unwrap(), Mode::Talk(None));
    }
    #[test]
    fn only_an_explicit_complete_session_is_accepted_and_modes_cannot_mix() {
        let id = "01a0fb48-0000-7000-8000-000000000001";
        assert_eq!(
            parse(["--resume".into(), id.into()]).unwrap(),
            Mode::Talk(Some(id.into()))
        );
        for args in [
            vec!["--resume"],
            vec!["--resume", "short"],
            vec!["--page", "unknown"],
            vec!["config", "--resume", id],
            vec!["--resume", id, "extra"],
        ] {
            assert!(parse(args.into_iter().map(str::to_string)).is_err());
        }
    }
}
