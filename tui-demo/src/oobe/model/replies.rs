//! 接模型那一步收到的回应（「第一次打开的引导」第 16–20 条）：三份读回来排好一行行，试的结果，存 key、写配置的结果。

use serde_json::Value;

use super::rows::{self, Row};
use super::{Form, ModelStep, Models, Phase, Slot};
use crate::core::Refusal;
use crate::oobe::asker::{Asker, Waiting};
use crate::oobe::texts::Model as Texts;
use crate::oobe::{Mood, Move};
use crate::settings::pages::target::said;

impl ModelStep {
    /// 等的回来了。
    pub fn answer(
        &mut self,
        waiting: &Waiting,
        result: Result<Value, Refusal>,
        asker: &mut Asker,
        texts: &Texts,
    ) -> Move {
        match waiting {
            Waiting::Models => {
                let list = result.unwrap_or_default();
                let data = {
                    let mut data = crate::settings::data::Data::default();
                    data.read_models(&list);
                    data
                };
                self.configured = data.providers.iter().map(|p| p.id.clone()).collect();
                self.chat = data.chat.as_ref().map(|chat| {
                    data.model(chat)
                        .and_then(|(_, m)| m.fact("name").as_str().map(str::to_string))
                        .unwrap_or_else(|| chat.clone())
                });
                self.list = Some(list);
            }
            Waiting::Detect => self.detect = Some(result.unwrap_or_default()),
            Waiting::Catalog => self.catalog = Some(result.unwrap_or_default()),
            Waiting::AllProviders => {
                let got = result.unwrap_or_default();
                let detect = self.detect.clone().unwrap_or_default();
                if let Some(more) = self.more.as_mut() {
                    more.loaded(super::rows::providers(&got, &detect, &self.configured));
                }
                return Move::Stay;
            }
            Waiting::Pools => {
                let got = result.unwrap_or_default();
                let has = got["items"]
                    .as_object()
                    .is_some_and(|items| items.keys().any(|k| k.starts_with("pools.")));
                self.pools = Some(has);
            }
            Waiting::Test => return self.tested(result, texts),
            Waiting::Secret => return self.secret_saved(result, asker),
            Waiting::SaveModel => return self.saved(result),
            _ => {}
        }
        self.loaded();
        Move::Stay
    }

    /// 三份都回来了：排好一行行，有聊天模型的问用它还是换。
    fn loaded(&mut self) {
        if !matches!(self.phase, Phase::Loading) {
            return;
        }
        let (Some(catalog), Some(detect), Some(_)) = (&self.catalog, &self.detect, &self.list)
        else {
            return;
        };
        self.rows = rows::rows(catalog, detect, &self.configured);
        self.cursor = self.rows.iter().position(Row::selectable).unwrap_or(0);
        self.phase = if self.chat.is_some() {
            Phase::Ready(true)
        } else {
            Phase::List
        };
    }

    fn tested(&mut self, result: Result<Value, Refusal>, texts: &Texts) -> Move {
        let Phase::Testing(mut form) = std::mem::take(&mut self.phase) else {
            return Move::Stay;
        };
        let got = match result {
            Ok(got) => got,
            Err(refusal) => {
                self.failed(form, said(&refusal));
                return Move::Stay;
            }
        };
        if got["ok"] == true {
            let tested = got["model"].as_str().unwrap_or_default().to_string();
            let mut names: Vec<String> = vec![tested.clone()];
            names.extend(
                got["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|n| *n != tested)
                    .map(str::to_string),
            );
            names.retain(|n| !n.is_empty());
            self.warn = None;
            self.mood = Some(Mood::Happy);
            self.phase = Phase::Models(Models {
                form,
                names,
                ..Models::default()
            });
            return Move::Stay;
        }
        let stage = got["stage"].as_str().unwrap_or_default();
        if stage == "list" && form.model.trimmed().is_empty() {
            form.needs_model = true;
            // 光标停到「模型名」那一格（最后一行是「测试连接」）。
            form.focus = form
                .slots()
                .iter()
                .position(|s| *s == Slot::Model)
                .unwrap_or(0);
            self.warn = Some(texts.no_list.clone());
            self.mood = Some(Mood::Calm);
            self.phase = Phase::Form(form);
            return Move::Stay;
        }
        let message = got["error"]["message"].as_str().unwrap_or_default();
        let stage = texts.stages.get(stage).map_or(stage, String::as_str);
        let line = texts
            .failed
            .replace("{stage}", stage)
            .replace("{message}", message);
        self.failed(form, line);
        Move::Stay
    }

    /// 没试通：红字；有格子填的回到那一屏（光标回到密钥或者地址），没有的回到选一家。
    fn failed(&mut self, mut form: Form, line: String) {
        self.error = Some(line);
        self.mood = Some(Mood::Sad);
        let slots = form.slots();
        if slots.is_empty() {
            self.phase = Phase::List;
            return;
        }
        form.focus = slots.iter().position(|s| *s == Slot::Key).unwrap_or(0);
        self.phase = Phase::Form(form);
    }

    fn secret_saved(&mut self, result: Result<Value, Refusal>, asker: &mut Asker) -> Move {
        let Phase::Saving(models) = std::mem::take(&mut self.phase) else {
            return Move::Stay;
        };
        if let Err(refusal) = result {
            self.error = Some(said(&refusal));
            self.phase = Phase::Models(models);
            return Move::Stay;
        }
        let name = self.chosen.clone().unwrap_or_default();
        if let Some(target) = models.form.target() {
            self.write(&target, &models.form.key.trimmed(), &name, asker);
        }
        self.phase = Phase::Saving(models);
        Move::Stay
    }

    fn saved(&mut self, result: Result<Value, Refusal>) -> Move {
        let Phase::Saving(models) = std::mem::take(&mut self.phase) else {
            return Move::Stay;
        };
        match result {
            Ok(_) => {
                self.phase = Phase::Models(models);
                Move::Next
            }
            Err(refusal) => {
                self.error = Some(said(&refusal));
                self.phase = Phase::Models(models);
                Move::Stay
            }
        }
    }
}
