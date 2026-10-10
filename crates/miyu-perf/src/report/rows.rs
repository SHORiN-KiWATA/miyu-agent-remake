//! 给人看的那一份：条件、对照预算的表、没有预算的、会话长大一路上、每个进程的内存。

use std::collections::BTreeMap;

use super::Results;
use crate::budget::{Budget, verdict};
use crate::measure::large::Large;
use crate::memory::Process;
use crate::stats::{percentile, spread};

const HOT: &str = "热启动到能提交第一轮";
const PROJECTION: &str = "一次请求的投影（增量，一万条事件的会话）";
const FROM_SCRATCH: &str = "同一个会话从头投影一次";
const APPEND: &str = "追加一步的事件并同步";
const IDLE: &str = "核心空闲、没有会话载入";
const PER_SESSION: &str = "核心里每多一个活动会话";

/// 要和 23 第二节比的几项：名字照那张表的第一格。
pub const ITEMS: &[&str] = &[HOT, PROJECTION, FROM_SCRATCH, APPEND, IDLE, PER_SESSION];

/// 整份 `.md`。
///
/// # Errors
///
/// 预算表里缺了要比的一项。
pub fn markdown(
    results: &Results,
    budgets: &BTreeMap<String, Budget>,
    name: &str,
) -> Result<String, String> {
    let budget = |item: &str| {
        budgets
            .get(item)
            .copied()
            .ok_or_else(|| format!("23 第二节没有「{item}」"))
    };
    let large = &results.large;
    let tail: Vec<f64> = large.turns[large.turns.len().saturating_sub(large.tail)..]
        .iter()
        .map(|said| said.request)
        .collect();

    let mut text = format!("## 量尺 {name}\n\n");
    conditions(&mut text, results);
    text.push_str("### 对照预算\n\n");
    text.push_str(
        "只写了一个数的预算照 p50 比，写了分位的照那个分位比。内存照核心进程的 PSS 比。\n\n",
    );
    text.push_str("| 项目 | 量到 | 预算 | 判 |\n|---|---|---|---|\n");
    let timed: [(String, &[f64], Budget); 5] = [
        (
            format!("{HOT}：连上、握手、订阅一个没载入的小会话"),
            &results.hot.load,
            budget(HOT)?,
        ),
        (
            format!("{HOT}：重启核心后打开大会话（连上、握手、订阅）"),
            &large.reload,
            budget(HOT)?,
        ),
        (
            "一次请求的投影：大会话上说一句到模型收到请求（上界：含落盘、组装、编码、连接）"
                .to_string(),
            &tail,
            budget(PROJECTION)?,
        ),
        (
            "从头投影：重启、打开大会话后头一次说到模型收到请求".to_string(),
            &large.first_request,
            budget(FROM_SCRATCH)?,
        ),
        (
            "追加一条事件并同步".to_string(),
            &results.appends,
            budget(APPEND)?,
        ),
    ];
    for (label, values, budget) in &timed {
        let measured = percentile(values, f64::from(budget.quantile));
        let cell = match (
            percentile(values, 50.0),
            percentile(values, 95.0),
            percentile(values, 99.0),
        ) {
            (Some(p50), Some(p95), Some(p99)) => format!(
                "p50 {p50:.1} / p95 {p95:.1} / p99 {p99:.1} ms（{} 次）",
                values.len()
            ),
            _ => "—".to_string(),
        };
        line(
            &mut text,
            &format!(
                "| {label} | {cell} | {} | {} |",
                budget.show(),
                verdict(measured, budget)
            ),
        );
    }

    let idle = budget(IDLE)?;
    line(
        &mut text,
        &format!(
            "| {IDLE} | {} | {} | {} |",
            memory_cell(results.hot.idle.first()),
            idle.show(),
            verdict(core_pss(&results.hot.idle), &idle)
        ),
    );
    let per = budget(PER_SESSION)?;
    let per_session = core_pss(&results.hot.active)
        .zip(core_pss(&results.hot.idle))
        .map(|(active, idle)| (active - idle) / results.hot.sessions.max(1) as f64);
    line(
        &mut text,
        &format!(
            "| {PER_SESSION}（{} 个小会话订阅着、各说过两句，核心 PSS 涨的平均） | {} | {} | {} |",
            results.hot.sessions,
            per_session.map_or("不量".to_string(), |mb| format!("{mb:.2} MB")),
            per.show(),
            verdict(per_session, &per)
        ),
    );
    unbudgeted(&mut text, results);
    growth(&mut text, large);
    memory(&mut text, results);
    Ok(text)
}

/// 条件。
fn conditions(text: &mut String, results: &Results) {
    let machine = &results.machine;
    let args = &results.args;
    let or_dash = |value: Option<String>| value.unwrap_or_else(|| "—".to_string());
    let memory = machine
        .memory
        .map(|bytes| format!("{:.1} GB", bytes as f64 / GB));
    line(
        text,
        &format!(
            "- 机器：{}，{} 个逻辑核，内存 {}",
            or_dash(machine.cpu.clone()),
            or_dash(machine.cores.map(|n| n.to_string())),
            or_dash(memory)
        ),
    );
    line(
        text,
        &format!(
            "- 系统：{} {}，内核 {}",
            machine.os,
            machine.arch,
            or_dash(machine.kernel.clone())
        ),
    );
    line(
        text,
        &format!(
            "- 构建：`cargo build --release -p miyu -p miyu-sandbox`，{}",
            or_dash(machine.rustc.clone())
        ),
    );
    line(
        text,
        &format!("- 提交：{}", or_dash(machine.commit.clone())),
    );
    line(
        text,
        &format!(
            "- 次数：冷启动先空跑一次、量 {} 次；小会话 {} 个；大会话说到 {} 条事件、再说 {} 轮；重启打开大会话 {} 次；追加 {} 条；假模型每次回 {} 字节；空闲等 {} 秒",
            args.runs,
            args.sessions,
            args.events,
            args.tail,
            args.reloads,
            args.appends,
            args.reply_bytes,
            args.idle_wait.as_secs()
        ),
    );
    text.push_str("- 量法：`docs/blueprint/perf.md`。原始数据在同名的 `.json`。\n\n");
}

