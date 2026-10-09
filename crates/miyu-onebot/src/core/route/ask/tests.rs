//! 问一次判官（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 12、13 条）：核心那一头是测试，照编号回。先拿群聊记录、
//! 再调 `model.call`，参数照图纸；记录的条数、模型照参数；读不出、核心拒了、等不到的再问，交回最后一次的
//! 为什么；打分、只查违规各照各的超时；名额满了排队，等不到的不问；记录被拒的不再问；核心断开了交回空的。钟停住，照停住的钟算。
//! 判官带的人格在 `persona_tests.rs`（施工 O-23 补），夹具在这里。

use std::sync::Arc;
use std::time::Duration;

use miyu_chat::{File, JudgeSources, JudgeTexts, Mode, Params, Source, Unreadable};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader, DuplexStream};
use tokio::task::JoinHandle;

use super::super::persona::Personas;
use super::{Answer, Asking, Slots, Unjudged, ask};
use crate::core::caller::{Waiting, Writer};
use crate::core::{Caller, Gone};

/// 编号的前缀。
const PREFIX: &str = "onebot-t-side-";

/// 一份读得出的回答：理由 `reason`。
pub(super) fn verdict(reason: &str) -> String {
    json!({"relevance": 5, "willingness": 5, "social": 5, "timing": 5, "continuity": 5,
        "should_reply": true, "to_bot": false, "severity": 0, "reason": reason})
    .to_string()
}

/// 出厂参数。
fn params() -> Params {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/software/onebot/defaults.toml"
    );
    let file = File {
        source: Source::Factory,
        name: "defaults.toml".to_string(),
        text: std::fs::read_to_string(path).expect("出厂的读得到"),
    };
    Params::read(&file).expect("出厂的读得出")
}

/// 判官的说明：每一份一个记号，看得出拼进去了哪几份。
fn texts() -> Arc<JudgeTexts> {
    let tag = |name: &str| format!("<{name}>\n");
    Arc::new(
        JudgeTexts::new(JudgeSources {
            system: tag("system"),
            persona_open: tag("persona"),
            persona_close: tag("/persona"),
            reply: tag("reply"),
            moderation_only: tag("moderation-only"),
            violations: "<violations {severity_min}>\n".to_string(),
            answer: tag("answer"),
            records_open: tag("records"),
            records_close: tag("/records"),
            current_open: tag("current"),
            current_close: tag("/current"),
            decoded_open: tag("decoded"),
            decoded_close: tag("/decoded"),
        })
        .expect("造得出"),
    )
}

/// 测试当的核心：读桥发来的请求，照编号回。
pub(super) struct Fake {
    pub(super) waiting: Waiting,
    lines: BufReader<DuplexStream>,
}

impl Fake {
    /// 下一条请求。钟停着，一个小时还没来的（问的一方不再发了）是测试错了，当场说，不卡住。
    pub(super) async fn next(&mut self) -> Value {
        let mut line = String::new();
        let read = tokio::time::timeout(Duration::from_secs(3600), self.lines.read_line(&mut line));
        read.await.expect("等得到请求").expect("读得到");
        serde_json::from_str(&line).expect("是 JSON")
    }

    /// 回请求 `request`：接受，`result`。
    fn answer(&self, request: &Value, result: Value) {
        let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
        assert_eq!(self.waiting.sort(reply), None, "有人等着");
    }

    /// 回请求 `request`：拒绝，原因码 `reason`。
    pub(super) fn refuse(&self, request: &Value, reason: &str) {
        let reply = json!({"jsonrpc": "2.0", "id": request["id"],
            "error": {"code": -32010, "message": "no", "data": {"reason": reason}}});
        assert_eq!(self.waiting.sort(reply), None, "有人等着");
    }

    /// 回 `venue.records`：记录 `records`、这一条 `current`。
    pub(super) async fn records(&mut self, records: &str, current: &str) -> Value {
        let request = self.next().await;
        assert_eq!(request["method"], "venue.records", "{request}");
        self.answer(&request, json!({"records": records, "current": current}));
        request
    }

    /// 回下一个 `persona.read`：原文 `text`（可以是 `null`）。
    pub(super) async fn persona(&mut self, text: Value) -> Value {
        let request = self.next().await;
        assert_eq!(request["method"], "persona.read", "{request}");
        self.answer(&request, json!({"text": text, "version": null}));
        request
    }

    /// 回下一个 `model.call`：判官说 `text`。
    pub(super) async fn says(&mut self, text: &str) -> Value {
        let request = self.next().await;
        assert_eq!(request["method"], "model.call", "{request}");
        self.answer(
            &request,
            json!({"text": text, "provider": "p", "model": "m", "usage": null}),
        );
        request
    }
}

/// 一个调用口和测试当的核心。
pub(super) fn connected() -> (Caller, Fake) {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let waiting = Waiting::new(PREFIX.to_string());
    let writer: Writer = Arc::new(tokio::sync::Mutex::new(Box::new(ours)));
    let fake = Fake {
        waiting: waiting.clone(),
        lines: BufReader::new(theirs),
    };
    (Caller::new(writer, waiting), fake)
}

