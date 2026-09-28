//! 测试程序自己当命令（照 Codex 的做法）：环境变量 [`WHAT`] 说做什么，照做；成了印 `child ok`，不成印
//! `child failed: <原话>`。平时跑测试时没有这个变量，它什么都不做。

use std::fs::File;
use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

/// 说做什么的环境变量：`tcp:<地址>`、`unix:<路径>`、`pair`、`listen`、`resolve:<名字>`、`fcntl-blocked:<路径>`。
pub const WHAT: &str = "MIYU_SANDBOX_CHILD";

#[test]
#[ignore = "由别的测试经助手起，平时不跑"]
fn child() {
    let Ok(what) = std::env::var(WHAT) else {
        return;
    };
    let (verb, arg) = what.split_once(':').unwrap_or((what.as_str(), ""));
    let done = match verb {
        "tcp" => TcpStream::connect(arg).map(drop),
        "unix" => UnixStream::connect(arg).map(drop),
        "pair" => UnixStream::pair().map(drop),
        "listen" => TcpListener::bind("127.0.0.1:0").map(drop),
        "resolve" => (arg, 80).to_socket_addrs().map(drop),
        "fcntl-blocked" => fcntl_blocked(arg),
        _ => panic!("不认得 {what}"),
    };
    match done {
        Ok(()) => println!("child ok"),
        Err(error) => println!("child failed: {error}"),
    }
}

/// 只读地打开 `path`，在这个只读的文件描述符上试那两个能改文件的 `fcntl`（`F_MAKECOMPRESSED` 80、
/// `F_TRANSFEREXTENTS` 110）：两个都被沙盒挡住（`EPERM`）才算成。沙盒先查，再看参数，所以参数给 0 就够。
#[allow(unsafe_code, reason = "调 libc 的 fcntl，下面写明为什么安全")]
fn fcntl_blocked(path: &str) -> io::Result<()> {
    let file = File::open(path)?;
    for command in [80, 110] {
        // SAFETY: `file` 活到函数结束，描述符一直有效；这两个命令只收一个整数参数，给的是 0。
        let result = unsafe { libc::fcntl(file.as_raw_fd(), command, 0) };
        let error = io::Error::last_os_error();
        if result != -1 || error.raw_os_error() != Some(libc::EPERM) {
            return Err(io::Error::other(format!(
                "fcntl {command} returned {result} ({error})"
            )));
        }
    }
    Ok(())
}
