//! 远程的 embedding（施工 R-5 补，`docs/blueprint/recall.md` 第四条第 1 款）：`models.embedding` 写 `<供应商>/<模型>`，假服务器
//! 当那一家，执行器替身照剧本调 `memory_search`。那一家回的向量都是 `[3, 4, 0, 0]`（没归一化过，什么都挺像）。
//!
//! 照它算问句、补向量，关键词对不上的照意思找得到；向量照 `<供应商>/<模型>` 存、归一化过；请求带着认证、模型名、档案另配的头
//! （种子是用途 `embedding`）；报了用量的记一笔 `usage.oneshot`（用途 `embedding`），没报的不记；那一家出错、回的读不懂、等不到
//! 的只走关键词，连着三条算不出的这一回不补了（中间算成一条的重新数）。

use std::sync::Arc;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::testkit::{Play, Script};
use miyu_session::{ModelData, Observed, Vectors};

use crate::support::meaning::*;
use crate::support::*;

/// 向量记的模型编号。
const MODEL: &str = "emb/text-emb";

/// 200，JSON 的 `body`。
fn json(body: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

/// 那一家回的一个向量 `values`；`usage` 的报了 7 个输入 token（合计写 9，读的是输入那一格）。
fn answer(values: &str, usage: bool) -> Reply {
    let usage = match usage {
        true => r#","usage":{"prompt_tokens":7,"total_tokens":9}"#,
        false => "",
    };
    json(&format!(
        r#"{{"object":"list","data":[{{"object":"embedding","index":0,"embedding":{values}}}],"model":"text-emb"{usage}}}"#
    ))
}

/// 那一家回的 `[3, 4, 0, 0]`。
fn vector(usage: bool) -> Reply {
    answer("[3,4,0,0]", usage)
}

/// 场地接上远程的：一家 `emb` 在假服务器上，key 是 `sk-emb`，档案给它另配一个头 `x-trace`，`models.embedding` 指着它的
/// `text-emb`；没有本机的。
fn remote(home: &mut Home, server: &Server) {
    let source = format!(
        "[providers.emb]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"EMB_KEY\" }}\n\n[models]\nembedding = \"{MODEL}\"\n",
        server.base_url
    );
    home.configs = crate::support::routing::configs(
        &source,
        &[(Reference::Env("EMB_KEY".to_string()), "sk-emb")],
    );
    let profiles =
        serde_json::json!({"providers": {"emb": {"headers": {"x-trace": "t_{session_digest}"}}}});
    let data = ModelData::new(
        Profiles::parse(&profiles).expect("档案写法对"),
        Vendors::default(),
        None,
    )
    .with_fetcher(miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"));
    data.loaded(None, Observed::default());
    data.keep_ledger(Arc::clone(&home.usage));
    let given = home
        .memory
        .give_vectors(Arc::new(Vectors::new(None, Arc::new(data))));
    assert!(given);
}

/// 账号日志里的 `usage.oneshot`。
fn billed(home: &Home) -> Vec<String> {
    let journal = std::fs::read_to_string(
        home.root
            .account_dir(&alice_account())
            .join(miyu_store::journal::FILE),
    )
    .unwrap_or_default();
    journal
        .lines()
        .filter(|line| line.contains(r#""kind":"usage.oneshot""#))
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn a_remote_model_finds_a_memory_by_meaning() {
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    save(&home, "我的猫");
    let script = Script::new([
        search("喝茶"),
        Play::Says("没找到。"),
        search("喝茶"),
        Play::Says("找到了。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, "我喝什么").await;
    assert!(!last_result(&log).contains("我的猫"), "第一次只走关键词");
    filled(&home, MODEL, true, "m1").await;
    let log = chat(&home, &handle, 2, "再找找").await;
    let found = last_result(&log);
    assert!(found.contains("我的猫"), "照意思找得到：{found}");
    stop(&handle).await;

    let first = &server.received()[0];
    assert_eq!(
        (first.method.as_str(), first.path.as_str()),
        ("POST", "/v1/embeddings")
    );
    assert_eq!(first.header("authorization"), Some("Bearer sk-emb"));
    let trace = format!("t_{}", miyu_models::headers::digest("embedding"));
    assert_eq!(first.header("x-trace"), Some(trace.as_str()));
    let body: serde_json::Value = serde_json::from_slice(&first.body).expect("是 JSON");
    assert_eq!(
        body,
        serde_json::json!({"model": "text-emb", "input": "喝茶"})
    );
    // 存的是归一化过的：和 `[0.6, 0.8]` 一模一样。
    let (log_of, _) = home.logs.open(&persona()).expect("开得了");
    let near = log_of
        .index()
        .nearest(MODEL, &[0.6, 0.8, 0.0, 0.0], 1)
        .expect("读得了");
    assert!((near[0].similar - 1.0).abs() < 1e-4, "{}", near[0].similar);
    // 每一次都记了一笔，记在属主名下。
    let billed = billed(&home);
    assert!(billed.len() >= 3, "两次问句、补一条：{billed:?}");
    assert!(
        billed.iter().all(|line| line.contains(
            r#""body":{"purpose":"embedding","endpoint":"emb","model":"text-emb","usage":{"uncached":7"#
        )),
        "{billed:?}"
    );
}

#[tokio::test]
async fn unreported_usage_is_not_billed() {
    let server = Server::start((0..10).map(|_| vector(false)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    save(&home, "我的猫");
    let script = Script::new([search("喝茶"), Play::Says("没找到。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我喝什么").await;
    filled(&home, MODEL, true, "m1").await;
    stop(&handle).await;
    assert!(server.received().len() >= 2);
    assert_eq!(billed(&home), Vec::<String>::new());
}

/// 问句等不到（那一家停住不动）：等 1 秒，照关键词找得到。补：出错的跳过；算成一条的重新数；读不懂、全是零、没有向量连着三条，
/// 这一回不补了，第六条不发。出错的、读不懂的不记账；全是零的报了用量，照记。
#[tokio::test]
async fn a_failing_remote_falls_back_to_keywords() {
    let server = Server::start(vec![
        Reply::stream(vec![Piece::Stall]),
        Reply::error(500, &[], r#"{"error":{"message":"down"}}"#),
        vector(true),
        json("not json"),
        answer("[0,0,0,0]", true),
        json(r#"{"data":[]}"#),
        vector(true),
    ])
    .await;
    let mut home = Home::new();
    remote(&mut home, &server);
    for text in ["我的猫", "猫粮", "猫砂", "猫爬架", "猫薄荷", "猫抓板"] {
        save(&home, text);
    }
    let script = Script::new([search("猫"), Play::Says("找到了。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let started = std::time::Instant::now();
    let log = chat(&home, &handle, 1, "我的猫呢").await;
    assert!(last_result(&log).contains("我的猫"), "关键词照旧找得到");
    assert!(started.elapsed() >= std::time::Duration::from_secs(1));
    within("补的那几条发出去", async {
        while server.received().len() < 6 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    stop(&handle).await;
    assert_eq!(server.received().len(), 6, "连着三条算不出就停");
    let (log_of, _) = home.logs.open(&persona()).expect("开得了");
    let missing: Vec<String> = log_of
        .index()
        .missing(MODEL, 0, 10)
        .expect("读得了")
        .into_iter()
        .map(|(_, key, _)| key)
        .collect();
    assert_eq!(
        missing,
        ["m1", "m3", "m4", "m5", "m6"],
        "只补上算成的那一条"
    );
    assert_eq!(billed(&home).len(), 2, "算成的、全是零的那两条报了用量");
}
