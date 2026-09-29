//! 后台命令在会话里（施工 7-3，`docs/blueprint/agents.md` 第四条、第八条）：真的 `shell` 放到后台，调用当场结束、命令接着
//! 跑，输出写进 `jobs/<编号>.out`，结束了记 `job.reported`（`by` 是起它的那次调用，`cause` 是那一轮的，输出存成 blob），闲着
//! 的她被叫醒；编号照日志往后数，撤掉的回合里用过的不再用；有计划地停下先记 `restarted`、落了盘再杀；没记就停了的，
//! 再载入时补 `aborted`。假的后台命令（`miyu_tool::testkit::Held`）三个平台一样；真的 `shell` 三个平台都跑，沙盒那一条
//! 只在 Unix 上有收紧手段时跑。

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use miyu_kernel::event::{Body, Event, JobReason, JobReported, Level, Permission, ToolResult};
use miyu_kernel::id::{JobId, TurnId};
use miyu_kernel::origin::{By, Tool as ByTool};
use miyu_kernel::session::Command;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, Tool};

use support::*;

/// 真的基础系统。
fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 只有一件假工具 `start`：把 `held` 交给任务端口。
fn starting(held: &Arc<Held>) -> Catalog {
    let tool = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    Catalog::new([tool as Arc<dyn Tool>]).expect("合写法")
}

/// 完全放开这一级、没人能确认，在场地的 `work/` 里：执行命令不问、不进沙盒。
fn full(home: &Home) -> Opening {
    Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        ..Opening::default()
    }
}

/// 说一句，等这一轮结束。
async fn talk(handle: &miyu_session::Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里的 `job.reported`，和那一条事件。
fn reports(log: &[Event]) -> Vec<(&Event, &JobReported)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::JobReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect()
}

