//! 内容块的测试：图纸上的五种读写一字不差；驱动私有数据、工具名和参数、不认识的块，
//! 都一字不差；坏的报错。

use super::*;
use crate::test_support::{rejected, round_trip};

const HASH: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn every_block_from_the_drawing_round_trips() {
    for json in [
        r#"{"type":"text","text":"我先看一下目录。"}"#.to_string(),
        r#"{"type":"reasoning","text":"先看目录","private":{"driver":"anthropic","data":{"signature":"sig"}}}"#.to_string(),
        r#"{"type":"reasoning","text":"没有私有数据"}"#.to_string(),
        format!(r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","width":800,"height":600}}"#),
        format!(r#"{{"type":"image","blob":"{HASH}","name":"晚霞.png","media_type":"image/png","width":800,"height":600}}"#),
        format!(r#"{{"type":"file","blob":"{HASH}","name":"报告.pdf","media_type":"application/pdf"}}"#),
        r#"{"type":"tool_call","call_id":"call_44_1","name":"read","args":"{\"path\":\"src\"}"}"#.to_string(),
    ] {
        round_trip::<Block>(&json);
        let block: Block = serde_json::from_str(&json).unwrap();
        assert!(!matches!(block, Block::Unknown(_)), "{json} 应该认得出种类");
    }
}

/// 驱动私有数据里的空格、数字的写法（1.50）都原样留着。
#[test]
fn private_data_is_kept_byte_for_byte() {
    round_trip::<Block>(
        r#"{"type":"reasoning","text":"…","private":{"driver":"anthropic","data":{ "signature" : "EqQBCkYI", "n": 1.50, "list": [ ] }}}"#,
    );
}

#[test]
fn tool_call_keeps_name_and_args_as_the_model_wrote_them() {
    round_trip::<Block>(
        r#"{"type":"tool_call","call_id":"call_44_2","name":"不存在的 工具!","args":"{ \"path\" : \"src\" , }"}"#,
    );
    round_trip::<Block>(
        r#"{"type":"tool_call","call_id":"call_44_3","name":"read","args":"{}","private":{"driver":"openai-chat","data":"call_abc123"}}"#,
    );
}

/// 图片块的名字（施工 3-9 四补）：人附的带着，`read` 读出来的、以前的日志里的没有，读回来是没有，写出去也没有这一格。
#[test]
fn an_image_carries_its_name_only_when_it_has_one() {
    let named = format!(
        r#"{{"type":"image","blob":"{HASH}","name":"晚霞.png","media_type":"image/png","width":800,"height":600}}"#
    );
    let Block::Image(image) = serde_json::from_str(&named).unwrap() else {
        panic!("是图片块");
    };
    assert_eq!(image.name.as_ref().map(FileName::as_str), Some("晚霞.png"));
    let old = format!(
        r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","width":800,"height":600}}"#
    );
    let Block::Image(image) = serde_json::from_str(&old).unwrap() else {
        panic!("是图片块");
    };
    assert_eq!(image.name, None);
    assert!(
        !serde_json::to_string(&Block::Image(image))
            .unwrap()
            .contains("name")
    );
}

#[test]
fn unknown_block_is_kept_byte_for_byte() {
    let json = format!(r#"{{"type":"audio", "blob":"{HASH}","seconds":3.20}}"#);
    let block: Block = serde_json::from_str(&json).unwrap();
    assert!(matches!(block, Block::Unknown(_)), "{block:?}");
    assert_eq!(serde_json::to_string(&block).unwrap(), json);
}

#[test]
fn broken_blocks_are_errors() {
    rejected::<Block>(
        &format!(r#"{{"type":"image","blob":"{HASH}","media_type":"image/png","height":600}}"#),
        "width",
    );
    rejected::<Block>(r#"{"text":"没有 type"}"#, "missing field `type`");
    rejected::<Block>(
        &format!(
            r#"{{"type":"image","blob":"{HASH}","name":"shots/a.png","media_type":"image/png","width":1,"height":1}}"#
        ),
        "bad file name",
    );
    rejected::<Block>(
        r#"{"type":"file","blob":"sha256:12","name":"a.txt","media_type":"text/plain"}"#,
        "bad content hash",
    );
    rejected::<Block>(
        r#"{"type":"tool_call","call_id":"c1","name":"read","args":"{}"}"#,
        "bad call id",
    );
    rejected::<Block>(
        r#"{"type":"reasoning","text":"x","private":{"driver":"Anthropic","data":{}}}"#,
        "bad driver family",
    );
}
