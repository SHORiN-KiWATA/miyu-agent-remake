//! 试玩台这一个会话的策略（`docs/construction/3-5-试玩台（补）.md`）：出厂的组装、事实模板、写给
//! 模型的句子，都从资源目录编进来；工具一件都没有；system 是占位的一句，`--system` 可以换。
//!
//! 真的策略快照（人格、预设、提示词拼好，按哈希存档）是 3-6 的事，到时候换掉这里。

use std::collections::BTreeMap;

use miyu_assemble::{DefaultAssembler, Stable, Texts, TurnEndedTexts};
use miyu_drivers::openai_chat::{Compat, ReasoningField, ReasoningReplay};
use miyu_drivers::{DriverTextSources, DriverTexts};
use miyu_kernel::facts::FactTemplates;
use miyu_kernel::session::Policy;
use miyu_kernel::tool::{ToolTextSources, ToolTexts};

/// 占位的 system，3-6 换成人格的。
pub const SYSTEM: &str = "You are a helpful assistant.";

/// 这一个会话的策略：`system` 进稳定区，工具面是空的，回合不限步数（没有工具，一轮只请求一次）。
///
/// # Panics
///
/// 实际不会 panic：出厂的模板和句子编进来之前，各自的测试已经试过换得了。
pub fn policy(system: &str) -> Policy {
    let stable = Stable {
        tools: Vec::new(),
        system: system.to_string(),
        demos: Vec::new(),
    };
    Policy {
        assembler: Box::new(DefaultAssembler::new(stable, texts())),
        facts: FactTemplates::new(
            include_str!("../../../resources/core/facts/env.txt"),
            include_str!("../../../resources/core/facts/permission.txt"),
            include_str!("../../../resources/core/facts/reply-cut.txt"),
        )
        .expect("出厂的事实模板用得了"),
        tools: BTreeMap::new(),
        step_limit: None,
        tool_texts: tool_texts(),
        attended: true,
        resumes: 3,
    }
}

/// DeepSeek 的开关：每条 assistant 都带回 `reasoning_content`，输出上限写在 `max_tokens`，要最后
/// 一块的用量。
pub fn compat() -> Compat {
    Compat {
        reasoning: ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true,
        },
        ..Compat::default()
    }
}

/// 出厂的驱动占位：图片、文件发不了的时候写给模型的那几句。
///
/// # Panics
///
/// 实际不会 panic：出厂的占位编进来之前，驱动的测试已经试过。
pub fn driver_texts() -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../resources/core/drivers/tool-attachments-only.txt"
        ),
    })
    .expect("出厂的驱动占位用得了")
}

/// 出厂的英文：压缩检查点的开头结尾、回合怎么结束的那几句。
fn texts() -> Texts {
    Texts {
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
    }
}

/// 出厂的那几句：内核替工具写给模型的。试玩台没有工具，模型编了工具名才用得上。
fn tool_texts() -> ToolTexts {
    ToolTexts::new(ToolTextSources {
        unknown: include_str!("../../../resources/core/tool-results/unknown.txt"),
        not_an_object: include_str!("../../../resources/core/tool-results/not-an-object.txt"),
        cancelled_before: include_str!("../../../resources/core/tool-results/cancelled-before.txt"),
        cancelled_running: include_str!(
            "../../../resources/core/tool-results/cancelled-running.txt"
        ),
        skipped: include_str!("../../../resources/core/tool-results/skipped.txt"),
        read_only: include_str!("../../../resources/core/tool-results/read-only.txt"),
        denied: include_str!("../../../resources/core/tool-results/denied.txt"),
        denied_with_reason: include_str!(
            "../../../resources/core/tool-results/denied-with-reason.txt"
        ),
        unattended: include_str!("../../../resources/core/tool-results/unattended.txt"),
        question_interrupted: include_str!(
            "../../../resources/core/tool-results/question-interrupted.txt"
        ),
        question_voided: include_str!("../../../resources/core/tool-results/question-voided.txt"),
        question_unattended: include_str!(
            "../../../resources/core/tool-results/question-unattended.txt"
        ),
        restarted: include_str!("../../../resources/core/tool-results/restarted.txt"),
    })
    .expect("出厂的几句用得了")
}
