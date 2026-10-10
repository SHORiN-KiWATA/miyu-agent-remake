//! 载入和崩溃（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」第 1、2 条）：走完的会话载入
//! 以后接着走和不崩一样；坏日志拒绝；崩在各处都收尾、不接着开；崩之前接受过的编号不再生效；
//! 权限照日志回来。有计划的重启在 [`super::restart`]。

use super::executor::*;
use super::question::{build_question, picked, reply};
use super::*;
use crate::event::{EndReason, ToolStatus};
use crate::id::ModuleId;
use crate::ledger::LedgerError;
use crate::tool::Access;

/// 一个活着的会话，一路记下它追加过的事件，好从哪一条「崩」掉再载入。
pub(super) struct Logged {
    pub(super) session: Session,
    pub(super) log: Vec<Event>,
}

impl Logged {
    /// 造一个会话，第 1 条落了盘。
    pub(super) fn new() -> Logged {
        let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
        let (mut session, actions) = Session::create(
            session_id(),
            id(0),
            alice(),
            at(0),
            created,
            policy(),
            environment("~/src/miyu"),
        );
        let log = appended_events(&actions);
        session.handle(stored(1));
        Logged { session, log }
    }

    /// 送进一条输入，记下追加的事件。
    pub(super) fn handle(&mut self, input: Input) -> Vec<Action> {
        let actions = self.session.handle(input);
        self.log.extend(appended_events(&actions));
        actions
    }

    /// 同上，执行前的链都放行。
    pub(super) fn allowing(&mut self, input: Input) -> Vec<Action> {
        let actions = allowing(&mut self.session, input);
        self.log.extend(appended_events(&actions));
        actions
    }

    /// 最后一条的序号。
    pub(super) fn last(&self) -> u64 {
        self.log.last().map_or(0, |event| event.seq.get())
    }

    /// 发 `words`，开回合，全落了盘，挂接点跑完，请求交给了执行器：返回这次请求的 `seen`。
    pub(super) fn ask(&mut self, n: u64, words: &str) -> u64 {
        self.open(n, words).0.get()
    }

    /// 同上，返回这次请求：`seen`，和替身的组装列出来的有效历史。
    pub(super) fn open(&mut self, n: u64, words: &str) -> (Seq, String) {
        self.handle(send(n, words));
        self.handle(stored(self.last()));
        let turn = self
            .log
            .iter()
            .rev()
            .find(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| TurnId::new(event.seq))
            .unwrap();
        let actions = self.handle(hooks_done(turn, Vec::new()));
        calls(&actions).remove(0)
    }

    /// 请求 `seen` 调了这几件工具，说完了，回复落了盘，链都放行。
    pub(super) fn tools(&mut self, seen: u64, tools: &[(&str, &str)]) {
        let actions = call_tools(&mut self.session, seen, tools);
        self.log.extend(appended_events(&actions));
        self.allowing(stored(self.last()));
    }

    /// 请求 `seen` 说了 `text`、说完了，都落了盘。
    pub(super) fn say(&mut self, seen: u64, text: &str) {
        let actions = answer(&mut self.session, seen, text);
        self.log.extend(appended_events(&actions));
        self.handle(stored(self.last()));
    }

    /// 崩了：只剩落了盘的，到第 `upto` 条。
    pub(super) fn upto(&self, upto: u64) -> Vec<Event> {
        self.log
            .iter()
            .filter(|event| event.seq.get() <= upto)
            .cloned()
            .collect()
    }
}

