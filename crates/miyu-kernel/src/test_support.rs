//! 测试共用的检查。

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::event::{Body, Event};

/// 从 JSON 读进来，再写出去，要和原文一字不差。
pub(crate) fn round_trip<T: Serialize + DeserializeOwned>(json: &str) {
    let value: T = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), json);
}

/// 从 JSON 读，要被拦下，报错里说清错在哪。
pub(crate) fn rejected<T: DeserializeOwned + fmt::Debug>(json: &str, why: &str) {
    let err = serde_json::from_str::<T>(json).unwrap_err().to_string();
    assert!(err.contains(why), "{json} 的报错里没有「{why}」：{err}");
}

/// 用给定的种类和 `body` 拼一行事件，外壳用一套固定的写法。
pub(crate) fn event_line(kind: &str, body: &str) -> String {
    format!(
        r#"{{"seq":7,"at":"2026-09-25T07:00:00.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    )
}

/// 读一行事件：读写要一字不差，而且认得出种类。返回读好的 `body`。
pub(crate) fn read_body(kind: &str, body: &str) -> Body {
    let line = event_line(kind, body);
    let event = Event::from_line(&line).unwrap();
    assert_eq!(event.to_line(), line);
    assert!(
        !matches!(event.body, Body::Unknown { .. }),
        "{kind} 应该认得出种类"
    );
    event.body
}
