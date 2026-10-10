//! 软件包列表的推送（施工 F-8 三补，`docs/blueprint/protocol.md`「软件包列表的推送」）：核心重读清单的前后，照英文各算一份每个
//! 包在列表里的样子，多了的、少了的、变了的各报一声「这个包变了」；订阅着的连接照这一刻、照自己的语言算那一项推出去
//! （`subscriptions/listed.rs`）。装、卸、装回、关掉出厂的内置包都重读清单；扩展开关、状态变了另由扩展那边报。

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use serde_json::Value;
use tokio::sync::broadcast;

use super::{listed, packages, words};
use crate::Core;

/// 广播里最多攒几条没收的：一次装卸变的包没几个。
const CHANGES: usize = 64;

/// 「哪个包变了」的广播。
#[derive(Debug)]
pub(crate) struct Changes(broadcast::Sender<String>);

impl Default for Changes {
    fn default() -> Changes {
        Changes(broadcast::channel(CHANGES).0)
    }
}

impl Changes {
    /// 收「哪个包变了」的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.0.subscribe()
    }

    /// 广播包 `id` 变了。
    fn notify(&self, id: &str) {
        if self.0.send(id.to_string()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}

/// 每个包这时在列表里的样子，照编号：照英文算，只用来比变没变。英文的字读不出来的是空的（前后一比，后面的全算变了）。
///
/// 核心手里装着的照手里的算，卸掉了的出厂包照磁盘上记的那一笔：重读以前算的那一份，磁盘上已经记上了「卸掉」、手里还装着，
/// 照手里的（同 [`super::entry`] 的先后），不然前后一样、卸掉的那一下推不出去。
pub(super) fn snapshot(core: &Core) -> BTreeMap<String, Value> {
    let Ok(words) = words(core, "en") else {
        return BTreeMap::new();
    };
    let places = packages(core);
    let mut items: BTreeMap<String, Value> = core
        .packages()
        .iter()
        .map(|found| {
            let item = listed(core, found, &places, &words, "en", false);
            (found.id.clone(), item)
        })
        .collect();
    for found in places.read_removed() {
        if let Entry::Vacant(slot) = items.entry(found.id.clone()) {
            slot.insert(listed(core, &found, &places, &words, "en", true));
        }
    }
    items
}

/// 前后一比，多了的、少了的、样子变了的，各报一声。
pub(super) fn announce(
    core: &Core,
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) {
    let gone = before.keys().filter(|id| !after.contains_key(*id));
    let differs = after
        .iter()
        .filter(|(id, item)| before.get(*id) != Some(item))
        .map(|(id, _)| id);
    for id in gone.chain(differs) {
        core.package_changes.notify(id);
    }
}
