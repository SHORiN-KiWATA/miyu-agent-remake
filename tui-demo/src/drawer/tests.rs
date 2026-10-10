//! 抽屉（蓝图「确认和提问的抽屉」）：选、勾、进编辑写「其他」、按 n 补充、换题、「确认」页、交回去的形状、
//! 了结以后留什么。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    Answer, Approval, Asked, Decision, Drawer, Drawers, Edit, Item, Outcome, Report, Step,
};
use crate::config::Config;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn typed(d: &mut Drawer, text: &str) {
    for c in text.chars() {
        d.key(key(KeyCode::Char(c)));
    }
}

/// 核心推的样本（`docs/designs/samples/events/`）：照它的 `body` 读。
fn sample<T: serde::de::DeserializeOwned>(jsonl: &str) -> T {
    let event: serde_json::Value = serde_json::from_str(jsonl.lines().next().unwrap()).unwrap();
    serde_json::from_value(event["body"].clone()).unwrap()
}

fn asked() -> Asked {
    sample(include_str!(
        "../../../docs/designs/samples/events/question.asked.jsonl"
    ))
}

fn approval() -> Approval {
    sample(include_str!(
        "../../../docs/designs/samples/events/tool.approval_requested.jsonl"
    ))
}

/// 两道题：第一道单选（带短名），第二道能多选（没有短名）。
fn two() -> Asked {
    serde_json::from_value(serde_json::json!({
        "call_id": "c1",
        "questions": [
            {"header": "build", "question": "删不删？",
             "options": [{"label": "删掉"}, {"label": "保留", "description": "只清缓存", "preview": "┌─┐\n└─┘"}]},
            {"question": "要哪几样？", "multiple": true,
             "options": [{"label": "甲"}, {"label": "乙"}, {"label": "丙"}]}
        ]
    }))
    .unwrap()
}

fn answers(step: Step) -> Vec<Answer> {
    match step {
        Step::Done(Outcome::Answered(a)) => a.answers,
        other => panic!("没交：{other:?}"),
    }
}

fn picked(labels: &[&str]) -> Answer {
    Answer {
        picked: labels.iter().map(|s| (*s).to_string()).collect(),
        ..Answer::default()
    }
}

#[test]
fn the_kernel_samples_read_and_there_is_no_decline() {
    let d = Drawer::question(None, asked());
    // 两个选项，接着「输入其他答案」；没有「不回答」（第 3 条）。
    assert_eq!(
        d.items(0),
        vec![Item::Choice(0), Item::Choice(1), Item::Other]
    );
    let a = Drawer::approval(None, approval());
    assert_eq!(
        a.items(0).len(),
        3,
        "允许本次、本会话内始终允许、不允许（核心 D-1 还没有工作区）"
    );
    let mut bare = approval();
    bare.rule = None;
    assert_eq!(
        Drawer::approval(None, bare).items(0),
        vec![
            Item::Decision(Decision::Once),
            Item::Decision(Decision::Deny)
        ],
        "没提放行规则的不给「本会话内始终允许」：选了核心回 no_rule"
    );
    assert!(!d.has_review(), "一道题没有「确认」页");
    assert!(Drawer::question(None, two()).has_review());
}

#[test]
fn one_question_submits_on_enter_and_the_shape_is_question_answered() {
    let mut d = Drawer::question(None, asked());
    d.key(key(KeyCode::Down));
    let got = answers(d.key(key(KeyCode::Enter)));
    assert_eq!(got, vec![picked(&["保留"])]);
    // 交回去的形状：`{call_id, answers:[{picked}]}`，没有的字段不写。
    let body = serde_json::to_value(super::Answered {
        call_id: "call_77_1".into(),
        answers: got,
    })
    .unwrap();
    assert_eq!(
        body,
        serde_json::json!({"call_id": "call_77_1", "answers": [{"picked": ["保留"]}]})
    );
}

