//! 提问事件的测试：图纸上的两条读写一字不差、认得出种类；没写的格子不写；回答对不对得上题目；
//! 坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const ASKED: &str = r#"{"call_id":"call_77_1","questions":[{"header":"build","question":"旧的 build 目录要删掉，还是保留？","options":[{"label":"删掉","description":"下次编译从头来"},{"label":"保留","description":"只清掉缓存"}]}]}"#;
const ANSWERED: &str = r#"{"call_id":"call_77_1","answers":[{"picked":["保留"]}]}"#;

fn choice(label: &str) -> Choice {
    Choice {
        label: label.to_string(),
        description: None,
    }
}

fn question(options: &[&str], multiple: bool) -> Question {
    Question {
        header: None,
        question: "?".to_string(),
        options: options.iter().map(|label| choice(label)).collect(),
        multiple,
    }
}

fn picked(labels: &[&str]) -> Response {
    Response {
        picked: labels.iter().map(|label| label.to_string()).collect(),
        text: None,
    }
}

#[test]
fn the_question_events_from_the_drawing_round_trip() {
    match read_body("question.asked", ASKED) {
        Body::QuestionAsked(asked) => {
            assert_eq!(asked.call_id.to_string(), "call_77_1");
            let [question] = asked.questions.as_slice() else {
                panic!("{asked:?}");
            };
            assert_eq!(question.header.as_deref(), Some("build"));
            assert_eq!(question.options.len(), 2);
            assert!(!question.multiple, "单选的不写");
        }
        other => panic!("{other:?}"),
    }
    match read_body("question.answered", ANSWERED) {
        Body::QuestionAnswered(answered) => {
            assert_eq!(answered.answers, [picked(&["保留"])]);
        }
        other => panic!("{other:?}"),
    }
}

/// 没写的格子读进来是空的，写出去也不写（read_body 查了一字不差）。
#[test]
fn what_is_not_written_is_left_out() {
    let bare = r#"{"call_id":"call_77_1","questions":[{"question":"写点什么？"},{"question":"选几个？","options":[{"label":"a"},{"label":"b"}],"multiple":true}]}"#;
    match read_body("question.asked", bare) {
        Body::QuestionAsked(asked) => {
            assert_eq!(asked.questions[0].options, []);
            assert!(asked.questions[1].multiple);
        }
        other => panic!("{other:?}"),
    }
    let unanswered = r#"{"call_id":"call_77_1","answers":[{},{"text":"自己写的"}]}"#;
    match read_body("question.answered", unanswered) {
        Body::QuestionAnswered(answered) => {
            assert_eq!(answered.answers[0], picked(&[]));
            assert_eq!(answered.answers[1].text.as_deref(), Some("自己写的"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn answers_must_fit_the_questions() {
    let questions = [question(&["a", "b"], false), question(&["x", "y"], true)];
    assert!(fits(&questions, &[picked(&["a"]), picked(&["x", "y"])]));
    assert!(fits(&questions, &[picked(&[]), picked(&[])]), "没答也行");
    for (answers, why) in [
        (vec![picked(&["a"])], "题数不对"),
        (vec![picked(&["c"]), picked(&[])], "没有这个选项"),
        (vec![picked(&["a", "b"]), picked(&[])], "单选选了两项"),
        (vec![picked(&[]), picked(&["x", "x"])], "同一项选了两次"),
    ] {
        assert!(!fits(&questions, &answers), "{why}");
    }
}

#[test]
fn broken_question_events_say_which_kind() {
    for (kind, body) in [
        ("question.asked", r#"{"call_id":"call_77_1"}"#),
        (
            "question.asked",
            r#"{"call_id":"call_77_1","questions":[{"header":"h"}]}"#,
        ),
        ("question.answered", r#"{"call_id":"call_77_1"}"#),
        (
            "question.answered",
            r#"{"call_id":"call_77_1","answers":[{"picked":"a"}]}"#,
        ),
    ] {
        let line = event_line(kind, body);
        rejected::<Event>(&line, &format!("{kind} 的 body 读不出来"));
    }
}
