//! 退信（施工 O-25 下，`onebot.md` 第一条「退信」；18 第十节、Q15）：真核心拉起真桥、假 NapCat，她照台词、剧本说。她的话没发
//! 出去（NapCat 拒了、排过期了），记了 `ext.onebot.venues.failed` 以后经核心的 `session.note` 给那个会话记一块
//! `context.injected`（`kind` 是 `undelivered`，命令编号是入队那一条的序号加 `/note`），写着为什么和那一段的开头：这一轮还在跑
//! 的，她下一次请求看到；不在跑的，下一轮开头看到。一段一块、只退一次；提示、回执失败了不退；私聊的也退。
//! 退信的模板是出厂数据：写坏了、要了别的字段的桥起不来，少要几个照收。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use miyu_chat::Source;
use miyu_config::problem::Code;
use miyu_onebot::rules::Factory;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;

use crate::rules::{clean, copied_resources};
use crate::support::answering::refused;
use crate::support::group::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 一小时一轮、不抽样的群：别人开过一轮以后再叫她，回一句提示。
const GROUP: i64 = 777;

/// 不抽样的群：她被禁言着，排过期。
const QUIET: i64 = 778;

/// 群里的别人。
const LIN: i64 = 20002;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群都不抽样，[`GROUP`] 一小时一轮。
fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{GROUP}] }}\nrate = \"1/1h\"\n"
    )
}

/// 入队、失败记成的事件。
const QUEUED: &str = "ext.onebot.venues.queued";
const FAILED: &str = "ext.onebot.venues.failed";

/// 退信的几块：`context.injected` 里 `kind` 是 `undelivered` 的（核心自己也记事实：时间、权限这些），照先后。
fn undelivered(events: &[Value]) -> Vec<Value> {
    of_kind(events, "context.injected")
        .into_iter()
        .filter(|one| one["body"]["kind"] == "undelivered")
        .collect()
}

/// 等到群 `group` 里有 `n` 块退信：交回那时的全部事件。
async fn until_notes(home: &Home, group: i64, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        undelivered(events).len() == n
    })
    .await
}

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 等到群 `group` 里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, group: i64, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

/// 等到群 `group` 里第 `message` 条消息的判断记下了。
async fn until_decided(home: &Home, group: i64, message: i64) {
    let cause = format!("qq:{BOT}:{message}:{TIME}/decided");
    until_event(&home.root, &venue(group), |event| event["cause"] == cause).await;
}

/// 出厂的模板照 `why`、`detail`、`text` 填出来的那一块（字段里没有要转义的字）。
fn fact(why: &str, detail: &str, text: &str) -> String {
    let path = resources().join("software/onebot/facts/undelivered.txt");
    std::fs::read_to_string(path)
        .expect("读得出")
        .replace("{why}", why)
        .replace("{detail}", detail)
        .replace("{text}", text)
}

/// 退信的几块：照先后，每块是（命令编号、`body`、回合）。
fn notes(events: &[Value]) -> Vec<(Value, Value, Value)> {
    undelivered(events)
        .iter()
        .map(|one| {
            (
                one["cause"].clone(),
                one["body"].clone(),
                one["turn"].clone(),
            )
        })
        .collect()
}

/// 入队了的她的话（`kind` 是 `reply`）的序号加 `/note`，照先后。
fn note_ids(events: &[Value]) -> Vec<Value> {
    of_kind(events, QUEUED)
        .iter()
        .filter(|one| one["body"]["kind"] == "reply")
        .map(|one| json!(format!("{}/note", one["seq"])))
        .collect()
}