#[test]
fn several_questions_walk_to_the_review_page_and_submit_there() {
    let mut d = Drawer::question(None, two());
    assert_eq!(d.key(key(KeyCode::Enter)), Step::Stay);
    assert_eq!(d.tab, 1, "跳到下一道没答的");
    // 能多选：空格勾上甲、丙，Enter 交这一道；都答了，到「确认」页，不直接交。
    d.key(key(KeyCode::Char(' ')));
    d.key(key(KeyCode::Down));
    d.key(key(KeyCode::Down));
    d.key(key(KeyCode::Char(' ')));
    assert_eq!(d.key(key(KeyCode::Enter)), Step::Stay);
    assert!(d.on_review(), "都答了到「确认」页");
    let got = answers(d.key(key(KeyCode::Enter)));
    assert_eq!(got, vec![picked(&["删掉"]), picked(&["甲", "丙"])]);
}

#[test]
fn unanswered_questions_are_submitted_empty_from_the_review_page() {
    let mut d = Drawer::question(None, two());
    // 第一道不答，直接换到「确认」页交：没答的那道是空的回答。
    d.key(key(KeyCode::Left));
    assert!(d.on_review(), "往左绕到「确认」页");
    let got = answers(d.key(key(KeyCode::Enter)));
    assert_eq!(got, vec![Answer::default(), Answer::default()]);
}

#[test]
fn multiple_with_nothing_checked_takes_the_one_under_the_cursor() {
    let mut d = Drawer::question(None, two());
    d.key(key(KeyCode::Right));
    d.key(key(KeyCode::Down));
    assert_eq!(d.key(key(KeyCode::Enter)), Step::Stay);
    assert_eq!(d.tab, 0, "第一道还没答：跳回去");
    assert_eq!(d.answers[1], Some(picked(&["乙"])));
}

#[test]
fn other_needs_enter_to_edit_and_enter_saves_it() {
    let mut d = Drawer::question(None, asked());
    d.key(key(KeyCode::Down));
    d.key(key(KeyCode::Down));
    assert_eq!(d.current(), Some(Item::Other));
    // 没进编辑：打的数字是选第几项，不是字。
    assert_eq!(d.editing, None);
    assert_eq!(d.key(key(KeyCode::Enter)), Step::Stay);
    assert_eq!(d.editing, Some(Edit::Other), "Enter 才进编辑");
    typed(&mut d, "先 2 天");
    d.key(key(KeyCode::Backspace));
    d.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    typed(&mut d, "再说");
    // Esc 只退出编辑，字留着。
    assert_eq!(d.key(key(KeyCode::Esc)), Step::Stay);
    assert_eq!(d.editing, None);
    d.key(key(KeyCode::Enter));
    let got = answers(d.key(key(KeyCode::Enter)));
    assert_eq!(got[0].text.as_deref(), Some("先 2 \n再说"));
    assert!(got[0].picked.is_empty());
}

#[test]
fn n_adds_notes_without_picking_other() {
    let mut d = Drawer::question(None, asked());
    d.key(key(KeyCode::Char('n')));
    assert_eq!(d.editing, Some(Edit::Notes));
    typed(&mut d, "顺便清 target");
    d.key(key(KeyCode::Enter));
    assert_eq!(d.editing, None, "Enter 保存补充");
    let got = answers(d.key(key(KeyCode::Char('1'))));
    assert_eq!(got[0].picked, vec!["删掉".to_string()]);
    assert_eq!(got[0].notes.as_deref(), Some("顺便清 target"));
}

#[test]
fn digits_pick_the_nth_item() {
    let mut d = Drawer::question(None, asked());
    assert_eq!(
        answers(d.key(key(KeyCode::Char('1'))))[0],
        picked(&["删掉"])
    );
    let mut d = Drawer::question(None, asked());
    assert_eq!(d.key(key(KeyCode::Char('3'))), Step::Stay);
    assert_eq!(d.editing, Some(Edit::Other), "第 3 项是「其他」：进编辑");
    let mut d = Drawer::question(None, asked());
    assert_eq!(d.key(key(KeyCode::Char('9'))), Step::Stay, "没有第 9 项");
}

