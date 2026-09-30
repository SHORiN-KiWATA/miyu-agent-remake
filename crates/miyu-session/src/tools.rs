//! 执行工具的端口（`02-内核.md` 第四节「执行工具」「停下工具」，`05-内核接口.md` 第六节「执行这一步」，
//! 施工 4-2）：照工具名在目录里找到那一件，一件一个任务地跑，量用时。叫停有两种：「叫它停」只举这次调用的旗，
//! 工具自己停在改之前或者做完（施工 4-9 再补一）；「掐掉」掐掉那个任务。回报送回 actor 的收件箱，由它写成内核的
//! 输入。
//!
//! 工具报的效果（施工 4-6 上）：跑完以后在阻塞线程里把改前改后存成 blob，再送回来；送回来的先记下她看过的，
//! 再交进内核。她看过的交给以后每一次调用。
//!
//! 沙盒（施工 5-4 上）：这台机器上的沙盒能用的，每次调用照派出去那一刻实际生效的那一级写上沙盒（`crate::sandbox`），
//! 在跑它的任务里、阻塞线程上算；写不成的不跑，照崩了算。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::Instrument;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Effect, Permission, Restored};
use miyu_kernel::id::{CallId, ContentHash, JobId};
use miyu_kernel::session::{Input, Reread, Step, Subagent};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::RunTexts;
use miyu_store::blob::Blobs;
use miyu_tool::{Call, Catalog, Done, JobPort, Log, Progress, Seen, Stop};

use crate::TARGET;
use crate::agents::Agents;
use crate::blocking::blocking;
use crate::effects;
use crate::job_ids::JobIds;
use crate::lines::millis;
use crate::messages;
use crate::pictures;
use crate::port::Back;
use crate::sandbox::{Sandbox, SandboxCache};

/// 执行工具要的：工具目录、替工具写的两句、系统的家目录（施工 4-4 上，交给每次调用）。
pub(crate) struct ToolKit {
    /// 工具目录。
    pub(crate) catalog: Catalog,
    /// 替工具写的两句。
    pub(crate) texts: RunTexts,
    /// 系统的家目录。
    pub(crate) home: Option<PathBuf>,
    /// Miyu 的数据根：交给工具，往下走目录的走到这里跳过（施工 4-4 下）。
    pub(crate) data_root: PathBuf,
    /// 这个会话的 blob：效果里改前改后的内容存进这里（施工 4-6 上）。
    pub(crate) blobs: Blobs,
    /// 她看过的文件：新会话是空的，载入的从日志里重建（施工 4-6 上）。
    pub(crate) seen: Seen,
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。
    pub(crate) sandbox: Option<PathBuf>,
    /// 沙盒的缓存：工具链的缓存用沙盒自己的一份（施工 5-4 下）。核心算不出缓存目录的没有。
    pub(crate) sandbox_cache: Option<SandboxCache>,
    /// 这个会话日志的只读入口：交给每次调用，`history` 用（施工 6-4）。
    pub(crate) log: Log,
    /// 会话的时区：开会话时的环境里的（施工 6-4）。
    pub(crate) offset: UtcOffset,
    /// 这个会话的任务编号（施工 7-5）：从日志里用过的最大编号往下数，几次调用一起跑的各领各的。
    pub(crate) job_ids: Arc<JobIds>,
    /// 派子代理要的（施工 7-5）：会话表交进来了端口才有。
    pub(crate) agents: Option<Arc<Agents>>,
}

/// 执行工具的端口：一个会话一份。
pub(crate) struct Tools {
    catalog: Catalog,
    texts: RunTexts,
    home: Option<PathBuf>,
    data_root: PathBuf,
    blobs: Blobs,
    /// 她看过的文件：交给每一次调用，工具报了效果就跟着改。
    seen: Arc<Seen>,
    /// 给每次调用写沙盒的：这台机器上的沙盒能用才有（施工 5-4 上）。
    sandbox: Option<Sandbox>,
    /// 这个会话日志的只读入口（施工 6-4）。
    log: Log,
    /// 会话现在的时区：头报上来换了跟着换（施工 6-4）。
    offset: UtcOffset,
    /// 任务编号（施工 7-5）。
    job_ids: Arc<JobIds>,
    /// 派子代理要的（施工 7-5）：交给每一次调用一个照这一轮抄好的端口。
    agents: Option<Arc<Agents>>,
    /// 在跑的调用：掐掉它的那一头、它的旗、开始跑的那一刻、工具名。
    running: BTreeMap<CallId, Running>,
    backs: mpsc::UnboundedSender<Back>,
}

