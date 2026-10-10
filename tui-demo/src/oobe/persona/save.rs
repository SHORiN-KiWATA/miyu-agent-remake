//! 建人格那一步的存和读（「第一次打开的引导」第 21–23 条）：「下一步」时新建的一次带上全部，已有的只交改了的（带版本）；
//! 填了头像的先 `blob.put` 传上去，再在 `persona.set` 里带 `avatar`；回应、读回已有的人格都在这里。

use serde_json::{Map, Value, json};

use super::super::asker::{Asker, Waiting};
use super::super::field::Field;
use super::super::texts::Persona as Texts;
use super::super::{Mood, Move};
use super::{Existing, PROMPTS, PersonaStep, Slot};
use crate::core::Refusal;
use crate::pairs::Pair;
use crate::settings::pages::target::said;

impl PersonaStep {
    /// 下一步：名字空着的不走；新建的一次带上四样，已有的只交改了的（带版本）。
    pub(super) fn save(&mut self, asker: &mut Asker, texts: &Texts) {
        let name = self.name.trimmed();
        if name.is_empty() {
            self.error = Some(texts.name_needed.clone());
            self.focus = Slot::Name;
            return;
        }
        self.error = None;
        // 填了头像、还没传的：先传，传上去了再接着存（第 21 条）。
        if self.avatar_blob.is_none()
            && let Some(path) = self.avatar_path()
        {
            self.busy = true;
            // 超了核心的上限、核心不认的格式，先缩小（第 21 条，2026-10-11 项目主人）。
            let path = crate::avatars::shrink::ready(&path);
            let params = json!({"path": path.display().to_string()});
            asker.ask("blob.put", params, Waiting::AvatarPut);
            return;
        }
        let pairs: Vec<Value> = self
            .pairs
            .iter()
            .map(|p| json!({"user": p.user, "assistant": p.assistant}))
            .collect();
        let texts_now = [
            self.prompt.text().trim_end().to_string(),
            self.reminders.text().trim_end().to_string(),
        ];
        let mut params = Map::new();
        let mut prompts = Map::new();
        match &self.existing {
            Some(old) => {
                params.insert("persona".into(), json!(old.id));
                if old.name != name {
                    params.insert(
                        "changes".into(),
                        json!([{"key": "persona.name", "value": name}]),
                    );
                }
                for (i, now) in texts_now.iter().enumerate() {
                    let before = old.texts[i].as_deref().unwrap_or_default().trim_end();
                    if before != now {
                        prompts.insert(
                            PROMPTS[i].into(),
                            json!({"text": now, "expect": old.versions[i]}),
                        );
                    }
                }
                if old.pairs.as_deref().unwrap_or_default() != self.pairs.as_slice() {
                    prompts.insert(
                        "examples".into(),
                        json!({"pairs": pairs, "expect": old.versions[2]}),
                    );
                }
            }
            None => {
                params.insert(
                    "changes".into(),
                    json!([{"key": "persona.name", "value": name}]),
                );
                for (i, now) in texts_now.iter().enumerate() {
                    if !now.is_empty() {
                        prompts.insert(PROMPTS[i].into(), json!({"text": now}));
                    }
                }
                if !pairs.is_empty() {
                    prompts.insert("examples".into(), json!({"pairs": pairs}));
                }
            }
        }
        if !prompts.is_empty() {
            params.insert("prompts".into(), Value::Object(prompts));
        }
        if let Some(blob) = &self.avatar_blob {
            let mut avatar = json!({"blob": blob});
            if let Some(old) = &self.existing {
                avatar["expect"] = json!(old.avatar);
            }
            params.insert("avatar".into(), avatar);
        }
        self.busy = true;
        let unchanged = !params.contains_key("changes")
            && !params.contains_key("prompts")
            && !params.contains_key("avatar");
        match (&self.existing, unchanged) {
            (Some(old), true) => {
                let id = old.id.clone();
                pick(asker, &id);
            }
            _ => asker.ask("persona.set", Value::Object(params), Waiting::PersonaSave),
        }
    }