/// 问判官要的：群会话 `s` 的第 12 条，打分，base64 解出来一段，判官的几项照出厂的再交 `change` 改。
pub(super) fn asking(change: impl FnOnce(&mut Asking)) -> Asking {
    let params = params();
    let mut asking = Asking {
        session: "s".to_string(),
        msg: 12,
        mode: Mode::Reply,
        decoded: Some("hidden words".to_string()),
        judge: params.judge,
        chatty: params.chatty,
        persona: None,
    };
    change(&mut asking);
    asking
}

/// 起一个问判官的任务：名额照 `slots`，人格的原文另记一份（[`kept`]）。
pub(super) fn asked(asking: Asking, caller: &Caller, slots: &Slots) -> JoinHandle<Option<Answer>> {
    asked_with(asking, caller, slots, &kept())
}

/// 同 [`asked`]，人格的原文照 `personas` 记：几个任务共用一份。
pub(super) fn asked_with(
    asking: Asking,
    caller: &Caller,
    slots: &Slots,
    personas: &Personas,
) -> JoinHandle<Option<Answer>> {
    tokio::spawn(ask(
        asking,
        caller.clone(),
        texts(),
        slots.clone(),
        personas.clone(),
    ))
}

/// 人格的原文记 60 秒（出厂的 `judge_persona_seconds`）。
pub(super) fn kept() -> Personas {
    Personas::new(Duration::from_secs(60))
}

/// 四个名额、排队等 15 秒（出厂的）。
pub(super) fn slots() -> Slots {
    Slots::new(4, Duration::from_secs(15))
}

#[tokio::test(start_paused = true)]
async fn the_records_come_first_then_the_call_shaped_as_drawn() {
    let (caller, mut core) = connected();
    let task = asked(asking(|_| {}), &caller, &slots());
    let records = core.records("R1\n", "C1\n").await;
    assert_eq!(
        records["params"],
        json!({"session": "s", "msg": 12, "count": 20})
    );
    let call = core
        .says(&format!("```json\n{}\n```", verdict("asked")))
        .await;
    let params = &call["params"];
    assert_eq!(params["purpose"], "judge");
    assert_eq!(params["max_tokens"], 400);
    assert!(params.get("model").is_none(), "没写模型的不带：{params}");
    let messages = params["messages"].as_array().expect("是列表");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    let system = messages[0]["text"].as_str().expect("是字");
    assert!(system.starts_with("<system>\n"), "{system}");
    assert!(
        system.contains("<reply>") && system.contains("<violations 7>"),
        "{system}"
    );
    assert!(!system.contains("<persona>"), "没给人格的不带：{system}");
    assert_eq!(messages[1]["role"], "user");
    let user = messages[1]["text"].as_str().expect("是字");
    for part in ["R1", "C1", "hidden words"] {
        assert!(user.contains(part), "{part}：{user}");
    }
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.tries, 1);
    assert_eq!(answer.millis, 0, "钟停着");
    assert_eq!(answer.model.as_deref(), Some("p/m"));
    let judgement = answer.result.expect("读得出");
    assert_eq!(judgement.reason, "asked");
    assert_eq!(judgement.scores, [5.0; 5]);
}

#[tokio::test(start_paused = true)]
async fn records_are_asked_as_many_as_set_and_the_model_if_written() {
    let (caller, mut core) = connected();
    // 条数照参数（1 到 100，群聊内核查过）；写了模型的带上；没解出 base64 的不夹那一段。
    let task = asked(
        asking(|asking| {
            asking.judge.records = 3;
            asking.judge.model = Some("@cheap".to_string());
            asking.decoded = None;
        }),
        &caller,
        &slots(),
    );
    let records = core.records("R1\n", "C1\n").await;
    assert_eq!(records["params"]["count"], 3);
    let call = core.says(&verdict("x")).await;
    assert_eq!(call["params"]["model"], "@cheap");
    let user = call["params"]["messages"][1]["text"]
        .as_str()
        .expect("是字");
    assert!(user.contains("R1") && user.contains("C1"), "{user}");
    assert!(!user.contains("<decoded>"), "没解出来的不夹：{user}");
    task.await.expect("没崩").expect("核心在");
}

