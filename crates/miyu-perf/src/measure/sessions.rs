//! 热启动和每多一个会话（`23-性能预算.md` 第二节「热启动到能提交第一轮」「核心里每多一个活动会话」）。
//!
//! 先造几个小会话各说一句，重启核心：一个会话都没载入。量核心空闲的内存；再一个个连上、握手、订阅（订阅时核心才
//! 载入它），量这一段；都订阅着、各再说一句以后量内存，涨的除以会话数。

use std::time::Instant;

use crate::fake::Fake;
use crate::measure::{SETTLE, open, say, subscribe};
use crate::memory::{self, Process};
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;
use crate::stats::ms;

/// 量到的。
#[derive(Debug, Default)]
pub struct Hot {
    /// 每个会话一个：连上、握手、订阅用了多久（毫秒）。
    pub load: Vec<f64>,
    /// 核心刚起来、一个会话都没载入时的内存。
    pub idle: Vec<Process>,
    /// 几个会话都订阅着、各说了一句以后的内存。
    pub active: Vec<Process>,
    /// 几个会话。
    pub sessions: usize,
}

/// 造 `count` 个小会话，量。
///
/// # Errors
///
/// 起不来、连不上、哪一句没说成。
pub async fn hot(sandbox: &Sandbox, fake: &mut Fake, count: usize) -> Result<Hot, String> {
    let running = sandbox.start()?;
    let mut ids = Vec::new();
    {
        let mut rpc = Rpc::connect(&sandbox.root).await?;
        for n in 0..count {
            let session = open(&mut rpc, &sandbox.work).await?;
            say(&mut rpc, fake, &session, &format!("perf-small-{n:03}")).await?;
            ids.push(session);
        }
    }
    sandbox.stop(running)?;

    let running = sandbox.start()?;
    tokio::time::sleep(SETTLE).await;
    let idle = memory::tree(running.pid());
    let mut load = Vec::new();
    let mut connections = Vec::new();
    for session in &ids {
        let began = Instant::now();
        let mut rpc = Rpc::connect(&sandbox.root).await?;
        subscribe(&mut rpc, session).await?;
        load.push(ms(began.elapsed()));
        connections.push(rpc);
    }
    for (n, (rpc, session)) in connections.iter_mut().zip(&ids).enumerate() {
        say(rpc, fake, session, &format!("perf-again-{n:03}")).await?;
    }
    tokio::time::sleep(SETTLE).await;
    let active = memory::tree(running.pid());
    drop(connections);
    sandbox.stop(running)?;
    Ok(Hot {
        load,
        idle,
        active,
        sessions: count,
    })
}
