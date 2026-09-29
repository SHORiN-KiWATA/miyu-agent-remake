//! `miyu compact`（施工 6-8，`docs/blueprint/cli/compact.md`）：在进程里起一个核心，她用 `miyu ask` 说过一轮，`miyu compact`
//! 压上一次 `miyu ask` 开的那个会话，附的要求记进压缩，印压好了那一行，退出码 0；刚压过的再压，照核心的话说没有能压的，
//! 退出码 1；一个会话都没有的，说清楚，退出码 1；`--session` 压的是指定的那个；按 Ctrl+C 打断，压缩期间别的头说的那句
//! 接着发。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use miyu_cli::language::Language;
use miyu_cli::{CompactPlan, Plan, Target};
use miyu_kernel::event::{Body, CompactTrigger, ContextCompacted};
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use support::{Home, plan, within};

/// 出厂的尾巴是 16000 token：说得短的全在尾巴里，没有能压的。说七万个字，一组就超了尾巴，压得掉它。
fn long() -> String {
    "x".repeat(70_000)
}

/// 起一个核心：窗口大到不会自动压，请求模型照 `plays`。
fn home(plays: impl IntoIterator<Item = Play>) -> Home {
    Home::new(Arc::new(Script::new(plays).window(1_000_000)))
}

/// 一句答，一段摘要。
fn one_turn_and_a_summary() -> Vec<Play> {
    vec![
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said a lot.\n</summary>"),
    ]
}

/// 压 `session`（没有的是上一次 `miyu ask` 开的），附上 `instructions`，中文。
fn compacting(session: Option<&SessionId>, instructions: Option<&str>) -> CompactPlan {
    CompactPlan {
        session: session.map(|session| session.as_str().to_string()),
        instructions: instructions.map(str::to_string),
        language: Language::Chinese,
    }
}

/// 会话日志里的压缩，照先后。
fn compactions(home: &Home, session: &SessionId) -> Vec<ContextCompacted> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_last_ask_is_compacted_with_what_was_asked() {
    let home = home(one_turn_and_a_summary());
    let asked = home.ask(&plan(&long())).await;
    assert_eq!(asked.code, 0, "{}", asked.err);
    let compacted = home.compact(&compacting(None, Some("keep the plan"))).await;
    assert_eq!(compacted.code, 0, "{}", compacted.err);
    assert_eq!(compacted.out, "", "标准输出上什么都不印");
    assert!(
        compacted.err.starts_with("· 上下文已压缩："),
        "{}",
        compacted.err
    );
    let session = home.sessions()[0].clone();
    let done = compactions(&home, &session);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].trigger, Some(CompactTrigger::Manual));
    assert_eq!(done[0].instructions.as_deref(), Some("keep the plan"));
    // 刚压过，没有新内容：照核心的话说，退出码 1。
    let again = home.compact(&compacting(None, None)).await;
    assert_eq!(again.code, 1);
    assert_eq!(
        again.err,
        "没有能压的：还没压过的内容都在原样留着的最近一段里。\n"
    );
}

#[tokio::test]
async fn without_a_session_it_says_so() {
    let home = home([]);
    let compacted = home.compact(&compacting(None, None)).await;
    assert_eq!(compacted.code, 1);
    assert!(
        compacted.err.contains("还没有 miyu ask 开过的会话"),
        "{}",
        compacted.err
    );
}

/// `--session` 压的是指定的那个会话，不是最新的那个一次性会话。
#[tokio::test]
async fn a_given_session_is_the_one_compacted() {
    let mut plays = one_turn_and_a_summary();
    plays.insert(1, Play::Says("嗯。"));
    let home = home(plays);
    assert_eq!(home.ask(&plan(&long())).await.code, 0);
    assert_eq!(home.ask(&plan(&long())).await.code, 0);
    let sessions = home.sessions();
    let (newest, first) = (sessions[0].clone(), sessions[1].clone());
    let compacted = home.compact(&compacting(Some(&first), None)).await;
    assert_eq!(compacted.code, 0, "{}", compacted.err);
    assert_eq!(compactions(&home, &first).len(), 1);
    assert!(compactions(&home, &newest).is_empty());
}

/// 按 Ctrl+C 打断压缩（退出码 3）：压缩期间另一个终端里 `miyu ask -c` 说的那句排着，接着发（`queued: "send"`），那边
/// 等得到自己那一轮；退回了，那边就一直等下去。
#[tokio::test]
async fn interrupting_lets_what_was_said_meanwhile_go_on() {
    let script = Arc::new(
        Script::new([Play::Says("好。"), Play::Holds, Play::Says("嗯。")]).window(1_000_000),
    );
    let home = Home::new(script.clone());
    assert_eq!(home.ask(&plan(&long())).await.code, 0);
    let (press, presses) = mpsc::channel(1);
    let asked_to_compact = compacting(None, None);
    let compacting = home.compact_with(&asked_to_compact, presses);
    let asking = async {
        within("摘要请求停住", async {
            while script.requests().len() < 2 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        home.ask(&Plan {
            target: Target::Continue,
            ..plan("next")
        })
        .await
    };
    let pressing = async {
        within("那句排上了", async {
            loop {
                let sessions = home.sessions();
                let queued = sessions.first().is_some_and(|session| {
                    home.log(session).iter().any(|event| {
                        matches!(event.body, Body::MessageUser(_)) && event.turn.is_some()
                    })
                });
                if queued {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        press.send(()).await.expect("还在等 Ctrl+C");
    };
    let (compacted, asked, ()) = tokio::join!(compacting, asking, pressing);
    assert_eq!(compacted.code, 3, "{}", compacted.err);
    assert_eq!(asked.code, 0, "{}", asked.err);
    assert_eq!(asked.out, "嗯。\n");
    let session = home.sessions()[0].clone();
    assert!(compactions(&home, &session).is_empty(), "打断了，不写压缩");
}
