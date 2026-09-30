//! 回顾进快照（施工 3-8 四补，`docs/blueprint/policy.md`「`CoreTexts`」）：出厂的快照带着回顾的五份字和两个数，造出的组装器
//! 回顾得出来；字、数少了哪一样都不回顾；以前造的快照没有这两格，读进来再写出去一字不差，不回顾。

use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::ledger::Ledger;

use super::*;

/// 一段说完了一轮的有效历史：人说「hi」，她答「好。」。
fn answered() -> History {
    let lines = [
        r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"kernel"},"body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#,
        r#"{"seq":2,"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{"kind":"person","account":"alice"},"body":{"blocks":[{"type":"text","text":"hi"}]}}"#,
        r#"{"seq":3,"at":"2026-09-25T07:00:00.000Z","kind":"turn.started","turn":3,"by":{"kind":"kernel"},"body":{"trigger":2}}"#,
        r#"{"seq":4,"at":"2026-09-25T07:00:00.000Z","kind":"message.assistant","turn":3,"by":{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"},"body":{"blocks":[{"type":"text","text":"好。"}],"seen":3}}"#,
    ];
    let (mut ledger, mut history) = (Ledger::default(), History::default());
    for line in lines {
        let event = Event::from_line(line).unwrap();
        ledger.append(&event).unwrap();
        history.append(event);
    }
    history
}

/// 照快照造的组装器回顾这一段，交回那一条 user 的字。
fn recapped(snapshot: &Snapshot) -> Option<String> {
    let (request, upto) = snapshot.policy().unwrap().assembler.recap(&answered())?;
    assert_eq!(upto.get(), 4);
    Some(serde_json::to_string(&request.messages).unwrap())
}

#[test]
fn recap_texts_and_numbers_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    assert_eq!(snapshot.recap, Some(crate::RECAP));
    let text = recapped(&snapshot).expect("出厂的回顾得出来");
    assert!(
        text.contains("Write a short recap for a user")
            && text.contains("User: hi\\n\\nAssistant: 好。"),
        "{text}"
    );
    let mut no_numbers = snapshot.clone();
    no_numbers.recap = None;
    assert_eq!(recapped(&no_numbers), None, "没有数的不回顾");
    let mut no_texts = snapshot.clone();
    no_texts.core.recap = None;
    assert_eq!(recapped(&no_texts), None, "没有字的不回顾");
    // 快照里的数照快照的：一个 token 也放不下指令，对话记录一个字节都不剩。
    let mut tiny = snapshot.clone();
    tiny.recap = Some(crate::RecapNumbers {
        turns: 8,
        tokens: 1,
    });
    let tiny = recapped(&tiny).unwrap();
    assert!(tiny.ends_with("Conversation:\\n\"}]}]"), "{tiny}");

    let bytes = String::from_utf8(snapshot.to_bytes()).unwrap();
    let texts_start = bytes.find(r#","recap":{"instruction""#).unwrap();
    let texts_end = texts_start + bytes[texts_start..].find(r#""}"#).unwrap() + 2;
    let older = bytes[..texts_start].to_string() + &bytes[texts_end..];
    let older = older.replacen(RECAP_NUMBERS, "", 1);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!((read.recap, read.core.recap.is_none()), (None, true));
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(recapped(&read), None);
}