/// 在 07:00:55 载入这份日志。
pub(super) fn load(log: Vec<Event>) -> (Session, Vec<Action>) {
    Session::load(
        session_id(),
        log,
        at(55),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap()
}

pub(super) fn ended_with(event: &Event) -> &EndReason {
    match &event.body {
        Body::TurnEnded(ended) => &ended.reason,
        body => panic!("应该是 turn.ended：{body:?}"),
    }
}

/// 走完了第 `last` 条的会话接着说一句：交给执行器的那次请求。
fn next_request(session: &mut Session, last: u64) -> (Seq, String) {
    session.handle(send(2, "再来"));
    session.handle(stored(last + 2));
    calls(&session.handle(hooks_done(TurnId::new(seq(last + 2)), Vec::new()))).remove(0)
}

#[test]
fn a_finished_session_loads_and_goes_on_the_same() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    let (mut loaded, actions) = load(logged.log.clone());
    assert!(actions.is_empty(), "走完了的，载入时什么都不补");
    assert_eq!(
        next_request(&mut loaded, 8),
        next_request(&mut logged.session, 8)
    );
}

#[test]
fn a_broken_log_is_refused() {
    assert_eq!(
        Session::load(session_id(), Vec::new(), at(55), policy(), environment("~")).unwrap_err(),
        LoadError::Empty
    );
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    let mut gap = logged.upto(5);
    gap.remove(2);
    let Err(LoadError::Broken(LedgerError { seq: broken, why })) =
        Session::load(session_id(), gap, at(55), policy(), environment("~"))
    else {
        panic!("跳了号的日志应该拒绝");
    };
    assert_eq!(broken, seq(4));
    assert!(why.contains("seq should be 3"), "{why}");
}

#[test]
fn a_crash_anywhere_closes_the_turn_and_waits() {
    // 回合刚开头、挂接点在跑、请求在路上：日志里都只到第 5 条，没有调用，只结束这一轮。
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    let (_, actions) = load(logged.upto(5));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6]));
    assert_eq!(ended_with(&events[0]), &EndReason::Aborted);
    assert_eq!(
        (
            events[0].by.clone(),
            events[0].cause.clone(),
            events[0].turn,
            events[0].at
        ),
        (By::Kernel, Some(id(1)), Some(turn3()), at(55))
    );
    // 工具在跑：没结果的调用都补上，再结束。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}"), ("write", "{}")]);
    let (mut loaded, actions) = load(logged.upto(7));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9, 10]));
    for (k, call_id) in [(0, call(6, 1)), (1, call(6, 2))] {
        assert_eq!(
            result_of(&events[k]),
            (
                call_id,
                ToolStatus::Cancelled,
                By::Kernel,
                "restarted".to_string()
            )
        );
    }
    assert_eq!(ended_with(&events[2]), &EndReason::Aborted);
    assert!(stopped(&actions).is_empty(), "执行器已经没了，不用叫停");
    // 不接着开；落了盘跑回合结束的挂接点；你开口时照常开一轮。
    assert!(
        loaded
            .handle(stored(10))
            .contains(&Action::RunTurnEndHooks { turn: turn3() })
    );
    assert_eq!(appended(&loaded.handle(send(2, "接着来"))).len(), 2);
}

#[test]
fn a_crash_while_you_are_asked_closes_what_waits() {
    let ask = Verdict::Ask {
        module: ModuleId::parse("permissions").unwrap(),
        access: Access::Write,
        rule: None,
        detail: None,
    };
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    logged.handle(stored(7));
    logged.handle(guarded(call(6, 1), ask));
    let (_, actions) = load(logged.upto(8));
    assert_eq!(appended(&actions), seqs(&[9, 10]), "确认跟着了结");
    // 在等回答的也一样。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("ask_user", "{}")]);
    logged.handle(asks(call(6, 1), build_question()));
    let (mut loaded, actions) = load(logged.upto(8));
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    loaded.handle(stored(10));
    assert_eq!(
        loaded.handle(reply(2, call(6, 1), vec![picked(&["保留"])])),
        [Action::Reply {
            id: id(2),
            outcome: Outcome::Rejected {
                reason: Reason::NotAsking,
                about: Vec::new(),
            },
        }],
        "题跟着了结了"
    );
}

