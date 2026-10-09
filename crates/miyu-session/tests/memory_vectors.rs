//! 照意思找记忆（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：真的 `miyu-embed`、手造的小模型（文件事先放在缓存目录里，
//! 不下），真记忆日志、真回合库，执行器替身照剧本调 `memory_search`。
//!
//! 搜一句关键词对不上的（「喝茶」对「我的猫」）：第一次只走关键词、找不到，搜的时候起的后台把向量补齐以后照意思找得到；以前
//! 的对话也一样；`models.embedding = "off"` 的不算向量、不补、不拉起小程序，只照关键词。小模型四维，什么都挺像（相似度都过
//! 下限）：下限挡不挡得住在 `miyu-recall` 的单元测试里。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
use miyu_recall::{MemoryEvent, Saved};
use miyu_session::testkit::{Play, Script};
use miyu_session::{EmbedSetup, Embedder, Handle, Vectors};
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use crate::support::*;

const MODEL: &str = "local:tiny";

/// 出厂的真模型（量尺用）。
const REAL: &str = "local:bge-small-zh-v1.5";

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

/// 给场地的记忆接上照意思找的那一路：小模型的文件事先放进缓存目录（核对得上，不下）；另放一个清单以外的 `stray`（备文件时
/// 会被删掉，看备过没有）。交回算向量的（看它拉起没有）。
fn meaning(home: &Home) -> Embedder {
    let cache = home.scratch.0.join("cache").join("embed");
    std::fs::create_dir_all(cache.join("tiny")).expect("建得了");
    for name in ["model.onnx", "vocab.txt"] {
        std::fs::copy(tiny().join(name), cache.join("tiny").join(name)).expect("放得下");
    }
    std::fs::write(cache.join("tiny").join("stray"), b"x").expect("写得进");
    attach(home, tiny().join("manifest.toml"), cache)
}

/// 照清单 `manifest`、缓存目录 `cache` 接上。
fn attach(home: &Home, manifest: PathBuf, cache: PathBuf) -> Embedder {
    let embedder = Embedder::new(EmbedSetup {
        program: Some(program()),
        manifest,
        cache: Some(cache),
        client: miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"),
        idle: Duration::from_secs(600),
    });
    let given = home
        .memory
        .give_vectors(Arc::new(Vectors::new(embedder.clone())));
    assert!(given);
    embedder
}

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 人记一条。
fn save(home: &Home, text: &str) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

fn search(query: &str) -> Play {
    let args = serde_json::json!({ "query": query }).to_string();
    Play::calls(&[("memory_search", &args)])
}

/// 说 `words`，等到结束了 `turns` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turns: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turns}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

/// 日志里最后一次调用的结果。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

/// 这一份库里键是 `key` 的那一条补上了这个模型的向量没有。
fn has_vector(index: &miyu_store::recall::RecallIndex, model: &str, key: &str) -> bool {
    let missing = index.missing(model, 0, 1000).expect("读得了");
    !missing.iter().any(|(_, missing, _)| missing == key)
        && index.keys().expect("读得了").iter().any(|have| have == key)
}

