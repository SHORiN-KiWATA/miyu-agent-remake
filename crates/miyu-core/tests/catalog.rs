//! 读目录、后台更新（施工 8-7，`docs/blueprint/models.md`「怎么走」第二条第 1 到 3 条）：快照和缓存挑新的、坏的退回另一份、
//! 都坏照样起来；后台拉、304、失败一小时后再试、关掉 `update` 不拉。用假服务器，不连外网。地址写成环境变量的引用的照核心
//! 的环境取（施工 8-8）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde_json::json;
use tokio::sync::watch;

use miyu_config::secret::Reference;
use miyu_config::{Address, Value, Values};
use miyu_core::models::catalog::{FILE, META, Meta, Places, load};
use miyu_core::models::refresh::{RETRY, Refreshed, Refresher, Schedule, next_try};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::catalog::CatalogSource;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::CatalogSettings;
use miyu_session::{ModelData, Observed};

/// 用完就删的临时目录。
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "miyu-catalog-{}-{}-{n}",
            std::process::id(),
            stamp()
        ));
        std::fs::create_dir_all(&dir).expect("建得了");
        Dir(dir)
    }

    fn at(&self, name: &str) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir_all(&dir).expect("建得了");
        dir
    }
}

impl Drop for Dir {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 只有一家 `p`、一个模型 `model` 的目录。
fn catalog(model: &str) -> String {
    json!({"p": {"id": "p", "models": {model: {"limit": {"context": 1000}}}}}).to_string()
}

/// 在 `dir` 写一份目录 `text`，`fetched` 是什么时候拉的（没有的不写 `meta`）。
fn put(dir: &Path, text: &str, fetched: Option<&str>, etag: Option<&str>) {
    std::fs::write(dir.join(FILE), text).expect("写得进");
    if let Some(fetched) = fetched {
        let meta = Meta {
            source: "https://models.dev/api.json".to_string(),
            fetched: fetched.to_string(),
            etag: etag.map(str::to_string),
        };
        std::fs::write(dir.join(META), meta.to_json()).expect("写得进");
    }
}

/// 在用的目录里有没有模型 `model`，和它是哪一份。
fn has(places: &Places, model: &str) -> Option<(bool, CatalogSource)> {
    let loaded = load(places)?;
    Some((loaded.catalog.model("p", model).is_some(), loaded.source))
}

const OLD: &str = "2026-09-01T00:00:00.000Z";
const NEW: &str = "2026-10-01T00:00:00.000Z";

#[test]
fn the_newer_copy_wins_and_a_broken_one_falls_back() {
    let dir = Dir::new();
    let places = Places {
        snapshot: dir.at("snapshot"),
        cache: Some(dir.at("cache")),
    };
    put(&places.snapshot, &catalog("old"), Some(OLD), None);
    put(
        dir.at("cache").as_path(),
        &catalog("new"),
        Some(NEW),
        Some("\"e\""),
    );
    assert_eq!(
        has(&places, "new"),
        Some((true, CatalogSource::Cache)),
        "缓存新"
    );
    put(
        &places.snapshot,
        &catalog("newest"),
        Some("2026-12-01T00:00:00.000Z"),
        None,
    );
    assert_eq!(
        has(&places, "newest"),
        Some((true, CatalogSource::Snapshot)),
        "快照新"
    );
    // 新的坏了：退回另一份。
    put(
        &places.snapshot,
        "{not json",
        Some("2026-12-01T00:00:00.000Z"),
        None,
    );
    assert_eq!(has(&places, "new"), Some((true, CatalogSource::Cache)));
    // 没有 meta 的当最旧。
    put(&places.snapshot, &catalog("nometa"), None, None);
    std::fs::remove_file(places.snapshot.join(META)).expect("删得掉");
    assert_eq!(has(&places, "new"), Some((true, CatalogSource::Cache)));
    // 都坏了：没有目录，照样起来。
    put(dir.at("cache").as_path(), "[]", Some(NEW), None);
    put(&places.snapshot, "", Some(OLD), None);
    assert_eq!(has(&places, "x"), None);
    // 没有缓存目录、缓存里还没有：只读快照。
    let only = Places {
        snapshot: places.snapshot.clone(),
        cache: None,
    };
    put(&only.snapshot, &catalog("snap"), Some(OLD), None);
    assert_eq!(has(&only, "snap"), Some((true, CatalogSource::Snapshot)));
    let empty_cache = Places {
        snapshot: only.snapshot.clone(),
        cache: Some(dir.at("never")),
    };
    assert_eq!(
        has(&empty_cache, "snap"),
        Some((true, CatalogSource::Snapshot))
    );
}

#[test]
fn the_next_try_waits_for_every_or_an_hour_after_a_failure() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    let day = Duration::from_secs(86_400);
    assert_eq!(
        next_try(now, None, day, None),
        Duration::ZERO,
        "缓存里没有：马上拉"
    );
    let fetched = now - Duration::from_secs(3600);
    assert_eq!(
        next_try(now, Some(fetched), day, None),
        day - Duration::from_secs(3600)
    );
    assert_eq!(
        next_try(now, Some(now - 2 * day), day, None),
        Duration::ZERO,
        "旧过了"
    );
    assert_eq!(
        next_try(
            now,
            Some(now - 2 * day),
            day,
            Some(now - Duration::from_secs(60))
        ),
        RETRY - Duration::from_secs(60),
        "失败了一小时后再试"
    );
    assert_eq!(RETRY, Duration::from_secs(3600));
}

