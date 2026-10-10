//! 配置页怎么存（蓝图「配置页」第 16、24 条）：先把新贴的 key 存进密钥库，全成了再一次 `config.set`；存成了重读，
//! 没存成的这一次的改动扔掉、原因写出来。从 `mod.rs` 分出来。

use serde_json::{Value, json};

use super::{Popup, Settings, Texts, Tone, Waiting, forms};
use crate::core::Refusal;

impl Settings {
    /// 存（悬浮窗里 `s`、`q`，删除、选默认模型确认了也是）：先把新贴的 key 存进密钥库，全成了再一次 `config.set`
    /// （「配置页」第 24 条）。`close` 是存成了关掉悬浮窗。
    pub fn save(&mut self, close: bool, texts: &Texts) {
        if self.draft.is_empty() {
            if close {
                self.popup = None;
            }
            return;
        }
        let Some(data) = self.data.as_ref().filter(|_| self.online) else {
            self.draft.clear();
            self.refresh();
            self.failed(texts.status("offline"));
            return;
        };
        if self
            .waiting
            .values()
            .any(|w| matches!(w, Waiting::Save | Waiting::Secret(..)))
        {
            return;
        }
        let secrets = self.draft.secrets(data);
        self.close_after_save = close;
        self.named.clear();
        self.say(texts.status("saving"), Tone::Busy);
        if secrets.is_empty() {
            self.send_config();
        } else {
            self.secrets_left = secrets.len();
            for (provider, name, value) in secrets {
                let params = json!({"name": name, "value": value});
                self.ask("secret.set", params, Waiting::Secret(provider, name));
            }
        }
    }

    /// 没存成：这一次的改动扔掉（界面照回读来的），原因写在开着的编辑窗里，没开着的写在状态行。
    pub(super) fn failed(&mut self, text: String) {
        self.close_after_save = false;
        self.draft.clear();
        self.refresh();
        match &mut self.popup {
            Some(Popup::Form(form)) => {
                form.error = Some(text);
                self.status = None;
            }
            _ => self.say(text, Tone::Bad),
        }
    }

    fn send_config(&mut self) {
        let Some(data) = &self.data else {
            return;
        };
        let params = self.draft.request(data, &self.named);
        self.ask("config.set", params, Waiting::Save);
    }

    pub(super) fn secret_saved(
        &mut self,
        provider: String,
        name: String,
        result: Result<Value, Refusal>,
    ) {
        match result {
            Ok(_) => {
                self.named.push((provider, name));
                self.secrets_left = self.secrets_left.saturating_sub(1);
                if self.secrets_left == 0 && !self.draft.is_empty() {
                    self.send_config();
                }
            }
            Err(refusal) => {
                // 一个没存成：别的存成了的留在密钥库里，配置里的引用照旧。
                self.secrets_left = 0;
                self.failed(refusal.message);
            }
        }
    }

    pub(super) fn saved(&mut self, result: Result<Value, Refusal>, texts: &Texts) {
        match result {
            Ok(_) => {
                // 存成了：界面上看的那一份先当成读来的，等重读回来再换成核心的（不闪一下旧的）。
                let reconnect = self
                    .data
                    .as_ref()
                    .map(|d| self.draft.reconnects(d))
                    .unwrap_or_default();
                self.data = Some(self.view.clone());
                self.draft.clear();
                self.named.clear();
                self.say(texts.status("saved"), Tone::Good);
                self.reload();
                self.refresh();
                // 新加的、换了地址、key 的：马上取模型列表，不用再按 r（2026-10-07 项目主人）。
                for id in reconnect {
                    let name = self
                        .view
                        .provider(&id)
                        .map_or(id.clone(), |p| p.shown().to_string());
                    self.say(
                        texts.status("fetching").replace("{name}", &name),
                        Tone::Busy,
                    );
                    let params = json!({"provider": id, "refresh": true});
                    self.ask("model.list", params, Waiting::Refresh(id, name));
                }
                if std::mem::take(&mut self.close_after_save) {
                    self.popup = None;
                }
            }
            Err(refusal) => {
                // 核心那一句是总的（「配置有几处不对」）：接上第一处具体的。
                let first = refusal.data["problems"][0]["message"].as_str();
                let text = if refusal.reason.as_deref() == Some("config_conflict") {
                    texts.status("conflict").replace(
                        "{current}",
                        &forms::plain(&refusal.data["current"]["value"]),
                    )
                } else if let Some(first) = first {
                    format!("{} {first}", refusal.message)
                } else {
                    refusal.message
                };
                self.failed(text);
            }
        }
    }
}
