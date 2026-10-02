//! `miyu login` 在真的伪终端里（施工 8-5 补，`docs/blueprint/cli/login.md`「怎么走」第 4 条）：贴 key 的时候按
//! `Ctrl+C`，进程以 130 退出、印了取消那一句，结束后终端设置和开始前一样（回显开着）；送正常的 key 加回车，读到的对，
//! 终端设置也照原样。只在 Unix 上跑：Windows 没有伪终端，`Guard`、控制台模式的读写写在一个单元测试里，在
//! `crates/miyu-cli/src/config/console/windows.rs`。
//!
//! `miyu setup` 贴 key 那一步取消的测试（配置文件、密钥文件都没动）不需要真的伪终端：那条逻辑不碰终端的设置，照
//! 剧本回的假终端在 `crates/miyu-cli/tests/setup.rs` 的 `cancelling_the_key_paste_writes_nothing` 测过了。

#![cfg(unix)]

mod support;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{self, LocalModes};

use miyu_ipc::connect_or_start;
use support::{Home, MIYU, offline, resources, within};

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

/// 一对真的伪终端：主端（测试自己留着写按键）和从端的路径（给子进程的标准输入、输出、错误各开一份；查 termios 也
/// 走从端——见 [`echoing`]）。
fn pty() -> (File, String) {
    let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("开得起伪终端");
    grantpt(&main).expect("放得开");
    unlockpt(&main).expect("解得开");
    let name = ptsname(&main, Vec::new()).expect("问得到名字");
    let name = name.to_str().expect("是 UTF-8").to_string();
    (File::from(main), name)
}

/// 再开一份从端：子进程的标准输入、输出、错误各自独立的一份文件描述符（和真的终端一样）。
fn secondary(path: &str) -> Stdio {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("开得起从端");
    Stdio::from(file)
}

/// 这一对现在的回显、信号键、行缓冲是不是都开着（伪终端刚建好、谁都没按过 `miyu login` 之前的样子）。照从端的路径
/// 另开一份短命的文件描述符来问：Linux 上主端、从端的 termios 是同一份，问哪一头都一样；macOS（BSD 的 pty）主端不认
/// `tcgetattr` 这个 ioctl（`ENOTTY`，CI 的 macOS 上跑出来的，2026-10-02），只有从端认。
fn echoing(slave: &str) -> bool {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(slave)
        .expect("开得起从端");
    let modes = termios::tcgetattr(&file).expect("读得到").local_modes;
    modes.contains(LocalModes::ECHO)
        && modes.contains(LocalModes::ISIG)
        && modes.contains(LocalModes::ICANON)
}

/// 主端边写边收：开一个专门的线程一直阻塞着读，不是等子进程退出以后再读一轮——从端的最后一份文件描述符关掉的
/// 瞬间，macOS（BSD 的 pty）会把这时候还没读走的字节直接丢掉，子进程退出前最后写的那一句因此收不全（CI 上
/// 实测到的，2026-10-02）。这个线程自己在从端全关、读到错误的时候结束，不用另外叫它停。
struct Capture {
    buffer: Arc<Mutex<Vec<u8>>>,
    reader: JoinHandle<()>,
}

