//! 换吉祥物（蓝图 `tui.md`「吉祥物包」第 3、5 条）：连上时、配置变了时读 `tui.mascot`；是一个吉祥物包的编号就照
//! `package.list` 找它的模型、`package.file` 读来、查过换上；没写的、包没了、读不成、查不过的照内置画，后三种弹一句。
//! 起终端时设了 `MIYU_TUI_MASCOT` 的照那份本机文件画、不看配置（做吉祥物的人试画、测具用），查不过的提示里带原因。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::App;
use crate::core::{Command, Update};
use crate::mascot::{Look, package};
use crate::oobe::asker::TAG_BASE;

/// 读 `tui.mascot` 的请求编号。
const SETTING_TAG: u64 = TAG_BASE - 6;
/// 找吉祥物包的模型的请求编号（`package.list`）。
const LIST_TAG: u64 = TAG_BASE - 7;
/// 读模型的请求编号（`package.file`）。
const FILE_TAG: u64 = TAG_BASE - 8;

/// 本机试画的模型文件。
const PREVIEW_ENV: &str = "MIYU_TUI_MASCOT";

/// 换吉祥物这件事的状态。
#[derive(Debug, Default)]
pub struct MascotPick {
    /// 内置的那只：换回来时照它。
    builtin: Option<Look>,
    /// 照 `MIYU_TUI_MASCOT` 画着：不看配置。
    preview: bool,
    /// 在读的那个包。
    loading: Option<String>,
    /// 现在画着的包；内置的是 `None`。
    shown: Option<String>,
}

impl App {
    /// 起终端时：设了 `MIYU_TUI_MASCOT` 的照它画。
    pub fn mascot_preview(&mut self) {
        let Some(path) = std::env::var_os(PREVIEW_ENV).filter(|p| !p.is_empty()) else {
            return;
        };
        self.mascot_pick.preview = true;
        let read = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| package::read(&bytes));
        match read {
            Ok(look) => self.put_mascot(look, None),
            Err(why) => self.mascot_failed(Some(&why)),
        }
    }

    /// 连上、配置变了去读 `tui.mascot`；几个请求的回应归这里（交回 `true`）。
    pub(super) fn mascot_pick_update(&mut self, update: &Update) -> bool {
        match update {
            Update::HeadConfig(_) if !self.mascot_pick.preview => {
                self.mascot_ask(SETTING_TAG, "config.get", json!({"keys": ["tui.mascot"]}));
                false
            }
            Update::Answer { tag, result } if *tag == SETTING_TAG => {
                let id = result.as_ref().ok().and_then(|got| {
                    got["items"]["tui.mascot"]["value"]
                        .as_str()
                        .map(str::to_string)
                });
                self.mascot_chosen(id);
                true
            }
            Update::Answer { tag, result } if *tag == LIST_TAG => {
                let model = result.as_ref().ok().and_then(|got| self.model_of(got));
                match (self.mascot_pick.loading.clone(), model) {
                    (Some(id), Some(path)) => {
                        self.mascot_ask(
                            FILE_TAG,
                            "package.file",
                            json!({"package": id, "path": path}),
                        );
                    }
                    (Some(_), None) => self.mascot_failed(None),
                    (None, _) => {}
                }
                true
            }
            Update::Answer { tag, result } if *tag == FILE_TAG => {
                let Some(id) = self.mascot_pick.loading.take() else {
                    return true;
                };
                let look = result
                    .as_ref()
                    .ok()
                    .and_then(|got| STANDARD.decode(got["data"].as_str()?).ok())
                    .and_then(|bytes| package::read(&bytes).ok());
                match look {
                    Some(look) => self.put_mascot(look, Some(id)),
                    None => self.mascot_failed(None),
                }
                true
            }
            _ => false,
        }
    }

    /// 配置里选的：没写的换回内置的；和画着的一样的不动；别的去找它的模型。
    fn mascot_chosen(&mut self, id: Option<String>) {
        if id == self.mascot_pick.shown && self.mascot_pick.loading.is_none() {
            return;
        }
        match id {
            None => {
                self.mascot_pick.loading = None;
                self.restore_mascot();
            }
            Some(id) => {
                self.mascot_pick.loading = Some(id);
                self.mascot_ask(LIST_TAG, "package.list", json!({}));
            }
        }
    }

    /// `package.list` 里在读的那个包的模型路径：得是读成了的吉祥物包。
    fn model_of(&self, got: &Value) -> Option<String> {
        let id = self.mascot_pick.loading.as_deref()?;
        got["packages"]
            .as_array()?
            .iter()
            .find(|p| p["package"] == id && p["kind"] == "mascot")
            .and_then(|p| p["mascot"]["model"].as_str())
            .map(str::to_string)
    }

    /// 换上一只：内置的那只先留一份。
    fn put_mascot(&mut self, look: Look, id: Option<String>) {
        let old = std::mem::replace(&mut self.config.mascot, look);
        self.mascot_pick.builtin.get_or_insert(old);
        self.mascot_pick.shown = id;
    }

    /// 换回内置的。
    fn restore_mascot(&mut self) {
        if let Some(builtin) = self.mascot_pick.builtin.take() {
            self.config.mascot = builtin;
        }
        self.mascot_pick.shown = None;
    }

    /// 读不成、查不过：照内置画，弹一句；本机试画的带上原因。
    fn mascot_failed(&mut self, why: Option<&str>) {
        self.mascot_pick.loading = None;
        self.restore_mascot();
        let text = self.config.text.mascot_failed.clone();
        let text = match why {
            Some(why) => format!("{text}：{why}"),
            None => text,
        };
        self.hint(text, false);
    }

    /// 发一个请求，回应照编号回来。
    fn mascot_ask(&mut self, tag: u64, method: &'static str, params: Value) {
        self.core.send(Command::Ask {
            tag,
            method,
            params,
        });
    }
}
