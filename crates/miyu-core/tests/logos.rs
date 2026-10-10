//! 供应商的图标（施工 8-31，`docs/blueprint/models.md`「图标」）：先认 models.dev 的默认图，再拉目录里每一家的，收下的存进缓存、
//! 换上；默认图、不是 SVG 的当没有；读缓存照单色、彩色分开。用假服务器，不连外网：一次连接回剧本的一份，所以目录里只放一家，
//! 先后定得住。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use miyu_core::models::logos::{Logos, load};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::logos::{Logo, LogoTable};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed};

/// 测完删的临时目录。
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Dir(std::env::temp_dir().join(format!("miyu-logos-{}-{n}", std::process::id())))
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            // 留在临时目录里，不影响测试。
        }
    }
}

/// 目录里只有编号 `id` 的一家。
fn data(id: &str) -> Arc<ModelData> {
    let text = json!({id: {"id": id, "models": {}}}).to_string();
    let catalog = Catalog::parse(&text).expect("写法对").catalog;
    let data = ModelData::new(Profiles::default(), Vendors::default(), None);
    data.loaded(
        Some(Loaded {
            catalog,
            source: CatalogSource::Cache,
            fetched: "2026-10-11T00:00:00.000Z".to_string(),
        }),
        Observed::default(),
    );
    Arc::new(data)
}

/// 两份地址都指到假服务器；`colored` 是有彩色版的那几家。
fn logos(dir: &Dir, data: &Arc<ModelData>, base: &str, colored: &[&str]) -> Logos {
    Logos {
        data: Arc::clone(data),
        client: fetcher(Proxy::Off).expect("造得出客户端"),
        dir: dir.0.join("models").join("logos"),
        table: LogoTable {
            models_dev: format!("{base}/logos/{{id}}.svg"),
            lobe: format!("{base}/lobe/{{name}}.svg"),
            colored: colored
                .iter()
                .map(|id| ((*id).to_string(), format!("{id}-color")))
                .collect::<BTreeMap<_, _>>(),
        },
    }
}

fn ok(body: &str) -> Reply {
    Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

const DEFAULT: &str = "<svg><circle/></svg>";
const MONO: &str = "<svg><path d=\"M0 0\" fill=\"currentColor\"/></svg>";

#[tokio::test]
async fn a_mono_logo_is_stored_and_shown() {
    let dir = Dir::new();
    let data = data("anthropic");
    let server = Server::start(vec![ok(DEFAULT), ok(MONO)]).await;
    let logos = logos(&dir, &data, &server.base_url, &[]);
    assert_eq!(logos.once().await, Ok(1));
    let want = Logo {
        svg: MONO.to_string(),
        tint: true,
    };
    assert_eq!(data.logo("anthropic"), Some(want.clone()));
    let paths: Vec<String> = server
        .received()
        .iter()
        .map(|got| got.path.clone())
        .collect();
    assert_eq!(
        paths,
        [
            "/v1/logos/miyu-no-such-provider.svg",
            "/v1/logos/anthropic.svg"
        ],
        "先认默认图"
    );
    let cached = load(&dir.0.join("models").join("logos"));
    assert_eq!(cached.get("anthropic"), Some(&want), "存进了缓存");
    assert!(dir.0.join("models/logos/fetched").is_file());
}

#[tokio::test]
async fn the_default_picture_and_non_svg_count_as_none() {
    for body in [DEFAULT, "<html>not here</html>"] {
        let dir = Dir::new();
        let data = data("someone");
        let server = Server::start(vec![ok(DEFAULT), ok(body)]).await;
        assert_eq!(
            logos(&dir, &data, &server.base_url, &[]).once().await,
            Ok(0),
            "{body}"
        );
        assert_eq!(data.logo("someone"), None);
    }
}

#[tokio::test]
async fn a_colored_one_comes_from_lobe_and_is_not_tinted() {
    let dir = Dir::new();
    let data = data("deepseek");
    // 彩色的不和默认图比：models.dev 的默认图只管单色的那一路。
    let server = Server::start(vec![ok(DEFAULT), ok(DEFAULT)]).await;
    let logos = logos(&dir, &data, &server.base_url, &["deepseek"]);
    assert_eq!(logos.once().await, Ok(1));
    assert_eq!(
        data.logo("deepseek"),
        Some(Logo {
            svg: DEFAULT.to_string(),
            tint: false
        })
    );
    assert_eq!(server.received()[1].path, "/v1/lobe/deepseek-color.svg");
    assert!(dir.0.join("models/logos/deepseek.color.svg").is_file());
}

#[tokio::test]
async fn without_the_default_picture_nothing_is_replaced() {
    let dir = Dir::new();
    let data = data("anthropic");
    let server = Server::start(vec![Reply::error(503, &[], "busy")]).await;
    assert!(
        logos(&dir, &data, &server.base_url, &[])
            .once()
            .await
            .is_err()
    );
    assert_eq!(data.logo("anthropic"), None);
    assert!(!dir.0.join("models/logos").exists(), "缓存不动");
}
