//! 本机模型当场换（施工 R-5 四补，`docs/blueprint/recall.md` 第四条第 1、4 款）：装卸内置语义模型那个包以后，核心照新的清单
//! 再拼一次交给 `Vectors::replace_local`。真的 `miyu-embed`、手造的小模型摆成临时的包目录。
//!
//! 换成另一份的：新的算得出、名字是新的，旧的小程序退了，旧的副本（在补的后台手里那种）要的交回用不了、不再拉起；同样的一份
//! 换了地方的也算换；一样的什么都不动；路径一样、模型清单内容变了的算换；换成空的就没有本机的那一路，空的再换成一份照它算。
//! 排队时被关掉的不再拉起，在 `src/embed/tests.rs`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use miyu_session::{EmbedSetup, Embedder, Turn, Unavailable, Using, Vectors};

use crate::support::meaning::model_data;
use crate::support::package::tiny_package;
use crate::support::{Home, alice_account};

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

/// 把 `dir` 里模型清单的 `id` 换成 `id`（模型文件不动）。
fn rename(dir: &Path, id: &str) {
    let path = dir.join("model.toml");
    let text = std::fs::read_to_string(&path).expect("读得到");
    let renamed = text.replacen("id = \"tiny\"", &format!("id = \"{id}\""), 1);
    assert_ne!(text, renamed, "清单里有 id");
    std::fs::write(path, renamed).expect("写得进");
}

/// 一直问，直到算得出（最多 30 秒）：交回算它的模型编号。
async fn answered(vectors: &Vectors, using: &Using) -> String {
    for _ in 0..600 {
        if let Some(query) = vectors.query(using, "我的猫").await {
            return query.model;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("三十秒还没算出来");
}

/// 一份照 `setup` 接上本机的向量那一路，先算出一条（小程序拉起着）。交回它和造它的那一个。
async fn running(home: &Home, setup: EmbedSetup) -> (Vectors, Embedder) {
    let first = Embedder::new(setup);
    let vectors = Vectors::new(Some(first.clone()), model_data(home));
    assert_eq!(answered(&vectors, &using(home)).await, "local:tiny");
    assert!(first.running().await, "拉起着");
    (vectors, first)
}

#[tokio::test]
async fn another_package_takes_over_and_the_old_one_stops_for_good() {
    let home = Home::new();
    let (vectors, first) = running(&home, tiny_package(&home.scratch.0.join("a/embed"))).await;
    let other = home.scratch.0.join("b/embed");
    let setup = tiny_package(&other);
    rename(&other, "tiny-b");
    vectors.replace_local(Some(setup)).await;
    assert!(!first.running().await, "旧的小程序退了");
    assert!(
        matches!(first.embed("猫").await, Err(Unavailable::Off(_))),
        "旧的副本要的交回用不了"
    );
    assert!(!first.running().await, "不再拉起旧的");
    assert_eq!(vectors.local_name().as_deref(), Some("tiny-b"));
    assert_eq!(answered(&vectors, &using(&home)).await, "local:tiny-b");
}

#[tokio::test]
async fn the_same_setup_changes_nothing() {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages/embed"));
    let (vectors, first) = running(&home, setup.clone()).await;
    vectors.replace_local(Some(setup)).await;
    assert!(first.running().await, "一样的不关");
    first.embed("猫").await.expect("还是那一个，照样算得出");
    assert_eq!(vectors.local_name().as_deref(), Some("tiny"));
}

#[tokio::test]
async fn a_changed_model_list_at_the_same_place_is_a_replacement() {
    let home = Home::new();
    let dir = home.scratch.0.join("packages/embed");
    let setup = tiny_package(&dir);
    let (vectors, first) = running(&home, setup.clone()).await;
    rename(&dir, "tiny-c");
    vectors.replace_local(Some(setup)).await;
    assert!(!first.running().await, "清单变了算换");
    assert_eq!(vectors.local_name().as_deref(), Some("tiny-c"));
    assert_eq!(answered(&vectors, &using(&home)).await, "local:tiny-c");
}

#[tokio::test]
async fn none_takes_the_local_way_away_and_a_setup_brings_it_back() {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages/embed"));
    let (vectors, first) = running(&home, setup.clone()).await;
    vectors.replace_local(None).await;
    assert!(!first.running().await, "关掉了");
    assert_eq!(vectors.local_name(), None);
    assert_eq!(
        vectors.query(&using(&home), "我的猫").await,
        None,
        "没有本机的那一路"
    );
    vectors.replace_local(None).await;
    vectors.replace_local(Some(setup)).await;
    assert_eq!(answered(&vectors, &using(&home)).await, "local:tiny");
}

#[tokio::test]
async fn the_same_model_from_another_place_is_a_replacement() {
    // 出厂的那一份卸掉、家目录装了同样的一份：内容一样，程序照新的位置拉起。
    let home = Home::new();
    let (vectors, first) = running(&home, tiny_package(&home.scratch.0.join("a/embed"))).await;
    vectors
        .replace_local(Some(tiny_package(&home.scratch.0.join("b/embed"))))
        .await;
    assert!(!first.running().await, "包目录换了算换");
    assert_eq!(answered(&vectors, &using(&home)).await, "local:tiny");
}
