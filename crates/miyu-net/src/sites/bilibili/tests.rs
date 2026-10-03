use serde_json::json;

use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect(text)
}

#[test]
fn video_pages_are_bv_or_av() {
    for yes in [
        "https://www.bilibili.com/video/BV1Rxam6kEtU",
        "https://www.bilibili.com/video/BV1Rxam6kEtU/?p=2&share_source=copy",
        "https://m.bilibili.com/video/av170001",
        "https://www.bilibili.com/video/av170001/",
    ] {
        assert!(is_video(&url(yes)), "{yes}");
    }
    // 长短不对、带了怪字、不是视频页的都不认
    for no in [
        "https://www.bilibili.com/video/BV1Rxam6kEt",
        "https://www.bilibili.com/video/BV1Rxam6kEtUU",
        "https://www.bilibili.com/video/BV1Rxam6-EtU",
        "https://www.bilibili.com/video/av",
        "https://www.bilibili.com/video/av12x",
        "https://www.bilibili.com/video/",
        "https://www.bilibili.com/bangumi/play/ep1",
        "https://space.bilibili.com/123",
        "https://www.bilibili.com/x/video/BV1Rxam6kEtU",
    ] {
        assert!(!is_video(&url(no)), "{no}");
    }
}

/// 一页 B 站视频页的样子：`<head>` 里有 og，视频数据在后面的脚本里（2026-10-03 项目主人实测的那一页，删短了）。
fn page(state: &str) -> String {
    format!(
        r#"<html><head><title>《明日方舟》宣传PV_游戏热门视频</title>
        <meta data-vue-meta="true" name="author" content="页面上的 UP 主">
        <meta data-vue-meta="true" property="og:title" content="《明日方舟》宣传PV_游戏热门视频"></head><body>
        <script>window.__INITIAL_STATE__={state};(function(){{var s;}}());</script></body></html>"#
    )
}

#[test]
fn the_video_data_in_the_page_script_is_read() {
    let state = json!({"aid": 1, "videoData": {"bvid": "BV1Rxam6kEtU",
        "title": "《明日方舟》宣传PV", "desc": "简介", "pic": "http://i2.hdslb.com/bfs/archive/x.jpg",
        "duration": 408, "owner": {"mid": 161775300, "name": "明日方舟"},
        "pages": [{"duration": 204}]}});
    let text = page(&state.to_string());
    assert_eq!(
        video_data(&text).as_ref().and_then(read),
        Some(Video {
            title: "《明日方舟》宣传PV".to_string(),
            description: "简介".to_string(),
            picture: Some("http://i2.hdslb.com/bfs/archive/x.jpg".to_string()),
            author: "明日方舟".to_string(),
            duration: Some(408),
        })
    );
    // 少了的几格是空的；时长是 0 的、不是数的不写
    let bare = json!({"title": "t", "duration": 0});
    assert_eq!(
        read(&bare),
        Some(Video {
            title: "t".to_string(),
            description: String::new(),
            picture: None,
            author: String::new(),
            duration: None,
        })
    );
    for broken in [json!({"title": "  "}), json!({}), json!("not an object")] {
        assert_eq!(read(&broken), None, "{broken}");
    }
}

#[test]
fn without_the_script_data_the_page_meta_is_used() {
    let rules = crate::rules::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
    )
    .unwrap()
    .sites
    .bilibili;
    for text in [
        page("{\"videoData\":"),
        page("not json"),
        page("{\"noVideoData\":1}"),
        "<head><meta name=\"author\" content=\"页面上的 UP 主\"></head>".to_string(),
    ] {
        let mut draft = Draft::default();
        draft.found.title = "页面的标题".to_string();
        fill(&text, &rules, &mut draft);
        assert_eq!(draft.kind, Kind::Video);
        assert_eq!(draft.found.title, "页面的标题", "{text}");
        assert_eq!(draft.author, "页面上的 UP 主");
        assert_eq!(draft.duration, None);
        assert_eq!(draft.site_fallback, "哔哩哔哩");
    }
}
