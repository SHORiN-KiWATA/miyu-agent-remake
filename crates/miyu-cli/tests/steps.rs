//! `miyu ask` 印出每一步（`docs/construction/4-5-miyu ask 印出每一步（下）.md`）：在进程里起一个核心，工具是真的
//! 三件读的；她读到了、没读成、被拒了，每一步在标准错误上印一行；工作目录太宽的，开头说一句。还有一个假的
//! 核心，照协议允许的最晚的先后说话。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use miyu_cli::{Format, Plan, Target};
use miyu_ipc::Listener;
use miyu_session::testkit::{Play, Script};
use support::{AccountIdOf, Asked, Bare, Home, Outside, ask_at, dirs, plan, resources};

/// 起一个核心：请求模型照 `plays`，工具是真的三件读的。
fn home(plays: impl IntoIterator<Item = Play>) -> Home {
    let tools = miyu_core::tools(&resources()).expect("出厂的资源读得出来");
    Home::with_tools(Arc::new(Script::new(plays)), tools)
}

/// 调一次 `read`，读 `file_path`。
fn read(file_path: &str) -> Play {
    Play::calls(&[("read", &json!({ "file_path": file_path }).to_string())])
}

#[tokio::test]
async fn each_step_is_one_line_before_the_answer() {
    let work = Outside::new();
    let notes = work.file("notes.md", "一\n二\n三\n");
    let elsewhere = Outside::new();
    let plan_md = elsewhere.file("plan.md", "别处\n");
    let home = home([
        read(&notes.to_string_lossy()),
        read("missing.md"),
        read(&plan_md.to_string_lossy()),
        Play::Says("读完了。"),
    ]);
    let asked = home
        .ask(&Plan {
            cwd: work.text(),
            ..plan("读一下")
        })
        .await;
    let Asked {
        code,
        out,
        err,
        screen,
    } = asked;
    assert_eq!(code, 4, "有一步要确认没做：{err}");
    assert_eq!(out, "读完了。\n", "步骤不进标准输出");
    // 工作区外面的写全了；太长的留后面 80 个字。
    let outside = plan_md.to_string_lossy();
    let count = outside.chars().count();
    let outside = match count > 80 {
        true => format!("…{}", outside.chars().skip(count - 80).collect::<String>()),
        false => outside.into_owned(),
    };
    assert_eq!(
        screen,
        format!(
            "· 读取 notes.md → 3 行\n· 读取 missing.md → 出错：没有这个文件\n· 读取 {outside} → 没做：要确认，这里没人能确认\n\n读完了。\n· 输入 400 · 命中缓存 160（40%）· 输出 40\n· 1 步没做：要你确认，miyu ask 里确认不了\n"
        ),
        "工作区里的写相对的，外面的照原样；最后说有几步因为要确认没做（施工 4-9）"
    );
}

