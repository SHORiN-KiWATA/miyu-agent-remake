//! 原始数据：每一次量的毫秒数（到微秒）、每个进程的内存（KB）、每个程序的字节数。V-3 的回归闸门、以后校准预算都照它。

use serde_json::{Value, json};

use super::Results;
use crate::memory::Process;

/// 毫秒数留到微秒：再细的是噪声，还让文件白长。
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn times(values: &[f64]) -> Value {
    values.iter().copied().map(round).collect()
}

fn processes(found: &[Process]) -> Value {
    found.iter().map(Process::to_json).collect()
}

/// 整份原始数据。
pub fn json(results: &Results) -> Value {
    let args = &results.args;
    let large = &results.large;
    let turns: Vec<Value> = large
        .turns
        .iter()
        .map(|said| {
            json!([
                said.seq,
                round(said.request),
                round(said.turn),
                round(said.projection)
            ])
        })
        .collect();
    json!({
        "date": results.date,
        "machine": results.machine.to_json(),
        "plan": {"runs": args.runs, "sessions": args.sessions, "events": args.events, "tail": args.tail,
            "reloads": args.reloads, "appends": args.appends, "reply_bytes": args.reply_bytes,
            "idle_wait_s": args.idle_wait.as_secs()},
        "binaries": results.sizes.iter().map(crate::measure::size::Size::to_json).collect::<Vec<_>>(),
        "cold": {"ready_ms": times(&results.cold.ready), "hello_ms": times(&results.cold.hello),
            "submit_ms": times(&results.cold.submit)},
        "hot": {"sessions": results.hot.sessions, "load_ms": times(&results.hot.load),
            "idle": processes(&results.hot.idle), "active": processes(&results.hot.active)},
        "large": {"events": large.events, "log_bytes": large.log_bytes, "tail": large.tail,
            "turns_seq_request_ms_turn_ms_projection_ms": turns,
            "loaded": processes(&large.loaded), "idle": processes(&large.idle),
            "reload_ms": times(&large.reload), "first_request_ms": times(&large.first_request),
            "first_projection_ms": times(&large.first_projection),
            "reloaded": processes(&large.reloaded)},
        "append": {"sync_ms": times(&results.appends)},
    })
}