#[test]
fn esc_twice_within_the_window_cancels() {
    use std::time::{Duration, Instant};
    let window = Duration::from_millis(1500);
    let t0 = Instant::now();
    let mut d = Drawer::question(None, asked());
    // 第一下只是准备好，提示再按一次；过了时限再按，重新算第一下。
    assert_eq!(d.key(key(KeyCode::Esc)), Step::Escape);
    assert_eq!(d.escape(t0, window), Step::Stay);
    assert!(d.armed(t0, window));
    let late = t0 + Duration::from_millis(2000);
    assert!(!d.armed(late, window), "过了时限提示消失");
    assert_eq!(d.escape(late, window), Step::Stay);
    // 中间按了别的键：也重新算。
    d.key(key(KeyCode::Down));
    assert!(!d.armed(late, window));
    assert_eq!(d.escape(late, window), Step::Stay);
    let soon = late + Duration::from_millis(300);
    assert_eq!(d.escape(soon, window), Step::Done(Outcome::Cancelled));
    // 编辑时 Esc 先退出编辑，不算第一下。
    let mut d = Drawer::question(None, asked());
    d.key(key(KeyCode::Char('n')));
    assert_eq!(d.key(key(KeyCode::Esc)), Step::Stay);
    assert_eq!(d.editing, None);
    assert!(!d.armed(t0, window));
}

#[test]
fn approval_decides_and_deny_asks_for_a_reason_first() {
    let mut d = Drawer::approval(None, approval());
    match d.key(key(KeyCode::Char('2'))) {
        Step::Done(Outcome::Decided(x)) => {
            assert_eq!((x.decision, x.reason), (Decision::Session, None));
        }
        other => panic!("{other:?}"),
    }
    let mut d = Drawer::approval(None, approval());
    assert_eq!(d.key(key(KeyCode::Char('3'))), Step::Stay);
    assert_eq!(d.editing, Some(Edit::Reason), "不允许：先写理由");
    assert_eq!(d.key(key(KeyCode::Char('n'))), Step::Stay, "编辑时 n 是字");
    d.key(key(KeyCode::Backspace));
    typed(&mut d, "别动家目录");
    let Step::Done(Outcome::Decided(x)) = d.key(key(KeyCode::Enter)) else {
        panic!("没定");
    };
    assert_eq!(
        serde_json::to_value(&x).unwrap(),
        serde_json::json!({"call_id": "call_67_1", "decision": "deny", "reason": "别动家目录"})
    );
}

#[test]
fn drawers_open_one_at_a_time() {
    let mut desk = Drawers::default();
    desk.push(Drawer::question(None, asked()));
    desk.push(Drawer::approval(None, approval()));
    assert!(desk.open() && desk.current.as_ref().unwrap().is_question());
    assert!(desk.finish().unwrap().is_question());
    assert!(desk.open(), "排着的下一个打开");
    assert!(!desk.current.as_ref().unwrap().is_question());
}

#[test]
fn what_is_left_behind() {
    let texts = Config::builtin().unwrap().text.drawer;
    // 提问：旧版的引用块，短名（没有的写问题）：回答；补充接在后面。
    let mut d = Drawer::question(None, two());
    d.key(key(KeyCode::Char('n')));
    typed(&mut d, "快点");
    d.key(key(KeyCode::Enter));
    d.key(key(KeyCode::Enter));
    let Step::Done(outcome) = ({
        d.key(key(KeyCode::Right));
        d.key(key(KeyCode::Enter))
    }) else {
        panic!("没交");
    };
    assert_eq!(
        d.report(&outcome, &texts),
        // 2026-10-11 项目主人：回答写进时间线里「提问」那一步下面，不写「已回答」那一行。
        Report::Answers(vec![
            "build：删掉（补充：快点）".into(),
            "要哪几样？：未回答".into(),
        ])
    );
    // 确认：允许了不留字；不允许红叉；取消的暗色一行。
    let a = Drawer::approval(None, approval());
    let allow = Outcome::Decided(super::Decided {
        call_id: "x".into(),
        decision: Decision::Once,
        reason: None,
    });
    assert_eq!(a.report(&allow, &texts), Report::Nothing);
    let deny = Outcome::Decided(super::Decided {
        call_id: "x".into(),
        decision: Decision::Deny,
        reason: Some("不要".into()),
    });
    assert_eq!(
        a.report(&deny, &texts),
        Report::Denied("不允许 · 不要".into())
    );
    // 2026-10-11 项目主人：取消一定顺带打断这一轮，那一步的结果、「已中断」已经说了，不再写「提问已取消」「批准已取消」。
    assert_eq!(a.report(&Outcome::Cancelled, &texts), Report::Nothing);
    assert_eq!(d.report(&Outcome::Cancelled, &texts), Report::Nothing);
}