#[tokio::test]
async fn a_directory_too_wide_is_said_first_and_once() {
    let home = home([
        Play::calls(&[("glob", r#"{"pattern":"*.md"}"#)]),
        Play::Says("没有。"),
    ]);
    let Asked {
        code, err, screen, ..
    } = home
        .ask(&Plan {
            cwd: "~".to_string(),
            ..plan("有哪些笔记")
        })
        .await;
    assert_eq!(code, 0, "{err}");
    let workspace = home.root.workspace(&AccountIdOf::admin());
    assert_eq!(
        screen,
        format!(
            "· 目录太宽（~），这次在 {} 里干活\n· 找文件 *.md → 一个都没找到\n\n没有。\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n",
            workspace.display()
        )
    );
}

#[tokio::test]
async fn going_on_from_a_directory_too_wide_says_it_too() {
    let work = Outside::new();
    let home = home([Play::Says("一。"), Play::Says("二。")]);
    let first = home
        .ask(&Plan {
            cwd: work.text(),
            ..plan("第一句")
        })
        .await;
    assert_eq!(first.code, 0, "{}", first.err);
    assert!(
        !first.err.contains("目录太宽"),
        "项目目录不说：{}",
        first.err
    );
    // 接着说时在 `~` 里：头报的目录跟着这一句送进会话，说话的回应里说实际在哪。剧本里的模型当场就回话，
    // 回应和她的回答谁先到不一定（真的模型要等网络，回应总是先到），这里只看说了、只说了一次；先后由跟着
    // 一轮的那一头的测试照固定的顺序喂着看。
    let second = home
        .ask(&Plan {
            target: Target::Continue,
            cwd: "~".to_string(),
            ..plan("第二句")
        })
        .await;
    assert_eq!(second.code, 0, "{}", second.err);
    let workspace = home.root.workspace(&AccountIdOf::admin());
    let said = format!("· 目录太宽（~），这次在 {} 里干活\n", workspace.display());
    assert_eq!(second.err.matches(&said).count(), 1, "{}", second.err);
    assert_eq!(second.out, "二。\n");
}

#[tokio::test]
async fn json_prints_no_steps() {
    let work = Outside::new();
    let notes = work.file("notes.md", "一\n");
    let home = home([read(&notes.to_string_lossy()), Play::Says("一行。")]);
    let Asked { code, out, err, .. } = home
        .ask(&Plan {
            cwd: work.text(),
            format: Format::Json,
            ..plan("读一下")
        })
        .await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "", "给脚本的：标准错误上只印出错");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(printed["turns"][0]["text"], "一行。");
}

/// 给脚本的：有一步因为要确认没做，只看退出码 4，标准错误上照旧只印出错（施工 4-9）。
#[tokio::test]
async fn json_says_a_step_needed_approval_only_by_the_exit_code() {
    let work = Outside::new();
    let elsewhere = Outside::new();
    let plan_md = elsewhere.file("plan.md", "别处\n");
    let home = home([read(&plan_md.to_string_lossy()), Play::Says("读不到。")]);
    let Asked { code, out, err, .. } = home
        .ask(&Plan {
            cwd: work.text(),
            format: Format::Json,
            ..plan("读一下")
        })
        .await;
    assert_eq!(code, 4, "{err}");
    assert_eq!(err, "");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(printed["turns"][0]["text"], "读不到。");
}

/// 假的核心：接一个连接，照协议允许的最晚的先后说话：一轮的推送都推完了，才回应说话的那一条（`04-核心协议.md`
/// 第六节第 2 条「先见结果，后见回应」）。造会话的回应说会话实际在 `used` 里干活，说话的回应不带目录。
async fn answers_last(mut listener: Listener, used: String) {
    let Ok(connection) = listener.accept().await else {
        return;
    };
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let request: Value = serde_json::from_str(&line).expect("是 JSON");
        let id = request["id"].clone();
        let mut out = Vec::new();
        let result = match request["method"].as_str() {
            Some("hello") => json!({"protocol": 1, "core": {"version": "0"}, "account": "admin"}),
            Some("session.create") => json!({"session": "s1", "events": [1], "cwd": used}),
            Some("session.send") => {
                let event = |kind: &str, body: Value| {
                    json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1", "event": {
                        "kind": kind, "turn": 3, "cause": id, "body": body}}})
                };
                out.extend([
                    event("turn.started", json!({"trigger": 2})),
                    event("model.delta", json!({"index": 0, "start": "text"})),
                    event("model.delta", json!({"index": 0, "text": "好。"})),
                    event(
                        "message.assistant",
                        json!({"blocks": [{"type": "text", "text": "好。"}]}),
                    ),
                    event("turn.ended", json!({"reason": "completed"})),
                ]);
                json!({ "events": [2] })
            }
            _ => json!({}),
        };
        out.push(json!({"jsonrpc": "2.0", "id": id, "result": result}));
        for message in out {
            let line = format!("{message}\n");
            if write.write_all(line.as_bytes()).await.is_err() {
                return;
            }
        }
    }
}

#[tokio::test]
async fn a_core_that_answers_last_still_hears_about_the_directory_first() {
    // 回应排在这一轮全部推送的后面：说话的回应还没到，这一轮就完了。目录太宽那一句靠造会话的回应，照样在最前面。
    let bare = Bare::new();
    let opened = miyu_ipc::open(&bare.root, &dirs()).expect("起得来");
    let core = tokio::spawn(answers_last(
        opened.listener,
        "/elsewhere/workspace".to_string(),
    ));
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        cwd: "~".to_string(),
        ..plan("在吗")
    };
    let Asked { code, screen, .. } = ask_at(&bare.root, &plan, presses).await;
    core.abort();
    assert_eq!(code, 0);
    assert_eq!(
        screen,
        "· 目录太宽（~），这次在 /elsewhere/workspace 里干活\n\n好。\n"
    );
}