/// 日志里的工具结果。
fn results(log: &[Event]) -> Vec<&ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 等任务表空了，最多十秒。
async fn until_idle(home: &Home) {
    within("任务表空了", async {
        while home.jobs.running() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn a_background_command_ends_on_its_own_and_wakes_her() {
    let home = Home::outside_temp();
    let command = if cfg!(windows) {
        "Start-Sleep -Milliseconds 1000; Write-Output hello; exit 2"
    } else {
        "sleep 1; echo hello; exit 2"
    };
    let args = serde_json::json!({ "command": command, "description": "Say hello", "run_in_background": true })
        .to_string();
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("放出去了。"),
        Play::Says("看到了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "后台跑一下").await;
    assert!(home.jobs.running(), "这一轮结束了，命令还在跑");
    let log = until_logged(&home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2
    })
    .await;
    let started = results(&log)[0];
    assert!(
        started.effects.iter().any(|effect| matches!(effect, miyu_kernel::event::Effect::JobStarted(job) if job.job == JobId::new(1).unwrap() && job.title == "Say hello")),
        "{started:?}"
    );
    let [(event, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    assert_eq!(reported.reason, JobReason::Exited);
    assert_eq!(reported.exit_code, Some(2));
    assert_eq!(
        event.by,
        By::Tool(ByTool {
            call_id: started.call_id
        }),
        "起它的那次调用"
    );
    assert_eq!(event.cause, Some(id("cmd-1")), "那一轮的 cause");
    assert_eq!(event.turn, None, "回报不带回合编号");
    let output = reported.output.clone().expect("存了输出");
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    let text = String::from_utf8(blobs.get(&output).expect("blob 在")).expect("是 UTF-8");
    assert_eq!(text, "hello\n");
    assert_eq!(reported.chars, Some(6));
    let file = miyu_store::jobs::output_path(
        &home.root.session_dir(&alice_account(), handle.id()),
        &JobId::new(1).unwrap(),
    );
    assert_eq!(
        std::fs::read_to_string(file).expect("输出文件在"),
        "hello\n"
    );
    let woke = log
        .iter()
        .find_map(|later| match &later.body {
            Body::TurnStarted(started) if later.seq > event.seq => Some(started.trigger),
            _ => None,
        })
        .expect("开了一轮");
    assert_eq!(woke, Some(event.seq), "闲着的她由它叫醒");
    until_idle(&home).await;
}

#[tokio::test]
async fn ids_count_on_after_an_undo_and_a_reload() {
    let home = Home::new();
    let first = Held::new(&[]);
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("结束了。"),
    ]);
    let handle = home.create_with(&script, &starting(&first)).await;
    talk(&handle, "cmd-1", "放一个").await;
    first.end(Exit::Code(0));
    // 结束了叫醒她说一句：等那一轮也结束。
    let log = until_logged(&home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2
    })
    .await;
    let turn = log
        .iter()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .expect("开过一轮");
    ask(&handle, "cmd-2", Command::Revert { turn: Some(turn) })
        .await
        .expect("会话在跑");
    let session = handle.id().clone();
    stop(&handle).await;
    drop(handle);
    let second = Held::new(&[]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("又放了一个。")]);
    let handle = home.load_with(&session, &script, &starting(&second)).await;
    talk(&handle, "cmd-3", "再放一个").await;
    let log = home.log(handle.id());
    let last = results(&log).last().copied().expect("有结果");
    assert!(
        matches!(&last.effects[..], [miyu_kernel::event::Effect::JobStarted(job)] if job.job == JobId::new(2).unwrap()),
        "撤掉的那一轮用过 j1，不再用：{last:?}"
    );
    second.end(Exit::Code(0));
}

#[tokio::test]
async fn a_planned_stop_records_restarted_before_it_kills() {
    let home = Home::new();
    let held = Held::new(&["half\n"]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("放出去了。")]);
    let handle = home.create_with(&script, &starting(&held)).await;
    talk(&handle, "cmd-1", "放一个").await;
    let seen_at_kill = Arc::new(Mutex::new(None));
    let (root, seen) = (
        home.root.session_dir(&alice_account(), handle.id()),
        Arc::clone(&seen_at_kill),
    );
    held.on_kill(move || {
        let log = miyu_store::log::read_events(&root).expect("日志读得出");
        let recorded = reports(&log)
            .iter()
            .any(|(_, reported)| reported.reason == JobReason::Restarted);
        *seen.lock().unwrap() = Some(recorded);
    });
    stop(&handle).await;
    assert_eq!(held.killed(), 1, "停下时整组杀掉");
    assert_eq!(
        *seen_at_kill.lock().unwrap(),
        Some(true),
        "杀的时候 restarted 已经落了盘"
    );
    let log = home.log(handle.id());
    let [(event, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    assert_eq!(reported.reason, JobReason::Restarted);
    assert_eq!(event.by, By::Kernel);
    assert_eq!(event.cause, None);
    assert!(reported.duration_ms.is_some());
    assert_eq!(reported.chars, Some(5), "到这时的输出");
    assert!(!home.jobs.running());
}

#[tokio::test]
async fn a_session_that_stops_without_a_record_gets_aborted_on_load() {
    let home = Home::new();
    let held = Held::new(&[]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("放出去了。")]);
    let handle = home.create_with(&script, &starting(&held)).await;
    let session = handle.id().clone();
    talk(&handle, "cmd-1", "放一个").await;
    // 没人拿着了：actor 停下，整组杀掉，什么都不记，和核心崩了一样。
    drop(handle);
    within("杀掉", async {
        while held.killed() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    until_idle(&home).await;
    assert!(
        reports(&home.log(&session)).is_empty(),
        "停的时候什么都没记"
    );
    let handle = home.load(&session, &Script::new([])).await;
    let log = until_logged(&home, &session, |log| !reports(log).is_empty()).await;
    let [(event, reported)] = reports(&log)[..] else {
        panic!("补了一条：{log:#?}")
    };
    assert_eq!(reported.reason, JobReason::Aborted);
    assert_eq!(event.by, By::Kernel);
    assert_eq!(reported.duration_ms, None, "什么时候没的不知道");
    assert!(
        !log.iter()
            .skip_while(|later| later.seq <= event.seq)
            .any(|later| matches!(later.body, Body::TurnStarted(_))),
        "只记下，不开轮"
    );
    drop(handle);
}

/// 真的经助手把命令放到后台（Unix 上有收紧手段的）：和前台一样关在沙盒里，工作区里写得进，外面写不进。
#[cfg(unix)]
#[tokio::test]
async fn a_background_command_stays_in_the_sandbox() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, Duration::from_secs(5)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let outside = home.scratch.0.join("other").join("outside.txt");
    let command = format!(
        "echo in > inside.txt && echo wrote-inside; \
         (echo out > '{}') 2>/dev/null && echo wrote-outside || echo blocked-outside",
        outside.display()
    );
    let args =
        serde_json::json!({ "command": command, "description": "Test", "run_in_background": true })
            .to_string();
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("放出去了。"),
        Play::Says("看到了。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        sandbox: Some(helper),
        ..Opening::default()
    };
    let handle = home.create_as(&script, &base_system(), opening).await;
    talk(&handle, "cmd-1", "后台跑一下").await;
    let log = until_logged(&home, handle.id(), |log| !reports(log).is_empty()).await;
    let [(_, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    let output = blobs
        .get(reported.output.as_ref().expect("存了输出"))
        .expect("blob 在");
    assert_eq!(
        String::from_utf8(output).expect("是 UTF-8"),
        "wrote-inside\nblocked-outside\n"
    );
    assert!(work.join("inside.txt").exists());
    assert!(!outside.exists());
    until_idle(&home).await;
}

/// 第 `depth` 层的子会话，造它的命令是 `<父会话>/<job>`（施工 7-1 补：它领的号照这个编号带前缀）。
fn child_lines(depth: u32, job: &str) -> Lines {
    let parent = miyu_kernel::id::SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99")
        .expect("会话编号合写法");
    Lines {
        lineage: Some(miyu_session::Lineage {
            parent: parent.clone(),
            depth,
        }),
        command: Some(id(&format!("{parent}/{job}"))),
        ..Lines::default()
    }
}

/// 假工具 `start` 把 `held` 交给任务端口，再加上真的 `jobs`：工具面造会话时定下，载入以后照它。
fn starting_with_jobs(held: &Arc<Held>) -> Catalog {
    let jobs = Arc::clone(base_system().get("jobs").expect("基础系统里有 jobs"));
    let start = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    Catalog::new([start as Arc<dyn Tool>, jobs]).expect("合写法")
}

/// 日志里最后一条结果派的任务编号。
fn last_started(log: &[Event]) -> String {
    results(log)
        .iter()
        .rev()
        .flat_map(|result| &result.effects)
        .find_map(|effect| match effect {
            miyu_kernel::event::Effect::JobStarted(started) => Some(started.job.to_string()),
            _ => None,
        })
        .expect("派过任务")
}

/// 孙会话放到后台的命令编号是三段（施工 7-1 补）：造它的命令是 `<子会话>/j2.1`，它的后台命令是 `j2.1.1`，结果那一句、效果、
/// 输出文件 `jobs/j2.1.1.out` 都照这个编号。
#[tokio::test]
async fn a_grandchild_numbers_its_background_commands_in_three_parts() {
    let home = Home::new();
    let held = Held::new(&["half\n"]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("放出去了。")]);
    let handle = home
        .create_full(
            &script,
            &starting(&held),
            Opening::default(),
            child_lines(2, "j2.1"),
        )
        .await;
    talk(&handle, "cmd-1", "放一个").await;
    let log = home.log(handle.id());
    assert_eq!(last_started(&log), "j2.1.1");
    let [result] = results(&log)[..] else {
        panic!("一次调用：{log:#?}")
    };
    assert!(
        matches!(result.blocks.as_slice(), [miyu_kernel::block::Block::Text(text)] if text.text == "started j2.1.1"),
        "{result:?}"
    );
    let file = miyu_store::jobs::output_path(
        &home.root.session_dir(&alice_account(), handle.id()),
        &JobId::parse("j2.1.1").unwrap(),
    );
    assert!(file.ends_with("jobs/j2.1.1.out"), "{}", file.display());
    within("输出写进文件", async {
        while std::fs::read_to_string(&file).unwrap_or_default() != "half\n" {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    stop(&handle).await;
    until_idle(&home).await;
}

/// 以前的日志里子会话派的 `j1` 照认（施工 7-1 补）：它是只有一段的编号，载入读得进来，`jobs` 照它列出来；它占着 1，接着
/// 领的是 `j2.2`。旧日志照这个样子造：子会话 `j2` 派一个、结束了，把日志里的 `j2.1` 换回改之前写的 `j1`。
#[tokio::test]
async fn an_old_child_log_that_numbered_from_j1_still_loads() {
    let home = Home::new();
    let first = Held::new(&[]);
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("结束了。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &starting_with_jobs(&first),
            Opening::default(),
            child_lines(1, "j2"),
        )
        .await;
    talk(&handle, "cmd-1", "放一个").await;
    first.end(Exit::Code(0));
    until_logged(&home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2
    })
    .await;
    let session = handle.id().clone();
    stop(&handle).await;
    drop(handle);
    until_idle(&home).await;
    let segment = home
        .root
        .session_dir(&alice_account(), &session)
        .join("000000000001.jsonl");
    let text = std::fs::read_to_string(&segment).expect("读得了第一段");
    assert!(text.contains(r#""job":"j2.1""#), "改之前是 j2.1");
    std::fs::write(
        &segment,
        text.replace(r#""job":"j2.1""#, r#""job":"j1""#)
            .replace("started j2.1", "started j1"),
    )
    .expect("写得回去");

    let second = Held::new(&[]);
    let tools = starting_with_jobs(&second);
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::calls(&[("jobs", r#"{"action":"list"}"#)]),
        Play::Says("两个。"),
    ]);
    let handle = home.load_with(&session, &script, &tools).await;
    talk(&handle, "cmd-2", "再放一个").await;
    let log = home.log(handle.id());
    assert_eq!(last_started(&log), "j2.2", "j1 占着 1");
    let listed = results(&log).last().copied().expect("列了一次");
    let [miyu_kernel::block::Block::Text(listed)] = listed.blocks.as_slice() else {
        panic!("一段字：{listed:?}")
    };
    let jobs: Vec<&str> = listed
        .text
        .lines()
        .map(|line| line.split(' ').next().unwrap_or_default())
        .collect();
    assert_eq!(jobs, ["j1", "j2.2"], "{}", listed.text);
    stop(&handle).await;
    until_idle(&home).await;
}