#[test]
fn landing_on_deny_opens_the_reason_and_one_enter_submits() {
    // 2026-10-07 项目主人：光标移到「不允许」就能写理由，回车直接交，不用按两次。
    let mut d = Drawer::approval(None, approval());
    d.key(key(KeyCode::Down));
    assert_eq!(d.editing, None, "「本会话内始终允许」上不写字");
    d.key(key(KeyCode::Down));
    assert_eq!(d.editing, Some(Edit::Reason), "落到「不允许」就在写理由");
    typed(&mut d, "jk别动");
    assert_eq!(d.typed[0], "jk别动", "字母进理由，不是上下");
    d.key(key(KeyCode::Up));
    assert_eq!(d.editing, None, "方向键照样能离开");
    assert_eq!(d.typed[0], "jk别动", "写的字留着");
    d.key(key(KeyCode::Down));
    let Step::Done(Outcome::Decided(x)) = d.key(key(KeyCode::Enter)) else {
        panic!("一下就交");
    };
    assert_eq!(
        (x.decision, x.reason.as_deref()),
        (Decision::Deny, Some("jk别动"))
    );
    let mut d = Drawer::approval(None, approval());
    d.key(key(KeyCode::Char('3')));
    assert_eq!(d.editing, Some(Edit::Reason), "数字选到「不允许」也一样");
    assert_eq!(
        d.key(key(KeyCode::Esc)),
        Step::Escape,
        "Esc 照旧算两下取消的第一下"
    );
}

#[test]
fn a_command_ask_takes_its_title_and_command_from_the_core_and_says_when_unsandboxed() {
    // 核心 D-4：跑命令的确认 `detail` 带短标题、命令、`sandbox`；子会话问的也有，不用翻时间线。
    let texts = Config::builtin().unwrap().text.drawer;
    let approval: Approval = serde_json::from_value(serde_json::json!({
        "call_id": "c1", "access": "execute",
        "detail": {"command": "rm -rf build", "sandbox": false, "title": "清掉构建目录", "tool": "shell"}
    }))
    .unwrap();
    let d = Drawer::approval(None, approval);
    assert_eq!(d.title(&texts), "清掉构建目录");
    assert_eq!(
        d.command.as_ref().map(|(_, c)| c.as_str()),
        Some("rm -rf build")
    );
    assert!(d.unsandboxed(), "sandbox:false 要写出来");
    assert_eq!(d.items(0).len(), 2, "没有 rule：只有允许本次、不允许");
}

#[test]
fn vim_keys_move_and_turn_pages_while_not_typing() {
    // 2026-10-11 项目主人：「问问题抽屉不支持 vimkey 是个问题」。没在写字时 `j` `k` 上下、`h` `l` 换题，同引导。
    let mut d = Drawer::question(None, two());
    d.key(key(KeyCode::Char('j')));
    assert_eq!(d.cursor[0], 1, "j 往下");
    d.key(key(KeyCode::Char('k')));
    assert_eq!(d.cursor[0], 0, "k 往上");
    d.key(key(KeyCode::Char('l')));
    assert_eq!(d.tab, 1, "l 下一题");
    d.key(key(KeyCode::Char('h')));
    assert_eq!(d.tab, 0, "h 上一题");
    d.key(key(KeyCode::Char('n')));
    typed(&mut d, "jk");
    assert_eq!(d.notes[0], "jk", "写字时照常打字");
}
