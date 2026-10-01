//! 任务表和别处来的话落进正文的字（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 5、6 条、「别处来的话」第 2 条）：
//! 任务结束的那一行、子代理正在做什么、用了多少 token、别的 harness 报的名字怎么洗。

use std::time::Instant;

use miyu_store::human::Human;
use unicode_width::UnicodeWidthChar;

use crate::config::JobTexts;
use crate::core::Usage;
use crate::jobs::{Board, JobKind, JobState};
use crate::meter;
use crate::transcript::{Entry, JobMark, Kind, StepKind, Transcript};

/// 一条后台任务结束了：正文末尾一行，记号有颜色、字暗；点开看全文（「后台命令、子代理和侧边栏」第 5 条）。
pub(super) fn announce(
    board: &Board,
    transcript: &mut Transcript,
    id: u64,
    now: Instant,
    words: &JobTexts,
) {
    let Some(job) = board.jobs.iter().find(|j| j.id == id) else {
        return;
    };
    let elapsed = meter::clock(job.elapsed(now).as_secs());
    let why = match job.state {
        JobState::Undone => words.why_undone.as_str(),
        JobState::Restarted => words.why_restarted.as_str(),
        _ => "",
    };
    let stopped = matches!(
        job.state,
        JobState::Stopped | JobState::Undone | JobState::Restarted
    );
    let (mark, text) = match (job.kind, job.state) {
        (JobKind::Agent, _) if stopped => (JobMark::Stopped, words.agent_stopped_note.clone()),
        (JobKind::Agent, _) => (JobMark::Done, words.agent_note.clone()),
        (_, JobState::Failed(code)) => (
            JobMark::Failed,
            words.failed_note.replace("{code}", &code.to_string()),
        ),
        (_, JobState::Killed(signal)) => (
            JobMark::Failed,
            words.killed_note.replace("{signal}", &signal.to_string()),
        ),
        _ if stopped => (JobMark::Stopped, words.stopped_note.clone()),
        _ => (JobMark::Done, words.done_note.clone()),
    };
    let text = text
        .replace("{title}", &job.headline())
        .replace("{elapsed}", &elapsed)
        .replace("{why}", why);
    // 点开看的全文：子代理是交回来的回答；命令先是命令本身，输出读回来了接在后面（`app/output.rs`）。
    let detail = match job.kind {
        JobKind::Agent => job.report.clone(),
        JobKind::Shell => job.title.trim().to_string(),
    };
    transcript.job_from(mark, text, detail, Some(job.job.clone()));
}

/// 子会话正在做什么：在想、在说，或者最新那一步「显示名 · 对象」；没在跑的空着。
pub(super) fn doing(transcript: &Transcript, human: &Human, words: &JobTexts) -> String {
    if transcript.running.is_none() {
        return String::new();
    }
    // 排着的话（在忙时切进去说的）不算：看她这一轮最后在做的。
    let doing = |e: &&Entry| !e.hidden && !e.queued;
    let Some(last) = transcript.entries.iter().rev().find(doing) else {
        return String::new();
    };
    if last.kind == Kind::Reply {
        return words.doing_replying.clone();
    }
    let step = last.segment.as_ref().and_then(|s| s.steps.last());
    match step.map(|s| (&s.kind, s)) {
        Some((StepKind::Thought { .. }, _)) => words.doing_thinking.clone(),
        Some((StepKind::Tool { name, .. }, step)) => {
            let face = human.tool(name);
            let mut out = face.map_or(name.clone(), |f| f.name.clone());
            let subject = face
                .and_then(|f| f.subject.as_deref())
                .or((name == "shell").then_some("description"))
                .and_then(|k| step.arg(k));
            if let Some(subject) = subject {
                out.push_str(" · ");
                out.push_str(&crate::local::home_short(subject));
            }
            out
        }
        None => String::new(),
    }
}

/// 子会话一共用了多少 token（照它的 `model.called` 加起来）。
pub(super) fn tokens(transcript: &Transcript) -> u64 {
    let total = &transcript.total;
    total.input() + total.output
}

