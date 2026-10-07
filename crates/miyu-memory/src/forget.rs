//! `forget`（施工 R-3 中）：作废一条，原文还在记忆日志里，`memory_search` 带 `forgotten` 搜得到。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, FORGET, Progress, Running, Spec, Tool};

use crate::PACKAGE;
use crate::texts::{Texts, line, memory_id, said};

/// `forget`。
pub(crate) struct Forget {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    id: String,
    why: String,
}

impl Forget {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Forget, LoadError> {
        Ok(Forget {
            spec: load::spec(resources, PACKAGE, FORGET, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Forget {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.memory.as_ref() else {
                return texts.off();
            };
            let id = match memory_id(&args.id) {
                Ok(id) => id,
                Err(id) => return texts.no_such(&id),
            };
            match port.retire(id, args.why).await {
                Ok(()) => {
                    let id = id.to_string();
                    Done::ok(line(&texts.all().retired, &[("id", &id)]))
                        .said(said("forget/retired", &[("id", &id)]))
                }
                Err(refused) => texts.refused(&refused),
            }
        })
    }
}
