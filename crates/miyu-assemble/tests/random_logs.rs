//! 随机日志（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，施工 1-14、2-9 下）：五百份随机的
//! 剧本交给执行器替身，跑出真会话，每一次请求都查五条性质。同样的种子跑两遍，日志和请求要一字
//! 不差。还查剧本真走到了：五百份里，每种走法至少一次。CI 另有一项长跑，接着往后跑两万份。
//!
//! 随机数是自己写的 SplitMix64，种子固定，每次跑都是同样的五百份。红了会打印种子和那份日志。

mod support;

use std::collections::BTreeSet;

use miyu_kernel::event::{Body, ErrorClass, Event, ToolStatus};
use miyu_kernel::origin::By;
use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{Line, Play, Stage};
use support::{check, lines, sent, stage};

/// SplitMix64：十来行的伪随机数，够造剧本用。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 到 `n - 1` 里的一个。
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// 百分之 `percent` 的机会。
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// 造剧本时记着的：说到第几句了、只读开着没有。
struct Writer {
    rng: Rng,
    spoken: u32,
    read_only: bool,
}

/// 一个回合里开的口子：替身跑到这里停住，人在这时插手。
enum Window {
    /// 不开口子，一口气跑完。
    None,
    /// 第几步的请求说到一半停住，人插一句（或者不插）、切只读（或者不切），再打断。
    Model(u64),
    /// 第几步的两件读的工具都停住，人插一句、切只读，再照随机的先后放行。
    Tools(u64),
}

impl Writer {
    /// 人说一句新的。
    fn say(&mut self, s: &mut Stage) {
        self.spoken += 1;
        s.say(&format!("第 {} 句话", self.spoken));
    }

    /// 开关只读。
    fn toggle(&mut self, s: &mut Stage) {
        self.read_only = !self.read_only;
        s.set_permission(None, Some(self.read_only));
    }

    /// 一件工具怎么回：多半成，偶尔出错。
    fn play(&mut self) -> Play {
        match self.rng.chance(25) {
            true => Play::Fails("出错了".to_string()),
            false => Play::done("结果"),
        }
    }

    /// 两轮之间：过一会儿（偶尔过整点）、开关只读、撤销上一轮（撤了又恢复的也有）、压缩。
    fn between(&mut self, s: &mut Stage) {
        let minutes = match self.rng.chance(25) {
            true => 60,
            false => self.rng.below(15) as i64,
        };
        s.advance(minutes);
        if self.rng.chance(20) {
            self.toggle(s);
        }
        if self.rng.chance(15)
            && let Some(&last) = s.turns().last()
        {
            s.revert(last);
            if self.rng.chance(30) {
                s.unrevert();
            }
        }
        if self.rng.chance(10) {
            s.compact("Earlier turns were summarized.");
        }
    }