#[tokio::test]
async fn her_refused_words_come_back_to_her_in_the_same_turn() {
    let (release, held) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在。"),
        Line::calls("我看看。").released_by(held),
        Line::says("好。"),
    ]);
    let models = Arc::new(lines.clone());
    let (home, napcat, _) = started_tuned(models, &rules(), &Value::Null, |napcat| {
        napcat.refusing(MEMBERS)
    })
    .await;
    // 小林说一句，只记下；测试照判官点了头的样子开一轮（别人开的，一小时一轮满了）。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([plain("大家好")]),
        ("小林", "lin"),
    ));
    let events = until_count(&home, GROUP, "message.user", 1).await;
    respond(
        &home,
        &venue(GROUP),
        "refused-1",
        &[said(&events)[0]["seq"].clone()],
    )
    .await;
    // 「在。」被拒了：这一轮还在跑（第二句压着），退信带这一轮的回合编号。
    let events = until_notes(&home, GROUP, 1).await;
    let turn = of_kind(&events, "turn.started")[0]["seq"].clone();
    let detail: String = refused().trim().chars().take(200).collect();
    assert_eq!(
        notes(&events),
        [(
            note_ids(&events)[0].clone(),
            json!({"kind": "undelivered", "text": fact("rejected", &detail, "在。")}),
            turn.clone(),
        )]
    );
    release.send(()).expect("还在等");
    until_count(&home, GROUP, "turn.ended", 1).await;
    let events = until_events(&home.root, &venue(GROUP), |events| {
        undelivered(events).len() == 3 && of_kind(events, FAILED).len() == 3
    })
    .await;
    let texts: Vec<Value> = notes(&events)
        .into_iter()
        .map(|(_, body, _)| body["text"].clone())
        .collect();
    assert_eq!(
        texts,
        [
            fact("rejected", &detail, "在。"),
            fact("rejected", &detail, "我看看。"),
            fact("rejected", &detail, "好。"),
        ],
        "一段一块"
    );
    let ids: Vec<Value> = notes(&events).into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(ids, note_ids(&events), "命令编号是入队那一条的序号加 /note");
    // 她这一轮的下一次请求看得到第一块：第二句放行以后才请求的。
    let requests = lines.requests();
    assert_eq!(requests.len(), 3, "{events:#?}");
    assert!(!format!("{:?}", requests[0]).contains("<undelivered"));
    assert!(
        format!("{:?}", requests[2]).contains("<undelivered"),
        "{:?}",
        requests[2]
    );
    // 别人再叫她：限流的提示被拒了，不退；终端管理员的 `/stop` 回执被拒了，不退。
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    until_count(&home, GROUP, FAILED, 4).await;
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        3,
        json!([plain("/stop")]),
        ("终端管理员", "o"),
    ));
    let events = until_count(&home, GROUP, FAILED, 5).await;
    let kinds: Vec<Value> = of_kind(&events, QUEUED)
        .iter()
        .map(|one| one["body"]["kind"].clone())
        .collect();
    assert_eq!(kinds, ["reply", "reply", "reply", "notice", "receipt"]);
    // 记了失败的那一步办完了，桥才办下一条：判下一条以前，要退的早就退了。
    napcat.send(group_frame(
        GROUP,
        LIN,
        4,
        json!([plain("好吧")]),
        ("小林", "lin"),
    ));
    until_decided(&home, GROUP, 4).await;
    let events = venue_events(&home.root, &venue(GROUP));
    assert_eq!(undelivered(&events).len(), 3, "提示、回执不退：{events:#?}");
    let log = run_log(&home.root);
    assert_eq!(log.matches("undelivered noted").count(), 3, "{log}");
    assert!(!log.contains("在。"), "正文不进运行日志：{log}");
    stopped(home).await;
}