/// 一次在跑的调用。
struct Running {
    task: AbortHandle,
    /// 交给工具的那面旗。
    stop: Stop,
    /// 叫它停过：还没交回来就被掐掉的，改动可能不留效果，记一行 `WARN`。
    stopping: bool,
    started: Instant,
    name: String,
}

/// 跑工具的任务送回来的。
#[derive(Debug)]
pub(crate) enum ToolBack {
    /// 执行中的一段输出。
    Progress { call_id: CallId, text: String },
    /// 跑完了：工具交回的，和它报的效果（改前改后已经存成了 blob）。
    Done {
        call_id: CallId,
        done: Done,
        effects: Vec<Effect>,
    },
    /// 工具自己崩了（panic）。
    Crashed { call_id: CallId },
}

/// 派一次调用要的：内核的「执行一次工具调用」动作里的几样（`docs/blueprint/kernel/session.md`）。
#[derive(Debug)]
pub(crate) struct Dispatch {
    /// 哪一次调用。
    pub(crate) call_id: CallId,
    /// 工具名。
    pub(crate) name: String,
    /// 修正过的参数：一个 JSON 对象的原文。
    pub(crate) args: String,
    /// 这一轮的工作目录。
    pub(crate) cwd: String,
    /// 这一轮加进来的目录（施工 5-10 上）：沙盒照工作区放行。
    pub(crate) dirs: Vec<String>,
    /// 派出去那一刻实际生效的那一级：沙盒照它写规格。
    pub(crate) permission: Permission,
    /// 这次调用的任务端口（施工 7-3）：`shell` 把后台命令交给它。
    pub(crate) jobs: Arc<dyn JobPort>,
    /// 这个会话这一刻派出去的子代理（施工 7-7）：`message_agent` 照它认 `to`。
    pub(crate) subagents: BTreeMap<JobId, Subagent>,
}

impl Tools {
    /// 照 `kit` 跑，回报送进 `backs`。
    pub(crate) fn new(kit: ToolKit, backs: mpsc::UnboundedSender<Back>) -> Tools {
        let sandbox = kit.sandbox.map(|helper| {
            Sandbox::new(
                helper,
                kit.home.clone(),
                kit.data_root.clone(),
                kit.sandbox_cache,
            )
        });
        Tools {
            catalog: kit.catalog,
            texts: kit.texts,
            home: kit.home,
            data_root: kit.data_root,
            blobs: kit.blobs,
            seen: Arc::new(kit.seen),
            sandbox,
            log: kit.log,
            offset: kit.offset,
            job_ids: kit.job_ids,
            agents: kit.agents,
            running: BTreeMap::new(),
            backs,
        }
    }

    /// 会话的时区换成 `offset`：头报上来的环境换了（施工 6-4）。以后派出去的调用照它。
    pub(crate) fn locate(&mut self, offset: UtcOffset) {
        self.offset = offset;
    }

    /// 她看过的文件换成 `seen`：撤销、恢复以后照日志重算的（施工 4-7 上）。在跑的调用拿着的是原来那一份。
    pub(crate) fn see(&mut self, seen: Seen) {
        self.seen = Arc::new(seen);
    }

    /// 改回文件（施工 4-7 上）：照这几步在阻塞线程里做完，一步一项交回结局。写回的内容从这个会话的 blob 里取，
    /// 移进回收站照交给工具的那个家目录。
    pub(crate) async fn restore(&self, steps: Vec<Step>) -> Vec<Restored> {
        let blobs = self.blobs.clone();
        let home = self.home.clone();
        blocking(move || crate::restore::restore(&steps, &blobs, home.as_deref())).await
    }

    /// 压完重读（施工 6-5）：在阻塞线程里一个一个读，读到的存进这个会话的 blob。
    pub(crate) async fn reread(&self, paths: Vec<String>, limit: u64) -> Vec<Reread> {
        let blobs = self.blobs.clone();
        blocking(move || crate::reread::reread(&paths, limit, &blobs)).await
    }

