//! 取带的东西的动作和回应（施工 O-33）：图 `get_image`、视频文件 `get_file`；东西在哪照 url、本机路径、base64 的先后，url 只认
//! `http`、`https`，`file://` 当路径，相对路径不认，同一个路径只算一次。

use std::path::PathBuf;

use serde_json::json;

use super::*;

/// 这台机器上的一个绝对路径（三个平台都是绝对的）。
fn absolute(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn the_actions_follow_how_to_fetch() {
    assert_eq!(get_msg("8815"), ("get_msg", json!({"message_id": "8815"})));
    assert_eq!(
        get_media(Fetch::Image, "A.jpg"),
        ("get_image", json!({"file": "A.jpg"}))
    );
    assert_eq!(
        get_media(Fetch::File, "u-1"),
        ("get_file", json!({"file": "u-1"}))
    );
}

#[test]
fn sources_come_as_url_then_path_then_base64() {
    let path = absolute("napcat-a.jpg");
    let text = path.to_string_lossy().into_owned();
    let data = json!({"url": " https://multimedia.nt.qq.com.cn/a ", "file": text, "base64": "base64://AAEC"});
    assert_eq!(
        sources(&data),
        [
            Source::Url("https://multimedia.nt.qq.com.cn/a".to_string()),
            Source::Path(path.clone()),
            Source::Base64("AAEC".to_string()),
        ]
    );
    // 文件的 url 是本机路径（`GetFile.ts`）：和 `file` 一样的只算一次。
    let same = json!({"url": text, "file": text, "base64": ""});
    assert_eq!(sources(&same), [Source::Path(path.clone())]);
    // `file://` 当路径；HTTP 写成大写也认。
    let file = format!("file://{text}");
    assert_eq!(
        sources(&json!({"url": "HTTP://x/y", "file": file})),
        [Source::Url("HTTP://x/y".to_string()), Source::Path(path)]
    );
}

#[test]
fn other_schemes_relative_paths_and_empties_are_not_sources() {
    for data in [
        json!({"url": "ftp://x/y"}),
        json!({"url": "javascript:alert(1)"}),
        json!({"file": "a.jpg"}),
        json!({"file": "../a.jpg", "url": ""}),
        json!({"base64": "  "}),
        json!({"base64": "base64://"}),
        json!({}),
        json!(null),
    ] {
        assert_eq!(sources(&data), [], "{data}");
    }
    assert_eq!(
        sources(&json!({"base64": "QUJD"})),
        [Source::Base64("QUJD".to_string())],
        "没有前缀的照原样"
    );
}

#[test]
fn a_file_name_is_read_when_there() {
    assert_eq!(
        file_name(&json!({"file_name": " 报告.pdf "})),
        Some("报告.pdf")
    );
    assert_eq!(file_name(&json!({"file_name": "  "})), None);
    assert_eq!(file_name(&json!({})), None);
}