/// 等记忆库里的 `key`（`memory`）或者回合库里以 `key` 开头的那一条（不是 `memory`）补上向量（最多 30 秒）。补是搜的时候起
/// 的：起的时候已经在库里的都补，之后才放进去的（这一轮自己的回合）等下一次搜。
async fn filled(home: &Home, model: &str, memory: bool, key: &str) {
    for _ in 0..600 {
        let done = if memory {
            let (log, _) = home.logs.open(&persona()).expect("开得了");
            has_vector(log.index(), model, key)
        } else {
            let (turns, _) = home.recall.turns(&persona());
            let keys = turns.keys().expect("读得了");
            keys.iter()
                .filter(|have| have.starts_with(key))
                .all(|have| has_vector(&turns, model, have))
                && keys.iter().any(|have| have.starts_with(key))
        };
        if done {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("三十秒没补上 {key}");
}

#[tokio::test]
async fn a_memory_is_found_by_meaning_once_its_vector_is_filled() {
    let home = Home::new();
    let embedder = meaning(&home);
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
    assert!(
        !last_result(&log).contains("我的猫"),
        "第一次只走关键词：{}",
        last_result(&log)
    );
    filled(&home, MODEL, true, "m1").await;
    let log = chat(&home, &handle, 2, "再找找").await;
    let found = last_result(&log);
    assert!(found.contains("我的猫"), "照意思找得到：{found}");
    assert!(embedder.running().await);
    stop(&handle).await;
}

#[tokio::test]
async fn past_conversations_are_found_by_meaning_too() {
    let home = Home::new();
    meaning(&home);
    // 另一个会话说过一轮「我的猫」：进了回合库。
    let before = home.create(&Script::new([Play::Says("好。")])).await;
    let mut pushes = watch(&before).await;
    ask(&before, "cmd-1", say("我的猫"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&before).await;

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
    chat(&home, &handle, 1, "以前聊过什么").await;
    filled(&home, MODEL, false, &format!("{}/", before.id())).await;
    let log = chat(&home, &handle, 2, "再找找").await;
    let found = last_result(&log);
    assert!(found.contains("我的猫"), "以前的对话照意思找得到：{found}");
    stop(&handle).await;
}

#[tokio::test]
async fn off_means_keywords_only_and_nothing_runs() {
    let mut home = Home::new();
    home.configs = crate::support::routing::configs("[models]\nembedding = \"off\"\n", &[]);
    let embedder = meaning(&home);
    save(&home, "我的猫");
    let script = Script::new([
        search("喝茶"),
        Play::Says("没找到。"),
        search("我的猫"),
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
    assert!(!last_result(&log).contains("我的猫"));
    tokio::time::sleep(Duration::from_millis(500)).await;
    let stray = home.scratch.0.join("cache/embed/tiny/stray");
    assert!(stray.exists(), "不备模型的文件（不核对、不下）");
    let (log_of, _) = home.logs.open(&persona()).expect("开得了");
    assert_eq!(
        log_of.index().missing(MODEL, 0, 10).expect("读得了").len(),
        1,
        "不补"
    );
    assert!(!embedder.running().await, "不拉起小程序");
    let log = chat(&home, &handle, 2, "找我的猫").await;
    assert!(last_result(&log).contains("我的猫"), "关键词照旧找得到");
    stop(&handle).await;
}

/// 模型还在下的时候，补的那一个等它下完再补（`recall.md` 第三条第 5 款）：假服务器慢慢交小模型的文件，只搜一次，之后不再搜，
/// 也补齐了。
#[tokio::test]
async fn the_fill_waits_while_the_model_is_downloaded() {
    use miyu_http::testkit::{Piece, Reply, Server};
    use sha2::{Digest, Sha256};
    let home = Home::new();
    let slow = |name: &str| Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![
            Piece::Wait(Duration::from_millis(500)),
            Piece::Bytes(std::fs::read(tiny().join(name)).expect("读得到")),
        ],
    };
    let server = Server::start(vec![slow("model.onnx"), slow("vocab.txt")]).await;
    let hex = |name: &str| -> (String, usize) {
        let bytes = std::fs::read(tiny().join(name)).expect("读得到");
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        (digest, bytes.len())
    };
    let ((m, ms), (v, vs)) = (hex("model.onnx"), hex("vocab.txt"));
    let manifest = home.scratch.0.join("tiny.toml");
    let text = format!(
        "id = \"tiny\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 6\n\n\
         [[files]]\nrole = \"model\"\nname = \"model.onnx\"\nurl = \"{base}/model.onnx\"\nsha256 = \"{m}\"\nsize = {ms}\n\n\
         [[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nurl = \"{base}/vocab.txt\"\nsha256 = \"{v}\"\nsize = {vs}\n",
        base = server.base_url,
    );
    std::fs::write(&manifest, text).expect("写得进");
    attach(&home, manifest, home.scratch.0.join("cache").join("embed"));
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
}

/// 量尺（验收时手动跑）：真模型（`MIYU_EMBED_MODEL_DIR` 指到 Release 的文件，先拷进缓存目录，不下），记八条、问八句意思对字
/// 不对的，量她搜不搜得到、第一次搜等多久、补一百条要多久。
#[tokio::test]
#[ignore = "要真模型的文件：MIYU_EMBED_MODEL_DIR=… cargo test -p miyu-session --test all measure_meaning -- --ignored --nocapture"]
async fn measure_meaning_with_the_real_model() {
    use miyu_session::Keeper;
    let dir =
        PathBuf::from(std::env::var_os("MIYU_EMBED_MODEL_DIR").expect("设了 MIYU_EMBED_MODEL_DIR"));
    let home = Home::new();
    let cache = home.scratch.0.join("cache").join("embed");
    std::fs::create_dir_all(cache.join("bge-small-zh-v1.5")).expect("建得了");
    for name in ["model_quantized.onnx", "vocab.txt"] {
        std::fs::copy(dir.join(name), cache.join("bge-small-zh-v1.5").join(name)).expect("拷得了");
    }
    let manifest = home.resources.embed_manifest();
    attach(&home, manifest, cache);
    let memories = [
        "用户养了一只猫，叫团子，三岁",
        "用户喜欢喝乌龙茶，下午不喝咖啡",
        "回答先说结论，再给依据",
        "用户的显卡是 N 卡，型号 4090",
        "用户住在东京",
        "项目仓库是 GPL-3.0-or-later，加依赖要先查许可证",
        "用户周末一般在家写 Rust",
        "用户对花粉过敏，春天出门戴口罩",
    ];
    for text in memories {
        save(&home, text);
    }
    let questions = [
        ("我家宠物叫什么", "团子"),
        ("我平时喝什么饮料", "乌龙茶"),
        ("你回答问题的习惯", "结论"),
        ("我用的什么显卡", "4090"),
        ("我在哪个城市", "东京"),
        ("加第三方库要注意什么", "许可证"),
        ("我周末做什么", "Rust"),
        ("我有什么过敏", "花粉"),
    ];
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let vectors = keeper.vectors().expect("接上了").clone();
    let started = std::time::Instant::now();
    let mut first = vectors.query("我家宠物叫什么").await;
    while first.is_none() {
        tokio::time::sleep(Duration::from_millis(50)).await;
        first = vectors.query("我家宠物叫什么").await;
    }
    eprintln!("第一次算出问句（核对文件、拉起）：{:?}", started.elapsed());
    let filling = std::time::Instant::now();
    keeper.fill();
    filled(&home, REAL, true, &format!("m{}", memories.len())).await;
    eprintln!("补 {} 条：{:?}", memories.len(), filling.elapsed());
    let mut keyword_only = 0;
    let mut by_meaning = 0;
    for (question, expected) in questions {
        let plain = keeper.search(question, false, 3, None).expect("搜得了");
        let near = vectors.query(question).await;
        let both = keeper
            .search(question, false, 3, near.as_ref())
            .expect("搜得了");
        let top = |found: &[miyu_recall::Entry]| {
            found
                .first()
                .is_some_and(|entry| entry.text.contains(expected))
        };
        keyword_only += usize::from(top(&plain));
        by_meaning += usize::from(top(&both));
        eprintln!("{question}：只关键词 {}，两路 {}", top(&plain), top(&both));
    }
    eprintln!("第一条对的：只关键词 {keyword_only}/8，两路 {by_meaning}/8");
    let hundred = std::time::Instant::now();
    for n in 0..100 {
        save(
            &home,
            &format!("第 {n} 条：用户今天做了一件小事，记下来以后能想起来"),
        );
    }
    keeper.fill();
    filled(&home, REAL, true, "m108").await;
    eprintln!("补一百条：{:?}", hundred.elapsed());
}
