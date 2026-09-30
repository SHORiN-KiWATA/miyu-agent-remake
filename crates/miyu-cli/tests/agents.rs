//! `miyu ask` 等子代理（施工 7-9，`docs/blueprint/cli/ask.md`「等子代理」）：在进程里起一个核心，工具是出厂的，替身模型在
//! 主会话、子会话里各答各的。等子代理都回报过、被回报叫醒的几轮也结束了才退出，这期间每一轮都印；会话有头跟着时回报
//! 叫醒她，头走了只记下。

mod support;

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_cli::{Format, Plan};
use miyu_kernel::event::Body;
use miyu_session::testkit::Play;
use support::router::{Gate, Router};
use support::{Asked, Home, Tape, ask_onto, plan, resources, within};

/// 起一个核心：工具是出厂的，请求模型照 `router`。
fn home(router: Router) -> Home {
    let tools = miyu_core::tools(&resources()).expect("出厂的资源读得出来");
    Home::with_tools(std::sync::Arc::new(router), tools)
}

/// 派一个子代理：标题和交代都是 `task`。
fn agent(task: &str) -> Play {
    let args = json!({"description": task, "prompt": task}).to_string();
    Play::calls(&[("agent", &args)])
}

/// 日志里有几轮开了头。
fn turns_started(home: &Home, session: &miyu_kernel::id::SessionId) -> usize {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count()
}

/// 主会话派两个子代理，它们先后回报；替身模型交回主会话和两道闸。
fn two_agents() -> (Router, Gate, Gate) {
    let mut router = Router::new(
        "分头查一下 A 和 B",
        [
            agent("查 A"),
            agent("查 B"),
            Play::Says("派出去了，等它们回来。"),
            Play::Says("A 查完了。"),
            Play::Says("B 也查完了，都齐了。"),
        ],
    );
    let a = router.gated("查 A", [Play::Says("A 的结果。")]);
    let b = router.gated("查 B", [Play::Says("B 的结果。")]);
    (router, a, b)
}

/// 照 `format` 说那一句，第一轮结束了放 A 回报，叫醒的那一轮结束了再放 B：先后回报，印的先后一定。
async fn ask_two(format: Format) -> Asked {
    let (router, a, b) = two_agents();
    let home = home(router);
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        format,
        ..plan("分头查一下 A 和 B")
    };
    let asking = ask_onto(&home.root, &plan, presses, Tape::default());
    let releasing = async {
        let main = home.oneshot().await;
        home.until_ended(&main, 1).await;
        a.open();
        home.until_ended(&main, 2).await;
        b.open();
    };
    let (asked, ()) = tokio::join!(asking, releasing);
    asked
}

#[tokio::test]
async fn it_waits_for_both_reports_and_prints_every_turn_like_the_sample() {
    let Asked {
        code,
        out,
        err,
        screen,
    } = ask_two(Format::Text).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "派出去了，等它们回来。\nA 查完了。\nB 也查完了，都齐了。\n"
    );
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/cli/ask-agents-text.txt");
    let drawn = std::fs::read_to_string(&sample).expect("有样本");
    assert_eq!(screen, drawn, "和样本逐字节一样");
}

#[tokio::test]
async fn json_lists_every_turn_in_order() {
    let Asked { code, out, err, .. } = ask_two(Format::Json).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "", "给脚本的标准错误上什么都不印");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    let usage = |n: u64| json!({"input": 100 * n, "cache_read": 40 * n, "cache_write": 0, "output": 10 * n});
    assert_eq!(
        printed["turns"],
        json!([
            {"text": "派出去了，等它们回来。", "usage": usage(3)},
            {"text": "A 查完了。", "usage": usage(1)},
            {"text": "B 也查完了，都齐了。", "usage": usage(1)},
        ]),
        "第一轮三次请求，叫醒的两轮各一次"
    );
}

#[tokio::test]
async fn a_subagent_messaged_after_it_reported_is_waited_for_again() {
    // A 回报里问了一句，她留言答它：A 又欠一份回报，等它再报、叫醒她的那一轮也结束了才退出（施工 7-7 的 `job.messaged`）。
    let answer = json!({"to": "j1", "message": "要加"}).to_string();
    let mut router = Router::new(
        "派 A 去查",
        [
            agent("查 A"),
            Play::Says("派出去了。"),
            Play::calls(&[("message_agent", &answer)]),
            Play::Says("答它了。"),
            Play::Says("A 加好了。"),
        ],
    );
    let a = router.gated(
        "查 A",
        [Play::Says("要不要加测试？"), Play::Says("加好了。")],
    );
    let home = home(router);
    let (_press, presses) = mpsc::channel(1);
    let plan = plan("派 A 去查");
    let asking = ask_onto(&home.root, &plan, presses, Tape::default());
    let releasing = async {
        let main = home.oneshot().await;
        home.until_ended(&main, 1).await;
        a.open();
        home.until_ended(&main, 2).await;
        a.open();
    };
    let (
        Asked {
            code, err, screen, ..
        },
        (),
    ) = tokio::join!(asking, releasing);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        screen,
        "↗ 派子代理 查 A · 派出去了：j1\n\n派出去了。\n· 等 1 个子代理回报…（按 Ctrl+C 不等了）\n· j1「查 A」报回来了\n↗ 留言 j1 · 送到了：j1\n\n答它了。\n· j1「查 A」报回来了\n\nA 加好了。\n· 输入 500 · 命中缓存 200（40%）· 输出 50\n",
        "答了它以后还在等，它再报、叫醒的那一轮也印了"
    );
}

