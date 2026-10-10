//! 白名单成员（施工 O-27，`onebot.md` 第一条「怎么走」第 7 条、「好友请求」、「群里怎么叫她」第 3、5 条；18 第三节，2026-10-10
//! 项目主人定）：真核心照开关拉起真桥（系统配置写 `onebot.whitelist`），假 NapCat 发私聊、好友请求、群消息。
//!
//! - 白名单成员私聊她，每一条都开一轮、她的话发回去，会话归系统账号、照场所规则造（规则的预设）；陌生人的照旧不接；终端
//!   管理员的私聊不照规则；从白名单里删了，下一条就不接。
//! - 白名单成员加她好友，桥调 `set_friend_add_request {flag, approve: true}`；陌生人的放着、不回，群邀请也放着，各记一行运行日志。
//! - 睡觉时间里白名单成员在群里 @ 她照回，冲她来的不过判官；别人照旧只记下，醒着时别人的 @ 照旧问判官。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::*;

/// 白名单成员。
const JIE: i64 = 20003;

/// 群里的别人。
const LIN: i64 = 20004;

/// 规则里睡觉时间盖住此刻的群。
const SLEEPY: i64 = 666;

/// 醒着的群。
const AWAKE: i64 = 777;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统配置的 `[onebot]` 多写的：白名单里只有 [`JIE`]。
fn whitelist() -> String {
    format!("whitelist = [\"qq:{JIE}\"]\n")
}

/// 系统的场所规则：群都不抽样；[`SLEEPY`] 的睡觉时间盖住此刻（前后各一个小时，照本机的时区）。
fn rules() -> String {
    let now = jiff::Zoned::now();
    let minute = i64::from(now.hour()) * 60 + i64::from(now.minute());
    let clock = |minute: i64| {
        let minute = minute.rem_euclid(24 * 60);
        format!("{:02}:{:02}", minute / 60, minute % 60)
    };
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SLEEPY}] }}\nsleep = \"{}-{}\"\n",
        clock(minute - 60),
        clock(minute + 60),
    )
}

/// 系统的场所规则：私聊用预设 `dev`（出厂的是 `full`）。
const PRIVATE_RULES: &str = "[[rule]]\nmatch = { kind = \"private\" }\npreset = \"dev\"\n";

/// 下一个动作是发给 `user` 的 `send_private_msg`：交回里面的字。
async fn private_reply(napcat: &mut Answering, user: i64) -> String {
    let action = napcat.action().await;
    assert_eq!(action["action"], "send_private_msg", "{action}");
    assert_eq!(action["params"]["user_id"], user, "{action}");
    action["params"]["message"][0]["data"]["text"]
        .as_str()
        .expect("有字")
        .to_string()
}

/// 一条好友请求，照 NapCat 发的样子：`user` 要加她，标记是 `flag`。
fn befriend(user: i64, flag: &str) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "request",
        "request_type": "friend",
        "user_id": user,
        "comment": "加个好友",
        "flag": flag,
    })
}

/// 一条群邀请：`user` 邀请她进群 `group`。
fn invited(user: i64, group: i64) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "request",
        "request_type": "group",
        "sub_type": "invite",
        "group_id": group,
        "user_id": user,
        "comment": "",
        "flag": "invite-1",
    })
}

/// 第 `message` 条群消息的那一笔判断的 `body`；没有的是空的。
fn decided(events: &[Value], message: i64) -> Option<Value> {
    let cause = format!("qq:{BOT}:{message}:{TIME}/decided");
    events
        .iter()
        .find(|event| event["kind"] == "ext.onebot.chat.decided" && event["cause"] == cause)
        .map(|event| event["body"].clone())
}

