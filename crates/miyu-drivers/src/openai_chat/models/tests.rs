//! 列模型的回应怎么读（施工 8-7）。

use super::parse_models;
use crate::Listed;

fn listed(id: &str, window: Option<u64>) -> Listed {
    Listed {
        id: id.to_string(),
        window,
    }
}

#[test]
fn names_and_stated_windows_are_read() {
    let body = br#"{"object":"list","data":[
        {"id":"deepseek-flash","object":"model","owned_by":"deepseek"},
        {"id":"qwen3","context_length":131072},
        {"id":"llama","context_window":8192,"context_length":4096},
        {"id":"old","max_context_length":32000},
        {"id":"odd","context_length":"big"},
        {"id":"zero","context_length":0},
        {"id":""},
        {"object":"model"},
        {"id":7}
    ]}"#;
    assert_eq!(
        parse_models(body).expect("读得进"),
        [
            listed("deepseek-flash", None),
            listed("qwen3", Some(131_072)),
            listed("llama", Some(8192)),
            listed("old", Some(32_000)),
            listed("odd", None),
            listed("zero", None),
        ]
    );
}

#[test]
fn a_broken_list_says_so() {
    for body in [&b"not json"[..], br#"{"models":[]}"#, br#"{"data":{}}"#] {
        let error = parse_models(body).expect_err("读不进");
        assert!(error.starts_with("model list not readable: "), "{error}");
    }
}
