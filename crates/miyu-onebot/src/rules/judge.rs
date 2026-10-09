//! 判官的说明（施工 O-23 下，`onebot.md` 第一条「场所规则和出厂数据」第 1、2 条）：资源 `software/onebot/judge/` 下的十三份
//! （`chat.md` 第六条，给模型看的字，登记在 `26-提示词.md` 第十节），桥起来时和别的出厂数据一起读，交群聊内核的
//! [`JudgeTexts::new`] 查。没有系统那一份：给模型看的字随包走。
//!
//! 文件名照 [`JudgeSources`] 的格名把 `_` 换成 `-`、加 `.txt`；问题里的文件名带 `judge/`，看得出是判官的那一份。

use std::path::Path;

use miyu_chat::{JudgeSources, JudgeTexts, Problem, Source};
use miyu_config::problem::Code;

use super::{files, required};

/// 判官的说明在出厂目录里的子目录。
const DIR: &str = "judge";

/// 写模板的那一份：[`JudgeTexts::new`] 不收的，问题记在它上面。
const VIOLATIONS: &str = "violations.txt";

/// 读出厂目录 `dir`（`software/onebot/`）里的判官的说明，查过交回。不在、读不成的每一份记一条问题；都读到了、`violations.txt`
/// 的模板写坏了（`JudgeTexts::new` 不收）的记一条 `bad_format`，原话是模板的错。有问题的交回空的。
pub(super) fn texts(dir: &Path, problems: &mut Vec<Problem>) -> Option<JudgeTexts> {
    let base = dir.join(DIR);
    let mut read = |file: &str| required(&base.join(file), &format!("{DIR}/{file}"), problems);
    // 先都读一遍再看缺没缺：缺几份就报几份。
    let read = [
        read("system.txt"),
        read("persona-open.txt"),
        read("persona-close.txt"),
        read("reply.txt"),
        read("moderation-only.txt"),
        read(VIOLATIONS),
        read("answer.txt"),
        read("records-open.txt"),
        read("records-close.txt"),
        read("current-open.txt"),
        read("current-close.txt"),
        read("decoded-open.txt"),
        read("decoded-close.txt"),
    ];
    let [
        Some(system),
        Some(persona_open),
        Some(persona_close),
        Some(reply),
        Some(moderation_only),
        Some(violations),
        Some(answer),
        Some(records_open),
        Some(records_close),
        Some(current_open),
        Some(current_close),
        Some(decoded_open),
        Some(decoded_close),
    ] = read
    else {
        return None;
    };
    let sources = JudgeSources {
        system,
        persona_open,
        persona_close,
        reply,
        moderation_only,
        violations,
        answer,
        records_open,
        records_close,
        current_open,
        current_close,
        decoded_open,
        decoded_close,
    };
    match JudgeTexts::new(sources) {
        Ok(texts) => Some(texts),
        Err(error) => {
            let file = format!("{DIR}/{VIOLATIONS}");
            problems.push(files::whole(
                Code::BadFormat,
                Source::Factory,
                &file,
                Some(error.to_string()),
            ));
            None
        }
    }
}
