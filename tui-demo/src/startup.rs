//! 启动参数（蓝图 `tui.md`「指定会话启动与 herdr 恢复」）：进入全屏前检查；显式会话不回退到最近会话。

use std::io;

/// 从参数读取要恢复的完整会话编号；没给参数的照配置启动。
pub fn resume(args: impl IntoIterator<Item = String>) -> io::Result<Option<String>> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(None),
        [flag, id] if flag == "--resume" && miyu_kernel::id::SessionId::parse(id).is_ok() => {
            Ok(Some(id.clone()))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Usage: miyu-tui-demo [--resume <session UUID>]",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::resume;
    #[test]
    fn only_an_explicit_complete_session_is_accepted() {
        let id = "01a0fb48-0000-7000-8000-000000000001";
        assert_eq!(resume(Vec::new()).unwrap(), None);
        assert_eq!(
            resume(["--resume".into(), id.into()]).unwrap(),
            Some(id.into())
        );
        for args in [
            vec!["--resume"],
            vec!["--resume", "short"],
            vec!["--unknown"],
            vec!["--resume", id, "extra"],
        ] {
            assert!(resume(args.into_iter().map(str::to_string)).is_err());
        }
    }
}
