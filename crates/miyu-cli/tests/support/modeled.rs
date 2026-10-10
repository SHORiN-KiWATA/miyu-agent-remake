//! 配了模型这一块的核心（施工 8-10）：`miyu ask --model` 要核心照配置解析引用。系统配置照测试给的字写进数据根，清单照核心
//! 登记的全部，环境变量一个都不看；请求模型照给的端口，没有工具，沙盒当能用。请求模型是真路由的（[`Home::routed`]，施工
//! R-7 补）：整理记忆经它的一次性入口发给假服务器。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_http::{Proxy, client};
use miyu_sandbox::Availability;
use miyu_session::{Models, Routes};
use miyu_tool::Catalog;

use super::onboarding::{data, profiles};
use super::{AccountIdOf, Home, dirs, resources, temp_root};

impl Home {
    /// 起一个核心：系统配置是 `config`，请求模型是真的路由（供应商写在 `config` 里，指到假服务器）。
    pub fn routed(config: &str) -> Home {
        let routes = Routes {
            client: client(Proxy::Off).expect("造得出客户端"),
            direct: client(Proxy::Off).expect("造得出客户端"),
            data: data(profiles(json!({}))),
            idle: Duration::from_secs(60),
        };
        Home::configured(Arc::new(routes), config)
    }

    /// 起一个核心：系统配置是 `config`，请求模型照 `models`。
    pub fn configured(models: Arc<dyn Models>, config: &str) -> Home {
        let (dir, root) = temp_root();
        let file = root.path().join("system").join("config.toml");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
        std::fs::write(&file, config).expect("写得进");
        let opened = miyu_ipc::open(&root, &dirs()).expect("起得来");
        let admin = AccountIdOf::admin();
        let loaded = Config::load(
            &root,
            &admin,
            None,
            miyu_core::settings::items(),
            Environment::of(&[]),
        );
        let core = Core::new(
            root.clone(),
            resources(),
            models,
            Catalog::default(),
            None,
            admin,
            opened.token.clone(),
        )
        .with_sandbox(Availability::Usable(PathBuf::from("miyu-sandbox")))
        .with_config(loaded);
        let core = Arc::new(core);
        let running = tokio::spawn(miyu_endpoint::run(opened.listener, Arc::clone(&core)));
        Home {
            dir,
            root,
            core,
            running,
        }
    }
}
