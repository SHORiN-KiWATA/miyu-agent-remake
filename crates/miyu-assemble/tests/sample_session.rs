//! 样本会话组装出样本请求（`docs/designs/08-上下文投影.md` 第四节「默认的组装怎么写」）：
//! 事件的样本（`docs/designs/samples/events/`）讲的是同一个会话，喂进有效历史，
//! 组装出来的请求逐字节等于请求的样本（`docs/designs/samples/requests/`）。
//!
//! - 喂到 47 号为止，是 42 号回合里工具结果回来以后的那一次请求（`second-step.json`）。46 号是
//!   第一次请求的 `model.called`，不渲染；
//! - 喂到 54 号为止，是压缩以后的样子，只剩检查点（`after-compaction.json`）；
//! - 全部喂进去：压缩以后那一轮里排着队、又被撤回的那句话，不在请求里。
//!
//! 出厂的英文用资源目录里的真文件，在编译时拿进来。样本是图纸的一部分，住在设计文档旁边，
//! 所以要读文件；纯逻辑门禁只扫 `src/`，集成测试可以读。样本的序号中间有空当（省掉了
//! 前面的几十条），过不了账本，所以直接交给有效历史。

use std::fs;
use std::path::PathBuf;

use miyu_assemble::{DefaultAssembler, JobTexts, RestoredWrap, Stable, Texts, TurnEndedTexts};
use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::ToolSpec;
use miyu_kernel::template::Template;

/// 样本会话的工具面：只有一件 `read`。
const READ: &str = "Read a text file by line pages, an image, a PDF, or list a directory. Prefer this over `cat` in the shell: files read here come back after compaction.";
const READ_PARAMETERS: &str = r#"{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}"#;

/// 出厂的英文，资源目录里的真文件。
fn texts() -> Texts {
    Texts {
        checkpoint_open: include_str!("../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../resources/core/checkpoint-close.txt").to_string(),
        checkpoint_end: include_str!("../../../resources/core/checkpoint-end.txt").to_string(),
        restored: Some(RestoredWrap {
            open: Template::parse(include_str!(
                "../../../resources/core/compaction/restored-open.txt"
            ))
            .expect("出厂的模板合写法"),
            close: include_str!("../../../resources/core/compaction/restored-close.txt")
                .to_string(),
        }),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../resources/core/turn-ended/aborted.txt").to_string(),
            restarted: include_str!("../../../resources/core/turn-ended/restarted.txt").to_string(),
        },
        summarize_task: include_str!("../../../resources/core/compaction/summarize-task.txt")
            .to_string(),
        truncated: include_str!("../../../resources/core/compaction/truncated.txt").to_string(),
        summarize_system: include_str!("../../../resources/core/compaction/summarize-system.txt")
            .to_string(),
        summarize_instructions: include_str!(
            "../../../resources/core/compaction/summarize-instructions.txt"
        )
        .to_string(),
        summarize_end: include_str!("../../../resources/core/compaction/summarize-end.txt")
            .to_string(),
        jobs: Some(job_texts()),
    }
}

/// 出厂的回报写法（施工 7-2），资源目录里的真文件。
fn job_texts() -> JobTexts {
    macro_rules! job {
        ($name:literal) => {
            include_str!(concat!("../../../resources/core/jobs/", $name)).to_string()
        };
    }
    let template = |text: String| Template::parse(&text).expect("出厂的模板合写法");
    JobTexts {
        command_open: template(job!("command-open.txt")),
        command_exit: template(job!("command-exit.txt")),
        command_signal: template(job!("command-signal.txt")),
        command_duration: template(job!("command-duration.txt")),
        command_output: template(job!("command-output.txt")),
        command_close: job!("command-close.txt"),
        subagent_open: template(job!("subagent-open.txt")),
        subagent_person: job!("subagent-person.txt"),
        subagent_truncated: job!("subagent-truncated.txt"),
        subagent_silent: job!("subagent-silent.txt"),
        subagent_close: job!("subagent-close.txt"),
        stopped_by_user: job!("stopped-by-user.txt"),
        subagent_message_open: template(job!("subagent-message-open.txt")),
        subagent_message_close: job!("subagent-message-close.txt"),
    }
}