/// 没有预算的几项。
fn unbudgeted(text: &mut String, results: &Results) {
    let large = &results.large;
    let cold = &results.cold;
    text.push_str("\n### 没有预算的\n\n| 项目 | 量到 |\n|---|---|\n");
    for (label, values) in [
        ("冷启动：拉起核心到它写来 ready", &cold.ready),
        ("冷启动：到握完手", &cold.hello),
        (
            "冷启动：到造好会话、订阅上（能提交第一轮，核心这一截；23 没给数，头先画后连，能打字不等核心）",
            &cold.submit,
        ),
    ] {
        line(
            text,
            &format!("| {label}（p50 / p95） | {} ms |", spread(values)),
        );
    }
    line(
        text,
        &format!(
            "| 大会话 | {} 条事件，{} 轮，日志 {:.1} MB |",
            large.events,
            large.turns.len() + large.reload.len(),
            large.log_bytes as f64 / MB
        ),
    );
    for (moment, found) in [
        ("载着大会话，核心".to_string(), &large.loaded),
        (
            format!(
                "不订阅了、等 {} 秒以后，核心",
                results.args.idle_wait.as_secs()
            ),
            &large.idle,
        ),
        (
            "重启、打开大会话、说了一句以后，核心".to_string(),
            &large.reloaded,
        ),
    ] {
        line(
            text,
            &format!("| {moment} | {} |", memory_cell(found.first())),
        );
    }
    for size in &results.sizes {
        let stripped = size.stripped.map_or(String::new(), |bytes| {
            format!("，strip 以后 {:.1} MB", bytes as f64 / MB)
        });
        line(
            text,
            &format!(
                "| 二进制 `{}` | {:.1} MB{stripped} |",
                size.name,
                size.bytes as f64 / MB
            ),
        );
    }
}

/// 会话长大的一路上：照说之前有几条事件每一千条一档，说一句到模型收到请求、到这一轮结束。
fn growth(text: &mut String, large: &Large) {
    text.push_str("\n### 会话长大的一路上\n\n");
    text.push_str(
        "| 事件数 | 轮数 | 说一句到收到请求 p50 / p95 | 一轮 p50 / p95 |\n|---|---|---|---|\n",
    );
    let mut buckets: BTreeMap<u64, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for said in &large.turns {
        let (request, turn) = buckets.entry(said.seq / 1000).or_default();
        request.push(said.request);
        turn.push(said.turn);
    }
    for (bucket, (request, turn)) in &buckets {
        line(
            text,
            &format!(
                "| {}–{} | {} | {} ms | {} ms |",
                bucket * 1000,
                bucket * 1000 + 999,
                request.len(),
                spread(request),
                spread(turn)
            ),
        );
    }
}

/// 每个进程的内存。
fn memory(text: &mut String, results: &Results) {
    text.push_str("\n### 每个进程\n\n| 时候 | 进程 | PSS | 匿名 | RSS |\n|---|---|---|---|---|\n");
    let large = &results.large;
    let moments = [
        ("核心空闲".to_string(), &results.hot.idle),
        (
            format!("{} 个小会话订阅着", results.hot.sessions),
            &results.hot.active,
        ),
        ("载着大会话".to_string(), &large.loaded),
        ("大会话空闲以后".to_string(), &large.idle),
        ("重启打开大会话".to_string(), &large.reloaded),
    ];
    let mb = |kb: u64| format!("{:.1} MB", kb as f64 / 1024.0);
    for (moment, found) in moments {
        if found.is_empty() {
            line(text, &format!("| {moment} | — | 不量 | 不量 | 不量 |"));
        }
        for process in found {
            line(
                text,
                &format!(
                    "| {moment} | {}（{}） | {} | {} | {} |",
                    process.name,
                    process.pid,
                    mb(process.usage.pss),
                    mb(process.usage.anon),
                    mb(process.usage.rss)
                ),
            );
        }
    }
}

/// 一兆字节、一吉字节，照 1024 算（和 `smaps_rollup` 的 kB 一样）。
const MB: f64 = 1024.0 * 1024.0;
const GB: f64 = MB * 1024.0;

/// 核心进程的 PSS，MB。
fn core_pss(found: &[Process]) -> Option<f64> {
    found.first().map(|core| core.usage.pss as f64 / 1024.0)
}

/// 一个进程的内存写成一格。
fn memory_cell(process: Option<&Process>) -> String {
    process.map_or("不量".to_string(), |process| {
        format!(
            "PSS {:.1} MB，匿名 {:.1} MB",
            process.usage.pss as f64 / 1024.0,
            process.usage.anon as f64 / 1024.0
        )
    })
}

/// 加一行。
fn line(text: &mut String, line: &str) {
    text.push_str(line);
    text.push('\n');
}
