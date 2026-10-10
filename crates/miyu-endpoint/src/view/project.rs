//! 把一页日志算成条目（施工 9-8 中，`docs/blueprint/view.md`「`view.page`」）：投影要的字、改了多少行的端口，在这里照
//! 资源目录和属主的 blob 接上。投影本身是纯逻辑，在 `miyu-view`。

use std::sync::Arc;

use miyu_kernel::event::{Event, Said};
use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_view::{Entry, Face, Kinds, Lines, Projector, Texts, Words};

use crate::diffs;

/// 工具的分类：资源目录里的 `core/view.json`。
const KINDS: &str = "core/view.json";

/// 读不成投影要的字：哪一份、为什么，记进运行日志。
#[derive(Debug)]
pub(super) struct Unreadable(pub(super) String);

/// 一种语言的字：照 [`Human`] 交给投影。
struct HumanWords(Human);

impl Words for HumanWords {
    fn face(&self, name: &str) -> Option<Face> {
        self.0.tool(name).map(|face| Face {
            name: face.name.clone(),
            subject: face.subject.clone(),
        })
    }

    fn say(&self, said: &Said) -> Option<String> {
        self.0.say(said)
    }
}

/// 改了多少行：照属主的 blob 算，和 `view.detail` 的差异同一个算法。
struct BlobLines(Blobs);

impl Lines for BlobLines {
    fn count(&self, before: Option<&ContentHash>, after: &ContentHash) -> Option<(u64, u64)> {
        let then = diffs::side(&self.0, before).ok()?;
        let now = diffs::side(&self.0, Some(after)).ok()?;
        let whole = diffs::unified(&then, &now).ok()?;
        Some((
            u64::try_from(whole.added).ok()?,
            u64::try_from(whole.removed).ok()?,
        ))
    }
}

/// 照资源目录 `resources` 读投影要的字：`language` 一份、英文一份，工具的分类，这台机器的家目录。
pub(super) fn texts(resources: &ResourceRoot, language: &str) -> Result<Texts, Unreadable> {
    let load = |language: &str| {
        Human::load(resources, language)
            .map(HumanWords)
            .map_err(|error| Unreadable(error.to_string()))
    };
    let path = resources.path().join(KINDS);
    let kinds: Kinds = std::fs::read_to_string(&path)
        .map_err(|error| Unreadable(format!("{}: {error}", path.display())))
        .and_then(|text| {
            serde_json::from_str(&text)
                .map_err(|error| Unreadable(format!("{}: {error}", path.display())))
        })?;
    Ok(Texts {
        local: Box::new(load(language)?),
        english: Box::new(load("en")?),
        kinds,
        home: std::env::home_dir().map(|home| home.to_string_lossy().into_owned()),
    })
}

/// 这一页的条目：先照 `earlier`（切点前的日志）学派出去的任务，再把这一页的事件 `page` 喂一遍，取最后的样子。
pub(super) fn entries(
    texts: Texts,
    blobs: Blobs,
    earlier: &[Event],
    page: &[&Event],
) -> Vec<Entry> {
    let mut projector = Projector::new(Arc::new(texts), Some(Arc::new(BlobLines(blobs))));
    projector.learn(earlier);
    for event in page {
        projector.event(event);
    }
    projector.entries().to_vec()
}