#[tokio::test]
async fn the_whitelist_chats_in_private_and_strangers_do_not() {
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。"), Play::Says("好。")]);
    let (home, mut napcat, _) = started(&script, PRIVATE_RULES, &whitelist(), MEMBERS).await;
    // 陌生人先说，白名单成员接着说：一条条照先后办，白名单成员这一句的回话先到，陌生人的就是办完了、没回。
    napcat.send(private_frame(STRANGER, 1, json!([plain("你好")])));
    napcat.send(private_frame(JIE, 2, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat, JIE).await, "在。");
    napcat.send(private_frame(JIE, 3, json!([plain("还在吗")])));
    assert_eq!(
        private_reply(&mut napcat, JIE).await,
        "嗯。",
        "每一条都开一轮"
    );
    let theirs = until_events(&home.root, &format!("qq:private:{JIE}"), |events| {
        of_kind(events, "turn.ended").len() == 2
    })
    .await;
    let spoken = said(&theirs);
    let texts: Vec<&str> = spoken.iter().map(words).collect();
    assert_eq!(texts, ["在吗", "还在吗"], "会话归系统账号");
    assert_eq!(spoken[0]["by"]["id"], format!("qq:{JIE}"), "{theirs:#?}");
    assert_eq!(
        theirs[0]["body"]["preset"], "dev",
        "照场所规则造：{:#}",
        theirs[0]
    );
    let stranger = venue_events(&home.root, &format!("qq:private:{STRANGER}"));
    assert!(
        said(&stranger).is_empty(),
        "陌生人的话不进会话：{stranger:#?}"
    );
    // 从白名单里删了：推来就照新的认，下一条不接。
    let mut core = miyu_webserve::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let changes = json!({"layer": "system", "changes": [{"key": "onebot.whitelist", "value": []}]});
    let reply = core
        .call("whitelist-1", "config.set", changes)
        .await
        .expect("改得了");
    assert!(reply.get("error").is_none(), "{reply}");
    until_log(&home, "whitelist changed count=0").await;
    napcat.send(private_frame(JIE, 4, json!([plain("我还在")])));
    // 终端管理员接着说：她的回话先到，前面那一条就是办完了、没回。
    napcat.send(private_frame(ADMIN, 5, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat, ADMIN).await, "好。");
    let theirs = venue_events(&home.root, &format!("qq:private:{JIE}"));
    assert_eq!(said(&theirs).len(), 2, "删了以后的不进会话：{theirs:#?}");
    let admins = home.sessions();
    assert_eq!(admins.len(), 1, "{admins:?}");
    let created = &home.events(&admins[0])[0];
    assert_eq!(created["body"]["venue"], format!("qq:private:{ADMIN}"));
    assert_ne!(
        created["body"]["preset"], "dev",
        "终端管理员的私聊不照规则造：{created:#}"
    );
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

#[tokio::test]
async fn friend_requests_from_the_whitelist_are_approved_and_others_wait() {
    let (home, mut napcat, _) = started(&Script::new([]), "", &whitelist(), MEMBERS).await;
    napcat.send(befriend(JIE, "flag-1"));
    let action = napcat.action().await;
    assert_eq!(
        (&action["action"], &action["params"]),
        (
            &json!("set_friend_add_request"),
            &json!({"flag": "flag-1", "approve": true})
        ),
        "{action}"
    );
    // 陌生人的请求、群邀请：不回、放着。白名单成员再来一次：下一个动作就是它，前面两件什么都没调。
    napcat.send(befriend(STRANGER, "flag-2"));
    napcat.send(invited(JIE, 888));
    napcat.send(befriend(JIE, "flag-3"));
    let action = napcat.action().await;
    assert_eq!(action["params"]["flag"], "flag-3", "{action}");
    until_log(&home, "friend request approved").await;
    let log = run_log(&home.root);
    assert!(
        log.contains(&format!("user={STRANGER}")) && log.contains("friend request left pending"),
        "{log}"
    );
    assert!(
        log.contains("group invite left pending") && log.contains("group=888"),
        "{log}"
    );
    assert!(!log.contains("flag-"), "标记不进运行日志：{log}");
    assert!(napcat.pending().is_none(), "别的什么都不调");
    stopped(home).await;
}

#[tokio::test]
async fn the_whitelist_is_answered_while_she_sleeps_and_skips_the_judge() {
    let script = Script::new([Play::Says("醒着呢"), Play::Says("来了")]);
    let (home, mut napcat, _) = started(&script, &rules(), &whitelist(), MEMBERS).await;
    let sleepy = format!("qq:group:{SLEEPY}");
    // 睡觉时间：别人 @ 她只记下，白名单成员 @ 她照回、不问判官。
    napcat.send(group_frame(
        SLEEPY,
        LIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        SLEEPY,
        JIE,
        2,
        json!([at(BOT), plain(" 在吗")]),
        ("阿杰", "jie"),
    ));
    assert_eq!(napcat.group_reply(SLEEPY).await, "醒着呢");
    let events = until_events(&home.root, &sleepy, |events| decided(events, 2).is_some()).await;
    let member = decided(&events, 1).expect("记了");
    assert_eq!(
        (&member["inbound"], &member["why"]),
        (&json!("record_only"), &json!("asleep")),
        "{member}"
    );
    let listed = decided(&events, 2).expect("记了");
    assert_eq!(
        (
            &listed["standing"],
            &listed["inbound"],
            &listed["route"],
            &listed["outcome"]
        ),
        (
            &json!("whitelisted"),
            &json!("pass"),
            &json!("commit"),
            &json!("reply")
        ),
        "{listed}"
    );
    assert!(listed.get("judge").is_none(), "不问判官：{listed}");
    // 醒着的群：别人 @ 她照旧问判官（剧本没有一次性入口，判不了、不回），白名单成员的直接回。
    let awake = format!("qq:group:{AWAKE}");
    napcat.send(group_frame(
        AWAKE,
        LIN,
        3,
        json!([at(BOT), plain(" 嗨")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        AWAKE,
        JIE,
        4,
        json!([at(BOT), plain(" 嗨")]),
        ("阿杰", "jie"),
    ));
    assert_eq!(napcat.group_reply(AWAKE).await, "来了");
    let events = until_events(&home.root, &awake, |events| {
        decided(events, 3).is_some() && decided(events, 4).is_some()
    })
    .await;
    let member = decided(&events, 3).expect("记了");
    assert_eq!(
        (&member["route"], &member["outcome"]),
        (&json!("judge"), &json!("record")),
        "{member}"
    );
    let listed = decided(&events, 4).expect("记了");
    assert_eq!(
        (&listed["route"], &listed["outcome"]),
        (&json!("commit"), &json!("reply")),
        "{listed}"
    );
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

/// 等到运行日志里有 `wanted`。
async fn until_log(home: &Home, wanted: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !run_log(&home.root).contains(wanted) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "运行日志里等不到 {wanted}：{}",
            run_log(&home.root)
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