    /// 取回原文（施工 6-9）：在阻塞线程里照 blob 读这个会话的 blob，读不出来的、不是 UTF-8 的不交。
    pub(crate) async fn recall(&self, blobs: Vec<ContentHash>) -> BTreeMap<ContentHash, String> {
        let store = self.blobs.clone();
        blocking(move || crate::reread::recall(&blobs, &store)).await
    }

    /// 执行一次调用：在自己的任务里跑，马上返回。目录里没有这件工具的，不派，当场交回出错的结果。沙盒照派出去
    /// 那一刻实际生效的那一级、这一轮加进来的目录写。
    pub(crate) fn run(&mut self, at: Timestamp, dispatch: Dispatch) -> Option<Input> {
        let Dispatch {
            call_id,
            name,
            args,
            cwd,
            dirs,
            permission,
            jobs,
            subagents,
        } = dispatch;
        let stop = Stop::default();
        // 派子代理的端口照这一轮的目录、这一刻的权限抄（施工 7-5）：沙盒下面照样要用它们。
        let agents = self.agents.as_ref().map(|agents| {
            let ids = Arc::clone(&self.job_ids);
            agents.for_call(ids, cwd.clone(), dirs.clone(), permission.clone())
        });
        // 留言的端口照这一刻派出去的子代理抄（施工 7-7）。
        let messages = self
            .agents
            .as_ref()
            .map(|agents| messages::for_call(agents, call_id, subagents));
        let call = Call {
            args,
            cwd,
            home: self.home.clone(),
            data_root: Some(self.data_root.clone()),
            seen: Arc::clone(&self.seen),
            stop: stop.clone(),
            sandbox: None,
            log: Some(self.log.clone()),
            offset: self.offset,
            agents,
            messages,
            jobs: Some(jobs),
        };
        let call_text = call_id.to_string();
        let Some(tool) = self.catalog.get(&name).cloned() else {
            tracing::warn!(target: TARGET, call = call_text.as_str(), tool = name.as_str(), "unavailable");
            let worded = self.texts.unavailable(&name);
            return Some(Input::ToolDone {
                at,
                call_id,
                error: true,
                blocks: text(worded.text),
                duration_ms: None,
                human: worded.said,
                effects: Vec::new(),
                stopped: false,
            });
        };
        tracing::info!(target: TARGET, call = call_text.as_str(), tool = name.as_str(), "running");
        let progress = {
            let backs = self.backs.clone();
            Progress::new(move |text| send(&backs, ToolBack::Progress { call_id, text }))
        };
        let span = tracing::Span::current();
        let sandbox = self.sandbox.clone();
        let inner = tokio::spawn(
            async move {
                let call = confine(call, sandbox, permission, dirs, call_id).await?;
                Some(tool.run(call, progress).await)
            }
            .instrument(span.clone()),
        );
        let task = inner.abort_handle();
        let backs = self.backs.clone();
        let blobs = self.blobs.clone();
        // 看着它的任务、存 blob 的阻塞线程也带着会话的 span：存不进去的那一行有会话编号（施工 4-9 再补四上）。
        let watching = span.clone();
        tokio::spawn(
            async move {
                match inner.await {
                    Ok(Some(mut done)) => {
                        // 改前改后、交回的图片先落 blob，再送回去写引用它们的事件（07 第四节；图片施工 4-13）。
                        let reported = std::mem::take(&mut done.effects);
                        let images = std::mem::take(&mut done.images);
                        let stored = tokio::task::spawn_blocking(move || {
                            span.in_scope(|| {
                                let effects = effects::store(&blobs, reported);
                                pictures::store(&blobs, images).map(|images| (effects, images))
                            })
                        })
                        .await;
                        match stored {
                            Ok(Some((effects, images))) => {
                                done.blocks.extend(images);
                                send(
                                    &backs,
                                    ToolBack::Done {
                                        call_id,
                                        done,
                                        effects,
                                    },
                                );
                            }
                            // 图片存不下来、存 blob 的线程 panic 了：照崩了算。
                            Ok(None) | Err(_) => send(&backs, ToolBack::Crashed { call_id }),
                        }
                    }
                    // 沙盒写不成，没跑；工具 panic 了。
                    Ok(None) => send(&backs, ToolBack::Crashed { call_id }),
                    Err(error) if error.is_panic() => send(&backs, ToolBack::Crashed { call_id }),
                    // 叫停了：没人要了。
                    Err(_) => {}
                }
            }
            .instrument(watching),
        );
        self.running.insert(
            call_id,
            Running {
                task,
                stop,
                stopping: false,
                started: Instant::now(),
                name,
            },
        );
        None
    }

