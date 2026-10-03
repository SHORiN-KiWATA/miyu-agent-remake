//! 列模型的回应怎么读（施工 8-12）：`{"data":[{"id":…,"max_input_tokens":…}],"has_more":…}`。

use super::{MODELS_PATH, parse_models};
use crate::Listed;

fn listed(id: &str, window: Option<u64>) -> Listed {
    Listed {
        id: id.to_string(),
        window,
    }
}

#[test]
fn names_and_windows_are_read() {
    assert_eq!(MODELS_PATH, "/models?limit=1000");
    let body = br#"{"data":[
        {"type":"model","id":"claude-opus-5","display_name":"Claude Opus 5","created_at":"2026-05-01T00:00:00Z","max_input_tokens":1000000,"max_tokens":128000},
        {"type":"model","id":"claude-haiku-4-5","max_input_tokens":200000},
        {"id":"claude-old"},
        {"id":"odd","max_input_tokens":"big"},
        {"id":"zero","max_input_tokens":0},
        {"id":"context-window-is-not-ours","context_window":8192},
        {"id":""},
        {"type":"model"},
        {"id":7}
    ],"has_more":true,"first_id":"claude-opus-5","last_id":"zero"}"#;
    assert_eq!(
        parse_models(body).expect("读得进"),
        [
            listed("claude-opus-5", Some(1_000_000)),
            listed("claude-haiku-4-5", Some(200_000)),
            listed("claude-old", None),
            listed("odd", None),
            listed("zero", None),
            listed("context-window-is-not-ours", None),
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
