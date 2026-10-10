//! 终端自己的配置项（`tui.*`，终端软件包清单 `resources/packages/tui/package.toml` 的 `[settings]`）：连上时、配置变了时
//! `config.get` 读。图标那一套照 `tui.icons` 换（蓝图 `tui.md`「图标」第 4 条）；时间线的几个开关照 `tui.timeline_*`
//! 盖过 `timeline.json`（「时间线」第 18 条）；吉祥物的两个开关照 `tui.mascot_*` 盖过 `layout.json`（「后台命令、子代理和
//! 侧边栏」第 7 条）。几样分开问：旧核心不认时间线那几个键时，整份拒，图标照样读得到。

use serde_json::{Value, json};

use super::App;
use crate::config::{Layout, Timeline};
use crate::core::{Command, Update};
use crate::oobe::asker::TAG_BASE;

/// 读图标那一套的请求编号：在引导自己的编号下面，和「引导走过没有」的（`TAG_BASE - 1`）分开。
const ICONS_TAG: u64 = TAG_BASE - 2;
/// 读时间线那几个开关的请求编号。
const TIMELINE_TAG: u64 = TAG_BASE - 3;
/// 读吉祥物那两个开关的请求编号（头像要的是 `TAG_BASE - 5`，`avatars.rs`）。
const MASCOT_TAG: u64 = TAG_BASE - 4;

impl App {
    /// 头的配置读到了（连上、配置变了）就去读；回应归这里（交回 `true`）。
    pub(super) fn head_settings_update(&mut self, update: &Update) -> bool {
        match update {
            Update::HeadConfig(_) => {
                self.ask_config(ICONS_TAG, &["tui.icons"]);
                self.ask_config(TIMELINE_TAG, &Timeline::KEYS);
                self.ask_config(MASCOT_TAG, &Layout::MASCOT_KEYS);
                false
            }
            Update::Answer { tag, result } if *tag == ICONS_TAG => {
                if let Some(name) = result
                    .as_ref()
                    .ok()
                    .and_then(|got| got["items"]["tui.icons"]["value"].as_str())
                {
                    let name = name.to_string();
                    self.use_icons(&name);
                }
                true
            }
            Update::Answer { tag, result } if *tag == TIMELINE_TAG => {
                if let Ok(got) = result {
                    self.config.timeline.apply(got);
                }
                true
            }
            Update::Answer { tag, result } if *tag == MASCOT_TAG => {
                if let Ok(got) = result {
                    self.config.layout.apply_mascot(got);
                }
                true
            }
            _ => false,
        }
    }

    /// 读几个配置项。
    fn ask_config(&mut self, tag: u64, keys: &[&str]) {
        let params: Value = json!({ "keys": keys });
        self.core.send(Command::Ask {
            tag,
            method: "config.get",
            params,
        });
    }
}
