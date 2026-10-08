//! `miyu setup` 一步步怎么走（`docs/blueprint/cli/setup.md`「怎么走」第 3 到 11 条，施工 8-11、8-11 再补）：选一家
//! （`choose.rs`，自定义的在 `custom.rs`），拿 key，试；取不到模型列表的印「未获取到模型列表」、让人填模型名再试；试不通回到
//! 上一步，贴的 key 试通了才存，选模型（`model.rs`），写配置。
//!
//! 贴的 key 只在内存里：先照 `{value}` 试，通了才 `secret.set`（「施工时定的」8-11：试不通的不留下，也不盖掉原来的同名
//! 密钥）。从不印出来。
//!
//! 配置里一个池都没有的，写配置时一起写三个预设的池（施工 8-8 补，[`PRESET_POOLS`]）。

use std::io::{self, Write};

use serde_json::{Value, json};

use super::SetupPlan;
use super::pick::config_id;
use crate::config::Console;
use crate::exit;
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{Line, say, write};

/// 预先建好的三个池（施工 8-8 补，`docs/blueprint/models.md` 第七条第 5 条第 7 款）：成员是空的，开关开着，不带说明。填了
/// 成员才出现在子代理的选项里；除了是预先建好的，它们是普通的池，能删、能改名（2026-10-01 项目主人定）。
const PRESET_POOLS: [&str; 3] = ["lite", "standard", "flagship"];

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
    /// 目录、档案里的编号；自定义的是照主机名起的，试通了再照配置里有没有撞改（`custom.rs`）。
    pub(super) id: String,
    /// 给人看的名字。
    pub(super) name: String,
    /// key 从哪来。
    pub(super) key: Key,
    /// 自定义的（施工 8-11 再补）：地址、驱动。目录、档案里的没有。
    pub(super) custom: Option<Custom>,
}

/// 自定义的一家：地址、驱动的写法。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Custom {
    pub(super) base_url: String,
    pub(super) driver: String,
}

/// key 从哪来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Key {
    /// 引用核心环境里的这个变量，不复制。
    Env(String),
    /// 要人贴（不在终端里的从管道读）。
    Paste,
    /// 自定义的：要人贴，直接回车是不要 key（施工 8-11 再补）。
    Optional,
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

