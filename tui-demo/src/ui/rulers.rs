//! 量尺（`cargo test ruler -- --ignored --nocapture`）：思考越长、会话越长，排一帧要多久。只打数字，不断言耗时。

use std::time::{Duration, Instant};

use serde_json::json;

use crate::core::ToolStatus;
use crate::transcript::{Segment, Step, StepKind, ToolState};
use crate::ui::test_support::Fixture;
use crate::ui::timeline::rows;

fn step(kind: StepKind, t0: Instant, start: u64, took: u64) -> Step {
    let mut s = Step::new(kind);
    s.started = t0 + Duration::from_secs(start);
    s.took = Some(Duration::from_secs(took));
    s
}

fn command(t0: Instant, start: u64, status: ToolStatus) -> Step {
    step(
        StepKind::Tool {
            name: "shell".into(),
            args: String::new(),
            parsed: json!({"command": "ls", "description": "列目录"}),
            state: ToolState::Done(status),
            output: "a.txt".into(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

fn segment(steps: Vec<Step>, open: Option<bool>) -> Segment {
    let mut s = Segment::new();
    s.steps = steps;
    s.finished = true;
    s.open = open;
    s
}

/// 思考越长，排一帧时间线要多久。
#[test]
#[ignore]
fn ruler_thought() {
    let f = Fixture::new();
    for chars in [2_000usize, 20_000, 100_000, 300_000] {
        let line = "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n";
        let text: String = line.repeat(chars / line.chars().count() + 1);
        let mut seg = segment(vec![Step::new(StepKind::Thought { text })], None);
        seg.finished = false;
        let t = Instant::now();
        let n = 20;
        for _ in 0..n {
            std::hint::black_box(rows(0, &seg, &f.ctx()));
        }
        println!(
            "思考 {chars} 字：一帧 {:.2}ms",
            t.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
        // 最坏的：一整段不换行。
        let flat: String = "想".repeat(chars);
        let mut seg = segment(vec![Step::new(StepKind::Thought { text: flat })], None);
        seg.finished = false;
        let t = Instant::now();
        for _ in 0..n {
            std::hint::black_box(rows(0, &seg, &f.ctx()));
        }
        println!(
            "  一整段不换行：一帧 {:.2}ms",
            t.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
    }
}

/// 长会话里正文一帧排成行要多久：整份重排和按条缓存对比。
#[test]
#[ignore]
fn ruler_body() {
    use crate::transcript::{Kind, Transcript};
    let f = Fixture::new();
    let t0 = Instant::now();
    let think =
        "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n".repeat(120);
    let reply = "## 结论\n\n- 第一点：**要紧的**先说\n- 第二点：`code` 放这里\n\n一段普通的回答文字，稍微长一点，好折几行。\n".repeat(20);
    // 只量一档：`RULER_TURNS=150`（内存要一档一个进程量，不然上一档释放的被下一档复用）。
    let only = std::env::var("RULER_TURNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    for turns in [10usize, 40, 100, 150]
        .into_iter()
        .filter(|t| only.is_none_or(|o| o == *t))
    {
        let before = anon_kb();
        let mut t = Transcript::default();
        for _ in 0..turns {
            t.note(Kind::User, "帮我看看这个目录".into());
            t.note(Kind::Steps, String::new());
            let mut seg = segment(
                vec![
                    step(
                        StepKind::Thought {
                            text: think.clone(),
                        },
                        t0,
                        0,
                        3,
                    ),
                    command(t0, 3, ToolStatus::Ok),
                    command(t0, 4, ToolStatus::Ok),
                ],
                None,
            );
            seg.finished = true;
            t.entries.last_mut().unwrap().segment = Some(seg);
            t.note(Kind::Reply, reply.clone());
        }
        let ctx = f.ctx();
        let text_done = anon_kb();
        let fresh = Instant::now();
        let n = crate::ui::test_support::fresh_rows(&t.entries, &ctx).len();
        let whole = fresh.elapsed();
        let md_done = anon_kb();
        let cache = std::cell::RefCell::new(crate::ui::row_cache::RowCache::default());
        let cold = Instant::now();
        crate::ui::row_cache::build(&t.entries, &ctx, &cache);
        let cold = cold.elapsed();
        let cache_done = anon_kb();
        if let (Some(b), Some(x), Some(m), Some(c)) = (before, text_done, md_done, cache_done) {
            println!(
                "  拆开：正文原文 {} KB，排一遍以后（Markdown 等缓存）{} KB，按条行缓存 {} KB",
                x.saturating_sub(b),
                m.saturating_sub(x),
                c.saturating_sub(m)
            );
        }
        let warm = Instant::now();
        for _ in 0..10 {
            std::hint::black_box(crate::ui::row_cache::build(&t.entries, &ctx, &cache));
        }
        let grew = anon_kb().zip(before).map(|(a, b)| a.saturating_sub(b));
        println!(
            "{turns} 轮（{n} 行）：整份重排 {:.2}ms；缓存头一帧 {:.2}ms，之后每帧 {:.3}ms；私有匿名内存多了 {} KB",
            whole.as_secs_f64() * 1000.0,
            cold.as_secs_f64() * 1000.0,
            warm.elapsed().as_secs_f64() * 100.0,
            grew.map_or_else(|| "?".to_string(), |k| k.to_string())
        );
        std::hint::black_box(&t);
    }
}

/// 这个进程的私有匿名内存（KB），读 `/proc/self/smaps_rollup`；别的系统没有就交回 `None`。
fn anon_kb() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/smaps_rollup").ok()?;
    text.lines()
        .find(|l| l.starts_with("Anonymous:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// 一行缓存里各部分占多少字节：带样式的字、复制用的纯文字、链接、结构本身（第 2 条内存踩线，量了再定怎么省）。
#[test]
#[ignore]
fn ruler_row_bytes() {
    use crate::transcript::{Kind, Transcript};
    let f = Fixture::new();
    let t0 = Instant::now();
    let think =
        "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n".repeat(120);
    let reply = "## 结论\n\n- 第一点：**要紧的**先说\n- 第二点：`code` 放这里\n\n一段普通的回答文字，稍微长一点，好折几行。\n".repeat(20);
    let mut t = Transcript::default();
    for _ in 0..10 {
        t.note(Kind::User, "帮我看看这个目录".into());
        t.note(Kind::Steps, String::new());
        let mut seg = segment(
            vec![
                step(
                    StepKind::Thought {
                        text: think.clone(),
                    },
                    t0,
                    0,
                    3,
                ),
                command(t0, 3, ToolStatus::Ok),
            ],
            None,
        );
        seg.finished = true;
        t.entries.last_mut().unwrap().segment = Some(seg);
        t.note(Kind::Reply, reply.clone());
    }
    let rows = crate::ui::test_support::fresh_rows(&t.entries, &f.ctx());
    let n = rows.len();
    let spans: usize = rows.iter().map(|r| r.line.spans.len()).sum();
    let span_structs = spans * std::mem::size_of::<ratatui::text::Span<'static>>();
    let span_text: usize = rows
        .iter()
        .flat_map(|r| r.line.spans.iter())
        .map(|s| match &s.content {
            std::borrow::Cow::Owned(o) => o.capacity(),
            std::borrow::Cow::Borrowed(_) => 0,
        })
        .sum();
    let plain: usize = rows.iter().map(|r| r.plain.capacity()).sum();
    let links: usize = rows
        .iter()
        .map(|r| {
            r.links.capacity() * std::mem::size_of::<(u16, u16, String)>()
                + r.links.iter().map(|l| l.2.capacity()).sum::<usize>()
        })
        .sum();
    let row_struct = n * std::mem::size_of::<crate::ui::rows::Row>();
    let total = span_structs + span_text + plain + links + row_struct;
    println!(
        "{n} 行，每行平均 {:.1} 段字；合计 {} KB，每行 {} 字节",
        spans as f64 / n as f64,
        total / 1024,
        total / n
    );
    for (name, b) in [
        ("Row 结构本身", row_struct),
        ("每段字的结构（Span，含样式）", span_structs),
        ("每段字的文字", span_text),
        ("复制用的纯文字", plain),
        ("链接", links),
    ] {
        println!(
            "  {name}: {} KB（{:.0}%），每行 {} 字节",
            b / 1024,
            b as f64 * 100.0 / total as f64,
            b / n
        );
    }
    println!(
        "  sizeof: Row {}，Span {}，Style {}",
        std::mem::size_of::<crate::ui::rows::Row>(),
        std::mem::size_of::<ratatui::text::Span<'static>>(),
        std::mem::size_of::<ratatui::style::Style>()
    );
}

/// 长会话照 App 的做法只建按条行缓存，内存都花在哪（第 2 条内存踩线）：正文原文、Markdown 缓存、行缓存各存了多少字节，
/// 进程的私有匿名内存一共多了多少。`RULER_TURNS` 定几轮（默认 150）。
#[test]
#[ignore]
fn ruler_memory() {
    use crate::transcript::{Kind, Transcript};
    use ratatui::text::Span;
    fn spans(v: &Vec<Span<'static>>) -> usize {
        v.capacity() * std::mem::size_of::<Span<'static>>()
            + v.iter()
                .map(|s| match &s.content {
                    std::borrow::Cow::Owned(o) => o.capacity(),
                    std::borrow::Cow::Borrowed(_) => 0,
                })
                .sum::<usize>()
    }
    let turns = std::env::var("RULER_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150usize);
    let f = Fixture::new();
    let t0 = Instant::now();
    let think =
        "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n".repeat(120);
    let reply = "## 结论\n\n- 第一点：**要紧的**先说\n- 第二点：`code` 放这里\n\n一段普通的回答文字，稍微长一点，好折几行。\n".repeat(20);
    let start = anon_kb();
    let mut t = Transcript::default();
    for _ in 0..turns {
        t.note(Kind::User, "帮我看看这个目录".into());
        t.note(Kind::Steps, String::new());
        let mut seg = segment(
            vec![
                step(
                    StepKind::Thought {
                        text: think.clone(),
                    },
                    t0,
                    0,
                    3,
                ),
                command(t0, 3, ToolStatus::Ok),
                command(t0, 4, ToolStatus::Ok),
            ],
            None,
        );
        seg.finished = true;
        t.entries.last_mut().unwrap().segment = Some(seg);
        t.note(Kind::Reply, reply.clone());
    }
    let text_kb = anon_kb();
    let ctx = f.ctx();
    let cache = std::cell::RefCell::new(crate::ui::row_cache::RowCache::default());
    let rows = crate::ui::row_cache::build(&t.entries, &ctx, &cache);
    let built_kb = anon_kb();
    let md = ctx.md.borrow().len();
    let row: usize = rows
        .iter()
        .map(|r| {
            std::mem::size_of::<crate::ui::rows::Row>() + spans(&r.line.spans) + r.plain.capacity()
        })
        .sum();
    let text: usize = t.entries.iter().map(|e| e.text.capacity()).sum::<usize>()
        + t.entries
            .iter()
            .filter_map(|e| e.segment.as_ref())
            .flat_map(|s| s.steps.iter())
            .map(|s| match &s.kind {
                StepKind::Thought { text } => text.capacity(),
                _ => 0,
            })
            .sum::<usize>();
    println!(
        "{turns} 轮、{} 行：存着的——正文原文 {} KB，Markdown 缓存留着 {} 篇，行缓存 {} KB；进程私有匿名多了——建正文 {} KB，排版 {} KB",
        rows.len(),
        text / 1024,
        md,
        row / 1024,
        text_kb.zip(start).map_or(0, |(a, b)| a.saturating_sub(b)),
        built_kb
            .zip(text_kb)
            .map_or(0, |(a, b)| a.saturating_sub(b)),
    );
    std::hint::black_box((&t, &rows));
}

/// 流式回答时每帧整篇重排：照 `RULER_REPLY` 那份回答原文，每 40 个字截一段排一次，列出最慢的几截和截在哪
/// （第 3 条：长回答里偶尔一帧二三十毫秒）。
#[test]
#[ignore]
fn ruler_stream_reply() {
    use crate::transcript::{Kind, Transcript};
    let Some(text) = std::env::var("RULER_REPLY")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
    else {
        println!("给 RULER_REPLY=回答原文的文件");
        return;
    };
    let mut f = Fixture::new();
    let mut times = Vec::new();
    let cuts: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .step_by(40)
        .chain(std::iter::once(text.len()))
        .collect();
    for &cut in &cuts {
        let mut t = Transcript::default();
        t.note(Kind::Reply, text[..cut].to_string());
        let mut ctx = f.ctx();
        ctx.width = 140;
        let start = Instant::now();
        let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &ctx);
        let took = start.elapsed();
        std::hint::black_box(rows);
        times.push((took, cut));
        drop(ctx);
        f = Fixture::new();
    }
    let mut sorted = times.clone();
    sorted.sort_by_key(|t| std::cmp::Reverse(t.0));
    let median = {
        let mut d: Vec<_> = times.iter().map(|t| t.0).collect();
        d.sort();
        d[d.len() / 2]
    };
    println!(
        "{} 截，中位 {:.2}ms",
        times.len(),
        median.as_secs_f64() * 1000.0
    );
    for (took, cut) in sorted.iter().take(8) {
        let tail: String = text[..*cut]
            .chars()
            .rev()
            .take(50)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        println!(
            "  {:.2}ms 截到第 {cut} 字节：…{}",
            took.as_secs_f64() * 1000.0,
            tail.replace('\n', "⏎")
        );
    }
}

/// 一轮结束时收尾行要本地时间：进程里第一次取本地时间要读时区，花多久（第 3 条：第一轮结束那一帧慢）。
#[test]
#[ignore]
fn ruler_first_local_time() {
    for n in 1..=3 {
        let start = Instant::now();
        let time = jiff::Zoned::now().strftime("%H:%M").to_string();
        println!(
            "第 {n} 次取本地时间 {:.2}ms（{time}）",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/// 思考写完了、这一段还没结束（她正要开口、正要调工具）那一两帧：进行中的一段排一次多久（第 3 条）。
#[test]
#[ignore]
fn ruler_finished_thought_in_a_live_segment() {
    let f = Fixture::new();
    let t0 = Instant::now();
    for chars in [2_000usize, 16_000, 50_000] {
        let line = "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n";
        let text: String = line.repeat(chars / line.chars().count() + 1);
        let mut seg = segment(vec![step(StepKind::Thought { text }, t0, 0, 3)], None);
        seg.finished = false;
        let start = Instant::now();
        let n = rows(0, &seg, &f.ctx()).len();
        println!(
            "思考 {chars} 字写完了、这一段还在进行：一帧 {:.2}ms，留下 {n} 行",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
