//! 出厂的英文：组装器要的固定字，资源目录里的真文件（施工 7-2 从 `mod.rs` 挪出来，加上回报的写法）。

use miyu_assemble::{JobTexts, RestoredWrap, Texts, TurnEndedTexts};
use miyu_kernel::template::Template;

/// 出厂的摘要指令（3-9 三补合并时从 `mod.rs` 挪来）：摘要请求的最后一块（施工 6-2 上）；正文接最后那一句（施工 6-8 拆开）。
pub const SUMMARIZE: &str = concat!(
    include_str!("../../../../resources/core/compaction/summarize-task.txt"),
    include_str!("../../../../resources/core/compaction/summarize-end.txt")
);

/// 子代理的场所说明（施工 7-5）：出厂的原文，子会话的 system 接在人设后面。
pub const VENUE: &str = include_str!("../../../../resources/core/jobs/subagent-venue.txt");

/// 出厂的英文，资源目录里的真文件。
pub(super) fn texts() -> Texts {
    // 压缩的几份字：`resources/core/compaction/` 下的同名文件。
    macro_rules! compaction {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/compaction/", $name)).to_string()
        };
    }
    Texts {
        checkpoint_open: include_str!("../../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../../resources/core/checkpoint-close.txt")
            .to_string(),
        checkpoint_end: include_str!("../../../../resources/core/checkpoint-end.txt").to_string(),
        restored: Some(RestoredWrap {
            open: Template::parse(include_str!(
                "../../../../resources/core/compaction/restored-open.txt"
            ))
            .expect("出厂的模板合写法"),
            close: compaction!("restored-close.txt"),
        }),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../../resources/core/turn-ended/aborted.txt").to_string(),
            restarted: include_str!("../../../../resources/core/turn-ended/restarted.txt")
                .to_string(),
        },
        summarize_task: compaction!("summarize-task.txt"),
        truncated: compaction!("truncated.txt"),
        summarize_system: compaction!("summarize-system.txt"),
        summarize_instructions: compaction!("summarize-instructions.txt"),
        summarize_end: compaction!("summarize-end.txt"),
        jobs: Some(job_texts()),
    }
}

/// 出厂的回报写法（施工 7-2），资源目录里的真文件。
fn job_texts() -> JobTexts {
    macro_rules! job {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/jobs/", $name)).to_string()
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
        subagent_message_open: template(job!("subagent-message-open.txt")),
        subagent_message_close: job!("subagent-message-close.txt"),
    }
}
