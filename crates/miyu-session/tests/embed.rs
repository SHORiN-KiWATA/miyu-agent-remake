//! 核心接上本机 embedding 的小程序（施工 R-5 中，`docs/blueprint/recall.md` 第四条第 3、4 款）：假服务器给手造的小模型
//! （`crates/miyu-embed/tests/fixtures/tiny/`），真的 `miyu-embed`（cargo 编在测试程序旁边），缓存目录在临时目录里。
//!
//! 第一次要的交回「还没好」、在后台下，下完算得出、和小程序直接算的一样；有了的、核对过的不再下；对不上的删掉重下；下坏
//! 了的不留文件、一小时内不再试；没有小程序、没有缓存目录、清单坏了的用不了；两个一起要的都拿到；闲了它退出，下一条再拉起；
//! 起不来过三次以后不再拉起。

use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_session::{EmbedSetup, Embedder, Unavailable};
use sha2::{Digest, Sha256};

use crate::support::Scratch;

/// 小模型的文件在哪（`miyu-embed` 的测试数据）。
fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/tests/fixtures/tiny")
}

/// cargo 编出来的 `miyu-embed`：和测试程序所在的 `deps/` 同一层（`cargo test --workspace` 会编它）。
fn program() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("在 target/<profile>/deps/ 里");
    let program = dir.join(format!("miyu-embed{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.is_file(),
        "{} 不在：先 cargo build -p miyu-embed",
        program.display()
    );
    program
}

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

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 一个场地：假服务器、清单（地址指到它）、缓存目录。`model` 是服务器交出去的模型文件的内容（清单照真的写）。
struct Stage {
    _scratch: Scratch,
    server: Server,
    manifest: PathBuf,
    cache: PathBuf,
}

impl Stage {
    /// 服务器照先后回 `replies`。
    async fn new(replies: Vec<Reply>) -> Stage {
        let scratch = Scratch::new();
        std::fs::create_dir_all(&scratch.0).expect("建得了");
        let server = Server::start(replies).await;
        let model = std::fs::read(tiny().join("model.onnx")).expect("读得到");
        let vocab = std::fs::read(tiny().join("vocab.txt")).expect("读得到");
        let manifest = scratch.0.join("tiny.toml");
        let text = format!(
            "id = \"tiny\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 6\n\n\
             [[files]]\nrole = \"model\"\nname = \"model.onnx\"\nurl = \"{base}/model.onnx\"\nsha256 = \"{m}\"\nsize = {ms}\n\n\
             [[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nurl = \"{base}/vocab.txt\"\nsha256 = \"{v}\"\nsize = {vs}\n",
            base = server.base_url,
            m = hex(&model),
            ms = model.len(),
            v = hex(&vocab),
            vs = vocab.len(),
        );
        std::fs::write(&manifest, text).expect("写得进");
        let cache = scratch.0.join("cache").join("embed");
        Stage {
            _scratch: scratch,
            server,
            manifest,
            cache,
        }
    }

    fn setup(&self) -> EmbedSetup {
        EmbedSetup {
            program: Some(program()),
            manifest: self.manifest.clone(),
            cache: Some(self.cache.clone()),
            client: miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"),
            idle: Duration::from_secs(600),
        }
    }

    /// 模型放下以后在哪个目录。
    fn dir(&self) -> PathBuf {
        self.cache.join("tiny")
    }

    /// 服务器收到的请求要的是哪个文件，照先后。
    fn asked(&self) -> Vec<String> {
        let name = |path: &str| path.rsplit('/').next().unwrap_or_default().to_string();
        self.server
            .received()
            .iter()
            .map(|r| name(&r.path))
            .collect()
    }
}