    /// 叫它停（施工 4-9 再补一）：举起这次调用的旗，不掐任务。工具看旗，停在改之前或者做完，照常送回来。不在跑的，
    /// 什么都不做。
    pub(crate) fn stop(&mut self, call_id: CallId) {
        if let Some(running) = self.running.get_mut(&call_id) {
            running.stop.raise();
            running.stopping = true;
        }
    }

    /// 掐掉一次在跑的调用：举旗，掐掉跑它的任务。之后什么都不再报。叫它停过、还没交回来的，多记一行 `WARN`：它要是
    /// 后来改完了，这次改动不留效果。
    pub(crate) fn cancel(&mut self, call_id: CallId) {
        let Some(running) = self.running.remove(&call_id) else {
            return;
        };
        running.stop.raise();
        running.task.abort();
        let call = call_id.to_string();
        tracing::info!(
            target: TARGET,
            call = call.as_str(),
            took_ms = millis(running.started.elapsed()),
            "stopped"
        );
        if running.stopping {
            tracing::warn!(target: TARGET, call = call.as_str(), "cancelled while stopping");
        }
    }

    /// 跑工具的任务送回来的，写成内核的输入。不在跑的（已经叫停了的）不理。
    pub(crate) fn back(&mut self, at: Timestamp, back: ToolBack) -> Option<Input> {
        match back {
            ToolBack::Progress { call_id, text } => self
                .running
                .contains_key(&call_id)
                .then_some(Input::ToolProgress { at, call_id, text }),
            ToolBack::Done {
                call_id,
                done,
                effects,
            } => {
                let running = self.running.remove(&call_id)?;
                effects::saw(Arc::make_mut(&mut self.seen), &effects);
                let took_ms = millis(running.started.elapsed());
                tracing::info!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    took_ms,
                    error = done.error.then_some(true),
                    stopped = done.stopped.then_some(true),
                    "ran"
                );
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: done.error,
                    blocks: done.blocks,
                    duration_ms: Some(took_ms),
                    human: done.human,
                    effects,
                    stopped: done.stopped,
                })
            }
            ToolBack::Crashed { call_id } => {
                let running = self.running.remove(&call_id)?;
                let took_ms = millis(running.started.elapsed());
                tracing::error!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    tool = running.name.as_str(),
                    took_ms,
                    "crashed"
                );
                let worded = self.texts.crashed(&running.name);
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: true,
                    blocks: text(worded.text),
                    duration_ms: Some(took_ms),
                    human: worded.said,
                    effects: Vec::new(),
                    stopped: false,
                })
            }
        }
    }
}

/// 会话停了，在跑的工具都掐掉：结果没人要了。
impl Drop for Tools {
    fn drop(&mut self) {
        for running in self.running.values() {
            running.stop.raise();
            running.task.abort();
        }
    }
}

/// 照实际生效的那一级 `permission` 给调用写上沙盒（第 1a 条）：这台机器上的沙盒用不了的，照原样。在阻塞线程里算。
/// 写不成的不跑：记一行 `ERROR`，交回空的，照崩了算，不在沙盒外跑。
async fn confine(
    mut call: Call,
    sandbox: Option<Sandbox>,
    permission: Permission,
    dirs: Vec<String>,
    call_id: CallId,
) -> Option<Call> {
    let Some(sandbox) = sandbox else {
        return Some(call);
    };
    let cwd = call.cwd.clone();
    match blocking(move || sandbox.for_call(&permission, &cwd, &dirs)).await {
        Ok(sandboxed) => {
            call.sandbox = sandboxed.map(Arc::new);
            Some(call)
        }
        Err(error) => {
            tracing::error!(
                target: TARGET,
                call = call_id.to_string().as_str(),
                error = %error,
                "sandbox not set up"
            );
            None
        }
    }
}

/// 一段字的内容块。
fn text(text: String) -> Vec<Block> {
    vec![Block::Text(Text { text })]
}

/// 送回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：工具的回报没人要了，丢掉"
)]
fn send(backs: &mpsc::UnboundedSender<Back>, back: ToolBack) {
    let _ = backs.send(Back::Tool(back));
}
