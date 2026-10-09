//! 内置模型接到核心上（施工 R-5 下；R-5 三补改成照包，`docs/blueprint/recall.md` 第四条第 1 款，设计 `30-插件框架.md` 第六节）：
//! 人格记忆装着、它推荐的小程序包 `embed` 也装着（读成了、种类是小程序），就照它拼 `Embedder` 要的：程序照小程序清单的
//! `program` 在主程序真实位置的旁边找，模型清单是包目录里的 `model.toml`，模型的文件在包目录里。没装的本机那一路没有，记一行
//! `INFO embedder unavailable reason=no embed package`（出厂不装，这是常态）；程序不在、模型清单读不了的由 `Embedder` 造的
//! 时候记 `WARN`。
//!
//! [`setup`] 能再调：装卸当场生效（F-5）时照那时读到的清单再拼一次，换上。

use miyu_session::{EMBED_IDLE, EmbedSetup};
use miyu_store::env::Env;
use miyu_store::packages::{Found, locate};

use crate::TARGET;

/// 内置模型这个小程序包的编号：人格记忆推荐的是它。
const WORKER: &str = "embed";

/// 小程序包目录里模型清单叫什么：内置模型这个小程序和用它的包之间的约定，小程序清单里不另写（2026-10-09 核心的主会话定）。
const MODEL: &str = "model.toml";

/// 照核心的环境 `env`、这时读到的清单 `found` 拼好造 `Embedder` 要的：人格记忆没装、没推荐它、它没装（或者不是小程序）的
/// 没有。
pub(crate) fn setup(env: &Env, found: &[Found]) -> Option<EmbedSetup> {
    let manifest = |id: &str| {
        found
            .iter()
            .find(|found| found.id == id)
            .and_then(|found| found.read.as_ref().ok().map(|read| (found, read)))
    };
    let (_, memory) = manifest(miyu_memory::PACKAGE)?;
    if !memory.recommends.iter().any(|worker| worker == WORKER) {
        return None;
    }
    // 有 `[worker]` 的就是小程序：清单的读法管着（小程序必写，别的种类不能写）。
    let Some((package, worker)) = manifest(WORKER)
        .and_then(|(found, read)| read.worker.as_ref().map(|worker| (found, worker)))
    else {
        tracing::info!(target: TARGET, reason = "no embed package", "embedder unavailable");
        return None;
    };
    let dir = package.files_dir();
    Some(EmbedSetup {
        program: env
            .exe
            .as_deref()
            .and_then(|exe| locate(&worker.program, exe)),
        manifest: dir.join(MODEL),
        dir,
        idle: EMBED_IDLE,
    })
}

#[cfg(test)]
mod tests;
