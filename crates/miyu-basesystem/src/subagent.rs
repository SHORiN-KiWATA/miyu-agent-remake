//! `subagent`（`docs/blueprint/tools/subagent.md`，施工 7-5）：派一个子代理去做一件事。只声明 `description`（短标题）、
//! `prompt`（整段交代）；经 [`Call::agents`] 交给执行器，执行器照父会话抄好权限、目录，造子会话、把交代送进去，交回编号就
//! 返回，不等它做完。回报由子会话自己送来（7-6）。
//!
//! 施工 7-5 再补从 `agent` 改名：以前的名字照样认（[`Tool::formerly`]），输出那两句的目录和说法的编号照旧叫 `agent`
//! （[`SAYINGS`]）。挡位、人格、预设三个参数随配置和预设那一步（`agents.md`「还没有的」）。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::{JobKind, JobStarted};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, SUBAGENT, SUBAGENT_FORMERLY, Spec, Tool};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// 输出里那两句的目录（`software/basesystem/agent/`）：改名以后照旧叫 `agent`（施工 7-5 再补），和给人看的说法的编号
/// （`agent/started`、`agent/not-started`）一样。说法的编号记在日志里（`tool.result` 的 `human`），头照它找给人看的字，
/// 改了以前的日志就换不出字；这两句和说法同名，一起不动。
const SAYINGS: &str = "agent";

/// `subagent`。
pub(crate) struct Subagent {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/agent/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    started: Template,
    not_started: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    description: String,
    prompt: String,
}

impl Subagent {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读：派出去这一下什么都不改，子会话照父会话抄了
    /// 权限，改不改由它自己的权限管；读的调用连着的一起派，一步里调几次就同时派几个（`agents.md` 第一条第 4 条）。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Subagent, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, SAYINGS, name, fields);
        Ok(Subagent {
            spec: load::spec(resources, SUBAGENT, Access::Read)?,
            texts: Texts {
                common,
                started: text("started", &["job", "title"])?,
                not_started: text("not-started", &[])?,
            },
        })
    }
}

impl Tool for Subagent {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    /// 改名以前叫 `agent`：以前造的会话快照里冻着它，她照它调（施工 7-5 再补）。
    fn formerly(&self) -> &'static [&'static str] {
        &[SUBAGENT_FORMERLY]
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let not_started =
                || Done::error(say(&texts.not_started, &[])).said(said("agent/not-started"));
            let Some(port) = call.agents else {
                return not_started();
            };
            let Ok(spawned) = port.spawn(&args.description, &args.prompt).await else {
                return not_started();
            };
            let job = spawned.job.to_string();
            let title = args.description.as_str();
            Done::ok(say(&texts.started, &[("job", &job), ("title", title)]))
                .said(said("agent/started").with("job", &job).with("title", title))
                .effect(Effect::JobStarted(JobStarted {
                    job: spawned.job,
                    what: JobKind::Agent,
                    title: args.description,
                    session: Some(spawned.session),
                }))
        })
    }
}
