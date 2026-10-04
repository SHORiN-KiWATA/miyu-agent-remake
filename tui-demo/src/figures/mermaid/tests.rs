//! 点开看的大图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：垫了底、只写一次、多了删最早的。

use super::zoom;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><text fill="#a9b1d6">A</text></svg>"##;

#[test]
fn the_zoomed_copy_is_floored_written_once_and_old_ones_are_dropped() {
    let dir = std::env::temp_dir().join(format!("miyu-tui-zoom-test-{}", std::process::id()));
    let backdrop = (0x1a, 0x1b, 0x26);
    let first = zoom(SVG, backdrop, &dir, 2).unwrap();
    let written = std::fs::read_to_string(&first).unwrap();
    // 底垫在最前面，字画在它上面。
    let rect = written.find(r##"fill="#1a1b26""##).unwrap();
    assert!(written.find("<svg").unwrap() < rect && rect < written.find("<text").unwrap());
    assert_eq!(zoom(SVG, backdrop, &dir, 2).unwrap(), first);
    for n in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        zoom(&SVG.replace('A', &format!("N{n}")), backdrop, &dir, 2).unwrap();
    }
    let left = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(left, 2, "只留最近的两张");
    assert!(!first.exists(), "最早的删了");
    assert!(zoom("不是图", backdrop, &dir, 2).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
