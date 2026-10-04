//! 全屏配置页的状态；布局在 ui/settings.rs。读写配置均通过异步 IPC，详见 tui.md「全屏配置页」。

mod actions;
pub mod data;
pub mod forms;
mod keys;
mod options;

use crate::core::Command;
use crate::input::Editor;
use data::{Model, Pool, Provider};
use forms::{Entity, Form, Kind};
use ratatui::layout::Rect;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// 配置页的一次编辑，无 Debug，密钥不进入日志。
pub struct Editing {
    /// 在草稿中的哪一格。
    pub field: usize,
    /// 文字编辑状态。
    pub editor: Editor,
    /// 有选项时显示列表，空数组表示自由输入。
    pub options: Vec<String>,
    /// 单选当前项。
    pub selected: usize,
    /// 多选能力当前状态。
    pub checked: Option<Vec<bool>>,
    /// 密钥输入，绘制掩码。
    pub secret: bool,
}

enum Pending {
    Models,
    Config,
    Write,
    Secret,
    Test,
}

/// 配置页的选择和草稿；开关页面不改对话状态。
pub struct Settings {
    /// 独立进程，关闭页时退出程序。
    pub standalone: bool,
    /// 四个分页，0 到 3。
    pub tab: usize,
    /// 当前列表/表单行。
    pub selected: usize,
    /// 选中的供应商。
    pub provider: usize,
    /// 供应商页左/右栏。
    pub column: usize,
    /// 核心返回的供应商全部模型。
    pub providers: Vec<Provider>,
    /// 核心返回的池，空池照列。
    pub pools: Vec<Pool>,
    /// 最后一次 config.get（all）回应。
    pub config: Value,
    /// 默认用途引用。
    pub uses: Value,
    /// 页内草稿，后台更新不覆盖。
    pub form: Option<Form>,
    /// 当前字段编辑。
    pub editing: Option<Editing>,
    /// 状态文字，原样显示核心的错误。
    pub note: String,
    /// 核心已连上。
    pub connected: bool,
    /// 数据已经读过，初次连接前不允许保存。
    pub loaded: bool,
    /// 绘制重建的鼠标命中区域，动作与键盘共用处理。
    pub hits: Vec<(Rect, Hit)>,
    models_loaded: bool,
    config_loaded: bool,
    outbox: Vec<Command>,
    pending: HashMap<u64, Pending>,
    refresh_pending: Option<bool>,
}

/// 鼠标命中的动作。
#[derive(Clone, Copy)]
pub enum Hit {
    /// 底栏的按键动作，和键盘共用处理。
    Action(ratatui::crossterm::event::KeyCode),
    /// 分页。
    Tab(usize),
    /// 供应商。
    Provider(usize),
    /// 列表、表单或模型行。
    Row(usize),
    /// 编辑选项。
    Option(usize),
}

impl Settings {
    /// 打开配置页，独立模式不恢复会话。
    pub fn new(standalone: bool, connected: bool) -> Self {
        let mut page = Self {
            standalone,
            connected,
            tab: 0,
            selected: 0,
            provider: 0,
            column: 0,
            providers: Vec::new(),
            pools: Vec::new(),
            config: Value::Null,
            uses: Value::Null,
            form: None,
            editing: None,
            note: String::new(),
            loaded: false,
            hits: Vec::new(),
            models_loaded: false,
            config_loaded: false,
            outbox: Vec::new(),
            pending: HashMap::new(),
            refresh_pending: None,
        };
        page.fetch(false);
        page
    }

    /// 已经发起保存或测试，重复按键不重复写入。
    pub fn busy(&self) -> bool {
        self.pending
            .values()
            .any(|p| matches!(p, Pending::Write | Pending::Secret | Pending::Test))
    }

    /// 每个页面实例共用递增编号，旧页面的迟到回应不会落到新页面。
    fn ask(&mut self, method: &'static str, params: Value, kind: Pending) {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let tag = NEXT.fetch_add(1, Ordering::Relaxed);
        self.pending.insert(tag, kind);
        self.outbox.push(Command::SettingsRpc {
            tag,
            method,
            params,
        });
    }

