//! 大会话（`23-性能预算.md` 第一节「会话越长越要量」）：一句一句真说出一个会话，到一万条事件，每一轮都记下说出去到
//! 模型收到请求用了多久，这是一次请求的投影的上界：含 `message.user` 落盘、组装、编码、连上假模型。到了以后再说几轮，
//! 一次请求的投影照这几轮算分位。
//!
//! 接着量载着它的内存；不订阅了，等会话 actor 空闲退出，再量一次（分配器攒着不还的就在这里看得出）。最后几次重启核心、
//! 订阅它（从头读日志、重放），量打开它要多久，和打开以后头一次请求要多久：没有投影缓存，这就是从头投影一次。

use std::path::{Path, PathBuf};
use std::time::Instant;

use miyu_kernel::id::{AccountId, SessionId};

use crate::fake::Fake;
use crate::measure::{SETTLE, Said, open, say, subscribe};
use crate::memory::{self, Process};
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;
use crate::stats::ms;

/// 量多大、量几次。
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// 说到第几条事件。
    pub events: u64,
    /// 到了以后再说几轮。
    pub tail: usize,
    /// 重启再打开几次。
    pub reloads: usize,
    /// 不订阅以后等多久再量内存。
    pub idle_wait: std::time::Duration,
}

/// 量到的。
#[derive(Debug, Default)]
pub struct Large {
    /// 一轮一个，照先后：长大的一路上，和到了以后的那几轮（最后 `tail` 个）。
    pub turns: Vec<Said>,
    /// 到了以后再说的几轮有几个。
    pub tail: usize,
    /// 说完以后日志里有几条事件。
    pub events: u64,
    /// 日志占几个字节。
    pub log_bytes: u64,
    /// 会话目录：追加并同步照它的事件量。
    pub dir: PathBuf,
    /// 载着它的内存。
    pub loaded: Vec<Process>,
    /// 不订阅了、等过以后的内存。
    pub idle: Vec<Process>,
    /// 重启以后连上、握手、订阅它用了多久（毫秒）。
    pub reload: Vec<f64>,
    /// 重启、打开以后头一次说，到模型收到请求用了多久（毫秒）。
    pub first_request: Vec<f64>,
    /// 最后一次重启、打开、说了一句以后的内存。
    pub reloaded: Vec<Process>,
}

/// 造一个大会话，量。
///
/// # Errors
///
/// 起不来、连不上、哪一句没说成、日志读不到。
pub async fn large(sandbox: &Sandbox, fake: &mut Fake, plan: Plan) -> Result<Large, String> {
    let running = sandbox.start()?;
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    let session = open(&mut rpc, &sandbox.work).await?;
    let mut large = Large {
        tail: plan.tail,
        dir: directory(sandbox, &session)?,
        ..Large::default()
    };
    let mut extra = 0;
    while extra < plan.tail {
        let n = large.turns.len();
        let said = say(&mut rpc, fake, &session, &format!("perf-large-{n:06}")).await?;
        if said.seq >= plan.events {
            extra += 1;
        }
        if n.is_multiple_of(200) {
            eprintln!(
                "  大会话：{} 条事件，这一轮说出去到收到请求 {:.1} ms",
                said.seq, said.request
            );
        }
        large.turns.push(said);
    }
    tokio::time::sleep(SETTLE).await;
    large.loaded = memory::tree(running.pid());
    drop(rpc);
    eprintln!(
        "  等 {} 秒，会话 actor 空闲退出以后再量内存",
        plan.idle_wait.as_secs()
    );
    tokio::time::sleep(plan.idle_wait).await;
    large.idle = memory::tree(running.pid());
    sandbox.stop(running)?;

    for n in 0..plan.reloads {
        let running = sandbox.start()?;
        let began = Instant::now();
        let mut rpc = Rpc::connect(&sandbox.root).await?;
        subscribe(&mut rpc, &session).await?;
        large.reload.push(ms(began.elapsed()));
        let said = say(&mut rpc, fake, &session, &format!("perf-reload-{n:03}")).await?;
        large.first_request.push(said.request);
        if n + 1 == plan.reloads {
            tokio::time::sleep(SETTLE).await;
            large.reloaded = memory::tree(running.pid());
        }
        drop(rpc);
        sandbox.stop(running)?;
    }
    large.events = count_events(&large.dir)?;
    large.log_bytes = size_of(&large.dir);
    Ok(large)
}

/// 会话的目录。
fn directory(sandbox: &Sandbox, session: &str) -> Result<PathBuf, String> {
    let admin = AccountId::parse("admin").map_err(|e| format!("账号名写坏了：{e}"))?;
    let session = SessionId::parse(session).map_err(|e| format!("会话编号写坏了：{e}"))?;
    Ok(sandbox.root.session_dir(&admin, &session))
}

/// 日志里有几条事件。
fn count_events(dir: &Path) -> Result<u64, String> {
    miyu_store::log::read_events(dir)
        .map(|events| events.len() as u64)
        .map_err(|e| format!("大会话的日志读不了：{e}"))
}

/// 目录里的文件一共几个字节（不往下钻）。
fn size_of(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .filter(|meta| meta.is_file())
                .map(|meta| meta.len())
                .sum()
        })
        .unwrap_or(0)
}
