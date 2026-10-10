//! 「没装内置语义模型」那一行（施工 F-5 再补，`docs/blueprint/recall.md` 第四条第 1 款）：只有核心起来时那一路
//! （`embed::setup`）记，装卸时照清单拼的（`embed::find`）不记。装日志订阅者：自己一个程序（调用点记下谁在听是全进程的）。

use std::path::Path;

use miyu_core::embed::{find, setup};
use miyu_store::env::{Env, Platform};
use miyu_store::packages::{Found, Issue, Layer};

/// 人格记忆的清单：推荐内置语义模型。
const MEMORY: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Persona memory\" }\n\n[recommends]\nworkers = [\"embed\"]\n\n[builtin]\n";

fn memory(dir: &Path) -> Found {
    Found {
        id: "memory".to_string(),
        layer: Layer::Home,
        path: dir.join("memory.toml"),
        read: miyu_config::package::read(MEMORY).map_err(Issue::Wrong),
    }
}

fn env() -> Env {
    Env {
        platform: Platform::current(),
        miyu_home: None,
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    }
}

#[test]
fn only_startup_logs_the_missing_package() {
    let dir = std::env::temp_dir();
    let log = miyu_log::Memory::new();
    tracing::subscriber::with_default(
        miyu_log::subscriber(log.clone(), miyu_log::LevelFilter::INFO, None),
        || {
            assert!(find(&env(), &[&memory(&dir)]).is_err());
            assert!(log.lines().is_empty(), "装卸那一路不记：{:?}", log.lines());
            assert!(setup(&env(), &[memory(&dir)]).is_none());
        },
    );
    let lines = log.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("embedder unavailable") && lines[0].contains("no embed package"),
        "{lines:?}"
    );
}
