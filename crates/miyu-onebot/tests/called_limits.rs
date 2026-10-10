//! 群里的限流和桥重启（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、2、5、7 条）：真核心照开关拉起真桥，假 NapCat 发群
//! 消息。限流满了（规则 `rate = "1/1h"`，别人的那一轮由测试照判官点了头的样子经 `session.respond` 开），终端管理员照样回；白名单成员
//! （握手交来的 `onebot.whitelist`）不受限流，冲她来的不过判官、照样回（施工 O-27，「群里怎么叫她」第 14 条）；别人冲她来回一句
//! 提示、记 `ext.onebot.venues.queued`，再来只记下。桥重启以后照日志
//! 重建：限流照旧满、提示过的不再提示、引用她以前的话照样认得，以前的回复不再发一遍。改了白名单成员，推来就照新的认。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::spawning::{bridge_up, cli, extension, text};
use crate::support::*;

/// 限流是一小时一轮的群。
const GROUP: i64 = 555;

/// 群里的别人、白名单成员。
const LIN: i64 = 20002;
const JIE: i64 = 20003;
const WANG: i64 = 20005;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 限流满了的提示（`group/rate-limited`，中文）。
const LIMITED: &str = "这会儿叫的人太多了，过几分钟再来吧。";

/// 系统的场所规则：[`GROUP`] 一小时一轮、不抽样。
fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\", group = [{GROUP}] }}\nrate = \"1/1h\"\nchatty = {{ probability = 0 }}\n"
    )
}

/// 第 `message` 条消息的命令编号加 `/what`。
fn cause(message: i64, what: &str) -> String {
    format!("qq:{BOT}:{message}:{TIME}/{what}")
}

/// 种类是 `kind`、命令编号是第 `message` 条的加 `/what` 的那一条的 `body`；没有的是空的。
fn noted(events: &[Value], kind: &str, message: i64, what: &str) -> Option<Value> {
    events
        .iter()
        .find(|event| event["kind"] == kind && event["cause"] == cause(message, what))
        .map(|event| event["body"].clone())
}

/// 第 `message` 条消息的那一笔判断。
fn decided(events: &[Value], message: i64) -> Option<Value> {
    noted(events, "ext.onebot.chat.decided", message, "decided")
}

/// 等到第 `message` 条消息的判断记下了：交回那时的全部事件。
async fn until_decided(home: &Home, message: i64) -> Vec<Value> {
    until_events(&home.root, &format!("qq:group:{GROUP}"), |events| {
        decided(events, message).is_some()
    })
    .await
}