    /// 初次、重连或外部修改后重读；保存中的草稿仍用原快照。
    pub fn fetch(&mut self, refresh: bool) {
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Models | Pending::Config))
        {
            self.refresh_pending = Some(self.refresh_pending.unwrap_or(false) || refresh);
        }
        if !self.pending.values().any(|p| matches!(p, Pending::Models)) {
            self.ask("model.list", json!({"refresh":refresh}), Pending::Models);
        }
        if !self.pending.values().any(|p| matches!(p, Pending::Config)) {
            self.ask("config.get", json!({"all":true}), Pending::Config);
        }
    }

    /// 将已构造的 IPC 请求交给 App，不对敏感参数 Debug。
    pub fn outgoing(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.outbox)
    }

    /// 回复到达；失败不抹草稿、不显示成功。
    pub fn replied(
        &mut self,
        tag: u64,
        result: Result<Value, String>,
        texts: &HashMap<String, String>,
    ) {
        let Some(kind) = self.pending.remove(&tag) else {
            return;
        };
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                self.note = if matches!(kind, Pending::Write)
                    && self.form.as_ref().is_some_and(|f| {
                        f.fields.iter().any(|v| {
                            matches!(v.kind, Kind::Key) && v.value.starts_with("$secret:tui-")
                        })
                    }) {
                    text(texts, "key_saved_config_failed").replace("{reason}", &error)
                } else {
                    error
                };
                if matches!(kind, Pending::Secret)
                    && let Some(f) = &mut self.form
                    && let Some(key) = f.fields.iter_mut().find(|f| matches!(f.kind, Kind::Key))
                {
                    key.value = key.original.clone();
                }
                self.followup();
                return;
            }
        };
        match kind {
            Pending::Models => {
                let id = self.providers.get(self.provider).map(|p| p.id.clone());
                self.providers = data::providers(&value);
                self.pools = data::pools(&value);
                self.uses = value["uses"].clone();
                self.provider = id
                    .and_then(|id| self.providers.iter().position(|p| p.id == id))
                    .unwrap_or(0);
                self.models_loaded = true;
                self.loaded = self.config_loaded;
                self.clamp();
            }
            Pending::Config => {
                self.config = value;
                self.config_loaded = true;
                self.loaded = self.models_loaded;
            }
            Pending::Secret => self.write(texts),
            Pending::Write => {
                self.form = None;
                self.editing = None;
                self.selected = 0;
                self.note = text(texts, "saved").into();
                self.fetch(false);
            }
            Pending::Test => {
                if value["ok"] == true {
                    self.note = text(texts, "test_ok").into();
                    self.fetch(true);
                } else {
                    self.note = text(texts, "test_failed").replace(
                        "{reason}",
                        value["error"]["message"]
                            .as_str()
                            .or_else(|| value["error"].as_str())
                            .unwrap_or_default(),
                    );
                }
            }
        }
        self.followup();
    }

    // 读请求在变更通知以前取到了旧快照，也要在完成后再取一次，不丢掉刷新意图。
    fn followup(&mut self) {
        if !self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Models | Pending::Config))
            && let Some(refresh) = self.refresh_pending.take()
        {
            self.fetch(refresh);
        }
    }

    /// 单模型的默认列表；视觉列表只列明确支持图像的模型。
    pub fn models(&self) -> Vec<(&Provider, &Model)> {
        self.providers
            .iter()
            .flat_map(|p| p.models.iter().map(move |m| (p, m)))
            .filter(|(_, m)| self.tab != 2 || data::inputs(m).iter().any(|v| v == "image"))
            .collect()
    }

    /// 当前页有几行，编辑池时含两项字段和所有模型成员。
    pub fn count(&self) -> usize {
        if let Some(f) = &self.form {
            f.fields.len()
                + if matches!(f.entity, Entity::Pool { .. }) {
                    self.models().len()
                } else {
                    0
                }
        } else if self.tab == 0 {
            if self.column == 0 {
                self.providers.len()
            } else {
                self.providers
                    .get(self.provider)
                    .map_or(0, |p| p.models.len())
            }
        } else if self.tab == 3 {
            self.pools.len()
        } else {
            self.models().len()
        }
    }

    /// 后台刷新减少列表时限制选择，不越界访问。
    pub fn clamp(&mut self) {
        self.provider = self.provider.min(self.providers.len().saturating_sub(1));
        self.selected = self.selected.min(self.count().saturating_sub(1));
    }
}

