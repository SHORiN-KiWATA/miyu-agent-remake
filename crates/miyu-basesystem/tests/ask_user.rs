//! `ask_user`（`docs/blueprint/tools/ask_user.md`，施工 D-2）：参数照 Claude Code，题交给端口原样（`multiSelect` 记成
//! `multiple`、`preview` 带着）；回答一道一行写成结果，逐字节比；参数不对的、一道题都没有的，端口一次都不问；没有端口的说这里
//! 没人能回答；没答到就了结的照叫停收场。

use std::sync::{Arc, Mutex};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Choice, Question, Response};
use miyu_kernel::tool::Access;
use miyu_tool::{Answering, Done, QuestionPort};

use crate::support::{Site, check, human, readable, said, tool};

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

/// 假的提问端口：记下交来的题，交回给的回答。
struct Port {
    answers: Option<Vec<Response>>,
    asked: Mutex<Vec<Vec<Question>>>,
}

impl QuestionPort for Port {
    fn ask(&self, questions: Vec<Question>) -> Answering<'_> {
        self.asked.lock().expect("没 panic").push(questions);
        let answers = self.answers.clone();
        Box::pin(async move { answers })
    }
}

fn port(answers: Option<Vec<Response>>) -> Arc<Port> {
    Arc::new(Port {
        answers,
        asked: Mutex::new(Vec::new()),
    })
}

fn response(picked: &[&str], text: Option<&str>, notes: Option<&str>) -> Response {
    Response {
        picked: picked.iter().map(|label| label.to_string()).collect(),
        text: text.map(str::to_string),
        notes: notes.map(str::to_string),
    }
}

/// 四道题：单选带预览的、多选的、没有选项的、没答的。
fn args() -> serde_json::Value {
    json!({"questions": [
        {"question": "用哪个？", "header": "库", "options": [
            {"label": "甲 (Recommended)", "description": "快", "preview": "fn a() {}\n"},
            {"label": "乙"}
        ]},
        {"question": "要哪些？", "options": [{"label": "乙"}, {"label": "丙"}], "multiSelect": true},
        {"question": "叫什么？"},
        {"question": "还有吗？", "options": [{"label": "有"}]}
    ]})
}

#[tokio::test]
async fn the_answers_come_back_one_line_each() {
    let site = Site::new();
    let port = port(Some(vec![
        response(&["甲 (Recommended)"], None, None),
        response(&["乙", "丙"], None, Some("先这样")),
        response(&[], Some("小美"), None),
        response(&[], None, None),
    ]));
    let done = site
        .done_with_questions("ask_user", args(), Some(port.clone()))
        .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "\"用哪个？\" = 甲 (Recommended)\n\"要哪些？\" = 乙, 丙 (note: 先这样)\n\"叫什么？\" = 小美\n\"还有吗？\" = (no answer)\nGo on with these answers in mind.\n"
    );
    let mut checked = Vec::new();
    check(&mut checked, human(done), said("ask_user/answered"));
    readable(&checked, &["ask_user"]);
    let asked = port.asked.lock().expect("没 panic");
    assert_eq!(asked.len(), 1);
    assert_eq!(
        asked[0][0],
        Question {
            header: Some("库".to_string()),
            question: "用哪个？".to_string(),
            options: vec![
                Choice {
                    label: "甲 (Recommended)".to_string(),
                    description: Some("快".to_string()),
                    preview: Some("fn a() {}\n".to_string()),
                },
                Choice {
                    label: "乙".to_string(),
                    description: None,
                    preview: None,
                },
            ],
            multiple: false,
        }
    );
    assert!(asked[0][1].multiple, "multiSelect 记成 multiple");
    assert!(asked[0][2].options.is_empty());
}

#[tokio::test]
async fn bad_arguments_never_reach_the_port() {
    let site = Site::new();
    for args in [
        json!({}),
        json!({"questions": []}),
        json!({"questions": [{"options": []}]}),
        json!({"questions": [{"question": "?", "options": [{"description": "没标题"}]}]}),
    ] {
        let port = port(Some(Vec::new()));
        let done = site
            .done_with_questions("ask_user", args.clone(), Some(port.clone()))
            .await;
        assert!(done.error, "{args}");
        assert!(
            text(&done).starts_with("The arguments are not right"),
            "{args}: {}",
            text(&done)
        );
        assert!(port.asked.lock().expect("没 panic").is_empty(), "{args}");
    }
}

#[tokio::test]
async fn without_a_port_no_one_can_answer() {
    let done = Site::new()
        .done_with_questions("ask_user", args(), None)
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "No one can answer questions here. Ask in your reply instead.\n"
    );
    let mut checked = Vec::new();
    check(&mut checked, human(done), said("ask_user/unattended"));
    readable(&checked, &[]);
}

#[tokio::test]
async fn settled_without_answers_ends_like_a_stop() {
    let done = Site::new()
        .done_with_questions("ask_user", args(), Some(port(None)))
        .await;
    assert!(done.stopped);
    assert!(done.blocks.is_empty());
}

#[test]
fn it_only_reads() {
    assert_eq!(tool("ask_user").spec().access, Access::Read);
}
