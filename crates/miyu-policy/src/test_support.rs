//! 测试的夹具：出厂的随核心附带的字、软件工程师的快照。随核心附带的字用仓库里出厂的那一份（编译时
//! 拿进来，不是读文件）。

use crate::compose::{PersonaTexts, Sources, compose};
use crate::snapshot::{
    CoreTexts, DriverPlaceholders, FactTexts, Snapshot, ToolResultTexts, TurnEndedTexts,
};

/// 出厂的随核心附带的字。
pub(crate) fn core() -> CoreTexts {
    CoreTexts {
        checkpoint_open: include_str!("../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../resources/core/checkpoint-close.txt").to_string(),
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
            restarted: include_str!("../../../resources/core/tool-results/restarted.txt")
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
        },
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
