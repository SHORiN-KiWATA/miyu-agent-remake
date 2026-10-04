//! 鼠标指针的样子（蓝图 `tui.md`「对外的样子」鼠标那一张表）：悬停在链接上变成手形，移开变回来。
//! 用 OSC 22，kitty、foot、ghostty 认；不认的终端当没有。

use std::io::{self, Write};

/// 现在是不是手形。只在变了的时候发，不每帧都发。
#[derive(Debug, Default)]
pub struct Pointer {
    hand: bool,
}

impl Pointer {
    /// 照这一帧该不该是手形改；变了才写。
    ///
    /// # Errors
    ///
    /// 写不出去。
    pub fn set(&mut self, hand: bool, out: &mut impl Write) -> io::Result<()> {
        if hand != self.hand {
            self.hand = hand;
            write!(out, "{}", shape(hand))?;
        }
        Ok(())
    }
}

/// 退出时变回默认的样子。
///
/// # Errors
///
/// 写不出去。
pub fn reset(out: &mut impl Write) -> io::Result<()> {
    write!(out, "{}", shape(false))
}

fn shape(hand: bool) -> &'static str {
    if hand {
        "\x1b]22;pointer\x1b\\"
    } else {
        "\x1b]22;default\x1b\\"
    }
}

#[cfg(test)]
mod tests {
    use super::Pointer;

    #[test]
    fn only_changes_are_written() {
        let mut pointer = Pointer::default();
        let mut out = Vec::new();
        pointer.set(false, &mut out).unwrap();
        assert!(out.is_empty(), "本来就不是手形，不发");
        pointer.set(true, &mut out).unwrap();
        pointer.set(true, &mut out).unwrap();
        assert_eq!(out, b"\x1b]22;pointer\x1b\\");
        out.clear();
        pointer.set(false, &mut out).unwrap();
        assert_eq!(out, b"\x1b]22;default\x1b\\");
    }
}
