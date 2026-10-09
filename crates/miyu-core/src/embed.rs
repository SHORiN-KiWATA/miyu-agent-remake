//! 本机 embedding 接到核心上（施工 R-5 下，`docs/blueprint/recall.md` 第四条）：小程序在主程序真实位置的旁边找（照软件包的
//! 找法），模型放在缓存目录的 `embed/` 下，清单是资源目录里出厂的那一份，下载照环境变量走代理。哪一样没有，`Embedder`
//! 造的时候记一行 `WARN embedder unavailable`，记忆照旧只照关键词找。

use miyu_http::{Proxy, fetcher};
use miyu_session::{EMBED_IDLE, EmbedSetup};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;

use crate::TARGET;

/// 小程序叫什么（不带 `.exe`，找的时候照平台接上）。
const PROGRAM: &str = "miyu-embed";

/// 照核心的环境 `env`、资源目录 `resources` 拼好造 `Embedder` 要的。下载的客户端造不出来的（几乎不会）记一行，没有。
pub(crate) fn setup(env: &Env, resources: &ResourceRoot) -> Option<EmbedSetup> {
    let client = match fetcher(Proxy::FromEnvironment) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(target: TARGET, reason = %error, "embedder unavailable");
            return None;
        }
    };
    Some(EmbedSetup {
        program: env
            .exe
            .as_deref()
            .and_then(|exe| miyu_store::packages::locate(PROGRAM, exe)),
        manifest: resources.embed_manifest(),
        cache: miyu_store::root::cache_root(env)
            .ok()
            .map(|root| root.join("embed")),
        client,
        idle: EMBED_IDLE,
    })
}
