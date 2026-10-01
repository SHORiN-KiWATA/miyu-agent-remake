//! 试一家供应商（`docs/blueprint/models.md`「协议」`provider.test`、「怎么走」第七条第 4 条，施工 8-11）：推驱动、地址、
//! key，列模型，挑一个，发一句，收到第一段正文就停。
//!
//! 1. 照最终值推这一家（和会话的路由同一个 `miyu_models::provider::provider`）。推不出的、地址或 key 取不到的：`config`。
//! 2. 列模型（[`list_models`]，和拉列表一样整个 30 秒）。拉到了的，`save` 的（配好了的一家）存进供应商的列表；拉不到的
//!    照目录里对上的那一家列，出错照驱动分类留着。
//! 3. 挑模型：写了的用它；没写的照推荐挑（`miyu_models::onboard::recommend`）。列表是空的：`list`，交第 2 步的出错。
//! 4. 发：只有一条 user，没有 system、没有工具面；收到正文那一块的第一段字就叫停，空闲 60 秒。客户端和列模型用同一个
//!    （照地址挑，施工 8-11 补：地址落在本机的不走代理，别的照环境变量，和会话真发时一样）。请求发了就报请求的结果
//!    （「施工时定的」8-11）。
//! 5. 不记会话日志、不记用量；记一行 `INFO provider tested`。key、地址不进任何一行。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use miyu_config::Values;
use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference, Secret};
use miyu_drivers::classify::Failure;
use miyu_drivers::{Call, Driver, Inputs, OpenAiChat};
use miyu_http::{Attempt, Endpoint, Failed, Outcome, Progress, send};
use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::ModelName;
use miyu_kernel::request::{Message, Request};
use miyu_models::facts::facts;
use miyu_models::observed::ProviderList;
use miyu_models::onboard::{Offered, recommend, released};
use miyu_models::provider::{self, NoModel, Provider};

use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::route::lists::{TIMEOUT, list_models, listing_texts};
use crate::route::shared::ModelData;

/// 发那一句时多久没收到新的字节就算断了。
const IDLE: Duration = Duration::from_secs(60);

/// 试哪一家、照什么试。
pub struct Probe<'a> {
    /// 这一家在里面的一份最终值：配好的照配置，候选的照它写成的（`miyu_models::onboard::Candidate`）。
    pub values: &'a Values,
    /// 模型手写的资料从哪查：配好的照配置，候选的是空的。
    pub resolved: &'a Resolved,
    /// 照引用取 key、取是环境变量的引用的地址。
    pub secret: &'a (dyn Fn(&Reference) -> Option<Secret> + Sync),
    /// 编号。
    pub id: &'a str,
    /// 拿哪个模型试；没有的照推荐挑。
    pub model: Option<&'a str>,
    /// 发的那一句（`core/models/probe.txt`，去掉了行尾的空白）。
    pub text: &'a str,
    /// 列到的模型存进供应商的列表：配好的一家才存。
    pub save: bool,
}

/// 哪一步没成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// 推不出驱动、地址，取不到 key。
    Config,
    /// 列模型：没有模型可试。
    List,
    /// 发请求。
    Request,
}

impl Stage {
    /// 协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Config => "config",
            Stage::List => "list",
            Stage::Request => "request",
        }
    }
}

/// 试的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probed {
    /// 通了。
    Worked {
        /// 列出来的模型名，照字节排、去重。
        models: Vec<String>,
        /// 是不是照目录列的（供应商列不出）。
        from_catalog: bool,
        /// 试的哪个。
        model: String,
        /// 从发出去到收到第一段增量的毫秒数。
        first_token_ms: u64,
    },
    /// 没成：哪一步、出错（和 `model.called` 的一样）。
    Failed {
        /// 哪一步。
        stage: Stage,
        /// 分类、状态、原话。
        error: CallError,
    },
}

