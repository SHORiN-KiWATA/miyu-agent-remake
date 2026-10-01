//! `miyu setup` 一步步怎么走（`docs/blueprint/cli/setup.md`「怎么走」第 3 到 11 条，施工 8-11）：选一家（`choose.rs`），
//! 拿 key，试，试不通回到上一步，贴的 key 试通了才存，选模型（`model.rs`），写配置。
//!
//! 贴的 key 只在内存里：先照 `{value}` 试，通了才 `secret.set`（「施工时定的」8-11：试不通的不留下，也不盖掉原来的同名
//! 密钥）。从不印出来。

use std::io::Write;

use serde_json::{Value, json};

use super::SetupPlan;
use super::pick::config_id;
use crate::config::Console;
use crate::exit;
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{Line, say, write};

/// 走一遍时手里的几样。
pub(super) struct Flow<'a> {
    pub(super) rpc: &'a mut Rpc,
    pub(super) plan: &'a SetupPlan,
    pub(super) console: &'a mut dyn Console,
    pub(super) err: &'a mut dyn Write,
}

/// 选了的一家。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Chosen {
    /// 目录、档案里的编号。
    pub(super) id: String,
    /// 给人看的名字。
    pub(super) name: String,
    /// key 从哪来。
    pub(super) key: Key,
}

/// key 从哪来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Key {
    /// 引用核心环境里的这个变量，不复制。
    Env(String),
    /// 要人贴（不在终端里的从管道读）。
    Paste,
    /// 不要 key：本机的服务。
    Nothing,
    /// 已经配好的那一家（配置里的编号）：照配置试，只写 `models.chat`。
    Configured(String),
}

/// 试通了的：列出来的模型、试的那一个（推荐的）。
pub(super) struct Tried {
    pub(super) models: Vec<String>,
    pub(super) model: String,
}

/// 试一次的结局。
enum Attempted {
    /// 通了：贴了的 key（要存），试的结果。
    Worked(Option<String>, Tried),
    /// 不通，回到选一家。
    Again,
}