/// 读完了、没有目录的模型资料。
fn data() -> Arc<ModelData> {
    let data = ModelData::new(Profiles::default(), Vendors::default(), None);
    data.loaded(None, Observed::default());
    Arc::new(data)
}

fn refresher(dir: &Dir, data: &Arc<ModelData>) -> Refresher {
    Refresher {
        data: Arc::clone(data),
        client: fetcher(Proxy::Off).expect("造得出客户端"),
        cache: dir.at("cache"),
    }
}

fn ok(body: &str, etag: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![("ETag".to_string(), etag.to_string())],
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

#[tokio::test]
async fn a_refresh_writes_the_cache_and_swaps_the_catalog() {
    let dir = Dir::new();
    let data = data();
    let refresher = refresher(&dir, &data);
    let server = Server::start(vec![
        ok(&catalog("fresh"), "\"v1\""),
        Reply::error(304, &[], ""),
        Reply::error(503, &[], "busy"),
        ok("{}", "\"v2\""),
    ])
    .await;
    let url = format!("{}/api.json", server.base_url);
    // 200：写进缓存（连同 meta 和 ETag），换上。
    assert_eq!(refresher.once(&url).await, Refreshed::Replaced);
    let loaded = data.catalog().expect("换上了");
    assert_eq!(loaded.source, CatalogSource::Cache);
    assert!(loaded.catalog.model("p", "fresh").is_some());
    assert_eq!(
        std::fs::read_to_string(refresher.cache.join(FILE)).expect("写了"),
        catalog("fresh"),
        "原样的字节"
    );
    let meta = Meta::read(&refresher.cache).expect("写了 meta");
    assert_eq!(
        (meta.source.as_str(), meta.etag.as_deref()),
        (url.as_str(), Some("\"v1\""))
    );
    assert_eq!(meta.fetched, loaded.fetched);
    // 304：带上次的 ETag 去问，只改 meta 的时刻。
    let mut older = meta.clone();
    older.fetched = OLD.to_string();
    std::fs::write(refresher.cache.join(META), older.to_json()).expect("写得进");
    assert_eq!(refresher.once(&url).await, Refreshed::NotModified);
    assert_eq!(server.received()[1].header("if-none-match"), Some("\"v1\""));
    let after = Meta::read(&refresher.cache).expect("有");
    assert!(after.fetched.as_str() > OLD, "{}", after.fetched);
    assert_eq!(after.etag.as_deref(), Some("\"v1\""));
    // 503、一家供应商都没有的：失败，缓存不动。
    assert_eq!(refresher.once(&url).await, Refreshed::Failed);
    assert_eq!(refresher.once(&url).await, Refreshed::Failed);
    assert_eq!(
        std::fs::read_to_string(refresher.cache.join(FILE)).expect("还在"),
        catalog("fresh")
    );
}

#[tokio::test]
async fn the_loop_fetches_when_due_and_not_when_update_is_off() {
    let dir = Dir::new();
    let data = data();
    let server = Server::start(vec![ok(&catalog("looped"), "\"v1\"")]).await;
    let url = format!("{}/api.json", server.base_url);
    let settings = |update: bool| Schedule {
        update,
        url: Some(url.clone()),
        every: Duration::from_secs(86_400),
    };
    // 关着：不拉。
    let (sender, receiver) = watch::channel(settings(false));
    let running = tokio::spawn(refresher(&dir, &data).run(receiver));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(server.received().is_empty(), "关掉 update 不拉");
    // 打开（当场生效）：缓存里没有，马上拉。
    sender.send_replace(settings(true));
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while data.catalog().is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "打开以后拉了");
    assert_eq!(server.received().len(), 1);
    // 刚拉过：等一天，不再拉。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.received().len(), 1);
    drop(sender);
    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("配置没了就停")
        .expect("没有 panic");
}