/// 试一次，记一行 `INFO provider tested`。
pub async fn probe(data: &Arc<ModelData>, probe: Probe<'_>) -> Probed {
    let mut tried = None;
    let probed = match run(data, &probe, &mut tried).await {
        Ok(worked) | Err(worked) => worked,
    };
    let ok = matches!(probed, Probed::Worked { .. });
    let model = tried.unwrap_or_default();
    tracing::info!(target: TARGET, provider = probe.id, model = %model, ok, "provider tested");
    probed
}

/// 照先后走一遍：哪一步没成就停在那一步（`Err`）。试的模型记进 `tried`。
async fn run(
    data: &Arc<ModelData>,
    probe: &Probe<'_>,
    tried: &mut Option<String>,
) -> Result<Probed, Probed> {
    let config = |NoModel(why): NoModel| failed(Stage::Config, ErrorClass::NoModel, why);
    let provider = data
        .with(|knowledge| provider::provider(probe.values, knowledge, probe.id))
        .map_err(config)?;
    let base_url = provider::resolve_base_url(&provider, probe.secret).map_err(config)?;
    let key = match provider.keys.is_empty() {
        true => None,
        false => Some(provider.keys.iter().find_map(probe.secret).ok_or_else(|| {
            config(NoModel(format!(
                "provider {:?} has no usable key",
                probe.id
            )))
        })?),
    };
    let unready = |why: String| failed(Stage::Config, ErrorClass::Unclassified, why);
    // 地址落在本机的不走代理（施工 8-11 补，`route/shared.rs` `ModelData::fetcher_for`）。
    let client = data
        .fetcher_for(&base_url)
        .cloned()
        .ok_or_else(|| unready("no client to send with".to_string()))?;
    let texts = listing_texts().map_err(unready)?;
    let driver = OpenAiChat::new(provider.compat.clone(), texts);
    let headers = key
        .as_ref()
        .map(|key| driver.auth(key.expose()))
        .unwrap_or_default();
    let listed = list_models(&client, &driver, &base_url, &headers, TIMEOUT).await;
    let (models, from_catalog, unlisted) = match listed {
        Ok(listed) => {
            let mut models: Vec<String> = listed.iter().map(|model| model.id.clone()).collect();
            models.sort_unstable();
            models.dedup();
            if probe.save {
                let (data, id) = (Arc::clone(data), probe.id.to_string());
                let list = ProviderList {
                    fetched: wall_now(),
                    models: listed,
                };
                blocking(move || data.set_list(&id, list)).await;
            }
            (models, false, None)
        }
        Err(why) => (
            catalog_models(data, &provider),
            true,
            Some(classify(&driver, &why)),
        ),
    };
    let model = match probe.model {
        Some(model) => model.to_string(),
        None => pick(data, probe.resolved, &provider, &models).ok_or_else(|| Probed::Failed {
            stage: Stage::List,
            error: unlisted.unwrap_or_else(|| CallError {
                class: ErrorClass::Unclassified,
                message: "no models listed".to_string(),
                status: None,
            }),
        })?,
    };
    *tried = Some(model.clone());
    let endpoint = match &key {
        Some(key) => Endpoint::new(base_url, key.expose()),
        None => Endpoint::keyless(base_url),
    };
    let first_token_ms = ask(&client, &driver, &endpoint, &model, probe.text).await?;
    Ok(Probed::Worked {
        models,
        from_catalog,
        model,
        first_token_ms,
    })
}

