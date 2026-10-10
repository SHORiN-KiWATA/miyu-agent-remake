//! 会话状态里的会话树（施工 9-8 补下，`docs/blueprint/view.md`「会话状态」）：这个会话派出去的子代理各自那一支有没有在跑、
//! 自己又派了几个、那一支在跑的一共几个、那一支用了多少；整个会话在跑的一共几个；连子代理一起累计用了多少。
//!
//! - 只看载入了的会话：在跑的子代理一定载入着；没载入的那一支算没有在跑的。往下最多走 [`DEPTH`] 层。
//! - 用量照账本连子会话一起查（`usage::branch`）：没载入的、做完的子代理也查得到。
//! - 什么时候重算由视图流定：这个会话的任务表变了；有子代理时别的会话动了（一轮开始、空下来）。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_kernel::event::JobKind;
use miyu_kernel::id::{JobId, SessionId};
use miyu_store::usage::Total;
use miyu_view::JobRow;

use crate::Core;
use crate::usage;

/// 往下最多走几层：子代理派孙代理，再深就不数了（防着哪里成了环）。
const DEPTH: usize = 8;

/// 一支：一个子代理那一路。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Branch {
    /// 子会话这会儿有没有在跑一轮。
    pub(crate) busy: bool,
    /// 它自己派出、还在跑的有几个。
    pub(crate) spawned: u64,
    /// 它那一支在跑的一共几个（不算它自己）。
    pub(crate) running_deep: u64,
    /// 它那一支一共用了多少；账本读不了的没有。
    pub(crate) total: Option<Total>,
}

/// 量出来的会话树。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Tree {
    /// 子代理任务到它那一支。
    pub(crate) branches: BTreeMap<JobId, Branch>,
    /// 这个会话在跑的任务一共几个（连子孙的）。
    pub(crate) running_deep: u64,
    /// 子代理那几支加起来用了多少。
    pub(crate) kids: Option<Total>,
}

/// 照这个会话的任务表量一遍。
pub(crate) async fn measure(core: &Core, rows: &[JobRow]) -> Tree {
    let mut tree = Tree::default();
    let mut kids: Option<Total> = None;
    let mut running = 0;
    for row in rows {
        if row.state.running() {
            running += 1;
        }
        let (JobKind::Agent, Some(session)) = (&row.what, &row.session) else {
            continue;
        };
        let busy = match core.sessions.loaded(session).await {
            Some(handle) => handle.busy(),
            None => false,
        };
        let (spawned, deep) = walk(core, session, DEPTH).await;
        let total = usage::branch(core, session).await;
        if let Some(total) = &total {
            kids = Some(match kids {
                Some(sum) => usage::plus(sum, total),
                None => total.clone(),
            });
        }
        running += deep;
        tree.branches.insert(
            row.job.clone(),
            Branch {
                busy,
                spawned,
                running_deep: deep,
                total,
            },
        );
    }
    tree.running_deep = running;
    tree.kids = kids;
    tree
}

/// 会话 `session` 那一支：它自己派出、还在跑的有几个，连子孙在跑的一共几个。没载入的算零。
async fn walk(core: &Core, session: &SessionId, depth: usize) -> (u64, u64) {
    let Some(handle) = core.sessions.loaded(session).await else {
        return (0, 0);
    };
    let Ok(current) = handle.current().await else {
        return (0, 0);
    };
    let direct = u64::try_from(current.jobs.len()).unwrap_or(u64::MAX);
    let mut deep = direct;
    if depth > 1 {
        for job in &current.jobs {
            let (JobKind::Agent, Some(child)) = (&job.what, &job.session) else {
                continue;
            };
            let (_, below) = Box::pin(walk(core, child, depth - 1)).await;
            deep += below;
        }
    }
    (direct, deep)
}

/// 把会话树并进整份状态：子代理任务多 `busy`、`spawned`、`running_deep`、`usage`；整份多 `running_deep`、连子代理一起的
/// `usage_tree`（自己的 `own` 加上子代理那几支）。
pub(crate) fn merge(core: &Core, status: &mut Value, tree: &Tree, own: Option<Total>) {
    if let Some(jobs) = status.get_mut("jobs").and_then(Value::as_array_mut) {
        for job in jobs {
            let Some(branch) = job
                .get("job")
                .and_then(Value::as_str)
                .and_then(|id| JobId::parse(id).ok())
                .and_then(|id| tree.branches.get(&id))
            else {
                continue;
            };
            job["busy"] = json!(branch.busy);
            job["spawned"] = json!(branch.spawned);
            job["running_deep"] = json!(branch.running_deep);
            if let Some(total) = &branch.total {
                job["usage"] = usage::total_row(core, total.clone());
            }
        }
    }
    status["running_deep"] = json!(tree.running_deep);
    if let Some(own) = own {
        let all = match &tree.kids {
            Some(kids) => usage::plus(own, kids),
            None => own,
        };
        status["usage_tree"] = usage::total_row(core, all);
    }
}
