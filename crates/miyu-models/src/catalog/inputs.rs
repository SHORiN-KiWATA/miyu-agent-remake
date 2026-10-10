//! 认得的几种输入（施工 V-2 下）：几个开关，不存字。原来每个模型存一份 `Vec<String>`，八千多个模型就是两万多次分配。

/// 认得的几种，照这个先后。
const KINDS: [&str; 5] = ["text", "image", "pdf", "audio", "video"];

/// 一个模型能收的输入里认得的几种：`text`、`image`、`pdf`、`audio`、`video`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Inputs(u8);

impl Inputs {
    /// 照写了的几种算，不认识的不理。
    pub fn of<'a>(written: impl IntoIterator<Item = &'a str>) -> Inputs {
        let mut bits = 0;
        for name in written {
            if let Some(at) = KINDS.iter().position(|kind| *kind == name) {
                bits |= 1 << at;
            }
        }
        Inputs(bits)
    }

    /// 能收 `kind` 这一种。
    pub fn has(self, kind: &str) -> bool {
        KINDS
            .iter()
            .position(|known| *known == kind)
            .is_some_and(|at| self.0 & (1 << at) != 0)
    }

    /// 能收的几种，照 `text`、`image`、`pdf`、`audio`、`video` 的先后。
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        KINDS
            .iter()
            .enumerate()
            .filter(move |(at, _)| self.0 & (1 << at) != 0)
            .map(|(_, kind)| *kind)
    }

    /// 一种都没有。
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}
