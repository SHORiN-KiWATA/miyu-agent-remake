//! 启动参数（蓝图 `tui.md`「指定会话启动与 herdr 恢复」、「配置页」第 1 条）：进入全屏前检查；显式会话不回退到最近会话；
//! `--page config` 直接停在配置页，不进会话。

use std::io;

use crate::core::Start;

/// `-h` 印的用法（`miyu tui -h` 原样转给它，核心 I9：要完整）。
pub const USAGE: &str = "Usage: miyu-tui-demo [--resume <session UUID> | --page <page> | config]

  (no arguments)        start as configured (tui.startup: new or recent session)
  --resume <UUID>       open this session; fail if it can't be loaded
  --page config, config open the settings page without a session; Esc on its menu exits
  -h, --help            print this help

Environment: MIYU_HOME, MIYU_RESOURCES, MIYU_CORE_BIN, MIYU_TUI_IME=0 (don't switch the input method)";

/// 认得的页名。认不出的照常打开首页、不报错。
const PAGES: [&str; 1] = ["config"];

/// 照参数怎么起来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// 进哪个会话。
    pub start: Start,
    /// 直接停在哪一页（现在只有 `config`）。
    pub page: Option<&'static str>,
    /// 只印用法（`-h`、`--help`）。
    pub help: bool,
}

/// 读参数：不带的照配置启动；`--resume <完整会话编号>`；`--page <页名>` 或 `config`。
pub fn launch(args: impl IntoIterator<Item = String>) -> io::Result<Launch> {
    let args: Vec<String> = args.into_iter().collect();
    let usual = Launch {
        start: Start::Usual,
        page: None,
        help: false,
    };
    match args.as_slice() {
        [] => Ok(usual),
        [flag] if flag == "-h" || flag == "--help" => Ok(Launch {
            help: true,
            ..usual
        }),
        [flag, id] if flag == "--resume" && miyu_kernel::id::SessionId::parse(id).is_ok() => {
            Ok(Launch {
                start: Start::Resume(id.clone()),
                ..usual
            })
        }
        [flag, page] if flag == "--page" => Ok(page_named(page).unwrap_or(usual)),
        [page] if page == "config" => Ok(page_named(page).unwrap_or(usual)),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
    }
}

/// 认得的页：不进会话、停在这一页。
fn page_named(name: &str) -> Option<Launch> {
    PAGES.iter().find(|p| **p == name).map(|page| Launch {
        start: Start::Bare,
        page: Some(page),
        help: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{Launch, launch};
    use crate::core::Start;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_an_explicit_complete_session_is_accepted() {
        let id = "01a0fb48-0000-7000-8000-000000000001";
        assert_eq!(launch(Vec::new()).unwrap().start, Start::Usual);
        assert!(launch(args(&["-h"])).unwrap().help);
        assert!(launch(args(&["--help"])).unwrap().help);
        assert_eq!(
            launch(args(&["--resume", id])).unwrap().start,
            Start::Resume(id.into())
        );
        for bad in [
            &["--resume"][..],
            &["--resume", "short"],
            &["--unknown"],
            &["--resume", id, "extra"],
            &["--page"],
        ] {
            assert!(launch(args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_config_page_starts_without_a_session_and_unknown_pages_open_home() {
        let config = Launch {
            start: Start::Bare,
            page: Some("config"),
            help: false,
        };
        assert_eq!(launch(args(&["--page", "config"])).unwrap(), config);
        assert_eq!(launch(args(&["config"])).unwrap(), config);
        let home = launch(args(&["--page", "nope"])).unwrap();
        assert_eq!(
            (home.start, home.page),
            (Start::Usual, None),
            "认不出的页名照常打开首页"
        );
    }
}
