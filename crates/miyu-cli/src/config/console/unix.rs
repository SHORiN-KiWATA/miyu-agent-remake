//! Unix 上关掉回显读一行（施工 8-5 补，`docs/blueprint/cli/login.md`「怎么走」第 4 条）：存下标准输入的 termios，
//! 关 `ECHO`（回显）、`ISIG`（信号键）、`ICANON`（行缓冲），自己一个字节一个字节读（[`read_line`]）。
//!
//! 关了 `ISIG`，`Ctrl+C` 不再触发 `SIGINT`——它变成普通的一个字节（`0x03`），我们自己认出来当取消；进程不会被信号
//! 直接打断，`hidden::read_line` 返回以后、不管走哪条路，[`Guard`] 的 `Drop` 都会把 termios 照原样写回去，`panic` 也算
//! （见它自己的测试）。
//!
//! 只有 `typed()` 是真的时才会调用这里（标准输入是终端），所以直接操作 `io::stdin()`，不用像 `rpassword` 以前那样另开
//! `/dev/tty`：这份设计里标准输入不是终端的那一半（管道）走的是 [`super::Console::all`]，不会走到这里。

use std::io::{self, Read};
use std::os::fd::AsFd;

use rustix::termios::{self, LocalModes, OptionalActions, SpecialCodeIndex};

use super::hidden::{Step, read_line};

/// 关掉回显读一行。
pub(super) fn read_hidden() -> io::Result<Option<String>> {
    let stdin = io::stdin();
    let _guard = Guard::enter(&stdin)?;
    let mut lock = stdin.lock();
    read_line(|| {
        let mut byte = [0u8; 1];
        match lock.read(&mut byte)? {
            0 => Ok(Step::Eof),
            _ => Ok(Step::Byte(byte[0])),
        }
    })
}

/// 存下 `fd` 原来的 termios，关回显、信号键、行缓冲；丢掉的时候（哪条路出去都算，包括 `panic`）照原样写回去。
struct Guard<'a, Fd: AsFd> {
    fd: &'a Fd,
    original: termios::Termios,
}

impl<'a, Fd: AsFd> Guard<'a, Fd> {
    /// 读一遍原来的设置，换上一个字节一个字节读用的设置：不回显、`Ctrl+C` 不触发信号、不等回车就能读到、读一个字节
    /// 不等（`VMIN = 1`、`VTIME = 0`）。
    ///
    /// # Errors
    ///
    /// `fd` 不是终端，或者改不了它的设置。
    fn enter(fd: &'a Fd) -> io::Result<Guard<'a, Fd>> {
        let original = termios::tcgetattr(fd)?;
        let mut raw = original.clone();
        raw.local_modes
            .remove(LocalModes::ECHO | LocalModes::ISIG | LocalModes::ICANON);
        raw.special_codes[SpecialCodeIndex::VMIN] = 1;
        raw.special_codes[SpecialCodeIndex::VTIME] = 0;
        termios::tcsetattr(fd, OptionalActions::Now, &raw)?;
        Ok(Guard { fd, original })
    }
}

impl<Fd: AsFd> Drop for Guard<'_, Fd> {
    fn drop(&mut self) {
        if let Err(error) = termios::tcsetattr(self.fd, OptionalActions::Now, &self.original) {
            tracing::warn!(
                target: "miyu::cli",
                error = %error,
                "terminal settings not restored after reading a hidden key",
            );
        }
    }
}

/// `Guard` 本身的单元测试：开一对真的伪终端，不用真进程、不用真人按键（施工 8-5 补，验收单「读到一半出错、panic」
/// 那一条）。整段读取流程（认 `Ctrl+C`、取消以后印什么、退出码）的测试在 `crates/miyu/tests/login_tty.rs`，要一个
/// 真的子进程。
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::panic;

    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};

    use super::*;

    /// 开一对真的伪终端，交回主端（用来问从端现在的 termios）和从端（给 [`Guard`] 用）。
    fn pair() -> (std::fs::File, std::fs::File) {
        let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("开得起伪终端");
        grantpt(&main).expect("放得开");
        unlockpt(&main).expect("解得开");
        let name = ptsname(&main, Vec::new()).expect("问得到名字");
        let name = name.to_str().expect("是 UTF-8");
        let secondary = OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .expect("开得起从端");
        (std::fs::File::from(main), secondary)
    }

    #[test]
    fn enter_turns_off_echo_isig_icanon_and_drop_restores_them() {
        let (_main, secondary) = pair();
        let before = termios::tcgetattr(&secondary).expect("读得到");
        assert!(before.local_modes.contains(LocalModes::ECHO));
        assert!(before.local_modes.contains(LocalModes::ISIG));
        assert!(before.local_modes.contains(LocalModes::ICANON));
        {
            let _guard = Guard::enter(&secondary).expect("进得去");
            let raw = termios::tcgetattr(&secondary).expect("读得到");
            assert!(!raw.local_modes.contains(LocalModes::ECHO));
            assert!(!raw.local_modes.contains(LocalModes::ISIG));
            assert!(!raw.local_modes.contains(LocalModes::ICANON));
        }
        let after = termios::tcgetattr(&secondary).expect("读得到");
        assert!(after.local_modes.contains(LocalModes::ECHO));
        assert!(after.local_modes.contains(LocalModes::ISIG));
        assert!(after.local_modes.contains(LocalModes::ICANON));
    }

    #[test]
    fn drop_restores_even_when_a_panic_unwinds_through_it() {
        let (_main, secondary) = pair();
        let unwound = panic::catch_unwind(|| {
            let _guard = Guard::enter(&secondary).expect("进得去");
            panic!("半路出错，守卫得照样写回");
        });
        assert!(unwound.is_err(), "panic 该被 catch_unwind 接住");
        // 只查三个关心的位，不比对整份 `local_modes`：macOS（BSD 的 termios）在关、开 `ICANON` 这样切过一轮以后，
        // 会自己多标一个 `PENDIN`（有要重打一遍的输入），这是内核自己的记账，不是 `Guard` 该管、也不是它该照原样
        // 写回的那几个位（CI 的 macOS 上实测到的，2026-10-02）。
        let after = termios::tcgetattr(&secondary).expect("读得到");
        assert!(after.local_modes.contains(LocalModes::ECHO));
        assert!(after.local_modes.contains(LocalModes::ISIG));
        assert!(after.local_modes.contains(LocalModes::ICANON));
    }
}