/// 发那一句：收到正文那一块的第一段字就叫停。交回第一段增量的毫秒数。
async fn ask(
    client: &miyu_http::Client,
    driver: &OpenAiChat,
    endpoint: &Endpoint,
    model: &str,
    text: &str,
) -> Result<u64, Probed> {
    let name = ModelName::parse(model)
        .map_err(|error| failed(Stage::Request, ErrorClass::Unclassified, error.to_string()))?;
    let request = Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::User {
            blocks: vec![Block::Text(Text {
                text: text.to_string(),
            })],
        }],
        stable: 0,
        continuation: false,
    };
    let call = Call {
        model: name,
        max_output: None,
        inputs: Inputs::default(),
    };
    let encoded = driver
        .encode(&request, &call, &BTreeMap::new())
        .map_err(|error| failed(Stage::Request, ErrorClass::Unclassified, error.to_string()))?;
    let attempt = Attempt {
        client,
        endpoint,
        driver,
        body: &encoded.body,
        path: &encoded.path,
        idle: IDLE,
    };
    let stop = Notify::new();
    let seen = Mutex::new(Seen::default());
    let outcome = send(attempt, stop.notified(), |progress| {
        let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
        match progress {
            Progress::Sent { .. } => seen.sent = Some(Instant::now()),
            Progress::Delta(delta) => {
                seen.first.get_or_insert_with(Instant::now);
                if seen.text(&delta) {
                    stop.notify_one();
                }
            }
        }
    })
    .await;
    let seen = seen.into_inner().unwrap_or_else(PoisonError::into_inner);
    match outcome {
        Outcome::Ended {
            error: Some(classified),
            ..
        } => Err(Probed::Failed {
            stage: Stage::Request,
            error: classified.error,
        }),
        Outcome::Ended { error: None, .. } | Outcome::Cancelled => Ok(seen.first_token_ms()),
    }
}

/// 发那一句时看到的：什么时候发出去的、第一段增量什么时候到的、哪几块是正文。
#[derive(Default)]
struct Seen {
    sent: Option<Instant>,
    first: Option<Instant>,
    text_blocks: Vec<usize>,
}

impl Seen {
    /// 记下这一段增量；是正文那一块的一段字的交回真：该停了。
    fn text(&mut self, delta: &Delta) -> bool {
        match delta {
            Delta::Start {
                index,
                kind: Kind::Text,
            } => {
                self.text_blocks.push(*index);
                false
            }
            Delta::Text { index, text } => !text.is_empty() && self.text_blocks.contains(index),
            _ => false,
        }
    }

    /// 从发出去到第一段增量；一段都没有的算到这一刻。
    fn first_token_ms(&self) -> u64 {
        let sent = self.sent.unwrap_or_else(Instant::now);
        let first = self.first.unwrap_or_else(Instant::now);
        u64::try_from(first.saturating_duration_since(sent).as_millis()).unwrap_or(u64::MAX)
    }
}

/// 列模型没成：照驱动分类（状态码、头、响应体；连不上的只有原话），和 `model.called` 的一样。
fn classify(driver: &OpenAiChat, failed: &Failed) -> CallError {
    let headers: Vec<(&str, &str)> = failed
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let body = match failed.status {
        Some(_) => failed.body.as_slice(),
        None => failed.message.as_bytes(),
    };
    driver
        .classify(&Failure {
            status: failed.status,
            headers: &headers,
            body,
        })
        .error
}

/// 目录里对上的那一家的模型名（照字节排）；没对上、没有目录的是空的。
fn catalog_models(data: &ModelData, provider: &Provider) -> Vec<String> {
    data.with(|knowledge| {
        let recognized = provider.recognized.as_ref();
        recognized
            .zip(knowledge.catalog)
            .and_then(|(recognized, loaded)| loaded.catalog.provider(&recognized.provider))
            .map(|entry| entry.models.keys().cloned().collect())
            .unwrap_or_default()
    })
}

/// 照推荐挑一个（`miyu_models::onboard::recommend`）：每个模型的资料照 `resolved` 和手头的资料查。
fn pick(
    data: &ModelData,
    resolved: &Resolved,
    provider: &Provider,
    models: &[String],
) -> Option<String> {
    data.with(|knowledge| {
        let offered: Vec<Offered> = models
            .iter()
            .map(|model| {
                let (facts, found) = facts(resolved, knowledge, provider, model);
                Offered::of(model, &facts, released(knowledge.catalog, &found))
            })
            .collect();
        recommend(&offered).map(str::to_string)
    })
}

/// 没成：哪一步、分类、原话，没有状态码。
fn failed(stage: Stage, class: ErrorClass, message: String) -> Probed {
    Probed::Failed {
        stage,
        error: CallError {
            class,
            message,
            status: None,
        },
    }
}