    /// 等的回来了。
    pub fn answer(
        &mut self,
        waiting: &Waiting,
        result: Result<Value, Refusal>,
        asker: &mut Asker,
        texts: &Texts,
    ) -> Move {
        match waiting {
            Waiting::PersonaDefault => {
                let id = result.ok().and_then(|g| {
                    g["items"]["persona.default"]["value"]
                        .as_str()
                        .map(str::to_string)
                });
                match id {
                    Some(id) => asker.ask(
                        "persona.get",
                        json!({"persona": id}),
                        Waiting::PersonaGet(id),
                    ),
                    None => self.busy = false,
                }
            }
            Waiting::PersonaGet(id) => match result {
                Ok(got) => {
                    self.existing = Some(Existing {
                        id: id.clone(),
                        name: got["name"].as_str().unwrap_or_default().to_string(),
                        avatar: got["avatar"].as_str().map(str::to_string),
                        ..Existing::default()
                    });
                    for prompt in PROMPTS {
                        let params = json!({"persona": id, "prompt": prompt});
                        asker.ask(
                            "persona.read",
                            params,
                            Waiting::PersonaRead(id.clone(), prompt),
                        );
                    }
                }
                Err(_) => self.busy = false,
            },
            Waiting::PersonaRead(_, prompt) => self.read(prompt, result),
            // 头像传上去了：接着存；传不上的照核心的原话写红字，光标回到头像。
            Waiting::AvatarPut => {
                // 传完了，缩小的那份不留。
                crate::avatars::shrink::done();
                match result {
                    Ok(got) => {
                        self.busy = false;
                        self.avatar_blob = got["blob"].as_str().map(str::to_string);
                        if self.avatar_blob.is_some() {
                            self.save(asker, texts);
                        }
                    }
                    Err(refusal) => {
                        self.refused(&refusal);
                        self.focus = Slot::Avatar;
                    }
                }
            }
            Waiting::PersonaSave => match result {
                Ok(got) => {
                    let id = got["persona"].as_str().unwrap_or_default().to_string();
                    pick(asker, &id);
                }
                Err(refusal) => {
                    self.refused(&refusal);
                    // 核心不收这张头像（不是图、太大、传的找不到了）：光标回到头像，重填了再传。
                    if refusal
                        .reason
                        .as_deref()
                        .is_some_and(|r| r.starts_with("avatar") || r == "unknown_attachment")
                    {
                        self.focus = Slot::Avatar;
                        self.avatar_blob = None;
                    }
                }
            },
            Waiting::PersonaPick(id) => match result {
                Ok(_) => {
                    self.busy = false;
                    self.chosen_id = Some(id.clone());
                    let name = self.name.trimmed();
                    self.chosen = Some(name.clone());
                    self.badge = Some(name);
                    return Move::Next;
                }
                Err(refusal) => self.refused(&refusal),
            },
            _ => {}
        }
        Move::Stay
    }

    /// 已有的人格读回来一份：三份都到了就填进四格。
    fn read(&mut self, prompt: &str, result: Result<Value, Refusal>) {
        let Some(old) = self.existing.as_mut() else {
            return;
        };
        let got = result.unwrap_or_default();
        let at = PROMPTS.iter().position(|p| *p == prompt).unwrap_or(0);
        old.versions[at] = got["version"].as_str().map(str::to_string);
        if at < 2 {
            old.texts[at] = got["text"].as_str().map(str::to_string);
        } else {
            let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
            old.pairs = Some(
                got["pairs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| Pair {
                        user: text(&p["user"]),
                        assistant: text(&p["assistant"]),
                    })
                    .collect(),
            );
        }
        old.read += 1;
        if old.read < PROMPTS.len() {
            return;
        }
        let old = old.clone();
        self.name = Field::line().with(&old.name);
        self.prompt = Field::lines().with(old.texts[0].as_deref().unwrap_or_default().trim_end());
        self.reminders =
            Field::lines().with(old.texts[1].as_deref().unwrap_or_default().trim_end());
        self.pairs = old.pairs.clone().unwrap_or_default();
        self.existing_name = Some(old.name.clone());
        self.busy = false;
    }

    /// 被拒：写核心照界面语言给的那一句。
    fn refused(&mut self, refusal: &Refusal) {
        self.busy = false;
        self.error = Some(said(refusal));
        self.mood = Some(Mood::Sad);
    }
}

/// 写成默认人格。
fn pick(asker: &mut Asker, id: &str) {
    let params = json!({"layer": "personal", "changes": [{"key": "persona.default", "value": id}]});
    asker.ask("config.set", params, Waiting::PersonaPick(id.to_string()));
}
