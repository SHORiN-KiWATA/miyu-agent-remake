//! `jobs`（`docs/blueprint/tools/jobs.md`，施工 7-4）：看她派出去的后台命令和子代理、读输出、停掉。经 [`Call::jobs`] 交给
//! 执行器：列出来的照日志算，后台命令的输出从会话目录、blob 读，子代理那一头执行器经会话表去看、去停。
//!
//! 说明里写明做完会自己报、不用轮询（旧版实测，`26-提示词.md` 附录）。

mod page;

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::id::JobId;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, JobError, JobPort, Listed, Output, Progress, Running, Spec, Tool};

use crate::blocking::blocking;
use crate::common::{Common, given, said};
use crate::load::{self, LoadError, say};
use page::Page;

/// 工具名。
const NAME: &str = "jobs";

/// `jobs`。
pub(crate) struct Jobs {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/jobs/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    listed: Template,
    none: Template,
    unknown: Template,
    ended: Template,
    stopped: Template,
    more: Template,
    past_end: Template,
    empty: Template,
    running: Template,
    using: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    action: Action,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    offset: Option<u64>,
}

/// 三个动作。
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Action {
    List,
    Output,
    Stop,
}

impl Jobs {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读：列、读什么都不改；停掉的是她自己派出去的，
    /// 不碰文件，只读的时候也停得了（`tools/jobs.md`）。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Jobs, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, NAME, name, fields);
        Ok(Jobs {
            spec: load::spec(resources, NAME, Access::Read)?,
            texts: Texts {
                common,
                listed: text("listed", &["job", "what", "title", "status", "ms"])?,
                none: text("none", &[])?,
                unknown: text("unknown", &["id"])?,
                ended: text("ended", &["job"])?,
                stopped: text("stopped", &["job"])?,
                more: text("more", &["from", "to", "total", "next"])?,
                past_end: text("past-end", &["total", "offset"])?,
                empty: text("empty", &[])?,
                running: text("running", &["job"])?,
                using: text("using", &["job", "tools"])?,
            },
        })
    }
}

impl Tool for Jobs {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let port = call.jobs.clone();
            if let Action::List = args.action {
                let listed = port.as_deref().map(JobPort::list).unwrap_or_default();
                return texts.list(&listed);
            }
            // 读、停都要编号：没给的参数不对；不合编号写法的、没有任务端口的（会话外面的调用），照没有这个任务。
            let Some(id) = given(args.id) else {
                return texts.common.bad_args(&"missing field `id`");
            };
            let (Some(port), Ok(job)) = (port, JobId::parse(&id)) else {
                return texts.unknown(&id);
            };
            if let Action::Output = args.action {
                return match port.output(job.clone()).await {
                    Ok(output) => texts.output(call, job, output, args.offset).await,
                    Err(_) => texts.unknown(&id),
                };
            }
            texts.stop(port.stop(job.clone()).await, job, &id)
        })
    }
}

impl Texts {
    /// 停掉了、没有这个任务、已经结束了。
    fn stop(&self, stopped: Result<(), JobError>, job: JobId, id: &str) -> Done {
        match stopped {
            Ok(()) => {
                let job = job.to_string();
                Done::ok(say(&self.stopped, &[("job", &job)]))
                    .said(said("jobs/stopped").with("job", job))
            }
            Err(JobError::Unknown) => self.unknown(id),
            Err(JobError::Ended) => {
                let job = job.to_string();
                Done::error(say(&self.ended, &[("job", &job)]))
                    .said(said("jobs/ended").with("job", job))
            }
        }
    }

    /// 列出来的，一个一行；一个都没有的说一句。
    fn list(&self, listed: &[Listed]) -> Done {
        if listed.is_empty() {
            return Done::ok(say(&self.none, &[])).said(said("jobs/none"));
        }
        let text: String = listed
            .iter()
            .map(|listed| {
                let job = listed.job.to_string();
                let ms = listed.took_ms.to_string();
                let status = listed.ended.as_deref().unwrap_or("running");
                say(
                    &self.listed,
                    &[
                        ("job", &job),
                        ("what", listed.what.as_str()),
                        ("title", &listed.title),
                        ("status", status),
                        ("ms", &ms),
                    ],
                )
            })
            .collect();
        Done::ok(text).said(said("jobs/listed").with("count", listed.len().to_string()))
    }

    /// 没有这个任务：`id` 是她给的原样。
    fn unknown(&self, id: &str) -> Done {
        Done::error(say(&self.unknown, &[("id", id)])).said(said("jobs/unknown").with("id", id))
    }

    /// 读到的输出，从第 `offset` 行起分一页；还在跑的末尾说一句，子代理这一步在跑什么也说。
    async fn output(&self, call: Call, job: JobId, output: Output, offset: Option<u64>) -> Done {
        let Output {
            text,
            running,
            doing,
            ..
        } = output;
        let offset = offset.filter(|offset| *offset > 0).unwrap_or(1);
        let page = match text {
            Some(text) => blocking(call.stop, move |_| Page::read(text, offset)).await,
            None => Page::default(),
        };
        let job = job.to_string();
        let mut shown = match page.lines {
            _ if page.total == 0 => say(&self.empty, &[]),
            None => {
                let total = page.total.to_string();
                let offset = offset.to_string();
                say(&self.past_end, &[("total", &total), ("offset", &offset)])
            }
            Some((from, to)) => {
                let mut shown = page.text;
                if to < page.total {
                    let [from, to, total, next] =
                        [from, to, page.total, to + 1].map(|n| n.to_string());
                    shown.push_str(&say(
                        &self.more,
                        &[
                            ("from", &from),
                            ("to", &to),
                            ("total", &total),
                            ("next", &next),
                        ],
                    ));
                }
                shown
            }
        };
        if running {
            shown.push_str(&match doing.is_empty() {
                true => say(&self.running, &[("job", &job)]),
                false => say(&self.using, &[("job", &job), ("tools", &doing.join(", "))]),
            });
        }
        Done::ok(shown).said(said("jobs/output").with("job", job))
    }
}
