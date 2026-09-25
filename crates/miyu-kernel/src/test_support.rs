//! 测试共用的两个检查。

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

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
