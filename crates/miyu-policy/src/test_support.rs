//! 测试的夹具：出厂的随核心附带的字、软件工程师的快照。随核心附带的字用仓库里出厂的那一份（编译时
//! 拿进来，不是读文件）。

use crate::compose::{PersonaTexts, Sources, compose};
use crate::drivers::DriverPlaceholders;
use crate::snapshot::{
    CompactionTexts, CoreTexts, FactTexts, PermissionTexts, Snapshot, ToolResultTexts,
    TurnEndedTexts,
};

/// 出厂的随核心附带的字。
pub(crate) fn core() -> CoreTexts {
    CoreTexts {
        checkpoint_open: include_str!("../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../resources/core/checkpoint-close.txt").to_string(),
        checkpoint_end: include_str!("../../../resources/core/checkpoint-end.txt").to_string(),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../resources/core/turn-ended/aborted.txt").to_string(),
            restarted: include_str!("../../../resources/core/turn-ended/restarted.txt").to_string(),
        },
        facts: FactTexts {
            env: include_str!("../../../resources/core/facts/env.txt").to_string(),
            permission: include_str!("../../../resources/core/facts/permission.txt").to_string(),
            reply_cut: include_str!("../../../resources/core/facts/reply-cut.txt").to_string(),
        },
        tool_results: ToolResultTexts {
            unknown: include_str!("../../../resources/core/tool-results/unknown.txt").to_string(),
            not_an_object: include_str!("../../../resources/core/tool-results/not-an-object.txt")
                .to_string(),
            cancelled_before: include_str!(
                "../../../resources/core/tool-results/cancelled-before.txt"
            )
            .to_string(),
            cancelled_running: include_str!(
                "../../../resources/core/tool-results/cancelled-running.txt"
            )
            .to_string(),
            skipped: include_str!("../../../resources/core/tool-results/skipped.txt").to_string(),
            read_only: include_str!("../../../resources/core/tool-results/read-only.txt")
                .to_string(),
            denied: include_str!("../../../resources/core/tool-results/denied.txt").to_string(),
            denied_with_reason: include_str!(
                "../../../resources/core/tool-results/denied-with-reason.txt"
            )
            .to_string(),
            unattended: include_str!("../../../resources/core/tool-results/unattended.txt")
                .to_string(),
            question_interrupted: include_str!(
                "../../../resources/core/tool-results/question-interrupted.txt"
            )
            .to_string(),
            question_voided: include_str!(
                "../../../resources/core/tool-results/question-voided.txt"
            )
            .to_string(),
            question_unattended: include_str!(
                "../../../resources/core/tool-results/question-unattended.txt"
            )
            .to_string(),
            unavailable: include_str!("../../../resources/core/tool-results/unavailable.txt")
                .to_string(),
            crashed: include_str!("../../../resources/core/tool-results/crashed.txt").to_string(),
            restarted: include_str!("../../../resources/core/tool-results/restarted.txt")
                .to_string(),
        },
        permissions: PermissionTexts {
            forbidden: include_str!("../../../resources/core/permissions/forbidden.txt")
                .to_string(),
            unresolvable: include_str!("../../../resources/core/permissions/unresolvable.txt")
                .to_string(),
        },
        drivers: DriverPlaceholders {
            image_omitted: include_str!("../../../resources/core/drivers/image-omitted.txt")
                .to_string(),
            file_omitted: include_str!("../../../resources/core/drivers/file-omitted.txt")
                .to_string(),
            no_output: include_str!("../../../resources/core/drivers/no-output.txt").to_string(),
            tool_attachments: include_str!("../../../resources/core/drivers/tool-attachments.txt")
                .to_string(),
            tool_attachments_only: include_str!(
                "../../../resources/core/drivers/tool-attachments-only.txt"
            )
            .to_string(),
            text_file: Some(crate::TextFileTexts {
                file_open: include_str!("../../../resources/core/drivers/file-open.txt")
                    .to_string(),
                file_cut: include_str!("../../../resources/core/drivers/file-cut.txt").to_string(),
                file_close: include_str!("../../../resources/core/drivers/file-close.txt")
                    .to_string(),
            }),
            image_name: Some(crate::ImageNameTexts {
                image_open: include_str!("../../../resources/core/drivers/image-open.txt")
                    .to_string(),
                image_close: include_str!("../../../resources/core/drivers/image-close.txt")
                    .to_string(),
                image_omitted_named: include_str!(
                    "../../../resources/core/drivers/image-omitted-named.txt"
                )
                .to_string(),
            }),
        },
        compaction: Some(CompactionTexts {
            summarize_task: include_str!("../../../resources/core/compaction/summarize-task.txt")
                .to_string(),
            summarize_instructions: include_str!(
                "../../../resources/core/compaction/summarize-instructions.txt"
            )
            .to_string(),
            summarize_end: include_str!("../../../resources/core/compaction/summarize-end.txt")
                .to_string(),
            rebuild: Some(crate::RebuildTexts {
                notes_files: include_str!("../../../resources/core/compaction/notes-files.txt")
                    .to_string(),
                notes_files_more: include_str!(
                    "../../../resources/core/compaction/notes-files-more.txt"
                )
                .to_string(),
                notes_retrieve: include_str!(
                    "../../../resources/core/compaction/notes-retrieve.txt"
                )
                .to_string(),
                notes_too_large: include_str!(
                    "../../../resources/core/compaction/notes-too-large.txt"
                )
                .to_string(),
                restored_open: include_str!("../../../resources/core/compaction/restored-open.txt")
                    .to_string(),
                restored_close: include_str!(
                    "../../../resources/core/compaction/restored-close.txt"
                )
                .to_string(),
                notes_jobs: include_str!("../../../resources/core/compaction/notes-jobs.txt")
                    .to_string(),
                notes_job: include_str!("../../../resources/core/compaction/notes-job.txt")
                    .to_string(),
            }),
            summarize_system: Some(
                include_str!("../../../resources/core/compaction/summarize-system.txt").to_string(),
            ),
            shorten: Some(crate::ShortenTexts {
                truncated: include_str!("../../../resources/core/compaction/truncated.txt")
                    .to_string(),
                notes_uncovered: include_str!(
                    "../../../resources/core/compaction/notes-uncovered.txt"
                )
                .to_string(),
            }),
        }),
        jobs: Some(jobs()),
    }
}