#[tokio::test(start_paused = true)]
async fn what_cannot_be_judged_is_asked_again_and_the_last_why_kept() {
    let (caller, mut core) = connected();
    // 再问一次：头一次读不出，第二次核心拒了。
    let task = asked(asking(|asking| asking.judge.retries = 1), &caller, &slots());
    core.records("", "C\n").await;
    core.says("I think she should reply").await;
    let second = core.next().await;
    assert_eq!(second["method"], "model.call");
    core.refuse(&second, "cooling");
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.tries, 2);
    assert_eq!(answer.model.as_deref(), Some("p/m"), "回过的那一次的模型");
    assert_eq!(answer.result, Err(Unjudged::Refused("cooling".to_string())));
    // 不再问：读不出就是读不出，少了哪一维照实说。
    let task = asked(asking(|asking| asking.judge.retries = 0), &caller, &slots());
    core.records("", "C\n").await;
    core.says(r#"{"relevance": 5}"#).await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.tries, 1);
    assert_eq!(
        answer.result,
        Err(Unjudged::Unreadable(Unreadable::Dimension("willingness")))
    );
    // 再问两次：第三次读出来了。
    let task = asked(asking(|asking| asking.judge.retries = 2), &caller, &slots());
    core.records("", "C\n").await;
    let refused = core.next().await;
    core.refuse(&refused, "model_failed");
    core.says("nope").await;
    core.says(&verdict("third")).await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.tries, 3);
    assert_eq!(answer.result.expect("读得出").reason, "third");
    // 只查违规的没有 `severity`：读不出。
    let task = asked(
        asking(|asking| {
            asking.mode = Mode::ModerationOnly;
            asking.judge.retries = 0;
        }),
        &caller,
        &slots(),
    );
    core.records("", "C\n").await;
    let call = core
        .says(
            &json!({"relevance": 0, "willingness": 0, "social": 0, "timing": 0, "continuity": 0})
                .to_string(),
        )
        .await;
    let system = call["params"]["messages"][0]["text"]
        .as_str()
        .expect("是字");
    assert!(
        system.contains("<moderation-only>") && !system.contains("<reply>"),
        "{system}"
    );
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(
        answer.result,
        Err(Unjudged::Unreadable(Unreadable::NoSeverity))
    );
}

#[tokio::test(start_paused = true)]
async fn slow_answers_time_out_by_mode_and_are_asked_again() {
    let (caller, mut core) = connected();
    // 打分的一次最多等 60 秒（出厂的），不再问。
    let task = asked(asking(|asking| asking.judge.retries = 0), &caller, &slots());
    core.records("", "C\n").await;
    core.next().await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(
        (answer.tries, answer.millis, answer.model, answer.result),
        (1, 60_000, None, Err(Unjudged::Timeout))
    );
    // 只查违规的等 120 秒。
    let task = asked(
        asking(|asking| {
            asking.mode = Mode::ModerationOnly;
            asking.judge.retries = 0;
        }),
        &caller,
        &slots(),
    );
    core.records("", "C\n").await;
    core.next().await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(
        (answer.millis, answer.result),
        (120_000, Err(Unjudged::Timeout))
    );
    // 头一次等不到，再问一次回了：前一次晚来的回答没人收。
    let task = asked(asking(|asking| asking.judge.retries = 1), &caller, &slots());
    core.records("", "C\n").await;
    let late = core.next().await;
    core.says(&verdict("second")).await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!((answer.tries, answer.millis), (2, 60_000));
    assert_eq!(answer.result.expect("读得出").reason, "second");
    let reply = json!({"jsonrpc": "2.0", "id": late["id"], "result": {"text": verdict("late")}});
    assert_eq!(core.waiting.sort(reply), None, "晚来的丢掉");
}

#[tokio::test(start_paused = true)]
async fn a_full_house_queues_and_gives_up_after_the_wait() {
    let (caller, mut core) = connected();
    let one = Slots::new(1, Duration::from_secs(15));
    let held = Arc::clone(&one.slots)
        .try_acquire_owned()
        .expect("还有名额");
    // 等够 15 秒还没有名额：判不了，一次都没问。
    let task = asked(asking(|_| {}), &caller, &one);
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(
        (answer.tries, answer.millis, answer.result),
        (0, 15_000, Err(Unjudged::Queue))
    );
    let nothing = tokio::time::timeout(Duration::from_secs(1), core.next()).await;
    assert!(nothing.is_err(), "核心那一头什么都没收到");
    // 等到 10 秒有了名额：接着问，耗时从交出去算。
    let task = asked(asking(|_| {}), &caller, &one);
    tokio::time::sleep(Duration::from_secs(10)).await;
    drop(held);
    core.records("", "C\n").await;
    core.says(&verdict("waited")).await;
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.millis, 10_000);
    assert_eq!(answer.result.expect("读得出").reason, "waited");
}

#[tokio::test(start_paused = true)]
async fn refused_records_are_not_asked_again_and_a_gone_core_gives_nothing() {
    let (caller, mut core) = connected();
    let task = asked(asking(|asking| asking.judge.retries = 3), &caller, &slots());
    let records = core.next().await;
    core.refuse(&records, "bad_params");
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(
        (answer.tries, answer.result),
        (0, Err(Unjudged::Refused("bad_params".to_string())))
    );
    let nothing = tokio::time::timeout(Duration::from_secs(1), core.next()).await;
    assert!(nothing.is_err(), "不再问");
    // 核心断开了：交回空的。
    let task = asked(asking(|_| {}), &caller, &slots());
    core.records("", "C\n").await;
    core.next().await;
    core.waiting.close();
    assert_eq!(task.await.expect("没崩"), None);
    assert!(matches!(caller.call("x", json!({})).await, Err(Gone)));
}