impl<'a> Flow<'a> {
    /// 照 `plan` 走，问人经 `console`，问的话、印的都在 `err` 上。
    pub(super) fn new(
        rpc: &'a mut Rpc,
        plan: &'a SetupPlan,
        console: &'a mut dyn Console,
        err: &'a mut dyn Write,
    ) -> Flow<'a> {
        Flow {
            rpc,
            plan,
            console,
            err,
        }
    }

    /// 走一遍，交回退出码。
    pub(super) async fn run(&mut self) -> u8 {
        match self.steps().await {
            Ok(()) => exit::OK,
            Err(code) => code,
        }
    }

    async fn steps(&mut self) -> Result<(), u8> {
        let detected = self.request("provider.detect", json!({})).await?;
        let fixed = match self.plan.setup.provider.clone() {
            Some(id) => Some(self.chosen_by_param(&id, &detected).await?),
            None => None,
        };
        let (chosen, pasted, tried) = loop {
            let chosen = match &fixed {
                Some(chosen) => chosen.clone(),
                None => self.choose(&detected).await?,
            };
            match self.attempt(&chosen, fixed.is_some()).await? {
                Attempted::Worked(pasted, tried) => break (chosen, pasted, tried),
                Attempted::Again => {}
            }
        };
        if let Some(key) = pasted {
            self.store(&chosen.id, key).await?;
        }
        let model = match self.plan.setup.model.clone() {
            Some(model) => model,
            None => self.pick_model(&tried)?,
        };
        self.write_config(&chosen, &model).await
    }

    /// 拿 key、试；不通的在终端里回到上一步：贴的 key 回到贴 key，别的回到选一家（`fixed` 的没有上一步）。
    async fn attempt(&mut self, chosen: &Chosen, fixed: bool) -> Result<Attempted, u8> {
        let language = self.plan.language;
        loop {
            let pasted = match chosen.key {
                Key::Paste => Some(self.read_key(&config_id(&chosen.id))?),
                _ => None,
            };
            say(self.err, &language.trying(&chosen.name));
            let mut params = match &chosen.key {
                Key::Configured(id) => json!({"provider": id}),
                Key::Env(name) => {
                    json!({"candidate": {"catalog": chosen.id, "key": {"env": name}}})
                }
                Key::Paste => {
                    json!({"candidate": {"catalog": chosen.id, "key": {"value": pasted}}})
                }
                Key::Nothing => json!({"candidate": {"catalog": chosen.id}}),
            };
            if let Some(model) = &self.plan.setup.model {
                params["model"] = json!(model);
            }
            let tested = self.request("provider.test", params).await?;
            if tested["ok"] == json!(true) {
                let model = tested["model"].as_str().unwrap_or_default().to_string();
                let ms = tested["first_token_ms"].as_u64().unwrap_or_default();
                self.gray(language.it_works(&model, ms));
                if tested["listed"] == json!("catalog") {
                    self.gray(language.listed_from_catalog().to_string());
                }
                let models = tested["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                return Ok(Attempted::Worked(pasted, Tried { models, model }));
            }
            let error = &tested["error"];
            say(
                self.err,
                &language.did_not_work(
                    tested["stage"].as_str().unwrap_or_default(),
                    error["class"].as_str().unwrap_or_default(),
                    error["message"].as_str().unwrap_or_default(),
                ),
            );
            match (self.console.terminal(), &chosen.key, fixed) {
                (false, _, _) => return Err(exit::ERROR),
                (true, Key::Paste, _) => {}
                (true, _, true) => return Err(exit::ERROR),
                (true, _, false) => return Ok(Attempted::Again),
            }
        }
    }

    /// 读贴的 key：标准输入是终端的关掉回显读一行，不是的整份读。去掉前后空白是空的：说「没收到 key」。
    fn read_key(&mut self, name: &str) -> Result<String, u8> {
        let language = self.plan.language;
        let read = match self.console.typed() {
            true => {
                write(self.err, &language.paste_key(name));
                self.console.hidden().map(Option::unwrap_or_default)
            }
            false => self.console.all(),
        };
        match read {
            Ok(key) if !key.trim().is_empty() => Ok(key),
            Ok(_) => {
                say(self.err, language.no_key_given());
                Err(exit::ERROR)
            }
            Err(error) => {
                say(self.err, &error.to_string());
                Err(exit::ERROR)
            }
        }
    }

    /// 贴的 key 试通了：存成密钥，名字是这一家在配置里的编号（和 `miyu login` 同一个方法）。
    async fn store(&mut self, id: &str, key: String) -> Result<(), u8> {
        let name = config_id(id);
        let stored = self
            .request("secret.set", json!({"name": name, "value": key}))
            .await?;
        let replaced = stored["replaced"] == json!(true);
        self.gray(self.plan.language.key_saved(&name, replaced));
        Ok(())
    }

    /// 写系统配置：这一家的 `keys`（已经配好的那一家不写）和 `models.chat`。
    async fn write_config(&mut self, chosen: &Chosen, model: &str) -> Result<(), u8> {
        let (id, mut changes) = match &chosen.key {
            Key::Configured(id) => (id.clone(), Vec::new()),
            key => {
                let id = config_id(&chosen.id);
                let keys = match key {
                    Key::Env(name) => json!([{"env": name}]),
                    Key::Paste => json!([{"secret": id}]),
                    _ => json!([]),
                };
                let mut changes =
                    vec![json!({"key": format!("providers.{id}.keys"), "value": keys})];
                if id != chosen.id {
                    changes.push(
                        json!({"key": format!("providers.{id}.catalog"), "value": chosen.id}),
                    );
                }
                (id, changes)
            }
        };
        let reference = format!("{id}/{model}");
        changes.push(json!({"key": "models.chat", "value": reference}));
        self.request("config.set", json!({"layer": "system", "changes": changes}))
            .await?;
        say(self.err, &self.plan.language.set_up(&reference));
        Ok(())
    }

    /// 问一句，读人敲的一行；读不了、读到头的是空的。
    pub(super) fn ask(&mut self, prompt: &str) -> Option<String> {
        write(self.err, prompt);
        self.console.line().ok().flatten()
    }

    /// 印一行灰字。
    pub(super) fn gray(&mut self, text: String) {
        write(self.err, &Line::gray(text).paint(self.plan.gray));
    }

    /// 发一条请求，交回 `result`；被拒绝的印核心的原话、核心断开的说一句，交回退出码 1。
    pub(super) async fn request(&mut self, method: &str, params: Value) -> Result<Value, u8> {
        link::request(self.rpc, method, params, &self.plan.language, self.err).await
    }
}
