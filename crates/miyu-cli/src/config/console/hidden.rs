//! 关掉回显读一行时，把一个字节一个字节的流认成「这一行」还是「取消了」：两个平台共用，各自只管怎么开关终端的
//! 设置、怎么读下一个字节（施工 8-5 补，`docs/blueprint/cli/login.md`「怎么走」第 4 条）。
//!
//! 不关心这一行的字从哪来、键盘上敲的字节怎么到这里：那是 `unix`、`windows` 两个子模块的事，这里只认字节。

use std::io;

/// 读下一个字节的结局。
pub(super) enum Step {
    /// 读到了这个字节。
    Byte(u8),
    /// 读到头了：这一头关了，再也不会有字节了。
    Eof,
}

/// 从 `next` 一个字节一个字节地攒成一行：
/// - 回车（`\r`、`\n`）结束，交回攒到的那一截（可能是空的，去掉前后空白是调用的人自己的事）；
/// - `Ctrl+C`（`0x03`）随时取消；
/// - `Ctrl+D`（`0x04`）只在还没攒到字的时候当取消，攒了字的当没按（标准终端下 `Ctrl+D` 本来也只在空行退出）；
/// - 退格（`DEL` `0x7f`、`BS` `0x08`）删掉上一个字节；
/// - 读到头：当什么都没收到（和没攒到字时 `Ctrl+D` 不是一回事：这是这一头真的关了，不是人按的）。
///
/// # Errors
///
/// `next` 出错；取消了（[`io::ErrorKind::Interrupted`]）；攒到的字节不是 UTF-8。
pub(super) fn read_line(mut next: impl FnMut() -> io::Result<Step>) -> io::Result<Option<String>> {
    let mut buffer = Vec::new();
    loop {
        match next()? {
            Step::Eof => return Ok(None),
            Step::Byte(b'\r' | b'\n') => {
                return String::from_utf8(buffer)
                    .map(Some)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
            }
            Step::Byte(0x03) => return Err(cancelled()),
            Step::Byte(0x04) if buffer.is_empty() => return Err(cancelled()),
            Step::Byte(0x04) => {}
            Step::Byte(0x7f | 0x08) => {
                buffer.pop();
            }
            Step::Byte(byte) => buffer.push(byte),
        }
    }
}

/// 取消了：按了 `Ctrl+C`，或者还没攒到字时按了 `Ctrl+D`。调用的人认 `kind()`，不认这句话。
pub(super) fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "key paste cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 喂一串写死的字节，一个一个交出去。
    fn bytes(input: &[u8]) -> impl FnMut() -> io::Result<Step> {
        let mut input = input.iter();
        move || match input.next() {
            Some(byte) => Ok(Step::Byte(*byte)),
            None => Ok(Step::Eof),
        }
    }

    #[test]
    fn enter_ends_the_line_cr_and_lf_both_work() {
        assert_eq!(
            read_line(bytes(b"sk-FAKE\n")).unwrap(),
            Some("sk-FAKE".to_string())
        );
        assert_eq!(
            read_line(bytes(b"sk-FAKE\r")).unwrap(),
            Some("sk-FAKE".to_string())
        );
    }

    #[test]
    fn enter_on_an_empty_line_is_an_empty_string_not_cancelled() {
        assert_eq!(read_line(bytes(b"\n")).unwrap(), Some(String::new()));
    }

    #[test]
    fn ctrl_c_cancels_whether_the_line_is_empty_or_not() {
        let error = read_line(bytes(b"\x03")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        let error = read_line(bytes(b"sk-FAKE\x03")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn ctrl_d_on_an_empty_line_cancels() {
        let error = read_line(bytes(b"\x04")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn ctrl_d_after_typing_something_is_ignored_not_cancelled() {
        assert_eq!(
            read_line(bytes(b"sk-\x04FAKE\n")).unwrap(),
            Some("sk-FAKE".to_string())
        );
    }

    #[test]
    fn backspace_deletes_the_last_byte_del_and_bs_both_work() {
        assert_eq!(
            read_line(bytes(b"sk-FAKEX\x7f\n")).unwrap(),
            Some("sk-FAKE".to_string())
        );
        assert_eq!(
            read_line(bytes(b"sk-FAKEX\x08\n")).unwrap(),
            Some("sk-FAKE".to_string())
        );
    }

    #[test]
    fn backspace_on_an_empty_line_does_nothing_to_underflow() {
        assert_eq!(
            read_line(bytes(b"\x7f\x7fsk\n")).unwrap(),
            Some("sk".to_string())
        );
    }

    #[test]
    fn eof_with_nothing_typed_is_none() {
        assert_eq!(read_line(bytes(b"")).unwrap(), None);
    }

    #[test]
    fn eof_mid_line_is_none_not_the_partial_line() {
        assert_eq!(read_line(bytes(b"sk-FAKE")).unwrap(), None);
    }

    #[test]
    fn invalid_utf8_is_an_error_not_a_panic() {
        let error = read_line(bytes(&[0xff, 0xfe, b'\n'])).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