#[tokio::test]
async fn the_rate_limit_holds_and_survives_a_restart() {
    let script = Script::new([
        Play::Says("一"),
        Play::Says("二"),
        Play::Says("三"),
        Play::Says("四"),
        Play::Says("五"),
    ]);
    let whitelist = format!("whitelist = [\"qq:{JIE}\"]\n");
    let (home, mut napcat, (listen, web)) = started(&script, &rules(), &whitelist, MEMBERS).await;
    let venue = format!("qq:group:{GROUP}");
    // 1：小林说一句，只记下；测试照判官点了头的样子开一轮（别人开的，算进限流）：一小时一轮满了。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([plain("大家好")]),
        ("小林", "lin"),
    ));
    let events = until_decided(&home, 1).await;
    let (session, _) = venue_session(&home.root, &venue).expect("有会话");
    let mut core = miyu_webserve::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let seq = said(&events)[0]["seq"].clone();
    core.call(
        "judged-1",
        "session.respond",
        json!({"session": session, "to": [seq]}),
    )
    .await
    .expect("开得了一轮");
    assert_eq!(napcat.group_reply(GROUP).await, "一", "她的话发回群里");
    until_events(&home.root, &venue, |events| {
        of_kind(events, "turn.ended").len() == 1
    })
    .await;
    // 2：终端管理员照样回。3：白名单成员不受限流、不过判官，照样回。4：别人冲她来回一句提示。5：再来只记下。
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        2,
        json!([at(BOT), plain(" 在吗")]),
        ("终端管理员", "o"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "二");
    napcat.send(group_frame(
        GROUP,
        JIE,
        3,
        json!([at(BOT), plain(" 嗨")]),
        ("阿杰", "jie"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "三");
    napcat.send(group_frame(
        GROUP,
        WANG,
        4,
        json!([at(BOT), plain(" 嗨")]),
        ("老王", "w"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, LIMITED);
    napcat.send(group_frame(
        GROUP,
        WANG,
        5,
        json!([at(BOT), plain(" 嗨嗨")]),
        ("老王", "w"),
    ));
    let events = until_decided(&home, 5).await;
    assert_eq!(decided(&events, 2).expect("记了")["outcome"], "reply");
    let listed = decided(&events, 3).expect("记了");
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
    let limited = decided(&events, 4).expect("记了");
    assert_eq!(
        (&limited["inbound"], &limited["why"], &limited["outcome"]),
        (&json!("notice"), &json!("rate_limited"), &json!("notice")),
        "{limited}"
    );
    assert_eq!(
        noted(&events, "ext.onebot.venues.queued", 4, "queued"),
        Some(json!({"kind": "notice", "reason": "rate_limited", "text": LIMITED})),
        "提示入队（施工 O-25 中）"
    );
    let again = decided(&events, 5).expect("记了");
    assert_eq!(
        (&again["inbound"], &again["why"], &again["outcome"]),
        (
            &json!("record_only"),
            &json!("rate_limited"),
            &json!("record")
        ),
        "提示过了只记下：{again}"
    );

    // 桥重启：照日志重建。
    let before = extension(&home.root).await["pid"].as_u64();
    drop(napcat);
    let restarted = cli(&home.root, &["restart"]).await;
    assert_eq!(
        restarted.status.code(),
        Some(0),
        "{}",
        text(&restarted.stderr)
    );
    bridge_up(&home.root, listen, web, before)
        .await
        .expect("桥重新起来");
    let mut napcat = admin_napcat(listen).await.answering(MEMBERS);
    // 6：小林冲她来：限流照旧满、这一回提示过了，只记下。7：终端管理员引用她以前的「一」：照样认得，回。
    napcat.send(group_frame(
        GROUP,
        LIN,
        6,
        json!([at(BOT), plain(" 还在吗")]),
        ("小林", "lin"),
    ));
    let quoting = json!([{"type": "reply", "data": {"id": FIRST_SENT.to_string()}}, plain("这个")]);
    napcat.send(group_frame(GROUP, ADMIN, 7, quoting, ("终端管理员", "o")));
    assert_eq!(
        napcat.group_reply(GROUP).await,
        "四",
        "以前的回复不再发，只有新的这一句"
    );
    let events = until_decided(&home, 7).await;
    let after = decided(&events, 6).expect("记了");
    assert_eq!(
        (&after["inbound"], &after["why"], &after["outcome"]),
        (
            &json!("record_only"),
            &json!("rate_limited"),
            &json!("record")
        ),
        "重启以后限流、提示照日志重建：{after}"
    );
    let quoted = decided(&events, 7).expect("记了");
    assert_eq!(quoted["conditions"][0]["kind"], "direct", "{quoted}");
    assert_eq!(quoted["outcome"], "reply", "{quoted}");
    assert_eq!(of_kind(&events, "turn.started").len(), 4);
    // 改了白名单成员（核心推来 `extension.config`）：不重启，老王从下一条起不受限流、冲她来的照回。
    let changes = json!({"layer": "system", "changes": [
        {"key": "onebot.whitelist", "value": [format!("qq:{JIE}"), format!("qq:{WANG}")]},
    ]});
    core.call("whitelist-1", "config.set", changes)
        .await
        .expect("改得了");
    until_log(&home, "whitelist changed count=2").await;
    napcat.send(group_frame(
        GROUP,
        WANG,
        8,
        json!([at(BOT), plain(" 嗨")]),
        ("老王", "w"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "五");
    let events = until_decided(&home, 8).await;
    let listed = decided(&events, 8).expect("记了");
    assert_eq!(
        (&listed["standing"], &listed["inbound"], &listed["route"]),
        (&json!("whitelisted"), &json!("pass"), &json!("commit")),
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