#[test]
fn commands_seen_before_a_crash_are_not_applied_again() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    let (mut loaded, _) = load(logged.log.clone());
    // 照日志重建，和没重启时一样：发消息的只有 `message.user` 那一条，它开的那一轮不在里面（施工 2-1 补）。
    assert_eq!(loaded.handle(send(1, "hi")), [accepted_reply(1, &[2])]);
    assert_eq!(appended(&loaded.handle(send(2, "新的"))), seqs(&[9, 10]));
}

#[test]
fn the_permission_in_force_comes_back() {
    use super::permission::read_only;
    let mut logged = Logged::new();
    logged.handle(read_only(1, true));
    let seen = logged.ask(2, "hi");
    logged.say(seen, "好");
    let (mut loaded, _) = load(logged.log.clone());
    // 只读还开着：开回合时不再注入权限那一块，写文件的照样拦下。
    assert_eq!(
        appended(&loaded.handle(send(3, "写个文件"))),
        seqs(&[10, 11])
    );
    loaded.handle(stored(11));
    loaded.handle(hooks_done(TurnId::new(seq(11)), Vec::new()));
    let events = appended_events(&call_tools(&mut loaded, 11, &[("write", "{}")]));
    assert_eq!(result_of(&events[2]).1, ToolStatus::Denied);
}

#[test]
fn load_errors_say_it_in_english() {
    // 写进运行日志（施工 4-9 再补四中：原来是中文）。
    assert_eq!(LoadError::Empty.to_string(), "the log has no events");
    let broken = LoadError::Broken(LedgerError {
        seq: seq(4),
        why: "seq should be 3".to_string(),
    });
    assert_eq!(
        broken.to_string(),
        "the log is broken: event 4 cannot be appended: seq should be 3"
    );
}

/// 还算数的检查点重读过文件的，载入交回的动作里第一个是取回原文，崩了的那一轮收尾排在它后面（施工 6-9）。
#[test]
fn a_checkpoint_with_reread_files_recalls_them_first() {
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    let blob = crate::id::ContentHash::of(b"fn main() {}\n");
    let seq_now = seq(logged.last() + 1);
    logged.log.push(Event {
        seq: seq_now,
        at: at(54),
        turn: Some(turn3()),
        by: By::Kernel,
        cause: None,
        body: Body::ContextCompacted(crate::event::ContextCompacted {
            upto: seq(2),
            summary: "S".to_string(),
            trigger: None,
            instructions: None,
            notes: String::new(),
            restored: vec![crate::event::RestoredFile {
                path: "main.rs".to_string(),
                blob: blob.clone(),
                tokens: 4,
            }],
            refills: None,
            prepared: false,
        }),
    });
    let (_, actions) = load(logged.log);
    assert_eq!(actions[0], Action::Recall { blobs: vec![blob] });
    assert!(
        matches!(&actions[1..], [Action::Append(events)] if matches!(events.last().map(|event| &event.body), Some(Body::TurnEnded(_)))),
        "崩了的那一轮收尾排在后面：{actions:?}"
    );
}

/// 接受过的编号再来，重启前后回应一样（施工 2-1 补，`kernel/session.md`「命令和回应」第 3、4 条）：发消息的只有那一条，清空的
/// 是那一轮的 `turn.started`。载入时照日志重建：一个编号的事件在它开的头一个 `turn.started` 前面截住，头一条就是
/// `turn.started` 的（手动压缩、清空）只留它。
#[test]
fn a_command_sent_again_gets_the_same_reply_before_and_after_a_reload() {
    use super::clear::clear;
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    let said = logged.handle(send(1, "hi"));
    logged.handle(clear(2));
    logged.handle(stored(logged.last()));
    let cleared = logged.handle(clear(2));
    assert_eq!(said, [accepted_reply(1, &[2])], "没重启：只有那一条");
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.handle(send(1, "hi")), said);
    assert_eq!(loaded.handle(clear(2)), cleared);
}
