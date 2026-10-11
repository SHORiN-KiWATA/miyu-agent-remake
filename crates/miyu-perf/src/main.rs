//! 量尺（施工 V-1，`docs/designs/23-性能预算.md` 第一节，`docs/blueprint/perf.md`）：拉起 release 构建的真 `miyu`，
//! 模型是同一个进程里的假服务，数据根是沙盒；量核心这一边的启动、内存、长会话上的请求、追加并同步、二进制大小，
//! 原始数据和对照预算的表写进 `docs/perf/`。
//!
//! 工具 crate，不属于任何一层，谁都不许依赖它（`01-架构.md` 第九节规则 5）。`cargo xtask perf` 编好再跑它。

mod args;
mod budget;
mod fake;
mod machine;
mod measure;
mod memory;
mod report;
mod rpc;
mod sandbox;
mod stats;

use std::process::ExitCode;
use std::time::SystemTime;

use args::Args;
use fake::Fake;
use measure::large::Plan;
use report::Results;
use sandbox::Sandbox;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args::parse(&words) {
        Ok(args) => run(args).await,
        Err(error) => Err(error),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// 照 `args` 量一遍，写结果；带 `--render` 的不量，照原始数据重出表。
async fn run(args: Args) -> Result<(), String> {
    let design = std::fs::read_to_string(&args.design)
        .map_err(|e| format!("读不了 {}：{e}", args.design.display()))?;
    // 预算先读：表对不上的，不白量几分钟。
    let budgets = budget::parse(&design)?;
    if let Some(item) = report::ITEMS
        .iter()
        .find(|item| !budgets.contains_key(**item))
    {
        return Err(format!("23 第二节没有「{item}」"));
    }
    if let Some(raw) = &args.render {
        let text =
            std::fs::read_to_string(raw).map_err(|e| format!("读不了 {}：{e}", raw.display()))?;
        let raw: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("原始数据不是 JSON：{e}"))?;
        let results = report::load(&raw, args.clone())?;
        let written = report::write_markdown(&results, &budgets, &args.out)?;
        eprintln!("照原始数据重出了 {}", written.display());
        return Ok(());
    }
    let mut fake = Fake::start(args.reply_bytes).await?;
    let sandbox = Sandbox::new(&args.work, &args.miyu, &args.resources, &fake.base_url)?;

    eprintln!("冷启动：空跑一次，量 {} 次", args.runs);
    let cold = measure::startup::cold(&sandbox, args.runs).await?;
    eprintln!("热启动、每多一个会话：{} 个小会话", args.sessions);
    let hot = measure::sessions::hot(&sandbox, &mut fake, args.sessions).await?;
    eprintln!("大会话：说到 {} 条事件", args.events);
    let plan = Plan {
        events: args.events,
        tail: args.tail,
        reloads: args.reloads,
        idle_wait: args.idle_wait,
    };
    let large = measure::large::large(&sandbox, &mut fake, plan).await?;
    eprintln!("追加并同步：{} 条", args.appends);
    let appends = measure::append::append(&large.dir, &args.work.join("append"), args.appends)?;
    let sizes = measure::size::sizes(&args.miyu, &args.work.join("strip"));

    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("时钟在 1970 年以前：{e}"))?
        .as_secs();
    let results = Results {
        date: report::date_of(seconds),
        machine: machine::Machine::read(),
        args: args.clone(),
        sizes,
        cold,
        hot,
        large,
        appends,
    };
    let written = report::write(&results, &budgets, &args.out)?;
    eprintln!("写好了 {}", written.display());
    report::enforce(&results, &budgets, args.gate)
}