#[tokio::test]
async fn expired_words_come_back_at_the_next_turn_once() {
    let long = "大家说得都有道理。那就这么定了：周六上午十点在东门集合，别迟到。";
    let script = Script::new([Play::Says(long), Play::Says("嗯")]);
    let tuned = json!({"queue_expire_seconds": 1});
    let (home, mut napcat, _) =
        started_tuned(Arc::new(script.clone()), &rules(), &tuned, |napcat| {
            napcat.answering(MEMBERS)
        })
        .await;
    // 禁言着，主线开的一轮她的话排着，过了期限记 `expired`：这一轮早完了，退信不带回合编号。
    napcat.send(group_ban(QUIET, BOT, "ban", Some(600)));
    until_count(&home, QUIET, "ext.onebot.venues.muted", 1).await;
    napcat.send(group_frame(
        QUIET,
        ADMIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("终端管理员", "o"),
    ));
    let events = until_count(&home, QUIET, "message.user", 1).await;
    respond(
        &home,
        &venue(QUIET),
        "expired-1",
        &[said(&events)[0]["seq"].clone()],
    )
    .await;
    let events = until_notes(&home, QUIET, 1).await;
    let head: String = long.chars().take(30).collect();
    assert_eq!(head.chars().count(), 30);
    assert_eq!(
        notes(&events),
        [(
            note_ids(&events)[0].clone(),
            json!({"kind": "undelivered", "text": fact("expired", "", &head)}),
            Value::Null,
        )],
        "只带头 30 个字符"
    );
    // 解禁以后终端管理员再叫她：下一轮开头看到，排在触发前面。
    napcat.send(group_ban(QUIET, BOT, "lift_ban", None));
    napcat.send(group_frame(
        QUIET,
        ADMIN,
        2,
        json!([at(BOT), plain(" 还有别的吗")]),
        ("终端管理员", "o"),
    ));
    let words = napcat.group_message(QUIET).await;
    assert_eq!(words.last().expect("有段")["data"]["text"], "嗯");
    let events = until_count(&home, QUIET, "venue.delivered", 1).await;
    let request = format!("{:?}", script.requests()[1].1);
    let (Some(fact), Some(trigger)) = (request.find("<undelivered"), request.find("还有别的吗"))
    else {
        panic!("请求里两样都有：{request}");
    };
    assert!(fact < trigger, "排在触发前面：{request}");
    assert_eq!(request.matches("<undelivered").count(), 1, "{request}");
    assert_eq!(undelivered(&events).len(), 1, "同一段只退一次");
    stopped(home).await;
}

#[tokio::test]
async fn private_words_come_back_too() {
    let script = Script::new([Play::Says("在。")]);
    let (home, mut napcat, _) = started_tuned(Arc::new(script), "", &Value::Null, |napcat| {
        napcat.refusing(MEMBERS)
    })
    .await;
    napcat.send(private_frame(ADMIN, 51, json!([plain("在吗")])));
    let sent = napcat.action().await;
    assert_eq!(sent["action"], "send_private_msg", "{sent}");
    let deadline = tokio::time::Instant::now() + WAIT;
    let noted = loop {
        let found: Vec<Value> = home
            .sessions()
            .iter()
            .flat_map(|session| undelivered(&home.events(session)))
            .collect();
        if !found.is_empty() {
            break found;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到退信");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    let detail: String = refused().trim().chars().take(200).collect();
    assert_eq!(noted.len(), 1);
    assert_eq!(
        noted[0]["body"],
        json!({"kind": "undelivered", "text": fact("rejected", &detail, "在。")})
    );
    stopped(home).await;
}

#[test]
fn a_broken_undelivered_template_is_a_factory_mistake() {
    // 退信的模板（施工 O-25 下）：写坏了、要了别的字段，记一条 `bad_format`；不在的记一条读不成。文件名带 `facts/`。
    let resources = copied_resources();
    let facts = resources.join("software/onebot/facts");
    for (text, wanted) in [
        ("<undelivered why=\"{why}\">{text\n", "{text"),
        ("<undelivered who=\"{who}\">{text}</undelivered>\n", "who"),
    ] {
        std::fs::write(facts.join("undelivered.txt"), text).expect("写得进");
        let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("模板不对");
        assert_eq!(
            problems
                .iter()
                .map(|problem| (problem.code, problem.source, problem.file.as_str()))
                .collect::<Vec<_>>(),
            [(Code::BadFormat, Source::Factory, "facts/undelivered.txt")]
        );
        assert!(
            problems[0]
                .why
                .as_deref()
                .is_some_and(|why| why.contains(wanted)),
            "{problems:?}"
        );
    }
    std::fs::remove_file(facts.join("undelivered.txt")).expect("删得了");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("不在");
    assert_eq!(
        problems
            .iter()
            .map(|problem| (problem.code, problem.file.as_str()))
            .collect::<Vec<_>>(),
        [(Code::Unreadable, "facts/undelivered.txt")]
    );
    // 少一个字段照样收：只要她认得出是哪一句、为什么。
    std::fs::write(
        facts.join("undelivered.txt"),
        "<undelivered why=\"{why}\">{text}</undelivered>\n",
    )
    .expect("写得进");
    assert!(Factory::load(&ResourceRoot::at(&resources)).is_ok());
    clean(resources.parent().expect("有上一级"));
}