/// 文案全部来自资源；缺少资源时显示字段名，避免 panic。
pub fn text<'a>(texts: &'a HashMap<String, String>, key: &'a str) -> &'a str {
    texts.get(key).map_or(key, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_replies_do_not_enter_another_page_and_rejection_keeps_the_form() {
        let mut a = Settings::new(false, true);
        let commands = a.outgoing();
        let mut b = Settings::new(false, true);
        let Command::SettingsRpc { tag, .. } = &commands[0] else {
            panic!()
        };
        b.replied(
            *tag,
            Ok(json!({"providers":[{"id":"old","models":[]}]})),
            &HashMap::new(),
        );
        assert!(b.providers.is_empty());
        b.loaded = true;
        b.form = Some(forms::pool(&json!({}), None));
        if let Some(f) = &mut b.form {
            f.fields[0].value = "daily".into();
            f.fields[1].value = "pin".into();
        }
        b.save(&HashMap::new());
        let pending = b.outgoing();
        let Command::SettingsRpc { tag, .. } = pending.last().unwrap() else {
            panic!()
        };
        b.replied(*tag, Err("config_conflict".into()), &HashMap::new());
        assert!(b.form.is_some());
        assert_eq!(b.note, "config_conflict");
    }
    #[test]
    fn connection_test_displays_the_original_provider_error() {
        let mut page = Settings::new(false, true);
        page.providers = data::providers(&json!({"providers":[{"id":"test","models":[]}]}));
        page.outgoing();
        page.test(&HashMap::new());
        let Command::SettingsRpc { tag, .. } = page.outgoing().pop().unwrap() else {
            panic!()
        };
        let texts = HashMap::from([("test_failed".into(), "失败：{reason}".into())]);
        page.replied(
            tag,
            Ok(json!({"ok":false,"error":{"message":"模型不存在","class":"not_found"}})),
            &texts,
        );
        assert!(page.note.contains("模型不存在"));
    }
    #[test]
    fn a_failed_secret_save_does_not_remove_the_existing_provider_key_on_retry() {
        let mut page = Settings::new(false, true);
        page.loaded = true;
        page.outgoing();
        let config = json!({"items":{"providers.p.keys":{"value":[{"env":"OLD_KEY"}],"origin":{"layer":"personal"}}}});
        let provider = Provider {
            id: "p".into(),
            name: "p".into(),
            models: vec![],
        };
        let mut form = forms::provider(&config, Some(&provider));
        form.fields[1].value = "https://new.invalid/v1".into();
        form.fields[3].value = "replacement-key".into();
        page.form = Some(form);
        page.save(&HashMap::new());
        let Command::SettingsRpc { tag, method, .. } = page.outgoing().pop().unwrap() else {
            panic!()
        };
        assert_eq!(method, "secret.set");
        page.replied(tag, Err("secret write failed".into()), &HashMap::new());
        let changes = page.form.as_ref().unwrap().changes();
        assert!(changes.iter().all(|c| c["key"] != "providers.p.keys"));
        assert!(changes.iter().any(|c| c["key"] == "providers.p.base_url"));
    }

    #[test]
    fn changes_received_during_a_read_trigger_a_followup_snapshot() {
        let mut page = Settings::new(false, true);
        let reads = page.outgoing();
        page.fetch(false);
        for request in reads {
            let Command::SettingsRpc { tag, method, .. } = request else {
                panic!()
            };
            let response = if method == "model.list" {
                json!({"providers":[]})
            } else {
                json!({"items":{}})
            };
            page.replied(tag, Ok(response), &HashMap::new());
        }
        let followup = page.outgoing();
        assert!(followup.iter().any(|r| matches!(
            r,
            Command::SettingsRpc {
                method: "config.get",
                ..
            }
        )));
        assert!(followup.iter().any(|r| matches!(
            r,
            Command::SettingsRpc {
                method: "model.list",
                ..
            }
        )));
    }
    #[test]
    fn testing_a_selected_model_targets_that_model_and_provider() {
        let mut page = Settings::new(false, true);
        page.providers = data::providers(&json!({"providers":[{"id":"p","models":[
            {"model":"alpha","ref":"p/alpha"},{"model":"beta","ref":"p/beta"}]}]}));
        page.outgoing();
        page.column = 1;
        page.selected = 1;
        page.test(&HashMap::new());
        let Command::SettingsRpc { method, params, .. } = page.outgoing().pop().unwrap() else {
            panic!()
        };
        assert_eq!(method, "provider.test");
        assert_eq!(params, json!({"provider":"p","model":"beta"}));
    }
}
