//! `remember`（施工 R-3 中）：记一条；写了 `replaces` 的是改那一条。超长的、类不认识的、编号写错的不记，说一句。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_recall::CLASSES;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, Progress, REMEMBER, Remember as Remembering, Running, Spec, Tool};

use crate::texts::{Texts, line, memory_id, said};
use crate::{PACKAGE, TEXT_CHARS};

/// `remember`。
pub(crate) struct Remember {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    class: String,
    text: String,
    #[serde(default)]
    replaces: Option<String>,
}

impl Remember {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Remember, LoadError> {
        Ok(Remember {
            spec: load::spec(resources, PACKAGE, REMEMBER, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Remember {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let all = texts.all();
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.memory.as_ref() else {
                return texts.off();
            };
            if !CLASSES.contains(&args.class.as_str()) {
                return texts.error(
                    &all.unknown_class,
                    "remember/unknown-class",
                    &[("class", &args.class)],
                );
            }
            let chars = args.text.chars().count();
            if chars > TEXT_CHARS {
                let (chars, limit) = (chars.to_string(), TEXT_CHARS.to_string());
                return texts.error(
                    &all.too_long,
                    "remember/too-long",
                    &[("chars", &chars), ("limit", &limit)],
                );
            }
            // 「没传」的几种写法（空串、`null`）当没传：照基础系统的 `given`。
            let replaces = match args
                .replaces
                .filter(|old| !matches!(old.as_str(), "" | "null" | "undefined"))
            {
                None => None,
                Some(old) => match memory_id(&old) {
                    Ok(old) => Some(old),
                    Err(old) => return texts.no_such(&old),
                },
            };
            let remembering = Remembering {
                class: args.class,
                text: args.text,
                replaces,
            };
            match port.save(remembering).await {
                Ok(id) => {
                    let id = id.to_string();
                    match replaces {
                        None => Done::ok(line(&all.saved, &[("id", &id)]))
                            .said(said("remember/saved", &[("id", &id)])),
                        Some(old) => {
                            let fields = [("id", id.as_str()), ("old", &old.to_string())];
                            Done::ok(line(&all.replaced, &fields))
                                .said(said("remember/replaced", &fields))
                        }
                    }
                }
                Err(refused) => texts.refused(&refused),
            }
        })
    }
}