impl Capture {
    /// 开始收：`main` 另外克隆一份文件描述符交给读的线程，原来这一份留给调用的人写按键。
    fn start(main: &File) -> Capture {
        let read_end = main.try_clone().expect("克隆得出读的那一份");
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reader = {
            let buffer = Arc::clone(&buffer);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match (&read_end).read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => buffer
                            .lock()
                            .expect("锁没中毒")
                            .extend_from_slice(&chunk[..n]),
                        // 从端全关了：Linux 上这是 EIO，不是干净的 EOF；当读到头，没有更多字节了。
                        Err(_) => break,
                    }
                }
            })
        };
        Capture { buffer, reader }
    }

    /// 现在攒到的字，去掉伪终端自己加的 `\r`（默认开着 `ONLCR`，写出去的每个 `\n` 在从端变成 `\r\n`）。
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().expect("锁没中毒")).replace('\r', "")
    }

    /// 一直等，直到攒到的字里有 `needle`：给子进程时间印出「粘贴 ... 的 key（不显示）：」这一句，再送按键。最多
    /// 一分钟（CI 的机器可能慢），到点还没见到就说清楚现在攒到了什么。
    fn wait_for(&self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let text = self.text();
            if text.contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "一分钟内没等到「{needle}」，现在攒到的是：{text}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// 子进程走完，交回它的退出码和收到的全部字。读的线程这时该已经自己结束了（从端的三份文件描述符都是子进程的，
    /// 它一退出、读的线程马上也会读到「从端全关了」收尾），等它一下确保真的收全了。
    fn finish(self, child: &mut std::process::Child) -> (i32, String) {
        let status = child.wait().expect("等得到");
        let buffer = Arc::clone(&self.buffer);
        self.reader.join().expect("读的线程没 panic");
        let text = String::from_utf8_lossy(&buffer.lock().expect("锁没中毒")).replace('\r', "");
        (status.code().expect("Unix 上有退出码"), text)
    }
}

/// 起核心，交回数据根；核心起来以后再建伪终端，免得核心自己的日志混进屏幕（施工 8-5 补）。
async fn core() -> Home {
    let home = Home::new();
    let core = || {
        let mut core = home.core();
        core.env("MIYU_LOG", "trace");
        core
    };
    let (_held, _) = within("拉起", connect_or_start(&home.root, core))
        .await
        .expect("拉得起");
    home
}

/// 在真的伪终端上跑一次 `miyu login deepseek`：标准输入、输出、错误都是从端。
fn spawn(root: &std::path::Path, slave: &str) -> std::process::Child {
    Command::new(MIYU)
        .args(["login", "deepseek"])
        .env("MIYU_HOME", root)
        .envs(offline(root))
        .env("MIYU_RESOURCES", resources())
        .env("LANG", "zh_CN.UTF-8")
        .env("NO_COLOR", "1")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(secondary(slave))
        .stdout(secondary(slave))
        .stderr(secondary(slave))
        .spawn()
        .expect("跑得起来")
}

/// 密钥文件现在的字。
fn secrets(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("system/secrets.toml")).unwrap_or_default()
}

#[tokio::test]
async fn ctrl_c_while_pasting_cancels_exits_130_and_restores_echo() {
    let home = core().await;
    let (main, slave) = pty();
    assert!(echoing(&slave), "伪终端刚建好，回显该是开着的");

    let mut child = spawn(home.root.path(), &slave);
    let capture = Capture::start(&main);
    capture.wait_for("粘贴 deepseek 的 key（不显示）：");

    // 贴了一截就按 Ctrl+C（0x03），不等按回车：构造 8-5 补要修的那个场景。
    (&main)
        .write_all(b"sk-FAKE-partial\x03")
        .expect("写得进主端");

    let (code, screen) = capture.finish(&mut child);
    assert_eq!(code, 130, "{screen}");
    assert!(screen.ends_with("没存，取消了\n"), "{screen}");
    assert_eq!(secrets(&home), "", "密钥文件一个字都没写");
    assert!(
        echoing(&slave),
        "按了 Ctrl+C 以后回显也该开回来，不是停在关着的样子"
    );
}

#[tokio::test]
async fn a_normal_paste_with_enter_is_read_and_echo_stays_as_it_was() {
    let home = core().await;
    let (main, slave) = pty();
    assert!(echoing(&slave), "伪终端刚建好，回显该是开着的");

    let mut child = spawn(home.root.path(), &slave);
    let capture = Capture::start(&main);
    capture.wait_for("粘贴 deepseek 的 key（不显示）：");

    (&main)
        .write_all(format!("{FAKE}\n").as_bytes())
        .expect("写得进主端");

    let (code, screen) = capture.finish(&mut child);
    assert_eq!(code, 0, "{screen}");
    assert!(screen.ends_with("· deepseek 的 key 存好了\n"), "{screen}");
    assert!(
        secrets(&home).contains(&format!("deepseek = \"{FAKE}\"")),
        "{}",
        secrets(&home)
    );
    assert!(echoing(&slave), "读完以后回显照原样开着");
}
