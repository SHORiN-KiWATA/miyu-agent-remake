//! 两种回报的写法（施工 7-2，`docs/blueprint/kernel/request.md`「回报」）：出厂的字渲染出来和样本逐字节一样，每种原因、
//! 截过的、人插过话的、没说话的各一个（`docs/designs/samples/reports/`）；闲着时开这一轮的那条挪到回合开始的地方，排在事实
//! 后面；回合中途到的排在那一步的工具结果后面；派它的那一轮撤掉了的不渲染；派它的那一条压缩掉了，照样有标题。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::{ChildReason, ChildReported, JobReason, JobReported};
use miyu_kernel::id::{ContentHash, JobId, SessionId};

use super::*;
use crate::test_support::{KERNEL, Log, shape, text_json, texts};
use crate::{DefaultAssembler, Stable};

mod messages;

const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 第一轮派出去后台命令 `j1`（跑全部测试）、子代理 `j2`（查 CI 为什么红），说完了。
fn dispatched() -> Log {
    let mut log = Log::new();
    let asked = log.say("查一下 CI，顺便跑测试");
    log.start(asked);
    let call = log.reply_calling("派出去。");
    let effects = format!(
        r#"[{{"kind":"job.started","job":"j1","what":"command","title":"跑全部测试"}},{{"kind":"job.started","job":"j2","what":"agent","title":"查 CI 为什么红","session":"{CHILD}"}}]"#
    );
    let body = format!(r#"{{"call_id":"{call}","status":"ok","blocks":[],"effects":{effects}}}"#);
    log.push(KERNEL, "tool.result", &body);
    log.reply(&format!("[{}]", text_json("派出去了。")));
    log.end("completed");
    log
}

/// 出厂的写法：资源目录里的真文件。
fn shipped() -> JobTexts {
    macro_rules! job {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/jobs/", $name)).to_string()
        };
    }
    let template = |text: String| Template::parse(&text).unwrap();
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
        subagent_message_open: template(job!("subagent-message-open.txt")),
        subagent_message_close: job!("subagent-message-close.txt"),
    }
}

/// `j1` 结束了：原因、退出码、信号、用时、输出的字数（有字数的带一份输出）。
fn ended(
    reason: JobReason,
    exit_code: Option<i32>,
    signal: Option<u32>,
    ms: Option<u64>,
    chars: Option<u64>,
) -> JobReported {
    JobReported {
        job: JobId::new(1).unwrap(),
        reason,
        exit_code,
        signal,
        by_model: false,
        duration_ms: ms,
        output: chars.map(|_| ContentHash::of(b"output")),
        chars,
    }
}

/// `j2` 的回报：原因、正文、截过没有、人插过话没有。
fn child(reason: ChildReason, text: &str, truncated: bool, person: bool) -> ChildReported {
    ChildReported {
        job: JobId::new(2).unwrap(),
        session: SessionId::parse(CHILD).unwrap(),
        reason,
        text: text.to_string(),
        truncated,
        person,
        by_model: false,
    }
}

macro_rules! sample {
    ($name:literal) => {
        include_str!(concat!("../../../../docs/designs/samples/reports/", $name))
    };
}

#[test]
fn a_command_ending_reads_like_the_samples() {
    let log = dispatched();
    let texts = shipped();
    let cases = [
        (
            sample!("command-exited.txt"),
            ended(JobReason::Exited, Some(0), None, Some(81_234), Some(48_213)),
        ),
        (
            sample!("command-killed.txt"),
            ended(JobReason::Exited, None, Some(9), Some(1_200), Some(0)),
        ),
        (
            sample!("command-stopped.txt"),
            ended(
                JobReason::Stopped,
                None,
                Some(15),
                Some(300_000),
                Some(5_120),
            ),
        ),
        (
            sample!("command-undone.txt"),
            ended(JobReason::Undone, None, Some(15), Some(4_000), Some(96)),
        ),
        (
            sample!("command-restarted.txt"),
            ended(
                JobReason::Restarted,
                None,
                Some(15),
                Some(60_000),
                Some(2_048),
            ),
        ),
        (
            sample!("command-aborted.txt"),
            ended(JobReason::Aborted, None, None, None, None),
        ),
    ];
    for (sample, reported) in cases {
        assert_eq!(
            command(log.history(), &reported, &texts).as_deref(),
            Some(sample)
        );
    }
    // Windows 的退出码可以是负的，照原样；没存下输出的不写字数。
    let mut windows = ended(
        JobReason::Exited,
        Some(-1_073_741_819),
        None,
        Some(5),
        Some(3),
    );
    windows.output = None;
    let block = command(log.history(), &windows, &texts).unwrap();
    assert!(block.contains("Exit code -1073741819.\n"), "{block}");
    assert!(!block.contains("characters"), "{block}");
}

