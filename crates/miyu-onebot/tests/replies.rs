//! 发回去的（施工 O-8，`onebot.md` 第一条「怎么走」第 9、10 条）：只发她回话里的文字，思考、工具调用不发，空的不发；
//! 订阅不写 `after`，桥重启以后以前的回话不再发一遍。

use miyu_session::testkit::{Play, Script};

use crate::support::*;

#[tokio::test]
async fn thinking_tool_calls_and_empty_replies_are_not_sent() {
    let script = Script::new([
        Play::Thinks {
            thinking: "想一想",
            text: "答案",
        },
        Play::calls(&[("nope", "{}")]),
        Play::Says("说完了"),
        Play::Says("  \n "),
        Play::Says("最后"),
    ]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "问一").await;
    assert_eq!(napcat.reply().await, "答案");
    napcat.admin_says(2, "问二").await;
    assert_eq!(napcat.reply().await, "说完了");
    napcat.admin_says(3, "问三").await;
    // 空的那一句没发：下一个到的是第四句的回话。
    napcat.admin_says(4, "问四").await;
    assert_eq!(napcat.reply().await, "最后");
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn after_a_restart_old_replies_are_not_sent_again() {
    let script = Script::new([Play::Says("第一句"), Play::Says("第二句")]);
    let home = Home::new(&script);
    let first = bridge(&home).await;
    let mut napcat = admin_napcat(first.port).await;
    napcat.admin_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "第一句");
    napcat.close().await;
    first.stop().await.expect("停得下");

    let second = bridge(&home).await;
    let mut napcat = admin_napcat(second.port).await;
    napcat.admin_says(2, "还在吗").await;
    assert_eq!(napcat.reply().await, "第二句", "以前的回话没再发一遍");
    assert_eq!(home.sessions().len(), 1, "重启以后找回的是同一个会话");
    second.stop().await.expect("停得下");
}
