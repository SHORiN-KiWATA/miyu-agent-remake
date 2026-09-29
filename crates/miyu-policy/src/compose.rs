//! 拼快照（`docs/designs/26-提示词.md` 第四节「怎么拼」）：system 照第四节的先后排，每一块去掉末尾的
//! 空白，块和块之间空一行，没有的块不留空行。
//!
//! 施工 3-6（上）时只有人设。别的块跟着各自的功能来，按 J12 先实测证明不加不行：核心的两行规则
//! （检查点、权限）2026-09-27 项目主人定先不拼，到 M6、M4 实测再定；场所说明也等实测出需要再加。

use crate::snapshot::{CompactionNumbers, CoreTexts, Snapshot};

/// 有计划的重启打断了一轮，再起来时连着接着干几次：`02-内核.md` 第六节「载入、崩溃、重启」的初值。
const RESUMES: u32 = 3;

/// 压缩用的数的出厂值（`compaction.md`「对外的样子」）：输出预留的上限 20000、余量 13000（照 Claude Code），
/// 一张图、一个文件各算 2000。
const COMPACTION: CompactionNumbers = CompactionNumbers {
    reserve_cap: 20_000,
    margin: 13_000,
    image: 2_000,
    file: 2_000,
};

/// 读好的原文：随核心附带的字，和这个人格的字。执行器从资源目录读（`miyu-store` 的资源目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// 随核心附带的字（`resources/core/`）。
    pub core: CoreTexts,
    /// 这个人格的字。
    pub persona: PersonaTexts,
}

/// 一个人格的字（`resources/personas/<编号>/prompts/`）。3-6（上）只有人设；示范对话、角色扮演
/// 提示随 3-6（下）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaTexts {
    /// 人设（`persona.md`）。
    pub persona: String,
}

/// 照 `26-提示词.md` 第四节拼出人格 `persona` 的快照。`attended` 是这个场所有没有人能确认。
pub fn compose(persona: &str, sources: Sources, attended: bool) -> Snapshot {
    Snapshot {
        persona: persona.to_string(),
        system: system(&[&sources.persona.persona]),
        tools: Vec::new(),
        core: sources.core,
        step_limit: None,
        attended,
        resumes: RESUMES,
        compaction: Some(COMPACTION),
    }
}

/// system：照先后，每一块去掉末尾的空白，没有的块不留空行，块和块之间空一行。
fn system(pieces: &[&str]) -> String {
    pieces
        .iter()
        .map(|piece| piece.trim_end())
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_are_trimmed_and_joined_with_a_blank_line() {
        assert_eq!(system(&["人设\n", "", "  \n", "场所\n\n"]), "人设\n\n场所");
        assert_eq!(
            system(&["You are a helpful software engineer.\n"]),
            "You are a helpful software engineer."
        );
        assert_eq!(system(&[]), "");
        // 开头的空白是人格自己写的，照留。
        assert_eq!(system(&["  缩进的第一行\n"]), "  缩进的第一行");
    }
}
