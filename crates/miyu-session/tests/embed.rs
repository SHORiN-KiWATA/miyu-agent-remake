//! 核心接上本机 embedding 的小程序（施工 R-5 中；R-5 三补改成照包，`docs/blueprint/recall.md` 第四条第 3、4 款）：手造的小模型
//! （`crates/miyu-embed/tests/fixtures/tiny/`）放进一份包目录（模型清单 `model.toml` 加两个文件），真的 `miyu-embed`（cargo
//! 编在测试程序旁边），都在临时目录里。不联网：没有下载那一条路了。
//!
//! 第一次要的交回「还没好」、在后台核对，核对完算得出、和小程序直接算的一样，包里的文件一个不动；对不上的、少了的这一回用
//! 不了、不删；没有小程序、模型清单读不了的用不了；两个一起要的都拿到；闲了它退出，下一条再拉起；起不来过三次以后不再拉起。

use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_session::{EmbedSetup, Embedder, Unavailable};

use crate::support::Scratch;
use crate::support::package::{package, tiny, tiny_package};

/// 小模型对「我的猫」（编号 [2, 9, 10, 5, 3]）算出的向量（`expected.json` 的头一条）。
fn cat() -> Vec<f32> {
    let text = std::fs::read_to_string(tiny().join("expected.json")).expect("读得到");
    let expected: serde_json::Value = serde_json::from_str(&text).expect("合写法");
    expected[0]["vector"]
        .as_array()
        .expect("有向量")
        .iter()
        .map(|value| value.as_f64().expect("是数") as f32)
        .collect()
}

/// 一份临时的包目录（`packages/embed/`）和照它接上要的。
struct Stage {
    _scratch: Scratch,
    dir: PathBuf,
    setup: EmbedSetup,
}

impl Stage {
    /// 模型文件的内容是 `model`，清单照它写。
    fn with(model: &[u8]) -> Stage {
        let scratch = Scratch::new();
        let dir = scratch.0.join("packages").join("embed");
        let setup = package(&dir, model);
        Stage {
            _scratch: scratch,
            dir,
            setup,
        }
    }

    /// 小模型原样放着的。
    fn new() -> Stage {
        let scratch = Scratch::new();
        let dir = scratch.0.join("packages").join("embed");
        let setup = tiny_package(&dir);
        Stage {
            _scratch: scratch,
            dir,
            setup,
        }
    }

    fn setup(&self) -> EmbedSetup {
        self.setup.clone()
    }
}

/// 一直要，直到不再是「还没好」（最多 30 秒）。
async fn settled(embedder: &Embedder, text: &str) -> Result<Vec<f32>, Unavailable> {
    for _ in 0..600 {
        match embedder.embed(text).await {
            Err(Unavailable::Preparing) => tokio::time::sleep(Duration::from_millis(50)).await,
            other => return other,
        }
    }
    panic!("三十秒还没好");
}

/// 每一格差不过 1e-6。
fn same(got: &[f32], want: &[f32]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() <= 1e-6)
}

/// 目录里有哪些文件，照名字排。
fn listed(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("读得了")
        .map(|entry| {
            entry
                .expect("读得了")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn the_first_call_checks_the_package_and_then_it_answers() {
    let stage = Stage::new();
    let embedder = Embedder::new(stage.setup());
    assert_eq!(embedder.embed("我的猫").await, Err(Unavailable::Preparing));
    let vector = settled(&embedder, "我的猫").await.expect("算得出");
    assert!(same(&vector, &cat()), "{vector:?}");
    assert_eq!(
        listed(&stage.dir),
        ["model.onnx", "model.toml", "vocab.txt"],
        "包里的一个不动"
    );
    assert_eq!(embedder.model().as_deref(), Some("local:tiny"));
}

#[tokio::test]
async fn a_file_that_does_not_match_or_is_missing_is_off_and_left_alone() {
    let stage = Stage::new();
    let model = std::fs::read(tiny().join("model.onnx")).expect("读得到");
    let mut wrong = model.clone();
    wrong[0] ^= 1;
    std::fs::write(stage.dir.join("model.onnx"), &wrong).expect("写得进");
    let off = settled(&Embedder::new(stage.setup()), "猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("model.onnx") && why.contains("does not match")),
        "{off:?}"
    );
    assert_eq!(
        std::fs::read(stage.dir.join("model.onnx")).expect("还在"),
        wrong,
        "不删、不改"
    );
    let stage = Stage::new();
    std::fs::remove_file(stage.dir.join("vocab.txt")).expect("删得掉");
    let off = settled(&Embedder::new(stage.setup()), "猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("vocab.txt") && why.contains("missing")),
        "{off:?}"
    );
    let embedder = Embedder::new(stage.setup());
    settled(&embedder, "猫").await.expect_err("用不了");
    assert!(
        matches!(embedder.embed("猫").await, Err(Unavailable::Off(_))),
        "这一回不再核对：装卸要重启核心"
    );
}

#[tokio::test]
async fn without_the_program_or_a_manifest_it_is_off() {
    let stage = Stage::new();
    let mut setup = stage.setup();
    setup.program = None;
    let off = Embedder::new(setup).embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("miyu-embed")),
        "{off:?}"
    );
    let mut setup = stage.setup();
    setup.manifest = stage.dir.join("nothing.toml");
    let embedder = Embedder::new(setup);
    let off = embedder.embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("cannot read")),
        "{off:?}"
    );
    assert_eq!(embedder.model(), None);
}

#[tokio::test]
async fn two_at_once_both_get_answers() {
    let stage = Stage::new();
    let embedder = Embedder::new(stage.setup());
    settled(&embedder, "我的猫").await.expect("好了");
    let (a, b) = tokio::join!(embedder.embed("我的猫"), embedder.embed("我的猫"));
    assert!(same(&a.expect("算得出"), &cat()));
    assert!(same(&b.expect("算得出"), &cat()));
}

#[tokio::test]
async fn it_leaves_when_idle_and_comes_back_for_the_next_one() {
    let stage = Stage::new();
    let mut setup = stage.setup();
    setup.idle = Duration::from_millis(300);
    let embedder = Embedder::new(setup);
    settled(&embedder, "我的猫").await.expect("好了");
    assert!(embedder.running().await);
    for _ in 0..100 {
        if !embedder.running().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!embedder.running().await, "闲了退出");
    let vector = embedder.embed("我的猫").await.expect("下一条再拉起");
    assert!(same(&vector, &cat()));
    assert!(embedder.running().await);
}

#[tokio::test]
async fn a_program_that_cannot_start_is_given_up_after_three_tries() {
    // 模型文件是坏的、清单照它的哈希写：核对得上，小程序载入不了。
    let stage = Stage::with(b"not an onnx model\n");
    let embedder = Embedder::new(stage.setup());
    for k in 0..3 {
        let failed = settled(&embedder, "猫").await;
        assert!(
            matches!(&failed, Err(Unavailable::Failed(why)) if why.contains("cannot load")),
            "第 {k} 次：{failed:?}"
        );
    }
    let off = embedder.embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("3 times")),
        "{off:?}"
    );
}
