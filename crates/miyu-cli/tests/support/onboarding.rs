//! 第一次接入（施工 8-11）：带模型资料的核心（真目录裁出来的一份、测试给的档案，能拉列表、能探本机），在它上面走一遍
//! `miyu setup`、`miyu ask` 说话之前那一段；人那一头照剧本回。

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_cli::language::Language;
use miyu_cli::{Console, HeadEnv, Setup, SetupPlan, model_ready_on, setup_on};
use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_http::testkit::Server;
use miyu_http::{Proxy, fetcher};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_sandbox::Availability;
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Observed};
use miyu_tool::Catalog as ToolCatalog;

use super::{AccountIdOf, Asked, Home, Tape, dirs, resources, temp_root, within};

/// 照剧本回的人那一头：敲的几行、贴的几个 key（关掉回显读的）、管道里的。记着读了几次。
#[derive(Default)]
pub struct Typist {
    pub terminal: bool,
    pub answers: VecDeque<&'static str>,
    pub keys: VecDeque<String>,
    pub piped: String,
    /// 读过几行、关掉回显读过几次、整份读过几次。
    pub lines: usize,
    pub hidden: usize,
    pub all: usize,
    /// 关掉回显读的时候照取消办（按了 `Ctrl+C`），不照 `keys`（施工 8-5 补）。
    pub cancel_key: bool,
}

impl Typist {
    /// 在终端里，敲 `answers`，贴 `keys`。
    pub fn at_terminal(answers: &[&'static str], keys: &[&str]) -> Typist {
        Typist {
            terminal: true,
            answers: answers.iter().copied().collect(),
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            ..Typist::default()
        }
    }

    /// 不在终端里，管道里是 `piped`。
    pub fn piping(piped: &str) -> Typist {
        Typist {
            piped: piped.to_string(),
            ..Typist::default()
        }
    }
}

impl Console for Typist {
    fn terminal(&self) -> bool {
        self.terminal
    }

    fn line(&mut self) -> io::Result<Option<String>> {
        self.lines += 1;
        Ok(self.answers.pop_front().map(str::to_string))
    }

    fn edit(&mut self, _path: &Path) -> io::Result<Option<i32>> {
        Err(io::Error::other("setup 不开编辑器"))
    }

    fn typed(&self) -> bool {
        self.terminal
    }

    fn hidden(&mut self) -> io::Result<Option<String>> {
        self.hidden += 1;
        match self.cancel_key {
            true => Err(io::Error::new(io::ErrorKind::Interrupted, "test cancel")),
            false => Ok(self.keys.pop_front()),
        }
    }

    fn all(&mut self) -> io::Result<String> {
        self.all += 1;
        Ok(self.piped.clone())
    }
}

/// 假服务器的地址写成 `127.1`（WHATWG 的 IPv4 简写，连的还是本机回环）：本机的服务只认 `127.0.0.1`、`localhost`、`::1`
/// 三种写法，这样它就当成外面的供应商，要贴 key、不去探。
pub fn remote(server: &Server) -> String {
    server.base_url.replace("127.0.0.1", "127.1")
}

/// 档案：出厂的 `[npm]`，加上 `extra` 里的几家（编号 → 档案）。
pub fn profiles(extra: Value) -> Profiles {
    Profiles::parse(&json!({
        "npm": {
            "@ai-sdk/openai-compatible": "openai-chat",
            "@ai-sdk/anthropic": "anthropic",
            "@ai-sdk/openai": "openai-responses"
        },
        "providers": extra,
    }))
    .expect("档案写法对")
}

/// 真目录裁出来的一份、档案 `profiles`，拉列表、探本机都不走代理。
fn data(profiles: Profiles) -> Arc<ModelData> {
    let text = include_str!("../../../miyu-models/testdata/models-dev-trimmed.json");
    let vendors = Vendors::parse(&json!({"deepseek": ["deepseek"]})).expect("读得进");
    let data = ModelData::new(profiles, vendors, None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"))
        .with_local(fetcher(Proxy::Off).expect("造得出客户端"));
    data.loaded(
        Some(Loaded {
            catalog: Catalog::parse(text).expect("读得进").catalog,
            source: CatalogSource::Snapshot,
            fetched: "2026-10-01T03:25:54.000Z".to_string(),
        }),
        Observed::default(),
    );
    Arc::new(data)
}

/// 中文、不上色、头这边的环境里设了 `here`，参数是 `setup`。
pub fn plan(setup: Setup, here: &[&str]) -> SetupPlan {
    SetupPlan {
        setup,
        language: Language::Chinese,
        gray: false,
        here: HeadEnv::of(here),
    }
}

impl Home {
    /// 起一个核心：系统配置是 `config`（空的不写），核心的环境是 `env`，档案照 `extra`，清单照核心登记的全部。
    pub fn onboarding(config: &str, env: &[(&str, &str)], extra: Value) -> Home {
        let (dir, root) = temp_root();
        if !config.is_empty() {
            let file = root.path().join("system").join("config.toml");
            std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
            std::fs::write(&file, config).expect("写得进");
        }
        let opened = miyu_ipc::open(&root, &dirs()).expect("起得来");
        let admin = AccountIdOf::admin();
        let loaded = Config::load(
            &root,
            &admin,
            None,
            miyu_core::settings::items(),
            Environment::of(env),
        );
        let core = Core::new(
            root.clone(),
            resources(),
            Arc::new(Script::new([])),
            ToolCatalog::default(),
            None,
            admin,
            opened.token.clone(),
        )
        .with_sandbox(Availability::Usable(PathBuf::from("miyu-sandbox")))
        .with_config(loaded)
        .with_model_data(data(profiles(extra)));
        let core = Arc::new(core);
        let running = tokio::spawn(miyu_endpoint::run(opened.listener, Arc::clone(&core)));
        Home {
            dir,
            root,
            core,
            running,
        }
    }

    /// 在真的套接字上连上核心，照 `plan` 走一遍 `miyu setup`，人那一头是 `console`。
    pub async fn setup(&self, plan: &SetupPlan, console: &mut dyn Console) -> Asked {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let mut err = tape.pen(true);
        let code = within(
            "走完",
            setup_on(connection, &token, plan, console, &mut err),
        )
        .await;
        Asked {
            code,
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        }
    }

    /// `miyu ask` 说话之前那一段：有模型的 `Ok`，没有的在终端里先走一遍 setup。
    pub async fn ready(
        &self,
        plan: &SetupPlan,
        console: &mut dyn Console,
    ) -> (Result<(), u8>, Asked) {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let mut err = tape.pen(true);
        let ready = within(
            "看完",
            model_ready_on(connection, &token, plan, console, &mut err),
        )
        .await;
        let asked = Asked {
            code: ready.err().unwrap_or(0),
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        };
        (ready, asked)
    }

    /// 系统配置现在的字。
    pub fn system_config(&self) -> String {
        std::fs::read_to_string(self.root.path().join("system/config.toml")).unwrap_or_default()
    }

    /// 密钥文件现在的字。
    pub fn secrets(&self) -> String {
        std::fs::read_to_string(self.root.path().join("system/secrets.toml")).unwrap_or_default()
    }
}