/// 会话发来的话是谁发的（「别处来的话」第 2 条）。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Relation {
    /// 自己派的子代理（孙代理也是），带任务编号。
    Agent(String),
    /// 收话的是子代理：发话的是派它的那个会话。
    Parent,
    /// 别的主会话（核心 C-5）。
    Other,
}

/// 照发话的会话是不是谁派的子代理（`sender_job`）、收话的是不是子代理（`receiver_is_child`）定关系。只看这两个会话，
/// 不看界面正在显示哪个（切会话补发时收话的那个还停放着，2026-10-01 实测过照显示的判会把别的会话认成主会话）。
pub(super) fn relation(sender_job: Option<String>, receiver_is_child: bool) -> Relation {
    match sender_job {
        Some(job) => Relation::Agent(job),
        None if receiver_is_child => Relation::Parent,
        None => Relation::Other,
    }
}

/// 「空了告诉我」那一行（核心 C-6，「别处来的话」第 5 条）：空下来了绿点，等作废了、那个会话没了暗点，认不得的原样写。
pub(super) fn peer_note(
    id: &str,
    title: Option<&str>,
    reason: &str,
    status: Option<&str>,
    words: &JobTexts,
) -> (JobMark, String) {
    let short = crate::session_list::short(id);
    let who = match title {
        Some(title) => words
            .peer_who
            .replace("{id}", &short)
            .replace("{title}", &untrusted(title)),
        None => words.peer_who_untitled.replace("{id}", &short),
    };
    match reason {
        "idle" => {
            let mut text = words.peer_idle.replace("{who}", &who);
            if let Some(status) = status.filter(|s| !s.trim().is_empty()) {
                text.push_str(" · ");
                text.push_str(&untrusted(status));
            }
            (JobMark::Done, text)
        }
        "expired" => (JobMark::Stopped, words.peer_expired.replace("{who}", &who)),
        "gone" => (JobMark::Stopped, words.peer_gone.replace("{who}", &who)),
        other => (
            JobMark::Stopped,
            words
                .peer_other
                .replace("{who}", &who)
                .replace("{reason}", &untrusted(other)),
        ),
    }
}

/// 别的主会话发来的话的来处（「别处来的话」第 2 条，核心 C-5）：短编号，有标题的再写标题（标题照不可信的字清理）。
pub(super) fn from_session(id: &str, title: Option<&str>, words: &JobTexts) -> String {
    let short = crate::session_list::short(id);
    match title {
        Some(title) => words
            .from_session
            .replace("{id}", &short)
            .replace("{title}", &untrusted(title)),
        None => words.from_session_untitled.replace("{id}", &short),
    }
}

/// 别的 harness 报的名字不可信（「别处来的话」第 2 条）：去掉控制字符、换行换成空格，最多 64 列，放不下的截掉、末尾写 `…`。
pub(super) fn untrusted(name: &str) -> String {
    const MOST: usize = 64;
    let clean: Vec<char> = name
        .chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !c.is_control())
        .collect();
    let width = |c: &char| c.width().unwrap_or(0);
    if clean.iter().map(width).sum::<usize>() <= MOST {
        return clean.into_iter().collect();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in clean {
        if used + width(&c) > MOST - 1 {
            break;
        }
        used += width(&c);
        out.push(c);
    }
    out.push('…');
    out
}

