//! 装卸当场生效（施工 F-5 中，`docs/blueprint/packages.md`「装卸」，设计 30 第九节）：真核心走一遍。卸掉一个内置包，它的工具
//! 当场出目录、新开的会话没有，用过它的会话照旧留着、调到时报「已卸载」，它登记的查询当没有；装回来，工具、查询都回来。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::builtins::Builtins;
use miyu_endpoint::queries::Queries;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::packages::Found;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 测试用的内置包：一件工具 `xprobe`，一个查询 `xkit.ping`。
const XKIT: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"X kit\" }\n\n[builtin]\n";

/// 照清单给内置包 `xkit` 的工具：装着才有。
struct Kit;

impl Builtins for Kit {
    fn tools(&self, found: &[Found]) -> Result<Vec<miyu_endpoint::builtins::Group>, String> {
        let installed = miyu_endpoint::packages::is_installed(found, "xkit");
        let probe: Arc<dyn Tool> = Fake::new("xprobe", Access::Read, Act::Echo);
        Ok(match installed {
            true => vec![("xkit".to_string(), vec![probe])],
            false => Vec::new(),
        })
    }

    /// 核心造的时候那一份，接上包声明的。
    fn settings(&self, found: &mut [Found]) -> Vec<miyu_config::Item> {
        let own = [
            miyu_endpoint::settings::UiSettings::ITEMS,
            miyu_endpoint::settings::PersonaSettings::ITEMS,
            miyu_endpoint::settings::PresetSettings::ITEMS,
            miyu_endpoint::settings::PermissionSettings::ITEMS,
            miyu_endpoint::settings::EXTERNAL_BINDINGS,
        ]
        .concat();
        let packaged = miyu_endpoint::packages::settle(found, &own);
        own.into_iter().chain(packaged).collect()
    }

    fn embed(&self, _: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        None
    }
}

/// 照起来时那样造核心：清单里装着 `xkit`，编进来的内置包有它和出厂的几个。
fn core(home: &Home, script: &Script) -> Arc<Core> {
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    let groups = Kit.tools(&found).expect("拼得出");
    let catalog = Catalog::in_packages(
        groups
            .iter()
            .map(|(id, tools)| (id.as_str(), tools.clone()))
            .collect::<Vec<_>>(),
    )
    .expect("合写法");
    let queries = Queries::new().register_for("xkit", "xkit.ping", |_core, _params| async {
        Ok(json!({"pong": true}))
    });
    Arc::new(
        home.core_full(script, catalog, None, TOKEN)
            .with_built_in(vec![
                "basesystem",
                "memory",
                "roleplay",
                "mermaid",
                "net",
                "xkit",
            ])
            .with_builtins(Arc::new(Kit))
            .with_queries(queries),
    )
}

/// 第 `n` 次请求的工具面。
fn face(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

/// 会话最后一条工具结果的字。
fn last_result(home: &Home, session: &str) -> Option<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .next_back()
}

async fn ping(client: &mut Client) -> Value {
    client.call("p", "xkit.ping", json!({})).await
}

#[tokio::test]
async fn a_removed_builtin_leaves_old_sessions_saying_uninstalled_and_comes_back() {
    let home = Home::new();
    home.write("home/alice/packages/xkit/package.toml", XKIT);
    let script = Script::new([
        Play::Says("在。"),
        Play::calls(&[("xprobe", "{}")]),
        Play::Says("没了。"),
        Play::Says("新的。"),
        Play::Says("回来了。"),
    ]);
    let mut client = Client::connect(core(&home, &script));
    client.hello().await;
    assert_eq!(ping(&mut client).await["result"]["pong"], true);
    let old = client.create("c1", "~").await;
    client.say("s1", &old, "你好").await;
    home.until_turns(&old, 1).await;
    assert!(face(&script, 0).contains(&"xprobe".to_string()));

    let removed = client
        .call("r1", "package.remove", json!({"package": "xkit"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert_eq!(
        reason(&ping(&mut client).await),
        Some("unknown_method"),
        "卸掉的包的查询当没有"
    );
    client.say("s2", &old, "调一下").await;
    home.until_turns(&old, 2).await;
    assert!(
        face(&script, 1).contains(&"xprobe".to_string()),
        "用过它的会话工具面不变"
    );
    assert_eq!(
        last_result(&home, &old).as_deref(),
        Some("The tool \"xprobe\" was uninstalled.\n")
    );
    let fresh = client.create("c2", "~").await;
    client.say("s3", &fresh, "你好").await;
    home.until_turns(&fresh, 1).await;
    assert!(
        !face(&script, 3).contains(&"xprobe".to_string()),
        "新开的会话没有"
    );

    let source = home.work.join("xkit");
    std::fs::create_dir_all(&source).expect("建得了");
    std::fs::write(source.join("package.toml"), XKIT).expect("写得进");
    let back = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(back["result"]["package"], "xkit", "{back}");
    assert_eq!(
        ping(&mut client).await["result"]["pong"],
        true,
        "装回来查询回来"
    );
    let again = client.create("c3", "~").await;
    client.say("s4", &again, "你好").await;
    home.until_turns(&again, 1).await;
    assert!(
        face(&script, 4).contains(&"xprobe".to_string()),
        "装回来工具回来"
    );
}