/// 出厂的两种回报的写法（施工 7-2）。
fn jobs() -> crate::JobTexts {
    crate::JobTexts {
        command_open: include_str!("../../../resources/core/jobs/command-open.txt").to_string(),
        command_exit: include_str!("../../../resources/core/jobs/command-exit.txt").to_string(),
        command_signal: include_str!("../../../resources/core/jobs/command-signal.txt").to_string(),
        command_duration: include_str!("../../../resources/core/jobs/command-duration.txt")
            .to_string(),
        command_output: include_str!("../../../resources/core/jobs/command-output.txt").to_string(),
        command_close: include_str!("../../../resources/core/jobs/command-close.txt").to_string(),
        subagent_open: include_str!("../../../resources/core/jobs/subagent-open.txt").to_string(),
        subagent_person: include_str!("../../../resources/core/jobs/subagent-person.txt")
            .to_string(),
        subagent_truncated: include_str!("../../../resources/core/jobs/subagent-truncated.txt")
            .to_string(),
        subagent_silent: include_str!("../../../resources/core/jobs/subagent-silent.txt")
            .to_string(),
        subagent_close: include_str!("../../../resources/core/jobs/subagent-close.txt").to_string(),
        subagent_omitted: include_str!("../../../resources/core/jobs/subagent-omitted.txt")
            .to_string(),
        stopped_by_user: include_str!("../../../resources/core/jobs/stopped-by-user.txt")
            .to_string(),
        subagent_message_open: include_str!(
            "../../../resources/core/jobs/subagent-message-open.txt"
        )
        .to_string(),
        subagent_message_close: include_str!(
            "../../../resources/core/jobs/subagent-message-close.txt"
        )
        .to_string(),
    }
}

/// 软件工程师的快照，有人能确认。
pub(crate) fn engineer() -> Snapshot {
    compose(
        "engineer",
        Sources {
            core: core(),
            persona: PersonaTexts {
                persona: include_str!("../../../resources/personas/engineer/prompts/persona.md")
                    .to_string(),
            },
        },
        true,
    )
}
