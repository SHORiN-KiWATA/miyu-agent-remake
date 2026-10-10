//! 终端这个软件包（核心 9-1 上，蓝图「输入历史列表」第 8 条）：连上以后要一次 `package.list`，照 `tui` 那一项的
//! `state` 把输入历史放进包的状态目录（老位置有的先搬过去），读进来，和连上以前打的合在一起。只放一次。

use std::path::PathBuf;

use super::App;
use crate::core::Command;
use crate::input::{Saved, place};

/// 终端这个包的编号（`resources/packages/tui/package.toml`）。
const PACKAGE: &str = "tui";

impl App {
    /// 连上了：输入历史还没放好的，去要软件包。
    pub(super) fn ask_packages(&mut self) {
        if !self.history_placed {
            self.core.send(Command::ListPackages);
        }
    }

    /// 软件包回来了：照 `tui` 的状态目录放输入历史；没有的（旧核心、没装清单）照旧放老位置。
    pub(super) fn packages_arrived(&mut self, packages: &[(String, Option<PathBuf>)]) {
        if self.history_placed {
            return;
        }
        self.history_placed = true;
        let state = packages
            .iter()
            .find(|(id, _)| id == PACKAGE)
            .and_then(|(_, state)| state.as_deref());
        let path = place(state, self.old_history.take()).filter(|_| !cfg!(test));
        let keep = self.config.layout.history_keep;
        self.input.keep_history(Saved::at(path), keep);
    }
}