/// 地址写成环境变量的引用（施工 8-8，8-6b 留下的：以前读成空的、一直拉不到）：照核心的环境取，后台照它拉；没设、设成空的
/// 没有地址，到点了照失败算。
#[tokio::test]
async fn an_address_from_the_environment_is_fetched_from_where_it_points() {
    let mut values = Values::default();
    values.set(
        "models.catalog.url",
        Value::Secret(Reference::Env("CAT_URL".to_string())),
    );
    let settings = CatalogSettings::from(&values);
    assert_eq!(settings.url, Address::Env("CAT_URL".to_string()));
    let server = Server::start(vec![ok(&catalog("from-env"), "\"v1\"")]).await;
    let url = format!("{}/api.json", server.base_url);
    let env = |name: &str| (name == "CAT_URL").then(|| url.clone());
    let schedule = Schedule::of(settings.clone(), &env);
    assert_eq!(schedule.url.as_deref(), Some(url.as_str()));
    assert_eq!(Schedule::of(settings.clone(), &|_| None).url, None, "没设");
    assert_eq!(
        Schedule::of(settings, &|_| Some(String::new())).url,
        None,
        "设成空的"
    );
    let dir = Dir::new();
    let data = data();
    let (sender, receiver) = watch::channel(schedule);
    let running = tokio::spawn(refresher(&dir, &data).run(receiver));
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while data.catalog().is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "照环境变量里的地址拉到了");
    assert_eq!(server.received().len(), 1);
    drop(sender);
    running.await.expect("没有 panic");
    // 没有地址的：到点了什么都拉不到，照失败算，接着等配置变。
    let schedule = Schedule {
        update: true,
        url: None,
        every: Duration::from_secs(86_400),
    };
    let (sender, receiver) = watch::channel(schedule);
    let (other, nothing) = (Dir::new(), self::data());
    let running = tokio::spawn(refresher(&other, &nothing).run(receiver));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(nothing.catalog().is_none());
    assert!(!running.is_finished(), "没有地址不停下");
    drop(sender);
    running.await.expect("没有 panic");
}

/// 量尺（施工 8-7，「起草时定的」第 13 条，预算 150 毫秒）：读安装包带的快照要多久，读五次各印一行。
/// `cargo test --release -p miyu-core --test catalog -- --ignored --nocapture`。只断言读得出来，不断言耗时。
#[test]
#[ignore = "量尺：看耗时，照 release 量"]
fn measure_reading_the_bundled_snapshot() {
    let places = Places {
        snapshot: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/models"),
        cache: None,
    };
    for round in 1..=5 {
        let started = std::time::Instant::now();
        let loaded = load(&places).expect("读得出来");
        println!(
            "round {round}: {} ms, {} providers, {} models",
            started.elapsed().as_millis(),
            loaded.catalog.providers().count(),
            loaded.catalog.model_count()
        );
    }
}

/// 纳秒时刻：临时目录名里加上它，Windows 很快复用进程号，光靠进程号和序号会撞上前一个测试进程留下的目录。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}
