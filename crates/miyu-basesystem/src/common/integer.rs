//! 写成字符串的整数也认（施工 4-9 再补二）：参数格式里没声明的别名（`grep` 的 `-A`、`-B`、`-C`），内核照声明修正
//! 不到，模型写成 `"3"` 的由工具自己认。不是整数的，照 serde 的写法报参数不对。

use serde::de::{Error, Unexpected};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// 读一个可以不写的整数：整数、写成字符串的整数都认；没写、写了 `null` 的是空的。
pub(crate) fn integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    let wanted = &"an integer";
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| Error::invalid_value(Unexpected::Other("number"), wanted)),
        Some(Value::String(text)) => text
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(&text), wanted)),
        Some(Value::Bool(value)) => Err(Error::invalid_type(Unexpected::Bool(value), wanted)),
        Some(Value::Array(_)) => Err(Error::invalid_type(Unexpected::Seq, wanted)),
        Some(Value::Object(_)) => Err(Error::invalid_type(Unexpected::Map, wanted)),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    /// 带一格可以不写的整数。
    #[derive(Deserialize, Debug)]
    struct Holder {
        #[serde(default, deserialize_with = "super::integer")]
        n: Option<i64>,
    }

    fn read(json: &str) -> Result<Option<i64>, String> {
        serde_json::from_str::<Holder>(json)
            .map(|holder| holder.n)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn numbers_and_numbers_written_as_text_are_both_taken() {
        assert_eq!(read(r#"{"n": 3}"#), Ok(Some(3)));
        assert_eq!(read(r#"{"n": " 3 "}"#), Ok(Some(3)));
        assert_eq!(read(r#"{"n": -2}"#), Ok(Some(-2)));
        assert_eq!(read("{}"), Ok(None));
        assert_eq!(read(r#"{"n": null}"#), Ok(None));
    }

    #[test]
    fn anything_else_is_refused() {
        for json in [
            r#"{"n": "x"}"#,
            r#"{"n": 1.5}"#,
            r#"{"n": true}"#,
            r#"{"n": [1]}"#,
        ] {
            let error = read(json).expect_err(json);
            assert!(error.contains("expected an integer"), "{json}: {error}");
        }
    }
}
