//! 请求形状探针（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，`26-提示词.md` 第七节）：
//! 一段终端会话，由真内核照剧本跑出来（执行器替身，施工 2-9 下），每一次请求和存档逐字节比对，
//! 再查五条性质。
//!
//! 存档在 `docs/designs/samples/probe/terminal/`：`log.jsonl` 是真内核记下的日志，`requests/`
//! 下一次请求一个文件，写的是规范字节，末尾一个换行；`openai-chat/` 下是同一次请求编码成 OpenAI
//! 兼容接口的字节（施工 3-4 上）。字节变了必须是有意的：设上 `MIYU_PROBE_WRITE=1` 跑一遍，重写
//! 存档，提交说明里写为什么变。

mod support;

use std::fs;
use std::path::PathBuf;

use miyu_kernel::event::ErrorClass;
use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{Line, Play, Stage};
use support::{check, lines, sent, stage, wire};

/// 终端会话的剧本，八个回合，1-12、1-13 画过的走法都走一遍。照真内核会怎么走写（施工 2-9 下）：
/// 回合中途的那句话在工具还在跑时说；两轮之间换只读，改成请求还在路上时先切、再打断。
fn terminal() -> Stage {
    let mut s = stage();

    // 1. 第一轮：环境、权限两块事实；调一次工具，结果回来，再回一句。
    s.model([
        Line::calls("我先看一下目录。", &[("read", r#"{"path":"src"}"#)]),
        Line::says("src 下有 lib.rs 和 main.rs。"),
    ]);
    s.tools([Play::done("lib.rs\nmain.rs")]);
    s.say("看看 src 目录");

    // 2. 调两次工具，结果倒着回来；工具在跑时来了一句话，下一步听到了；再调一次，说完。
    s.advance(5);
    s.model([
        Line::calls(
            "好，两个一起读。",
            &[
                ("read", r#"{"path":"src/lib.rs"}"#),
                ("read", r#"{"path":"src/main.rs"}"#),
            ],
        ),
        Line::calls("再读 Cargo.toml。", &[("read", r#"{"path":"Cargo.toml"}"#)]),
        Line::says("三个文件都看完了。"),
    ]);
    s.tools([
        Play::done("pub mod assemble;").held(),
        Play::done("fn main() {}").held(),
        Play::done("[package]\nname = \"miyu\""),
    ]);
    s.say("两个文件都读一下");
    let running = s.ran().len();
    let (lib, main) = (s.ran()[running - 2].0, s.ran()[running - 1].0);
    s.release_tool(main);
    s.say("顺便看看 Cargo.toml");
    s.release_tool(lib);

    // 3. 过了整点，环境重新注入；回复还在路上，人插了一句，又切成只读，再打断：收全了的那次调用
    //    补「已取消」。
    // 4. 插的那一句接着开了下一轮，排在回合开始的地方；权限重新注入；一次调用被只读拦下。
    s.advance(60);
    s.model([
        Line::calls("我先改 main.rs", &[("write", r#"{"path":"src/main.rs"}"#)]).held(),
        Line::calls(
            "我试着写一个说明文件。",
            &[("write", r#"{"path":"NOTES.md"}"#)],
        ),
        Line::says("现在是只读，写不了。"),
    ]);
    s.say("把 main.rs 改成打印 hello");
    s.say("等等，先别改了，只读着看看");
    s.set_permission(None, Some(true));
    s.interrupt(Queued::Send);

    // 5. 第一次请求就出错，没等到回复。
    s.model([Line::fails(
        ErrorClass::Retryable,
        "503 Service Unavailable",
    )]);
    s.say("列一下 tests 目录");

    // 6. 连着三次调工具，走到步数上限。
    s.model([
        Line::calls("我来列目录。", &[("read", r#"{"path":"tests"}"#)]),
        Line::calls("再往下一层。", &[("read", r#"{"path":"tests/support"}"#)]),
        Line::calls(
            "再看看 mod.rs。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
    ]);
    s.tools([
        Play::done("probe.rs\nsupport/"),
        Play::done("mod.rs"),
        Play::done("pub fn check() {}"),
    ]);
    s.say("再试一次");
    let sixth = *s.turns().last().expect("第 6 轮开过");

    // 7. 撤销第 6 轮以后，再说一句。
    s.revert(sixth);
    s.model([Line::says("一个文件，一个目录。")]);
    s.say("换个问法：tests 下有几个文件？");

    // 8. 压缩以后，两块事实重新注入。
    s.compact("The user explored src and tests in read-only mode. Nothing is in progress.");
    s.model([Line::says("好的。")]);
    s.say("接着来");

    s
}

/// 存档所在的目录：这个 crate 的目录往上两级是仓库根。
fn archive() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/probe/terminal")
}

/// 这段会话要存档的几个文件：相对存档目录的路径，和内容。
fn files(stage: &Stage) -> Vec<(String, String)> {
    let mut files = vec![("log.jsonl".to_string(), lines(stage).join("\n") + "\n")];
    for (index, (_, request)) in stage.requests().iter().enumerate() {
        let bytes = String::from_utf8(request.canonical_bytes()).expect("规范的字节是 UTF-8");
        files.push((format!("requests/{:02}.json", index + 1), bytes + "\n"));
        let body = String::from_utf8(wire(request).body).expect("请求字节是 UTF-8");
        files.push((format!("openai-chat/{:02}.json", index + 1), body + "\n"));
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
        fs::create_dir_all(dir.join("openai-chat")).expect("建得了存档目录");
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
    for folder in ["requests", "openai-chat"] {
        let archived = fs::read_dir(dir.join(folder))
            .expect("读得了存档的请求目录")
            .count();
        assert_eq!(
            archived,
            (files.len() - 1) / 2,
            "存档 {folder}/ 里的请求数和这一次的不一样"
        );
    }
}

#[test]
fn the_terminal_session_keeps_the_properties() {
    let sent = sent(&terminal());
    if let Err(why) = check(&sent) {
        panic!("{why}");
    }
    // 查的是真东西：八个回合的第一次请求都由人的一句话触发，都查了最后一块；撤销、压缩以后的
    // 那两次算改写过。
    let triggered = sent.iter().filter(|sent| sent.trigger.is_some()).count();
    let rewritten: Vec<usize> = (0..sent.len()).filter(|&k| sent[k].rewritten).collect();
    assert_eq!(triggered, 8);
    assert_eq!(rewritten, [12, 13], "第 13、14 次请求");
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&terminal()), files(&terminal()));
}
