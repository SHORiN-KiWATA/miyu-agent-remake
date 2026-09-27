//! 测试用的假工具（施工 4-2）：规格照给的，执行照剧本。只在 `testkit` 开关打开时编进去：工具目录自己的
//! 测试、会话和协议端点的测试都用它，在各自的 dev-dependencies 里打开（照会话的剧本端口，施工 3-8 上）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use tokio::sync::Barrier;

use crate::{Call, Done, Progress, Running, Spec, Tool};

/// 假工具跑起来做什么。
#[derive(Debug, Clone)]
pub enum Act {
    /// 回一句成功：工作目录和参数原文，写成 `cwd=<目录> args=<参数>`。
    Echo,
    /// 回一句出错。
    Fails(&'static str),
    /// 先推这几段输出，再回一句成功 `pushed`。
    Pushes(&'static [&'static str]),
    /// 停住不回，等被叫停（丢掉）。
    Holds,
    /// 等凑齐一起跑的（`Barrier` 的人数）再回一句成功 `met`：一起派的才走得完。
    Meets(Arc<Barrier>),
    /// 工具自己的 bug：一跑就 panic。
    Panics,
}

/// 一件假工具。
#[derive(Debug)]
pub struct Fake {
    spec: Spec,
    act: Act,
    calls: Mutex<Vec<Call>>,
    dropped: Arc<AtomicUsize>,
}

impl Fake {
    /// 叫 `name` 的假工具：参数格式是 `{"type":"object"}`，说明是 `The <name> tool.`。
    ///
    /// # Panics
    ///
    /// 实际不会：参数格式是写死的 JSON。
    pub fn new(name: &str, access: Access, act: Act) -> Arc<Fake> {
        Fake::with_parameters(name, access, r#"{"type":"object"}"#, act)
    }

    /// 参数格式照 `parameters` 的原文。
    ///
    /// # Panics
    ///
    /// `parameters` 不是 JSON。
    pub fn with_parameters(name: &str, access: Access, parameters: &str, act: Act) -> Arc<Fake> {
        Fake::from_spec(
            Spec {
                name: name.to_string(),
                description: format!("The {name} tool."),
                parameters: serde_json::from_str::<RawJson>(parameters).expect("参数格式是 JSON"),
                access,
            },
            act,
        )
    }

    /// 规格照 `spec`。
    pub fn from_spec(spec: Spec, act: Act) -> Arc<Fake> {
        Arc::new(Fake {
            spec,
            act,
            calls: Mutex::new(Vec::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// 交给它的每一次调用，照先后。
    pub fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 有几次没跑完就被丢掉了：叫停了的。
    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::Acquire)
    }
}

/// 跑完以前被丢掉，就数一次。
struct Guard {
    dropped: Arc<AtomicUsize>,
    finished: bool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            self.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }
}

impl Tool for Fake {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call.clone());
        let act = self.act.clone();
        let guard = Guard {
            dropped: Arc::clone(&self.dropped),
            finished: false,
        };
        Box::pin(async move {
            // 整个挪进来：只写 `guard.finished` 的话，闭包只捕获那一格，守卫在 `run` 返回时就被丢掉了。
            let mut guard = guard;
            let done = match act {
                Act::Echo => Done::ok(format!("cwd={} args={}", call.cwd, call.args)),
                Act::Fails(text) => Done::error(text),
                Act::Pushes(pieces) => {
                    for piece in pieces {
                        progress.push(*piece);
                    }
                    Done::ok("pushed")
                }
                Act::Holds => std::future::pending::<Done>().await,
                Act::Meets(barrier) => {
                    barrier.wait().await;
                    Done::ok("met")
                }
                Act::Panics => panic!("假工具自己的 bug"),
            };
            guard.finished = true;
            done
        })
    }
}
