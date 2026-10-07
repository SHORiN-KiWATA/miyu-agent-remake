//! `todowrite`（`docs/blueprint/tools/todowrite.md`，施工 D-3）：她做多步的活时维护一张待办清单，头照着显示。每次整份换，
//! 照 Claude Code 的 TodoWrite、codex 的 `update_plan`、opencode 的 `todowrite`（2026-10-07 项目主人定）。访问类别是读：只改
//! 这个会话自己的清单，只读开着也能写。
//!
//! - 结果一句短话，不把清单原样回给她（回一遍就又是一份 token）。
//! - 换上的整份记成效果 `todo.written`，内核照它算当前的清单、交给头、压缩时写进检查点。
//! - 全部做完的清空（照 Claude Code）：效果是空列表，结果说清空了。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::{Todo, TodoStatus, TodoWritten};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, TODOWRITE, Tool};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// `todowrite`。
pub(crate) struct TodoWrite {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/todowrite/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    updated: Template,
    cleared: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    todos: Vec<TodoArg>,
}

/// 一项。状态只认三种：不认识的是参数不对。
#[derive(Deserialize)]
struct TodoArg {
    content: String,
    status: StatusArg,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusArg {
    Pending,
    InProgress,
    Completed,
}

impl TodoWrite {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<TodoWrite, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, TODOWRITE, name, fields);
        Ok(TodoWrite {
            spec: load::spec(resources, TODOWRITE, Access::Read)?,
            texts: Texts {
                common,
                updated: text("updated", &["done", "total"])?,
                cleared: text("cleared", &[])?,
            },
        })
    }
}

impl Tool for TodoWrite {
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
            let todos: Vec<Todo> = args.todos.into_iter().map(todo).collect();
            let done = todos
                .iter()
                .filter(|todo| todo.status == TodoStatus::Completed)
                .count();
            // 全部做完（连同空的）：清空。
            if done == todos.len() {
                // 做完的那一份带着（施工 D-3 补）：头让人看到最后一项打勾再收起。
                let effect = Effect::TodoWritten(TodoWritten {
                    todos: Vec::new(),
                    done: todos,
                });
                return Done::ok(say(&texts.cleared, &[]))
                    .said(said("todowrite/cleared"))
                    .effect(effect);
            }
            let (done, total) = (done.to_string(), todos.len().to_string());
            let text = say(&texts.updated, &[("done", &done), ("total", &total)]);
            Done::ok(text)
                .said(
                    said("todowrite/updated")
                        .with("done", done)
                        .with("total", total),
                )
                .effect(Effect::TodoWritten(TodoWritten {
                    todos,
                    done: Vec::new(),
                }))
        })
    }
}

/// 参数里的一项换成效果里的样子。
fn todo(arg: TodoArg) -> Todo {
    Todo {
        content: arg.content,
        status: match arg.status {
            StatusArg::Pending => TodoStatus::Pending,
            StatusArg::InProgress => TodoStatus::InProgress,
            StatusArg::Completed => TodoStatus::Completed,
        },
    }
}
