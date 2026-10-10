//! 群里叫她就回（施工 O-23，`onebot.md` 第一条「群里怎么叫她」）：真核心照开关拉起真桥，假 NapCat 发群消息，模型替身照剧本
//! 说。终端管理员 @ 她、叫她的名字（触发词开头）、引用她的消息，各开一轮，她的回话转成纯文本发回群里、记 `venue.delivered`；别人的
//! @ 要问判官，照剧本回的核心没有一次性入口、回 `no_model`，判不了、不回（O-23 下；判官说回、说不回的在 `judged.rs`）；没条件的只记下；每一条都记一笔 `ext.onebot.chat.decided`。睡觉时间里终端管理员照回、别人只记下；终端管理员连发两条，
//! 正在跑的一轮并进去；同一条消息平台重发不再判。她的话拆成几段发，NapCat 回的先后和发的先后不一样，`venue.delivered` 也照
//! 发的先后记。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::*;

/// 触发词是她的名字的群。
const GROUP: i64 = 555;

/// 规则里睡觉时间盖住此刻的群。
const SLEEPY: i64 = 666;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 核心照中文写的 `/stop` 的回执（`resources/core/human/zh.json`）。
const STOPPED: &str = "已全部停下。";

/// 系统的场所规则：两个群都不抽样（判断不随序号变）、她的话一段最多 4 个字符；[`GROUP`] 的触发词是她的名字，[`SLEEPY`] 的
/// 睡觉时间盖住此刻（前后各一个小时，照本机的时区）。
fn rules() -> String {
    let now = jiff::Zoned::now();
    let minute = i64::from(now.hour()) * 60 + i64::from(now.minute());
    let clock = |minute: i64| {
        let minute = minute.rem_euclid(24 * 60);
        format!("{:02}:{:02}", minute / 60, minute % 60)
    };
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\noutbound = {{ split_chars = 4 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{GROUP}] }}\nkeywords = [\"米尤\"]\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SLEEPY}] }}\nsleep = \"{}-{}\"\n",
        clock(minute - 60),
        clock(minute + 60),
    )
}

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 第 `message` 条消息的命令编号加 `/what`（「施工时定的」第 77 条）。
fn cause(message: i64, what: &str) -> String {
    format!("qq:{BOT}:{message}:{TIME}/{what}")
}

/// 第 `message` 条消息的那一笔判断的 `body`；没有的是空的。
fn decided(events: &[Value], message: i64) -> Option<Value> {
    events
        .iter()
        .find(|event| {
            event["kind"] == "ext.onebot.chat.decided"
                && event["cause"] == cause(message, "decided")
        })
        .map(|event| event["body"].clone())
}

/// 等到群 `group` 里第 `message` 条消息的判断记下了：交回那时的全部事件。
async fn until_decided(home: &Home, group: i64, message: i64) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        decided(events, message).is_some()
    })
    .await
}

/// 平台编号是 `message` 的那一条人说的话的序号。
fn seq_of(events: &[Value], message: i64) -> u64 {
    said(events)
        .iter()
        .find(|said| said["body"]["venue"]["msg"].as_str() == Some(message.to_string().as_str()))
        .and_then(|said| said["seq"].as_u64())
        .unwrap_or_else(|| panic!("没有第 {message} 条：{events:#?}"))
}

/// 等到群 `group` 里已经有 `turns` 轮走完、记了 `delivered` 条 `venue.delivered`。
async fn until_answered(home: &Home, group: i64, turns: usize, delivered: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, "turn.ended").len() == turns
            && of_kind(events, "venue.delivered").len() == delivered
    })
    .await
}