/// 服务器交一个文件。
fn file(name: &str) -> Reply {
    let bytes = std::fs::read(tiny().join(name)).expect("读得到");
    Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![Piece::Bytes(bytes)],
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
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("读得了")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[tokio::test]
async fn the_first_call_starts_the_download_and_then_it_answers() {
    let stage = Stage::new(vec![file("model.onnx"), file("vocab.txt")]).await;
    let embedder = Embedder::new(stage.setup());
    assert_eq!(
        embedder.embed("我的猫").await,
        Err(Unavailable::Preparing),
        "第一次不等"
    );
    let vector = settled(&embedder, "我的猫").await.expect("下完算得出");
    assert!(same(&vector, &cat()), "{vector:?}");
    assert_eq!(stage.asked(), ["model.onnx", "vocab.txt"]);
    assert_eq!(
        listed(&stage.dir()),
        ["model.onnx", "vocab.txt"],
        "临时文件不留"
    );
    assert!(embedder.running().await, "拉起来了");
}

#[tokio::test]
async fn files_already_there_are_checked_and_not_downloaded_again() {
    let stage = Stage::new(Vec::new()).await;
    std::fs::create_dir_all(stage.dir()).expect("建得了");
    for name in ["model.onnx", "vocab.txt"] {
        std::fs::copy(tiny().join(name), stage.dir().join(name)).expect("放得下");
    }
    // 不是清单里的（崩了留下的临时文件、旧的）删掉。
    std::fs::write(stage.dir().join(".model.onnx.1-0.tmp"), b"half").expect("写得进");
    let embedder = Embedder::new(stage.setup());
    let vector = settled(&embedder, "我的猫").await.expect("算得出");
    assert!(same(&vector, &cat()));
    assert!(
        stage.asked().is_empty(),
        "核对过的不再下：{:?}",
        stage.asked()
    );
    assert_eq!(listed(&stage.dir()), ["model.onnx", "vocab.txt"]);
}

#[tokio::test]
async fn a_file_that_does_not_match_is_downloaded_again() {
    let stage = Stage::new(vec![file("model.onnx")]).await;
    std::fs::create_dir_all(stage.dir()).expect("建得了");
    // 一样大、内容不对：只比大小看不出来，要照 SHA-256 核对。
    let size = std::fs::metadata(tiny().join("model.onnx"))
        .expect("在")
        .len();
    let garbage = vec![b'x'; usize::try_from(size).expect("不大")];
    std::fs::write(stage.dir().join("model.onnx"), garbage).expect("写得进");
    std::fs::copy(tiny().join("vocab.txt"), stage.dir().join("vocab.txt")).expect("放得下");
    let embedder = Embedder::new(stage.setup());
    let vector = settled(&embedder, "我的猫").await.expect("重下以后算得出");
    assert!(same(&vector, &cat()));
    assert_eq!(stage.asked(), ["model.onnx"], "只重下对不上的那一个");
}

#[tokio::test]
async fn a_bad_download_leaves_nothing_and_is_not_tried_again_soon() {
    let wrong = Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![Piece::Bytes(b"same size? no, wrong bytes".to_vec())],
    };
    let stage = Stage::new(vec![wrong, file("vocab.txt")]).await;
    let embedder = Embedder::new(stage.setup());
    let first = settled(&embedder, "我的猫").await;
    assert!(
        matches!(&first, Err(Unavailable::Off(why)) if why.contains("model.onnx")),
        "{first:?}"
    );
    assert!(
        listed(&stage.dir()).is_empty(),
        "对不上的不留：{:?}",
        listed(&stage.dir())
    );
    let again = embedder.embed("我的猫").await;
    assert!(matches!(again, Err(Unavailable::Off(_))), "{again:?}");
    assert_eq!(stage.asked(), ["model.onnx"], "一小时内不再试");
}

#[tokio::test]
async fn an_oversized_or_broken_download_is_refused() {
    let big = Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![Piece::Bytes(vec![b'x'; 1 << 20])],
    };
    let stage = Stage::new(vec![big]).await;
    let embedder = Embedder::new(stage.setup());
    let first = settled(&embedder, "我的猫").await;
    let size = std::fs::metadata(tiny().join("model.onnx"))
        .expect("在")
        .len();
    let over = format!("over {size} bytes");
    assert!(
        matches!(&first, Err(Unavailable::Off(why)) if why.contains(&over)),
        "下的时候就停：{first:?}"
    );
    assert!(listed(&stage.dir()).is_empty());

    let stage = Stage::new(vec![Reply::error(404, &[], "gone")]).await;
    let embedder = Embedder::new(stage.setup());
    let first = settled(&embedder, "我的猫").await;
    assert!(
        matches!(&first, Err(Unavailable::Off(why)) if why.contains("HTTP 404")),
        "{first:?}"
    );
}

