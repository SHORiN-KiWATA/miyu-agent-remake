//! 运行状态行的词（蓝图 `tui.md`「运行状态行和排队的消息」第 1 条）：中性的词，按这一轮跑了多久分档。
//! 事件（新的一步、想完、工具结果、开始写回答）来得很快，一串算一阵，安静下来这一阵才算完，完了换一个词；
//! 一个词至少停一会儿（随机），没停够等停够；没有事件时待久了也换（随机）。不连着重复刚才那个，
//! 新的一轮从头挑。词库在 `resources/pulse.json`。

use std::time::{Duration, Instant};

use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

/// 一档：这一轮跑了 `after` 秒以后用这些词。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    /// 这一轮跑了几秒以后用这一档。
    pub after: u64,
    /// 这一档的词。
    pub words: Vec<String>,
}

/// 词库（`resources/pulse.json`）。范围都是 `[最小, 最大]` 毫秒，每换上一个词在里面随机定一次。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Words {
    /// 一个词至少停多久：一阵事件完了，没停够也要等停够再换。
    pub dwell_ms: [u64; 2],
    /// 最后一件事之后安静多久，这一阵才算完。
    pub quiet_ms: u64,
    /// 没有事件时，一个词待多久换。
    pub idle_ms: [u64; 2],
    /// 各档，照 `after` 从小到大。
    pub tiers: Vec<Tier>,
}

impl Words {
    /// 最宽的词占几列：后面的用时照它留位置，换了长短不一的词不左右跳。
    pub fn widest(&self) -> usize {
        self.tiers
            .iter()
            .flat_map(|t| &t.words)
            .map(|w| w.width())
            .max()
            .unwrap_or(0)
    }

    /// 这一轮跑了 `ran` 该用哪一档的词：`after` 不超过它的最后一档；都超过的用第一档。
    fn pool(&self, ran: Duration) -> &[String] {
        let secs = ran.as_secs();
        self.tiers
            .iter()
            .rev()
            .find(|t| t.after <= secs)
            .or(self.tiers.first())
            .map_or(&[], |t| t.words.as_slice())
    }
}

/// 正在写的那个词，和换词要记的几样。
#[derive(Debug)]
pub struct Pulse {
    /// 这一轮开始的时刻：变了就是新的一轮，从头挑。
    turn: Option<Instant>,
    word: String,
    /// 这个词什么时候换上的：流光照它从头扫，停没停够也照它算。
    shown: Option<Instant>,
    /// 这个词至少停多久、没事件时待多久换：换上时随机定。
    dwell: Duration,
    idle: Duration,
    /// 最后看到的事件数，和它变的时刻：一阵事件还在来还是已经安静下来。
    beat: usize,
    last_event: Option<Instant>,
    /// 有一阵事件完了，还没换过词。
    due: bool,
    /// 挑词、定时长用的随机数。
    seed: u64,
}

impl Pulse {
    /// 空的，照 `seed` 挑词（给 0 也行）。
    pub fn new(seed: u64) -> Self {
        Self {
            turn: None,
            word: String::new(),
            shown: None,
            dwell: Duration::ZERO,
            idle: Duration::ZERO,
            beat: 0,
            last_event: None,
            due: false,
            seed: seed | 1,
        }
    }

    /// 这一刻写哪个词。`turn` 是这一轮开始的时刻，`beat` 是这一轮到现在出过几件事（只增不减）。
    pub fn word(&mut self, turn: Instant, beat: usize, now: Instant, words: &Words) -> &str {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.word.clear();
            self.beat = beat;
            self.last_event = None;
            self.due = false;
        }
        if beat != self.beat {
            self.beat = beat;
            self.last_event = Some(now);
        }
        // 最后一件事之后安静够了：这一阵完了，记一笔该换了。
        let quiet = Duration::from_millis(words.quiet_ms);
        if self
            .last_event
            .is_some_and(|at| now.saturating_duration_since(at) >= quiet)
        {
            self.last_event = None;
            self.due = true;
        }
        let pool = words.pool(now.saturating_duration_since(turn));
        let held = self
            .shown
            .map_or(Duration::MAX, |s| now.saturating_duration_since(s));
        let settled = held >= self.dwell;
        let change = self.word.is_empty()
            || (settled && (self.due || !pool.contains(&self.word)))
            || held >= self.idle;
        if change {
            self.pick(pool);
            self.shown = Some(now);
            self.due = false;
            self.dwell = self.between(words.dwell_ms);
            self.idle = self.between(words.idle_ms);
        }
        &self.word
    }

    /// 这个词什么时候换上的；还没写过的是 `None`。
    pub fn shown(&self) -> Option<Instant> {
        self.shown
    }

    /// 在 `pool` 里随机挑一个，不挑刚才那个（只有一个词的除外）。
    fn pick(&mut self, pool: &[String]) {
        let others: Vec<&String> = pool.iter().filter(|w| **w != self.word).collect();
        let choices = if others.is_empty() {
            pool.iter().collect()
        } else {
            others
        };
        if choices.is_empty() {
            return;
        }
        let at = usize::try_from(self.next() % choices.len() as u64).unwrap_or(0);
        self.word = choices[at].clone();
    }

    /// `[最小, 最大]` 毫秒里随机一个时长。
    fn between(&mut self, [low, high]: [u64; 2]) -> Duration {
        let span = high.saturating_sub(low) + 1;
        Duration::from_millis(low.min(high) + self.next() % span)
    }

    /// xorshift：够用，不为这个引随机数库。
    fn next(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
}

#[cfg(test)]
mod tests;
