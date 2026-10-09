//! 装卸以后本机的向量模型当场换（施工 F-5 再补，`docs/blueprint/packages.md`「装卸」第 2 条，`recall.md` 第四条第 4 款）：真
//! 核心装上一个小程序包，端口照这时的清单拼本机的那一路、换上，`config.schema` 里「内置模型」后面暗字当场写它的模型名；升级
//! 成另一个模型的换成新的；卸掉又没了。端口照 `miyu-core` 的拼法换成测试的：清单里装着 `xembed` 就照它的包目录拼。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_config::Item;
use miyu_endpoint::Core;
use miyu_endpoint::builtins::{Builtins, Group};
use miyu_session::EmbedSetup;
use miyu_session::testkit::Script;
use miyu_store::packages::Found;
use miyu_tool::Catalog;

use crate::support::*;

/// 核心自己的配置项：要看语义模型那一项。
fn core_items() -> Vec<Item> {
    [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_models::settings::UseSettings::ITEMS,
    ]
    .concat()
}

/// 清单里装着 `xembed` 的照它的包目录拼本机的那一路；程序是测试程序自己（不拉起，只看模型名）。
struct Port;

impl Builtins for Port {
    fn tools(&self, _: &[Found]) -> Result<Vec<Group>, String> {
        Ok(Vec::new())
    }

    fn settings(&self, found: &mut [Found]) -> Vec<Item> {
        let packaged = miyu_endpoint::packages::settle(found, &core_items());
        core_items().into_iter().chain(packaged).collect()
    }

    fn embed(&self, found: &[&Found]) -> Option<EmbedSetup> {
        let package = found
            .iter()
            .find(|one| one.id == "xembed" && one.read.is_ok())?;
        let dir = package.files_dir();
        Some(EmbedSetup {
            program: std::env::current_exe().ok(),
            manifest: dir.join("model.toml"),
            dir,
            idle: Duration::from_secs(600),
        })
    }
}

/// 小程序包的清单。
const XEMBED: &str = "[package]\nkind = \"worker\"\nprotocol = [1, 1]\nname = { en = \"X model\" }\n\n[worker]\nprogram = \"miyu-nothing\"\n";

/// 要装的那一份放在工作目录：清单，旁边的包目录里一份模型清单，模型名是 `id`（文件不摆：只看名字，不算）。
fn source(home: &Home, id: &str) -> std::path::PathBuf {
    let path = home.work.join("xembed.toml");
    std::fs::write(&path, XEMBED).expect("写得进");
    let dir = home.work.join("xembed");
    std::fs::create_dir_all(&dir).expect("建得了");
    let zero = "0".repeat(64);
    let model = format!(
        "id = \"{id}\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 6\n\n\
         [[files]]\nrole = \"model\"\nname = \"model.onnx\"\nsha256 = \"{zero}\"\nsize = 1\n\n\
         [[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nsha256 = \"{zero}\"\nsize = 1\n"
    );
    std::fs::write(Path::new(&dir).join("model.toml"), model).expect("写得进");
    path
}

/// `config.schema` 里「内置模型」那一个选项。
async fn built_in(client: &mut Client, id: &str) -> Value {
    let reply = client
        .call(id, "config.schema", json!({"keys": ["models.embedding"]}))
        .await;
    reply["result"]["items"][0]["options"][0].clone()
}

#[tokio::test]
async fn the_local_model_follows_install_upgrade_and_removal() {
    let home = Home::new();
    home.write("system/config.toml", "");
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        core_items(),
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core: Arc<Core> = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_builtins(Arc::new(Port))
            .with_vectors(None),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let before = built_in(&mut client, "c0").await;
    assert!(before.get("note").is_none(), "没装的没有：{before}");

    let path = source(&home, "xtiny");
    let reply = client
        .call("i1", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reply["result"]["package"], "xembed", "{reply}");
    let after = built_in(&mut client, "c1").await;
    assert_eq!(after["note"], "xtiny", "装上就有：{after}");

    let path = source(&home, "xtiny2");
    let reply = client
        .call("i2", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reply["result"]["package"], "xembed", "{reply}");
    let upgraded = built_in(&mut client, "c2").await;
    assert_eq!(upgraded["note"], "xtiny2", "升级换成新的：{upgraded}");

    let reply = client
        .call("r1", "package.remove", json!({"package": "xembed"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    let gone = built_in(&mut client, "c3").await;
    assert!(gone.get("note").is_none(), "卸掉又没了：{gone}");
}
