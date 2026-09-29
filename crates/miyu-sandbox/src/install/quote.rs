//! 起提升过的自己时，参数拼成一行（`docs/blueprint/sandbox/windows.md`「怎么走」第 3 条）。
//!
//! `ShellExecuteExW` 收的是一整行字，被起的程序照 C 运行库（`CommandLineToArgvW`）的规矩拆回参数。拼的时候照同一套
//! 规矩：有空格、制表符的，还有空的，整个包进引号；引号前加一个反斜杠，引号前原有的 n 个反斜杠变成 2n 个；包起来的
//! 结尾那几个反斜杠也加倍，免得把收尾的引号转义掉。别处的反斜杠照原样（Windows 的路径里全是它）。
//!
//! 拼的是 UTF-16：Windows 的路径可能有落单的代理项，转成 `String` 会丢，照原样一个单元一个单元地拼。

use std::io;

/// 反斜杠。
const BACKSLASH: u16 = b'\\' as u16;
/// 双引号。
const QUOTE: u16 = b'"' as u16;
/// 空格。
const SPACE: u16 = b' ' as u16;
/// 制表符。
const TAB: u16 = b'\t' as u16;

/// 几个参数拼成一行，中间一个空格。
///
/// # Errors
///
/// 有参数里带着 NUL：整行交给系统时 NUL 就是结尾，后面的会被截掉，拼不出来。
pub(crate) fn command_line(args: &[Vec<u16>]) -> io::Result<Vec<u16>> {
    let mut line = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an argument contains NUL",
            ));
        }
        if i > 0 {
            line.push(SPACE);
        }
        append(&mut line, arg);
    }
    Ok(line)
}

/// 把一个参数照规矩接到 `line` 后面。
fn append(line: &mut Vec<u16>, arg: &[u16]) {
    let quoted = arg.is_empty() || arg.iter().any(|&unit| unit == SPACE || unit == TAB);
    if quoted {
        line.push(QUOTE);
    }
    let mut backslashes = 0;
    for &unit in arg {
        if unit == BACKSLASH {
            backslashes += 1;
        } else {
            if unit == QUOTE {
                // 前面已经原样写了 n 个反斜杠，再补 n 个，加上转义引号的那一个。
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes + 1));
            }
            backslashes = 0;
        }
        line.push(unit);
    }
    if quoted {
        // 结尾的 n 个反斜杠补成 2n 个：收尾的引号才不会被当成字面的引号。
        line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
        line.push(QUOTE);
    }
}

#[cfg(test)]
mod tests;
