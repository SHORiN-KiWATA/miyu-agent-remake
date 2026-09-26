//! 随机日志（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，施工 1-14）：随机生成五百份
//! 合规的日志，每一次请求都查五条性质。同样的种子造两遍，日志和请求要一字不差。
//!
//! 随机数是自己写的 SplitMix64，种子固定，每次跑都是同样的五百份。红了会打印种子和那份日志。

mod support;

use support::{Session, check};

/// SplitMix64：十来行的伪随机数，够造日志用。
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

/// 照种子造一份会话：三到八个回合，每个回合一到三步。
///
/// 走法有：两轮之间过整点、开关只读、撤销上一轮、压缩；先来一句再开回合；请求在路上时来了
/// 一句；回合中途开关只读、压缩；第一次请求就出错；回到一半被打断；调零到两次工具，结果
/// 乱序回来、成败不定；结果回来以后又来一句；走到步数上限。没被请求看到的那句话，一半的
/// 机会成为下一回合的触发。
fn random_session(seed: u64) -> Session {
    let mut rng = Rng(seed);
    let mut s = Session::new();
    // 最近一次压缩以后开过的回合，撤销只能撤它们。
    let mut recent: Vec<u64> = Vec::new();
    // 还没有请求看到过的那句话。
    let mut unheard: Option<u64> = None;
    let mut spoken = 0;
    let mut say = |s: &mut Session| {
        spoken += 1;
        s.say(&format!("第 {spoken} 句话"))
    };
    for _ in 0..3 + rng.below(6) {
        s.advance(if rng.chance(25) {
            60
        } else {
            rng.below(15) as i64
        });
        if rng.chance(20) {
            let on = !s.is_read_only();
            s.read_only(on);
        }
        if rng.chance(15)
            && let Some(turn) = recent.pop()
        {
            s.undo(turn);
            unheard = None;
        }
        if rng.chance(10) {
            s.compact("Earlier turns were summarized.");
            recent.clear();
            unheard = None;
        }
        let trigger = match unheard.take() {
            Some(seq) if rng.chance(50) => seq,
            _ => {
                if rng.chance(20) {
                    say(&mut s);
                }
                say(&mut s)
            }
        };
        let turn = s.start(trigger);
        recent.push(turn);
        let steps = 1 + rng.below(3);
        let mut ended = false;
        for step in 0..steps {
            if step > 0 && rng.chance(15) {
                let on = !s.is_read_only();
                s.read_only(on);
            }
            if step > 0 && rng.chance(5) {
                s.compact("Earlier turns were summarized.");
                recent.clear();
            }
            let seen = s.request();
            unheard = None;
            if step == 0 && rng.chance(8) {
                s.end("error");
                ended = true;
                break;
            }
            if rng.chance(15) {
                unheard = Some(say(&mut s));
            }
            let calls: Vec<(&str, &str)> = (0..rng.below(3))
                .map(|_| ("read", r#"{"path":"src"}"#))
                .collect();
            if rng.chance(10) {
                for id in s.cut_off(seen, "我先……", &calls) {
                    s.result(
                        &id,
                        "cancelled",
                        "Cancelled: the user interrupted this turn.",
                    );
                }
                s.end("interrupted");
                ended = true;
                break;
            }
            let ids = s.reply(seen, "好。", &calls);
            let mut order: Vec<usize> = (0..ids.len()).collect();
            if order.len() == 2 && rng.chance(50) {
                order.swap(0, 1);
            }
            for index in order {
                let status = ["ok", "error", "denied"][rng.below(3) as usize];
                s.result(&ids[index], status, "结果");
            }
            if rng.chance(15) {
                unheard = Some(say(&mut s));
            }
            if calls.is_empty() {
                s.end("completed");
                ended = true;
                break;
            }
        }
        if !ended {
            s.end("step_limit");
        }
    }
    s
}

#[test]
fn five_hundred_random_sessions_keep_the_properties() {
    for seed in 0..500 {
        let session = random_session(seed);
        if let Err(why) = check(session.sent()) {
            panic!("种子 {seed}：{why}\n{}", session.lines().join("\n"));
        }
        let again = random_session(seed);
        assert_eq!(
            session.lines(),
            again.lines(),
            "种子 {seed}：同样的种子造出的日志不一样"
        );
        let bytes = |s: &Session| -> Vec<Vec<u8>> {
            s.sent()
                .iter()
                .map(|sent| sent.request.canonical_bytes())
                .collect()
        };
        assert_eq!(
            bytes(&session),
            bytes(&again),
            "种子 {seed}：同样的日志出了不同的字节"
        );
    }
}
