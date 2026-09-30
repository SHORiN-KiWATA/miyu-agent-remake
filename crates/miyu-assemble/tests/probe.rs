//! 请求形状探针（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，`26-提示词.md` 第七节）：
//! 一段终端会话，由真内核照剧本跑出来（执行器替身，施工 2-9 下），每一次请求和存档逐字节比对，
//! 再查五条性质。另一段是有回报的会话（施工 7-2）：派出去的任务回报到了，闲着时开一轮、正忙时排在工具结果后面。还有一段
//! 是子代理的会话（施工 7-5）：父会话的交代开了第一轮，system 多一段场所说明，会话编号那一块写的是它自己的编号（施工 1-13
//! 再补）。清空过的会话（施工 6-8 补）：清空以后的第一次请求只剩工具面、system、三块事实和那一句。
//!
//! 存档在 `docs/designs/samples/probe/<会话>/`（`terminal`、`reports`、`subagent`、`cleared`）：`log.jsonl` 是真内核记下的日志，`requests/`
//! 下一次请求一个文件，写的是规范字节，末尾一个换行；`openai-chat/` 下是同一次请求编码成 OpenAI
//! 兼容接口的字节（施工 3-4 上）。字节变了必须是有意的：设上 `MIYU_PROBE_WRITE=1` 跑一遍，重写
//! 存档，提交说明里写为什么变。怎么存、怎么比在 `support/archive.rs`；有别的 harness 来话的会话在 `probe_harness.rs`。

mod support;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{ChildReason, ErrorClass, JobReason};
use miyu_kernel::origin::{By, Tool};
use miyu_kernel::request::Message;
use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{CHILD_SESSION, Line, Play, SESSION, Stage};
use support::{
    PARENT, VENUE, anchored, check, child_stage, files, matches_the_archive, sent, stage,
    summarizes,
};

