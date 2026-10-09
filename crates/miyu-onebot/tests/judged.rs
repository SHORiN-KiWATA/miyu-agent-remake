//! 问判官（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 7、12、13 条）：真核心照开关拉起真桥，假 NapCat 发群消息，她的
//! 回合照剧本说，判官那一次（`model.call`）发到本机回环上的假服务器（`support::judge`）。别人 @ 她，判官说回就回、说不回就
//! 不回；抽样中了交判官；只有违规旗的只查违规，严重程度够了照回；判官回的读不出、等不到的照不回、记为什么，读不出的再问一次；
//! `ext.onebot.chat.decided` 的格都在；判官在判的时候，别人说的照样记进、判完。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::judge::*;
use crate::support::*;

/// 不抽样的群：别人 @ 她交判官。
const GROUP: i64 = 555;

/// 抽样必中的群。
const SAMPLED: i64 = 556;

/// 她没说过话的群：只有违规旗的消息只查违规。
const QUIET: i64 = 557;

/// 判官一秒就算等不到、不重试的群。
const SLOW: i64 = 558;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的违规词表里的一个词。
const BAD: &str = "坏词";

/// 系统的场所规则：群默认不抽样，[`SAMPLED`] 抽样必中，[`SLOW`] 判官等一秒、不重试。
fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SAMPLED}] }}\nchatty = {{ probability = 1000 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SLOW}] }}\njudge = {{ timeout = \"1s\", retries = 0 }}\n"
    )
}

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 第 `message` 条消息的命令编号加 `/what`。
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

/// 等到群 `group` 里已经有 `turns` 轮走完。
async fn until_turns(home: &Home, group: i64, turns: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, "turn.ended").len() == turns
    })
    .await
}

/// `value` 是数，和 `expected` 差不到百万分之一。
fn close(value: &Value, expected: f64) -> bool {
    value
        .as_f64()
        .is_some_and(|value| (value - expected).abs() < 1e-6)
}

/// 资源里判官的一份说明。
fn judge_text(name: &str) -> String {
    std::fs::read_to_string(resources().join("software/onebot/judge").join(name)).expect("读得出")
}

