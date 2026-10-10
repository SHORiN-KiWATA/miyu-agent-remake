//! 软件后台页的地址怎么拆、包编号的写法、核心拒的换成什么状态、类型怎么查（施工 F-6 下）。

use std::collections::BTreeMap;

use hyper::StatusCode;

use super::{content_type, split, status_of, valid_package};
use crate::settings::Settings;

#[test]
fn paths_split_into_ticket_package_and_file() {
    let three = |t: &str, p: &str, f: &str| Some((t.to_string(), p.to_string(), f.to_string()));
    assert_eq!(
        split("/p/abc/onebot/"),
        three("abc", "onebot", ""),
        "空的照核心认成 index.html"
    );
    assert_eq!(split("/p/abc/onebot"), three("abc", "onebot", ""));
    assert_eq!(
        split("/p/abc/onebot/app.js"),
        three("abc", "onebot", "app.js")
    );
    assert_eq!(
        split("/p/abc/onebot/a/b.css"),
        three("abc", "onebot", "a/b.css")
    );
    assert_eq!(
        split("/p/abc/onebot/sub/"),
        three("abc", "onebot", "sub/index.html")
    );
    assert_eq!(
        split("/p/abc/onebot/%E4%B8%AD.css"),
        three("abc", "onebot", "中.css")
    );
    assert_eq!(
        split("/p/abc/onebot/%2e%2e/x"),
        three("abc", "onebot", "../x"),
        "解开以后照原样交给核心，`..` 由核心拒"
    );
    assert_eq!(split("/p//onebot/"), None, "没有票据");
    assert_eq!(split("/p/abc/"), None, "没有包");
    assert_eq!(split("/p/abc/bad%20name/"), None, "包的编号写法不对");
    assert_eq!(split("/p/abc/onebot/%zz"), None, "`%` 后面不是十六进制");
    assert_eq!(split("/p/abc/onebot/%00"), None, "NUL");
    assert_eq!(split("/p/abc/onebot/%ff"), None, "不是 UTF-8");
    assert_eq!(split("/media/abc"), None);
}

#[test]
fn package_names_stay_plain() {
    for good in ["onebot", "backstage-demo", "a_b.c", "x", &"a".repeat(64)] {
        assert!(valid_package(good), "{good}");
    }
    for bad in ["", "a b", "a/b", "中文", "a%20", &"a".repeat(65)] {
        assert!(!valid_package(bad), "{bad}");
    }
}

#[test]
fn refusals_become_statuses() {
    assert_eq!(status_of("not_found"), StatusCode::NOT_FOUND);
    assert_eq!(status_of("no_page"), StatusCode::NOT_FOUND);
    assert_eq!(status_of("unknown_package"), StatusCode::NOT_FOUND);
    assert_eq!(status_of("bad_params"), StatusCode::BAD_REQUEST);
    assert_eq!(status_of("internal_error"), StatusCode::BAD_GATEWAY);
}

#[test]
fn types_follow_the_extension() {
    let settings = Settings {
        port: 0,
        idle_seconds: 1,
        csp: String::new(),
        backstage_csp: String::new(),
        types: BTreeMap::from([
            ("html".to_string(), "text/html; charset=utf-8".to_string()),
            (
                "js".to_string(),
                "text/javascript; charset=utf-8".to_string(),
            ),
        ]),
        ticket_idle_seconds: 1,
        most_tickets: 1,
    };
    assert_eq!(
        content_type(&settings, ""),
        "text/html; charset=utf-8",
        "空的是 index.html"
    );
    assert_eq!(
        content_type(&settings, "sub/index.html"),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        content_type(&settings, "APP.JS"),
        "text/javascript; charset=utf-8",
        "扩展名不分大小写"
    );
    assert_eq!(
        content_type(&settings, "data.bin"),
        "application/octet-stream"
    );
    assert_eq!(content_type(&settings, "noext"), "application/octet-stream");
}
