//! 随机剧本（施工 9-8 上）：三百份随机的剧本交给执行器替身跑出真会话（想、说话、调工具、出错、打断、排队、撤销恢复、
//! 手动压缩），每一份都查视图流和翻页最后一样、照推送拼出来的和投影手里的一样（[`same`]）。种子固定，红了打印种子。

use miyu_kernel::event::ErrorClass;
use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{Line, Play, Stage};

use crate::support::*;

/// SplitMix64：十来行的伪随机数，够造剧本用（同 `miyu-assemble` 的随机日志）。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// 一件随机的工具调用：名字和参数。
fn some_call(rng: &mut Rng) -> (&'static str, &'static str) {
    match rng.below(4) {
        0 => ("read", r#"{"file_path":"/home/alice/src/a.rs"}"#),
        1 => (
            "edit",
            r#"{"file_path":"a.txt","edits":[{"old_string":"a\n","new_string":"b\nc\n"}]}"#,
        ),
        2 => ("shell", r#"{"command":"ls","description":"List files"}"#),
        _ => ("write", r#"{"file_path":"b.txt","content":"x\ny\n"}"#),
    }
}

/// 一次随机的回复：可能先想，可能说一句，零到两件工具。
fn some_line(rng: &mut Rng) -> Line {
    let calls: Vec<(&str, &str)> = (0..rng.below(3)).map(|_| some_call(rng)).collect();
    let text = if rng.chance(40) { "Let me see." } else { "" };
    let mut line = Line::calls(text, &calls);
    if rng.chance(40) {
        line = line.thinking("thinking it over");
    }
    line
}

/// 一轮的剧本：一到三次请求，最后一次说完；偶尔说一半断了。多排几句、几件，打断留下的不会让后面的剧本不够用。
fn script(rng: &mut Rng, stage: &mut Stage) {
    let mut lines: Vec<Line> = (0..rng.below(3)).map(|_| some_line(rng)).collect();
    if rng.chance(10) {
        let mut broken = Line::breaks("I was saying", ErrorClass::Auth, "bad key");
        broken.calls = vec![("read".to_string(), r#"{"file_path":"a"}"#.to_string())];
        lines.push(broken);
    } else {
        lines.push(Line::says("Done.").thinking("almost"));
    }
    lines.extend((0..4).map(|_| Line::says("More.")));
    let plays: Vec<Play> = (0..8)
        .map(|_| match rng.below(10) {
            0 => Play::Fails("failed".to_string()),
            1 => Play::done("slow").held(),
            _ => Play::done("ok"),
        })
        .collect();
    stage.model(lines);
    stage.tools(plays);
}

/// 停住的调用：放行，或者先插一句再放行，或者打断（三种排着的办法随便挑）。
fn unblock(rng: &mut Rng, stage: &mut Stage, spoken: &mut u32) {
    for _ in 0..8 {
        let held = stage.held_tools();
        let Some(&call) = held.first() else {
            return;
        };
        if rng.chance(50) {
            *spoken += 1;
            stage.say(&format!("also {spoken}"));
        }
        match rng.below(3) {
            0 => {
                let queued = [Queued::Send, Queued::Return, Queued::Keep][rng.below(3) as usize];
                stage.interrupt(queued);
                return;
            }
            _ => stage.release_tool(call),
        }
    }
}

/// 一份随机的会话。
fn session(seed: u64) -> Stage {
    let mut rng = Rng(seed);
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    summarizes(&mut stage, "summary");
    let mut spoken = 0;
    for _ in 0..1 + rng.below(4) {
        script(&mut rng, &mut stage);
        spoken += 1;
        stage.say(&format!("turn {spoken}"));
        unblock(&mut rng, &mut stage, &mut spoken);
        if rng.chance(20)
            && let Some(&turn) = stage.turns().last()
        {
            stage.revert(turn);
            if rng.chance(50) {
                stage.unrevert();
            }
        }
        if rng.chance(15) {
            stage.request_compaction(None);
        }
    }
    stage
}

#[test]
fn random_sessions_stream_the_same_as_they_page() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..300 {
        let stage = session(seed);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| same(&stage)));
        if result.is_err() {
            for event in stage.log() {
                eprintln!("{}", event.to_line());
            }
            panic!("种子 {seed}：视图流和翻页对不上");
        }
        seen.extend(paths(&stage));
    }
    let missing: Vec<_> = PATHS.iter().filter(|p| !seen.contains(**p)).collect();
    assert!(missing.is_empty(), "三百份没走到：{missing:?}");
}

/// 三百份里每样至少走到一次。
const PATHS: [&str; 10] = [
    "tool error",
    "interrupted",
    "withdrawn",
    "compacted",
    "reverted",
    "unreverted",
    "remove",
    "queued",
    "moved",
    "thought",
];

/// 这一份走到了哪几样。
fn paths(stage: &Stage) -> Vec<&'static str> {
    let lines: Vec<String> = stage.log().iter().map(|e| e.to_line()).collect();
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    let (_, changes) = live(stage);
    let mut out = Vec::new();
    let mut mark = |hit: bool, name: &'static str| {
        if hit {
            out.push(name);
        }
    };
    mark(has(r#""status":"error""#), "tool error");
    mark(has(r#""reason":"interrupted""#), "interrupted");
    mark(has(r#""kind":"message.withdrawn""#), "withdrawn");
    mark(has(r#""kind":"context.compacted""#), "compacted");
    mark(has(r#""kind":"turn.reverted""#), "reverted");
    mark(has(r#""kind":"turn.unreverted""#), "unreverted");
    mark(
        changes
            .iter()
            .any(|c| matches!(c, miyu_view::Change::Remove { .. })),
        "remove",
    );
    mark(
        changes.iter().any(
            |c| matches!(c, miyu_view::Change::Add { entry, .. } if json(entry)["queued"] == true),
        ),
        "queued",
    );
    mark(
        changes
            .iter()
            .any(|c| matches!(c, miyu_view::Change::Update { after: Some(_), .. })),
        "moved",
    );
    mark(has(r#""type":"reasoning""#), "thought");
    out
}
