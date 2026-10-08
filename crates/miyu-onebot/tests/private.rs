//! 主人的私聊（施工 O-8，`onebot.md` 第一条「怎么走」第 5 到 10 条）：送进场所会话、记成主人本人；她的回话发回 QQ；同一条
//! 消息再来只算一次，同一个消息编号、时刻不同的是新的一条，没带时刻的照样送（O-8 补）；不是主人的不送也不回；段的数组、
//! CQ 字符串都认，只有图片的不送；同一个号再连进来，新的顶掉旧的。核心 O-4 中以后，`venue.session` 回的会话属主是桥自己
//! （系统账号）的是陌生人，不接；属主是别的账号、没带属主的照常接（「施工时定的」第 49 条，测试那一头照那时的样子改写账号）。

use serde_json::json;

use miyu_onebot::serve::Notice;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 核心 O-4 中以后，核心拉起的桥握手回的账号：系统账号。
const SYSTEM: &str = "onebot";

/// 起一个桥：握手回的账号是 [`SYSTEM`]，`venue.session` 回的属主是 `venue`（空的不带这一格）。交回桥和转接。
async fn owned_by(home: &Home, venue: Option<&'static str>) -> (Bridge, Relay) {
    let accounts = Accounts { own: SYSTEM, venue };
    let (serve, relay) = serve_relayed(home.root.clone(), settings(), Some(accounts));
    (start(serve).await, relay)
}

