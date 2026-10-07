//! `memory_search`（施工 R-3 中）：搜记下的和以前的对话。记下的一条一行在前，以前的对话一条一行在后；什么都没有说一句。
//! 挑哪些（现在算数的、出处活着的、听众合的）是端口的事，这里只照端口交回的写。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, MEMORY_SEARCH, Progress, Running, Spec, Tool};

use crate::texts::{Texts, line, said};
use crate::{PACKAGE, TURN_CHARS};

/// `memory_search`。
pub(crate) struct Search {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default)]
    forgotten: bool,
}

impl Search {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Search, LoadError> {
        Ok(Search {
            spec: load::spec(resources, PACKAGE, MEMORY_SEARCH, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Search {
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
            let found = match port.search(args.query, args.forgotten).await {
                Ok(found) => found,
                Err(error) => return texts.refused(&miyu_tool::Refused::Failed(error)),
            };
            if found.memories.is_empty() && found.turns.is_empty() {
                return Done::ok(line(&all.nothing, &[])).said(said("memory_search/nothing", &[]));
            }
            let mut lines = Vec::new();
            for memory in &found.memories {
                let template = if memory.retired {
                    &all.memory_retired
                } else {
                    &all.memory
                };
                let (id, date) = (memory.id.to_string(), memory.at.local_date(call.offset));
                lines.push(line(
                    template,
                    &[
                        ("id", &id),
                        ("class", &memory.class),
                        ("date", &date),
                        ("text", &memory.text),
                    ],
                ));
            }
            for turn in &found.turns {
                let date = turn.at.local_date(call.offset);
                lines.push(line(
                    &all.turn,
                    &[
                        ("date", &date),
                        ("session", turn.session.short()),
                        ("text", &flat(&turn.text)),
                    ],
                ));
            }
            let counts = [
                ("memories", found.memories.len().to_string()),
                ("turns", found.turns.len().to_string()),
            ];
            let human = said(
                "memory_search/found",
                &[
                    ("memories", counts[0].1.as_str()),
                    ("turns", counts[1].1.as_str()),
                ],
            );
            Done::ok(lines.join("\n")).said(human)
        })
    }
}

/// 一段对话写成一行：换行（人的话和她的回答之间的空行）换成 ` / `，截到 [`TURN_CHARS`] 个字，截了的末尾接 `…`。
fn flat(text: &str) -> String {
    let joined = text
        .split('\n')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    match joined.char_indices().nth(TURN_CHARS) {
        Some((cut, _)) => format!("{}…", &joined[..cut]),
        None => joined,
    }
}
