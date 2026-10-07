//! 测试共用的几样：出厂的十三份原文（资源目录里的真文件，`include_str!` 读进来：资源改一个字，测试拼出来的跟着变），
//! 一次典型的判断。

use super::{Ask, JudgeSources, JudgeTexts, Mode};

/// 出厂的 `reason` 上限：500 个字符（图纸 `chat.md` 第六条；代码里不写死，由外面交进来）。
pub(crate) const REASON_CHARS: usize = 500;

/// 资源 `software/onebot/judge/` 下的一份原文。
macro_rules! judge {
    ($name:literal) => {
        include_str!(concat!(
            "../../../../resources/software/onebot/judge/",
            $name
        ))
    };
}
pub(crate) use judge;

/// 出厂的十三份原文。
pub(crate) fn texts() -> JudgeTexts {
    JudgeTexts::new(JudgeSources {
        system: judge!("system.txt").to_string(),
        persona_open: judge!("persona-open.txt").to_string(),
        persona_close: judge!("persona-close.txt").to_string(),
        reply: judge!("reply.txt").to_string(),
        moderation_only: judge!("moderation-only.txt").to_string(),
        violations: judge!("violations.txt").to_string(),
        answer: judge!("answer.txt").to_string(),
        records_open: judge!("records-open.txt").to_string(),
        records_close: judge!("records-close.txt").to_string(),
        current_open: judge!("current-open.txt").to_string(),
        current_close: judge!("current-close.txt").to_string(),
        decoded_open: judge!("decoded-open.txt").to_string(),
        decoded_close: judge!("decoded-close.txt").to_string(),
    })
    .expect("出厂的模板合写法")
}

/// 一次典型的打分：不带人格、没有 base64，记录两行（末尾带换行），这一条一行（末尾不带），门槛 7。
pub(crate) fn ask() -> Ask {
    Ask {
        persona: None,
        records: "[1] 阿明: 晚上吃什么\n[2] 小红: 火锅？\n".to_string(),
        current: "[3] 阿明: @Miyu 你想吃什么".to_string(),
        decoded: None,
        mode: Mode::Reply,
        severity_min: 7,
    }
}
