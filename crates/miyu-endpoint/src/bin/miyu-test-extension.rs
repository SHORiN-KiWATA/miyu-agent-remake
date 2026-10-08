//! 测试用的扩展（施工 9-4 上，`docs/blueprint/extensions.md`「守着它的」）：端点的测试把它放在测试程序旁边，当成
//! `process` 包的程序让核心拉起。不随发行带出去。只用标准库，三个平台一样。
//!
//! 参数是一步步要做的，照先后：
//!
//! - `record:<路径>`：往这个文件追加记下的（先记一行 `cwd:<工作目录>`）；
//! - `err:<字>`：往标准错误写一行；
//! - `hello`：发握手（不带凭据），读一行回应记下；
//! - `call:<方法>`：发一条不带参数的请求，读一行回应记下；
//! - `wait`：读标准输入直到读到头；
//! - `hang`：不管标准输入，一直睡；
//! - `exit:<数>`：以这个退出码退出。
//!
//! 做完了以退出码 0 退出。

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut record: Option<File> = None;
    for (n, step) in std::env::args().skip(1).enumerate() {
        if let Some(path) = step.strip_prefix("record:") {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .expect("开得了记的文件");
            let cwd = std::env::current_dir().expect("有工作目录");
            writeln!(file, "cwd:{}", cwd.display()).expect("写得进");
            record = Some(file);
        } else if let Some(text) = step.strip_prefix("err:") {
            eprintln!("{text}");
        } else if step == "hello" {
            send(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"hello","params":{{"protocol":[1,1],"head":{{"kind":"test-extension","version":"0"}}}}}}"#
            ));
            keep(&mut record, &read(&mut input));
        } else if let Some(method) = step.strip_prefix("call:") {
            send(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"{method}","params":{{}}}}"#
            ));
            keep(&mut record, &read(&mut input));
        } else if step == "wait" {
            while !read(&mut input).is_empty() {}
        } else if step == "hang" {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        } else if let Some(code) = step.strip_prefix("exit:") {
            std::process::exit(code.parse().expect("退出码是数"));
        } else {
            panic!("不认识的一步：{step}");
        }
    }
}

/// 写一行到标准输出。写不出去（核心关了）的照样往下走。
fn send(line: &str) {
    let mut out = io::stdout().lock();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        // 核心那头关了：后面的步子照做。
    }
}

/// 读一行；读到头、读不了的是空的。
fn read(input: &mut impl BufRead) -> String {
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(_) => line.trim_end().to_string(),
        Err(_) => String::new(),
    }
}

/// 记下一行。
fn keep(record: &mut Option<File>, line: &str) {
    if let Some(file) = record {
        writeln!(file, "{line}").expect("写得进");
    }
}
