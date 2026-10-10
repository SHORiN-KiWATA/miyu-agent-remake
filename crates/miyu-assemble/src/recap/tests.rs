//! 回顾的请求（施工 3-8 四补）：一条 user，指令接对话记录，不带 system、工具面；每一轮只取人这边的话和她最后一条有正文的
//! 回复，工具、思考、事实不要；别的 harness、子代理的话带着外壳；照到的是喂进去的最新那一条；最多几轮、没答的那句也带；
//! 放不下先整轮去掉最老的、再截中间；一个回复都没有、快照里没有回顾的字的，没有。

use miyu_kernel::block::Block;
use miyu_kernel::id::HarnessName;
use miyu_kernel::origin::{By, Harness};
use miyu_kernel::request::Message;

use super::*;
use crate::test_support::{Log, recap_texts, text_json, texts};

/// 一块字的回复的内容块。
fn words(text: &str) -> String {
    format!("[{}]", text_json(text))
}

/// 完整的一轮：人说 `said`，她答 `answer`。交回两条的序号。
fn turn(log: &mut Log, said: &str, answer: &str) -> (u64, u64) {
    let asked = log.say(said);
    log.start(asked);
    let answered = log.reply(&words(answer));
    log.end("completed");
    (asked, answered)
}

/// 照替身的字组装回顾，交回那一条 user 的字和照到的那一条。
fn recap(history: &History, texts: &Texts) -> Option<(String, u64)> {
    let (request, upto) = request(history, texts)?;
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("只有一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    Some((text.text.clone(), upto.get()))
}

/// 对话记录：去掉指令那一段。
fn transcript_in(log: &Log) -> String {
    let (text, _) = recap(log.history(), &texts()).expect("有能回顾的");
    text.strip_prefix("<recap>\n")
        .expect("指令在最前")
        .to_string()
}

#[test]
fn the_request_is_one_user_message_with_no_system_and_no_tools() {
    let mut log = Log::new();
    let (_, answered) = turn(&mut log, "看看 src", "src 下有两个文件。");
    let (request, upto) = request(log.history(), &texts()).expect("有能回顾的");
    assert!(request.tools.is_empty());
    assert_eq!(request.system, "");
    assert_eq!((request.stable, request.continuation), (0, false));
    assert_eq!(upto.get(), answered);
    assert_eq!(transcript_in(&log), "U: 看看 src\n\nA: src 下有两个文件。");
}

/// 只要人的话和她每一轮最后一条有正文的回复：中间一步说的话、工具调用和结果、思考、事实都不要；最后一条只有调用、思考的，
/// 取它前面那条有正文的。
#[test]
fn only_what_was_said_and_her_last_answer_of_each_turn() {
    let mut log = Log::new();
    let asked = log.say("读一下 lib.rs");
    log.start(asked);
    log.fact("<env/>");
    let call = log.reply_calling("我先读一下。");
    log.result(&call, "ok", "pub mod assemble;");
    log.reply(
        r#"[{"type":"reasoning","text":"想一想"},{"type":"text","text":"  只有一个模块。\n"}]"#,
    );
    log.end("completed");
    let asked = log.say("再看 main.rs");
    log.start(asked);
    let call = log.reply_calling("我看看。");
    log.result(&call, "ok", "fn main() {}");
    let last = log.next();
    log.reply(r#"[{"type":"reasoning","text":"再想想"}]"#);
    log.end("interrupted");
    assert_eq!(
        transcript_in(&log),
        "U: 读一下 lib.rs\n\nA: 只有一个模块。\n\nU: 再看 main.rs\n\nA: 我看看。"
    );
    let (_, upto) = recap(log.history(), &texts()).unwrap();
    assert_eq!(
        upto,
        last - 2,
        "照到的是最后那条有正文的回复，不是只有思考的那条"
    );
}

/// 别的 harness、子代理发来的话带着主请求里的外壳，看得出来处；父会话的交代（子会话里）照人的话写。
#[test]
fn where_the_words_came_from_shows() {
    let mut log = Log::new();
    let claude = By::Harness(Harness {
        name: HarnessName::parse("claude-code").unwrap(),
    });
    let asked = log.detached(
        &serde_json::to_string(&claude).unwrap(),
        "message.user",
        &format!(r#"{{"blocks":{}}}"#, words("跑一下测试")),
    );
    log.start(asked);
    log.reply(&words("好。"));
    log.end("completed");
    assert_eq!(
        transcript_in(&log),
        "U: <agent claude-code>\n跑一下测试\n</agent>\n\nA: 好。"
    );
}

/// 别的会话发来的话也带着主请求里的外壳（施工 C-2）。
#[test]
fn words_from_another_session_show_where_they_came_from() {
    let mut log = Log::new();
    let asked = log.detached(
        r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#,
        "message.user",
        &format!(r#"{{"blocks":{}}}"#, words("迁移写完了")),
    );
    log.start(asked);
    log.reply(&words("好。"));
    log.end("completed");
    assert_eq!(
        transcript_in(&log),
        "U: <peer 22334455>\n迁移写完了\n</peer>\n\nA: 好。"
    );
}

/// 挨着的几句人的话并成一段（排着的、急着插的），中间空一行；照到的是最新那一条。
#[test]
fn words_side_by_side_join_and_the_newest_is_upto() {
    let mut log = Log::new();
    let asked = log.say("看看 src");
    log.start(asked);
    let queued = log.say("顺便看 tests");
    log.reply(&words("都看了。"));
    log.end("completed");
    let (text, upto) = recap(log.history(), &texts()).unwrap();
    assert_eq!(upto, log.next() - 2, "回复在排着的那句后面");
    assert!(queued < upto);
    assert!(
        text.ends_with("U: 看看 src\n\n顺便看 tests\n\nA: 都看了。"),
        "{text}"
    );
}

/// 最新那一轮还没答的也带上，照到的是它。
#[test]
fn the_unanswered_newest_words_come_along() {
    let mut log = Log::new();
    turn(&mut log, "一", "答一");
    let pending = log.say("二");
    let (text, upto) = recap(log.history(), &texts()).unwrap();
    assert_eq!(upto, pending);
    assert!(text.ends_with("A: 答一\n\nU: 二"), "{text}");
}

/// 一个带正文的回复都没有的，没有；快照里没有回顾的字的，也没有。
#[test]
fn nothing_to_recap_without_an_answer_or_the_texts() {
    let mut log = Log::new();
    assert!(recap(log.history(), &texts()).is_none(), "什么都没有");
    let asked = log.say("看看 src");
    assert!(recap(log.history(), &texts()).is_none(), "只有一句人的话");
    log.start(asked);
    log.reply_calling("");
    assert!(
        recap(log.history(), &texts()).is_none(),
        "只有调用，没有正文"
    );
    let mut log = Log::new();
    turn(&mut log, "一", "答一");
    let old = Texts {
        recap: None,
        ..texts()
    };
    assert!(recap(log.history(), &old).is_none(), "以前造的快照");
}

/// 最多 `turns` 轮答过的，再早的不喂。
#[test]
fn at_most_the_last_turns() {
    let mut log = Log::new();
    for k in 1..=10 {
        turn(&mut log, &format!("问{k}"), &format!("答{k}"));
    }
    let text = transcript_in(&log);
    assert!(text.starts_with("U: 问3\n\nA: 答3"), "{text}");
    assert_eq!(text.matches("U: ").count(), 8);
    let mut texts = texts();
    texts.recap = Some(Recap {
        turns: 1,
        ..recap_texts()
    });
    let (text, _) = recap(log.history(), &texts).unwrap();
    assert_eq!(text, "<recap>\nU: 问10\n\nA: 答10", "只要最近一轮");
}

/// 最老那一轮只有回答、前面没有人的话的也留（codex 丢掉它）：压缩以后留着的尾巴可以从她的回答开头。
#[test]
fn a_leading_answer_stays() {
    let mut log = Log::new();
    let first = log.say("问一");
    log.start(first);
    log.reply(&words("答一"));
    log.end("completed");
    let second = log.say("问二");
    log.start(second);
    log.compact(first, "The user asked one.");
    log.reply(&words("答二"));
    log.end("completed");
    assert_eq!(transcript_in(&log), "A: 答一\n\nU: 问二\n\nA: 答二");
}

/// 照 `recap` 把 `log` 的对话记录截到至多 `room` 字节。
fn cut(log: &Log, room: usize) -> String {
    let recap = recap_texts();
    let turns = exchanges(log.history(), &texts(), recap.turns).unwrap();
    transcript(&turns, &recap, room)
}

/// 放不下的先整轮去掉最老的，最前写一行省略了；最新的回答一定留，还放不下就每一段截中间。
#[test]
fn over_the_limit_the_oldest_go_first_then_the_middles() {
    let mut log = Log::new();
    turn(&mut log, &"a".repeat(100), &"b".repeat(100));
    turn(&mut log, &"q".repeat(10), &"r".repeat(10));
    let (q, r) = ("q".repeat(10), "r".repeat(10));
    let whole = format!(
        "U: {}\n\nA: {}\n\nU: {q}\n\nA: {r}",
        "a".repeat(100),
        "b".repeat(100)
    );
    assert_eq!(cut(&log, 238), whole, "正好放得下");
    assert_eq!(cut(&log, 39), format!("[omitted]\n\nU: {q}\n\nA: {r}"));
    assert_eq!(
        cut(&log, 38),
        format!("[omitted]\n\nU: {q}\n\nA: r\n[...]\nr"),
        "后面的留一份，前面的放得下就不截"
    );
}

/// 省略的那一行也算在上限里：去掉一轮以后放得下、加上那一行放不下的，再去掉一轮。
#[test]
fn the_omitted_line_counts_against_the_limit() {
    let mut log = Log::new();
    turn(&mut log, &"a".repeat(100), &"b".repeat(100));
    turn(&mut log, "ccccc", "ddddd");
    turn(&mut log, &"q".repeat(10), &"r".repeat(10));
    let (q, r) = ("q".repeat(10), "r".repeat(10));
    // 去掉第一轮剩 48 字节，放得下 50；加上省略的那一行（11 字节）放不下，第二轮也去掉。
    assert_eq!(cut(&log, 50), format!("[omitted]\n\nU: {q}\n\nA: {r}"));
}

/// 最新那一轮没答的：它前面那一轮（最新的回答）也一定留。
#[test]
fn over_the_limit_the_newest_answer_stays_with_the_unanswered() {
    let mut log = Log::new();
    turn(&mut log, &"a".repeat(100), &"b".repeat(100));
    turn(&mut log, &"q".repeat(10), &"r".repeat(10));
    log.say("ppppp");
    let q = "q".repeat(10);
    assert_eq!(
        cut(&log, 49),
        format!("[omitted]\n\nU: {q}\n\nA: {}\n\nU: ppppp", "r".repeat(10))
    );
    assert_eq!(
        cut(&log, 48),
        format!("[omitted]\n\nU: {q}\n\nA: r\n[...]\nr\n\nU: ppppp")
    );
}

/// 整份连指令至多 `tokens` 个 token：字节/4 折成字节，指令也算在里面。
#[test]
fn the_limit_counts_the_instruction_too() {
    let mut log = Log::new();
    turn(&mut log, &"a".repeat(100), &"b".repeat(100));
    turn(&mut log, "q", "r");
    let mut texts = texts();
    // 指令 8 字节，限 10 个 token 就是 40 字节：对话记录只剩 32 字节，前一轮放不下。
    texts.recap = Some(Recap {
        tokens: 10,
        ..recap_texts()
    });
    let (text, _) = recap(log.history(), &texts).unwrap();
    assert_eq!(text, "<recap>\n[omitted]\n\nU: q\n\nA: r");
}

/// 上限小到连标签、空行都放不下：整份截到上限，只在字的边界上截（照 codex 的 `RecapPrompt::new`）。
#[test]
fn a_limit_below_the_labels_cuts_the_whole_transcript() {
    let mut log = Log::new();
    turn(&mut log, "问", "答");
    let mut texts = texts();
    // 指令 8 字节，限 3 个 token 就是 12 字节：对话记录只剩 4 字节，两段的标签和空行就有 8 字节，截成「U: 」和一个换行。
    texts.recap = Some(Recap {
        tokens: 3,
        ..recap_texts()
    });
    let (text, _) = recap(log.history(), &texts).unwrap();
    assert_eq!(text, "<recap>\nU: \n");
    // 截处落在一个字中间的，退到这个字前面。
    texts.recap = Some(Recap {
        tokens: 3,
        user: "人：".to_string(),
        ..recap_texts()
    });
    let (text, _) = recap(log.history(), &texts).unwrap();
    assert_eq!(text, "<recap>\n人");
}

/// 一段截中间：头尾各留一半，中间夹记号；只在字的边界上截；连记号都放不下的只留开头。
#[test]
fn an_excerpt_keeps_both_ends_on_char_boundaries() {
    assert_eq!(excerpt("abcdef", 6, "~"), "abcdef");
    assert_eq!(excerpt("abcdefghij", 5, "~"), "ab~ij");
    assert_eq!(excerpt("中文字符串", 10, "~"), "中~串", "一个字三个字节");
    assert_eq!(excerpt("abcdef", 0, "~~"), "");
    assert_eq!(excerpt("中文", 4, "~~~~~"), "中", "截不到半个字");
}

/// 回顾的请求逐字节不变（施工 R-6 上：抽取和它共用 `entries`，那一份改了这里要先红）。
#[test]
fn the_recap_request_keeps_its_bytes() {
    let mut log = Log::new();
    turn(&mut log, "看看 src", "src 下有两个文件。");
    let call_turn = log.say("再看看 tests");
    log.start(call_turn);
    let call = log.reply_calling("我看看。");
    log.result(&call, "ok", "fn main() {}");
    log.reply(&words("tests 下有一个。"));
    log.end("completed");
    let (request, upto) = request(log.history(), &texts()).expect("有能回顾的");
    let bytes = serde_json::to_string(&request).expect("写得成");
    assert_eq!(upto.get(), log.next() - 2);
    assert_eq!(
        bytes,
        r#"{"tools":[],"system":"","messages":[{"role":"user","blocks":[{"type":"text","text":"<recap>\nU: 看看 src\n\nA: src 下有两个文件。\n\nU: 再看看 tests\n\nA: tests 下有一个。"}]}],"stable":0}"#
    );
}
