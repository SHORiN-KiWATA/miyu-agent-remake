//! 人经协议照意思找记忆（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：真核心接上本机 embedding（手造的小模型、真的
//! `miyu-embed`，小模型的测试数据目录当包目录：只读、核对得上），`memory.remember` 记下的时候就在后台补它的向量（施工 R-5 五补），补上以后 `memory.search` 照意思找得到。
//! 远程的（施工 R-5 补）照这时的配置发给假服务器，用量记在管理员名下。`off` 的那一路和她的工具共用一个判法
//! （`miyu_session::Vectors` 照 `Using` 的配置挑），在会话的测试里。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_session::EmbedSetup;
use miyu_session::testkit::Script;

use crate::support::memories::texts;
use crate::support::*;

fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/tests/fixtures/tiny")
}

/// cargo 编出来的 `miyu-embed`：和测试程序所在的 `deps/` 同一层。
fn program() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("在 target/<profile>/deps/ 里");
    let program = dir.join(format!("miyu-embed{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.is_file(),
        "{} 不在：先 cargo build -p miyu-embed（cargo test --workspace 会编它）",
        program.display()
    );
    program
}

/// 接上小模型的核心：系统配置里默认人格是软件工程师。
fn core(home: &Home) -> Arc<miyu_endpoint::Core> {
    home.write("system/config.toml", "[persona]\ndefault = \"engineer\"\n");
    let setup = local();
    let configured = venues::configured_core(home, &Script::new([]), miyu_tool::Catalog::default());
    let core = Arc::try_unwrap(configured).unwrap_or_else(|_| panic!("刚造的只有一份"));
    Arc::new(core.with_vectors(Some(setup)))
}

/// 本机的那一路：小模型的测试数据目录当包目录（核对只读，不改它）。
fn local() -> EmbedSetup {
    EmbedSetup {
        program: Some(program()),
        manifest: tiny().join("manifest.toml"),
        dir: tiny(),
        idle: Duration::from_secs(600),
    }
}

/// 接上远程的核心：一家 `emb` 在假服务器上，key 照核心的环境变量取，`models.embedding` 指着它的 `text-emb`；没有本机的。
fn remote(home: &Home, server: &Server) -> Arc<miyu_endpoint::Core> {
    home.write(
        "system/config.toml",
        &format!(
            "[persona]\ndefault = \"engineer\"\n\n[providers.emb]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"EMB_KEY\" }}\n\n[models]\nembedding = \"emb/text-emb\"\n",
            server.base_url
        ),
    );
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_models::settings::UseSettings::ITEMS,
        miyu_models::settings::ProviderSettings::ITEMS,
        miyu_models::settings::ModelSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[("EMB_KEY", "sk-emb")]),
    );
    let data = providers::data(providers::profiles(json!({})));
    let core = home
        .core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
        .with_config(config)
        .with_model_data(data)
        .with_vectors(None);
    Arc::new(core)
}

/// 记一条「我的猫」，搜「喝茶」：记下的时候就在后台补它的向量（施工 R-5 五补），补上以后照意思找得到。
async fn found_by_meaning(client: &mut Client) {
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "我的猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    let mut found = Vec::new();
    for n in 0..600 {
        let reply = client
            .call(&format!("s{n}"), "memory.search", json!({"query": "喝茶"}))
            .await;
        found = texts(&reply);
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        found,
        [("m1".to_string(), "我的猫".to_string())],
        "照意思找得到"
    );
}

#[tokio::test]
async fn search_finds_by_meaning_once_the_vectors_are_filled() {
    let home = Home::new();
    let mut client = Client::connect(core(&home));
    client.hello().await;
    found_by_meaning(&mut client).await;
}

#[tokio::test]
async fn a_remote_model_is_asked_with_the_current_config_and_billed_to_the_admin() {
    let reply = Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: vec![Piece::Bytes(
            br#"{"data":[{"embedding":[3,4,0,0]}],"usage":{"prompt_tokens":5}}"#.to_vec(),
        )],
    };
    let server = Server::start(vec![reply; 40]).await;
    let home = Home::new();
    let mut client = Client::connect(remote(&home, &server));
    client.hello().await;
    found_by_meaning(&mut client).await;
    let first = &server.received()[0];
    assert_eq!(first.path, "/v1/embeddings");
    assert_eq!(first.header("authorization"), Some("Bearer sk-emb"));
    let journal = std::fs::read_to_string(
        home.root
            .account_dir(&alice())
            .join(miyu_store::journal::FILE),
    )
    .expect("有账号日志");
    assert!(
        journal.contains(r#""body":{"purpose":"embedding","endpoint":"emb","model":"text-emb","usage":{"uncached":5"#),
        "{journal}"
    );
}

/// 接上本机的那一路的核心（施工 R-5 再补）：`config.schema` 里「内置模型」后面暗字写本机清单的模型名（小模型叫 `tiny`），选
/// 得了（不带 `available`，施工 R-5 三补）。
#[tokio::test]
async fn the_built_in_option_notes_the_local_model() {
    let home = Home::new();
    home.write("system/config.toml", "");
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_models::settings::UseSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = home
        .core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
        .with_config(config)
        .with_vectors(Some(local()));
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    let reply = client
        .call("c1", "config.schema", json!({"keys": ["models.embedding"]}))
        .await;
    let options = &reply["result"]["items"][0]["options"];
    assert_eq!(options[0]["note"], "tiny", "{reply}");
    assert!(options[1].get("note").is_none(), "关没有暗字：{reply}");
    assert!(options[0].get("available").is_none(), "{reply}");
    assert!(options[1].get("available").is_none(), "{reply}");
}

/// 记忆库里 `key` 那一条有没有小模型的向量（不经 `memory.search`）。
fn vectored(home: &Home, key: &str) -> bool {
    let path = home
        .root
        .account_dir(&alice())
        .join("index/recall/memory-engineer.db");
    let (index, _) = miyu_store::recall::RecallIndex::open(&path);
    let missing = index.missing("local:tiny", 0, 1000).unwrap_or_default();
    index
        .keys()
        .unwrap_or_default()
        .iter()
        .any(|have| have == key)
        && !missing.iter().any(|(_, gone, _)| gone == key)
}

/// 等 `key` 补上向量（最多 10 秒）。
async fn until_vectored(home: &Home, key: &str) {
    for _ in 0..200 {
        if vectored(home, key) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没补上 {key}");
}

/// 记下的时候就补（施工 R-5 五补）：`memory.remember`、`memory.update`、`/remember` 以后不搜，记忆库里就有了它的向量。
#[tokio::test]
async fn remembering_updating_and_slash_remember_fill_vectors_without_a_search() {
    let home = Home::new();
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "我的猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    until_vectored(&home, "m1").await;
    let reply = client
        .call(
            "u1",
            "memory.update",
            json!({"id": "m1", "text": "我的橘猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m2"}), "{reply}");
    until_vectored(&home, "m2").await;
    let session = client.create("s1", "~").await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/remember 我喜欢喝乌龙茶"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    until_vectored(&home, "m3").await;
}