#[tokio::test]
async fn the_admin_calling_her_gets_an_answer_in_the_group() {
    let script = Script::new([
        Play::Says("在。"),
        Play::Says("叫我？"),
        Play::Says("**看到**了。"),
    ]);
    let (home, mut napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    // 1：没条件的只记下。2：终端管理员 @ 她。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([plain("大家好")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        2,
        json!([at(BOT), plain(" 在吗")]),
        ("终端管理员", "o"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "在。");
    until_answered(&home, GROUP, 1, 1).await;
    // 3：叫她的名字（触发词开头）。
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        3,
        json!([plain("米尤，帮个忙")]),
        ("终端管理员", "o"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "叫我？");
    until_answered(&home, GROUP, 2, 2).await;
    // 4：引用她说的第一句（假 NapCat 回的编号）。
    let quoting =
        json!([{"type": "reply", "data": {"id": FIRST_SENT.to_string()}}, plain("这个呢")]);
    napcat.send(group_frame(GROUP, ADMIN, 4, quoting, ("终端管理员", "o")));
    assert_eq!(napcat.group_reply(GROUP).await, "看到了。", "转成纯文本");
    until_answered(&home, GROUP, 3, 3).await;
    // 5：别人的 @：只记判断，不回。
    napcat.send(group_frame(
        GROUP,
        JIE,
        5,
        json!([at(BOT), plain(" 你好")]),
        ("阿杰", "jie"),
    ));
    let events = until_decided(&home, GROUP, 5).await;

    let (session, _) = venue_session(&home.root, &venue(GROUP)).expect("有会话");
    let seqs: Vec<u64> = (1..=5).map(|message| seq_of(&events, message)).collect();
    assert_eq!(
        decided(&events, 1),
        Some(json!({
            "msgs": [seqs[0]], "standing": "member", "inbound": "pass", "discipline": "chatty", "conditions": [],
            "route": "record", "outcome": "record",
        })),
        "没条件的只记下"
    );
    assert_eq!(
        decided(&events, 2),
        Some(json!({
            "msgs": [seqs[1]], "standing": "admin", "inbound": "pass", "discipline": "chatty",
            "conditions": [{"kind": "direct", "bonus": 0.3}], "route": "commit", "outcome": "reply",
        })),
        "终端管理员 @ 她：终端管理员照核心记下的 by 认"
    );
    for message in [3, 4] {
        let body = decided(&events, message).expect("记了判断");
        assert_eq!(body["outcome"], "reply", "{body}");
        assert_eq!(body["conditions"][0]["kind"], "direct", "{body}");
    }
    let other = decided(&events, 5).expect("记了判断");
    assert_eq!(other["standing"], "member", "{other}");
    assert_eq!(other["conditions"][0]["kind"], "direct", "{other}");
    assert_eq!(other["route"], "judge", "{other}");
    assert_eq!(
        other["judge"],
        json!({"mode": "reply", "tries": 2, "millis": other["judge"]["millis"], "unjudged": "refused", "detail": "no_model"}),
        "别人的 @ 要问判官，判不了的再问一次：{other}"
    );
    assert_eq!(other["outcome"], "record", "判不了照不回算：{other}");

    let started = of_kind(&events, "turn.started");
    assert_eq!(started.len(), 3, "终端管理员叫了三次，开三轮：{started:#?}");
    for (turn, message) in started.iter().zip([2, 3, 4]) {
        assert_eq!(turn["cause"], cause(message, "respond"), "{turn}");
        assert_eq!(
            turn["body"]["triggers"],
            json!([seq_of(&events, message)]),
            "{turn}"
        );
    }
    let delivered = of_kind(&events, "venue.delivered");
    let texts = ["在。", "叫我？", "看到了。"];
    assert_eq!(delivered.len(), 3, "{delivered:#?}");
    for (n, (one, turn)) in delivered.iter().zip(&started).enumerate() {
        assert_eq!(
            one["body"],
            json!({
                "line": session, "turn": turn["seq"], "to": ["qq:10001"], "msg": (FIRST_SENT + n as i64).to_string(),
                "text": texts[n],
            }),
            "{one}"
        );
    }
    assert_eq!(script.requests().len(), 3, "别人的 @ 不开回合");
    assert!(napcat.pending().is_none(), "别人的 @ 不回");
    let log = run_log(&home.root);
    assert!(log.contains("chat decided"), "{log}");
    assert!(
        !log.contains("大家好") && !log.contains("帮个忙"),
        "原文不进运行日志"
    );
    stopped(home).await;
}

#[tokio::test]
async fn asleep_joined_and_resent() {
    let script = Script::new([Play::Stalls, Play::Says("醒着呢\n\n别吵")]);
    let (home, mut napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    // 终端管理员 @ 她，她还没开口；终端管理员又 @ 一次：并进正在跑的这一轮。
    let first = json!([at(BOT), plain(" 在吗")]);
    napcat.send(group_frame(GROUP, ADMIN, 1, first, ("终端管理员", "o")));
    until_event(&home.root, &venue(GROUP), |event| {
        event["kind"] == "turn.started"
    })
    .await;
    let again = json!([at(BOT), plain(" 还有这个")]);
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        2,
        again.clone(),
        ("终端管理员", "o"),
    ));
    let events = until_event(&home.root, &venue(GROUP), |event| {
        event["kind"] == "turn.joined"
    })
    .await;
    let joined = &of_kind(&events, "turn.joined")[0];
    assert_eq!(joined["body"]["triggers"], json!([seq_of(&events, 2)]));
    assert_eq!(joined["cause"], cause(2, "respond"));
    // 平台重发第 2 条：不再判。终端管理员 /stop 停下这一轮，她没开口，不发空的。
    napcat.send(group_frame(GROUP, ADMIN, 2, again, ("终端管理员", "o")));
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        3,
        json!([plain("/stop")]),
        ("终端管理员", "o"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, STOPPED);
    // 睡着的群：别人冲她来只记下，终端管理员照回；她的话照这个群的参数拆成两段，一段一条、各记一笔。
    let waking = json!([at(BOT), plain(" 醒醒")]);
    napcat.send(group_frame(SLEEPY, LIN, 4, waking.clone(), ("小林", "lin")));
    napcat.send(group_frame(SLEEPY, ADMIN, 5, waking, ("终端管理员", "o")));
    assert_eq!(napcat.group_reply(SLEEPY).await, "醒着呢");
    assert_eq!(napcat.group_reply(SLEEPY).await, "别吵");
    let sleepy = until_events(&home.root, &venue(SLEEPY), |events| {
        of_kind(events, "venue.delivered").len() == 2
    })
    .await;
    let delivered: Vec<Value> = of_kind(&sleepy, "venue.delivered")
        .iter()
        .map(|one| json!([one["body"]["turn"], one["body"]["msg"], one["body"]["text"]]))
        .collect();
    let turn = &of_kind(&sleepy, "turn.started")[0]["seq"];
    assert_eq!(
        delivered,
        [
            json!([turn, (FIRST_SENT + 1).to_string(), "醒着呢"]),
            json!([turn, (FIRST_SENT + 2).to_string(), "别吵"]),
        ],
        "/stop 的回执先用了一个编号"
    );
    assert_eq!(
        decided(&sleepy, 4),
        Some(json!({
            "msgs": [seq_of(&sleepy, 4)], "standing": "member", "inbound": "record_only", "why": "asleep",
            "outcome": "record",
        }))
    );
    assert_eq!(decided(&sleepy, 5).expect("记了")["outcome"], "reply");

    let events = venue_events(&home.root, &venue(GROUP));
    assert_eq!(
        of_kind(&events, "turn.started").len(),
        1,
        "并进去的、重发的不另开一轮"
    );
    let twice = of_kind(&events, "ext.onebot.chat.decided")
        .iter()
        .filter(|event| event["cause"] == cause(2, "decided"))
        .count();
    assert_eq!(twice, 1, "重发的不再判");
    let judged = run_log(&home.root)
        .lines()
        .filter(|line| line.contains("chat decided venue=qq:group:555 message=2 "))
        .count();
    assert_eq!(judged, 1, "重发的不再判，运行日志也只一行");
    assert_eq!(
        said(&events).len(),
        2,
        "核心只记一次；/stop 是命令，不进人说的话"
    );
    assert_eq!(
        script.requests().len(),
        2,
        "并进去的没再请求，睡着的群终端管理员那一轮"
    );
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

#[tokio::test]
async fn her_pieces_are_noted_in_the_order_they_were_sent() {
    // NapCat 并着办动作：后发的那一段先回到。`venue.delivered` 照发的先后记：核心照日志的先后画她说的话。
    let script = Script::new([Play::Says("醒着呢\n\n别吵")]);
    let (home, mut napcat, _) =
        started_with(&script, &rules(), "", |napcat| napcat.reversing(MEMBERS, 2)).await;
    let calling = json!([at(BOT), plain(" 在吗")]);
    napcat.send(group_frame(GROUP, ADMIN, 1, calling, ("终端管理员", "o")));
    assert_eq!(napcat.group_reply(GROUP).await, "醒着呢");
    assert_eq!(napcat.group_reply(GROUP).await, "别吵");
    let events = until_events(&home.root, &venue(GROUP), |events| {
        of_kind(events, "venue.delivered").len() == 2
    })
    .await;
    let noted: Vec<Value> = of_kind(&events, "venue.delivered")
        .iter()
        .map(|one| json!([one["body"]["msg"], one["body"]["text"]]))
        .collect();
    assert_eq!(
        noted,
        [
            json!([FIRST_SENT.to_string(), "醒着呢"]),
            json!([(FIRST_SENT + 1).to_string(), "别吵"]),
        ],
        "照发的先后记，不照 NapCat 回的先后"
    );
    stopped(home).await;
}
