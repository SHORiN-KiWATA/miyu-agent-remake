//! 照意思找记忆（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：真的 `miyu-embed`、手造的小模型（摆成一份包目录，R-5 三补），
//! 真记忆日志、真回合库，执行器替身照剧本调 `memory_search`。
//!
//! 搜一句关键词对不上的（「喝茶」对「我的猫」）：第一次只走关键词、找不到，搜的时候起的后台把向量补齐以后照意思找得到；以前
//! 的对话也一样；`models.embedding = "off"` 的不算向量、不补、不拉起小程序，只照关键词。小模型四维，什么都挺像（相似度都过
//! 下限）：下限挡不挡得住在 `miyu-recall` 的单元测试里。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use miyu_session::{EmbedSetup, Embedder, Keeper, Turn, Using, Vectors};

use crate::support::meaning::*;
use crate::support::package::{program, tiny_package};
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 出厂的真模型（量尺用）。
const REAL: &str = "local:bge-small-zh-v1.5";

/// 给场地的记忆接上照意思找的那一路：小模型摆在场地的 `packages/embed/`。交回算向量的（看它拉起没有）。
fn meaning(home: &Home) -> Embedder {
    attach(
        home,
        tiny_package(&home.scratch.0.join("packages").join("embed")),
    )
}

/// 照 `setup` 接上。
fn attach(home: &Home, setup: EmbedSetup) -> Embedder {
    let embedder = Embedder::new(setup);
    let given = home.memory.give_vectors(Arc::new(Vectors::new(
        Some(embedder.clone()),
        model_data(home),
    )));
    assert!(given);
    embedder
}

/// 不写 `models.embedding` 的照本机的（alice 的）。
fn using(home: &Home) -> Using {
    Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    }
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

/// 包里的文件还在核对的时候，补的那一个等它核对完再补（`recall.md` 第三条第 5 款）：补的是头一个要向量的，头一次要的一定交回
/// 「还在备」（核对在后台），等过了照样补齐。
#[tokio::test]
async fn the_fill_waits_while_the_files_are_checked() {
    let home = Home::new();
    meaning(&home);
    save(&home, "我的猫");
    Keeper::new(&home.memory, persona(), vec![alice()]).fill(&using(&home));
    filled(&home, MODEL, true, "m1").await;
}

/// 量尺（验收时手动跑）：真模型（`MIYU_EMBED_MODEL_DIR` 指到 Release 的文件，和仓库里的模型清单原本一起拷进一份包目录），记八
/// 条、问八句意思对字不对的，量她搜不搜得到、第一次搜等多久、补一百条要多久。
#[tokio::test]
#[ignore = "要真模型的文件：MIYU_EMBED_MODEL_DIR=… cargo test -p miyu-session --test all measure_meaning -- --ignored --nocapture"]
async fn measure_meaning_with_the_real_model() {
    let from =
        PathBuf::from(std::env::var_os("MIYU_EMBED_MODEL_DIR").expect("设了 MIYU_EMBED_MODEL_DIR"));
    let home = Home::new();
    let dir = home.scratch.0.join("packages").join("embed");
    std::fs::create_dir_all(&dir).expect("建得了");
    let original =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/package/embed/model.toml");
    std::fs::copy(original, dir.join("model.toml")).expect("拷得了");
    for name in ["model_quantized.onnx", "vocab.txt"] {
        std::fs::copy(from.join(name), dir.join(name)).expect("拷得了");
    }
    attach(
        &home,
        EmbedSetup {
            program: Some(program()),
            manifest: dir.join("model.toml"),
            dir,
            idle: Duration::from_secs(600),
        },
    );
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
    let using = using(&home);
    let started = std::time::Instant::now();
    let mut first = vectors.query(&using, "我家宠物叫什么").await;
    while first.is_none() {
        tokio::time::sleep(Duration::from_millis(50)).await;
        first = vectors.query(&using, "我家宠物叫什么").await;
    }
    eprintln!("第一次算出问句（核对文件、拉起）：{:?}", started.elapsed());
    let filling = std::time::Instant::now();
    keeper.fill(&using);
    filled(&home, REAL, true, &format!("m{}", memories.len())).await;
    eprintln!("补 {} 条：{:?}", memories.len(), filling.elapsed());
    let mut keyword_only = 0;
    let mut by_meaning = 0;
    for (question, expected) in questions {
        let plain = keeper.search(question, false, 3, None).expect("搜得了");
        let near = vectors.query(&using, question).await;
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
    keeper.fill(&using);
    filled(&home, REAL, true, "m108").await;
    eprintln!("补一百条：{:?}", hundred.elapsed());
}
