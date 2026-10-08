//! 代理和没开口子时的闸（`net.md`「怎么走」第 3、5 条，施工 W-7）：走代理的经本机一台假代理去抓（记下它收到的请求），
//! 先在本机解析一遍，解析出内网的不交给代理，解析不出来的照样交；`NO_PROXY` 里的直连、钉地址；不走代理、解析不出来的
//! 连不上。没开测试的口子时，回环、内网一律不去：连一台真在回环上听着的服务器都不去。
//!
//! 代理照测试的口子给的值（`Testing::proxy`），不改环境变量：同一个进程里别的测试不跟着变。读环境变量那一半在
//! `crates/miyu/tests/link_preview.rs`，真的核心、子进程。

use crate::support::{PNG, Reply, Site, Store, card, ip, miss, page, previewer, resources};
use miyu_net::testkit::Testing;
use miyu_net::{LinkPreview, Why};

/// 测试的口子：代理照 `proxy`，`NO_PROXY` 照 `no_proxy`；`public.test` 解析到测性能的段（当公网，从本机连不上，
/// 只有经代理才到得了），`inner.test` 解析到内网，`site.test` 解析到回环。回环当不当公网照 `loopback_public`。
fn proxied(proxy: &Site, no_proxy: &str, loopback_public: bool) -> Testing {
    Testing {
        loopback_public,
        hosts: vec![
            ("public.test".to_string(), ip("198.18.0.1")),
            ("inner.test".to_string(), ip("10.0.0.1")),
            ("mixed.test".to_string(), ip("198.18.0.2")),
            ("mixed.test".to_string(), ip("192.168.0.2")),
            ("site.test".to_string(), ip("127.0.0.1")),
        ],
        proxy: Some(format!("http://127.0.0.1:{}", proxy.port)),
        no_proxy: no_proxy.to_string(),
    }
}

#[tokio::test]
async fn a_proxied_page_and_its_picture_go_through_the_proxy() {
    let html = page(
        "Through",
        r#"<meta property="og:image" content="/card.png">"#,
    );
    let proxy = Site::start(vec![
        ("http://public.test/page".to_string(), Reply::html(&html)),
        (
            "http://public.test/card.png".to_string(),
            Reply::bytes("image/png", PNG),
        ),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store, proxied(&proxy, "", false));

    let card = card(&links, "http://public.test/page").await;

    assert_eq!(card.title, "Through");
    let image = card.image.expect("图也经代理抓到了");
    assert_eq!(store.blobs.get(&image.blob).unwrap(), PNG);
    // 代理收到的请求行写的是整个地址，Host 是原来的名字：交给代理的不钉地址
    assert_eq!(proxy.hits("http://public.test/page"), 1);
    assert_eq!(proxy.hits("http://public.test/card.png"), 1);
    assert!(
        proxy.seen().iter().all(|seen| seen.host == "public.test"),
        "{:?}",
        proxy.seen()
    );
}

#[tokio::test]
async fn a_name_unknown_here_is_still_handed_to_the_proxy() {
    // 被污染的域名常这样：本机解析不出来，代理那头解析得出来
    let proxy = Site::start(vec![(
        "http://blocked.test/page".to_string(),
        Reply::html(&page("Over there", "")),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store, proxied(&proxy, "", false));
    assert_eq!(
        card(&links, "http://blocked.test/page").await.title,
        "Over there"
    );
    assert_eq!(proxy.hits("http://blocked.test/page"), 1);
}

#[tokio::test]
async fn what_resolves_or_points_inside_is_never_handed_to_the_proxy() {
    // 代理什么都回：交过去就做成卡片了
    let anything = Reply::html(&page("Leaked", ""));
    let mut routes = Vec::new();
    for target in [
        "http://inner.test/",
        "http://mixed.test/",
        "http://10.0.0.1/",
        "http://192.168.1.1/admin",
        "http://169.254.169.254/latest/meta-data/",
        "http://localhost/",
        "http://[::1]/",
    ] {
        routes.push((target.to_string(), anything.clone()));
    }
    let proxy = Site::start(routes).await;
    let store = Store::new();
    let links = previewer(&store, proxied(&proxy, "", false));
    let local = format!("http://127.0.0.1:{}/", proxy.port);
    for url in [
        "http://inner.test/",
        "http://mixed.test/",
        "http://10.0.0.1/",
        "http://192.168.1.1/admin",
        "http://169.254.169.254/latest/meta-data/",
        "http://localhost/",
        "http://[::1]/",
        local.as_str(),
    ] {
        assert_eq!(miss(&links, url).await, Why::NoPreview, "{url}");
    }
    assert!(
        proxy.seen().is_empty(),
        "代理一条都没收到：{:?}",
        proxy.seen()
    );
}

#[tokio::test]
async fn no_proxy_hosts_are_fetched_directly_and_pinned() {
    let proxy = Site::start(Vec::new()).await;
    let site = Site::start(vec![(
        "/page".to_string(),
        Reply::html(&page("Direct", "")),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store, proxied(&proxy, "site.test", true));
    let url = format!("http://site.test:{}/page", site.port);
    assert_eq!(card(&links, &url).await.title, "Direct");
    // 直连：请求行是路径；钉住的地址：系统解析不了 site.test，照样到了
    assert_eq!(site.hits("/page"), 1);
    assert!(proxy.seen().is_empty(), "{:?}", proxy.seen());
}

#[tokio::test]
async fn without_a_proxy_a_name_unknown_here_is_unreachable() {
    let store = Store::new();
    let links = previewer(
        &store,
        Testing {
            loopback_public: true,
            ..Testing::default()
        },
    );
    assert_eq!(
        miss(&links, "http://nowhere.test/page").await,
        Why::Unreachable
    );
}

#[tokio::test]
async fn without_the_test_switch_loopback_and_private_addresses_are_refused() {
    // 一台真在回环上听着的服务器：平时的闸连它都不去
    let site = Site::start(vec![("/page".to_string(), Reply::html(&page("Local", "")))]).await;
    let store = Store::new();
    let links = LinkPreview::new(&resources(), store.blobs.clone());
    for url in [
        site.url("/page"),
        format!("http://[::1]:{}/page", site.port),
        format!("http://[::ffff:127.0.0.1]:{}/page", site.port),
        format!("http://localhost:{}/page", site.port),
        format!("http://127.1:{}/page", site.port),
        format!("http://0x7f000001:{}/page", site.port),
        "http://10.0.0.1/".to_string(),
        "http://169.254.169.254/latest/meta-data/".to_string(),
        "http://[fd00::1]/".to_string(),
    ] {
        assert_eq!(miss(&links, &url).await, Why::NoPreview, "{url}");
    }
    assert!(site.seen().is_empty(), "{:?}", site.seen());
}