#[tokio::test]
async fn without_the_program_the_cache_or_a_manifest_it_is_off() {
    let stage = Stage::new(Vec::new()).await;
    let mut setup = stage.setup();
    setup.program = None;
    let off = Embedder::new(setup).embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("miyu-embed")),
        "{off:?}"
    );
    let mut setup = stage.setup();
    setup.cache = None;
    let off = Embedder::new(setup).embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("cache")),
        "{off:?}"
    );
    let mut setup = stage.setup();
    setup.manifest = stage.manifest.with_file_name("nothing.toml");
    let off = Embedder::new(setup).embed("猫").await;
    assert!(
        matches!(&off, Err(Unavailable::Off(why)) if why.contains("cannot read")),
        "{off:?}"
    );
    assert!(stage.asked().is_empty(), "用不了的不下");
}

#[tokio::test]
async fn two_at_once_both_get_answers() {
    let stage = Stage::new(vec![file("model.onnx"), file("vocab.txt")]).await;
    let embedder = Embedder::new(stage.setup());
    settled(&embedder, "我的猫").await.expect("好了");
    let (a, b) = tokio::join!(embedder.embed("我的猫"), embedder.embed("我的猫"));
    assert!(same(&a.expect("算得出"), &cat()));
    assert!(same(&b.expect("算得出"), &cat()));
}

#[tokio::test]
async fn it_leaves_when_idle_and_comes_back_for_the_next_one() {
    let stage = Stage::new(vec![file("model.onnx"), file("vocab.txt")]).await;
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
    // 模型文件是坏的、清单照它的哈希写：下得下来、核对得上，小程序载入不了。
    let stage = Stage::new(Vec::new()).await;
    let broken = b"not an onnx model\n";
    let text = std::fs::read_to_string(&stage.manifest).expect("读得到");
    let model = std::fs::read(tiny().join("model.onnx")).expect("读得到");
    let text = text.replace(&hex(&model), &hex(broken)).replace(
        &format!("size = {}", model.len()),
        &format!("size = {}", broken.len()),
    );
    std::fs::write(&stage.manifest, text).expect("写得进");
    std::fs::create_dir_all(stage.dir()).expect("建得了");
    std::fs::write(stage.dir().join("model.onnx"), broken).expect("写得进");
    std::fs::copy(tiny().join("vocab.txt"), stage.dir().join("vocab.txt")).expect("放得下");
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

/// 量尺（验收时手动跑，联网）：照出厂的清单真从 Release 下一次 bge-small-zh-v1.5，量下载、第一次拉起、一条的时间。
#[tokio::test]
#[ignore = "联网下真模型（24 MB）：验收时手动跑，cargo test -p miyu-session --test all measure_the_real_model -- --ignored --nocapture"]
async fn measure_the_real_model() {
    let scratch = Scratch::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/models/embed/bge-small-zh-v1.5.toml");
    let embedder = Embedder::new(EmbedSetup {
        program: Some(program()),
        manifest,
        cache: Some(scratch.0.join("embed")),
        client: miyu_http::fetcher(miyu_http::Proxy::FromEnvironment).expect("造得出"),
        idle: Duration::from_secs(600),
    });
    let started = std::time::Instant::now();
    assert_eq!(embedder.embed("猫").await, Err(Unavailable::Preparing));
    let mut first = None;
    for _ in 0..6000 {
        match embedder.embed("用户养了一只猫，叫团子，三岁").await {
            Err(Unavailable::Preparing) => tokio::time::sleep(Duration::from_millis(100)).await,
            other => {
                first = Some(other);
                break;
            }
        }
    }
    let vector = first.expect("十分钟内好了").expect("算得出");
    eprintln!(
        "下载加第一次拉起、算一条：{:?}，{} 维",
        started.elapsed(),
        vector.len()
    );
    let timed = std::time::Instant::now();
    for _ in 0..50 {
        embedder
            .embed("回答先说结论，再给依据")
            .await
            .expect("算得出");
    }
    eprintln!("拉起着的一条：{:?}", timed.elapsed() / 50);
}
