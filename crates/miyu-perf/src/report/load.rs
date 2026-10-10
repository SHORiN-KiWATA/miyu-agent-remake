//! 从入库的原始数据读回量到的（`--render`）：预算改了（23 F1、F5：实测后校准），照现在的预算重出对照表，不用再量一遍。
//! 读法和 `raw.rs` 的写法一一对应，写出去再读回来一字不差（`report/tests.rs` 守着）。

use serde_json::Value;

use super::Results;
use crate::args::Args;
use crate::machine::Machine;
use crate::measure::Said;
use crate::measure::large::Large;
use crate::measure::sessions::Hot;
use crate::measure::size::Size;
use crate::measure::startup::Cold;
use crate::memory::{Process, Usage};

/// 读回一整份。次数、大小照原始数据里的 `plan`，路径照 `args`。
///
/// # Errors
///
/// 缺了哪一格、类型不对：交回是哪一格。
pub fn results(raw: &Value, mut args: Args) -> Result<Results, String> {
    let plan = &raw["plan"];
    args.runs = count(&plan["runs"], "plan.runs")?;
    args.sessions = count(&plan["sessions"], "plan.sessions")?;
    args.events = number(&plan["events"], "plan.events")?;
    args.tail = count(&plan["tail"], "plan.tail")?;
    args.reloads = count(&plan["reloads"], "plan.reloads")?;
    args.appends = count(&plan["appends"], "plan.appends")?;
    args.reply_bytes = count(&plan["reply_bytes"], "plan.reply_bytes")?;
    args.idle_wait =
        std::time::Duration::from_secs(number(&plan["idle_wait_s"], "plan.idle_wait_s")?);
    let machine = &raw["machine"];
    let large = &raw["large"];
    Ok(Results {
        date: text(&raw["date"], "date")?,
        machine: Machine {
            cpu: machine["cpu"].as_str().map(str::to_string),
            cores: machine["cores"].as_u64().map(|n| n as usize),
            memory: machine["memory_bytes"].as_u64(),
            os: text(&machine["os"], "machine.os")?,
            arch: text(&machine["arch"], "machine.arch")?,
            kernel: machine["kernel"].as_str().map(str::to_string),
            rustc: machine["rustc"].as_str().map(str::to_string),
            commit: machine["commit"].as_str().map(str::to_string),
        },
        args,
        sizes: list(&raw["binaries"], "binaries")?
            .iter()
            .map(|size| {
                Ok(Size {
                    name: text(&size["name"], "binaries.name")?,
                    bytes: number(&size["bytes"], "binaries.bytes")?,
                    stripped: size["stripped"].as_u64(),
                })
            })
            .collect::<Result<_, String>>()?,
        cold: Cold {
            ready: times(&raw["cold"]["ready_ms"], "cold.ready_ms")?,
            hello: times(&raw["cold"]["hello_ms"], "cold.hello_ms")?,
            submit: times(&raw["cold"]["submit_ms"], "cold.submit_ms")?,
        },
        hot: Hot {
            load: times(&raw["hot"]["load_ms"], "hot.load_ms")?,
            idle: processes(&raw["hot"]["idle"], "hot.idle")?,
            active: processes(&raw["hot"]["active"], "hot.active")?,
            sessions: count(&raw["hot"]["sessions"], "hot.sessions")?,
        },
        large: Large {
            turns: list(
                &large["turns_seq_request_ms_turn_ms"],
                "large.turns_seq_request_ms_turn_ms",
            )?
            .iter()
            .map(|turn| {
                Ok(Said {
                    seq: number(&turn[0], "large.turns[0]")?,
                    request: time(&turn[1], "large.turns[1]")?,
                    turn: time(&turn[2], "large.turns[2]")?,
                })
            })
            .collect::<Result<_, String>>()?,
            tail: count(&large["tail"], "large.tail")?,
            events: number(&large["events"], "large.events")?,
            log_bytes: number(&large["log_bytes"], "large.log_bytes")?,
            dir: std::path::PathBuf::new(),
            loaded: processes(&large["loaded"], "large.loaded")?,
            idle: processes(&large["idle"], "large.idle")?,
            reload: times(&large["reload_ms"], "large.reload_ms")?,
            first_request: times(&large["first_request_ms"], "large.first_request_ms")?,
            reloaded: processes(&large["reloaded"], "large.reloaded")?,
        },
        appends: times(&raw["append"]["sync_ms"], "append.sync_ms")?,
    })
}

fn missing(what: &str) -> String {
    format!("原始数据里的 {what} 缺了，或者类型不对")
}

fn text(value: &Value, what: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| missing(what))
}

fn number(value: &Value, what: &str) -> Result<u64, String> {
    value.as_u64().ok_or_else(|| missing(what))
}

fn count(value: &Value, what: &str) -> Result<usize, String> {
    number(value, what).map(|n| usize::try_from(n).unwrap_or(usize::MAX))
}

fn time(value: &Value, what: &str) -> Result<f64, String> {
    value.as_f64().ok_or_else(|| missing(what))
}

fn list<'a>(value: &'a Value, what: &str) -> Result<&'a Vec<Value>, String> {
    value.as_array().ok_or_else(|| missing(what))
}

fn times(value: &Value, what: &str) -> Result<Vec<f64>, String> {
    list(value, what)?
        .iter()
        .map(|item| time(item, what))
        .collect()
}

fn processes(value: &Value, what: &str) -> Result<Vec<Process>, String> {
    list(value, what)?
        .iter()
        .map(|process| {
            Ok(Process {
                pid: u32::try_from(number(&process["pid"], what)?).map_err(|_| missing(what))?,
                name: text(&process["name"], what)?,
                usage: Usage {
                    rss: number(&process["rss_kb"], what)?,
                    pss: number(&process["pss_kb"], what)?,
                    anon: number(&process["anon_kb"], what)?,
                },
            })
        })
        .collect()
}