#[tokio::test]
async fn a_background_command_is_not_waited_for_and_says_it_went_to_the_background() {
    let work = support::Outside::new();
    let args = json!({"command": "sleep 2", "description": "睡一会", "run_in_background": true});
    let router = Router::new(
        "后台睡一会",
        [
            Play::calls(&[("shell", &args.to_string())]),
            Play::Says("放到后台了。"),
        ],
    );
    let home = home(router);
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        cwd: work.text(),
        ..plan("后台睡一会")
    };
    let Asked {
        code, err, screen, ..
    } = ask_onto(&home.root, &plan, presses, Tape::default()).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        screen,
        "$ sleep 2 · 放到后台了：j1\n\n放到后台了。\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n",
        "不印给模型看的英文回执；不等它"
    );
    let main = home.oneshot().await;
    let ended = home
        .log(&main)
        .iter()
        .any(|event| matches!(event.body, Body::JobReported(_)));
    assert!(!ended, "退出时后台命令还在跑");
    home.until_disconnected().await;
    // 头走了：它结束了只记下，不叫醒她。
    within("后台命令结束", async {
        while !home
            .log(&main)
            .iter()
            .any(|event| matches!(event.body, Body::JobReported(_)))
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(turns_started(&home, &main), 1, "没人看着，只记下、不开轮");
}

#[tokio::test]
async fn ctrl_c_while_waiting_leaves_and_the_late_report_is_only_recorded() {
    let mut router = Router::new("派 A 去查", [agent("查 A"), Play::Says("派出去了。")]);
    let a = router.gated("查 A", [Play::Says("A 的结果。")]);
    let home = home(router);
    let (press, presses) = mpsc::channel(4);
    let tape = Tape::default();
    let plan = plan("派 A 去查");
    let asking = ask_onto(&home.root, &plan, presses, tape.clone());
    let pressing = async {
        within("印出等的那一行", async {
            while !tape.text(|_| true).contains("等 1 个子代理回报") {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        press.send(()).await.expect("还在等");
    };
    let (
        Asked {
            code, err, screen, ..
        },
        (),
    ) = tokio::join!(asking, pressing);
    assert_eq!(code, 3, "{err}");
    assert_eq!(
        screen,
        "↗ 派子代理 查 A · 派出去了：j1\n\n派出去了。\n· 等 1 个子代理回报…（按 Ctrl+C 不等了）\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n· 不等了，子代理还在后台跑，下次 miyu ask -c 时她会看到结果\n"
    );
    // 头走了才回报：只记下，不叫醒她。
    let main = home.oneshot().await;
    home.until_disconnected().await;
    a.open();
    within("回报记下", async {
        while !home
            .log(&main)
            .iter()
            .any(|event| matches!(event.body, Body::ChildReported(_)))
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(turns_started(&home, &main), 1, "没人看着，只记下、不开轮");
}

#[tokio::test]
async fn the_timeout_while_waiting_leaves_with_3() {
    let mut router = Router::new("派 A 去查", [agent("查 A"), Play::Says("派出去了。")]);
    let _never = router.gated("查 A", [Play::Says("A 的结果。")]);
    let home = home(router);
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        // 第一轮要在这之前说完：慢的机器上也够。
        timeout: Some(Duration::from_secs(3)),
        ..plan("派 A 去查")
    };
    let Asked {
        code, err, screen, ..
    } = ask_onto(&home.root, &plan, presses, Tape::default()).await;
    assert_eq!(code, 3, "{err}");
    assert_eq!(
        screen,
        "↗ 派子代理 查 A · 派出去了：j1\n\n派出去了。\n· 等 1 个子代理回报…（按 Ctrl+C 不等了）\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n· 等到时间了，没回报的子代理还在后台跑\n"
    );
}

#[tokio::test]
async fn the_timeout_interrupts_a_running_turn_and_leaves() {
    let home = home(Router::new("说个没完", [Play::Holds]));
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        timeout: Some(Duration::from_secs(1)),
        ..plan("说个没完")
    };
    let Asked {
        code, err, screen, ..
    } = ask_onto(&home.root, &plan, presses, Tape::default()).await;
    assert_eq!(code, 3, "{err}");
    // 替身开了个头（「…」）就停住；慢的机器上到时间时它可能还没开口，只看最后一行。
    assert!(
        screen.ends_with("· 等到时间了\n"),
        "没有子代理：只说到时间了：{screen}"
    );
    let main = home.oneshot().await;
    within("那一轮被打断", async {
        loop {
            let interrupted = home.log(&main).iter().any(|event| {
                matches!(&event.body, Body::TurnEnded(ended)
                    if ended.reason == miyu_kernel::event::EndReason::Interrupted)
            });
            if interrupted {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}