/// 样本会话的组装器：一件 `read` 工具，一句 system，没有示范对话。
fn assembler() -> DefaultAssembler {
    let parameters: RawJson = serde_json::from_str(READ_PARAMETERS).expect("参数格式是 JSON");
    let stable = Stable {
        tools: vec![ToolSpec {
            name: "read".to_string(),
            description: READ.to_string(),
            parameters,
        }],
        system: "You are a helpful software engineer.".to_string(),
        demos: vec![],
    };
    DefaultAssembler::new(stable, texts())
}

/// 仓库根：这个 crate 的目录往上两级。
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 样本会话的全部事件，照序号排好。
fn events() -> Vec<Event> {
    let dir = root().join("docs/designs/samples/events");
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|e| panic!("读不了样本目录 {}：{e}", dir.display()));
    let mut events = Vec::new();
    for entry in entries {
        let path = entry.expect("列样本目录时出错").path();
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        for line in text.lines() {
            let event = Event::from_line(line)
                .unwrap_or_else(|e| panic!("{} 读不出来：{e}", path.display()));
            events.push(event);
        }
    }
    // 带 `parent` 的 `session.created` 是样本会话派的子代理自己日志里的第一条，不是这个会话的（施工 7-1）。
    events.retain(
        |event| !matches!(&event.body, Body::SessionCreated(created) if created.parent.is_some()),
    );
    events.sort_by_key(|event| event.seq);
    events
}

/// 请求的样本，去掉行尾的换行。
fn sample(name: &str) -> String {
    let path = root().join("docs/designs/samples/requests").join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    text.strip_suffix('\n')
        .unwrap_or_else(|| panic!("{name} 要以一个换行结尾"))
        .to_string()
}

/// 把序号不超过 `upto` 的样本事件依次交给有效历史，组装出请求的规范字节。
fn assembled_upto(upto: u64) -> String {
    let mut history = History::default();
    for event in events() {
        if event.seq.get() <= upto {
            history.append(event);
        }
    }
    let bytes = assembler().assemble(&history).canonical_bytes();
    String::from_utf8(bytes).expect("规范的字节是 UTF-8")
}

#[test]
fn the_session_up_to_the_tool_result_is_the_second_step() {
    assert_eq!(assembled_upto(47), sample("second-step.json"));
}

#[test]
fn the_session_up_to_the_compaction_is_the_checkpoint_alone() {
    assert_eq!(assembled_upto(54), sample("after-compaction.json"));
}

#[test]
fn the_withdrawn_message_never_reaches_the_request() {
    let whole = assembled_upto(u64::MAX);
    assert!(whole.contains("再看看 tests 目录"), "{whole}");
    assert!(!whole.contains("README"), "撤回的那句不该在请求里：{whole}");
}

/// 请人确认和人的决定不进上下文，她看到的只有工具结果：被拒绝的那一句、带着理由在请求里，
/// 请求里给头看的说明不在。
#[test]
fn only_the_denied_result_of_an_approval_reaches_the_request() {
    let whole = assembled_upto(u64::MAX);
    assert!(
        whole.contains("the user denied it and said"),
        "被拒绝的结果要在请求里：{whole}"
    );
    assert!(
        !whole.contains("outside_workspace"),
        "请求给头看的说明不该在请求里：{whole}"
    );
}

/// 问人和人的回答也不进上下文：工具照回答写的结果在请求里，题目里给人看的说明不在。
#[test]
fn only_the_result_of_a_question_reaches_the_request() {
    let whole = assembled_upto(u64::MAX);
    assert!(
        whole.contains("The user answered"),
        "工具的结果要在请求里：{whole}"
    );
    assert!(
        !whole.contains("下次编译从头来"),
        "题目里的说明不该在请求里：{whole}"
    );
}

/// 任务的两种回报（施工 7-2）：带标签的事实，标题照派它的那条 `job.started`；后台命令不带输出本身。
#[test]
fn the_reports_reach_the_request_as_tagged_facts() {
    let whole = assembled_upto(u64::MAX);
    assert!(
        whole.contains(r#"<command-ended job=\"j1\" title=\"跑全部测试\" reason=\"exited\">\nExit code 0.\nRan for 81234 ms.\n"#),
        "{whole}"
    );
    assert!(
        whole.contains(r#"<subagent-report job=\"j2\" title=\"查 CI 为什么红\" reason=\"done\">\nCI 红在 macOS"#),
        "{whole}"
    );
}
