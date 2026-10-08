//! 私聊里的文字怎么读出来（`onebot.md` 第一条「怎么走」第 6 条）：消息是段的数组（NapCat 配成数组格式）时取 `text` 段；是
//! 字符串（CQ 码）时去掉 `[CQ:…]`，再把转义换回来。图片、表情、回复、@ 这些别的段这一步不管。

use serde_json::Value;

/// CQ 码的开头。
const CQ: &str = "[CQ:";

/// CQ 字符串里的转义和它们本来的字。`&amp;` 排在最后：先换它的话，`&amp;#91;`（本来就是 `&#91;` 这几个字）会被换成 `[`。
const ESCAPES: [(&str, &str); 4] = [
    ("&#91;", "["),
    ("&#93;", "]"),
    ("&#44;", ","),
    ("&amp;", "&"),
];

/// 读出消息 `message` 里的文字：段的数组取 `text` 段的 `data.text` 依次接起来；字符串去掉 CQ 码、换回转义；别的是空字。
/// 照原样，不去首尾空白（送不送由调的一方看）。
pub fn text_of(message: &Value) -> String {
    match message {
        Value::Array(segments) => segments
            .iter()
            .filter(|segment| segment["type"] == "text")
            .filter_map(|segment| segment["data"]["text"].as_str())
            .collect(),
        Value::String(text) => unescape(&strip_codes(text)),
        _ => String::new(),
    }
}

/// 去掉 `[CQ:…]`：从 `[CQ:` 到下一个 `]`。没关上的照原样留着。
fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(CQ) {
        let Some(length) = rest[start..].find(']') else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + length + 1..];
    }
    out.push_str(rest);
    out
}

/// 换回转义，照 [`ESCAPES`] 的先后。
fn unescape(text: &str) -> String {
    ESCAPES
        .iter()
        .fold(text.to_string(), |text, (escaped, plain)| {
            text.replace(escaped, plain)
        })
}
