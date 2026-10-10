//! 冷启动（`23-性能预算.md` 第一节「启动分三个点量」，核心这一边）：核心没在跑，拉起它到写来 `ready`；再连上、握手；
//! 再造一个会话、订阅它，就是能提交第一轮。头停着，「第一帧」「能打字」随头再量。
//!
//! 先空跑一次不算：第一次起来要建数据根里的几样（系统账号、索引），以后每次起来都不用。

use std::time::Instant;

use crate::measure::open;
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;
use crate::stats::ms;

/// 冷启动量到的，每一次一个数（毫秒），都从拉起核心算起。
#[derive(Debug, Default)]
pub struct Cold {
    /// 到核心写来 `ready`。
    pub ready: Vec<f64>,
    /// 到握完手。
    pub hello: Vec<f64>,
    /// 到造好会话、订阅上：能提交第一轮。
    pub submit: Vec<f64>,
}

/// 空跑一次，再量 `runs` 次。
///
/// # Errors
///
/// 哪一次起不来、连不上、造不了会话。
pub async fn cold(sandbox: &Sandbox, runs: usize) -> Result<Cold, String> {
    once(sandbox).await?;
    let mut cold = Cold::default();
    for _ in 0..runs {
        let (ready, hello, submit) = once(sandbox).await?;
        cold.ready.push(ready);
        cold.hello.push(hello);
        cold.submit.push(submit);
    }
    Ok(cold)
}

/// 起一次：交回到 `ready`、到握完手、到订阅上的毫秒数。
async fn once(sandbox: &Sandbox) -> Result<(f64, f64, f64), String> {
    let began = Instant::now();
    let running = sandbox.start()?;
    let ready = ms(running.ready);
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    let hello = ms(began.elapsed());
    open(&mut rpc, &sandbox.work).await?;
    let submit = ms(began.elapsed());
    drop(rpc);
    sandbox.stop(running)?;
    Ok((ready, hello, submit))
}