/// `own` 加上 `board` 里派出去的子代理（一层层往下）停放着的正文里用的（`App::usage_total`）。
pub(super) fn tree_usage(own: Usage, board: &Board, parked: &super::Lot) -> Usage {
    let mut total = own;
    let mut boards = vec![board];
    while let Some(board) = boards.pop() {
        for child in board.jobs.iter().filter_map(|j| j.session.as_deref()) {
            if let Some(p) = parked.get(child) {
                let usage = p.transcript.total;
                total.uncached += usage.uncached;
                total.cache_read += usage.cache_read;
                total.cache_write += usage.cache_write;
                total.output += usage.output;
                total.aux += usage.aux;
                boards.push(&p.board);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{announce, untrusted};
    use crate::config::Config;
    use crate::core::{JobEnd, JobReason, JobStart};
    use crate::jobs::Board;
    use crate::transcript::Transcript;

    #[test]
    fn a_harness_name_is_cleaned_and_clipped() {
        // 名字不可信（「别处来的话」第 2 条）：控制字符去掉、换行换成空格，最多 64 列。
        assert_eq!(untrusted("claude\u{1b}[31m-code\nx"), "claude[31m-code x");
        let width = |s: &str| unicode_width::UnicodeWidthStr::width(s);
        let long = untrusted(&"名".repeat(40));
        assert!(long.ends_with('…'));
        assert!(width(&long) <= 64);
        let exact = "a".repeat(64);
        assert_eq!(untrusted(&exact), exact, "正好 64 列的不截");
    }

    #[test]
    fn a_child_watched_after_its_turn_started_still_says_what_it_is_doing() {
        // 订阅只推订阅以后的：刚派出去的子代理那一轮的 `turn.started` 多半已经过去了，照样认它在跑（实测撞见：
        // 子代理状态行「正在做什么」一直空着）。
        use crate::core::{Block, Push, Update};
        let config = Config::builtin().unwrap();
        let mut child =
            Transcript::default().child("s-child".into(), "主会话".into(), "读 a.md".into());
        let pushes = [
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 0,
                text: r#"{"command":"sleep 15","description":"等十五秒"}"#.into(),
            },
            Push::BlockEnd(0),
        ];
        for push in pushes {
            child.update(Update::Push(push), &config.text);
        }
        let human = miyu_store::human::Human::default();
        assert_eq!(
            super::doing(&child, &human, &config.text.jobs),
            "shell · 等十五秒"
        );
        assert!(!child.entries[0].queued, "交代的活是开这一轮的，不是排着的");
        // 切进去说一句：它在忙，这句排着；正在做什么照旧是那一步。
        child.user("a.md 有几行".into(), Vec::new());
        assert!(child.entries.last().unwrap().queued);
        assert_eq!(
            super::doing(&child, &human, &config.text.jobs),
            "shell · 等十五秒"
        );
    }

    #[test]
    fn an_ended_job_leaves_a_line_that_says_how_it_ended() {
        let words = Config::builtin().unwrap().text.jobs;
        let t0 = Instant::now();
        let start = |job: &str, agent: bool| JobStart {
            call_id: String::new(),
            job: job.into(),
            agent,
            title: "查文档".into(),
            session: agent.then(|| format!("s-{job}")),
        };
        let end = |job: &str, reason, exit_code| JobEnd {
            job: job.into(),
            reason,
            exit_code,
            signal: None,
            duration_ms: Some(20_000),
            text: "两处说法不一样".into(),
        };
        let mut board = Board::default();
        let mut t = Transcript::default();
        let cases = [
            (
                "j1",
                false,
                JobReason::Finished,
                Some(0),
                "后台命令完成 · cargo test · 20s",
            ),
            (
                "j2",
                false,
                JobReason::Finished,
                Some(1),
                "后台命令失败 · cargo test · 退出码 1",
            ),
            (
                "j3",
                false,
                JobReason::Undone,
                None,
                "后台命令已停止 · cargo test · 撤销时停了",
            ),
            (
                "j4",
                true,
                JobReason::Finished,
                None,
                "后台任务完成 · 查文档 · 20s",
            ),
            (
                "j5",
                true,
                JobReason::Restarted,
                None,
                "后台任务已停止 · 查文档 · 核心重启时停了",
            ),
        ];
        for (job, agent, reason, code, text) in cases {
            board.start(&start(job, agent), Some("cargo test".into()), t0);
            let id = board
                .end(&end(job, reason, code), t0 + Duration::from_secs(99))
                .unwrap();
            announce(&board, &mut t, id, t0, &words);
            let last = t.entries.last().unwrap();
            assert_eq!(last.text, text);
            let detail = &last.job.as_ref().unwrap().detail;
            // 子代理点开是它交回来的回答；命令先是命令本身，输出读回来了接在后面（2026-09-30 项目主人：原来看不到完整命令）。
            let want = if agent {
                "两处说法不一样"
            } else {
                "cargo test"
            };
            assert_eq!(detail, want);
        }
    }

    #[test]
    fn the_usage_of_agents_adds_up_into_the_session_that_sent_them() {
        // 2026-09-30 项目主人：子代理用的 token 算进主会话，框下面那一行跟着变；孙代理也算。
        use super::super::{Lot, Parked};
        use crate::core::Usage;
        let t0 = Instant::now();
        let used = |uncached, output| Usage {
            uncached,
            cache_read: 100,
            cache_write: 0,
            output,
            aux: 0,
        };
        let start = |job: &str, session: &str| JobStart {
            call_id: String::new(),
            job: job.into(),
            agent: true,
            title: String::new(),
            session: Some(session.into()),
        };
        let mut main = Board::default();
        main.start(&start("j1", "s1"), None, t0);
        let mut child = Parked::default();
        child.transcript.total = used(10, 5);
        child.board.start(&start("j1.1", "s2"), None, t0);
        let mut grandchild = Parked::default();
        grandchild.transcript.total = used(1, 1);
        let mut lot = Lot::new();
        lot.insert("s1".into(), child);
        lot.insert("s2".into(), grandchild);
        let total = super::tree_usage(used(1000, 50), &main, &lot);
        assert_eq!(
            (total.uncached, total.cache_read, total.output),
            (1011, 300, 56)
        );
        assert_eq!(
            super::tree_usage(used(1, 1), &Board::default(), &lot).uncached,
            1,
            "没派过的只算自己"
        );
    }

    #[test]
    fn a_message_from_another_session_names_it_by_short_id_and_title() {
        // 2026-10-01 项目主人定照终端这一种写法（「别处来的话」第 2 条）。
        let words = crate::config::Config::builtin().unwrap().text.jobs;
        let id = "0192f3a0-1111-7abc-8def-001122334455";
        assert_eq!(
            super::from_session(id, Some("修 CI"), &words),
            "从会话 22334455「修 CI」收到消息"
        );
        assert_eq!(
            super::from_session(id, None, &words),
            "从会话 22334455 收到消息"
        );
    }

    #[test]
    fn who_sent_it_depends_only_on_the_two_sessions() {
        use super::{Relation, relation};
        assert_eq!(
            relation(Some("j10".into()), false),
            Relation::Agent("j10".into())
        );
        assert_eq!(
            relation(None, true),
            Relation::Parent,
            "子代理收到的：派它的会话发的"
        );
        // 2026-10-01 实测：切到会话 A 时 A 还在补发、界面上显示的是发话的 B，原来把 B 认成了主会话。
        assert_eq!(
            relation(None, false),
            Relation::Other,
            "主会话收到的不会是主会话发的"
        );
    }

    #[test]
    fn a_peer_going_idle_reads_like_a_job_ending_that_replied() {
        // 核心 C-6：2026-10-01 项目主人定照后台任务结束那一行画，绿点暗点；「空下来了」说法怪，说成「回复」。
        use crate::transcript::JobMark;
        let words = crate::config::Config::builtin().unwrap().text.jobs;
        let id = "0192f3a0-1111-7abc-8def-001122334455";
        let (mark, text) = super::peer_note(id, Some("工人"), "idle", Some("55"), &words);
        assert!(matches!(mark, JobMark::Done));
        assert_eq!(text, "会话 22334455「工人」回复 · 55");
        let (_, text) = super::peer_note(id, None, "idle", Some("55"), &words);
        assert_eq!(text, "会话 22334455 回复 · 55");
        let (mark, text) = super::peer_note(id, None, "expired", None, &words);
        assert!(matches!(mark, JobMark::Stopped));
        assert_eq!(text, "会话 22334455 等了 12 小时没空下来，不等了");
        let (_, text) = super::peer_note(id, Some("工人"), "gone", None, &words);
        assert_eq!(text, "会话 22334455「工人」已经不在了，不等了");
        let (mark, text) = super::peer_note(id, None, "paused", None, &words);
        assert!(matches!(mark, JobMark::Stopped));
        assert_eq!(text, "会话 22334455 · paused", "认不得的原样写");
    }
}