/// `provider.test` 的参数：配好了的照配置，自定义的带驱动、地址，别的照目录里的编号；贴的 key 照 `{value}` 交（不存、不记），
/// 写了模型的试它。
fn test_params(chosen: &Chosen, pasted: Option<&str>, model: Option<&str>) -> Value {
    let mut params = match (&chosen.key, &chosen.custom) {
        (Key::Configured(id), _) => json!({"provider": id}),
        (_, Some(custom)) => {
            json!({"candidate": {"driver": custom.driver, "base_url": custom.base_url}})
        }
        (Key::Env(name), None) => {
            json!({"candidate": {"catalog": chosen.id, "key": {"env": name}}})
        }
        (_, None) => json!({"candidate": {"catalog": chosen.id}}),
    };
    if let (Some(key), Some(candidate)) = (pasted, params.get_mut("candidate")) {
        candidate["key"] = json!({"value": key});
    }
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    params
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
        let (mut chosen, pasted, tried) = loop {
            let chosen = match &fixed {
                Some(chosen) => chosen.clone(),
                None => self.choose(&detected).await?,
            };
            match self.attempt(&chosen, fixed.is_some()).await? {
                Attempted::Worked(pasted, tried) => break (chosen, pasted, tried),
                Attempted::Again => {}
            }
        };
        if chosen.custom.is_some() {
            chosen.id = self.fresh_id(&chosen.id).await?;
        }
        let stored = pasted.is_some();
        if let Some(key) = pasted {
            self.store(&chosen.id, key).await?;
        }
        let model = match self.plan.setup.model.clone() {
            Some(model) => model,
            None => self.pick_model(&tried)?,
        };
        self.write_config(&chosen, stored, &model).await
    }

    /// 拿 key、试；取不到模型列表的问模型名、拿它再试（施工 8-11 再补）；不通的在终端里回到上一步：贴的 key 回到贴 key，别的
    /// 回到选一家（`fixed` 的没有上一步）。
    async fn attempt(&mut self, chosen: &Chosen, fixed: bool) -> Result<Attempted, u8> {
        let language = self.plan.language;
        loop {
            let pasted = match chosen.key {
                Key::Paste => Some(self.read_key(&config_id(&chosen.id), false)?),
                Key::Optional => Some(self.read_key(&config_id(&chosen.id), true)?)
                    .filter(|key| !key.trim().is_empty()),
                _ => None,
            };
            let mut model = self.plan.setup.model.clone();
            loop {
                say(self.err, &language.trying(&chosen.name));
                let params = test_params(chosen, pasted.as_deref(), model.as_deref());
                let tested = self.request("provider.test", params).await?;
                if tested["ok"] == json!(true) {
                    let typed = model.is_some() && self.plan.setup.model.is_none();
                    return Ok(Attempted::Worked(pasted, self.worked(&tested, typed)));
                }
                if tested["stage"] == json!("list") && model.is_none() && self.console.terminal() {
                    say(self.err, language.no_model_list());
                    match self
                        .ask(language.model_name())
                        .map(|line| line.trim().to_string())
                    {
                        Some(name) if !name.is_empty() => {
                            model = Some(name);
                            continue;
                        }
                        _ => return self.not_picked(),
                    }
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
                    (true, Key::Paste | Key::Optional, _) => break,
                    (true, _, true) => return Err(exit::ERROR),
                    (true, _, false) => return Ok(Attempted::Again),
                }
            }
        }
    }

    /// 试通了：印一行灰字，交回列出来的模型和试的那一个。`typed` 的（取不到列表、人填了模型名的）不再列、不再问：交回的
    /// 列表是空的。
    fn worked(&mut self, tested: &Value, typed: bool) -> Tried {
        let language = self.plan.language;
        let model = tested["model"].as_str().unwrap_or_default().to_string();
        let ms = tested["first_token_ms"].as_u64().unwrap_or_default();
        self.gray(language.it_works(&model, ms));
        if typed {
            return Tried {
                models: Vec::new(),
                model,
            };
        }
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
        Tried { models, model }
    }

    /// 说「没选」，交回退出码 1。
    pub(super) fn not_picked<T>(&mut self) -> Result<T, u8> {
        say(self.err, self.plan.language.not_picked());
        Err(exit::ERROR)
    }

    /// 读贴的 key：标准输入是终端的关掉回显读一行，不是的整份读。去掉前后空白是空的：`optional` 的（自定义的，施工 8-11
    /// 补）是不要 key，别的说「没收到 key」。按了 `Ctrl+C`、或者空行按了 `Ctrl+D`：取消，整个 `miyu setup` 照取消办（施工
    /// 8-5 补）。
    fn read_key(&mut self, name: &str, optional: bool) -> Result<String, u8> {
        let language = self.plan.language;
        let read = match self.console.typed() {
            true => {
                let prompt = match optional {
                    true => language.paste_key_optional().to_string(),
                    false => language.paste_key(name),
                };
                write(self.err, &prompt);
                self.console.hidden().map(Option::unwrap_or_default)
            }
            false => self.console.all(),
        };
        match read {
            Ok(key) if optional || !key.trim().is_empty() => Ok(key),
            Ok(_) => {
                say(self.err, language.no_key_given());
                Err(exit::ERROR)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                say(self.err, language.key_paste_cancelled());
                Err(exit::CANCELLED)
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

    /// 写系统配置：这一家的 `keys`（已经配好的那一家不写）和 `models.chat`；自定义的另写 `driver`、`base_url`（`stored` 是
    /// 存了贴的 key）；配置里一个池都没有的，同一次一起写三个预设的池。
    async fn write_config(&mut self, chosen: &Chosen, stored: bool, model: &str) -> Result<(), u8> {
        let (id, mut changes) = match (&chosen.key, &chosen.custom) {
            (Key::Configured(id), _) => (id.clone(), Vec::new()),
            (_, Some(custom)) => {
                let id = chosen.id.clone();
                let keys = match stored {
                    true => json!([{"secret": id}]),
                    false => json!([]),
                };
                let changes = vec![
                    json!({"key": format!("providers.{id}.driver"), "value": custom.driver}),
                    json!({"key": format!("providers.{id}.base_url"), "value": custom.base_url}),
                    json!({"key": format!("providers.{id}.keys"), "value": keys}),
                ];
                (id, changes)
            }
            (key, None) => {
                let id = config_id(&chosen.id);
                let keys = match key {
                    Key::Env(name) => json!([{"env": name}]),
                    Key::Paste | Key::Optional => json!([{"secret": id}]),
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
        if !self.has_pools().await? {
            for pool in PRESET_POOLS {
                changes.push(json!({"key": format!("pools.{pool}.models"), "value": []}));
                changes.push(json!({"key": format!("pools.{pool}.subagent"), "value": true}));
            }
        }
        self.request("config.set", json!({"layer": "system", "changes": changes}))
            .await?;
        say(self.err, &self.plan.language.set_up(&reference));
        Ok(())
    }

    /// 配置里有没有池（施工 8-8 补）：`config.get` 不带 `cwd`、`keys`（系统、个人合出来的；项目配置里本来不能写池），
    /// `items` 里有 `pools.` 开头的键就算有。
    async fn has_pools(&mut self) -> Result<bool, u8> {
        let got = self.request("config.get", json!({})).await?;
        Ok(got["items"]
            .as_object()
            .is_some_and(|items| items.keys().any(|key| key.starts_with("pools."))))
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