/// 终端会话的剧本，十一个回合，1-12、1-13 画过的走法都走一遍，最后两段是自动压缩（施工 6-2 上）。照真内核会怎么走写（施工 2-9 下）：
/// 回合中途的那句话在工具还在跑时说；两轮之间换只读，改成请求还在路上时先切、再打断。
fn terminal() -> Stage {
    let mut s = stage();

    // 1. 第一轮：环境、权限、会话编号三块事实；调一次工具，结果回来，再回一句。
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

    // 5. 出错了再来（施工 3-5 下）：第一次什么都没收到，原样再请求，一字不差；第二次想了一点、说了
    //    一半断了，半截写成回复，跟一句被打断的提示再请求；第三次认证失败，不再来，这一轮以出错结束，
    //    半截留着。
    s.model([
        Line::fails(ErrorClass::Retryable, "503 Service Unavailable"),
        Line::breaks("我先列一下", ErrorClass::Retryable, "connection reset")
            .thinking("用户要看 tests 目录。"),
        Line::fails(ErrorClass::Auth, "401 Unauthorized"),
    ]);
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

    // 8. 压缩以后，三块事实重新注入。
    s.compact("The user explored src and tests in read-only mode. Nothing is in progress.");
    s.model([Line::says("好的。")]);
    s.say("接着来");

    // 9. 交了限额（窗口 33400，压缩线 400，尾巴的预算 100）；这一轮问得长（约 200 个 token），报的用量是 40000。
    // 10. 下一轮一开头就过线：先压，最近几组留作尾巴（施工 6-2 下）：第 9 轮的回复和这一轮的那句，长的那一问压进
    //     摘要；压完三块事实重新注入，和触发的那句放在一起。
    summarizes(&mut s);
    s.limits(Some(33_400), None);
    s.model([Line::says("README 里写了怎么装。").reports(40_000)]);
    s.say(&"README 写了什么？装的时候要注意什么，每个平台有什么不一样？".repeat(8));
    s.model([Line::says("装好以后跑 miyu ask。").reports(100)]);
    s.say("装好以后呢？");

    // 11. 回合中途过线：调工具的那一次报 40000，结果回来以后先压。最新的一组（这次调用和它约 150 个 token 的结果）
    //     比尾巴的预算还大，不留尾巴，截到最后一条；再接着说完。
    s.model([
        Line::calls("我看一下 lib.rs。", &[("read", r#"{"path":"src/lib.rs"}"#)]).reports(40_000),
        Line::says("lib.rs 只有一行。").reports(1_000),
    ]);
    s.tools([Play::done(&"pub mod assemble;\n".repeat(33))]);
    s.say("lib.rs 里有什么？");

    s
}

/// 子代理的会话。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 有回报的会话的剧本（施工 7-2）。探针的工具面只有 `read`、`write`，派任务的那两次调用借 `read` 的名字：真的是 `shell`
/// 的后台命令、`agent`（7-3、7-5），这里看的是回报渲染进请求的样子。
fn reports() -> Stage {
    let mut s = stage();

    // 1. 派出去两个：后台命令 j1 跑测试、子代理 j2 查 CI。
    s.model([
        Line::calls(
            "我派出去，回报来了接着看。",
            &[
                ("read", r#"{"path":"tests"}"#),
                ("read", r#"{"path":".github"}"#),
            ],
        ),
        Line::says("派出去了。"),
    ]);
    s.tools([
        Play::starts_command(1, "跑全部测试"),
        Play::starts_agent(2, "查 CI 为什么红", CHILD),
    ]);
    let asked = s.say("跑一下全部测试，顺便查查 CI 为什么红");
    let started = s.ran()[0].0;

    // 2. 她闲着，子代理回报：由它开一轮，回合开始的地方就是这条回报。
    s.advance(3);
    s.model([Line::says("CI 的原因找到了：macOS 上临时目录在链接下面。")]);
    s.child_reports(
        2,
        CHILD,
        ChildReason::Done,
        "CI 红在 macOS：测试的临时目录在 /var 下，/var 是链接，安全打开不走链接，拒绝了。先把临时目录换成真实路径就好。",
    );

    // 3. 人问一句，她读文件；读的时候测试跑完了：下一次请求里排在这一步的工具结果后面，不另开一轮。
    s.advance(2);
    s.model([
        Line::calls(
            "我看看测试的临时目录是怎么建的。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
        Line::says("测试也都过了，把临时目录换成真实路径就行。"),
    ]);
    s.tools([Play::done("fn temp_dir() -> PathBuf { std::env::temp_dir() }").held()]);
    s.say("那该怎么改？");
    let reading = s.ran().last().expect("派了读").0;
    let starter = By::Tool(Tool { call_id: started });
    s.job_ends(1, JobReason::Exited, starter, Some(asked));
    s.release_tool(reading);

    // 4. 人再说一句，她正回着话时子代理又回报了一次（它被叫醒、说了没说完就崩了，只记下），接着人又说了一句：回合结束时
    //    由排着的那句接着开，只记下的那条回报在它前面一起看到。
    s.advance(2);
    s.model([Line::says("好的。").held(), Line::says("知道了，先不动。")]);
    s.say("先别改");
    s.child_reports(2, CHILD, ChildReason::Aborted, "");
    s.say("等我看完日志再说");
    s.release_model();

    s
}

/// 子代理的会话的剧本（施工 7-5）：父会话的交代开了它的第一轮，她读一个文件，最后的回答就是交回去的回报。`on` 造替身：
/// 子会话的，或者同一份剧本的主会话，两张脸只差 system 里的场所说明。
fn subagent(on: fn() -> Stage) -> Stage {
    let mut s = on();
    s.model([
        Line::calls("我先读 lib.rs。", &[("read", r#"{"path":"src/lib.rs"}"#)]),
        Line::says("lib.rs 只导出了 assemble 这一个模块。"),
    ]);
    s.tools([Play::done("pub mod assemble;")]);
    s.say("Read src/lib.rs and tell me what it exports. Report the module names.");
    s
}

/// 清空过的会话的剧本（施工 6-8 补）：第一轮读一次目录，清空，再问刚才看了哪个目录。
fn cleared() -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("我先看一下目录。", &[("read", r#"{"path":"src"}"#)]),
        Line::says("src 下有 lib.rs 和 main.rs。"),
    ]);
    s.tools([Play::done("lib.rs\nmain.rs")]);
    s.say("看看 src 目录");
    s.advance(2);
    s.request_clear();
    s.model([Line::says("我不知道，上下文里没有。")]);
    s.say("刚才我让你看了哪个目录？");
    s
}

#[test]
fn the_terminal_session_matches_the_archive() {
    matches_the_archive("terminal", &terminal());
}

#[test]
fn the_reports_session_matches_the_archive() {
    matches_the_archive("reports", &reports());
}

#[test]
fn the_cleared_session_matches_the_archive() {
    matches_the_archive("cleared", &cleared());
}

#[test]
fn the_subagent_session_matches_the_archive() {
    matches_the_archive("subagent", &subagent(child_stage));
}

/// 子代理这张脸（施工 7-5）：五条性质照查；交代是父会话发的、开了第一轮；和同一份剧本的主会话比，每一次请求只多 system
/// 里的场所说明那一段，会话编号那一块写的是它自己的编号（施工 1-13 再补），换成主会话的就一字不差。
#[test]
fn the_subagent_session_differs_only_by_its_venue_note() {
    let child = subagent(child_stage);
    if let Err(why) = check(&sent(&child)) {
        panic!("{why}");
    }
    let prompt = child
        .log()
        .iter()
        .find(|event| event.body.kind() == "message.user")
        .expect("交代");
    assert_eq!(
        prompt.by.clone(),
        serde_json::from_str::<By>(&format!(r#"{{"kind":"session","id":"{PARENT}"}}"#)).unwrap()
    );
    let main = subagent(stage);
    assert_eq!(child.requests().len(), 2);
    for ((_, child), (_, main)) in child.requests().iter().zip(main.requests()) {
        assert_eq!(child.tools, main.tools);
        assert_eq!(as_main(&child.messages), main.messages);
        assert_eq!(
            child.system,
            format!("{}\n\n{}", main.system, VENUE.trim_end())
        );
    }
}

/// 子会话的消息，会话编号那一块换成主会话的编号（施工 1-13 再补）。换之前得有这一块，换了才算数。
fn as_main(messages: &[Message]) -> Vec<Message> {
    let own = format!("<session id=\"{CHILD_SESSION}\"/>\n");
    let main = format!("<session id=\"{SESSION}\"/>\n");
    let mut swapped = 0;
    let messages = messages
        .iter()
        .cloned()
        .map(|mut message| {
            if let Message::User { blocks } = &mut message {
                for block in blocks {
                    if let Block::Text(text) = block
                        && text.text == own
                    {
                        text.text.clone_from(&main);
                        swapped += 1;
                    }
                }
            }
            message
        })
        .collect();
    assert_eq!(swapped, 1, "子会话的第一轮带着它自己的编号");
    messages
}

#[test]
fn the_terminal_session_keeps_the_properties() {
    let session = terminal();
    let sent = sent(&session);
    if let Err(why) = check(&sent) {
        panic!("{why}");
    }
    // 锚盖住的正好是锚那次请求加上它的回复（施工 6-1）：替身发了二十二次请求（出错再来的、两次摘要请求也算），第一次、
    // 三次压缩以后的第一次没有锚；摘要请求不估用量，不查（施工 6-2 下）。
    match anchored(&session) {
        Ok(count) => assert_eq!(
            (count, session.requests().len()),
            (16, 22),
            "有锚的请求的次数"
        ),
        Err(why) => panic!("{why}"),
    }
    // 查的是真东西：十一个回合的第一次请求都由人的一句话触发，都查了最后一块（回合开头压缩的那一轮，查的是压完的
    // 那一次）；撤销、三次压缩以后的那几次算改写过。两次摘要请求都是上一次请求的前缀延伸（施工 6-2 上）。
    let triggered = sent.iter().filter(|sent| sent.trigger.is_some()).count();
    let rewritten: Vec<usize> = (0..sent.len()).filter(|&k| sent[k].rewritten).collect();
    let summaries: Vec<usize> = (0..sent.len()).filter(|&k| sent[k].summary).collect();
    assert_eq!(triggered, 11);
    assert_eq!(rewritten, [14, 15, 18, 21], "第 15、16、19、22 次请求");
    assert_eq!(summaries, [17, 20], "第 18、21 次请求");
    assert!(
        summaries
            .iter()
            .all(|&k| !sent[k].rewritten && sent[k].trigger.is_none())
    );
    // 第 5 轮什么都没收到的那一次再来，和第一次一字不差：`model.called` 不进上下文。
    let fifth = sent
        .iter()
        .position(|sent| {
            sent.trigger.as_ref().is_some_and(
                |trigger| matches!(trigger, Block::Text(text) if text.text == "列一下 tests 目录"),
            )
        })
        .expect("第 5 轮由那一句开");
    assert_eq!(
        sent[fifth].request.canonical_bytes(),
        sent[fifth + 1].request.canonical_bytes(),
        "什么都没收到的再来，一字不差"
    );
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&terminal()), files(&terminal()));
    assert_eq!(files(&reports()), files(&reports()));
    assert_eq!(files(&subagent(child_stage)), files(&subagent(child_stage)));
    assert_eq!(files(&cleared()), files(&cleared()));
}

/// 清空过的会话（施工 6-8 补）：五条性质照查，清空以后的那一次算改写过；它只剩工具面、system 和一条 user：三块事实、那
/// 一句，检查点一个字都没有。
#[test]
fn the_cleared_session_keeps_the_properties() {
    let session = cleared();
    let sent = sent(&session);
    if let Err(why) = check(&sent) {
        panic!("{why}");
    }
    let rewritten: Vec<usize> = (0..sent.len()).filter(|&k| sent[k].rewritten).collect();
    assert_eq!(rewritten, [2], "清空以后的第一次");
    let (first, after) = (&sent[0].request, &sent[2].request);
    assert_eq!((&after.tools, &after.system), (&first.tools, &first.system));
    let [Message::User { blocks }] = after.messages.as_slice() else {
        panic!("只剩一条 user：{:?}", after.messages);
    };
    let texts: Vec<&str> = blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.as_str(),
            other => panic!("都是字：{other:?}"),
        })
        .collect();
    assert_eq!(texts.len(), 4, "{texts:?}");
    assert!(texts[0].starts_with("<env "), "{texts:?}");
    assert!(texts[1].starts_with("<permission "), "{texts:?}");
    assert_eq!(texts[2], format!("<session id=\"{SESSION}\"/>\n"));
    assert_eq!(texts[3], "刚才我让你看了哪个目录？");
}

/// 有回报的会话（施工 7-2）：五条性质照查；回报开的那一轮第一次请求的最后一块是那条回报，回合中途到的排在工具结果后面，
/// 只记下的在人那一句前面。
#[test]
fn the_reports_session_keeps_the_properties() {
    let session = reports();
    let sent = sent(&session);
    if let Err(why) = check(&sent) {
        panic!("{why}");
    }
    let texts: Vec<Vec<String>> = session
        .requests()
        .iter()
        .map(|(_, request)| last_user(request))
        .collect();
    let opened = &texts[2];
    assert!(
        opened.last().is_some_and(|text| text.starts_with(
            "<subagent-report job=\"j2\" title=\"查 CI 为什么红\" reason=\"done\">\n"
        )),
        "回报开的那一轮，最后一块是那条回报：{opened:?}"
    );
    let busy = &session.requests()[4].1;
    assert!(
        matches!(busy.messages.last(), Some(Message::User { blocks }) if matches!(blocks.as_slice(), [Block::Text(Text { text })] if text.starts_with("<command-ended job=\"j1\""))),
        "回合中途到的，单独一条 user 排在工具结果后面：{:?}",
        busy.messages.last()
    );
    let last = texts.last().expect("有请求");
    let aborted = last
        .iter()
        .position(|text| text.contains("reason=\"aborted\""));
    let said = last.iter().position(|text| text == "等我看完日志再说");
    assert!(
        aborted.is_some() && aborted < said,
        "只记下的在人那一句前面：{last:?}"
    );
}

/// 请求最后一条 user 的每一块文字。
fn last_user(request: &miyu_kernel::request::Request) -> Vec<String> {
    match request.messages.last() {
        Some(Message::User { blocks }) => blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
