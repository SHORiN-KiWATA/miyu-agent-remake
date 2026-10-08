//! 读出私聊里的文字（施工 O-8，`onebot.md` 第一条「怎么走」第 6 条）：段的数组取 `text` 段依次接起来；CQ 字符串去掉
//! `[CQ:…]`，再把四种转义换回来，`&amp;` 最后换。

use serde_json::json;

use miyu_onebot::onebot::text_of;

#[test]
fn text_segments_are_joined_and_other_segments_skipped() {
    let message = json!([
        {"type": "reply", "data": {"id": "1"}},
        {"type": "text", "data": {"text": "早"}},
        {"type": "face", "data": {"id": "14"}},
        {"type": "text", "data": {"text": "上好 "}},
        {"type": "image", "data": {"file": "a.png"}},
    ]);
    assert_eq!(text_of(&message), "早上好 ");
    assert_eq!(text_of(&json!([{"type": "image", "data": {}}])), "");
    assert_eq!(text_of(&json!([{"type": "text", "data": {}}])), "");
    assert_eq!(text_of(&json!(null)), "");
}

#[test]
fn cq_codes_are_dropped_and_escapes_turned_back() {
    assert_eq!(text_of(&json!("[CQ:at,qq=1] 你好[CQ:face,id=1]")), " 你好");
    assert_eq!(text_of(&json!("&#91;x&#93;&#44;y&amp;z")), "[x],y&z");
    assert_eq!(
        text_of(&json!("&amp;#91;")),
        "&#91;",
        "先换括号、最后换 &amp;"
    );
    assert_eq!(text_of(&json!("没关上的 [CQ:image")), "没关上的 [CQ:image");
    assert_eq!(text_of(&json!("[CQ:image,file=a.png]")), "");
}
