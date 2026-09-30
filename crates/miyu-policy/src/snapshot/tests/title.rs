//! 起标题进快照（施工 3-8 五补，`docs/blueprint/policy.md`「`CoreTexts`」）：出厂的快照带着起标题的指令和三个数，造出的组装器
//! 起得出标题、内核拿到试几次和最多几个字；字、数少了哪一样都不起（回顾的标签没有的也不起）；以前造的快照没有这两格，读进来
//! 再写出去一字不差，不起。

use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::ledger::Ledger;
use miyu_kernel::session::Titles;

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

/// 照快照造的策略：起标题的那一条 user 的字（组装器），和交给内核的两个数。
fn titled(snapshot: &Snapshot) -> (Option<String>, Option<Titles>) {
    let policy = snapshot.policy().unwrap();
    let text = policy.assembler.title(&answered()).map(|(request, upto)| {
        assert_eq!(upto.get(), 4);
        serde_json::to_string(&request.messages).unwrap()
    });
    (text, policy.titles)
}

#[test]
fn title_texts_and_numbers_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    assert_eq!(snapshot.title, Some(crate::TITLE));
    let (text, titles) = titled(&snapshot);
    let text = text.expect("出厂的起得出标题");
    assert!(
        text.contains("Write a title of 3 to 7 words")
            && text.ends_with("Conversation:\\nUser: hi\\n\\nAssistant: 好。\"}]}]"),
        "{text}"
    );
    assert_eq!(
        titles,
        Some(Titles {
            tries: 2,
            chars: 50
        })
    );
    let mut no_numbers = snapshot.clone();
    no_numbers.title = None;
    assert_eq!(titled(&no_numbers), (None, None), "没有数的不起");
    let mut no_texts = snapshot.clone();
    no_texts.core.title = None;
    assert_eq!(titled(&no_texts), (None, None), "没有字的不起");
    let mut no_labels = snapshot.clone();
    no_labels.core.recap = None;
    assert_eq!(titled(&no_labels).0, None, "没有回顾的标签的不起");
    // 快照里的数照快照的。
    let mut other = snapshot.clone();
    other.title = Some(crate::TitleNumbers {
        tokens: 1,
        chars: 7,
        tries: 5,
    });
    let (tiny, titles) = titled(&other);
    assert!(
        tiny.unwrap().ends_with("Conversation:\\n\"}]}]"),
        "一个 token 也放不下指令，对话记录一个字节都不剩"
    );
    assert_eq!(titles, Some(Titles { tries: 5, chars: 7 }));

    let bytes = String::from_utf8(snapshot.to_bytes()).unwrap();
    let texts_start = bytes.find(r#","title":{"instruction""#).unwrap();
    let texts_end = texts_start + bytes[texts_start..].find(r#""}"#).unwrap() + 2;
    let older = bytes[..texts_start].to_string() + &bytes[texts_end..];
    let older = older.replacen(TITLE_NUMBERS, "", 1);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!((read.title, read.core.title.is_none()), (None, true));
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(titled(&read), (None, None));
}
