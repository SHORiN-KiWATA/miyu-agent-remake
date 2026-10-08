//! 按站取（`net.md`「怎么走」第 12、13 条，W-7 再补），全在本机的假服务器上：真的主机名（`www.bilibili.com` 这些）
//! 经测试的口子解析到回环，认站照主机名。假服务器只照路径回，不看 `Host`：一个站要的几样（页面、接口、图）互相要
//! 写对方的端口时，分开起几台。

use crate::support::{PNG, Reply, Site, Store, card, local_with, miss, resources};
use miyu_net::{Kind, LinkPreview, Why};
use serde_json::{Value, json};

/// 测试里当真的那几个主机名。
const NAMES: &[&str] = &[
    "www.bilibili.com",
    "b23.tv",
    "i2.hdslb.com",
    "www.youtube.com",
    "youtu.be",
    "en.wikipedia.org",
    "zh.wikipedia.org",
    "wiki.example.org",
];

const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF";

/// 照出厂的规矩、打开测试的口子（`NAMES` 都解析到回环）的一份。
fn previewer(store: &Store) -> LinkPreview {
    LinkPreview::new(&resources(), store.blobs.clone()).testing(local_with(NAMES))
}

fn json_reply(body: &Value) -> Reply {
    Reply::bytes("application/json", body.to_string().as_bytes())
}

/// 照路径找收到过的请求。
fn find(site: &Site, prefix: &str) -> Vec<crate::support::Seen> {
    site.seen()
        .into_iter()
        .filter(|seen| seen.target.starts_with(prefix))
        .collect()
}

/// 一页 B 站视频页的样子（2026-10-03 项目主人实测的那一页，删短了）：`<head>` 里有 og 和 `<meta name="author">`，
/// 视频数据在后面的脚本里；`state` 是空的就没有那段脚本。
fn bilibili_page(state: &str) -> String {
    let script = if state.is_empty() {
        String::new()
    } else {
        format!("<script>window.__INITIAL_STATE__={state};(function(){{var s;}}());</script>")
    };
    format!(
        r#"<html><head><title>页面上的标题_游戏热门视频</title>
        <meta data-vue-meta="true" name="author" content="页面上的 UP 主">
        <meta data-vue-meta="true" property="og:title" content="页面上的标题_游戏热门视频">
        <meta data-vue-meta="true" property="og:description" content="页面上的简介">
        <meta data-vue-meta="true" property="og:site_name" content="哔哩哔哩"></head>
        <body>{}{script}</body></html>"#,
        "x".repeat(64 * 1024)
    )
}

