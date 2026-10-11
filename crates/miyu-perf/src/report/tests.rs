//! 日期的写法；一份造出来的结果写成表：照预算判、没量的写「不量」、缺预算的报错。

use std::collections::BTreeMap;

use super::*;
use crate::budget::Unit;
use crate::measure::Said;
use crate::memory::{Process, Usage};

#[test]
fn dates_are_civil_utc() {
    assert_eq!(date_of(0), "1970-01-01");
    assert_eq!(date_of(951_782_400), "2000-02-29");
    assert_eq!(date_of(951_868_800), "2000-03-01");
    assert_eq!(date_of(1_704_067_199), "2023-12-31");
    assert_eq!(date_of(1_791_590_400), "2026-10-10");
}

fn budgets() -> BTreeMap<String, Budget> {
    let ms = |quantile, value| Budget {
        quantile,
        value,
        unit: Unit::Ms,
    };
    let mb = |value| Budget {
        quantile: 50,
        value,
        unit: Unit::Mb,
    };
    [
        ("热启动到能提交第一轮", ms(50, 200.0)),
        ("一次请求的投影（增量，一万条事件的会话）", ms(95, 5.0)),
        ("同一个会话从头投影一次", ms(50, 100.0)),
        ("追加一步的事件并同步", ms(99, 20.0)),
        ("核心空闲、没有会话载入", mb(30.0)),
        ("核心里每多一个活动会话", mb(5.0)),
    ]
    .into_iter()
    .map(|(name, budget)| (name.to_string(), budget))
    .collect()
}

fn core(pss_mb: u64) -> Vec<Process> {
    vec![Process {
        pid: 42,
        name: "miyu".to_string(),
        usage: Usage {
            rss: pss_mb * 1024 + 100,
            pss: pss_mb * 1024,
            anon: pss_mb * 512,
        },
    }]
}

fn results(memory: bool) -> Results {
    let words: Vec<String> = "--miyu /m --resources /r --design /d --work /w --out /o --tail 2"
        .split_whitespace()
        .map(str::to_string)
        .collect();
    // 落了盘到收到请求是说出去到收到请求的一半：同步占了另一半。
    let said = |seq, request| Said {
        seq,
        request,
        turn: request * 10.0,
        projection: request / 2.0,
    };
    let pick = |found: Vec<Process>| if memory { found } else { Vec::new() };
    Results {
        date: "2026-10-10".to_string(),
        machine: Machine {
            cpu: None,
            cores: Some(8),
            memory: None,
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            kernel: None,
            rustc: None,
            commit: Some("abc1234".to_string()),
        },
        args: crate::args::parse(&words).unwrap(),
        sizes: Vec::new(),
        cold: Cold {
            ready: vec![40.0],
            hello: vec![41.0],
            submit: vec![45.0],
        },
        hot: Hot {
            load: vec![3.0, 4.0],
            idle: pick(core(20)),
            active: pick(core(40)),
            sessions: 10,
        },
        large: Large {
            // 长大的一路上三轮，到了以后两轮：投影只照最后两轮比。
            turns: vec![
                said(10, 1.0),
                said(1500, 2.0),
                said(9990, 3.0),
                said(10_002, 4.0),
                said(10_008, 6.0),
            ],
            tail: 2,
            events: 10_010,
            log_bytes: 8 * 1024 * 1024,
            reload: vec![150.0],
            first_request: vec![120.0],
            first_projection: vec![60.0],
            ..Large::default()
        },
        appends: vec![1.0, 2.0, 30.0],
    }
}

#[test]
fn the_table_judges_against_the_budgets() {
    let text = rows::markdown(&results(true), &budgets(), "2026-10-10-linux-x86_64").unwrap();
    let row = |start: &str| {
        text.lines()
            .find(|line| line.starts_with(&format!("| {start}")))
            .unwrap_or_else(|| panic!("没有「{start}」这一行：\n{text}"))
            .to_string()
    };
    assert!(row("热启动到能提交第一轮：连上").ends_with("| 200 ms | 过 |"));
    assert!(row("热启动到能提交第一轮：重启").ends_with("| 200 ms | 过 |"));
    // 最后两轮说出去到收到请求是 4、6，落了盘到收到请求是 2、3：投影照后者比，p95 是 3，没超 5（施工 V-2 中）。
    let projection = row("一次请求的投影");
    assert!(
        projection.contains("p50 2.0 / p95 3.0 / p99 3.0 ms（2 次）"),
        "{projection}"
    );
    assert!(projection.ends_with("| p95 5 ms | 过 |"), "{projection}");
    assert!(
        row("从头投影").ends_with("| 100 ms | 过 |"),
        "照落了盘的 60 比"
    );
    assert!(
        row("大会话上说一句到模型收到请求").contains("| 4.0 / 6.0 ms |"),
        "含同步的另列一行"
    );
    assert!(row("重启、打开大会话后头一次说").contains("| 120.0 / 120.0 ms |"));
    assert!(row("追加一条事件并同步").ends_with("| p99 20 ms | 超 |"));
    assert!(row("核心空闲").contains("| PSS 20.0 MB，匿名 10.0 MB | 30 MB | 过 |"));
    assert!(row("核心里每多一个活动会话").contains("| 2.00 MB | 5 MB | 过 |"));
    assert!(row("大会话 |").contains("10010 条事件，6 轮，日志 8.0 MB"));
    assert!(
        text.contains("| 0–999 | 1 | 1.0 / 1.0 ms | 0.5 / 0.5 ms | 10.0 / 10.0 ms |"),
        "{text}"
    );
    assert!(
        text.contains("| 10000–10999 | 2 | 4.0 / 6.0 ms |"),
        "{text}"
    );
    assert!(
        text.contains("| 核心空闲 | miyu（42） | 20.0 MB | 10.0 MB | 20.1 MB |"),
        "{text}"
    );
    assert!(text.contains("- 提交：abc1234\n"));
}

