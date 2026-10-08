//! 开头那几行旁白（`docs/blueprint/cli/ask.md`「起头那几行」，`config.md` 第十条第 10 条）：配置里有错（施工 8-2）、这里的
//! 项目配置还没信任（施工 8-3）、沙盒用不了（施工 5-4 下）、目录太宽（施工 4-5 下），照这个先后，都是标准错误上的灰字，
//! 只给人看的时候说（`--format json` 的不印）。
//!
//! 造会话、说话的回应都带着会话实际在哪个目录里干活、这里的项目配置还没信任：一样的只说一次（[`Follow::opened`]）。

use serde_json::Value;

use super::super::steps;
use super::super::{Format, Screen};
use super::Follow;
use crate::shown::Line;

impl Follow<'_> {
    /// 握手的回应说配置里有几处错误（施工 8-2）：说一句，`miyu check` 看是哪里（施工 8-30 起）。
    pub(crate) fn config_errors(&mut self, errors: u64, screen: &mut Screen<'_>) {
        if self.plan.format == Format::Text {
            self.aside(
                &Line::gray(self.plan.language.config_errors(errors)),
                screen,
            );
        }
    }

    /// 造会话、说话的回应说这里的项目配置 `file` 还没信任、这次没用它（施工 8-3）：说一句，`miyu config trust` 看一眼再定。
    /// 一次 `miyu ask` 只说一次。
    pub(crate) fn untrusted(&mut self, file: &str, screen: &mut Screen<'_>) {
        if self.untrusted {
            return;
        }
        self.untrusted = true;
        if self.plan.format == Format::Text {
            self.aside(
                &Line::gray(self.plan.language.untrusted_project(file)),
                screen,
            );
        }
    }

    /// 握手的回应说沙盒用不了，原因是 `reason`（协议上的写法）：执行命令都要确认，这里确认不了，说一句（施工 5-4 下）。
    /// 只给人看的时候说。
    pub(crate) fn unsandboxed(&mut self, reason: &str, screen: &mut Screen<'_>) {
        if self.plan.format == Format::Text {
            self.aside(&steps::unsandboxed(self.plan, reason), screen);
        }
    }

    /// 核心说会话实际在 `used` 里干活：和头报的不一样，就是目录太宽、退回了账号的工作区，说一句。造会话、
    /// 说话的回应都带着它，一样的不再说：一次 `miyu ask` 只说一次。
    pub(crate) fn moved(&mut self, used: &str, screen: &mut Screen<'_>) {
        if used == self.cwd {
            return;
        }
        self.cwd = used.to_string();
        if self.plan.format == Format::Text {
            self.aside(&steps::moved(self.plan, used), screen);
        }
    }

    /// 说话的回应到了（`result`）：项目配置没信任的、目录太宽的，照上面各说一句。
    pub(crate) fn opened(&mut self, result: &Value, screen: &mut Screen<'_>) {
        if let Some(file) = result["untrusted_project"].as_str() {
            self.untrusted(file, screen);
        }
        if let Some(used) = result["cwd"].as_str() {
            self.moved(used, screen);
        }
    }
}