#[test]
fn a_subagent_report_reads_like_the_samples() {
    let log = dispatched();
    let texts = shipped();
    let done = "CI 红在 macOS：测试的临时目录在 /var 下，/var 是链接，安全打开不走链接，拒绝了。先把临时目录换成真实路径就好。";
    let cases = [
        (
            sample!("subagent-done.txt"),
            child(ChildReason::Done, done, false, false),
        ),
        (
            sample!("subagent-stopped.txt"),
            child(
                ChildReason::Stopped,
                "查到一半：只看了 Linux 的日志。",
                false,
                false,
            ),
        ),
        (
            sample!("subagent-undone.txt"),
            child(ChildReason::Undone, "还没查完。\n", false, false),
        ),
        (
            sample!("subagent-aborted.txt"),
            child(ChildReason::Aborted, "先看了 macOS 的日志。", false, false),
        ),
        (
            sample!("subagent-silent.txt"),
            child(ChildReason::Done, "", false, false),
        ),
        (
            sample!("subagent-truncated.txt"),
            child(
                ChildReason::Done,
                "开头的几段。\n[... 1200 characters omitted ...]\n结尾的几段。",
                true,
                false,
            ),
        ),
        (
            sample!("subagent-person.txt"),
            child(ChildReason::Done, "按你说的，只查了 macOS。", false, true),
        ),
    ];
    for (sample, reported) in cases {
        assert_eq!(
            subagent(log.history(), &reported, &texts).as_deref(),
            Some(sample)
        );
    }
    // 两样都有的：先人插过话，再截过，再正文。
    let both = subagent(
        log.history(),
        &child(ChildReason::Done, "正文", true, true),
        &texts,
    )
    .unwrap();
    let person = both.find("talked").unwrap();
    let cut = both.find("was cut").unwrap();
    assert!(person < cut && cut < both.find("正文").unwrap(), "{both}");
}

#[test]
fn the_title_is_escaped_like_any_field() {
    let mut log = Log::new();
    let asked = log.say("派");
    log.start(asked);
    let call = log.reply_calling("派出去。");
    let effects = r#"[{"kind":"job.started","job":"j1","what":"command","title":"a\" b=\"<x>"}]"#;
    let body = format!(r#"{{"call_id":"{call}","status":"ok","blocks":[],"effects":{effects}}}"#);
    log.push(KERNEL, "tool.result", &body);
    let reported = ended(JobReason::Aborted, None, None, None, None);
    let block = command(log.history(), &reported, &shipped()).unwrap();
    assert!(
        block.starts_with(
            r#"<command-ended job="j1" title="a\u0022 b=\u0022\u003cx\u003e" reason="aborted">"#
        ),
        "{block}"
    );
}

/// 替身的组装器：没有工具面、没有 system，字是替身的。
fn assembler() -> DefaultAssembler {
    let stable = Stable {
        tools: Vec::new(),
        system: String::new(),
        demos: Vec::new(),
    };
    DefaultAssembler::new(stable, texts())
}

/// 后台命令 `j1` 自己退出了的 `body`。
const EXITED: &str = r#"{"job":"j1","reason":"exited","exit_code":0}"#;

#[test]
fn a_report_that_opens_a_turn_goes_to_where_the_turn_starts() {
    let mut log = dispatched();
    let reported = log.detached(KERNEL, "job.reported", EXITED);
    log.start(reported);
    log.fact("<env/>");
    let request = assembler().assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: <env/> | <command j1 跑全部测试 exited>\nexit 0\n</command>\n"),
        "开这一轮的那条挪到回合开始的地方，事实在前"
    );
}

#[test]
fn a_report_in_the_middle_of_a_turn_comes_after_the_results_of_that_step() {
    let mut log = dispatched();
    let asked = log.say("再读一个");
    log.start(asked);
    let call = log.reply_calling("我读一下。");
    log.detached(KERNEL, "job.reported", EXITED);
    log.result(&call, "ok", "A");
    let request = assembler().assemble(log.history());
    let shape = shape(&request.messages);
    assert_eq!(
        shape[shape.len() - 2..],
        [
            format!("tool {call} ok: A"),
            "user: <command j1 跑全部测试 exited>\nexit 0\n</command>\n".to_string(),
        ],
        "请求在路上时到的，排在这一步的工具结果后面"
    );
}

#[test]
fn a_report_for_a_job_of_an_undone_turn_is_not_rendered() {
    let mut log = dispatched();
    log.push(
        r#"{"kind":"person","account":"alice"}"#,
        "turn.reverted",
        r#"{"turns":[3]}"#,
    );
    log.detached(KERNEL, "job.reported", EXITED);
    let child = format!(r#"{{"job":"j2","session":"{CHILD}","reason":"done","text":"好了"}}"#);
    log.detached(
        &format!(r#"{{"kind":"session","id":"{CHILD}"}}"#),
        "child.reported",
        &child,
    );
    let request = assembler().assemble(log.history());
    let whole = shape(&request.messages).join("\n");
    assert!(!whole.contains("<command"), "{whole}");
    assert!(!whole.contains("<subagent"), "{whole}");
}

#[test]
fn a_report_after_its_start_was_compacted_still_has_the_title() {
    let mut log = dispatched();
    let upto = log.next() - 1;
    let asked = log.say("压一下");
    log.start(asked);
    log.compact(upto, "派了两个任务。");
    log.reply(&format!("[{}]", text_json("好。")));
    log.end("completed");
    log.detached(KERNEL, "job.reported", EXITED);
    let request = assembler().assemble(log.history());
    let whole = shape(&request.messages).join("\n");
    assert!(whole.contains("<command j1 跑全部测试 exited>"), "{whole}");
    // 旧快照没有回报的写法：不渲染。
    let old = DefaultAssembler::new(
        Stable {
            tools: Vec::new(),
            system: String::new(),
            demos: Vec::new(),
        },
        crate::Texts {
            jobs: None,
            ..texts()
        },
    );
    let whole = shape(&old.assemble(log.history()).messages).join("\n");
    assert!(!whole.contains("<command"), "{whole}");
}
