//! 照一家供应商拼出发请求要的几样（施工 R-5 补，从 `probe.rs` 抽出来）：地址、key、另配的头。不走路由、池、冷却：
//! `provider.test` 试一家，远程的 embedding（`embed/remote.rs`）照 `models.embedding` 点名的那一家发。配置照调的一方手里
//! 的那一份（会话里是这一轮冻结的）。

use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_http::Endpoint;
use miyu_models::Knowledge;
use miyu_models::provider::{self, NoModel, Provider};

/// 一家供应商：它的配置、地址、key（配了的）。
pub(crate) struct Reached {
    /// 这一家照配置和模型资料合出来的样子：驱动、另配的头。
    pub(crate) provider: Provider,
    /// 地址（环境变量的引用已经照核心的环境换好）。
    pub(crate) base_url: String,
    /// key：配了的；不带认证的本机服务没有。
    pub(crate) key: Option<Secret>,
}

impl Reached {
    /// 发请求的端点：地址、key，另配的头照种子 `seed` 换好（会话的是会话编号，一次性的是用途，试一家的是固定的那一个）。
    pub(crate) fn endpoint(&self, seed: &str) -> Endpoint {
        let endpoint = match &self.key {
            Some(key) => Endpoint::new(self.base_url.clone(), key.expose()),
            None => Endpoint::keyless(self.base_url.clone()),
        };
        self.provider
            .headers(seed)
            .into_iter()
            .fold(endpoint, |endpoint, (name, value)| {
                endpoint.with_header(name, value)
            })
    }
}

/// 照配置 `values`、模型资料 `knowledge` 找编号是 `id` 的那一家，地址、key 照 `secret` 取。
///
/// # Errors
///
/// 没配这一家；地址取不到；配了 key 却取不到：英文的一句（`no provider "x"` 这样，和路由说的一样）。
pub(crate) fn reach(
    values: &Values,
    knowledge: &Knowledge<'_>,
    id: &str,
    secret: &dyn Fn(&Reference) -> Option<Secret>,
) -> Result<Reached, NoModel> {
    let provider = provider::provider(values, knowledge, id)?;
    let base_url = provider::resolve_base_url(&provider, secret)?;
    let key = match &provider.key {
        None => None,
        Some(reference) => Some(
            secret(reference)
                .ok_or_else(|| NoModel(format!("provider {id:?} has no usable key")))?,
        ),
    };
    Ok(Reached {
        provider,
        base_url,
        key,
    })
}