#[tokio::test]
async fn a_bilibili_video_is_read_from_the_data_in_its_page() {
    let media = Site::start(vec![(
        "/cover.jpg".to_string(),
        Reply::bytes("image/jpeg", JPEG),
    )])
    .await;
    let state = json!({"videoData": {"title": "脚本里的标题", "desc": "一段  简介",
        "pic": format!("http://i2.hdslb.com:{}/cover.jpg", media.port),
        "duration": 408, "owner": {"mid": 1, "name": "某个 UP 主"}}});
    let bili = Site::start(vec![
        (
            "/video/BV1Rxam6kEtU?share=1".to_string(),
            Reply::html(&bilibili_page(&state.to_string())),
        ),
        // 没有那段脚本的：照页面的 og、<meta name="author">
        (
            "/video/av170001".to_string(),
            Reply::html(&bilibili_page("")),
        ),
    ])
    .await;
    let port = bili.port;
    let short = Site::start(vec![(
        "/abc".to_string(),
        Reply::redirect(&format!(
            "http://www.bilibili.com:{port}/video/BV1Rxam6kEtU?share=1"
        )),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store);

    let bv = format!("http://www.bilibili.com:{port}/video/BV1Rxam6kEtU?share=1");
    let found = card(&links, &bv).await;
    assert_eq!(found.title, "脚本里的标题");
    assert_eq!(found.description, "一段 简介", "脚本里的字也收拢空白");
    assert_eq!(found.author.as_deref(), Some("某个 UP 主"));
    assert_eq!(found.duration, Some(408));
    assert_eq!(found.kind, Kind::Video);
    assert_eq!(found.site, "哔哩哔哩");
    assert_eq!(found.url, bv);
    let image = found.image.expect("有封面");
    assert_eq!(image.media_type, "image/jpeg");
    assert_eq!(store.blobs.get(&image.blob).unwrap(), JPEG);

    let av = card(
        &links,
        &format!("http://www.bilibili.com:{port}/video/av170001"),
    )
    .await;
    assert_eq!(av.title, "页面上的标题_游戏热门视频");
    assert_eq!(av.description, "页面上的简介");
    assert_eq!(av.author.as_deref(), Some("页面上的 UP 主"));
    assert_eq!(av.duration, None);
    assert_eq!(av.kind, Kind::Video, "没有脚本数据也是视频");

    // 短链：跟完跳转再认，卡片写落到的那一页
    let landed = card(&links, &format!("http://b23.tv:{}/abc", short.port)).await;
    assert_eq!(landed.title, "脚本里的标题");
    assert_eq!(landed.url, bv);
    assert!(
        bili.seen()
            .iter()
            .all(|seen| !seen.target.contains("web-interface")),
        "不调接口"
    );
}

#[tokio::test]
async fn a_gone_bilibili_video_has_no_card() {
    // 删了的视频：页面标题是「视频去哪了呢？」，og 的简介是没填的模板（2026-10-02、10-03 实测）
    let gone = r#"<html><head><title>视频去哪了呢？_哔哩哔哩_bilibili</title>
        <meta property="og:description" content="视频去哪了呢？{$0}的视频"></head><body></body></html>"#;
    let bili = Site::start(vec![("/video/BV1GJ411x7h7".to_string(), Reply::html(gone))]).await;
    let store = Store::new();
    let links = previewer(&store);
    let url = format!("http://www.bilibili.com:{}/video/BV1GJ411x7h7", bili.port);
    assert_eq!(miss(&links, &url).await, Why::NoPreview);
    // 同样的标题不在 B 站的视频页上，不算
    let other = Site::start(vec![("/p".to_string(), Reply::html(gone))]).await;
    let plain = card(&links, &other.url("/p")).await;
    assert_eq!(plain.title, "视频去哪了呢？_哔哩哔哩_bilibili");
    assert_eq!(plain.description, "", "没填的模板那一格当没有");
}

#[tokio::test]
async fn a_site_api_is_guarded_like_everything_else() {
    // MediaWiki 站的 EditURI 指到内网：接口过不了闸，照页面合成卡片；接口那台一次都没收到
    let api = Site::start(vec![(
        "/api.php*".to_string(),
        json_reply(&json!({"query": {"pages": [{"title": "不该到这里"}]}})),
    )])
    .await;
    let edit = format!(
        r#"<meta name="generator" content="MediaWiki 1.43.1">
        <link rel="EditURI" href="http://inner.test:{}/api.php?action=rsd">"#,
        api.port
    );
    let pages = Site::start(vec![(
        "/title/Pacman".to_string(),
        Reply::html(&wiki_page("pacman - ArchWiki", "Pacman", &edit)),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store);
    let found = card(
        &links,
        &format!("http://wiki.example.org:{}/title/Pacman", pages.port),
    )
    .await;
    assert_eq!(found.title, "pacman", "照页面的标题，尾巴去掉");
    assert_eq!(found.kind, Kind::Article);
    assert!(api.seen().is_empty(), "{:?}", api.seen());
}

/// 一页 YouTube 的样子：`<head>` 里有 `<title>`，`og:*`、时长、频道名在 `</head>` 后面，中间隔着一大截。
fn youtube_page(duration: Option<&str>) -> String {
    let filler = "x".repeat(300 * 1024);
    let video = duration.map_or(String::new(), |iso| {
        format!(
            r#"<meta itemprop="duration" content="{iso}"><span itemprop="author"><link itemprop="url" href="/@r"><link itemprop="name" content="Rick"></span>"#
        )
    });
    format!(
        r#"<html><head><title>Plain - YouTube</title></head><body><script>{filler}</script>
        <meta property="og:title" content="Never Gonna"><meta property="og:image" content="/vi.jpg">
        <meta property="og:description" content="The official video">{video}"#
    )
}

#[tokio::test]
async fn youtube_is_read_past_the_head_for_the_duration_and_the_channel() {
    let watch = youtube_page(Some("PT3M33S"));
    let channel = youtube_page(None);
    let site = Site::start(vec![
        // 三样都见到了就停，不等对方说完
        ("/watch?v=x".to_string(), Reply::html(&watch).stalled()),
        ("/@channel".to_string(), Reply::html(&channel)),
        ("/vi.jpg".to_string(), Reply::bytes("image/jpeg", JPEG)),
    ])
    .await;
    let short = Site::start(vec![(
        "/x".to_string(),
        Reply::redirect(&format!("http://www.youtube.com:{}/watch?v=x", site.port)),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store);

    let video = card(
        &links,
        &format!("http://www.youtube.com:{}/watch?v=x", site.port),
    )
    .await;
    assert_eq!(
        video.title, "Never Gonna",
        "og 先，不照 <head> 里的 <title>"
    );
    assert_eq!(video.description, "The official video");
    assert_eq!(video.duration, Some(213));
    assert_eq!(video.author.as_deref(), Some("Rick"));
    assert_eq!(video.kind, Kind::Video);
    assert!(video.image.is_some());

    let shortened = card(&links, &format!("http://youtu.be:{}/x", short.port)).await;
    assert_eq!(shortened.duration, Some(213), "youtu.be 跟到 youtube.com");

    let page = card(
        &links,
        &format!("http://www.youtube.com:{}/@channel", site.port),
    )
    .await;
    assert_eq!(page.title, "Never Gonna");
    assert_eq!(page.kind, Kind::Page, "读不到时长的不是视频");
    assert_eq!(page.duration, None);
    assert_eq!(page.author, None);
}

/// 一页 MediaWiki 的样子：`RLCONF` 里有页面名，标题带着「 - 站名」的尾巴。
fn wiki_page(title: &str, name: &str, extra: &str) -> String {
    format!(
        r#"<html><head><title>{title}</title>{extra}
        <script>RLCONF={{"wgBreakFrames":false,"wgPageName":"{name}","wgTitle":"x"}};</script>
        </head><body><p>正文</p></body></html>"#
    )
}

#[tokio::test]
async fn a_listed_wiki_is_filled_from_its_api() {
    let thumb = Site::start(vec![(
        "/thumb.png".to_string(),
        Reply::bytes("image/png", PNG),
    )])
    .await;
    let summary = json!({"batchcomplete": true, "query": {
        "pages": [{"title": "Rust (programming language)", "extract": "Rust is a\n language.",
            "thumbnail": {"source": format!("http://en.wikipedia.org:{}/thumb.png", thumb.port)}}],
        "general": {"sitename": "Wikipedia"}}});
    let wiki = Site::start(vec![
        (
            "/wiki/Rust_(programming_language)".to_string(),
            Reply::html(&wiki_page(
                "Rust (programming language) - Wikipedia",
                "Rust_(programming_language)",
                "",
            )),
        ),
        ("/w/api.php?action=query*".to_string(), json_reply(&summary)),
        (
            "/wiki/Broken".to_string(),
            Reply::html(&wiki_page("Rust - 维基百科，自由的百科全书", "Broken", "")),
        ),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store);

    let article = card(
        &links,
        &format!(
            "http://en.wikipedia.org:{}/wiki/Rust_(programming_language)",
            wiki.port
        ),
    )
    .await;
    assert_eq!(article.title, "Rust (programming language)");
    assert_eq!(article.description, "Rust is a language.");
    assert_eq!(article.site, "Wikipedia");
    assert_eq!(article.kind, Kind::Article);
    assert_eq!(article.image.expect("有缩略图").media_type, "image/png");
    let query = &find(&wiki, "/w/api.php")[0].target;
    for part in [
        "titles=Rust_%28programming_language%29",
        "prop=extracts%7Cpageimages",
        "pithumbsize=640",
        "meta=siteinfo",
    ] {
        assert!(query.contains(part), "{part} 不在 {query} 里");
    }

    // 接口没成（这一页的请求回 404 也一样走不通）：照页面，标题去掉尾巴，尾巴当站名
    let wiki_broken = Site::start(vec![(
        "/wiki/Broken".to_string(),
        Reply::html(&wiki_page("Rust - 维基百科，自由的百科全书", "Broken", "")),
    )])
    .await;
    let fallback = card(
        &links,
        &format!("http://zh.wikipedia.org:{}/wiki/Broken", wiki_broken.port),
    )
    .await;
    assert_eq!(fallback.title, "Rust");
    assert_eq!(fallback.site, "维基百科，自由的百科全书");
    assert_eq!(fallback.kind, Kind::Article);
}

#[tokio::test]
async fn an_unlisted_wiki_is_found_by_its_generator_and_falls_back_to_the_first_paragraph() {
    let edit = |port: u16| {
        format!(
            r#"<meta name="generator" content="MediaWiki 1.43.1">
            <link rel="EditURI" type="application/rsd+xml" href="//wiki.example.org:{port}/api.php?action=rsd">"#
        )
    };
    let no_extracts = json!({"warnings": {"main": {"warnings": "Unrecognized value for parameter \"prop\": extracts."}},
        "query": {"pages": [{"title": "Pacman"}], "general": {"sitename": "ArchWiki"}}});
    let parsed = json!({"parse": {"title": "Pacman", "text":
        "<div class=\"mw-parser-output\"><p class=\"mw-empty-elt\">\n</p><p><b>pacman</b> is a package manager<sup class=\"reference\"><a>[1]</a></sup>.</p></div>"}});
    let wiki = Site::start(vec![
        (
            "/api.php?action=query*".to_string(),
            json_reply(&no_extracts),
        ),
        ("/api.php?action=parse*".to_string(), json_reply(&parsed)),
    ])
    .await;
    let pages = Site::start(vec![(
        "/title/Pacman".to_string(),
        Reply::html(&wiki_page("pacman - ArchWiki", "Pacman", &edit(wiki.port))),
    )])
    .await;
    let store = Store::new();
    let links = previewer(&store);
    let article = card(
        &links,
        &format!("http://wiki.example.org:{}/title/Pacman", pages.port),
    )
    .await;
    assert_eq!(article.title, "Pacman", "照接口的标题");
    assert_eq!(article.description, "pacman is a package manager.");
    assert_eq!(article.site, "ArchWiki");
    assert_eq!(article.kind, Kind::Article);
    let parse = &find(&wiki, "/api.php?action=parse")[0].target;
    assert!(
        parse.contains("page=Pacman") && parse.contains("section=0"),
        "{parse}"
    );
}

#[tokio::test]
async fn challenge_pages_have_no_card() {
    let site = Site::start(vec![
        (
            "/moment".to_string(),
            Reply::html(&crate::support::page("Just a moment...", "")),
        ),
        (
            "/spaced".to_string(),
            Reply::html(&crate::support::page("  安全检查 ", "")),
        ),
        (
            "/header".to_string(),
            Reply::html(&crate::support::page("Fine", ""))
                .with_header("cf-mitigated", " Challenge "),
        ),
        (
            "/forbidden".to_string(),
            Reply::html(&crate::support::page("Fine", "")).with_status(403),
        ),
        (
            "/busy".to_string(),
            Reply::html(&crate::support::page("Fine", "")).with_status(503),
        ),
        (
            "/forbidden-text".to_string(),
            Reply::bytes("text/plain", b"no").with_status(403),
        ),
        (
            "/teapot".to_string(),
            Reply::html(&crate::support::page("Fine", "")).with_status(418),
        ),
        (
            "/plain".to_string(),
            Reply::html(&crate::support::page("Just a moment", "")),
        ),
    ])
    .await;
    let store = Store::new();
    let links = previewer(&store);
    for (path, why) in [
        ("/moment", Why::NoPreview),
        ("/spaced", Why::NoPreview),
        ("/header", Why::NoPreview),
        ("/forbidden", Why::NoPreview),
        ("/busy", Why::NoPreview),
        ("/forbidden-text", Why::Unreachable),
        ("/teapot", Why::Unreachable),
    ] {
        assert_eq!(miss(&links, &site.url(path)).await, why, "{path}");
    }
    // 不是表里那几个的照常
    let plain = card(&links, &site.url("/plain")).await;
    assert_eq!(plain.title, "Just a moment");
    assert_eq!(plain.kind, Kind::Page);
    assert_eq!(plain.duration, None);
    assert_eq!(plain.author, None);
}