#[test]
fn memory_that_could_not_be_read_is_not_measured() {
    let text = rows::markdown(&results(false), &budgets(), "x").unwrap();
    assert!(
        text.contains("| 核心空闲、没有会话载入 | 不量 | 30 MB | 不量 |"),
        "{text}"
    );
    assert!(text.contains("| 不量 | 5 MB | 不量 |"), "{text}");
    assert!(
        text.contains("| 核心空闲 | — | 不量 | 不量 | 不量 |"),
        "{text}"
    );
}

#[test]
fn a_missing_budget_is_an_error() {
    let mut budgets = budgets();
    budgets.remove("同一个会话从头投影一次");
    let error = rows::markdown(&results(true), &budgets, "x").unwrap_err();
    assert_eq!(error, "23 第二节没有「同一个会话从头投影一次」");
}

#[test]
fn the_raw_data_keeps_every_number() {
    let raw = raw::json(&results(true));
    assert_eq!(
        raw["large"]["turns_seq_request_ms_turn_ms_projection_ms"][1],
        serde_json::json!([1500, 2.0, 20.0, 1.0])
    );
    assert_eq!(
        raw["large"]["first_projection_ms"],
        serde_json::json!([60.0])
    );
    assert_eq!(
        raw["append"]["sync_ms"],
        serde_json::json!([1.0, 2.0, 30.0])
    );
    assert_eq!(raw["hot"]["idle"][0]["pss_kb"], 20 * 1024);
    assert_eq!(raw["plan"]["tail"], 2);
    assert_eq!(raw["machine"]["commit"], "abc1234");
}

#[test]
fn the_raw_data_reads_back_exactly() {
    let written = raw::json(&results(true));
    // 交进去的是默认的次数：读回来的要照原始数据里的（`--tail 2`），不照它。
    let fresh: Vec<String> = "--render /o/x.json --design /d --out /o"
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let read = load(&written, crate::args::parse(&fresh).unwrap()).unwrap();
    assert_eq!(raw::json(&read), written);
    // 表照读回来的出，和量完当场出的一样。
    assert_eq!(
        rows::markdown(&read, &budgets(), "x").unwrap(),
        rows::markdown(&results(true), &budgets(), "x").unwrap()
    );
}

#[test]
fn a_broken_raw_file_says_which_cell() {
    let mut written = raw::json(&results(true));
    written["large"]["turns_seq_request_ms_turn_ms_projection_ms"][1][1] =
        serde_json::json!("slow");
    let error = load(&written, results(false).args).err().unwrap();
    assert_eq!(error, "原始数据里的 large.turns[1] 缺了，或者类型不对");
    written["plan"] = serde_json::json!({});
    assert!(load(&written, results(false).args).is_err());
}

#[test]
fn raw_data_from_before_v2_middle_reads_projection_as_the_whole_request() {
    // V-2 中以前的原始数据：一轮三格、没有 `first_projection_ms`。落了盘到收到请求照说出去到收到请求算（上界）。
    let mut written = raw::json(&results(true));
    let large = written["large"].as_object_mut().unwrap();
    let turns: Vec<serde_json::Value> = large["turns_seq_request_ms_turn_ms_projection_ms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|turn| serde_json::json!([turn[0], turn[1], turn[2]]))
        .collect();
    large.remove("turns_seq_request_ms_turn_ms_projection_ms");
    large.insert("turns_seq_request_ms_turn_ms".into(), turns.into());
    large.remove("first_projection_ms");
    let read = load(&written, results(false).args).unwrap();
    let said = read.large.turns[1];
    assert_eq!((said.request, said.projection), (2.0, 2.0));
    assert_eq!(read.large.first_projection, vec![120.0]);
}

#[test]
fn the_gate_names_only_the_rows_past_the_factor() {
    // 追加的 p99 约 30 ms，预算 20 ms：一倍拦下它一行，两倍都放过（施工 V-3）。
    let over = gate::over(&results(true), &budgets(), 1.0).unwrap();
    assert_eq!(over.len(), 1, "{over:?}");
    assert!(over[0].starts_with("追加一条事件并同步：量到 "), "{over:?}");
    assert!(
        over[0].contains("预算 p99 20 ms，闸门是它的 1 倍（20.0 ms）"),
        "{over:?}"
    );
    assert!(
        gate::over(&results(true), &budgets(), 2.0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_gate_holds_memory_to_the_budget_itself() {
    // 内存不随机器快慢变：倍数再大，空闲的核心超了预算照拦；没量的不算超（施工 V-3）。
    let mut tight = budgets();
    tight.insert(
        "核心空闲、没有会话载入".to_string(),
        Budget {
            quantile: 50,
            value: 10.0,
            unit: Unit::Mb,
        },
    );
    let over = gate::over(&results(true), &tight, 100.0).unwrap();
    assert_eq!(over.len(), 1, "{over:?}");
    assert!(
        over[0].starts_with("核心空闲、没有会话载入：量到 20.0 MB"),
        "{over:?}"
    );
    assert!(over[0].contains("闸门是它的 1 倍（10.0 MB）"), "{over:?}");
    assert!(
        gate::over(&results(false), &tight, 100.0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_gate_fails_the_run_only_when_asked_and_over() {
    assert_eq!(gate::enforce(&results(true), &budgets(), None), Ok(()));
    assert_eq!(gate::enforce(&results(true), &budgets(), Some(2.0)), Ok(()));
    let error = gate::enforce(&results(true), &budgets(), Some(1.0)).unwrap_err();
    assert!(
        error.starts_with("闸门没过：\n追加一条事件并同步："),
        "{error}"
    );
}