#[tokio::test]
async fn others_calling_her_are_judged() {
    let server = judge(vec![
        yes("they asked her"),
        no("talking to someone else"),
        yes("an easy one"),
        verdict(json!({
            "relevance": 0, "willingness": 0, "social": 0, "timing": 0, "continuity": 0,
            "should_reply": false, "to_bot": false, "severity": 8, "reason": "abusive",
        })),
    ])
    .await;
    let script = Script::new([
        Play::Says("在呢。"),
        Play::Says("是啊。"),
        Play::Says("别这样。"),
    ]);
    let words = format!("{BAD}\n");
    let (home, mut napcat, _) =
        started_judged(&script, &server, (&rules(), &words), "", MEMBERS).await;
    // 1：小林 @ 她，判官说回。2：阿杰 @ 她，判官说不回。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "在呢。");
    until_turns(&home, GROUP, 1).await;
    napcat.send(group_frame(
        GROUP,
        JIE,
        2,
        json!([at(BOT), plain(" 你看这个")]),
        ("阿杰", "jie"),
    ));
    let events = until_decided(&home, GROUP, 2).await;
    // 3：抽样中了。4：她没说过话的群里只有违规旗，只查违规，严重程度够了照回。
    napcat.send(group_frame(
        SAMPLED,
        LIN,
        3,
        json!([plain("今天天气不错")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.group_reply(SAMPLED).await, "是啊。");
    napcat.send(group_frame(
        QUIET,
        JIE,
        4,
        json!([plain(&format!("这里有{BAD}"))]),
        ("阿杰", "jie"),
    ));
    assert_eq!(napcat.group_reply(QUIET).await, "别这样。");
    let sampled = until_decided(&home, SAMPLED, 3).await;
    let quiet = until_decided(&home, QUIET, 4).await;

    let asked_her = decided(&events, 1).expect("记了判断");
    let seq = |events: &[Value], n: usize| said(events)[n]["seq"].clone();
    assert_eq!(asked_her["msgs"], json!([seq(&events, 0)]));
    assert_eq!(asked_her["discipline"], "chatty", "{asked_her}");
    assert_eq!(asked_her["route"], "judge", "{asked_her}");
    let judged = &asked_her["judge"];
    assert_eq!(
        (&judged["mode"], &judged["tries"], &judged["model"]),
        (&json!("reply"), &json!(1), &json!(JUDGE_MODEL)),
        "{asked_her}"
    );
    assert!(judged["millis"].is_u64(), "{asked_her}");
    assert_eq!(
        judged["answer"],
        json!({"scores": [9.0, 9.0, 9.0, 9.0, 9.0], "should_reply": true, "to_bot": true, "severity": 0, "reason": "they asked her"}),
        "{asked_her}"
    );
    assert!(judged.get("unjudged").is_none(), "{asked_her}");
    let score = &asked_her["score"];
    assert!(
        close(&score["raw"], 0.9)
            && close(&score["adjust"], 0.2)
            && close(&score["bonus"], 0.3)
            && close(&score["lift"], 0.0)
            && close(&score["threshold"], 0.8)
            && close(&score["total"], 1.4)
            && score["reply"] == true,
        "冲她来的免冷静：{score}"
    );
    assert_eq!(asked_her["outcome"], "reply");

    let refused = decided(&events, 2).expect("记了判断");
    assert_eq!(
        refused["judge"]["answer"]["should_reply"], false,
        "{refused}"
    );
    assert_eq!(
        refused["judge"]["answer"]["reason"],
        "talking to someone else"
    );
    assert_eq!(refused["score"]["reply"], false, "{refused}");
    assert_eq!(refused["outcome"], "record", "判官说不回就不回：{refused}");
    assert_eq!(of_kind(&events, "turn.started").len(), 1, "只开了一轮");

    let sample = decided(&sampled, 3).expect("记了判断");
    assert_eq!(
        sample["conditions"],
        json!([{"kind": "probability", "bonus": 0.0}])
    );
    assert_eq!(
        (&sample["route"], &sample["outcome"]),
        (&json!("judge"), &json!("reply")),
        "{sample}"
    );

    let violated = decided(&quiet, 4).expect("记了判断");
    assert_eq!(violated["flags"], json!(["moderation"]), "{violated}");
    assert_eq!(violated["route"], "moderation_only", "{violated}");
    assert_eq!(violated["judge"]["mode"], "moderation_only", "{violated}");
    assert_eq!(violated["judge"]["answer"]["severity"], 8, "{violated}");
    assert_eq!(
        violated["score"]["reply"], true,
        "严重程度够了照回：{violated}"
    );
    assert_eq!(violated["outcome"], "reply");

    // 判官看到的：一条 system、一条 user；打分的照 reply.txt，只查违规的照 moderation-only.txt；最多输出照出厂的 400。
    let first = asked(&server, 0);
    let messages = first["messages"].as_array().expect("有消息");
    assert_eq!(messages.len(), 2, "{first}");
    assert_eq!(messages[0]["role"], "system");
    let system = messages[0]["content"].as_str().expect("是字");
    assert!(system.starts_with(&judge_text("system.txt")), "{system}");
    assert!(system.contains(&judge_text("reply.txt")), "{system}");
    assert!(system.contains("7"), "违规的门槛换进去了：{system}");
    let user = messages[1]["content"].as_str().expect("是字");
    assert!(user.contains(&judge_text("current-open.txt")), "{user}");
    assert!(user.contains("在吗"), "{user}");
    assert_eq!(first["model"], "m", "判官没写模型，照 models.chat");
    assert!(
        first["max_tokens"] == 400 || first["max_completion_tokens"] == 400,
        "{first}"
    );
    let moderated = asked(&server, 3);
    let system = moderated["messages"][0]["content"].as_str().expect("是字");
    assert!(
        system.contains(&judge_text("moderation-only.txt")),
        "{system}"
    );
    let log = run_log(&home.root);
    assert!(
        !log.contains("they asked her") && !log.contains("在吗") && !log.contains("abusive"),
        "理由、原文不进运行日志：{log}"
    );
    assert_eq!(script.requests().len(), 3, "判官不说回的不开回合");
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

#[tokio::test]
async fn answers_that_cannot_be_read_or_come_too_late_are_not_replied() {
    let mut server = judge(vec![
        says("我觉得应该回"),
        says("还是应该回"),
        verdict(json!({"relevance": 9, "should_reply": true})),
        yes("second try"),
        slow(Duration::from_secs(3), yes("too late")),
    ])
    .await;
    let script = Script::new([Play::Says("来了。"), Play::Says("嗯。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (&rules(), ""), "", MEMBERS).await;
    // 1：两次都读不出：判不了，照不回算。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    let events = until_decided(&home, GROUP, 1).await;
    let unread = decided(&events, 1).expect("记了判断");
    assert_eq!(
        unread["judge"],
        json!({"mode": "reply", "tries": 2, "millis": unread["judge"]["millis"], "model": JUDGE_MODEL,
            "unjudged": "unreadable", "detail": "no_object"}),
        "{unread}"
    );
    assert!(unread.get("score").is_none(), "{unread}");
    assert_eq!(unread["outcome"], "record");
    // 2：头一次少了几维，再问一次读出来了，回。
    napcat.send(group_frame(
        GROUP,
        JIE,
        2,
        json!([at(BOT), plain(" 你好")]),
        ("阿杰", "jie"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "来了。");
    let events = until_decided(&home, GROUP, 2).await;
    let retried = decided(&events, 2).expect("记了判断");
    assert_eq!(retried["judge"]["tries"], 2, "{retried}");
    assert_eq!(retried["judge"]["answer"]["reason"], "second try");
    assert_eq!(retried["outcome"], "reply");
    // 3：判官一秒内没回：等不到，不重试，照不回算。
    napcat.send(group_frame(
        SLOW,
        LIN,
        3,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    let slow = until_decided(&home, SLOW, 3).await;
    let late = decided(&slow, 3).expect("记了判断");
    assert_eq!(
        (
            &late["judge"]["tries"],
            &late["judge"]["unjudged"],
            &late["outcome"]
        ),
        (&json!(1), &json!("timeout"), &json!("record")),
        "{late}"
    );
    assert!(
        late["judge"].get("model").is_none(),
        "没回的不知道是哪个模型：{late}"
    );
    assert!(
        late["judge"]["millis"]
            .as_u64()
            .is_some_and(|millis| millis >= 1000),
        "{late}"
    );
    // 晚到的回答丢掉：等假服务器把它回完，主人再叫她一次，这一轮只有主人的，判断只多主人那一笔。
    server.wait_closed(5).await;
    napcat.send(group_frame(
        SLOW,
        OWNER,
        4,
        json!([at(BOT), plain(" 在吗")]),
        ("主人", "o"),
    ));
    assert_eq!(napcat.group_reply(SLOW).await, "嗯。");
    let slow = until_decided(&home, SLOW, 4).await;
    let started = of_kind(&slow, "turn.started");
    assert_eq!(started.len(), 1, "{slow:#?}");
    assert_eq!(started[0]["cause"], cause(4, "respond"));
    assert_eq!(of_kind(&slow, "ext.onebot.chat.decided").len(), 2);
    let log = run_log(&home.root);
    assert!(log.contains("not judged"), "{log}");
    assert!(log.contains("why=timeout"), "{log}");
    assert_eq!(script.requests().len(), 2);
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

#[tokio::test]
async fn other_messages_are_recorded_while_the_judge_thinks() {
    // 判官一次几十秒：这段时间别人说的照样记进、判完（问判官的任务另走一路，核心的 `model.call` 在后台答，施工 8-20 补）。
    let server = judge(vec![slow(Duration::from_secs(30), yes("slow"))]).await;
    let script = Script::new([Play::Says("在。")]);
    let (home, napcat, _) = started_judged(&script, &server, (&rules(), ""), "", MEMBERS).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    let deadline = tokio::time::Instant::now() + WAIT;
    while server.received().is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "判官等不到请求");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    napcat.send(group_frame(
        GROUP,
        JIE,
        2,
        json!([plain("路过")]),
        ("阿杰", "jie"),
    ));
    // 比判官回得早得多：判官这一次排在前面的话，要 30 秒以后才轮得到它。
    let within = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let events = venue_events(&home.root, &venue(GROUP));
        if let Some(body) = decided(&events, 2) {
            assert_eq!(body["outcome"], "record", "{body}");
            assert!(decided(&events, 1).is_none(), "判官还没回");
            break;
        }
        assert!(
            tokio::time::Instant::now() < within,
            "判官在判的时候别人的话记不进：{events:#?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    stopped(home).await;
}