#[tokio::test]
async fn the_owners_private_chat_goes_in_and_her_reply_comes_back() {
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.owner_says(501, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    let said = home.said();
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(
        said[0]["by"],
        json!({"kind": "person", "account": "admin", "via": "qq:10001"})
    );
    assert_eq!(said[0]["body"]["blocks"][0]["text"], "在吗");
    assert_eq!(
        said[0]["cause"],
        format!("qq:{BOT}:501:{TIME}"),
        "命令编号拼机器人的号、消息编号和平台给的时刻"
    );
    let created = &home.events(&home.sessions()[0])[0];
    assert_eq!(created["body"]["venue"], "qq:private:10001", "{created}");
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn the_same_message_twice_is_said_once() {
    let script = Script::new([Play::Says("在。"), Play::Says("好。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.owner_says(7, "在吗").await;
    napcat.owner_says(7, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    napcat.owner_says(8, "那好").await;
    assert_eq!(napcat.reply().await, "好。");
    assert_eq!(home.said_texts(), ["在吗", "那好"]);
    assert_eq!(home.sessions().len(), 1);
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn the_same_message_id_at_another_time_is_another_message() {
    // NapCat 重置本地库以后消息编号从头数，同一个编号又来了：时刻不同，是新的一条，不当成重发吞掉（`chat.md` 第七条第 1 条）。
    let script = Script::new([Play::Says("在。"), Play::Says("好。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.owner_says(7, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    let mut again = private_frame(OWNER, 7, json!("又是我"));
    again["time"] = json!(TIME + 86_400);
    napcat.send(again).await;
    assert_eq!(napcat.reply().await, "好。");
    assert_eq!(home.said_texts(), ["在吗", "又是我"]);
    let causes: Vec<_> = home
        .said()
        .iter()
        .map(|said| said["cause"].clone())
        .collect();
    assert_eq!(
        causes,
        [
            json!(format!("qq:{BOT}:7:{TIME}")),
            json!(format!("qq:{BOT}:7:{}", TIME + 86_400)),
        ]
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_message_without_time_still_goes_in_with_time_zero() {
    // OneBot v11 的事件都带 `time`；没带的照 0 拼，照样送进去，不吞（`onebot.md`「施工时定的」第 20 条）。
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    let mut frame = private_frame(OWNER, 12, json!("没带时刻"));
    frame.as_object_mut().expect("是对象").remove("time");
    napcat.send(frame).await;
    assert_eq!(napcat.reply().await, "在。");
    let said = home.said();
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0]["cause"], format!("qq:{BOT}:12:0"));
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_strangers_private_chat_goes_nowhere() {
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat
        .private(
            STRANGER,
            1,
            json!([{"type": "text", "data": {"text": "你好"}}]),
        )
        .await;
    napcat.private(STRANGER, 2, json!("还在吗")).await;
    // 一条条照先后办：主人这一句的回话先到，前面陌生人的两句就是办完了、没回。
    napcat.owner_says(3, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    // 主人的会话已经有了，陌生人再来也不进它。
    napcat.private(STRANGER, 4, json!("我也在")).await;
    napcat.owner_says(5, "好").await;
    assert_eq!(napcat.reply().await, "嗯。");
    assert_eq!(home.said_texts(), ["在吗", "好"]);
    assert_eq!(home.sessions().len(), 1, "陌生人没有会话");
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn segments_and_cq_strings_are_read_and_image_only_messages_are_not_sent() {
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat
        .private(
            OWNER,
            1,
            json!([{"type": "image", "data": {"file": "a.png"}}]),
        )
        .await;
    napcat
        .private(
            OWNER,
            2,
            json!([
                {"type": "text", "data": {"text": "你"}},
                {"type": "image", "data": {"file": "a.png"}},
                {"type": "text", "data": {"text": "好"}},
            ]),
        )
        .await;
    assert_eq!(napcat.reply().await, "一。");
    napcat
        .private(OWNER, 3, json!("[CQ:image,file=b.png]  "))
        .await;
    napcat
        .private(
            OWNER,
            4,
            json!("[CQ:face,id=14]在&#91;吗&#93;&#44;&amp;[CQ:at,qq=1]"),
        )
        .await;
    assert_eq!(napcat.reply().await, "二。");
    assert_eq!(home.said_texts(), ["你好", "在[吗],&"]);
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_new_connection_for_the_same_bot_takes_over() {
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let heard = |notice: Notice| {
        let notices = std::sync::Arc::clone(&bridge.notices);
        within("桥说了", async move {
            while !notices.lock().expect("没 panic").contains(&notice) {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
    };
    let old = owner_napcat(bridge.port).await;
    heard(Notice::Connected { bot: Some(BOT) }).await;
    let mut new = owner_napcat(bridge.port).await;
    // 旧的那一条断开了：号已经是新的那一条的，不跟着拿掉。
    drop(old);
    heard(Notice::Disconnected { bot: Some(BOT) }).await;
    new.owner_says(9, "在吗").await;
    assert_eq!(new.reply().await, "在。");
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_venue_owned_by_the_bridge_itself_is_a_stranger_and_not_taken() {
    let home = Home::new(&Script::new([Play::Says("不该说话。")]));
    let (bridge, relay) = owned_by(&home, Some(SYSTEM)).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.owner_says(1, "在吗").await;
    napcat.owner_says(2, "/stop").await;
    napcat.owner_says(3, "还在吗").await;
    napcat.owner_says(4, "在吗").await;
    // 一条条照先后办：第四条去问 `venue.session` 的时候，前三条已经办完了。会话编号不记进缓存：每一条都再问。
    let asks = |relay: &Relay| {
        relay
            .asked()
            .iter()
            .filter(|method| *method == "venue.session")
            .count()
    };
    within("四条都问过会话", async {
        while asks(&relay) < 4 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    assert_eq!(
        relay.asked(),
        ["venue.session"; 4],
        "不交 session.send、command.run，不订阅"
    );
    assert!(home.said().is_empty(), "不进任何会话：{:?}", home.said());
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_venue_owned_by_someone_else_or_by_nobody_is_taken() {
    for venue in [Some("admin"), None] {
        let home = Home::new(&Script::new([Play::Says("在。")]));
        let (bridge, relay) = owned_by(&home, venue).await;
        let mut napcat = owner_napcat(bridge.port).await;
        napcat.owner_says(1, "在吗").await;
        assert_eq!(napcat.reply().await, "在。", "{venue:?}");
        assert_eq!(home.said_texts(), ["在吗"], "{venue:?}");
        assert_eq!(
            relay.asked(),
            ["venue.session", "subscribe", "session.send"],
            "{venue:?}"
        );
        bridge.stop().await.expect("停得下");
    }
}
