//! `miyu recap`（施工 3-8 四补，`docs/blueprint/cli/recap.md`）：在进程里起一个核心，她用 `miyu ask` 答过一轮，`miyu recap`
//! 回顾上一次 `miyu ask` 开的那个会话，标准输出上只有那一句，退出码 0；中间没有新内容再要一次，还是那一句、不请求；`--session`
//! 回顾的是指定的那个；没写成的照核心的话说，退出码 1；一个会话都没有的，说清楚，退出码 1。

mod support;

use std::sync::Arc;

use miyu_cli::language::Language;
use miyu_cli::{RecapPlan, recap_on};
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use support::{Asked, Home, Tape, plan, within};

/// 回顾 `session`（没有的是上一次 `miyu ask` 开的），界面是 `language`。
fn recapping(session: Option<&SessionId>, language: Language) -> RecapPlan {
    RecapPlan {
        session: session.map(|session| session.as_str().to_string()),
        language,
    }
}

/// 在真的套接字上连上核心，照 `plan` 要一句回顾。
async fn recap(home: &Home, plan: &RecapPlan) -> Asked {
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let code = within(
        "回顾完",
        recap_on(connection, &token, plan, &mut out, &mut err),
    )
    .await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

#[tokio::test]
async fn the_last_ask_is_recapped_on_standard_output() {
    let script = Arc::new(Script::new([Play::Says("好。"), Play::Says("在打招呼。")]));
    let home = Home::new(script.clone());
    assert_eq!(home.ask(&plan("hi")).await.code, 0);
    let recapped = recap(&home, &recapping(None, Language::Chinese)).await;
    assert_eq!(recapped.code, 0, "{}", recapped.err);
    assert_eq!(recapped.out, "在打招呼。\n", "标准输出上只有那一句");
    assert_eq!(recapped.err, "");
    let again = recap(&home, &recapping(None, Language::English)).await;
    assert_eq!((again.code, again.out.as_str()), (0, "在打招呼。\n"));
    assert_eq!(script.requests().len(), 2, "中间没有新内容，不再请求");
}

/// `--session` 回顾的是指定的那个会话，不是最新的那个一次性会话。
#[tokio::test]
async fn a_given_session_is_the_one_recapped() {
    let script = Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("说了一。"),
    ]));
    let home = Home::new(script.clone());
    assert_eq!(home.ask(&plan("说一")).await.code, 0);
    assert_eq!(home.ask(&plan("说二")).await.code, 0);
    let first = home.sessions()[1].clone();
    let recapped = recap(&home, &recapping(Some(&first), Language::Chinese)).await;
    assert_eq!(recapped.out, "说了一。\n", "{}", recapped.err);
    let requests = script.requests();
    let (_, request) = requests.last().expect("请求过");
    assert!(
        format!("{request:?}").contains("说一"),
        "回顾的是第一个会话"
    );
}

#[tokio::test]
async fn a_failed_recap_or_no_session_says_so() {
    let script = Arc::new(Script::new([
        Play::Says("好。"),
        Play::Fails {
            class: ErrorClass::Unclassified,
            wait_ms: None,
        },
    ]));
    let home = Home::new(script);
    let recapped = recap(&home, &recapping(None, Language::Chinese)).await;
    assert_eq!(recapped.code, 1);
    assert!(
        recapped.err.contains("还没有 miyu ask 开过的会话"),
        "{}",
        recapped.err
    );
    assert_eq!(home.ask(&plan("hi")).await.code, 0);
    let recapped = recap(&home, &recapping(None, Language::Chinese)).await;
    assert_eq!(recapped.code, 1);
    assert_eq!(recapped.out, "");
    assert_eq!(recapped.err, "回顾没写成：请求模型出错了。\n");
}
