use std::ffi::OsStr;

use super::colored;

#[test]
fn no_color_counts_only_when_set_and_not_empty() {
    // 终端上：没设、设成空的上色；设了不空的不上色，写什么都一样（no-color.org）。
    assert!(colored(true, None));
    assert!(colored(true, Some(OsStr::new(""))));
    assert!(!colored(true, Some(OsStr::new("1"))));
    assert!(!colored(true, Some(OsStr::new("0"))));
    // 不是终端的从不上色。
    assert!(!colored(false, None));
    assert!(!colored(false, Some(OsStr::new(""))));
}

#[test]
fn plain_segments_carry_no_color_and_colored_lines_end_in_a_reset() {
    use super::{Ink, Line};
    // 整行原色的：一个控制序列都没有，上不上色都一样。
    let plain = Line::inked(Ink::Plain, "$ ls");
    assert_eq!(plain.paint(true), "$ ls\n");
    assert_eq!(plain.paint(false), "$ ls\n");
    // 整行灰的：和以前一样，包在灰色里。
    assert_eq!(Line::gray("· x").paint(true), "\x1b[90m· x\x1b[0m\n");
    // 原色接灰的、灰的接回原色：换回原色写 ESC[0m，上过色的行尾再写一个。
    let mut mixed = Line::inked(Ink::Plain, "a");
    mixed.push(Ink::Gray, " · b");
    mixed.push(Ink::Plain, " c");
    assert_eq!(mixed.paint(true), "a\x1b[90m · b\x1b[0m c\x1b[0m\n");
    assert_eq!(mixed.paint(false), "a · b c\n");
}
