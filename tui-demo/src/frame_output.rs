//! 一帧输出的边界（蓝图 `tui.md`「每一帧」）：同步首尾、文字、图片和光标先一起准备，最后才送给终端。
//!
//! kitty 同步暂停期间的快照不含输入法预编辑。提前送出开头会让预编辑消失，露出底下的 tips。
//! 帧内的 flush 只完成内存准备；帧外仍直接写，查询、模式切换和通知不延迟。

// DEC 2026 的协议字节直接进缓冲，Windows 测具也不经控制台 API 绕过 writer。
const BEGIN: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";
use std::io::{self, Write};

/// 准备整帧时暂存终端字节；帧外直接写。只复用一帧的容量，不保存历史。
pub struct Output<W> {
    out: W,
    bytes: Vec<u8>,
    preparing: bool,
}

impl<W: Write> Output<W> {
    /// 包装一个终端输出端，初始没有待发送的帧。
    pub fn new(out: W) -> Self {
        Self {
            out,
            bytes: Vec::new(),
            preparing: false,
        }
    }

    /// 开始准备一帧，同步开头也只写进内存。
    ///
    /// # Errors
    /// 同步命令编码失败。
    pub fn begin(&mut self) -> io::Result<()> {
        self.preparing = true;
        self.write_all(BEGIN)
    }

    /// 送出包含同步首尾的完整帧，恢复帧外直写；短写由 write_all 接着写，错误照实返回。
    ///
    /// 即使画面准备失败也调用它，让已经准备的内容带上结尾。不能承诺操作系统一次传完大帧。
    ///
    /// # Errors
    /// 同步命令编码、输出或 flush 失败。
    pub fn finish(&mut self) -> io::Result<()> {
        self.write_all(END)?;
        self.preparing = false;
        let written = self.out.write_all(&self.bytes);
        self.bytes.clear();
        written?;
        self.out.flush()
    }
}

impl<W: Write> Write for Output<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.preparing {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        } else {
            self.out.write(bytes)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.preparing {
            Ok(())
        } else {
            self.out.flush()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Sink {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
    }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    #[test]
    fn preparing_a_frame_never_exposes_a_synchronized_pause() {
        let mut output = Output::new(Sink::default());
        output.begin().unwrap();
        output.write_all(b"text\n").unwrap();
        output.flush().unwrap();
        // 图片比通常的缓冲容量大；不能写满就中途送出去。
        let image = vec![b'x'; 100_000];
        output.write_all(&image).unwrap();
        output.flush().unwrap();
        assert!(output.out.bytes.is_empty(), "计算中不能先发同步开头");
        assert_eq!(output.out.flushes, 0);
        output.finish().unwrap();
        let mut expected = b"\x1b[?2026htext\n".to_vec();
        expected.extend_from_slice(&image);
        expected.extend_from_slice(b"\x1b[?2026l");
        assert_eq!(output.out.bytes, expected);
        assert_eq!(output.out.writes, 1);
        assert_eq!(output.out.flushes, 1);
    }

    #[test]
    fn frames_do_not_replay_and_outside_output_is_immediate() {
        let mut output = Output::new(Sink::default());
        for text in [b"one".as_slice(), b"two".as_slice()] {
            output.begin().unwrap();
            output.write_all(text).unwrap();
            output.finish().unwrap();
        }
        output.write_all(b"notification").unwrap();
        output.flush().unwrap();
        assert_eq!(
            output.out.bytes,
            b"\x1b[?2026hone\x1b[?2026l\x1b[?2026htwo\x1b[?2026lnotification"
        );
    }

    #[test]
    fn output_failure_is_reported() {
        struct Fails;
        impl Write for Fails {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut output = Output::new(Fails);
        output.begin().unwrap();
        output.write_all(b"frame").unwrap();
        assert_eq!(
            output.finish().unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
