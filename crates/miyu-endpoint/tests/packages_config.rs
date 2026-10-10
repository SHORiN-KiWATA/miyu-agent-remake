//! 装卸以后配置项当场换（施工 F-5 补，`docs/blueprint/packages.md`「配置项」第 2 条、`config.md` 推送 `config.changed`）：
//! 真核心装上一个声明了配置项的包，系统配置里早写着的它的键当场认得、推 `config.changed`（`via: package`）、`config.get`
//! 和 `config.schema` 有它；卸掉又报不认识；清单换了、认得的没变的不推。端口照 `miyu-core` 的拼法：核心自己的接上包的。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::Item;
use miyu_endpoint::Core;
use miyu_endpoint::builtins::{Builtins, Group};
use miyu_session::testkit::Script;
use miyu_store::packages::Found;
use miyu_tool::Catalog;

use crate::support::*;

/// 核心自己的配置项。
fn core_items() -> Vec<Item> {
    [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat()
}

/// 照 `miyu-core` 那样拼整份清单的端口：核心自己的，接上包声明的。没有内置包的工具。
struct Port;

impl Builtins for Port {
    fn tools(&self, _: &[Found]) -> Result<Vec<Group>, String> {
        Ok(Vec::new())
    }

    fn settings(&self, found: &mut [Found]) -> Vec<Item> {
        let packaged = miyu_endpoint::packages::settle(found, &core_items());
        core_items().into_iter().chain(packaged).collect()
    }

    fn embed(&self, _: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        None
    }
}

/// 一份手动拉起的扩展包的清单：带一项系统配置的整数 `port`。程序要在测试程序旁边（施工 F-6 上：程序不在的当没装，配置项不算）。
fn manifest(id: &str, program: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"X\" }}\n\n[process]\nstart = \"manual\"\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\"]\nname = {{ en = \"Port\" }}\n"
    )
}

/// 要装的那个包：放在数据根外面的工作目录里，交回包目录。
fn source(home: &Home, id: &str, program: &str) -> std::path::PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest(id, program)).expect("写得进");
    folder
}

/// 照磁盘上的系统配置起来、装了端口的核心。
fn served(home: &Home) -> Arc<Core> {
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        core_items(),
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_builtins(Arc::new(Port)),
    )
}

/// 发一条请求，交回回应以前读到的推送和回应。
async fn ask(client: &mut Client, id: &str, method: &str, params: Value) -> (Vec<Value>, Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// 接下来的 `n` 条 `config.changed` 的 `params`：回应以前读到了的先用，不够的接着等。
async fn changed(client: &mut Client, before: Vec<Value>, n: usize) -> Vec<Value> {
    let mut pushes = before.into_iter();
    let mut got = Vec::new();
    while got.len() < n {
        let next = match pushes.next() {
            Some(next) => next,
            None => client.next().await.expect("没断开"),
        };
        if next["method"] == "config.changed" {
            got.push(next["params"].clone());
        }
    }
    got
}

#[tokio::test]
async fn an_installed_packages_settings_are_known_at_once_and_forgotten_once_removed() {
    let home = Home::new();
    let program = crate::support::extensions::Program::new();
    home.write("system/config.toml", "xcfg.port = 9000\n");
    home.root.prepare_home(&alice()).expect("建得了家目录");
    // 只能放系统配置的项写进了个人设置：装上以后这一层认得的项没变，问题变了（不认识换成放错了层），照样推。
    home.write("home/alice/settings.toml", "xcfg.port = 1\n");
    let core = served(&home);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let (_, reply) = ask(&mut client, "s1", "subscribe", json!({"stream": "config"})).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let (_, before) = ask(
        &mut client,
        "g0",
        "config.get",
        json!({"keys": ["xcfg.port"]}),
    )
    .await;
    assert_eq!(
        reason(&before),
        Some("unknown_config_key"),
        "装之前不认识：{before}"
    );

    let path = source(&home, "xcfg", &program.name());
    let (pushes, reply) = ask(&mut client, "i1", "package.install", json!({"path": path})).await;
    assert_eq!(reply["result"]["package"], "xcfg", "{reply}");
    let pushed = changed(&mut client, pushes, 2).await;
    let (system, personal) = (&pushed[0], &pushed[1]);
    assert_eq!(system["via"], "package", "{system}");
    assert_eq!(system["layer"], "system", "{system}");
    assert!(system.get("by").is_none(), "{system}");
    assert_eq!(system["keys"]["xcfg.port"]["value"], 9000, "{system}");
    assert_eq!(
        system["keys"]["xcfg.port"]["effective"], 9000,
        "最终值重算了：{system}"
    );
    assert_eq!(system["problems"], json!([]), "不再报不认识：{system}");
    assert_eq!(personal["via"], "package", "{personal}");
    assert_eq!(personal["layer"], "personal", "{personal}");
    assert_eq!(personal["keys"], json!({}), "{personal}");
    assert_ne!(personal["problems"][0]["code"], "unknown_key", "{personal}");
    let (_, got) = ask(
        &mut client,
        "g1",
        "config.get",
        json!({"keys": ["xcfg.port"]}),
    )
    .await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9000, "{got}");
    let (_, schema) = ask(
        &mut client,
        "c1",
        "config.schema",
        json!({"keys": ["xcfg.port"]}),
    )
    .await;
    assert!(
        schema.get("error").is_none(),
        "config.schema 有它：{schema}"
    );

    // 再装一个带配置项、哪一层都没写它的：清单换了，认得的、问题都没变，不推。之后的第一条推送是 `config.set` 的。
    let plain = source(&home, "xquiet", &program.name());
    let (pushes, reply) = ask(&mut client, "i2", "package.install", json!({"path": plain})).await;
    assert_eq!(reply["result"]["package"], "xquiet", "{reply}");
    let (more, reply) = ask(
        &mut client,
        "set1",
        "config.set",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "en"}]}),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let push = &changed(&mut client, pushes.into_iter().chain(more).collect(), 1).await[0];
    assert_eq!(push["via"], "set", "没变的不推：{push}");

    let (pushes, reply) = ask(
        &mut client,
        "r1",
        "package.remove",
        json!({"package": "xcfg"}),
    )
    .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    let pushed = changed(&mut client, pushes, 2).await;
    let push = &pushed[0];
    assert_eq!(push["via"], "package", "{push}");
    assert_eq!(push["layer"], "system", "{push}");
    assert_eq!(
        push["problems"][0]["code"], "unknown_key",
        "卸掉又报不认识：{push}"
    );
    assert_eq!(
        pushed[1]["problems"][0]["code"], "unknown_key",
        "{}",
        pushed[1]
    );
    let (_, after) = ask(
        &mut client,
        "g2",
        "config.get",
        json!({"keys": ["xcfg.port"]}),
    )
    .await;
    assert_eq!(reason(&after), Some("unknown_config_key"), "{after}");
}
