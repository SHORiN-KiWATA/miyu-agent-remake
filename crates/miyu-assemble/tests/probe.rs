//! 请求形状探针（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，`26-提示词.md` 第七节）：
//! 一份终端会话的假日志，每一次请求组装出来，和存档逐字节比对，再查五条性质。
//!
//! 存档在 `docs/designs/samples/probe/terminal/`：`log.jsonl` 是假日志，`requests/` 下一次
//! 请求一个文件，写的是规范字节，末尾一个换行。字节变了必须是有意的：设上
//! `MIYU_PROBE_WRITE=1` 跑一遍，重写存档，提交说明里写为什么变。

mod support;

use std::fs;
use std::path::PathBuf;

use support::{Session, check};

/// 终端会话的剧本，八个回合，1-12、1-13 画过的走法都走一遍。
fn terminal() -> Session {
    let mut s = Session::new();

    // 1. 第一轮：环境、权限两块事实；调一次工具，结果回来，再回一句。
    let trigger = s.say("看看 src 目录");
    s.start(trigger);
    let seen = s.request();
    let calls = s.reply(seen, "我先看一下目录。", &[("read", r#"{"path":"src"}"#)]);
    s.result(&calls[0], "ok", "lib.rs\nmain.rs");
    let seen = s.request();
    s.reply(seen, "src 下有 lib.rs 和 main.rs。", &[]);
    s.end("completed");

    // 2. 调两次工具，结果倒着回来；回合中途来了一句话。
    s.advance(5);
    let trigger = s.say("两个文件都读一下");
    s.start(trigger);
    let seen = s.request();
    let calls = s.reply(
        seen,
        "好，两个一起读。",
        &[
            ("read", r#"{"path":"src/lib.rs"}"#),
            ("read", r#"{"path":"src/main.rs"}"#),
        ],
    );
    s.result(&calls[1], "ok", "fn main() {}");
    s.result(&calls[0], "ok", "pub mod assemble;");
    s.say("顺便看看 Cargo.toml");
    let seen = s.request();
    let calls = s.reply(
        seen,
        "再读 Cargo.toml。",
        &[("read", r#"{"path":"Cargo.toml"}"#)],
    );
    s.result(&calls[0], "ok", "[package]\nname = \"miyu\"");
    let seen = s.request();
    s.reply(seen, "三个文件都看完了。", &[]);
    s.end("completed");

    // 3. 过了整点，环境重新注入；回复还在路上，人插了一句，这一轮被打断，收全了的那次调用
    //    补「已取消」。
    s.advance(60);
    let trigger = s.say("把 main.rs 改成打印 hello");
    s.start(trigger);
    let seen = s.request();
    let interjection = s.say("等等，先别改了，只读着看看");
    let calls = s.cut_off(
        seen,
        "我先改 main.rs",
        &[("write", r#"{"path":"src/main.rs"}"#)],
    );
    s.result(
        &calls[0],
        "cancelled",
        "Cancelled: the user interrupted this turn.",
    );
    s.end("interrupted");

    // 4. 插的那一句开了下一轮，排在回合开始的地方；两轮之间换成只读，权限重新注入；
    //    一次调用被拒绝。
    s.read_only(true);
    s.start(interjection);
    let seen = s.request();
    let calls = s.reply(
        seen,
        "我试着写一个说明文件。",
        &[("write", r#"{"path":"NOTES.md"}"#)],
    );
    s.result(&calls[0], "denied", "Denied: the session is read-only.");
    let seen = s.request();
    s.reply(seen, "现在是只读，写不了。", &[]);
    s.end("completed");

    // 5. 第一次请求就出错，没等到回复。
    let trigger = s.say("列一下 tests 目录");
    s.start(trigger);
    s.request();
    s.end("error");

    // 6. 走到步数上限。
    let trigger = s.say("再试一次");
    let sixth = s.start(trigger);
    let seen = s.request();
    let calls = s.reply(seen, "我来列目录。", &[("read", r#"{"path":"tests"}"#)]);
    s.result(&calls[0], "ok", "probe.rs\nsupport/");
    let seen = s.request();
    let calls = s.reply(
        seen,
        "再往下一层。",
        &[("read", r#"{"path":"tests/support"}"#)],
    );
    s.result(&calls[0], "ok", "mod.rs");
    s.end("step_limit");

    // 7. 撤销第 6 轮以后，再说一句。
    s.undo(sixth);
    let trigger = s.say("换个问法：tests 下有几个文件？");
    s.start(trigger);
    let seen = s.request();
    s.reply(seen, "一个文件，一个目录。", &[]);
    s.end("completed");

    // 8. 压缩以后，两块事实重新注入。
    s.compact("The user explored src and tests in read-only mode. Nothing is in progress.");
    let trigger = s.say("接着来");
    s.start(trigger);
    let seen = s.request();
    s.reply(seen, "好的。", &[]);
    s.end("completed");

    s
}

/// 存档所在的目录：这个 crate 的目录往上两级是仓库根。
fn archive() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/probe/terminal")
}

/// 这份会话要存档的几个文件：相对存档目录的路径，和内容。
fn files(session: &Session) -> Vec<(String, String)> {
    let mut files = vec![("log.jsonl".to_string(), session.lines().join("\n") + "\n")];
    for (index, sent) in session.sent().iter().enumerate() {
        let bytes = String::from_utf8(sent.request.canonical_bytes()).expect("规范的字节是 UTF-8");
        files.push((format!("requests/{:02}.json", index + 1), bytes + "\n"));
    }
    files
}

#[test]
fn the_terminal_session_matches_the_archive() {
    let files = files(&terminal());
    let dir = archive();
    if std::env::var_os("MIYU_PROBE_WRITE").is_some() {
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("删得掉旧的存档");
        }
        fs::create_dir_all(dir.join("requests")).expect("建得了存档目录");
        for (name, content) in &files {
            fs::write(dir.join(name), content).expect("写得了存档");
        }
        return;
    }
    for (name, content) in &files {
        let path = dir.join(name);
        let archived =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        assert!(
            archived == *content,
            "{name} 和存档不一样。要是有意改的，设上 MIYU_PROBE_WRITE=1 跑一遍重写存档，提交说明里写为什么变"
        );
    }
    let archived = fs::read_dir(dir.join("requests"))
        .expect("读得了存档的请求目录")
        .count();
    assert_eq!(archived, files.len() - 1, "存档里的请求数和这一次的不一样");
}

#[test]
fn the_terminal_session_keeps_the_properties() {
    if let Err(why) = check(terminal().sent()) {
        panic!("{why}");
    }
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&terminal()), files(&terminal()));
}
