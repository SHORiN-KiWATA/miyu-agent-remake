//! 命令的输出给人看之前洗一遍（蓝图「后台命令、子代理和侧边栏」第 3、5 条）：终端转义序列（颜色、光标移动、标题）去掉；
//! 一行里回车覆盖的只留最后一段（进度条）；制表符换成空格；别的控制字符去掉。

use std::iter::Peekable;
use std::str::Chars;

/// 洗过的输出；`tab` 是制表符换成几个空格。
pub fn clean(text: &str, tab: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut line = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => skip_escape(&mut chars),
            '\n' => {
                out.push_str(&line);
                out.push('\n');
                line.clear();
            }
            // `\r\n` 照换行；单独的回车是这一行从头重写（进度条）。
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => line.clear(),
            '\t' => line.push_str(&" ".repeat(tab)),
            c if c.is_control() => {}
            c => line.push(c),
        }
    }
    out.push_str(&line);
    out
}

/// 结束了的命令，正文里那一行点开看的全文：洗过、去掉末尾的空行；前面还有没交的，第一行写 `…`；一个字都没有的写
/// `nothing`（「没有输出」）。
pub fn note_detail(output: &crate::core::JobOutput, tab: usize, nothing: &str) -> String {
    let text = clean(&output.text, tab);
    let text = text.trim_end();
    match (text.is_empty(), output.truncated) {
        (true, _) => nothing.to_string(),
        (false, true) => format!("…\n{text}"),
        (false, false) => text.to_string(),
    }
}

/// 跳过 `ESC` 后面的一段：CSI（`ESC [` 到结尾那个字）、OSC（`ESC ]` 到 `BEL` 或 `ESC \`），别的只跳一个字。
fn skip_escape(chars: &mut Peekable<Chars>) {
    match chars.next() {
        Some('[') => {
            for c in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&c) {
                    break;
                }
            }
        }
        Some(']') => {
            while let Some(c) = chars.next() {
                if c == '\x07' {
                    break;
                }
                if c == '\x1b' {
                    chars.next();
                    break;
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{clean, note_detail};
    use crate::core::JobOutput;

    #[test]
    fn an_ended_command_opens_on_its_cleaned_output() {
        let out = |text: &str, truncated| JobOutput {
            text: text.into(),
            lines: 0,
            running: false,
            truncated,
        };
        assert_eq!(
            note_detail(&out("\x1b[1mok\x1b[0m\n\n", false), 4, "没有输出"),
            "ok"
        );
        assert_eq!(
            note_detail(&out("tail\n", true), 4, "没有输出"),
            "…\ntail",
            "前面还有没交的"
        );
        assert_eq!(note_detail(&out("\n", false), 4, "没有输出"), "没有输出");
    }

    #[test]
    fn escapes_carriage_returns_and_tabs_are_cleaned() {
        assert_eq!(
            clean("\x1b[31merror\x1b[0m: x\n", 4),
            "error: x\n",
            "颜色去掉"
        );
        assert_eq!(
            clean("10%\r50%\r100%\ndone", 4),
            "100%\ndone",
            "进度条只留最后一段"
        );
        assert_eq!(clean("a\r\nb\r\n", 4), "a\nb\n", "\\r\\n 照换行");
        assert_eq!(clean("a\tb", 2), "a  b");
        assert_eq!(
            clean("\x1b]0;title\x07ok\x1b]8;;http://x\x1b\\link", 4),
            "oklink",
            "标题、链接去掉"
        );
        assert_eq!(
            clean("bell\x07 back\x08", 4),
            "bell back",
            "别的控制字符去掉"
        );
    }
}
