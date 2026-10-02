//! 本机的假服务器上整条走通（`net.md`「守着它的」，施工 W-7）：钉住的地址、元数据、图存成 blob、记着的不再抓、
//! blob 没了交 `null`；跳转每一跳过闸、最多 5 跳；不是 HTML、没有标题、4xx、5xx；读到 `</head>` 就停、最多 2 MiB；
//! 图只收五种、最多 3 MiB。假服务器在回环上，用测试的口子（`testkit`）把回环当公网、把 `site.test` 解析到回环。
//! 代理、没开口子时的闸在 `proxy.rs`。

mod support;

use miyu_net::{Preview, Why};
use support::{PNG, Reply, Site, Store, card, local, miss, page, previewer};

#[tokio::test]
async fn a_page_on_the_fake_server_becomes_a_card_with_its_pictures_stored() {
    let html = page(
        "Fallback",
        r#"<meta property="og:title" content="Miyu &amp; friends">
        <meta property="og:description" content="  a   card  ">
        <meta property="og:image" content="img/card.png">
        <link rel="shortcut icon" href="/favicon.gif">"#,
    );
    let gif = b"GIF89a\x01\x00\x01\x00".to_vec();
    let site = Site::start(vec![
        ("/a/page".to_string(), Reply::html(&html)),
        (
            "/a/img/card.png".to_string(),
            Reply::bytes("image/png", PNG),
        ),
        // 对方说的类型不算，照魔数认
        ("/favicon.gif".to_string(), Reply::bytes("text/plain", &gif)),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store, local());
    let url = format!("http://site.test:{}/a/page", site.port);

    let card = card(&links, &format!("  {url}  ")).await;

    assert_eq!(card.url, url);
    assert_eq!(card.title, "Miyu & friends");
    assert_eq!(card.description, "a card");
    assert_eq!(card.site, "site.test", "没写 og:site_name 的用主机名");
    let image = card.image.expect("有图");
    assert_eq!(image.media_type, "image/png");
    assert_eq!(store.blobs.get(&image.blob).unwrap(), PNG, "图存成了 blob");
    let icon = card.icon.expect("有图标");
    assert_eq!(icon.media_type, "image/gif");
    assert_eq!(store.blobs.get(&icon.blob).unwrap(), gif);
    // 连的是钉住的地址：系统的 DNS 解析不了 site.test，请求照样到了这台服务器，Host 还是原来的名字
    let seen = site.seen();
    assert!(
        seen.iter()
            .all(|seen| seen.host == format!("site.test:{}", site.port)),
        "{seen:?}"
    );
    assert!(
        seen.iter().all(|seen| seen.user_agent.contains("Mozilla")),
        "请求头照 link_preview.json"
    );
}

#[tokio::test]
async fn a_remembered_card_is_not_fetched_again_and_a_missing_blob_turns_null() {
    let html = page("Kept", r#"<meta property="og:image" content="/card.png">"#);
    let site = Site::start(vec![
        ("/page".to_string(), Reply::html(&html)),
        ("/card.png".to_string(), Reply::bytes("image/png", PNG)),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store, local());
    let url = site.url("/page");

    let first = card(&links, &url).await;
    let second = card(&links, &url).await;
    assert_eq!(first, second);
    assert_eq!(site.hits("/page"), 1, "记着的不再抓");
    assert!(first.icon.is_none(), "没有 /favicon.ico：那一格是空的");

    // 图的 blob 没了：记着的卡片那一格交 null，卡片照样成立
    let image = first.image.expect("有图");
    std::fs::remove_file(store.blobs.path(&image.blob)).unwrap();
    let third = card(&links, &url).await;
    assert_eq!(third.image, None);
    assert_eq!(third.title, "Kept");
    assert_eq!(site.hits("/page"), 1);
}

#[tokio::test]
async fn redirects_are_followed_up_to_five_and_every_hop_is_guarded() {
    let target = page("Landed", "");
    let mut routes = vec![("/page".to_string(), Reply::html(&target))];
    // /r1 → /r2 → … → /r6 → /page：从 /r2 起正好 5 次跳转，从 /r1 起 6 次。相对的 Location 照这一跳的地址接
    for hop in 1..6 {
        routes.push((
            format!("/r{hop}"),
            Reply::redirect(&format!("r{}", hop + 1)),
        ));
    }
    routes.push(("/r6".to_string(), Reply::redirect("/page")));
    for (path, location) in [
        ("/to-private", "http://10.0.0.1/"),
        ("/to-inner", "http://inner.test/"),
        ("/to-localhost", "http://localhost/page"),
        ("/to-ftp", "ftp://example.com/"),
        ("/to-metadata", "http://169.254.169.254/latest/"),
    ] {
        routes.push((path.to_string(), Reply::redirect(location)));
    }
    routes.push(("/no-location".to_string(), Reply::status(302)));
    let site = Site::start(routes).await;
    let store = Store::new();
    let links = previewer(&store, local());

    let landed = card(&links, &site.url("/r2")).await;
    assert_eq!(landed.url, site.url("/page"), "卡片写最后落到的那一页");
    assert_eq!(
        miss(&links, &site.url("/r1")).await,
        Why::NoPreview,
        "6 次跳转太多"
    );
    assert_eq!(site.hits("/page"), 1, "第 6 次跳转不再跟");
    for path in [
        "/to-private",
        "/to-inner",
        "/to-localhost",
        "/to-ftp",
        "/to-metadata",
    ] {
        assert_eq!(
            miss(&links, &site.url(path)).await,
            Why::NoPreview,
            "{path}：跳到的那一跳过不了闸"
        );
    }
    assert_eq!(
        miss(&links, &site.url("/no-location")).await,
        Why::Unreachable
    );
}

#[tokio::test]
async fn what_is_not_a_titled_page_gets_no_card() {
    let site = Site::start(vec![
        (
            "/text".to_string(),
            Reply::bytes("text/plain", b"<title>x</title>"),
        ),
        (
            "/untyped".to_string(),
            Reply::bytes("", b"<title>x</title>"),
        ),
        (
            "/untitled".to_string(),
            Reply::html("<head><meta name=description content=d></head>"),
        ),
        ("/gone".to_string(), Reply::status(404)),
        ("/broken".to_string(), Reply::status(500)),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store, local());
    for (path, why) in [
        ("/text", Why::NoPreview),
        ("/untyped", Why::NoPreview),
        ("/untitled", Why::NoPreview),
        ("/gone", Why::Unreachable),
        ("/broken", Why::Unreachable),
    ] {
        assert_eq!(miss(&links, &site.url(path)).await, why, "{path}");
    }
    assert_eq!(miss(&links, "not a url").await, Why::NotAUrl);
    assert_eq!(miss(&links, "example.com/page").await, Why::NotAUrl);
    assert_eq!(
        miss(&links, "ftp://example.com/").await,
        Why::UnsupportedScheme
    );
    assert_eq!(
        miss(&links, "javascript:alert(1)").await,
        Why::UnsupportedScheme
    );
    // 没有 Location 的跳转、4xx 都只到过这台服务器一次：读不成地址的不抓
    assert_eq!(site.seen().len(), 5);
}

#[tokio::test]
async fn the_head_is_read_until_its_end_and_at_most_two_mib() {
    let two_mib = 2 * 1024 * 1024;
    let filler = "x".repeat(two_mib + 1024 * 1024);
    let late = format!("<html><head>{filler}<title>Too late</title>");
    let early = format!("<html><head><title>Early</title>{filler}");
    let site = Site::start(vec![
        // 读到 </head> 不停的，要一直等到这一跳的时限
        (
            "/head".to_string(),
            Reply::html("<html><head><title>Head</title></head><body>").stalled(),
        ),
        (
            "/body".to_string(),
            Reply::html("<html><head><title>Body</title><body>").stalled(),
        ),
        // 2 MiB 以后的标题读不到；读满 2 MiB 就停，不等对方说完
        ("/late".to_string(), Reply::html(&late).stalled()),
        ("/early".to_string(), Reply::html(&early).stalled()),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store, local());
    assert_eq!(card(&links, &site.url("/head")).await.title, "Head");
    assert_eq!(card(&links, &site.url("/body")).await.title, "Body");
    assert_eq!(card(&links, &site.url("/early")).await.title, "Early");
    assert_eq!(miss(&links, &site.url("/late")).await, Why::NoPreview);
}

#[tokio::test]
async fn only_five_image_kinds_up_to_three_mib_are_kept() {
    let three_mib = 3 * 1024 * 1024;
    let png_of = |length: usize| {
        let mut bytes = PNG.to_vec();
        bytes.resize(length, 0);
        bytes
    };
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    let pages = [
        ("/svg", "/i.svg", Reply::bytes("image/svg+xml", svg)),
        (
            "/full",
            "/full.png",
            Reply::bytes("image/png", &png_of(three_mib)),
        ),
        (
            "/over",
            "/over.png",
            Reply::bytes("image/png", &png_of(three_mib + 1)),
        ),
        (
            "/over-unsized",
            "/over-unsized.png",
            Reply::bytes("image/png", &png_of(three_mib + 1)).without_length(),
        ),
        (
            "/webp",
            "/i.webp",
            Reply::bytes("image/webp", b"RIFF\x24\x00\x00\x00WEBPVP8 "),
        ),
        (
            "/jpeg",
            "/i.jpg",
            Reply::bytes("image/jpeg", b"\xff\xd8\xff\xe0\x00\x10JFIF"),
        ),
        (
            "/ico",
            "/i.ico",
            Reply::bytes("image/x-icon", b"\x00\x00\x01\x00\x01\x00"),
        ),
        ("/missing", "/nothing.png", Reply::status(404)),
    ];
    let mut routes = Vec::new();
    for (path, image, reply) in &pages {
        let html = page(
            path,
            &format!(r#"<meta property="og:image" content="{image}">"#),
        );
        routes.push(((*path).to_string(), Reply::html(&html)));
        routes.push(((*image).to_string(), reply.clone()));
    }
    let site = Site::start(routes).await;
    let store = Store::new();
    let links = previewer(&store, local());
    for (path, kind) in [
        ("/svg", None),
        ("/full", Some("image/png")),
        ("/over", None),
        ("/over-unsized", None),
        ("/webp", Some("image/webp")),
        ("/jpeg", Some("image/jpeg")),
        ("/ico", Some("image/x-icon")),
        ("/missing", None),
    ] {
        let card = card(&links, &site.url(path)).await;
        assert_eq!(
            card.image.as_ref().map(|image| image.media_type),
            kind,
            "{path}：图抓不到的那一格是空的，卡片照样成立"
        );
    }
}

#[tokio::test]
async fn a_missing_rules_file_is_not_ready_but_bad_addresses_are_still_answered() {
    let store = Store::new();
    let nowhere = std::env::temp_dir().join("miyu-net-no-resources-here");
    let links = miyu_net::LinkPreview::new(&nowhere, store.blobs.clone()).testing(local());
    assert_eq!(
        links.preview("not a url").await,
        Ok(Preview::Miss(Why::NotAUrl))
    );
    assert_eq!(
        links.preview("http://site.test/").await,
        Err(miyu_net::NotReady)
    );
}