    /// 一个回合，一到三步。第一次请求就出错的，只有一步。
    ///
    /// 替身一口气跑到底，所以这一轮模型、工具要说的，事先都排好；要人插手的地方开一个口子（停住）。
    /// 被打断的那一步以后的台词不排，那一步的调用也不排结果：它们派不出去，排了会漏到下一轮。
    fn turn(&mut self, s: &mut Stage) {
        if self.rng.chance(8) {
            s.model([Line::fails(ErrorClass::Retryable, "503")]);
            self.say(s);
            return;
        }
        let steps = 1 + self.rng.below(3);
        let window = match self.rng.below(20) {
            0..=2 => Window::Model(self.rng.below(steps)),
            3..=7 => Window::Tools(self.rng.below(steps)),
            _ => Window::None,
        };
        // 只有一口气跑完的回合调写的工具：只读在这一轮里不变，写的派不派得出去，事先就知道。
        let writes = matches!(window, Window::None);
        let mut lines = Vec::new();
        let mut plays = Vec::new();
        let mut reached = false;
        for step in 0..steps {
            let last = step + 1 == steps;
            let held_tools = matches!(window, Window::Tools(k) if k == step);
            let held_model = matches!(window, Window::Model(k) if k == step);
            let count = match (held_tools, last && steps < 3) {
                (true, _) => 2,
                // 不到三步就说完的，最后一步不调工具；三步的，最后一步调了就走到步数上限。
                (false, true) => 0,
                (false, false) => self.rng.below(3),
            };
            let mut calls = Vec::new();
            for _ in 0..count {
                let write = writes && count < 2 && self.rng.chance(25);
                calls.push(match write {
                    true => ("write", r#"{"path":"b"}"#),
                    false => ("read", r#"{"path":"a"}"#),
                });
                if !held_model && (!write || !self.read_only) {
                    let play = self.play();
                    plays.push(match held_tools {
                        true => play.held(),
                        false => play,
                    });
                }
            }
            let line = Line::calls("好。", &calls);
            reached |= held_tools || held_model;
            if held_model {
                lines.push(line.held());
                break;
            }
            lines.push(line);
            if count == 0 {
                break;
            }
            // 停住的两件工具在不到三步的最后一步：放行以后还要请求一次，补一句收尾。
            if held_tools && last && steps < 3 {
                lines.push(Line::says("好。"));
            }
        }
        s.model(lines);
        s.tools(plays);
        self.say(s);
        match window {
            _ if !reached => {}
            Window::None => {}
            Window::Model(_) => self.interrupt(s),
            // 停住的那一步是三步的最后一步：放行以后走到步数上限，这时插的那句还排着，接着开一轮。
            Window::Tools(k) => self.release(s, k + 1 == steps && steps == 3),
        }
    }

    /// 说到一半：插一句（或者不插）、切只读（或者不切），再打断，一半接着发、一半退回。接着发的，
    /// 插的那句开下一轮，替它排一句回复。
    fn interrupt(&mut self, s: &mut Stage) {
        let interjected = self.rng.chance(50);
        if interjected {
            self.say(s);
        }
        if self.rng.chance(20) {
            self.toggle(s);
        }
        let queued = match self.rng.chance(50) {
            true => Queued::Send,
            false => Queued::Return,
        };
        if interjected && queued == Queued::Send {
            s.model([Line::says("好的。")]);
        }
        s.interrupt(queued);
    }

    /// 两件工具都停着：插一句、切只读，各看运气，再照随机的先后放行。放行以后走到步数上限的
    /// （`limit`），插的那句接着开下一轮，替它排一句回复。
    fn release(&mut self, s: &mut Stage, limit: bool) {
        if self.rng.chance(40) {
            if limit {
                s.model([Line::says("好的。")]);
            }
            self.say(s);
        }
        if self.rng.chance(15) {
            self.toggle(s);
        }
        let running = s.ran().len();
        let (first, second) = (s.ran()[running - 2].0, s.ran()[running - 1].0);
        let order = match self.rng.chance(50) {
            true => [second, first],
            false => [first, second],
        };
        for call in order {
            s.release_tool(call);
        }
    }
}

/// 照种子造一份会话：三到八个回合。
fn random_session(seed: u64) -> Stage {
    let mut writer = Writer {
        rng: Rng(seed),
        spoken: 0,
        read_only: false,
    };
    let mut s = stage();
    for _ in 0..3 + writer.rng.below(6) {
        writer.between(&mut s);
        writer.turn(&mut s);
    }
    s
}

/// 这份日志走到了哪些走法。
fn paths(log: &[Event]) -> BTreeSet<&'static str> {
    let mut paths = BTreeSet::new();
    for (k, event) in log.iter().enumerate() {
        let path = match &event.body {
            Body::TurnEnded(ended) => match ended.reason.as_str() {
                "error" => Some("第一次请求就出错"),
                "step_limit" => Some("走到步数上限"),
                "interrupted" => Some("说到一半被打断"),
                _ => None,
            },
            Body::MessageWithdrawn(_) => Some("打断以后退回"),
            Body::TurnReverted(_) => Some("撤销"),
            Body::TurnUnreverted(_) => Some("恢复"),
            Body::ContextCompacted(_) => Some("压缩"),
            Body::PolicyChanged(_) if event.turn.is_some() => Some("回合中途切只读"),
            Body::MessageUser(_) if event.turn.is_some() => Some("回合中途说一句"),
            Body::ToolResult(result) if result.status == ToolStatus::Error => Some("工具出错"),
            Body::ToolResult(result)
                if result.status == ToolStatus::Denied && event.by == By::Kernel =>
            {
                Some("只读拦下写的")
            }
            Body::ToolResult(result) if result.call_id.index() == 2 => {
                // 第 2 个调用的结果先回来：乱序。
                let first = log[..k].iter().all(|earlier| {
                    !matches!(&earlier.body, Body::ToolResult(r)
                        if r.call_id.message() == result.call_id.message())
                });
                first.then_some("结果乱序回来")
            }
            Body::TurnStarted(started) => log
                .iter()
                .find(|trigger| trigger.seq == started.trigger)
                .filter(|trigger| {
                    matches!(trigger.body, Body::MessageUser(_)) && trigger.turn.is_some()
                })
                .map(|_| "排着的那句接着开一轮"),
            _ => None,
        };
        paths.extend(path);
    }
    paths
}

/// 跑一段种子，每一份查五条性质、查同样的种子跑两遍一字不差。返回走到过的走法。
fn run(seeds: std::ops::Range<u64>) -> BTreeSet<&'static str> {
    let mut seen = BTreeSet::new();
    for seed in seeds {
        let session = random_session(seed);
        if let Err(why) = check(&sent(&session)) {
            panic!("种子 {seed}：{why}\n{}", lines(&session).join("\n"));
        }
        let again = random_session(seed);
        assert_eq!(
            lines(&session),
            lines(&again),
            "种子 {seed}：同样的种子造出的日志不一样"
        );
        let bytes = |s: &Stage| -> Vec<Vec<u8>> {
            s.requests()
                .iter()
                .map(|(_, request)| request.canonical_bytes())
                .collect()
        };
        assert_eq!(
            bytes(&session),
            bytes(&again),
            "种子 {seed}：同样的日志出了不同的字节"
        );
        seen.extend(paths(session.log()));
        let sent = sent(&session);
        if sent.iter().any(|sent| sent.trigger.is_some()) {
            seen.insert("查了回合第一次请求的最后一块");
        }
        if sent.iter().skip(1).any(|sent| !sent.rewritten) {
            seen.insert("查了前缀延伸");
        }
    }
    seen
}

/// 五百份里每种都要走到的走法。
const EXPECTED_PATHS: &[&str] = &[
    "第一次请求就出错",
    "走到步数上限",
    "说到一半被打断",
    "打断以后退回",
    "排着的那句接着开一轮",
    "撤销",
    "恢复",
    "压缩",
    "回合中途切只读",
    "回合中途说一句",
    "工具出错",
    "只读拦下写的",
    "结果乱序回来",
    "查了回合第一次请求的最后一块",
    "查了前缀延伸",
];

#[test]
fn five_hundred_random_sessions_keep_the_properties() {
    let seen = run(0..500);
    for path in EXPECTED_PATHS {
        assert!(seen.contains(path), "五百份里一次都没走到「{path}」");
    }
}

/// 长跑：接着平时的往后跑两万份（`docs/designs/02-内核.md` 第九节「不变量怎么查」）。平时的
/// `cargo test` 跳过它，CI 的长跑那一项用 `--ignored`、release 模式跑。
#[test]
#[ignore = "长跑，CI 的长跑那一项用 --ignored 跑（施工 2-10）"]
fn random_sessions_keep_the_properties_for_longer() {
    run(500..20_500);
}
