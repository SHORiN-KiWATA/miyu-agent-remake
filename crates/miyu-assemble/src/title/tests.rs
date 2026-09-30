//! 起标题的请求（施工 3-8 五补）：一条 user，指令接对话记录，不带 system、工具面；只喂第一轮：第一个回答以前人这边的话
//! （挨着的并成一段，别的 harness 的带外壳）和那一轮最后一条有正文的回复，后面的轮不要；照到的是那个回答；放不下两段各截
//! 中间；没有回答、快照里没有起标题的字或者回顾的标签的，没有。

use miyu_kernel::block::Block;
use miyu_kernel::id::HarnessName;
use miyu_kernel::origin::{By, Harness};
use miyu_kernel::request::Message;

use super::*;
use crate::test_support::{Log, text_json, texts, title_texts};
use crate::texts::Title;

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

/// 照 `texts` 组装起标题的请求，交回那一条 user 的字和照到的那一条。
fn title(history: &History, texts: &Texts) -> Option<(String, u64)> {
    let (request, upto) = request(history, texts)?;
    assert!(request.tools.is_empty());
    assert_eq!(request.system, "");
    assert_eq!((request.stable, request.continuation), (0, false));
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
    let (text, _) = title(log.history(), &texts()).expect("有回答");
    text.strip_prefix("<title>\n")
        .expect("指令在最前")
        .to_string()
}

#[test]
fn only_the_first_turn_goes_in() {
    let mut log = Log::new();
    let (_, answered) = turn(&mut log, "看看 src", "src 下有两个文件。");
    turn(&mut log, "再看 tests", "tests 下有一个。");
    let (text, upto) = title(log.history(), &texts()).unwrap();
    assert_eq!(upto, answered, "照到的是第一个回答");
    assert_eq!(text, "<title>\nU: 看看 src\n\nA: src 下有两个文件。");
}

/// 第一个回答是那一轮最后一条有正文的回复：中间一步说的话、工具、思考、事实都不要。
#[test]
fn the_answer_is_the_last_words_of_that_turn() {
    let mut log = Log::new();
    let asked = log.say("读一下 lib.rs");
    log.start(asked);
    log.fact("<env/>");
    let call = log.reply_calling("我先读一下。");
    log.result(&call, "ok", "pub mod assemble;");
    let answered = log.reply(
        r#"[{"type":"reasoning","text":"想一想"},{"type":"text","text":"  只有一个模块。\n"}]"#,
    );
    log.end("completed");
    assert_eq!(transcript_in(&log), "U: 读一下 lib.rs\n\nA: 只有一个模块。");
    assert_eq!(title(log.history(), &texts()).unwrap().1, answered);
}

/// 第一轮没答出正文（出错了），第二轮答了：两句人的话并成一段，照到第二轮的回答。
#[test]
fn words_before_the_first_answer_join() {
    let mut log = Log::new();
    let asked = log.say("一");
    log.start(asked);
    log.end("error");
    let (_, answered) = turn(&mut log, "二", "答二");
    let (text, upto) = title(log.history(), &texts()).unwrap();
    assert_eq!(text, "<title>\nU: 一\n\n二\n\nA: 答二");
    assert_eq!(upto, answered);
}

/// 别的 harness 发来的话带着主请求里的外壳。
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

/// 没有回答的、快照里没有起标题的字的、没有回顾的标签的，都没有。
#[test]
fn nothing_without_an_answer_or_the_texts() {
    let mut log = Log::new();
    let asked = log.say("看看 src");
    assert!(title(log.history(), &texts()).is_none(), "只有一句人的话");
    log.start(asked);
    log.reply_calling("");
    assert!(title(log.history(), &texts()).is_none(), "只有调用");
    let mut log = Log::new();
    turn(&mut log, "一", "答一");
    for old in [
        Texts {
            title: None,
            ..texts()
        },
        Texts {
            recap: None,
            ..texts()
        },
    ] {
        assert!(title(log.history(), &old).is_none(), "以前造的快照");
    }
}

/// 整份连指令至多 `tokens` 个 token（字节/4）：放不下的两段各留头尾、截中间，只在字的边界上截；上限小到连标签都放不下的，
/// 整份截到上限。
#[test]
fn over_the_limit_both_sides_are_excerpted() {
    let mut log = Log::new();
    turn(&mut log, &"a".repeat(100), &"b".repeat(100));
    let mut texts = texts();
    // 指令 8 字节，限 10 个 token 就是 40 字节：对话记录 32 字节，标签和空行 8 字节，两段各 12 字节，记号 7 字节。
    texts.title = Some(Title {
        tokens: 10,
        ..title_texts()
    });
    let (text, _) = title(log.history(), &texts).unwrap();
    assert_eq!(text, "<title>\nU: aa\n[...]\naaa\n\nA: bb\n[...]\nbbb");
    assert_eq!(text.len(), 40);
    texts.title = Some(Title {
        tokens: 3,
        ..title_texts()
    });
    let (text, _) = title(log.history(), &texts).unwrap();
    assert_eq!(
        text, "<title>\nU: \n",
        "整份截到 12 字节：两段都截空了，只剩标签"
    );
}
